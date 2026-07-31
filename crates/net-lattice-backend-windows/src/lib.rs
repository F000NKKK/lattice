//! Windows backend for Net Lattice: implements `net-lattice-platform`'s provider
//! traits via the Windows IP Helper API.
//!
//! Only ever compiled for `target_os = "windows"` — its dependencies
//! (`windows`, Windows-only) are gated the same way in `Cargo.toml`. See
//! ARCHITECTURE.md for how this crate binds `net-lattice-platform`'s generic
//! `RouteProvider::Route` associated type to the concrete
//! `net_lattice_model::route::Route`.

#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::net::IpAddr;

use net_lattice_core::{Error, Id, PlatformErrorCode, Result};
use net_lattice_model::dns::{DnsConfig, NewDnsConfig};
use net_lattice_model::event::{ChangeKind, Event, EventFilter};
use net_lattice_model::ifaddr::{InterfaceAddress, InterfaceAddressId, NewInterfaceAddress};
use net_lattice_model::interface::{AdminState, Interface, InterfaceKind, OperationalState};
use net_lattice_model::mac::MacAddress;
use net_lattice_model::neighbor::{NeighborEntry, NeighborId, NeighborState};
use net_lattice_model::route::{Route, RouteId};
use net_lattice_model::{IpAddress, Network};
use net_lattice_platform::{
    AddressMutator, AddressProvider, Capability, CapabilityProvider, DnsMutator, DnsProvider,
    EventProvider, EventReceiver, EventSender, InterfaceProvider, NeighborProvider, RouteProvider,
};
#[cfg(feature = "async")]
use net_lattice_platform::{TokioEventProvider, TokioEventReceiver, TokioEventSender};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::IpHelper::{
    CancelMibChangeNotify2, CreateIpForwardEntry2, CreateUnicastIpAddressEntry,
    DNS_INTERFACE_SETTINGS, DNS_INTERFACE_SETTINGS_VERSION1, DNS_SETTING_NAMESERVER,
    DNS_SETTING_SEARCHLIST, DNS_SETTINGS, DNS_SETTINGS_VERSION1, DeleteIpForwardEntry2,
    DeleteUnicastIpAddressEntry, FreeMibTable, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_FRIENDLY_NAME,
    GAA_FLAG_SKIP_MULTICAST, GAA_FLAG_SKIP_UNICAST, GetAdaptersAddresses, GetIfTable2,
    GetIpForwardTable2, GetIpNetTable2, GetUnicastIpAddressTable, IP_ADAPTER_ADDRESSES_LH,
    InitializeIpForwardEntry, InitializeUnicastIpAddressEntry, MIB_IF_ROW2, MIB_IF_TABLE2,
    MIB_IPFORWARD_ROW2, MIB_IPFORWARD_TABLE2, MIB_IPNET_ROW2, MIB_IPNET_TABLE2,
    MIB_NOTIFICATION_TYPE, MIB_UNICASTIPADDRESS_ROW, MIB_UNICASTIPADDRESS_TABLE, MibAddInstance,
    MibDeleteInstance, NotifyIpInterfaceChange, NotifyRouteChange2, NotifyUnicastIpAddressChange,
    SetDnsSettings, SetInterfaceDnsSettings,
};
use windows::Win32::NetworkManagement::Ndis::{
    IfOperStatusDormant, IfOperStatusDown, IfOperStatusLowerLayerDown, IfOperStatusUp,
    NET_IF_ADMIN_STATUS_UP,
};
use windows::Win32::Networking::WinSock::{
    ADDRESS_FAMILY, AF_INET, AF_INET6, IN_ADDR, IN_ADDR_0, IN_ADDR_0_0, IN6_ADDR, IN6_ADDR_0,
    NL_NEIGHBOR_STATE, NlnsDelay, NlnsIncomplete, NlnsPermanent, NlnsProbe, NlnsReachable,
    NlnsStale, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6, SOCKADDR_IN6_0, SOCKADDR_INET,
};
use windows::core::{GUID, PWSTR};

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

