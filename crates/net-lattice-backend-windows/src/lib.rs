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

use net_lattice_core::{Error, PlatformErrorCode, Result};
use net_lattice_model::route::{Route, RouteId};
use net_lattice_model::{IpAddress, Network};
use net_lattice_platform::RouteProvider;
use windows::Win32::NetworkManagement::IpHelper::{
    CreateIpForwardEntry2, DeleteIpForwardEntry2, FreeMibTable, GetIpForwardTable2,
    InitializeIpForwardEntry, MIB_IPFORWARD_ROW2, MIB_IPFORWARD_TABLE2,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

/// The Windows IP Helper API-backed implementation of Net Lattice's provider
/// traits.
pub struct WindowsBackend {
    runtime: tokio::runtime::Runtime,
}

impl WindowsBackend {
    pub fn new() -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new().map_err(windows_error_code)?;
        Ok(Self { runtime })
    }
}

fn windows_error_code(err: std::io::Error) -> PlatformErrorCode {
    PlatformErrorCode::Windows(0)
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

fn row_to_route(row: &MIB_IPFORWARD_ROW2) -> Result<Option<Route>> {
    let destination = match row.DestinationPrefix.PrefixLength {
        32 => {
            let addr = row.DestinationPrefix.Prefix.Ipv4;
            let octets = [
                addr.S_un.S_un_w.s_w1,
                addr.S_un.S_un_w.s_w2,
                addr.S_un.S_un_w.s_w3,
                addr.S_un.S_un_w.s_w4,
            ];
            let ipv4 = net_lattice_ip::Ipv4Address::new(octets[0], octets[1], octets[2], octets[3]);
            let prefix =
                net_lattice_ip::Ipv4PrefixLength::new(row.DestinationPrefix.PrefixLength).ok()?;
            Network::from(net_lattice_ip::Ipv4Network::new(ipv4, prefix))
        }
        128 => {
            let bytes = row.DestinationPrefix.Prefix.Ipv6.s6_bytes;
            let octets: [u8; 16] = bytes;
            let ipv6 = net_lattice_ip::Ipv6Address::from(octets);
            let prefix =
                net_lattice_ip::Ipv6PrefixLength::new(row.DestinationPrefix.PrefixLength).ok()?;
            Network::from(net_lattice_ip::Ipv6Network::new(ipv6, prefix))
        }
        _ => return None,
    };

    let gateway = if row.NextHop.si_family != 0 {
        let gw = match row.NextHop.si_family {
            AF_INET => {
                let addr = row.NextHop.Ipv4;
                let octets = [
                    addr.S_un.S_un_w.s_w1,
                    addr.S_un.S_un_w.s_w2,
                    addr.S_un.S_un_w.s_w3,
                    addr.S_un.S_un_w.s_w4,
                ];
                IpAddr::V4(std::net::Ipv4Addr::new(
                    octets[0], octets[1], octets[2], octets[3],
                ))
            }
            AF_INET6 => {
                let bytes = row.NextHop.Ipv6.s6_bytes;
                IpAddr::V6(std::net::Ipv6Addr::from(bytes))
            }
            _ => IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
        };
        Some(net_lattice_model::IpAddress::from(match gw {
            IpAddr::V4(addr) => net_lattice_ip::Ipv4Address::from(addr).into(),
            IpAddr::V6(addr) => net_lattice_ip::Ipv6Address::from(addr).into(),
        }))
    } else {
        None
    };

    let metric = if row.Metric1 == u32::MAX {
        None
    } else {
        Some(row.Metric1)
    };

    let interface_index = if row.InterfaceIndex == u32::MAX {
        None
    } else {
        Some(row.InterfaceIndex)
    };

    let id = synthesize_route_id(&destination, &gateway, interface_index);

    let mut route = Route::new(id, destination);
    if let Some(gateway) = gateway {
        route = route.with_gateway(gateway);
    }
    if let Some(metric) = metric {
        route = route.with_metric(metric);
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

            let table_v4 = ip_forward_table(AF_INET as u16).await?;
            unsafe {
                let rows =
                    std::slice::from_raw_parts((*table_v4).Table, (*table_v4).NumEntries as usize);
                for row in rows {
                    if let Some(route) = row_to_route(row)? {
                        routes.push(route);
                    }
                }
            }
            free_table(table_v4);

            let table_v6 = ip_forward_table(AF_INET6 as u16).await?;
            unsafe {
                let rows =
                    std::slice::from_raw_parts((*table_v6).Table, (*table_v6).NumEntries as usize);
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
                if status != 0 {
                    return Err(Error::Platform(PlatformErrorCode::Windows(status as u32)));
                }
            }
            Ok(())
        })
    }

    fn remove_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(async move {
            let (address_family, _) = match route.destination {
                Network::V4(_) => (AF_INET as u16, 32u8),
                Network::V6(_) => (AF_INET6 as u16, 128u8),
            };

            let table = ip_forward_table(address_family).await?;
            unsafe {
                let rows = std::slice::from_raw_parts((*table).Table, (*table).NumEntries as usize);
                let mut found = false;
                for row in rows {
                    if row.InterfaceIndex == route.interface_index.unwrap_or(0) {
                        if let Ok(Some(existing)) = row_to_route(row) {
                            if existing.destination == route.destination {
                                let status = DeleteIpForwardEntry2(row);
                                if status != 0 {
                                    free_table(table);
                                    return Err(Error::Platform(PlatformErrorCode::Windows(
                                        status as u32,
                                    )));
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

async fn ip_forward_table(address_family: u16) -> Result<*mut MIB_IPFORWARD_TABLE2> {
    unsafe {
        let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        let status = GetIpForwardTable2(address_family, &mut table);
        if status != 0 {
            return Err(Error::Platform(PlatformErrorCode::Windows(status as u32)));
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
    let (mut row, _) = match route.destination {
        Network::V4(ref net) => {
            let octets = net.address().as_bytes();
            let mut in_addr = windows::Win32::Networking::WinSock::IN_ADDR::default();
            in_addr.S_un.S_un_w.s_w1 = octets[0] as u16;
            in_addr.S_un.S_un_w.s_w2 = octets[1] as u16;
            in_addr.S_un.S_un_w.s_w3 = octets[2] as u16;
            in_addr.S_un.S_un_w.s_w4 = octets[3] as u16;

            let gateway = route.gateway.map(|gw| match gw {
                IpAddress::V4(ref addr) => {
                    let octets = addr.as_bytes();
                    let mut gw_in = windows::Win32::Networking::WinSock::IN_ADDR::default();
                    gw_in.S_un.S_un_w.s_w1 = octets[0] as u16;
                    gw_in.S_un.S_un_w.s_w2 = octets[1] as u16;
                    gw_in.S_un.S_un_w.s_w3 = octets[2] as u16;
                    gw_in.S_un.S_un_w.s_w4 = octets[3] as u16;
                    windows::Win32::NetworkManagement::IpHelper::MIB_IPADDRESS_STRING {
                        si_family: AF_INET as u16,
                        Ipv4: gw_in,
                    }
                }
                IpAddress::V6(ref addr) => {
                    let bytes = addr.as_bytes();
                    let mut gw_in6 = windows::Win32::Networking::WinSock::IN6_ADDR::default();
                    gw_in6.u.Byte = bytes;
                    windows::Win32::NetworkManagement::IpHelper::MIB_IPADDRESS_STRING {
                        si_family: AF_INET6 as u16,
                        Ipv6: gw_in6,
                    }
                }
            });

            let mut r = MIB_IPFORWARD_ROW2::default();
            unsafe { InitializeIpForwardEntry(&mut r) };
            r.DestinationPrefix.PrefixLength = net.prefix().value() as u8;
            r.DestinationPrefix.Prefix.Ipv4 = in_addr;
            r.NextHop = gateway.unwrap_or(
                windows::Win32::NetworkManagement::IpHelper::MIB_IPADDRESS_STRING {
                    si_family: 0,
                    Ipv4: windows::Win32::Networking::WinSock::IN_ADDR::default(),
                },
            );
            r.InterfaceIndex = route.interface_index.unwrap_or(0);
            if let Some(metric) = route.metric {
                r.Metric1 = metric;
            }
            (r, AF_INET as u16)
        }
        Network::V6(ref net) => {
            let bytes = net.address().as_bytes();
            let mut in6_addr = windows::Win32::Networking::WinSock::IN6_ADDR::default();
            in6_addr.u.Byte = bytes;

            let gateway = route.gateway.map(|gw| match gw {
                IpAddress::V4(ref addr) => {
                    let octets = addr.as_bytes();
                    let mut gw_in = windows::Win32::Networking::WinSock::IN_ADDR::default();
                    gw_in.S_un.S_un_w.s_w1 = octets[0] as u16;
                    gw_in.S_un.S_un_w.s_w2 = octets[1] as u16;
                    gw_in.S_un.S_un_w.s_w3 = octets[2] as u16;
                    gw_in.S_un.S_un_w.s_w4 = octets[3] as u16;
                    windows::Win32::NetworkManagement::IpHelper::MIB_IPADDRESS_STRING {
                        si_family: AF_INET as u16,
                        Ipv4: gw_in,
                    }
                }
                IpAddress::V6(ref addr) => {
                    let bytes = addr.as_bytes();
                    let mut gw_in6 = windows::Win32::Networking::WinSock::IN6_ADDR::default();
                    gw_in6.u.Byte = bytes;
                    windows::Win32::NetworkManagement::IpHelper::MIB_IPADDRESS_STRING {
                        si_family: AF_INET6 as u16,
                        Ipv6: gw_in6,
                    }
                }
            });

            let mut r = MIB_IPFORWARD_ROW2::default();
            unsafe { InitializeIpForwardEntry(&mut r) };
            r.DestinationPrefix.PrefixLength = net.prefix().value() as u8;
            r.DestinationPrefix.Prefix.Ipv6 = in6_addr;
            r.NextHop = gateway.unwrap_or(
                windows::Win32::NetworkManagement::IpHelper::MIB_IPADDRESS_STRING {
                    si_family: 0,
                    Ipv4: windows::Win32::Networking::WinSock::IN_ADDR::default(),
                },
            );
            r.InterfaceIndex = route.interface_index.unwrap_or(0);
            if let Some(metric) = route.metric {
                r.Metric1 = metric;
            }
            (r, AF_INET6 as u16)
        }
    };
    row
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
    fn routes_reads_the_real_windows_routing_table() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let routes = backend
            .routes()
            .expect("GetIpForwardTable dump should not require privilege");
        // Not asserting on contents: the routing table of the machine
        // running this test is arbitrary. Reaching here without an error is
        // the assertion.
        let _ = routes;
    }

    /// Requires `Administrator` privileges. Not run by default because most
    /// development and CI environments don't grant it, and this test would
    /// otherwise fail with `PermissionDenied` rather than being skipped.
    ///
    /// Uses a documentation-only prefix (RFC 5737 `203.0.113.0/24`,
    /// TEST-NET-3) on `lo` so it can't collide with or disrupt real
    /// routing, and removes what it added regardless of assertion outcome.
    #[test]
    #[ignore = "requires Administrator; run manually from elevated cmd/PowerShell on Windows"]
    fn add_then_remove_route_round_trips_through_the_kernel() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let loopback_index = 1u32;

        let destination = Network::from(Ipv4Network::new(
            Ipv4Address::new(203, 0, 113, 0),
            Ipv4PrefixLength::new(24).unwrap(),
        ));
        let route = Route::new(RouteId::new(0), destination).with_interface_index(loopback_index);

        let add_result = backend.add_route(route.clone());
        if matches!(
            add_result,
            Err(Error::PermissionDenied) | Err(Error::Platform(_))
        ) {
            add_result.expect("add_route failed - are you running as Administrator?");
        }

        let routes = backend
            .routes()
            .expect("routes() failed after add_route succeeded");
        let found = routes
            .iter()
            .any(|r| r.destination == destination && r.interface_index == Some(loopback_index));

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
                .any(|r| r.destination == destination && r.interface_index == Some(loopback_index)),
            "removed route was still present in routes() afterward"
        );
    }
}
