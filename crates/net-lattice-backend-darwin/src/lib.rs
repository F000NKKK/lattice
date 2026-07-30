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
use std::sync::mpsc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::{io, mem};

use net_lattice_core::{Error, Id, PlatformErrorCode, Result};
use net_lattice_model::dns::DnsConfig;
use net_lattice_model::event::{ChangeKind, Event};
use net_lattice_model::ifaddr::{InterfaceAddress, InterfaceAddressId, NewInterfaceAddress};
use net_lattice_model::interface::{AdminState, Interface, InterfaceKind, OperationalState};
use net_lattice_model::mac::MacAddress;
use net_lattice_model::neighbor::{NeighborEntry, NeighborId, NeighborState};
use net_lattice_model::route::{Route, RouteId};
use net_lattice_model::{IpAddress, Network};
use net_lattice_platform::{
    AddressMutator, AddressProvider, Capability, CapabilityProvider, DnsProvider, EventProvider,
    EventReceiver, InterfaceProvider, NeighborProvider, RouteProvider,
};

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
const RTA_IFA: libc::c_int = 0x20;

const RTF_UP: libc::c_int = 0x0001;
const RTF_GATEWAY: libc::c_int = 0x0002;
const RTF_HOST: libc::c_int = 0x0004;
const RTF_STATIC: libc::c_int = 0x0800;

// `<net/route.h>`'s `RTF_REJECT` (unreachable, resolution failed) — not
// exposed as a named constant by the `libc` crate for `apple`.
const RTF_REJECT: libc::c_int = 0x0008;

const RTM_MAXSIZE: usize = 2048;

/// The initial four bytes shared by every PF_ROUTE message.  Reading this
/// small header first lets the watcher split a `read(2)` result containing
/// multiple route messages without assuming a particular message body.
#[repr(C)]
#[derive(Clone, Copy)]
struct RouteMessageHeader {
    msg_len: u16,
    version: u8,
    message_type: u8,
}

