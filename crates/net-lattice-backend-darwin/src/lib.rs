//! BSD/macOS backend for Net Lattice: implements `net-lattice-platform`'s provider
//! traits via route sockets.
//!
//! Only ever compiled for `target_os = "macos"` — its dependencies
//! (`libc`, macOS-only) are gated the same way in `Cargo.toml`. See
//! ARCHITECTURE.md for how this crate binds `net-lattice-platform`'s generic
//! `RouteProvider::Route` associated type to the concrete
//! `net_lattice_model::route::Route`.

#![cfg(target_os = "macos")]

use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::{io, mem};

use net_lattice_core::{Error, Id, PlatformErrorCode, Result};
use net_lattice_model::interface::{AdminState, Interface, InterfaceKind, OperationalState};
use net_lattice_model::mac::MacAddress;
use net_lattice_model::route::{Route, RouteId};
use net_lattice_model::{IpAddress, Network};
use net_lattice_platform::{InterfaceProvider, RouteProvider};

const RTM_VERSION: u8 = 5;
const RTM_GET: u8 = 4;
const RTM_ADD: u8 = 1;
const RTM_DELETE: u8 = 2;

// `IFT_*` constants from `<net/if_types.h>`, not exposed by the `libc` crate
// for `apple`.
const IFT_ETHER: libc::c_uchar = 0x06;
const IFT_LOOP: libc::c_uchar = 0x18;
const IFT_PPP: libc::c_uchar = 0x17;
const IFT_BRIDGE: libc::c_uchar = 0xd1;
const IFT_L2VLAN: libc::c_uchar = 0x87;

// `rt_msghdr::rtm_addrs`/`rtm_flags` are `c_int` (`i32`) on BSD/macOS, unlike
// Netlink's `u32` bitmasks — these are typed to match.
const RTA_DST: libc::c_int = 0x1;
const RTA_GATEWAY: libc::c_int = 0x2;
const RTA_NETMASK: libc::c_int = 0x4;

const RTF_UP: libc::c_int = 0x0001;
const RTF_GATEWAY: libc::c_int = 0x0002;
const RTF_HOST: libc::c_int = 0x0004;
const RTF_STATIC: libc::c_int = 0x0800;

const RTM_MAXSIZE: usize = 2048;

/// The BSD/macOS route socket-backed implementation of Net Lattice's provider
/// traits.
pub struct DarwinBackend {
    runtime: tokio::runtime::Runtime,
    fd: i32,
}

impl DarwinBackend {
    pub fn new() -> Result<Self> {
        let runtime =
            tokio::runtime::Runtime::new().map_err(|err| Error::Platform(io_error_code(&err)))?;
        let _guard = runtime.enter();
        let fd = unsafe { libc::socket(libc::PF_ROUTE, libc::SOCK_RAW, libc::AF_UNSPEC) };
        if fd < 0 {
            return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
        }
        Ok(Self { runtime, fd })
    }
}

impl Drop for DarwinBackend {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}

fn io_error_code(err: &std::io::Error) -> PlatformErrorCode {
    PlatformErrorCode::Darwin(err.raw_os_error().unwrap_or(0))
}

/// Placeholder identity scheme: a route has no kernel-assigned numeric ID,
/// so this hashes its defining fields. See ARCHITECTURE.md's open Object
/// Identity question — this is not a long-term answer, only enough to give
/// `Stage 0.3` a `RouteId` to work with.
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

unsafe fn sockaddr_to_ip(sa: *const libc::sockaddr) -> Option<IpAddr> {
    if sa.is_null() {
        return None;
    }
    let family = unsafe { (*sa).sa_family } as libc::c_int;
    match family {
        libc::AF_INET => {
            let sin = unsafe { &*(sa as *const libc::sockaddr_in) };
            let octets = u32::from_be(sin.sin_addr.s_addr).to_be_bytes();
            Some(IpAddr::V4(std::net::Ipv4Addr::new(
                octets[0], octets[1], octets[2], octets[3],
            )))
        }
        libc::AF_INET6 => {
            let sin6 = unsafe { &*(sa as *const libc::sockaddr_in6) };
            let bytes = sin6.sin6_addr.s6_addr;
            Some(IpAddr::V6(std::net::Ipv6Addr::from(bytes)))
        }
        _ => None,
    }
}

