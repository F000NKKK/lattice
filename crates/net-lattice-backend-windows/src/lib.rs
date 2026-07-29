//! Windows backend for Net Lattice: implements `net-lattice-platform`'s provider
//! traits via the Windows IP Helper API.
//!
//! Only ever compiled for `target_os = "windows"` — its dependencies
//! (`windows`, Windows-only) are gated the same way in `Cargo.toml`. See
//! ARCHITECTURE.md for how this crate binds `net-lattice-platform`'s generic
//! `RouteProvider::Route` associated type to the concrete
//! `net_lattice_model::route::Route`.

#![cfg(target_os = "windows")]

use std::net::IpAddr;

use net_lattice_core::{Error, Id, PlatformErrorCode, Result};
use net_lattice_model::interface::{AdminState, Interface, InterfaceKind, OperationalState};
use net_lattice_model::mac::MacAddress;
use net_lattice_model::route::{Route, RouteId};
use net_lattice_model::{IpAddress, Network};
use net_lattice_platform::{InterfaceProvider, RouteProvider};
use windows::Win32::NetworkManagement::IpHelper::{
    CreateIpForwardEntry2, DeleteIpForwardEntry2, FreeMibTable, GetIfTable2, GetIpForwardTable2,
    InitializeIpForwardEntry, MIB_IF_ROW2, MIB_IF_TABLE2, MIB_IPFORWARD_ROW2, MIB_IPFORWARD_TABLE2,
};
use windows::Win32::NetworkManagement::Ndis::{
    IfOperStatusDormant, IfOperStatusDown, IfOperStatusLowerLayerDown, IfOperStatusUp,
    NET_IF_ADMIN_STATUS_UP,
};
use windows::Win32::Networking::WinSock::{
    ADDRESS_FAMILY, AF_INET, AF_INET6, IN_ADDR, IN_ADDR_0, IN_ADDR_0_0, IN6_ADDR, IN6_ADDR_0,
    SOCKADDR_IN, SOCKADDR_IN6, SOCKADDR_IN6_0, SOCKADDR_INET,
};

const AF_UNSPEC: ADDRESS_FAMILY = ADDRESS_FAMILY(0);

// IANA `ifType` values (RFC 2863), not exposed as named constants by the
// `windows` crate's `MIB_IF_ROW2::Type` binding.
const IF_TYPE_ETHERNET_CSMACD: u32 = 6;
const IF_TYPE_PPP: u32 = 23;
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const IF_TYPE_IEEE80211: u32 = 71;
const IF_TYPE_L2_VLAN: u32 = 135;
const IF_TYPE_BRIDGE: u32 = 209;

/// The Windows IP Helper API-backed implementation of Net Lattice's provider
/// traits.
pub struct WindowsBackend {
    runtime: tokio::runtime::Runtime,
}

impl WindowsBackend {
    pub fn new() -> Result<Self> {
        let runtime =
            tokio::runtime::Runtime::new().map_err(|err| Error::Platform(io_error_code(&err)))?;
        Ok(Self { runtime })
    }
}

fn io_error_code(err: &std::io::Error) -> PlatformErrorCode {
    PlatformErrorCode::Windows(err.raw_os_error().unwrap_or(0) as u32)
}

/// Placeholder identity scheme: a route has no kernel-assigned numeric ID,
/// so this hashes its defining fields. See ARCHITECTURE.md's open Object
/// Identity question — this is not a long-term answer, only enough to give
/// `Stage 0.2` a `RouteId` to work with.
///
/// Hashes destination, gateway, and outgoing interface together so that
/// two routes to the same destination that differ only in gateway or
/// interface (a common case with multiple default routes, or ECMP-like
/// setups) don't collide on the same `RouteId`.
fn synthesize_route_id(
    destination: &Network,
    gateway: &Option<IpAddress>,
    interface_index: Option<u32>,
) -> RouteId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    destination.hash(&mut hasher);
    gateway.hash(&mut hasher);
    interface_index.hash(&mut hasher);
    RouteId::new(hasher.finish())
}

fn std_ip_to_ip_address(addr: IpAddr) -> IpAddress {
    match addr {
        IpAddr::V4(addr) => IpAddress::from(net_lattice_ip::Ipv4Address::from(addr)),
        IpAddr::V6(addr) => IpAddress::from(net_lattice_ip::Ipv6Address::from(addr)),
    }
}

fn ip_address_to_std(address: IpAddress) -> IpAddr {
    match address {
        IpAddress::V4(addr) => IpAddr::V4(addr.into()),
        IpAddress::V6(addr) => IpAddr::V6(addr.into()),
    }
}

