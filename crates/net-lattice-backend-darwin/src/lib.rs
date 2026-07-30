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
const RTM_ADD: u8 = 1;
const RTM_DELETE: u8 = 2;

// `rtm_seq` values this backend tags its own `RTM_ADD`/`RTM_DELETE`
// requests with, so the reply-matching loop in `send_route_request` can
// tell "the kernel's answer to *our* request" apart from another process's
// traffic on the same shared routing socket (every open `PF_ROUTE` socket
// receives every message written to any of them).
const RTM_SEQ_ADD: libc::c_int = 2;
const RTM_SEQ_DELETE: libc::c_int = 3;

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

/// Maps a `PF_ROUTE` `send()` failure's errno to Net Lattice's error
/// taxonomy where BSD's routing-socket semantics line up with a named
/// variant, per ARCHITECTURE.md's Error Model — `EEXIST` for `RTM_ADD`
/// (a route to that destination already exists) and `ESRCH`/`ENOENT` for
/// `RTM_DELETE` (no matching route to remove) are exactly `AlreadyExists`
/// and `NotFound`, not generic platform noise callers would otherwise have
/// to pattern-match an OS-specific errno to recognize.
fn route_socket_error(err: &io::Error) -> Error {
    match err.raw_os_error() {
        Some(libc::EPERM) | Some(libc::EACCES) => Error::PermissionDenied,
        Some(libc::ESRCH) | Some(libc::ENOENT) => Error::NotFound,
        Some(libc::EEXIST) => Error::AlreadyExists,
        _ => Error::Platform(io_error_code(err)),
    }
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

/// Dumps the entire routing table via `sysctl(CTL_NET, PF_ROUTE, 0,
/// AF_UNSPEC, NET_RT_DUMP, 0)` — the standard BSD mechanism for reading
/// every route at once. `RTM_GET` sent over a `PF_ROUTE` socket, by
/// contrast, looks up the route to one specific destination (it requires an
/// `RTA_DST`); sending it with no destination — as one might expect from
/// Netlink's dump-via-empty-request idiom — is rejected by the kernel with
/// `EINVAL` rather than returning every route.
///
/// The returned buffer is a back-to-back sequence of `rt_msghdr`-prefixed
/// messages, the same wire format `message_to_route` already parses.
fn dump_routing_table() -> Result<Vec<u8>> {
    let mut mib: [libc::c_int; 6] = [
        libc::CTL_NET,
        libc::PF_ROUTE,
        0,
        libc::AF_UNSPEC,
        libc::NET_RT_DUMP,
        0,
    ];

    let mut needed: usize = 0;
    unsafe {
        if libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            std::ptr::null_mut(),
            &mut needed,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
        }
    }
    if needed == 0 {
        return Ok(Vec::new());
    }

    let mut buf = vec![0u8; needed];
    unsafe {
        if libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            buf.as_mut_ptr().cast(),
            &mut needed,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
        }
    }
    buf.truncate(needed);
    Ok(buf)
}