/// Counts leading `1` bits across `bytes`, treated as a big-endian mask
/// zero-padded on the right. BSD routing sockets represent a netmask's
/// `sockaddr` with trailing zero bytes omitted entirely (`sa_len` shrinks
/// instead of the buffer being zero-filled) rather than always sending a
/// full-width mask, so an empty/short `bytes` correctly yields a shorter
/// prefix (e.g. no bytes at all means `/0`, the default route).
fn mask_bytes_to_prefix_len(bytes: &[u8]) -> u8 {
    let mut prefix = 0u8;
    for &byte in bytes {
        if byte == 0xff {
            prefix += 8;
        } else {
            prefix += byte.leading_ones() as u8;
            break;
        }
    }
    prefix
}

unsafe fn message_to_route(hdr: &libc::rt_msghdr) -> Option<Route> {
    let mut destination_addr = None;
    let mut gateway = None;
    let mut interface_index = None;
    let mut netmask_bytes: Option<Vec<u8>> = None;

    let mut ptr = unsafe { (hdr as *const libc::rt_msghdr).add(1) as *const u8 };
    let mut remaining = hdr.rtm_msglen as usize - mem::size_of::<libc::rt_msghdr>();
    let mut bit: libc::c_int = 1;
    while bit <= hdr.rtm_addrs && remaining >= 1 {
        if hdr.rtm_addrs & bit == 0 {
            bit <<= 1;
            continue;
        }
        // `sa_len` is the first byte of every variant of `sockaddr` — read
        // it directly rather than requiring a full-size `sockaddr` to be
        // present, since the netmask entry (`RTA_NETMASK`) is routinely
        // shorter than that (trailing zero mask bytes are omitted).
        let sa_len = unsafe { *ptr } as usize;
        let aligned_len = if sa_len == 0 { 4 } else { (sa_len + 3) & !3 };
        if aligned_len > remaining {
            break;
        }
        match bit {
            RTA_DST => {
                destination_addr = unsafe { sockaddr_to_ip(ptr as *const libc::sockaddr) };
            }
            RTA_GATEWAY => {
                gateway = unsafe { sockaddr_to_ip(ptr as *const libc::sockaddr) }
                    .map(std_ip_to_ip_address);
            }
            RTA_NETMASK => {
                // The mask's address bytes start at the same offset a real
                // address of the destination's family would (4 bytes in for
                // `sockaddr_in`: `sa_len`+`sa_family`+`sin_port`; 8 for
                // `sockaddr_in6`, which adds `sin6_flowinfo`) — `sa_family`
                // itself is unreliable here, BSD kernels routinely leave it
                // as `0` on netmask entries.
                let header = match destination_addr {
                    Some(IpAddr::V6(_)) => 8,
                    _ => 4,
                };
                let available = sa_len.saturating_sub(header);
                netmask_bytes = Some(if available > 0 {
                    unsafe { std::slice::from_raw_parts(ptr.add(header), available) }.to_vec()
                } else {
                    Vec::new()
                });
            }
            _ => {}
        }
        ptr = unsafe { ptr.add(aligned_len) };
        remaining -= aligned_len;
        bit <<= 1;
    }

    if hdr.rtm_index != 0 {
        interface_index = Some(hdr.rtm_index as u32);
    }

    let destination_addr = destination_addr?;
    // A host route (`RTF_HOST`) carries no `RTA_NETMASK` at all and is
    // implicitly `/32`/`/128`; otherwise derive the prefix from the actual
    // netmask bytes (an absent-but-non-host netmask, e.g. the default
    // route, correctly yields `/0` via `mask_bytes_to_prefix_len(&[])`).
    let full_len = match destination_addr {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    let prefix_len = if (hdr.rtm_flags & RTF_HOST) != 0 {
        full_len
    } else {
        netmask_bytes
            .as_deref()
            .map(mask_bytes_to_prefix_len)
            .unwrap_or(full_len)
    };
    let destination = match destination_addr {
        IpAddr::V4(addr) => {
            let prefix = net_lattice_ip::Ipv4PrefixLength::new(prefix_len)?;
            Network::from(net_lattice_ip::Ipv4Network::new(addr.into(), prefix))
        }
        IpAddr::V6(addr) => {
            let prefix = net_lattice_ip::Ipv6PrefixLength::new(prefix_len)?;
            Network::from(net_lattice_ip::Ipv6Network::new(addr.into(), prefix))
        }
    };

    let mut route = Route::new(
        synthesize_route_id(&destination, &gateway, interface_index),
        destination,
    );
    if let Some(gateway) = gateway {
        route = route.with_gateway(gateway);
    }
    if let Some(interface_index) = interface_index {
        route = route.with_interface_index(interface_index);
    }
    Some(route)
}

fn build_get_request() -> Vec<u8> {
    let mut buf = vec![0u8; mem::size_of::<libc::rt_msghdr>()];
    let hdr = unsafe { &mut *(buf.as_mut_ptr() as *mut libc::rt_msghdr) };
    hdr.rtm_msglen = mem::size_of::<libc::rt_msghdr>() as u16;
    hdr.rtm_version = RTM_VERSION;
    hdr.rtm_type = RTM_GET;
    hdr.rtm_flags = RTF_UP;
    hdr.rtm_addrs = 0;
    hdr.rtm_pid = unsafe { libc::getpid() };
    hdr.rtm_seq = 1;
    hdr.rtm_index = 0;
    buf
}

fn build_add_message(route: &Route) -> Result<Vec<u8>> {
    let (destination, prefix_len) = network_to_std(route.destination);
    let mut buf = vec![0u8; RTM_MAXSIZE];
    let hdr = unsafe { &mut *(buf.as_mut_ptr() as *mut libc::rt_msghdr) };
    hdr.rtm_version = RTM_VERSION;
    hdr.rtm_type = RTM_ADD;
    hdr.rtm_pid = unsafe { libc::getpid() };
    hdr.rtm_seq = 2;
    hdr.rtm_flags = RTF_UP | RTF_STATIC;
    hdr.rtm_addrs = RTA_DST;
    let mut offset = mem::size_of::<libc::rt_msghdr>();

    offset += push_sockaddr(&mut buf, offset, destination);

    if prefix_len == 32 || prefix_len == 128 {
        hdr.rtm_flags |= RTF_HOST;
    } else {
        hdr.rtm_addrs |= RTA_NETMASK;
        offset += push_netmask(&mut buf, offset, destination, prefix_len);
    }

    if let Some(gateway) = route.gateway.map(ip_address_to_std) {
        hdr.rtm_flags |= RTF_GATEWAY;
        hdr.rtm_addrs |= RTA_GATEWAY;
        offset += push_sockaddr(&mut buf, offset, gateway);
    }

    if let Some(interface_index) = route.interface_index {
        hdr.rtm_index = interface_index as u16;
    }

    hdr.rtm_msglen = offset as u16;
    buf.truncate(offset);
    Ok(buf)
}

fn build_delete_message(route: &Route) -> Result<Vec<u8>> {
    let (destination, prefix_len) = network_to_std(route.destination);
    let mut buf = vec![0u8; RTM_MAXSIZE];
    let hdr = unsafe { &mut *(buf.as_mut_ptr() as *mut libc::rt_msghdr) };
    hdr.rtm_version = RTM_VERSION;
    hdr.rtm_type = RTM_DELETE;
    hdr.rtm_pid = unsafe { libc::getpid() };
    hdr.rtm_seq = 3;
    hdr.rtm_flags = RTF_UP;
    hdr.rtm_addrs = RTA_DST;
    let mut offset = mem::size_of::<libc::rt_msghdr>();

    offset += push_sockaddr(&mut buf, offset, destination);

    if prefix_len == 32 || prefix_len == 128 {
        hdr.rtm_flags |= RTF_HOST;
    } else {
        hdr.rtm_addrs |= RTA_NETMASK;
        offset += push_netmask(&mut buf, offset, destination, prefix_len);
    }

    if let Some(gateway) = route.gateway.map(ip_address_to_std) {
        hdr.rtm_flags |= RTF_GATEWAY;
        hdr.rtm_addrs |= RTA_GATEWAY;
        offset += push_sockaddr(&mut buf, offset, gateway);
    }

    if let Some(interface_index) = route.interface_index {
        hdr.rtm_index = interface_index as u16;
    }

    hdr.rtm_msglen = offset as u16;
    buf.truncate(offset);
    Ok(buf)
}

fn push_sockaddr(buf: &mut [u8], offset: usize, addr: IpAddr) -> usize {
    match addr {
        IpAddr::V4(addr) => {
            let octets = addr.octets();
            let sin = libc::sockaddr_in {
                sin_family: libc::AF_INET as u8,
                sin_len: mem::size_of::<libc::sockaddr_in>() as u8,
                sin_port: 0,
                sin_addr: libc::in_addr {
                    s_addr: u32::from_be_bytes(octets).to_be(),
                },
                sin_zero: [0; 8],
            };
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &sin as *const _ as *const u8,
                    buf.as_mut_ptr().add(offset),
                    mem::size_of::<libc::sockaddr_in>(),
                );
            }
            mem::size_of::<libc::sockaddr_in>()
        }
        IpAddr::V6(addr) => {
            let octets = addr.octets();
            let sin6 = libc::sockaddr_in6 {
                sin6_family: libc::AF_INET6 as u8,
                sin6_len: mem::size_of::<libc::sockaddr_in6>() as u8,
                sin6_port: 0,
                sin6_flowinfo: 0,
                sin6_addr: libc::in6_addr { s6_addr: octets },
                sin6_scope_id: 0,
            };
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &sin6 as *const _ as *const u8,
                    buf.as_mut_ptr().add(offset),
                    mem::size_of::<libc::sockaddr_in6>(),
                );
            }
            mem::size_of::<libc::sockaddr_in6>()
        }
    }
}