struct DarwinWatch {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for DarwinWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            // The reader polls with a bounded timeout, so joining cannot block
            // indefinitely and guarantees the PF_ROUTE fd is closed before
            // this subscription is gone.
            let _ = thread.join();
        }
    }
}

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

    // `NET_RT_DUMP` is a live snapshot. Between its size query and the
    // second call the table may grow (especially while another watcher/test
    // adds a route), in which case Darwin reports ENOMEM. Re-query rather
    // than surfacing a spurious failure to `routes()` callers.
    for _ in 0..4 {
        let mut needed = 0usize;
        let status = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                std::ptr::null_mut(),
                &mut needed,
                std::ptr::null_mut(),
                0,
            )
        };
        if status != 0 {
            return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
        }
        if needed == 0 {
            return Ok(Vec::new());
        }

        let mut buf = vec![0u8; needed];
        let status = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                buf.as_mut_ptr().cast(),
                &mut needed,
                std::ptr::null_mut(),
                0,
            )
        };
        if status == 0 {
            buf.truncate(needed);
            return Ok(buf);
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::ENOMEM) {
            return Err(Error::Platform(io_error_code(&err)));
        }
    }

    Err(Error::Platform(PlatformErrorCode::Darwin(libc::ENOMEM)))
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

    // Sockaddrs must appear in the message body in ascending `RTAX_*`
    // index order (DST=0, GATEWAY=1, NETMASK=2, ...) — this isn't just a
    // convention, the kernel's parser walks the buffer expecting exactly
    // that order for whichever bits are set in `rtm_addrs`. Gateway must
    // therefore be written (if present) *before* netmask, even though the
    // netmask's presence is decided by the same prefix-length check that
    // comes right after computing the destination.
    offset += push_sockaddr(&mut buf, offset, destination);

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

    if prefix_len == 32 || prefix_len == 128 {
        hdr.rtm_flags |= RTF_HOST;
    } else {
        hdr.rtm_addrs |= RTA_NETMASK;
        offset += push_netmask(&mut buf, offset, destination, prefix_len);
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

    // Same `RTAX_*` ascending-order requirement as `build_add_message`:
    // gateway (if present) must be written before netmask.
    offset += push_sockaddr(&mut buf, offset, destination);

    if let Some(gateway) = route.gateway.map(ip_address_to_std) {
        hdr.rtm_flags |= RTF_GATEWAY;
        hdr.rtm_addrs |= RTA_GATEWAY;
        offset += push_sockaddr(&mut buf, offset, gateway);
    }

    if prefix_len == 32 || prefix_len == 128 {
        hdr.rtm_flags |= RTF_HOST;
    } else {
        hdr.rtm_addrs |= RTA_NETMASK;
        offset += push_netmask(&mut buf, offset, destination, prefix_len);
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
    // Every sockaddr in a routing-socket message is padded to a 4-byte
    // boundary (`kernelAlign` in golang.org/x/net/route's `roundup`) — the
    // *declared* `sdl_len` is the unrounded significant length, but the
    // space actually occupied in the buffer (and thus how far the next
    // sockaddr's offset advances) is the rounded-up value. Returning the
    // unrounded `sdl_len` here previously left the following sockaddr
    // (`NETMASK`) at a misaligned offset, corrupting how the kernel parsed
    // everything after this one.
    let space = (sdl_len + 3) & !3;

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
    space
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

// `libc` deliberately does not expose these two add-address request types on
// Darwin.  They are stable public BSD user-space ABIs from `<net/if.h>` and
// `<netinet6/in6_var.h>` respectively.  Keeping the layouts here lets the
// backend use the native ioctl interface directly rather than shelling out to
// `ifconfig`.
#[repr(C)]
struct IfAliasReq {
    ifra_name: [libc::c_char; libc::IFNAMSIZ],
    ifra_addr: libc::sockaddr,
    ifra_broadaddr: libc::sockaddr,
    ifra_mask: libc::sockaddr,
}

#[repr(C)]
struct In6AliasReq {
    ifra_name: [libc::c_char; libc::IFNAMSIZ],
    ifra_addr: libc::sockaddr_in6,
    ifra_dstaddr: libc::sockaddr_in6,
    ifra_prefixmask: libc::sockaddr_in6,
    ifra_flags: libc::c_int,
    ifra_lifetime: libc::in6_addrlifetime,
}

fn siocaifaddr() -> libc::c_ulong {
    let size = mem::size_of::<IfAliasReq>() as libc::c_ulong;
    IOC_IN | ((size & IOCPARM_MASK) << 16) | ((b'i' as libc::c_ulong) << 8) | 26
}

fn siocdifaddr() -> libc::c_ulong {
    let size = mem::size_of::<libc::ifreq>() as libc::c_ulong;
    IOC_IN | ((size & IOCPARM_MASK) << 16) | ((b'i' as libc::c_ulong) << 8) | 25
}

fn siocaifaddr_in6() -> libc::c_ulong {
    let size = mem::size_of::<In6AliasReq>() as libc::c_ulong;
    IOC_IN | ((size & IOCPARM_MASK) << 16) | ((b'i' as libc::c_ulong) << 8) | 26
}

fn siocdifaddr_in6() -> libc::c_ulong {
    let size = mem::size_of::<libc::in6_ifreq>() as libc::c_ulong;
    IOC_IN | ((size & IOCPARM_MASK) << 16) | ((b'i' as libc::c_ulong) << 8) | 25
}

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

/// Placeholder identity scheme, same rationale as `synthesize_route_id`: an
/// interface address has no kernel-assigned numeric ID, so this hashes its
/// interface and network together.
fn synthesize_interface_address_id(interface_index: u32, network: &Network) -> InterfaceAddressId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    interface_index.hash(&mut hasher);
    network.hash(&mut hasher);
    InterfaceAddressId::new(hasher.finish())
}

/// Counts the leading `1` bits of a netmask address, byte by byte — the BSD
/// equivalent of `mask_bytes_to_prefix_len` but reading a fully-populated
/// `sockaddr` (as `getifaddrs`'s `ifa_netmask` always is, unlike a
/// routing-socket message's variable-length `RTA_NETMASK`) rather than a
/// possibly-truncated byte slice.
fn mask_ip_to_prefix_len(mask: IpAddr) -> u8 {
    match mask {
        IpAddr::V4(addr) => mask_bytes_to_prefix_len(&addr.octets()),
        IpAddr::V6(addr) => mask_bytes_to_prefix_len(&addr.octets()),
    }
}

/// Reads an interface's IP address, netmask, and (for IPv4) broadcast
/// address out of one `AF_INET`/`AF_INET6` entry in `getifaddrs`'s linked
/// list. Returns `None` for anything not `AF_INET`/`AF_INET6` (the same list
/// also carries one `AF_LINK` entry per interface, which
/// `link_entry_to_interface` reads instead) or where the address/netmask
/// can't be decoded.
unsafe fn ifaddr_entry_to_interface_address(entry: &libc::ifaddrs) -> Option<InterfaceAddress> {
    let sa = entry.ifa_addr;
    if sa.is_null() {
        return None;
    }
    let family = unsafe { (*sa).sa_family } as i32;
    if family != libc::AF_INET && family != libc::AF_INET6 {
        return None;
    }

    let address_addr = unsafe { sockaddr_to_ip(sa) }?;
    let netmask_addr = unsafe { sockaddr_to_ip(entry.ifa_netmask) }?;
    let prefix_len = mask_ip_to_prefix_len(netmask_addr);

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

    let interface_index = unsafe { libc::if_nametoindex(entry.ifa_name) };
    let mut interface_address = InterfaceAddress::new(
        synthesize_interface_address_id(interface_index, &network),
        interface_index,
        network,
    );

    // `ifa_dstaddr`/`ifa_broadaddr` are the same union field in
    // `<ifaddrs.h>` (`ifa_ifu.ifu_broadaddr`/`ifu_dstaddr`) — for a
    // broadcast-capable IPv4 interface (`IFF_BROADCAST`) it holds the
    // subnet broadcast address; IPv6 has no broadcast concept, so this is
    // skipped for that family even if the flag happens to be set.
    if family == libc::AF_INET
        && entry.ifa_flags & (libc::IFF_BROADCAST as u32) != 0
        && let Some(broadcast) = unsafe { sockaddr_to_ip(entry.ifa_dstaddr) }
    {
        interface_address = interface_address.with_broadcast(std_ip_to_ip_address(broadcast));
    }

    Some(interface_address)
}

impl AddressProvider for DarwinBackend {
    type InterfaceAddress = InterfaceAddress;

    fn addresses(&self) -> Result<Vec<Self::InterfaceAddress>> {
        let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
        let addresses = unsafe {
            if libc::getifaddrs(&mut head) != 0 {
                return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
            }

            let mut addresses = Vec::new();
            let mut cursor = head;
            while !cursor.is_null() {
                addresses.extend(ifaddr_entry_to_interface_address(&*cursor));
                cursor = (*cursor).ifa_next;
            }
            libc::freeifaddrs(head);
            addresses
        };
        Ok(addresses)
    }
}

fn address_mutation_error(error: io::Error) -> Error {
    match error.raw_os_error() {
        Some(libc::EPERM | libc::EACCES) => Error::PermissionDenied,
        Some(libc::EEXIST) => Error::AlreadyExists,
        Some(libc::ENOENT | libc::EADDRNOTAVAIL) => Error::NotFound,
        _ => Error::Platform(io_error_code(&error)),
    }
}

fn interface_name_array(interface_index: u32) -> Result<[libc::c_char; libc::IFNAMSIZ]> {
    let mut name = [0; libc::IFNAMSIZ];
    if unsafe { libc::if_indextoname(interface_index, name.as_mut_ptr()) }.is_null() {
        return Err(address_mutation_error(io::Error::last_os_error()));
    }
    Ok(name)
}

fn sockaddr_in(address: std::net::Ipv4Addr) -> libc::sockaddr_in {
    libc::sockaddr_in {
        sin_len: mem::size_of::<libc::sockaddr_in>() as u8,
        sin_family: libc::AF_INET as libc::sa_family_t,
        sin_port: 0,
        sin_addr: libc::in_addr {
            s_addr: u32::from_ne_bytes(address.octets()).to_be(),
        },
        sin_zero: [0; 8],
    }
}

fn sockaddr_in6(address: std::net::Ipv6Addr) -> libc::sockaddr_in6 {
    libc::sockaddr_in6 {
        sin6_len: mem::size_of::<libc::sockaddr_in6>() as u8,
        sin6_family: libc::AF_INET6 as libc::sa_family_t,
        sin6_port: 0,
        sin6_flowinfo: 0,
        sin6_addr: libc::in6_addr {
            s6_addr: address.octets(),
        },
        sin6_scope_id: 0,
    }
}

fn ipv4_mask(prefix_len: u8) -> std::net::Ipv4Addr {
    let bits = if prefix_len == 0 {
        0
    } else {
        u32::MAX << (32 - prefix_len)
    };
    std::net::Ipv4Addr::from(bits.to_be_bytes())
}

fn ipv6_mask(prefix_len: u8) -> std::net::Ipv6Addr {
    let mut octets = [0u8; 16];
    let whole_bytes = (prefix_len / 8) as usize;
    let remainder = prefix_len % 8;
    octets[..whole_bytes].fill(u8::MAX);
    if whole_bytes < octets.len() && remainder != 0 {
        octets[whole_bytes] = u8::MAX << (8 - remainder);
    }
    std::net::Ipv6Addr::from(octets)
}

fn socket_for_address(address: IpAddr) -> Result<libc::c_int> {
    let family = match address {
        IpAddr::V4(_) => libc::AF_INET,
        IpAddr::V6(_) => libc::AF_INET6,
    };
    let socket = unsafe { libc::socket(family, libc::SOCK_DGRAM, 0) };
    if socket < 0 {
        Err(address_mutation_error(io::Error::last_os_error()))
    } else {
        Ok(socket)
    }
}

fn add_interface_address(address: &NewInterfaceAddress) -> Result<()> {
    let (ip, prefix_len) = network_to_std(address.address);
    if matches!(ip, IpAddr::V6(_)) && address.broadcast.is_some() {
        return Err(Error::InvalidState);
    }
    let name = interface_name_array(address.interface_id.value() as u32)?;
    let socket = socket_for_address(ip)?;
    let result = match ip {
        IpAddr::V4(ip) => {
            let mut request: IfAliasReq = unsafe { mem::zeroed() };
            request.ifra_name = name;
            request.ifra_addr = unsafe { mem::transmute(sockaddr_in(ip)) };
            request.ifra_mask = unsafe { mem::transmute(sockaddr_in(ipv4_mask(prefix_len))) };
            if let Some(broadcast) = address.broadcast {
                request.ifra_broadaddr =
                    unsafe { mem::transmute(sockaddr_in(std::net::Ipv4Addr::from(broadcast))) };
            }
            unsafe { libc::ioctl(socket, siocaifaddr(), &mut request) }
        }
        IpAddr::V6(ip) => {
            let mut request: In6AliasReq = unsafe { mem::zeroed() };
            request.ifra_name = name;
            request.ifra_addr = sockaddr_in6(ip);
            request.ifra_prefixmask = sockaddr_in6(ipv6_mask(prefix_len));
            unsafe { libc::ioctl(socket, siocaifaddr_in6(), &mut request) }
        }
    };
    let error = if result == 0 {
        None
    } else {
        Some(address_mutation_error(io::Error::last_os_error()))
    };
    unsafe { libc::close(socket) };
    error.map_or(Ok(()), Err)
}

fn remove_interface_address(address: &InterfaceAddress) -> Result<()> {
    let (ip, _) = network_to_std(address.address);
    let name = interface_name_array(address.interface_index)?;
    let socket = socket_for_address(ip)?;
    let result = match ip {
        IpAddr::V4(ip) => {
            let mut request: libc::ifreq = unsafe { mem::zeroed() };
            request.ifr_name = name;
            request.ifr_ifru.ifru_addr = unsafe { mem::transmute(sockaddr_in(ip)) };
            unsafe { libc::ioctl(socket, siocdifaddr(), &mut request) }
        }
        IpAddr::V6(ip) => {
            let mut request: libc::in6_ifreq = unsafe { mem::zeroed() };
            request.ifr_name = name;
            request.ifr_ifru.ifru_addr = sockaddr_in6(ip);
            unsafe { libc::ioctl(socket, siocdifaddr_in6(), &mut request) }
        }
    };
    let error = if result == 0 {
        None
    } else {
        Some(address_mutation_error(io::Error::last_os_error()))
    };
    unsafe { libc::close(socket) };
    error.map_or(Ok(()), Err)
}

impl AddressMutator for DarwinBackend {
    type NewInterfaceAddress = NewInterfaceAddress;
    type InterfaceAddress = InterfaceAddress;

    fn add_address(&self, address: Self::NewInterfaceAddress) -> Result<Self::InterfaceAddress> {
        add_interface_address(&address)?;
        self.addresses()?
            .into_iter()
            .find(|observed| {
                observed.interface_index == address.interface_id.value() as u32
                    && observed.address == address.address
            })
            .ok_or(Error::InvalidState)
    }

    fn remove_address(&self, address: Self::InterfaceAddress) -> Result<()> {
        remove_interface_address(&address)
    }
}

/// Placeholder identity scheme, same rationale as `synthesize_route_id`: a
/// neighbor entry has no kernel-assigned numeric ID, so this hashes its
/// interface and address together.
fn synthesize_neighbor_id(interface_index: u32, address: &IpAddress) -> NeighborId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    interface_index.hash(&mut hasher);
    address.hash(&mut hasher);
    NeighborId::new(hasher.finish())
}