fn network_to_std(network: Network) -> (IpAddr, u8) {
    match network {
        Network::V4(net) => (IpAddr::V4(net.address().into()), net.prefix().value()),
        Network::V6(net) => (IpAddr::V6(net.address().into()), net.prefix().value()),
    }
}

/// Reads the address out of a `SOCKADDR_INET` union, dispatching on its
/// `si_family` tag. Returns `None` for `AF_UNSPEC` (used by `NextHop` to mean
/// "no gateway, on-link route").
///
/// # Safety
/// `addr` must be a validly initialized `SOCKADDR_INET` (true for anything
/// returned by `GetIpForwardTable2`/`GetIfTable2`).
unsafe fn sockaddr_inet_to_ip(addr: &SOCKADDR_INET) -> Option<IpAddr> {
    match unsafe { addr.si_family } {
        AF_INET => {
            let sin = unsafe { addr.Ipv4 };
            let b = unsafe { sin.sin_addr.S_un.S_un_b };
            Some(IpAddr::V4(std::net::Ipv4Addr::new(
                b.s_b1, b.s_b2, b.s_b3, b.s_b4,
            )))
        }
        AF_INET6 => {
            let sin6 = unsafe { addr.Ipv6 };
            let bytes = unsafe { sin6.sin6_addr.u.Byte };
            Some(IpAddr::V6(std::net::Ipv6Addr::from(bytes)))
        }
        _ => None,
    }
}

/// Builds a `SOCKADDR_INET` for `addr`, tagged with the matching address
/// family. All other fields (port, flow info, scope) are zeroed — routing
/// tables don't use them.
fn ip_to_sockaddr_inet(addr: IpAddr) -> SOCKADDR_INET {
    match addr {
        IpAddr::V4(addr) => {
            let [b1, b2, b3, b4] = addr.octets();
            let in_addr = IN_ADDR {
                S_un: IN_ADDR_0 {
                    S_un_b: IN_ADDR_0_0 {
                        s_b1: b1,
                        s_b2: b2,
                        s_b3: b3,
                        s_b4: b4,
                    },
                },
            };
            SOCKADDR_INET {
                Ipv4: SOCKADDR_IN {
                    sin_family: AF_INET,
                    sin_port: 0,
                    sin_addr: in_addr,
                    sin_zero: [0; 8],
                },
            }
        }
        IpAddr::V6(addr) => {
            let in6_addr = IN6_ADDR {
                u: IN6_ADDR_0 {
                    Byte: addr.octets(),
                },
            };
            SOCKADDR_INET {
                Ipv6: SOCKADDR_IN6 {
                    sin6_family: AF_INET6,
                    sin6_port: 0,
                    sin6_flowinfo: 0,
                    sin6_addr: in6_addr,
                    Anonymous: SOCKADDR_IN6_0 { sin6_scope_id: 0 },
                },
            }
        }
    }
}

fn row_to_route(row: &MIB_IPFORWARD_ROW2) -> Result<Option<Route>> {
    let Some(destination_addr) = (unsafe { sockaddr_inet_to_ip(&row.DestinationPrefix.Prefix) })
    else {
        return Ok(None);
    };
    let prefix_len = row.DestinationPrefix.PrefixLength;

    let destination = match destination_addr {
        IpAddr::V4(addr) => {
            let Some(prefix) = net_lattice_ip::Ipv4PrefixLength::new(prefix_len) else {
                return Ok(None);
            };
            Network::from(net_lattice_ip::Ipv4Network::new(addr.into(), prefix))
        }
        IpAddr::V6(addr) => {
            let Some(prefix) = net_lattice_ip::Ipv6PrefixLength::new(prefix_len) else {
                return Ok(None);
            };
            Network::from(net_lattice_ip::Ipv6Network::new(addr.into(), prefix))
        }
    };

    let gateway = unsafe { sockaddr_inet_to_ip(&row.NextHop) }.map(std_ip_to_ip_address);

    let interface_index = if row.InterfaceIndex != 0 {
        Some(row.InterfaceIndex)
    } else {
        None
    };

    let id = synthesize_route_id(&destination, &gateway, interface_index);

    let mut route = Route::new(id, destination).with_metric(row.Metric);
    if let Some(gateway) = gateway {
        route = route.with_gateway(gateway);
    }
    if let Some(interface_index) = interface_index {
        route = route.with_interface_index(interface_index);
    }
    Ok(Some(route))
}