fn push_netmask(buf: &mut [u8], offset: usize, addr: IpAddr, prefix_len: u8) -> usize {
    match addr {
        IpAddr::V4(_) => {
            let mask = if prefix_len == 0 {
                0u32
            } else {
                !0u32 << (32 - prefix_len)
            };
            let sin = libc::sockaddr_in {
                sin_family: libc::AF_INET as u8,
                sin_len: mem::size_of::<libc::sockaddr_in>() as u8,
                sin_port: 0,
                sin_addr: libc::in_addr {
                    s_addr: mask.to_be(),
                },
                sin_zero: [0; 8],
            };
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &sin as *const _ as *const u8,
                    buf.as_mut_ptr().add(offset),
                    mem::size_of::<libc::sockaddr_in>(),
                );
            }
            mem::size_of::<libc::sockaddr_in>()
        }
        IpAddr::V6(_) => {
            let mut mask_bytes = [0u8; 16];
            let full_bytes = (prefix_len / 8) as usize;
            let remainder = prefix_len % 8;
            for byte in &mut mask_bytes[..full_bytes] {
                *byte = 0xff;
            }
            if remainder > 0 && full_bytes < 16 {
                mask_bytes[full_bytes] = !0u8 << (8 - remainder);
            }
            let sin6 = libc::sockaddr_in6 {
                sin6_family: libc::AF_INET6 as u8,
                sin6_len: mem::size_of::<libc::sockaddr_in6>() as u8,
                sin6_port: 0,
                sin6_flowinfo: 0,
                sin6_addr: libc::in6_addr {
                    s6_addr: mask_bytes,
                },
                sin6_scope_id: 0,
            };
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &sin6 as *const _ as *const u8,
                    buf.as_mut_ptr().add(offset),
                    mem::size_of::<libc::sockaddr_in6>(),
                );
            }
            mem::size_of::<libc::sockaddr_in6>()
        }
    }
}