fn build_add_message(route: &Route) -> Result<Vec<u8>> {
    let (destination, prefix_len) = network_to_std(route.destination);
    let mut buf = vec![0u8; RTM_MAXSIZE];
    let hdr = unsafe { &mut *(buf.as_mut_ptr() as *mut libc::rt_msghdr) };
    hdr.rtm_version = RTM_VERSION;
    hdr.rtm_type = RTM_ADD;
    hdr.rtm_pid = unsafe { libc::getpid() };
    hdr.rtm_seq = RTM_SEQ_ADD;
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

    match (route.gateway.map(ip_address_to_std), route.interface_index) {
        (Some(gateway), _) => {
            hdr.rtm_flags |= RTF_GATEWAY;
            hdr.rtm_addrs |= RTA_GATEWAY;
            offset += push_sockaddr(&mut buf, offset, gateway);
        }
        (None, Some(interface_index)) => {
            // No IP gateway: bind the route directly to an outgoing
            // interface instead, the way `route add -interface` does.
            // BSD's `RTM_ADD` needs *some* address in the `RTA_GATEWAY`
            // slot to determine the outgoing path — `rtm_index` in the
            // header alone is not honored for `ADD` (the kernel only
            // fills it in on the reply, describing what it picked); it
            // rejects a bare `rtm_index` with no gateway as `EINVAL`. A
            // link-layer (`AF_LINK`) `sockaddr_dl` naming the interface,
            // without `RTF_GATEWAY` (that flag specifically means "a real
            // next hop", not "no next hop, just this wire"), is how a
            // direct/on-link route is expressed.
            //
            // The interface's *name* matters, not just `sdl_index`: this
            // mirrors `route.tproj/route.c` (`route add -interface`),
            // which resolves the interface's real `sockaddr_dl` via
            // `getifaddrs` and copies it whole (name included) into the
            // gateway slot — a synthetic `sockaddr_dl` carrying only
            // `sdl_index` and an empty name is accepted by the kernel
            // (`rtm_errno == 0`) but doesn't actually resolve to a usable
            // interface reference, and no route gets created.
            let Some(name) = interface_name(interface_index) else {
                return Err(Error::NotFound);
            };
            hdr.rtm_addrs |= RTA_GATEWAY;
            offset += push_link_gateway(&mut buf, offset, interface_index, &name);
        }
        (None, None) => return Err(Error::InvalidState),
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
    hdr.rtm_seq = RTM_SEQ_DELETE;
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

/// Looks up interface `index`'s name via `if_indextoname` — needed because
/// the kernel resolves the `AF_LINK` gateway `push_link_gateway` builds by
/// name, not by `sdl_index` alone (see that function's doc comment).
fn interface_name(index: u32) -> Option<Vec<u8>> {
    let mut name_buf = [0 as libc::c_char; libc::IFNAMSIZ];
    let ptr = unsafe { libc::if_indextoname(index, name_buf.as_mut_ptr()) };
    if ptr.is_null() {
        return None;
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(name_buf.as_ptr()) };
    Some(cstr.to_bytes().to_vec())
}

/// Pushes an `AF_LINK` `sockaddr_dl` naming interface `interface_index`
/// (by both index and name), with no hardware address of its own — this
/// is the gateway-slot shape used for "send directly out this interface,
/// no next hop" routes (see `build_add_message`'s `(None,
/// Some(interface_index))` case).
///
/// Mirrors `route.tproj/route.c` (`route add -interface`): it resolves the
/// interface's *real* `sockaddr_dl` via `getifaddrs` — name, index, and
/// all — and copies that whole struct into the gateway slot. An
/// index-only `sockaddr_dl` with an empty name is silently accepted by the
/// kernel (`rtm_errno == 0`) but never resolves to a usable interface
/// reference, and no route actually gets created — that was this
/// function's original bug.
///
/// The on-wire `sdl_len` is the *significant* header length only (`8 +
/// name.len()`) — not `sizeof(struct sockaddr_dl)` (20, padded with an
/// unused 12-byte `sdl_data` array sized for the worst case). This matches
/// `golang.org/x/net/route`'s `LinkAddr.marshal()` (`lenAndSpace`: `8 +
/// len(Name) + len(Addr)`), the reference implementation for BSD routing
/// sockets.
fn push_link_gateway(buf: &mut [u8], offset: usize, interface_index: u32, name: &[u8]) -> usize {
    const HEADER_LEN: usize = 8;
    let nlen = name.len().min(12);
    let sdl_len = HEADER_LEN + nlen;

    let mut sdl = libc::sockaddr_dl {
        sdl_len: sdl_len as u8,
        sdl_family: libc::AF_LINK as u8,
        sdl_index: interface_index as u16,
        sdl_type: 0,
        sdl_nlen: nlen as u8,
        sdl_alen: 0,
        sdl_slen: 0,
        sdl_data: [0; 12],
    };
    for (dst, &src) in sdl.sdl_data[..nlen].iter_mut().zip(name) {
        *dst = src as libc::c_char;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(
            &sdl as *const _ as *const u8,
            buf.as_mut_ptr().add(offset),
            sdl_len,
        );
    }
    sdl_len
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
            let buf = dump_routing_table()?;

            let mut routes = Vec::new();
            let mut offset = 0usize;
            while offset + mem::size_of::<libc::rt_msghdr>() <= buf.len() {
                let hdr = unsafe { &*(buf.as_ptr().add(offset) as *const libc::rt_msghdr) };
                let step = hdr.rtm_msglen as usize;
                if step == 0 {
                    break;
                }
                if let Some(route) = unsafe { message_to_route(hdr) } {
                    routes.push(route);
                }
                offset += step;
            }
            Ok(routes)
        })
    }

    fn add_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(async {
            let message = build_add_message(&route)?;
            send_route_request(self.fd, &message, RTM_SEQ_ADD)
        })
    }

    fn remove_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(async {
            let message = build_delete_message(&route)?;
            send_route_request(self.fd, &message, RTM_SEQ_DELETE)
        })
    }
}