impl RouteProvider for WindowsBackend {
    type Route = Route;

    fn routes(&self) -> Result<Vec<Self::Route>> {
        self.runtime.block_on(async {
            let mut routes = Vec::new();

            let table_v4 = ip_forward_table(AF_INET).await?;
            unsafe {
                let rows = std::slice::from_raw_parts(
                    (*table_v4).Table.as_ptr(),
                    (*table_v4).NumEntries as usize,
                );
                for row in rows {
                    if let Some(route) = row_to_route(row)? {
                        routes.push(route);
                    }
                }
            }
            free_table(table_v4);

            let table_v6 = ip_forward_table(AF_INET6).await?;
            unsafe {
                let rows = std::slice::from_raw_parts(
                    (*table_v6).Table.as_ptr(),
                    (*table_v6).NumEntries as usize,
                );
                for row in rows {
                    if let Some(route) = row_to_route(row)? {
                        routes.push(route);
                    }
                }
            }
            free_table(table_v6);

            Ok(routes)
        })
    }

    fn add_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(async move {
            let row = build_row(route);
            unsafe {
                let status = CreateIpForwardEntry2(&row);
                if status.0 != 0 {
                    return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
                }
            }
            Ok(())
        })
    }

    fn remove_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(async move {
            let address_family = match route.destination {
                Network::V4(_) => AF_INET,
                Network::V6(_) => AF_INET6,
            };

            let table = ip_forward_table(address_family).await?;
            unsafe {
                let rows = std::slice::from_raw_parts(
                    (*table).Table.as_ptr(),
                    (*table).NumEntries as usize,
                );
                let mut found = false;
                for row in rows {
                    if row.InterfaceIndex == route.interface_index.unwrap_or(0)
                        && let Ok(Some(existing)) = row_to_route(row)
                    {
                        if existing.destination == route.destination {
                            let status = DeleteIpForwardEntry2(row);
                            if status.0 != 0 {
                                free_table(table);
                                return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
                            }
                            found = true;
                            break;
                            }
                        }
                    }
                }
                free_table(table);

                if !found {
                    return Err(Error::NotFound);
                }
            }
            Ok(())
        })
    }
}

async fn ip_forward_table(address_family: ADDRESS_FAMILY) -> Result<*mut MIB_IPFORWARD_TABLE2> {
    unsafe {
        let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        let status = GetIpForwardTable2(address_family, &mut table);
        if status.0 != 0 {
            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
        }
        Ok(table)
    }
}

fn free_table(table: *mut MIB_IPFORWARD_TABLE2) {
    if !table.is_null() {
        unsafe {
            FreeMibTable(table.cast());
        }
    }
}

fn build_row(route: Route) -> MIB_IPFORWARD_ROW2 {
    let (destination, prefix_len) = network_to_std(route.destination);

    let mut row = MIB_IPFORWARD_ROW2::default();
    unsafe { InitializeIpForwardEntry(&mut row) };

    row.DestinationPrefix.PrefixLength = prefix_len;
    row.DestinationPrefix.Prefix = ip_to_sockaddr_inet(destination);
    row.NextHop = match route.gateway.map(ip_address_to_std) {
        Some(gateway) => ip_to_sockaddr_inet(gateway),
        None => SOCKADDR_INET {
            si_family: AF_UNSPEC,
        },
    };
    row.InterfaceIndex = route.interface_index.unwrap_or(0);
    row.Metric = route.metric.unwrap_or(0);

    row
}

/// Maps an IANA `ifType` (RFC 2863, `MIB_IF_ROW2::Type`) to the
/// cross-platform [`InterfaceKind`]. Anything not covered falls back to
/// `Other`, carrying the raw type code for diagnostics.
fn if_type_to_kind(if_type: u32) -> InterfaceKind {
    match if_type {
        IF_TYPE_ETHERNET_CSMACD | IF_TYPE_L2_VLAN => InterfaceKind::Ethernet,
        IF_TYPE_SOFTWARE_LOOPBACK => InterfaceKind::Loopback,
        IF_TYPE_PPP => InterfaceKind::PointToPoint,
        IF_TYPE_IEEE80211 => InterfaceKind::Wireless,
        IF_TYPE_BRIDGE => InterfaceKind::Bridge,
        other => InterfaceKind::Other(other),
    }
}