/// Reads the link-layer address out of an `AF_LINK` `sockaddr_dl`, the same
/// shape `link_entry_to_interface` reads a MAC out of, but for the gateway
/// slot of an ARP/NDP entry (`RTA_GATEWAY`) rather than an interface address.
unsafe fn sockaddr_dl_to_mac(sa: *const libc::sockaddr) -> Option<MacAddress> {
    if sa.is_null() || unsafe { (*sa).sa_family } as i32 != libc::AF_LINK {
        return None;
    }
    let sdl = unsafe { &*(sa as *const libc::sockaddr_dl) };
    if sdl.sdl_alen != 6 {
        return None;
    }
    let start = sdl.sdl_nlen as usize;
    let data = &sdl.sdl_data;
    if start + 6 > data.len() {
        return None;
    }
    let mut octets = [0u8; 6];
    for (i, octet) in octets.iter_mut().enumerate() {
        *octet = data[start + i] as u8;
    }
    Some(MacAddress::new(octets))
}

/// Derives a [`NeighborState`] from an ARP/NDP `rt_msghdr`'s flags. BSD/macOS
/// route sockets don't expose the full Neighbor Unreachability Detection
/// state machine (`Stale`/`Delay`/`Probe`) the way Linux's Netlink does —
/// only whether resolution has completed (a link-layer address is present)
/// and whether it failed (`RTF_REJECT`).
fn neighbor_flags_to_state(flags: libc::c_int, has_mac: bool) -> NeighborState {
    if flags & RTF_REJECT != 0 {
        NeighborState::Failed
    } else if !has_mac {
        NeighborState::Incomplete
    } else if flags & RTF_STATIC != 0 {
        NeighborState::Permanent
    } else {
        NeighborState::Reachable
    }
}