/// Sends an `RTM_ADD`/`RTM_DELETE` request and waits for the kernel's own
/// reply to confirm the outcome.
///
/// A `PF_ROUTE` socket is not request/response in the way a syscall return
/// value implies: `send()` succeeding only means the message was accepted
/// into the socket's write buffer, not that the kernel actually performed
/// the requested change. After processing a request, the kernel echoes the
/// *same* message back — with `rtm_errno` filled in — to every open routing
/// socket on the system (not just the one that sent it, since routing
/// sockets are a broadcast domain). Skipping this reply and trusting
/// `send()`'s return value silently drops real failures: an `add_route`
/// the kernel actually rejected still reports `Ok(())`.
///
/// `expected_seq` filters the broadcast stream down to the reply for this
/// specific request — matching on `rtm_pid` (this process) and `rtm_seq`
/// (this call) rather than just `rtm_type`, since other processes' routing
/// changes arrive on the same socket interleaved with our own reply.
fn send_route_request(fd: i32, message: &[u8], expected_seq: libc::c_int) -> Result<()> {
    let n = unsafe { libc::send(fd, message.as_ptr() as *const _, message.len(), 0) };
    if n < 0 {
        return Err(route_socket_error(&io::Error::last_os_error()));
    }

    let pid = unsafe { libc::getpid() };
    let mut buf = [0u8; RTM_MAXSIZE];
    loop {
        let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut _, buf.len(), 0) };
        if n < 0 {
            return Err(route_socket_error(&io::Error::last_os_error()));
        }
        if (n as usize) < mem::size_of::<libc::rt_msghdr>() {
            continue;
        }
        let hdr = unsafe { &*(buf.as_ptr() as *const libc::rt_msghdr) };
        if hdr.rtm_pid != pid || hdr.rtm_seq != expected_seq {
            // Someone else's request or reply, broadcast to every routing
            // socket — not the answer to what we just sent.
            continue;
        }
        return if hdr.rtm_errno == 0 {
            Ok(())
        } else {
            Err(route_socket_error(&io::Error::from_raw_os_error(
                hdr.rtm_errno,
            )))
        };
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

// `<sys/sockio.h>`'s `SIOCGIFMTU` is `_IOWR('i', 51, struct ifreq)` — not
// exposed as a named constant by the `libc` crate for `apple`. `_IOWR`'s
// BSD ioctl-number encoding (`<sys/ioccom.h>`) bakes in `sizeof(struct
// ifreq)`, computed here via `size_of` rather than a hand-copied literal so
// it can't drift from the real, compiled layout of `libc::ifreq`.
const IOCPARM_MASK: libc::c_ulong = 0x1fff;
const IOC_IN: libc::c_ulong = 0x8000_0000;
const IOC_OUT: libc::c_ulong = 0x4000_0000;
const IOC_INOUT: libc::c_ulong = IOC_IN | IOC_OUT;

fn siocgifmtu() -> libc::c_ulong {
    let size = mem::size_of::<libc::ifreq>() as libc::c_ulong;
    IOC_INOUT | ((size & IOCPARM_MASK) << 16) | ((b'i' as libc::c_ulong) << 8) | 51
}

/// Reads an interface's MTU via `ioctl(SIOCGIFMTU)` on `sock` — `getifaddrs`
/// doesn't carry MTU itself, this is the standard BSD way to fetch it.
/// `sock` only needs to be any open `AF_INET`/`SOCK_DGRAM` socket; it is
/// never connected or written to.
fn interface_mtu(sock: i32, name: &str) -> Option<u32> {
    let mut req: libc::ifreq = unsafe { mem::zeroed() };
    let name_bytes = name.as_bytes();
    let len = name_bytes.len().min(req.ifr_name.len() - 1);
    for (dst, &src) in req.ifr_name[..len].iter_mut().zip(&name_bytes[..len]) {
        *dst = src as libc::c_char;
    }

    let status = unsafe { libc::ioctl(sock, siocgifmtu(), &mut req) };
    if status != 0 {
        return None;
    }
    let mtu = unsafe { req.ifr_ifru.ifru_mtu };
    u32::try_from(mtu).ok()
}

impl InterfaceProvider for DarwinBackend {
    type Interface = Interface;