impl RouteProvider for DarwinBackend {
    type Route = Route;

    fn routes(&self) -> Result<Vec<Self::Route>> {
        self.runtime.block_on(async {
            let request = build_get_request();
            let n = unsafe { libc::send(self.fd, request.as_ptr() as *const _, request.len(), 0) };
            if n < 0 {
                return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
            }

            let mut routes = Vec::new();
            let mut buf = [0u8; 65536];
            loop {
                let n = unsafe { libc::recv(self.fd, buf.as_mut_ptr() as *mut _, buf.len(), 0) };
                if n < 0 {
                    return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
                }
                if n == 0 {
                    break;
                }

                let mut offset = 0usize;
                while offset < n as usize {
                    let hdr = unsafe { &*(buf.as_ptr().add(offset) as *const libc::rt_msghdr) };
                    if hdr.rtm_version != RTM_VERSION {
                        break;
                    }
                    if hdr.rtm_type == RTM_GET
                        && let Some(route) = unsafe { message_to_route(hdr) }
                    {
                        routes.push(route);
                    }
                    let step = hdr.rtm_msglen as usize;
                    if step == 0 {
                        break;
                    }
                    offset += step;
                }
            }
            Ok(routes)
        })
    }

    fn add_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(async {
            let message = build_add_message(&route)?;
            let n = unsafe { libc::send(self.fd, message.as_ptr() as *const _, message.len(), 0) };
            if n < 0 {
                return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
            }
            Ok(())
        })
    }

    fn remove_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(async {
            let message = build_delete_message(&route)?;
            let n = unsafe { libc::send(self.fd, message.as_ptr() as *const _, message.len(), 0) };
            if n < 0 {
                return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
            }
            Ok(())
        })
    }
}