unsafe fn message_to_neighbor(hdr: &libc::rt_msghdr) -> Option<NeighborEntry> {
    let mut address = None;
    let mut mac = None;

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
        match bit {
            RTA_DST => {
                address = unsafe { sockaddr_to_ip(ptr as *const libc::sockaddr) }
                    .map(std_ip_to_ip_address);
            }
            RTA_GATEWAY => {
                mac = unsafe { sockaddr_dl_to_mac(ptr as *const libc::sockaddr) };
            }
            _ => {}
        }
        ptr = unsafe { ptr.add(aligned_len) };
        remaining -= aligned_len;
        bit <<= 1;
    }

    let address = address?;
    let interface_index = hdr.rtm_index as u32;
    let state = neighbor_flags_to_state(hdr.rtm_flags, mac.is_some());

    let mut entry = NeighborEntry::new(
        synthesize_neighbor_id(interface_index, &address),
        interface_index,
        address,
    )
    .with_state(state);
    if let Some(mac) = mac {
        entry = entry.with_mac(mac);
    }
    Some(entry)
}

/// Dumps the ARP (`AF_INET`) or NDP (`AF_INET6`) neighbor cache via
/// `sysctl(CTL_NET, PF_ROUTE, 0, family, NET_RT_FLAGS, RTF_LLINFO)` — the
/// same mechanism `arp -a`/`ndp -an` use. `RTF_LLINFO` filters the dump down
/// to link-layer-info (neighbor cache) entries, the same wire format
/// `dump_routing_table` returns for `NET_RT_DUMP`.
fn dump_neighbor_table(family: libc::c_int) -> Result<Vec<u8>> {
    let mut mib: [libc::c_int; 6] = [
        libc::CTL_NET,
        libc::PF_ROUTE,
        0,
        family,
        libc::NET_RT_FLAGS,
        libc::RTF_LLINFO,
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

impl NeighborProvider for DarwinBackend {
    type NeighborEntry = NeighborEntry;

    fn neighbors(&self) -> Result<Vec<Self::NeighborEntry>> {
        let mut neighbors = Vec::new();
        for family in [libc::AF_INET, libc::AF_INET6] {
            let buf = dump_neighbor_table(family)?;
            let mut offset = 0usize;
            while offset + mem::size_of::<libc::rt_msghdr>() <= buf.len() {
                let hdr = unsafe { &*(buf.as_ptr().add(offset) as *const libc::rt_msghdr) };
                let step = hdr.rtm_msglen as usize;
                if step == 0 {
                    break;
                }
                if let Some(entry) = unsafe { message_to_neighbor(hdr) } {
                    neighbors.push(entry);
                }
                offset += step;
            }
        }
        Ok(neighbors)
    }
}

/// Parses the `nameserver`/`search`/`domain` directives out of a
/// `resolv.conf`-format file (`man 5 resolv.conf`) — identical format to
/// Linux's, so this parser is shared verbatim with
/// `net-lattice-backend-linux`. On macOS `/etc/resolv.conf` is kept in sync
/// by `configd` from the dynamic store, so reading it reflects the system's
/// actual current resolver configuration even though it's not the source of
/// truth `scutil` would show.
///
/// `domain` is folded into `search_domains` too: it is the legacy
/// single-domain predecessor of `search` and every modern resolver treats a
/// lone `domain` entry as an implicit one-element search list.
fn parse_resolv_conf(contents: &str) -> DnsConfig {
    let mut config = DnsConfig::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("nameserver") => {
                if let Some(addr) = parts.next().and_then(|s| s.parse::<IpAddr>().ok()) {
                    config.nameservers.push(std_ip_to_ip_address(addr));
                }
            }
            Some("search") | Some("domain") => {
                config
                    .search_domains
                    .extend(parts.map(|domain| domain.to_string()));
            }
            _ => {}
        }
    }
    config
}