fn nul_terminated_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn config_list(config: &[impl std::fmt::Display]) -> String {
    config
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn adapter_guid(adapter: &IP_ADAPTER_ADDRESSES_LH) -> Result<GUID> {
    let adapter_name =
        unsafe { adapter.AdapterName.to_string() }.map_err(|_| Error::InvalidState)?;
    GUID::try_from(adapter_name.trim_matches(|character| character == '{' || character == '}'))
        .map_err(|_| Error::InvalidState)
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
                        && existing.destination == route.destination
                    {
                        let status = DeleteIpForwardEntry2(row);
                        if status.0 != 0 {
                            free_table(table);
                            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
                        }
                        found = true;
                        break;
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

/// Placeholder identity scheme, same rationale as `synthesize_route_id`: an
/// interface address has no kernel-assigned numeric ID, so this hashes its
/// interface and network together.
fn synthesize_interface_address_id(interface_index: u32, network: &Network) -> InterfaceAddressId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    interface_index.hash(&mut hasher);
    network.hash(&mut hasher);
    InterfaceAddressId::new(hasher.finish())
}

fn row_to_interface_address(row: &MIB_UNICASTIPADDRESS_ROW) -> Option<InterfaceAddress> {
    let address_addr = unsafe { sockaddr_inet_to_ip(&row.Address) }?;
    let prefix_len = row.OnLinkPrefixLength;

    let network = match address_addr {
        IpAddr::V4(addr) => {
            let prefix = net_lattice_ip::Ipv4PrefixLength::new(prefix_len)?;
            Network::from(net_lattice_ip::Ipv4Network::new(addr.into(), prefix))
        }
        IpAddr::V6(addr) => {
            let prefix = net_lattice_ip::Ipv6PrefixLength::new(prefix_len)?;
            Network::from(net_lattice_ip::Ipv6Network::new(addr.into(), prefix))
        }
    };

    Some(InterfaceAddress::new(
        synthesize_interface_address_id(row.InterfaceIndex, &network),
        row.InterfaceIndex,
        network,
    ))
}

impl AddressProvider for WindowsBackend {
    type InterfaceAddress = InterfaceAddress;

    fn addresses(&self) -> Result<Vec<Self::InterfaceAddress>> {
        self.runtime.block_on(async {
            let mut table: *mut MIB_UNICASTIPADDRESS_TABLE = std::ptr::null_mut();
            let status = unsafe { GetUnicastIpAddressTable(AF_UNSPEC, &mut table) };
            if status.0 != 0 {
                return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
            }

            let addresses = unsafe {
                let rows = std::slice::from_raw_parts(
                    (*table).Table.as_ptr(),
                    (*table).NumEntries as usize,
                );
                rows.iter().filter_map(row_to_interface_address).collect()
            };
            unsafe { FreeMibTable(table.cast()) };
            Ok(addresses)
        })
    }
}

fn build_unicast_address_row(address: &NewInterfaceAddress) -> MIB_UNICASTIPADDRESS_ROW {
    let (ip, prefix_len) = network_to_std(address.address);
    let mut row = MIB_UNICASTIPADDRESS_ROW::default();
    unsafe { InitializeUnicastIpAddressEntry(&mut row) };
    row.InterfaceIndex = address.interface_id.value() as u32;
    row.Address = ip_to_sockaddr_inet(ip);
    row.OnLinkPrefixLength = prefix_len;
    row
}

impl AddressMutator for WindowsBackend {
    type NewInterfaceAddress = NewInterfaceAddress;
    type InterfaceAddress = InterfaceAddress;

    fn add_address(&self, address: Self::NewInterfaceAddress) -> Result<Self::InterfaceAddress> {
        if matches!(address.address, Network::V6(_)) && address.broadcast.is_some() {
            return Err(Error::InvalidState);
        }
        // IP Helper derives IPv4 broadcast from the prefix and cannot accept
        // an override. Refuse one rather than silently discarding intent.
        if address.broadcast.is_some() {
            return Err(Error::Unsupported);
        }
        let row = build_unicast_address_row(&address);
        let status = unsafe { CreateUnicastIpAddressEntry(&row) };
        if status.0 != 0 {
            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
        }
        self.addresses()?
            .into_iter()
            .find(|observed| {
                observed.interface_index == row.InterfaceIndex
                    && observed.address == address.address
            })
            .ok_or(Error::InvalidState)
    }

    fn remove_address(&self, address: Self::InterfaceAddress) -> Result<()> {
        let request =
            NewInterfaceAddress::new(Id::new(address.interface_index as u64), address.address);
        let row = build_unicast_address_row(&request);
        let status = unsafe { DeleteUnicastIpAddressEntry(&row) };
        if status.0 != 0 {
            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
        }
        Ok(())
    }
}

/// Reads the address out of a raw `SOCKADDR`, dispatching on its
/// `sa_family` — used for `GetAdaptersAddresses`'s DNS server entries,
/// which point at a `sockaddr_in`/`sockaddr_in6`-sized buffer directly
/// rather than embedding a `SOCKADDR_INET` union like the routing APIs do.
///
/// # Safety
/// `ptr`, if non-null, must point at a validly initialized
/// `sockaddr_in`/`sockaddr_in6` for the family it claims (true for anything
/// returned by `GetAdaptersAddresses`).
unsafe fn sockaddr_ptr_to_ip(ptr: *const SOCKADDR) -> Option<IpAddr> {
    if ptr.is_null() {
        return None;
    }
    match unsafe { (*ptr).sa_family } {
        AF_INET => {
            let sin = unsafe { &*ptr.cast::<SOCKADDR_IN>() };
            let b = unsafe { sin.sin_addr.S_un.S_un_b };
            Some(IpAddr::V4(std::net::Ipv4Addr::new(
                b.s_b1, b.s_b2, b.s_b3, b.s_b4,
            )))
        }
        AF_INET6 => {
            let sin6 = unsafe { &*ptr.cast::<SOCKADDR_IN6>() };
            let bytes = unsafe { sin6.sin6_addr.u.Byte };
            Some(IpAddr::V6(std::net::Ipv6Addr::from(bytes)))
        }
        _ => None,
    }
}

/// Calls `GetAdaptersAddresses` with the standard two-call pattern (first
/// to learn the required buffer size, then to fill it) and returns the raw
/// buffer, since `IP_ADAPTER_ADDRESSES_LH` is a variable-length structure
/// chained via `Next` pointers into the buffer itself.
///
/// Skips unicast/anycast/multicast address enumeration via `GAA_FLAG_*`:
/// this backend only reads DNS servers and suffixes, which shrinks the
/// buffer considerably on machines with many addresses per adapter.
fn adapter_addresses() -> Result<Vec<u8>> {
    const FAMILY_UNSPEC: u32 = 0;
    let flags = GAA_FLAG_SKIP_UNICAST
        | GAA_FLAG_SKIP_ANYCAST
        | GAA_FLAG_SKIP_MULTICAST
        | GAA_FLAG_SKIP_FRIENDLY_NAME;

    let mut size: u32 = 0;
    unsafe {
        GetAdaptersAddresses(FAMILY_UNSPEC, flags, None, None, &mut size);
    }
    if size == 0 {
        return Ok(Vec::new());
    }

    let mut buffer = vec![0u8; size as usize];
    let status = unsafe {
        GetAdaptersAddresses(
            FAMILY_UNSPEC,
            flags,
            None,
            Some(buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>()),
            &mut size,
        )
    };
    if status != 0 {
        return Err(Error::Platform(PlatformErrorCode::Windows(status)));
    }
    Ok(buffer)
}

/// Placeholder identity scheme, same rationale as `synthesize_route_id`: a
/// neighbor entry has no kernel-assigned numeric ID, so this hashes its
/// interface and address together.
fn synthesize_neighbor_id(interface_index: u32, address: &IpAddress) -> NeighborId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    interface_index.hash(&mut hasher);
    address.hash(&mut hasher);
    NeighborId::new(hasher.finish())
}

/// Maps Windows `NL_NEIGHBOR_STATE` to the cross-platform [`NeighborState`].
fn neighbor_state_to_state(state: NL_NEIGHBOR_STATE) -> NeighborState {
    match state {
        s if s == NlnsIncomplete => NeighborState::Incomplete,
        s if s == NlnsReachable => NeighborState::Reachable,
        s if s == NlnsStale => NeighborState::Stale,
        s if s == NlnsDelay => NeighborState::Delay,
        s if s == NlnsProbe => NeighborState::Probe,
        s if s == NlnsPermanent => NeighborState::Permanent,
        _ => NeighborState::Unknown,
    }
}

fn row_to_neighbor(row: &MIB_IPNET_ROW2) -> Option<NeighborEntry> {
    let address = unsafe { sockaddr_inet_to_ip(&row.Address) }.map(std_ip_to_ip_address)?;

    let mac = if row.PhysicalAddressLength == 6 {
        let mut octets = [0u8; 6];
        octets.copy_from_slice(&row.PhysicalAddress[..6]);
        Some(MacAddress::new(octets))
    } else {
        None
    };

    let mut entry = NeighborEntry::new(
        synthesize_neighbor_id(row.InterfaceIndex, &address),
        row.InterfaceIndex,
        address,
    )
    .with_state(neighbor_state_to_state(row.State));
    if let Some(mac) = mac {
        entry = entry.with_mac(mac);
    }
    Some(entry)
}

impl NeighborProvider for WindowsBackend {
    type NeighborEntry = NeighborEntry;

    fn neighbors(&self) -> Result<Vec<Self::NeighborEntry>> {
        self.runtime.block_on(async {
            let mut table: *mut MIB_IPNET_TABLE2 = std::ptr::null_mut();
            let status = unsafe { GetIpNetTable2(AF_UNSPEC, &mut table) };
            if status.0 != 0 {
                return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
            }

            let neighbors = unsafe {
                let rows = std::slice::from_raw_parts(
                    (*table).Table.as_ptr(),
                    (*table).NumEntries as usize,
                );
                rows.iter().filter_map(row_to_neighbor).collect()
            };
            unsafe { FreeMibTable(table.cast()) };
            Ok(neighbors)
        })
    }
}

struct WindowsWatchState {
    sender: EventSender<Event>,
    filter: EventFilter,
}

#[cfg(feature = "async")]
struct WindowsTokioWatchState {
    sender: TokioEventSender<Event>,
    filter: EventFilter,
}

/// Owns all native notification handles and their callback context. IP Helper
/// guarantees `CancelMibChangeNotify2` waits for active callbacks, so freeing
/// `state` after cancellation cannot race a callback dereference.
struct WindowsWatch {
    state: *mut WindowsWatchState,
    route: HANDLE,
    interface: HANDLE,
    address: HANDLE,
}

unsafe impl Send for WindowsWatch {}

impl WindowsWatch {
    unsafe fn cancel(handle: HANDLE) {
        if !handle.is_invalid() {
            let _ = unsafe { CancelMibChangeNotify2(handle) };
        }
    }
}

impl Drop for WindowsWatch {
    fn drop(&mut self) {
        unsafe {
            Self::cancel(self.route);
            Self::cancel(self.interface);
            Self::cancel(self.address);
            drop(Box::from_raw(self.state));
        }
    }
}

/// Async counterpart to [`WindowsWatch`]. It owns the registration handles
/// and callback state for as long as the Tokio receiver remains alive.
#[cfg(feature = "async")]
struct WindowsTokioWatch {
    state: *mut WindowsTokioWatchState,
    route: HANDLE,
    interface: HANDLE,
    address: HANDLE,
}

#[cfg(feature = "async")]
unsafe impl Send for WindowsTokioWatch {}

#[cfg(feature = "async")]
impl Drop for WindowsTokioWatch {
    fn drop(&mut self) {
        unsafe {
            WindowsWatch::cancel(self.route);
            WindowsWatch::cancel(self.interface);
            WindowsWatch::cancel(self.address);
            drop(Box::from_raw(self.state));
        }
    }
}

fn change_kind(notification: MIB_NOTIFICATION_TYPE) -> ChangeKind {
    if notification == MibAddInstance {
        ChangeKind::Added
    } else if notification == MibDeleteInstance {
        ChangeKind::Removed
    } else {
        // MibParameterNotification is the common case. Treat the documented
        // initial notification the same way defensively; registrations below
        // request no initial snapshot.
        ChangeKind::Changed
    }
}

unsafe extern "system" fn route_change_callback(
    context: *const c_void,
    row: *const MIB_IPFORWARD_ROW2,
    notification: MIB_NOTIFICATION_TYPE,
) {
    if context.is_null() || row.is_null() {
        return;
    }
    let state = unsafe { &*(context.cast::<WindowsWatchState>()) };
    if let Ok(Some(route)) = row_to_route(unsafe { &*row }) {
        let event = Event::Route {
            id: route.id,
            kind: change_kind(notification),
        };
        if state.filter.matches(event) {
            let _ = state.sender.send(event, Event::resync_all());
        }
    }
}

unsafe extern "system" fn interface_change_callback(
    context: *const c_void,
    row: *const windows::Win32::NetworkManagement::IpHelper::MIB_IPINTERFACE_ROW,
    notification: MIB_NOTIFICATION_TYPE,
) {
    if context.is_null() || row.is_null() {
        return;
    }
    let state = unsafe { &*(context.cast::<WindowsWatchState>()) };
    let row = unsafe { &*row };
    let event = Event::Interface {
        id: Id::new(row.InterfaceIndex as u64),
        kind: change_kind(notification),
    };
    if state.filter.matches(event) {
        let _ = state.sender.send(event, Event::resync_all());
    }
}

unsafe extern "system" fn address_change_callback(
    context: *const c_void,
    row: *const MIB_UNICASTIPADDRESS_ROW,
    notification: MIB_NOTIFICATION_TYPE,
) {
    if context.is_null() || row.is_null() {
        return;
    }
    let state = unsafe { &*(context.cast::<WindowsWatchState>()) };
    if let Some(address) = row_to_interface_address(unsafe { &*row }) {
        let event = Event::Address {
            id: address.id,
            kind: change_kind(notification),
        };
        if state.filter.matches(event) {
            let _ = state.sender.send(event, Event::resync_all());
        }
    }
}

#[cfg(feature = "async")]
unsafe extern "system" fn tokio_route_change_callback(
    context: *const c_void,
    row: *const MIB_IPFORWARD_ROW2,
    notification: MIB_NOTIFICATION_TYPE,
) {
    if context.is_null() || row.is_null() {
        return;
    }
    let state = unsafe { &*(context.cast::<WindowsTokioWatchState>()) };
    if let Ok(Some(route)) = row_to_route(unsafe { &*row }) {
        let event = Event::Route {
            id: route.id,
            kind: change_kind(notification),
        };
        if state.filter.matches(event) {
            let _ = state.sender.send(event, Event::resync_all);
        }
    }
}

#[cfg(feature = "async")]
unsafe extern "system" fn tokio_interface_change_callback(
    context: *const c_void,
    row: *const windows::Win32::NetworkManagement::IpHelper::MIB_IPINTERFACE_ROW,
    notification: MIB_NOTIFICATION_TYPE,
) {
    if context.is_null() || row.is_null() {
        return;
    }
    let state = unsafe { &*(context.cast::<WindowsTokioWatchState>()) };
    let event = Event::Interface {
        id: Id::new(unsafe { (*row).InterfaceIndex } as u64),
        kind: change_kind(notification),
    };
    if state.filter.matches(event) {
        let _ = state.sender.send(event, Event::resync_all);
    }
}

#[cfg(feature = "async")]
unsafe extern "system" fn tokio_address_change_callback(
    context: *const c_void,
    row: *const MIB_UNICASTIPADDRESS_ROW,
    notification: MIB_NOTIFICATION_TYPE,
) {
    if context.is_null() || row.is_null() {
        return;
    }
    let state = unsafe { &*(context.cast::<WindowsTokioWatchState>()) };
    if let Some(address) = row_to_interface_address(unsafe { &*row }) {
        let event = Event::Address {
            id: address.id,
            kind: change_kind(notification),
        };
        if state.filter.matches(event) {
            let _ = state.sender.send(event, Event::resync_all);
        }
    }
}

impl CapabilityProvider for WindowsBackend {
    /// `IPV6` unconditionally, same rationale as the other backends: every
    /// provider this backend implements already handles both address
    /// families. `MONITORING` is available through IP Helper notification
    /// registrations. `VRF`/`NAMESPACES` remain unset because Net Lattice
    /// does not implement either domain yet.
    fn capabilities(&self) -> Capability {
        Capability::IPV6 | Capability::MONITORING | Capability::DNS_MUTATION
    }
}

impl EventProvider for WindowsBackend {
    type Event = Event;
    type EventFilter = EventFilter;

    fn watch(&self) -> Result<EventReceiver<Self::Event>> {
        self.watch_filtered(EventFilter::ALL)
    }
    fn watch_filtered(&self, filter: Self::EventFilter) -> Result<EventReceiver<Self::Event>> {
        let (sender, receiver) = EventReceiver::bounded();
        let state = Box::into_raw(Box::new(WindowsWatchState { sender, filter }));
        let mut route = HANDLE::default();
        let mut interface = HANDLE::default();
        let mut address = HANDLE::default();

        let status = unsafe {
            NotifyRouteChange2(
                AF_UNSPEC,
                Some(route_change_callback),
                state.cast(),
                false,
                &mut route,
            )
        };
        if status.0 != 0 {
            unsafe { drop(Box::from_raw(state)) };
            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
        }

        let status = unsafe {
            NotifyIpInterfaceChange(
                AF_UNSPEC,
                Some(interface_change_callback),
                Some(state.cast()),
                false,
                &mut interface,
            )
        };
        if status.0 != 0 {
            unsafe {
                WindowsWatch::cancel(route);
                drop(Box::from_raw(state));
            }
            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
        }

        let status = unsafe {
            NotifyUnicastIpAddressChange(
                AF_UNSPEC,
                Some(address_change_callback),
                Some(state.cast()),
                false,
                &mut address,
            )
        };
        if status.0 != 0 {
            unsafe {
                WindowsWatch::cancel(route);
                WindowsWatch::cancel(interface);
                drop(Box::from_raw(state));
            }
            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
        }

        Ok(receiver.with_subscription(WindowsWatch {
            state,
            route,
            interface,
            address,
        }))
    }
}

/// Native async monitoring: IP Helper invokes the callbacks directly and the
/// callbacks enqueue into a bounded Tokio transport without blocking a system
/// callback thread.
#[cfg(feature = "async")]
impl TokioEventProvider for WindowsBackend {
    type Event = Event;
    type EventFilter = EventFilter;

    fn watch_tokio(&self, filter: Self::EventFilter) -> Result<TokioEventReceiver<Self::Event>> {
        let (sender, receiver) = TokioEventReceiver::bounded();
        let state = Box::into_raw(Box::new(WindowsTokioWatchState { sender, filter }));
        let mut route = HANDLE::default();
        let mut interface = HANDLE::default();
        let mut address = HANDLE::default();
        let status = unsafe {
            NotifyRouteChange2(
                AF_UNSPEC,
                Some(tokio_route_change_callback),
                state.cast(),
                false,
                &mut route,
            )
        };
        if status.0 != 0 {
            unsafe {
                drop(Box::from_raw(state));
            }
            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
        }
        let status = unsafe {
            NotifyIpInterfaceChange(
                AF_UNSPEC,
                Some(tokio_interface_change_callback),
                Some(state.cast()),
                false,
                &mut interface,
            )
        };
        if status.0 != 0 {
            unsafe {
                WindowsWatch::cancel(route);
                drop(Box::from_raw(state));
            }
            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
        }
        let status = unsafe {
            NotifyUnicastIpAddressChange(
                AF_UNSPEC,
                Some(tokio_address_change_callback),
                Some(state.cast()),
                false,
                &mut address,
            )
        };
        if status.0 != 0 {
            unsafe {
                WindowsWatch::cancel(route);
                WindowsWatch::cancel(interface);
                drop(Box::from_raw(state));
            }
            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
        }
        Ok(receiver.with_subscription(WindowsTokioWatch {
            state,
            route,
            interface,
            address,
        }))
    }
}

impl DnsProvider for WindowsBackend {
    type DnsConfig = DnsConfig;

    fn dns_config(&self) -> Result<Self::DnsConfig> {
        let buffer = adapter_addresses()?;
        let mut config = DnsConfig::new();
        if buffer.is_empty() {
            return Ok(config);
        }

        // Every adapter can list the same DNS server / suffix (e.g. a
        // router advertised on both the wired and wireless adapter), so
        // dedupe across the whole machine rather than reporting duplicates.
        let mut seen_nameservers = std::collections::HashSet::new();
        let mut seen_suffixes = std::collections::HashSet::new();

        let mut adapter = buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        while !adapter.is_null() {
            unsafe {
                let mut dns_server = (*adapter).FirstDnsServerAddress;
                while !dns_server.is_null() {
                    let socket_address = (*dns_server).Address;
                    if let Some(ip) = sockaddr_ptr_to_ip(socket_address.lpSockaddr) {
                        let ip_address = std_ip_to_ip_address(ip);
                        if seen_nameservers.insert(ip_address) {
                            config.nameservers.push(ip_address);
                        }
                    }
                    dns_server = (*dns_server).Next;
                }

                if !(*adapter).DnsSuffix.is_null()
                    && let Ok(suffix) = (*adapter).DnsSuffix.to_string()
                    && !suffix.is_empty()
                    && seen_suffixes.insert(suffix.clone())
                {
                    config.search_domains.push(suffix);
                }

                adapter = (*adapter).Next;
            }
        }
        Ok(config)
    }
}

impl DnsMutator for WindowsBackend {
    type NewDnsConfig = NewDnsConfig;

    /// Uses the IP Helper DNS settings API rather than a command-line tool.
    /// Windows stores nameservers per adapter, so the system-wide model is
    /// applied consistently to every adapter returned by
    /// `GetAdaptersAddresses`; the global search list is updated too. Global
    /// and adapter settings are separate native calls, so a later adapter
    /// failure can leave an earlier update applied; callers must re-read
    /// [`DnsProvider::dns_config`] after an error. This operation does not
    /// produce a DNS watcher event.
    fn set_dns_config(&self, config: Self::NewDnsConfig) -> Result<Self::DnsConfig> {
        let nameservers = nul_terminated_wide(&config_list(&config.nameservers));
        let search_domains = nul_terminated_wide(&config.search_domains.join(","));

        let global = DNS_SETTINGS {
            Version: DNS_SETTINGS_VERSION1,
            Flags: DNS_SETTING_SEARCHLIST as u64,
            SearchList: PWSTR(search_domains.as_ptr().cast_mut()),
            ..Default::default()
        };
        let status = unsafe { SetDnsSettings(&global) };
        if status.0 != 0 {
            return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
        }

        let adapters = adapter_addresses()?;
        if adapters.is_empty() {
            return Err(Error::InvalidState);
        }
        let mut adapter = adapters.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        while !adapter.is_null() {
            let guid = unsafe { adapter_guid(&*adapter) }?;
            let settings = DNS_INTERFACE_SETTINGS {
                Version: DNS_INTERFACE_SETTINGS_VERSION1,
                Flags: (DNS_SETTING_NAMESERVER | DNS_SETTING_SEARCHLIST) as u64,
                NameServer: PWSTR(nameservers.as_ptr().cast_mut()),
                SearchList: PWSTR(search_domains.as_ptr().cast_mut()),
                ..Default::default()
            };
            let status = unsafe { SetInterfaceDnsSettings(guid, &settings) };
            if status.0 != 0 {
                return Err(Error::Platform(PlatformErrorCode::Windows(status.0)));
            }
            adapter = unsafe { (*adapter).Next };
        }
        self.dns_config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_lattice_ip::{Ipv4Address, Ipv4Network, Ipv4PrefixLength};

    #[cfg(feature = "async")]
    fn tokio_route_event(watcher: &mut TokioEventReceiver<Event>, id: RouteId) -> bool {
        use std::pin::Pin;
        use std::task::{Context, Poll, Waker};
        use std::time::Duration;

        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        for _ in 0..12 {
            match Pin::new(&mut *watcher).poll_recv(&mut context) {
                Poll::Ready(Some(Ok(Event::Route { id: event_id, .. }))) if event_id == id => {
                    return true;
                }
                Poll::Ready(None) => return false,
                Poll::Ready(Some(_)) | Poll::Pending => {
                    std::thread::sleep(Duration::from_millis(250))
                }
            }
        }
        false
    }

    #[test]
    fn ip_helper_row_fixtures_round_trip_all_read_models() {
        let destination = Network::from(Ipv4Network::new(
            Ipv4Address::new(198, 51, 100, 0),
            Ipv4PrefixLength::new(24).unwrap(),
        ));
        let route = Route::new(RouteId::new(1), destination)
            .with_gateway(IpAddress::from(Ipv4Address::new(192, 0, 2, 1)))
            .with_interface_index(7)
            .with_metric(42);
        let row = build_row(route);
        let observed = row_to_route(&row).unwrap().expect("valid route row");
        assert_eq!(observed.destination, destination);
        assert_eq!(observed.interface_index, Some(7));
        assert_eq!(observed.metric, Some(42));
        assert_eq!(
            observed.gateway,
            Some(IpAddress::from(Ipv4Address::new(192, 0, 2, 1)))
        );

        let address = MIB_UNICASTIPADDRESS_ROW {
            InterfaceIndex: 7,
            Address: ip_to_sockaddr_inet(IpAddr::V4(Ipv4Address::new(192, 0, 2, 7).into())),
            OnLinkPrefixLength: 24,
            ..Default::default()
        };
        let observed = row_to_interface_address(&address).expect("valid address row");
        assert_eq!(observed.interface_index, 7);
        assert_eq!(
            observed.address,
            Network::from(Ipv4Network::new(
                Ipv4Address::new(192, 0, 2, 7),
                Ipv4PrefixLength::new(24).unwrap(),
            ))
        );

        let mut interface = MIB_IF_ROW2 {
            InterfaceIndex: 7,
            Type: IF_TYPE_ETHERNET_CSMACD,
            AdminStatus: NET_IF_ADMIN_STATUS_UP,
            OperStatus: IfOperStatusUp,
            Mtu: 1500,
            ..Default::default()
        };
        interface.Alias[0] = 'e' as u16;
        interface.Alias[1] = 't' as u16;
        interface.Alias[2] = 'h' as u16;
        interface.PhysicalAddressLength = 6;
        interface.PhysicalAddress[..6].copy_from_slice(&[0, 1, 2, 3, 4, 5]);
        let observed = row_to_interface(&interface);
        assert_eq!(observed.name, "eth");
        assert_eq!(observed.kind, InterfaceKind::Ethernet);
        assert_eq!(observed.operational_state, OperationalState::Up);
        assert_eq!(observed.mac, Some(MacAddress::new([0, 1, 2, 3, 4, 5])));

        let mut neighbor = MIB_IPNET_ROW2 {
            InterfaceIndex: 7,
            Address: ip_to_sockaddr_inet(IpAddr::V4(Ipv4Address::new(192, 0, 2, 1).into())),
            State: NlnsReachable,
            PhysicalAddressLength: 6,
            ..Default::default()
        };
        neighbor.PhysicalAddress[..6].copy_from_slice(&[5, 4, 3, 2, 1, 0]);
        let observed = row_to_neighbor(&neighbor).expect("valid neighbor row");
        assert_eq!(observed.interface_index, 7);
        assert_eq!(observed.state, NeighborState::Reachable);
        assert_eq!(observed.mac, Some(MacAddress::new([5, 4, 3, 2, 1, 0])));
    }

    #[test]
    fn ip_helper_kind_and_state_mappings_cover_supported_values() {
        assert_eq!(
            if_type_to_kind(IF_TYPE_SOFTWARE_LOOPBACK),
            InterfaceKind::Loopback
        );
        assert_eq!(if_type_to_kind(IF_TYPE_PPP), InterfaceKind::PointToPoint);
        assert_eq!(if_type_to_kind(IF_TYPE_IEEE80211), InterfaceKind::Wireless);
        assert_eq!(if_type_to_kind(IF_TYPE_BRIDGE), InterfaceKind::Bridge);
        assert_eq!(if_type_to_kind(IF_TYPE_L2_VLAN), InterfaceKind::Ethernet);
        assert_eq!(if_type_to_kind(999), InterfaceKind::Other(999));
        assert_eq!(
            neighbor_state_to_state(NlnsIncomplete),
            NeighborState::Incomplete
        );
        assert_eq!(neighbor_state_to_state(NlnsStale), NeighborState::Stale);
        assert_eq!(neighbor_state_to_state(NlnsDelay), NeighborState::Delay);
        assert_eq!(neighbor_state_to_state(NlnsProbe), NeighborState::Probe);
        assert_eq!(
            neighbor_state_to_state(NlnsPermanent),
            NeighborState::Permanent
        );
    }

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

    /// Exercises a real round trip through `GetAdaptersAddresses`, no
    /// privilege required: every Windows system has a loopback adapter.
    #[test]
    fn interfaces_includes_the_loopback_interface() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let interfaces = backend
            .interfaces()
            .expect("GetAdaptersAddresses should not require privilege");
        assert!(
            interfaces
                .iter()
                .any(|interface| matches!(interface.kind, InterfaceKind::Loopback)),
            "expected a loopback interface, got: {interfaces:?}"
        );
    }

    /// Keeps the runtime capability advertisement aligned with the provider
    /// implementations exercised by this backend's native tests.
    #[test]
    fn capabilities_match_the_implemented_provider_surface() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let capabilities = backend.capabilities();
        assert!(capabilities.contains(Capability::IPV6));
        assert!(capabilities.contains(Capability::MONITORING));
        assert!(capabilities.contains(Capability::DNS_MUTATION));
    }

    /// Exercises a real round trip through `GetUnicastIpAddressTable`, no
    /// privilege required: every Windows system has a loopback address
    /// assigned.
    #[test]
    fn addresses_includes_loopbacks_address() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let addresses = backend
            .addresses()
            .expect("GetUnicastIpAddressTable dump should not require privilege");
        assert!(
            addresses.iter().any(|addr| matches!(
                addr.address,
                Network::V4(net) if net.address() == Ipv4Address::new(127, 0, 0, 1)
            )),
            "expected `127.0.0.1` among the assigned addresses, got: {addresses:?}"
        );
    }

    /// Exercises the complete IP Helper address-mutation path against the
    /// kernel: create a TEST-NET-1 address on the loopback adapter, read its
    /// canonical row, then delete that observed row.
    #[test]
    #[ignore = "requires Administrator; run from elevated cmd/PowerShell: cargo test -p net-lattice-backend-windows add_then_remove_address_round_trips_through_the_kernel -- --ignored"]
    fn add_then_remove_address_round_trips_through_the_kernel() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let interface_index = backend
            .interfaces()
            .expect("failed to list Windows interfaces")
            .into_iter()
            .find(|interface| matches!(interface.kind, InterfaceKind::Loopback))
            .expect("Windows loopback interface was not found")
            .index;
        let network = Network::from(Ipv4Network::new(
            Ipv4Address::new(192, 0, 2, 9),
            Ipv4PrefixLength::new(24).unwrap(),
        ));

        if let Some(existing) = backend
            .addresses()
            .expect("addresses() failed before add_address")
            .into_iter()
            .find(|address| {
                address.interface_index == interface_index && address.address == network
            })
        {
            let _ = backend.remove_address(existing);
        }

        let observed = backend
            .add_address(NewInterfaceAddress::new(
                Id::new(interface_index as u64),
                network,
            ))
            .expect("add_address failed - are you running as Administrator?");
        let present = backend
            .addresses()
            .expect("addresses() failed after add_address")
            .into_iter()
            .any(|address| address.id == observed.id);

        backend
            .remove_address(observed.clone())
            .expect("remove_address failed after successful add_address");
        let absent = !backend
            .addresses()
            .expect("addresses() failed after remove_address")
            .into_iter()
            .any(|address| address.id == observed.id);

        assert!(
            present,
            "added address was not present in addresses() afterward"
        );
        assert!(
            absent,
            "removed address was still present in addresses() afterward"
        );
    }

    /// Exercises a real round trip through `GetIpNetTable2`, no privilege
    /// required: reading the neighbor cache is readable by any user.
    #[test]
    fn neighbors_reads_the_real_kernel_neighbor_table() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let neighbors = backend
            .neighbors()
            .expect("GetIpNetTable2 dump should not require privilege");
        // Not asserting on contents: the neighbor table of the machine
        // running this test is arbitrary. Reaching here without an error is
        // the assertion.
        let _ = neighbors;
    }

    /// Exercises a real round trip through `GetAdaptersAddresses`, no
    /// privilege required: reading DNS configuration is readable by any
    /// user. Not asserting on contents since the machine running this test
    /// may have any DNS configuration (including none, e.g. a sandboxed CI
    /// runner) — reaching here without an error is the assertion.
    #[test]
    fn dns_config_reads_the_real_adapter_dns_settings() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let config = backend
            .dns_config()
            .expect("GetAdaptersAddresses should not require privilege");
        let _ = config;
    }

    #[test]
    fn dns_nameserver_list_preserves_requested_order() {
        let config = NewDnsConfig::with(
            vec![
                IpAddress::from(Ipv4Address::new(1, 1, 1, 1)),
                IpAddress::from(Ipv4Address::new(8, 8, 8, 8)),
            ],
            Vec::new(),
        );
        assert_eq!(config_list(&config.nameservers), "1.1.1.1,8.8.8.8");
    }

    /// Registers and immediately drops the three native notification handles
    /// without changing Windows networking state. The ignored test below
    /// verifies a real route notification end-to-end.
    #[test]
    fn watch_registers_ip_helper_notifications() {
        use std::time::Duration;

        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        assert!(backend.capabilities().contains(Capability::MONITORING));
        drop(
            backend
                .watch()
                .expect("failed to register IP Helper notifications"),
        );
        let filtered = backend
            .watch_filtered(EventFilter::none())
            .expect("failed to register filtered IP Helper notifications");
        assert_eq!(
            filtered.recv_timeout(Duration::from_millis(1)).unwrap(),
            None
        );
    }

    #[cfg(feature = "async")]
    #[test]
    fn watch_tokio_registers_ip_helper_notifications() {
        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        let watcher = backend
            .watch_tokio(EventFilter::none())
            .expect("failed to register IP Helper notifications");
        drop(watcher);
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

    /// End-to-end monitoring verification using a temporary route. This
    /// requires an elevated Windows token because `CreateIpForwardEntry2`
    /// modifies kernel routing state.
    #[test]
    #[ignore = "requires Administrator; run from elevated cmd/PowerShell: cargo test -p net-lattice-backend-windows watch_observes_route_changes -- --ignored"]
    fn watch_observes_route_changes() {
        use std::time::Duration;

        let backend = WindowsBackend::new().expect("failed to create Windows backend");
        assert!(backend.capabilities().contains(Capability::MONITORING));
        let watcher = backend
            .watch()
            .expect("failed to register IP Helper notifications");
        #[cfg(feature = "async")]
        let mut async_watcher = backend
            .watch_tokio(EventFilter::none().routes())
            .expect("failed to register async IP Helper notifications");
        let interface_index = backend
            .interfaces()
            .expect("failed to list Windows interfaces")
            .into_iter()
            .find(|interface| matches!(interface.kind, InterfaceKind::Loopback))
            .expect("Windows loopback interface was not found")
            .index;
        let destination = Network::from(Ipv4Network::new(
            // TEST-NET-2 is distinct from the route CRUD test's TEST-NET-3
            // prefix because ignored tests run in parallel.
            Ipv4Address::new(198, 51, 100, 0),
            Ipv4PrefixLength::new(24).unwrap(),
        ));
        let route = Route::new(RouteId::new(0), destination).with_interface_index(interface_index);

        backend
            .add_route(route.clone())
            .expect("failed to add monitoring test route");
        // The kernel may canonicalize an on-link next hop differently from
        // the `AF_UNSPEC` value used to create it. Read the canonical row so
        // the asserted ID is exactly the one the notification callback maps.
        let watched_id = backend
            .routes()
            .expect("failed to read routes after adding test route")
            .into_iter()
            .find(|candidate| {
                candidate.destination == destination
                    && candidate.interface_index == Some(interface_index)
            })
            .expect("test route was not present after it was added")
            .id;

        let observed = (0..12).any(|_| {
            matches!(
                watcher.recv_timeout(Duration::from_millis(250)),
                Ok(Some(Event::Route { id, .. })) if id == watched_id
            )
        });
        #[cfg(feature = "async")]
        let async_observed = tokio_route_event(&mut async_watcher, watched_id);
        let selected_watcher = backend
            .watch_filtered(EventFilter::none().route(watched_id))
            .expect("failed to register selected IP Helper route notifications");
        #[cfg(feature = "async")]
        let mut selected_async_watcher = backend
            .watch_tokio(EventFilter::none().route(watched_id))
            .expect("failed to register selected async IP Helper notifications");
        let _ = backend.remove_route(route);
        let selected_observed = (0..12).any(|_| {
            matches!(
                selected_watcher.recv_timeout(Duration::from_millis(250)),
                Ok(Some(Event::Route { id, kind: ChangeKind::Removed })) if id == watched_id
            )
        });
        #[cfg(feature = "async")]
        let selected_async_observed = tokio_route_event(&mut selected_async_watcher, watched_id);
        assert!(observed, "watch() did not report the route mutation");
        assert!(
            selected_observed,
            "object route filter did not report removal"
        );
        #[cfg(feature = "async")]
        assert!(
            async_observed,
            "watch_tokio() did not report the route mutation"
        );
        #[cfg(feature = "async")]
        assert!(
            selected_async_observed,
            "async object route filter did not report removal"
        );
    }
}