/// Maps `IFT_*` link-layer types (carried in `sockaddr_dl::sdl_type`) to the
/// cross-platform [`InterfaceKind`]. Anything not covered falls back to
/// `Other`, carrying the raw type code for diagnostics.
fn ift_type_to_kind(sdl_type: libc::c_uchar) -> InterfaceKind {
    match sdl_type {
        IFT_ETHER | IFT_L2VLAN => InterfaceKind::Ethernet,
        IFT_LOOP => InterfaceKind::Loopback,
        IFT_PPP => InterfaceKind::PointToPoint,
        IFT_BRIDGE => InterfaceKind::Bridge,
        other => InterfaceKind::Other(other as u32),
    }
}

/// Reads the interface name, index, hardware type, and MAC address out of an
/// `AF_LINK` `sockaddr_dl` — the only place `getifaddrs` exposes them on
/// BSD/macOS. Returns `None` if the address is not actually `AF_LINK` (the
/// same interface also appears once per configured IP address, with
/// `AF_INET`/`AF_INET6` entries this function ignores).
unsafe fn link_entry_to_interface(entry: &libc::ifaddrs) -> Option<Interface> {
    let sa = entry.ifa_addr;
    if sa.is_null() || unsafe { (*sa).sa_family } as i32 != libc::AF_LINK {
        return None;
    }
    let sdl = unsafe { &*(sa as *const libc::sockaddr_dl) };

    let name = if entry.ifa_name.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(entry.ifa_name) }
            .to_string_lossy()
            .into_owned()
    };

    let mac = if sdl.sdl_alen == 6 {
        let start = sdl.sdl_nlen as usize;
        let data = &sdl.sdl_data;
        if start + 6 <= data.len() {
            let mut octets = [0u8; 6];
            for (i, octet) in octets.iter_mut().enumerate() {
                *octet = data[start + i] as u8;
            }
            Some(MacAddress::new(octets))
        } else {
            None
        }
    } else {
        None
    };

    let admin_state = if entry.ifa_flags & (libc::IFF_UP as u32) != 0 {
        AdminState::Up
    } else {
        AdminState::Down
    };

    // `IFF_RUNNING` ("resources allocated", set once the link layer has
    // actually attached) is the closest BSD equivalent to Linux's carrier
    // state: up-but-not-running reads as no-carrier (cable unplugged,
    // Wi-Fi not associated, ...).
    let operational_state = match (
        entry.ifa_flags & (libc::IFF_UP as u32) != 0,
        entry.ifa_flags & (libc::IFF_RUNNING as u32) != 0,
    ) {
        (true, true) => OperationalState::Up,
        (true, false) => OperationalState::NoCarrier,
        (false, _) => OperationalState::Down,
    };

    let index = sdl.sdl_index as u32;
    let kind = ift_type_to_kind(sdl.sdl_type);

    let mut interface = Interface::new(Id::new(index as u64), index, name, kind)
        .with_admin_state(admin_state)
        .with_operational_state(operational_state);
    if let Some(mac) = mac {
        interface = interface.with_mac(mac);
    }
    Some(interface)
}