fn resolv_conf_error(err: &io::Error) -> Error {
    match err.kind() {
        io::ErrorKind::NotFound => Error::NotFound,
        io::ErrorKind::PermissionDenied => Error::PermissionDenied,
        _ => Error::Platform(io_error_code(err)),
    }
}

/// Extracts the address identity from a `RTM_NEWADDR`/`RTM_DELADDR` message.
/// The routing socket includes the interface index in its fixed header and
/// the address/netmask as `RTAX_IFA`/`RTAX_NETMASK` sockaddrs.
unsafe fn message_to_interface_address_id(hdr: &libc::ifa_msghdr) -> Option<InterfaceAddressId> {
    let mut address = None;
    let mut netmask_bytes = None;
    let mut ptr = unsafe { (hdr as *const libc::ifa_msghdr).add(1).cast::<u8>() };
    let mut remaining = hdr.ifam_msglen as usize - mem::size_of::<libc::ifa_msghdr>();
    let mut bit: libc::c_int = 1;
    while bit <= hdr.ifam_addrs && remaining > 0 {
        if hdr.ifam_addrs & bit == 0 {
            bit <<= 1;
            continue;
        }
        let sa_len = unsafe { *ptr } as usize;
        let aligned_len = if sa_len == 0 { 4 } else { (sa_len + 3) & !3 };
        if aligned_len > remaining {
            return None;
        }
        if bit == RTA_IFA {
            address = unsafe { sockaddr_to_ip(ptr.cast()) };
        } else if bit == RTA_NETMASK {
            let header = match address {
                Some(IpAddr::V6(_)) => 8,
                _ => 4,
            };
            let bytes = sa_len.saturating_sub(header);
            netmask_bytes = Some(if bytes == 0 {
                Vec::new()
            } else {
                unsafe { std::slice::from_raw_parts(ptr.add(header), bytes) }.to_vec()
            });
        }
        ptr = unsafe { ptr.add(aligned_len) };
        remaining -= aligned_len;
        bit <<= 1;
    }

    let address = address?;
    let prefix_len = netmask_bytes
        .as_deref()
        .map(mask_bytes_to_prefix_len)
        .unwrap_or_else(|| match address {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        });
    let network = match address {
        IpAddr::V4(address) => Network::from(net_lattice_ip::Ipv4Network::new(
            address.into(),
            net_lattice_ip::Ipv4PrefixLength::new(prefix_len)?,
        )),
        IpAddr::V6(address) => Network::from(net_lattice_ip::Ipv6Network::new(
            address.into(),
            net_lattice_ip::Ipv6PrefixLength::new(prefix_len)?,
        )),
    };
    Some(synthesize_interface_address_id(
        hdr.ifam_index as u32,
        &network,
    ))
}