    fn interfaces(&self) -> Result<Vec<Self::Interface>> {
        let mtu_sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        if mtu_sock < 0 {
            return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
        }

        let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
        let interfaces = unsafe {
            if libc::getifaddrs(&mut head) != 0 {
                let err = Error::Platform(io_error_code(&io::Error::last_os_error()));
                libc::close(mtu_sock);
                return Err(err);
            }

            let mut interfaces = Vec::new();
            let mut cursor = head;
            while !cursor.is_null() {
                if let Some(mut interface) = link_entry_to_interface(&*cursor) {
                    if let Some(mtu) = interface_mtu(mtu_sock, &interface.name) {
                        interface = interface.with_mtu(mtu);
                    }
                    interfaces.push(interface);
                }
                cursor = (*cursor).ifa_next;
            }
            libc::freeifaddrs(head);
            interfaces
        };
        unsafe { libc::close(mtu_sock) };
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
    /// `lo0`'s ifindex is not reliably `1` — GitHub-hosted macOS runners in
    /// particular carry enough virtual interfaces (Docker, VPN, `utun*`,
    /// ...) ahead of it that assuming so failed CI outright. Looked up
    /// dynamically via `InterfaceProvider` instead, same as the Linux and
    /// Windows equivalents of this test.
    fn loopback_interface_index(backend: &DarwinBackend) -> u32 {
        backend
            .interfaces()
            .expect("interfaces() failed")
            .into_iter()
            .find(|iface| iface.name == "lo0")
            .map(|iface| iface.index)
            .expect("this test environment has no `lo0` interface")
    }

    /// Raw `rt_msghdr` fields (bypassing `message_to_route` entirely) for
    /// every dumped entry whose `RTA_DST` decodes to `target`, regardless
    /// of `rtm_index`.
    ///
    /// Diagnostic-only. The `interface_index`-filtered raw scan
    /// (`raw_headers_for_interface`) came back with 13 entries, none
    /// carrying the exact flags `build_add_message` sets (`RTF_UP |
    /// RTF_STATIC`, no `RTF_CLONING`) — meaning either the route was
    /// filed under a different `rtm_index` than expected, or genuinely
    /// wasn't created. Matching on the destination address directly,
    /// ignoring `rtm_index`, tells them apart.
    unsafe fn dst_from_message(hdr: &libc::rt_msghdr) -> Option<IpAddr> {
        let mut ptr = unsafe { (hdr as *const libc::rt_msghdr).add(1) as *const u8 };
        let mut remaining = hdr.rtm_msglen as usize - mem::size_of::<libc::rt_msghdr>();
        let mut bit: libc::c_int = 1;
        while bit <= hdr.rtm_addrs && remaining >= 1 {
            if hdr.rtm_addrs & bit == 0 {
                bit <<= 1;
                continue;
            }
            let sa_len = unsafe { *ptr } as usize;
            let aligned_len = if sa_len == 0 { 4 } else { (sa_len + 3) & !3 };
            if aligned_len > remaining {
                break;
            }
            if bit == RTA_DST {
                return unsafe { sockaddr_to_ip(ptr as *const libc::sockaddr) };
            }
            ptr = unsafe { ptr.add(aligned_len) };
            remaining -= aligned_len;
            bit <<= 1;
        }
        None
    }

    fn raw_headers_matching_destination(target: IpAddr) -> Vec<String> {
        let buf = dump_routing_table().expect("dump_routing_table failed");
        let mut entries = Vec::new();
        let mut offset = 0usize;
        while offset + mem::size_of::<libc::rt_msghdr>() <= buf.len() {
            let hdr = unsafe { &*(buf.as_ptr().add(offset) as *const libc::rt_msghdr) };
            let step = hdr.rtm_msglen as usize;
            if step == 0 {
                break;
            }
            if unsafe { dst_from_message(hdr) } == Some(target) {
                entries.push(format!(
                    "type={} flags={:#x} addrs={:#x} msglen={} index={} errno={}",
                    hdr.rtm_type,
                    hdr.rtm_flags,
                    hdr.rtm_addrs,
                    hdr.rtm_msglen,
                    hdr.rtm_index,
                    hdr.rtm_errno,
                ));
            }
            offset += step;
        }
        entries
    }

    /// Raw `rt_msghdr` fields for every dumped entry tagged with
    /// `interface_index`, bypassing `message_to_route` entirely.
    ///
    /// Diagnostic-only, for telling apart "the kernel never actually
    /// created the route" from "it's in the dump but our own parsing drops
    /// it" — the two remaining explanations for `near_matches` (which
    /// already rules out "added with a different address" or "different
    /// prefix") coming back empty. If this is also empty, the kernel
    /// genuinely didn't create anything on this interface despite
    /// `rtm_errno == 0`; if it's non-empty, `message_to_route` is silently
    /// dropping an entry that's really there (e.g. destination parsing
    /// returning `None`, or the loop misplacing its length-`stepped` offset
    /// after this entry).
    fn raw_headers_for_interface(interface_index: u32) -> Vec<String> {
        let buf = dump_routing_table().expect("dump_routing_table failed");
        let mut entries = Vec::new();
        let mut offset = 0usize;
        while offset + mem::size_of::<libc::rt_msghdr>() <= buf.len() {
            let hdr = unsafe { &*(buf.as_ptr().add(offset) as *const libc::rt_msghdr) };
            let step = hdr.rtm_msglen as usize;
            if step == 0 {
                break;
            }
            if hdr.rtm_index as u32 == interface_index {
                entries.push(format!(
                    "type={} flags={:#x} addrs={:#x} msglen={} index={} errno={}",
                    hdr.rtm_type,
                    hdr.rtm_flags,
                    hdr.rtm_addrs,
                    hdr.rtm_msglen,
                    hdr.rtm_index,
                    hdr.rtm_errno,
                ));
            }
            offset += step;
        }
        entries
    }

    /// Uses a documentation-only prefix (RFC 5737 `203.0.113.0/24`,
    /// TEST-NET-3) on `lo0` so it can't collide with or disrupt real
    /// routing, and removes what it added regardless of assertion outcome.
    #[test]
    #[ignore = "requires root; run with `sudo -E cargo test -p net-lattice-backend-darwin -- --ignored`"]
    fn add_then_remove_route_round_trips_through_the_kernel() {
        let backend = DarwinBackend::new().expect("failed to open a route socket");
        let interface_index = loopback_interface_index(&backend);

        let destination = Network::from(Ipv4Network::new(
            Ipv4Address::new(203, 0, 113, 0),
            Ipv4PrefixLength::new(24).unwrap(),
        ));
        let route = Route::new(RouteId::new(0), destination).with_interface_index(interface_index);

        // Best-effort cleanup of a leftover route from a prior run of this
        // same test (e.g. a run that panicked between add and remove) —
        // guarantees a clean starting state regardless of why one might
        // already be there, rather than the add below spuriously failing
        // with `AlreadyExists`.
        let _ = backend.remove_route(route.clone());

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

        // Retry with a short delay before concluding the route is really
        // absent: `rtm_errno == 0` confirms the kernel accepted the
        // request, but it's cheap to rule out any propagation delay
        // between that reply and the route becoming visible in a fresh
        // `NET_RT_DUMP` before trusting a single immediate read.
        let mut routes = backend
            .routes()
            .expect("routes() failed after add_route succeeded");
        let mut found = routes
            .iter()
            .any(|r| r.destination == destination && r.interface_index == Some(interface_index));
        for _ in 0..4 {
            if found {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            routes = backend
                .routes()
                .expect("routes() failed after add_route succeeded (retry)");
            found = routes.iter().any(|r| {
                r.destination == destination && r.interface_index == Some(interface_index)
            });
        }

        // Diagnostic-only: this exact assertion has already failed in CI
        // for multiple different root causes without enough visibility
        // into what `routes()` actually returned to tell them apart.
        // Matching on the destination *address alone* (not the full
        // `Network`, which also compares prefix length) is deliberate: the
        // previous round of this diagnostic filtered on exact `Network`
        // equality, which can't distinguish "genuinely not added" from
        // "added but with an unexpected prefix length" — both look like an
        // empty list. This one can.
        let near_matches: Vec<_> = routes
            .iter()
            .filter(|r| match (&r.destination, &destination) {
                (Network::V4(actual), Network::V4(expected)) => {
                    actual.address() == expected.address()
                }
                (Network::V6(actual), Network::V6(expected)) => {
                    actual.address() == expected.address()
                }
                _ => false,
            })
            .collect();

        let raw_headers = raw_headers_for_interface(interface_index);
        let raw_headers_by_dst =
            raw_headers_matching_destination(IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 0)));

        // Clean up before asserting, so a failed assertion doesn't leave
        // the test route behind on the machine that ran this.
        let _ = backend.remove_route(route);

        assert!(
            found,
            "added route (destination={destination:?}, interface_index={interface_index}) \
             was not present in routes() afterward.\n\
             Entries matching the destination (any interface): {near_matches:#?}\n\
             Raw rt_msghdr entries tagged with interface_index={interface_index}: {raw_headers:#?}\n\
             Raw rt_msghdr entries with RTA_DST=203.0.113.0 (any index): {raw_headers_by_dst:#?}\n\
             Full table ({} entries): {routes:#?}",
            routes.len(),
        );

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