impl InterfaceProvider for DarwinBackend {
    type Interface = Interface;

    fn interfaces(&self) -> Result<Vec<Self::Interface>> {
        let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
        let interfaces = unsafe {
            if libc::getifaddrs(&mut head) != 0 {
                return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
            }

            let mut interfaces = Vec::new();
            let mut cursor = head;
            while !cursor.is_null() {
                if let Some(interface) = link_entry_to_interface(&*cursor) {
                    interfaces.push(interface);
                }
                cursor = (*cursor).ifa_next;
            }
            libc::freeifaddrs(head);
            interfaces
        };
        Ok(interfaces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_lattice_ip::{Ipv4Address, Ipv4Network, Ipv4PrefixLength};

    /// Exercises a real round trip through the route socket, no privilege
    /// required: routing table dumps are readable by any user. This is the
    /// one test in this module that runs by default and actually proves the
    /// backend talks to the kernel, rather than only exercising conversion
    /// logic.
    #[test]
    fn routes_reads_the_real_kernel_routing_table() {
        let backend = DarwinBackend::new().expect("failed to open a route socket");
        let routes = backend
            .routes()
            .expect("RTM_GET dump should not require privilege");
        // Not asserting on contents: the routing table of the machine
        // running this test is arbitrary (may even be empty in a minimal
        // container). Reaching here without an error is the assertion.
        let _ = routes;
    }

    /// Requires `root` privileges (root, or `sudo -E cargo test -- --ignored`
    /// in this crate). Not run by default because most development and CI
    /// environments — including the one this crate was originally written
    /// in — don't grant it, and this test would otherwise fail with
    /// `PermissionDenied` rather than being skipped.
    ///
    /// Uses a documentation-only prefix (RFC 5737 `203.0.113.0/24`,
    /// TEST-NET-3) on `lo0` so it can't collide with or disrupt real
    /// routing, and removes what it added regardless of assertion outcome.
    #[test]
    #[ignore = "requires root; run with `sudo -E cargo test -p net-lattice-backend-darwin -- --ignored`"]
    fn add_then_remove_route_round_trips_through_the_kernel() {
        let backend = DarwinBackend::new().expect("failed to open a route socket");
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
            add_result.expect("add_route failed - are you running as root?");
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