/// Maps one native routing-socket notification to the cross-platform signal.
/// `RTM_ADD` is also emitted for changes by BSD, so it deliberately maps to
/// `Changed`; deleting a route/address/ARP entry is unambiguous.
unsafe fn route_socket_message_to_event(message: *const u8, len: usize) -> Option<Event> {
    if len < mem::size_of::<RouteMessageHeader>() {
        return None;
    }
    let common = unsafe { std::ptr::read_unaligned(message.cast::<RouteMessageHeader>()) };
    match common.message_type as libc::c_int {
        libc::RTM_ADD => {
            if len < mem::size_of::<libc::rt_msghdr>() {
                return None;
            }
            let hdr = unsafe { &*message.cast::<libc::rt_msghdr>() };
            if hdr.rtm_flags & libc::RTF_LLINFO != 0 {
                unsafe { message_to_neighbor(hdr) }.map(|neighbor| Event::Neighbor {
                    id: neighbor.id,
                    kind: ChangeKind::Changed,
                })
            } else {
                unsafe { message_to_route(hdr) }.map(|route| Event::Route {
                    id: route.id,
                    kind: ChangeKind::Changed,
                })
            }
        }
        libc::RTM_DELETE => {
            if len < mem::size_of::<libc::rt_msghdr>() {
                return None;
            }
            let hdr = unsafe { &*message.cast::<libc::rt_msghdr>() };
            if hdr.rtm_flags & libc::RTF_LLINFO != 0 {
                unsafe { message_to_neighbor(hdr) }.map(|neighbor| Event::Neighbor {
                    id: neighbor.id,
                    kind: ChangeKind::Removed,
                })
            } else {
                unsafe { message_to_route(hdr) }.map(|route| Event::Route {
                    id: route.id,
                    kind: ChangeKind::Removed,
                })
            }
        }
        libc::RTM_IFINFO | libc::RTM_IFINFO2 => {
            if len < mem::size_of::<libc::if_msghdr>() {
                return None;
            }
            let hdr = unsafe { &*message.cast::<libc::if_msghdr>() };
            Some(Event::Interface {
                id: Id::new(hdr.ifm_index as u64),
                kind: ChangeKind::Changed,
            })
        }
        libc::RTM_NEWADDR | libc::RTM_DELADDR => {
            if len < mem::size_of::<libc::ifa_msghdr>() {
                return None;
            }
            let hdr = unsafe { &*message.cast::<libc::ifa_msghdr>() };
            unsafe { message_to_interface_address_id(hdr) }.map(|id| Event::Address {
                id,
                kind: if common.message_type as libc::c_int == libc::RTM_DELADDR {
                    ChangeKind::Removed
                } else {
                    ChangeKind::Changed
                },
            })
        }
        _ => None,
    }
}

impl CapabilityProvider for DarwinBackend {
    /// `IPV6` unconditionally, same rationale as the Linux backend: every
    /// provider this backend implements already handles both address
    /// families. `MONITORING` is available through a dedicated PF_ROUTE
    /// reader. `VRF`/`NAMESPACES` are left unset: BSD/macOS has no direct
    /// equivalent and Net Lattice does not implement either domain.
    /// equivalent to begin with.
    fn capabilities(&self) -> Capability {
        Capability::IPV6 | Capability::MONITORING
    }
}