fn row_to_interface(row: &MIB_IF_ROW2) -> Interface {
    let index = row.InterfaceIndex;
    let name = String::from_utf16_lossy(&row.Alias)
        .trim_end_matches('\0')
        .to_string();

    let mac = if row.PhysicalAddressLength == 6 {
        let mut octets = [0u8; 6];
        octets.copy_from_slice(&row.PhysicalAddress[..6]);
        Some(MacAddress::new(octets))
    } else {
        None
    };

    let admin_state = if row.AdminStatus == NET_IF_ADMIN_STATUS_UP {
        AdminState::Up
    } else {
        AdminState::Down
    };

    let operational_state = match row.OperStatus {
        s if s == IfOperStatusUp => OperationalState::Up,
        s if s == IfOperStatusDown => OperationalState::Down,
        s if s == IfOperStatusLowerLayerDown => OperationalState::Down,
        s if s == IfOperStatusDormant => OperationalState::NoCarrier,
        _ => OperationalState::Unknown,
    };

    let kind = if_type_to_kind(row.Type);

    let mut interface = Interface::new(Id::new(index as u64), index, name, kind)
        .with_admin_state(admin_state)
        .with_operational_state(operational_state)
        .with_mtu(row.Mtu);
    if let Some(mac) = mac {
        interface = interface.with_mac(mac);
    }
    interface
}

impl InterfaceProvider for WindowsBackend {
    type Interface = Interface;

    fn interfaces(&self) -> Result<Vec<Self::Interface>> {
        self.runtime.block_on(async {
            let mut interfaces = Vec::new();
            unsafe {
                let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
                let status = GetIfTable2(&mut table);
                if status.0 != 0 {
                    return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
                }
                let rows = std::slice::from_raw_parts(
                    (*table).Table.as_ptr(),
                    (*table).NumEntries as usize,
                );
                for row in rows {
                    interfaces.push(row_to_interface(row));
                }
                FreeMibTable(table.cast());
            }
            Ok(interfaces)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_lattice_ip::{Ipv4Address, Ipv4Network, Ipv4PrefixLength};

    /// Exercises a real round trip through the IP Helper API, no privilege
    /// required: routing table dumps are readable by any user. This is the
    /// one test in this module that runs by default and actually proves the
    /// backend talks to the kernel, rather than only exercising conversion
    /// logic.
    #[test]
    fn routes_reads_the_real_kernel_routing_table() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let routes = backend
            .routes()
            .expect("GetIpForwardTable dump should not require privilege");
        // Not asserting on contents: the routing table of the machine
        // running this test is arbitrary. Reaching here without an error is
        // the assertion.
        let _ = routes;
    }

    /// Requires `Administrator` privileges (root, or `sudo -E cargo test -- --ignored`
    /// in this crate). Not run by default because most development and CI
    /// environments — including the one this crate was originally written
    /// in — don't grant it, and this test would otherwise fail with
    /// `PermissionDenied` rather than being skipped.
    ///
    /// Uses a documentation-only prefix (RFC 5737 `203.0.113.0/24`,
    /// TEST-NET-3) on `lo` so it can't collide with or disrupt real
    /// routing, and removes what it added regardless of assertion outcome.
    #[test]
    #[ignore = "requires Administrator; run manually from elevated cmd/PowerShell on Windows"]
    fn add_then_remove_route_round_trips_through_the_kernel() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let interface_index = 1u32;

        let destination = Network::from(Ipv4Network::new(
            Ipv4Address::new(203, 0, 113, 0),
            Ipv4PrefixLength::new(24).unwrap(),
        ));
        let route = Route::new(RouteId::new(0), destination).with_interface_index(interface_index);

        let add_result = backend.add_route(route.clone());
        if matches!(
            add_result,
            Err(Error::PermissionDenied) | Err(Error::Platform(_))
        ) {
            // Best effort even under #[ignore]: if it's run without the
            // capability after all, fail loudly rather than silently
            // passing on a no-op.
            add_result.expect("add_route failed - are you running as Administrator?");
        }

        let routes = backend
            .routes()
            .expect("routes() failed after add_route succeeded");
        let found = routes
            .iter()
            .any(|r| r.destination == destination && r.interface_index == Some(interface_index));

        // Clean up before asserting, so a failed assertion doesn't leave
        // the test route behind on the machine that ran this.
        let _ = backend.remove_route(route);

        assert!(found, "added route was not present in routes() afterward");

        let routes_after_removal = backend
            .routes()
            .expect("routes() failed after remove_route");
        assert!(
            !routes_after_removal
                .iter()
                .any(|r| r.destination == destination && r.interface_index == Some(interface_index)),
            "removed route was still present in routes() afterward"
        );
    }
}