impl EventProvider for DarwinBackend {
    type Event = Event;

    fn watch(&self) -> Result<EventReceiver<Self::Event>> {
        // Never read from `self.fd`: it carries replies to this backend's
        // route mutations as well as system broadcasts. A dedicated socket
        // makes this subscription independent and avoids stealing replies
        // from route operations.
        let fd = unsafe { libc::socket(libc::PF_ROUTE, libc::SOCK_RAW, libc::AF_UNSPEC) };
        if fd < 0 {
            return Err(Error::Platform(io_error_code(&io::Error::last_os_error())));
        }

        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            // `c_long` gives this receive buffer at least the alignment of all
            // PF_ROUTE headers. The native parsers below can then safely view
            // each 4-byte-aligned message as its concrete header type.
            let words = RTM_MAXSIZE.div_ceil(mem::size_of::<libc::c_long>());
            let mut buffer = vec![0 as libc::c_long; words];
            while !thread_stop.load(Ordering::Acquire) {
                let mut poll_fd = libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                let ready = unsafe { libc::poll(&mut poll_fd, 1, 200) };
                if ready == 0
                    || (ready < 0
                        && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted)
                {
                    continue;
                }
                if ready < 0 {
                    break;
                }
                let received =
                    unsafe { libc::recv(fd, buffer.as_mut_ptr().cast(), RTM_MAXSIZE, 0) };
                if received <= 0 {
                    if received < 0
                        && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted
                    {
                        continue;
                    }
                    break;
                }
                let mut offset = 0usize;
                let received = received as usize;
                while offset + mem::size_of::<RouteMessageHeader>() <= received {
                    let message = unsafe { buffer.as_ptr().cast::<u8>().add(offset) };
                    let common =
                        unsafe { std::ptr::read_unaligned(message.cast::<RouteMessageHeader>()) };
                    let len = common.msg_len as usize;
                    if len < mem::size_of::<RouteMessageHeader>() || offset + len > received {
                        break;
                    }
                    if common.version == RTM_VERSION
                        && let Some(event) = unsafe { route_socket_message_to_event(message, len) }
                        && sender.send(event).is_err()
                    {
                        unsafe { libc::close(fd) };
                        return;
                    }
                    offset += (len + 3) & !3;
                }
            }
            unsafe { libc::close(fd) };
        });

        Ok(EventReceiver::with_subscription(
            receiver,
            DarwinWatch {
                stop,
                thread: Some(thread),
            },
        ))
    }
}

impl DnsProvider for DarwinBackend {
    type DnsConfig = DnsConfig;

    fn dns_config(&self) -> Result<Self::DnsConfig> {
        let contents =
            std::fs::read_to_string("/etc/resolv.conf").map_err(|err| resolv_conf_error(&err))?;
        Ok(parse_resolv_conf(&contents))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_lattice_ip::{Ipv4Address, Ipv4Network, Ipv4PrefixLength};

    /// Exercises a real round trip through `getifaddrs`, no privilege
    /// required: every macOS system has `lo0`'s `127.0.0.1/8`.
    #[test]
    fn addresses_includes_loopbacks_address() {
        let backend = DarwinBackend::new().expect("failed to open a route socket");
        let addresses = backend
            .addresses()
            .expect("getifaddrs should not require privilege");
        assert!(
            addresses.iter().any(|addr| matches!(
                addr.address,
                Network::V4(net) if net.address() == Ipv4Address::new(127, 0, 0, 1)
            )),
            "expected `127.0.0.1` among the assigned addresses, got: {addresses:?}"
        );
    }

    /// Exercises a real round trip through the route socket, no privilege
    /// required: `NET_RT_FLAGS` dumps are readable by any user.
    #[test]
    fn neighbors_reads_the_real_kernel_neighbor_table() {
        let backend = DarwinBackend::new().expect("failed to open a route socket");
        let neighbors = backend
            .neighbors()
            .expect("NET_RT_FLAGS dump should not require privilege");
        // Not asserting on contents: the neighbor table of the machine
        // running this test is arbitrary (may even be empty). Reaching here
        // without an error is the assertion.
        let _ = neighbors;
    }

    #[test]
    fn parse_resolv_conf_reads_nameservers_and_search_domains() {
        let contents = "# comment\nnameserver 1.1.1.1\nsearch example.com corp.example.com\n";
        let config = parse_resolv_conf(contents);
        assert_eq!(
            config.nameservers,
            vec![IpAddress::from(Ipv4Address::new(1, 1, 1, 1))]
        );
        assert_eq!(
            config.search_domains,
            vec!["example.com".to_string(), "corp.example.com".to_string()]
        );
    }

    /// Reads the real `/etc/resolv.conf` present on this test environment.
    /// Every macOS system has one, kept in sync by `configd`.
    #[test]
    fn dns_config_reads_the_real_resolv_conf() {
        let backend = DarwinBackend::new().expect("failed to open a route socket");
        let config = backend
            .dns_config()
            .expect("/etc/resolv.conf should be readable");
        let _ = config;
    }

    /// Opens and drops a dedicated PF_ROUTE event socket without modifying
    /// system state. The ignored test below verifies delivery end-to-end.
    #[test]
    fn watch_opens_a_real_route_socket() {
        let backend = DarwinBackend::new().expect("failed to open a route socket");
        assert!(backend.capabilities().contains(Capability::MONITORING));
        drop(backend.watch().expect("failed to open PF_ROUTE watcher"));
    }

    /// Diagnostic-only: formats `bytes` as a space-separated hex dump.
    fn hex_dump(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

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

        // Diagnostic-only: three rounds of fixes reasoned from reference
        // implementations (netmask parsing, sockaddr_dl shape/sdl_len/name,
        // RTAX wire order) have each been individually correct against
        // those references, yet the round trip still fails identically —
        // time to stop reasoning from references and look at the actual
        // bytes this run puts on the wire and gets back. A "spy" socket
        // opened before `add_route` receives the same kernel broadcast
        // `send_route_request` consumes on `backend`'s own socket (every
        // open `PF_ROUTE` socket gets every message), without touching any
        // production code path.
        let add_message = build_add_message(&route).expect("build_add_message failed");
        eprintln!(
            "[diag] outgoing RTM_ADD ({} bytes): {}",
            add_message.len(),
            hex_dump(&add_message)
        );
        let spy_fd = unsafe { libc::socket(libc::PF_ROUTE, libc::SOCK_RAW, libc::AF_UNSPEC) };
        assert!(spy_fd >= 0, "failed to open spy route socket");
        let timeout = libc::timeval {
            tv_sec: 2,
            tv_usec: 0,
        };
        unsafe {
            libc::setsockopt(
                spy_fd,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &timeout as *const _ as *const libc::c_void,
                mem::size_of::<libc::timeval>() as libc::socklen_t,
            );
        }

        let add_result = backend.add_route(route.clone());

        let mut spy_buf = [0u8; RTM_MAXSIZE];
        let mut spy_seen = Vec::new();
        for _ in 0..8 {
            let n = unsafe { libc::recv(spy_fd, spy_buf.as_mut_ptr() as *mut _, spy_buf.len(), 0) };
            if n <= 0 {
                break;
            }
            let n = n as usize;
            if n >= mem::size_of::<libc::rt_msghdr>() {
                let hdr = unsafe { &*(spy_buf.as_ptr() as *const libc::rt_msghdr) };
                if hdr.rtm_pid == unsafe { libc::getpid() } {
                    spy_seen.push(format!(
                        "type={} flags={:#x} addrs={:#x} msglen={} index={} errno={} bytes={}",
                        hdr.rtm_type,
                        hdr.rtm_flags,
                        hdr.rtm_addrs,
                        hdr.rtm_msglen,
                        hdr.rtm_index,
                        hdr.rtm_errno,
                        hex_dump(&spy_buf[..n]),
                    ));
                }
            }
        }
        unsafe { libc::close(spy_fd) };
        eprintln!("[diag] spy socket saw (our pid only): {spy_seen:#?}");
        // Fail loudly on *any* error rather than only `PermissionDenied`/
        // `Platform(_)`: this test genuinely swallowed a real
        // `Error::AlreadyExists` from the kernel this way once already
        // (masking a `push_link_gateway` alignment bug that corrupted
        // every add past the gateway sockaddr) — the destination was
        // pre-cleaned immediately above, so any error here is real.
        add_result.expect("add_route failed - are you running as root?");

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

    /// End-to-end monitoring verification: a route mutation must arrive on a
    /// second PF_ROUTE socket and be delivered by `watch()`. It requires root
    /// because modifying the routing table is privileged on macOS.
    #[test]
    #[ignore = "requires root; run with `sudo -E cargo test -p net-lattice-backend-darwin watch_observes_route_changes -- --ignored`"]
    fn watch_observes_route_changes() {
        use std::time::Duration;

        let backend = DarwinBackend::new().expect("failed to open a route socket");
        assert!(backend.capabilities().contains(Capability::MONITORING));
        let watcher = backend.watch().expect("failed to open PF_ROUTE watcher");
        let interface_index = loopback_interface_index(&backend);
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
        let watched_id = synthesize_route_id(&destination, &None, Some(interface_index));

        let observed = (0..12).any(|_| {
            matches!(
                watcher.recv_timeout(Duration::from_millis(250)),
                Ok(Some(Event::Route { id, .. })) if id == watched_id
            )
        });
        let _ = backend.remove_route(route);
        assert!(observed, "watch() did not report the route mutation");
    }
}
