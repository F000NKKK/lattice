//! BSD/macOS backend for Net Lattice: implements `net-lattice-platform`'s provider
//! traits via route sockets.
//!
//! Only ever compiled for `target_os = "macos"`. See ARCHITECTURE.md for how
//! this crate binds `net-lattice-platform`'s generic `RouteProvider::Route`
//! associated type to the concrete `net_lattice_model::route::Route`.

#![cfg(target_os = "macos")]

use std::net::IpAddr;
use std::os::unix::io::{AsRawFd, RawFd};
use std::{io, mem};

use net_lattice_core::{Error, PlatformErrorCode, Result};
use net_lattice_model::route::{Route, RouteId};
use net_lattice_model::{IpAddress, Network};
use net_lattice_platform::RouteProvider;
use tokio::io::unix::AsyncFd;

const RTM_VERSION: u8 = 5;
const RTM_GET: u8 = 4;
const RTM_ADD: u8 = 1;
const RTM_DELETE: u8 = 2;

const RTA_DST: u32 = 0x1;
const RTA_GATEWAY: u32 = 0x2;
const RTA_NETMASK: u32 = 0x4;
const RTA_IFP: u32 = 0x10;

const RTF_UP: u32 = 0x0001;
const RTF_GATEWAY: u32 = 0x0002;
const RTF_HOST: u32 = 0x0004;
const RTF_STATIC: u32 = 0x0800;

const RTM_MAXSIZE: usize = 2048;

/// The BSD/macOS route socket-backed implementation of Net Lattice's provider
/// traits.
pub struct DarwinBackend {
    runtime: tokio::runtime::Runtime,
    fd: AsyncFd<RawFd>,
}

impl DarwinBackend {
    pub fn new() -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new().map_err(darwin_error_code)?;
        let _guard = runtime.enter();
        let fd = unsafe { libc::socket(libc::PF_ROUTE, libc::SOCK_RAW, libc::AF_UNSPEC) };
        if fd < 0 {
            return Err(Error::Platform(PlatformErrorCode::Darwin(
                io::Error::last_os_error().raw_os_error().unwrap_or(0),
            )));
        }
        let async_fd = AsyncFd::new(fd).map_err(darwin_error_code)?;
        Ok(Self {
            runtime,
            fd: async_fd,
        })
    }

    async fn read_routes(&self) -> Result<Vec<Route>> {
        let request = build_get_request();
        self.send_message(&request)?;

        let mut routes = Vec::new();
        let mut buf = [0u8; 65536];
        loop {
            let mut guard = self.fd.readable().await.map_err(darwin_error_code)?;
            match guard.try_io(|inner| unsafe {
                let n = libc::recv(inner.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len(), 0);
                if n < 0 {
                    Err(Error::Platform(PlatformErrorCode::Darwin(
                        io::Error::last_os_error().raw_os_error().unwrap_or(0),
                    )))
                } else {
                    Ok(n as usize)
                }
            }) {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => {
                    let mut offset = 0usize;
                    while offset < n {
                        let hdr = unsafe { &*(buf.as_ptr().add(offset) as *const libc::rt_msghdr) };
                        if hdr.rtm_version != RTM_VERSION {
                            break;
                        }
                        if hdr.rtm_type == RTM_GET {
                            if let Some(route) = unsafe { message_to_route(hdr) } {
                                routes.push(route);
                            }
                        }
                        let step = hdr.rtm_msglen as usize;
                        if step == 0 {
                            break;
                        }
                        offset += step;
                    }
                }
                Ok(Err(err)) => return Err(err),
                Err(_) => continue,
            }
        }
        Ok(routes)
    }

    async fn add_route_impl(&self, route: Route) -> Result<()> {
        let message = build_add_message(&route)?;
        self.send_message(&message)
    }

    async fn remove_route_impl(&self, route: Route) -> Result<()> {
        let message = build_delete_message(&route)?;
        self.send_message(&message)
    }

    fn send_message(&self, message: &[u8]) -> Result<()> {
        let n = unsafe {
            libc::send(
                self.fd.as_raw_fd(),
                message.as_ptr() as *const _,
                message.len(),
                0,
            )
        };
        if n < 0 {
            return Err(Error::Platform(PlatformErrorCode::Darwin(
                io::Error::last_os_error().raw_os_error().unwrap_or(0),
            )));
        }
        Ok(())
    }
}

impl Drop for DarwinBackend {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd.as_raw_fd()) };
    }
}

fn darwin_error_code(err: std::io::Error) -> PlatformErrorCode {
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
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    destination.hash(&mut hasher);
    gateway.hash(&mut hasher);
    interface_index.hash(&mut hasher);
    RouteId::new(hasher.finish())
}

unsafe fn message_to_route(hdr: &libc::rt_msghdr) -> Option<Route> {
    let mut destination = None;
    let mut gateway = None;
    let interface_index = if hdr.rtm_index != 0 {
        Some(hdr.rtm_index as u32)
    } else {
        None
    };

    let mut ptr = (hdr as *const libc::rt_msghdr).add(1) as *const u8;
    let mut remaining = hdr.rtm_msglen as usize - mem::size_of::<libc::rt_msghdr>();
    let mut bit = 1u32;
    let mut count = 0u32;
    while count < 32 && remaining >= mem::size_of::<libc::sockaddr>() {
        if hdr.rtm_addrs & bit == 0 {
            bit <<= 1;
            count += 1;
            continue;
        }
        let sa = ptr as *const libc::sockaddr;
        let len = (*sa).sa_len as usize;
        let aligned_len = (len + 3) & !3;
        if aligned_len == 0 || aligned_len > remaining {
            break;
        }
        match bit {
            RTA_DST => {
                destination = sockaddr_to_ip(sa);
            }
            RTA_GATEWAY => {
                gateway = sockaddr_to_ip(sa);
            }
            _ => {}
        }
        ptr = ptr.add(aligned_len);
        remaining -= aligned_len;
        bit <<= 1;
        count += 1;
    }

    let destination = destination?;
    let prefix_len = if (hdr.rtm_flags & RTF_HOST) != 0 {
        32u8
    } else {
        32u8
    };
    let destination = match destination {
        IpAddr::V4(addr) => {
            let ipv4 = net_lattice_ip::Ipv4Address::from(addr);
            let prefix = net_lattice_ip::Ipv4PrefixLength::new(prefix_len).ok()?;
            Network::from(net_lattice_ip::Ipv4Network::new(ipv4, prefix))
        }
        IpAddr::V6(addr) => {
            let ipv6 = net_lattice_ip::Ipv6Address::from(addr);
            let prefix = net_lattice_ip::Ipv6PrefixLength::new(128).ok()?;
            Network::from(net_lattice_ip::Ipv6Network::new(ipv6, prefix))
        }
    };
    let gateway = gateway.map(|ip| match ip {
        IpAddr::V4(addr) => IpAddress::from(net_lattice_ip::Ipv4Address::from(addr)),
        IpAddr::V6(addr) => IpAddress::from(net_lattice_ip::Ipv6Address::from(addr)),
    });
    let id = synthesize_route_id(&destination, &gateway, interface_index);
    let mut route = Route::new(id, destination);
    if let Some(gateway) = gateway {
        route = route.with_gateway(gateway);
    }
    if let Some(interface_index) = interface_index {
        route = route.with_interface_index(interface_index);
    }
    Some(route)
}

unsafe fn sockaddr_to_ip(sa: *const libc::sockaddr) -> Option<IpAddr> {
    if sa.is_null() {
        return None;
    }
    let family = (*sa).sa_family;
    match family {
        libc::AF_INET => {
            let sin = &*(sa as *const libc::sockaddr_in);
            let octets = u32::from_be(sin.sin_addr.s_addr).to_be_bytes();
            Some(IpAddr::V4(std::net::Ipv4Addr::new(
                octets[0], octets[1], octets[2], octets[3],
            )))
        }
        libc::AF_INET6 => {
            let sin6 = &*(sa as *const libc::sockaddr_in6);
            let bytes = sin6.sin6_addr.s6_addr;
            Some(IpAddr::V6(std::net::Ipv6Addr::from(bytes)))
        }
        _ => None,
    }
}

fn build_get_request() -> Vec<u8> {
    let mut buf = vec![0u8; mem::size_of::<libc::rt_msghdr>()];
    let hdr = unsafe { &mut *(buf.as_mut_ptr() as *mut libc::rt_msghdr) };
    hdr.rtm_msglen = mem::size_of::<libc::rt_msghdr>() as u16;
    hdr.rtm_version = RTM_VERSION;
    hdr.rtm_type = RTM_GET;
    hdr.rtm_flags = RTF_UP as u32;
    hdr.rtm_addrs = 0;
    hdr.rtm_pid = unsafe { libc::getpid() };
    hdr.rtm_seq = 1;
    hdr.rtm_index = 0;
    buf
}

fn build_add_message(route: &Route) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; RTM_MAXSIZE];
    let hdr = unsafe { &mut *(buf.as_mut_ptr() as *mut libc::rt_msghdr) };
    hdr.rtm_version = RTM_VERSION;
    hdr.rtm_type = RTM_ADD;
    hdr.rtm_pid = unsafe { libc::getpid() };
    hdr.rtm_seq = 2;
    hdr.rtm_flags = RTF_UP | RTF_STATIC;
    hdr.rtm_addrs = RTA_DST;
    let mut offset = mem::size_of::<libc::rt_msghdr>();

    let (dst_addr, prefix_len) = match route.destination {
        Network::V4(ref net) => {
            let addr = net.address();
            let octets: [u8; 4] = addr.octets();
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
            offset += mem::size_of::<libc::sockaddr_in>();
            (net.address(), net.prefix().value())
        }
        Network::V6(ref net) => {
            let addr = net.address();
            let octets: [u8; 16] = addr.octets();
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
            offset += mem::size_of::<libc::sockaddr_in6>();
            (net.address(), net.prefix().value())
        }
    };
    let _ = dst_addr;

    if prefix_len == 32 || prefix_len == 128 {
        hdr.rtm_flags |= RTF_HOST;
    } else {
        hdr.rtm_addrs |= RTA_NETMASK;
        let mask = if prefix_len <= 32 {
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
            offset += mem::size_of::<libc::sockaddr_in>();
        } else {
            let mut mask_bytes = [0u8; 16];
            let full_bytes = (prefix_len / 8) as usize;
            let remainder = prefix_len % 8;
            for i in 0..full_bytes {
                mask_bytes[i] = 0xff;
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
            offset += mem::size_of::<libc::sockaddr_in6>();
        };
    }

    if let Some(ref gateway) = route.gateway {
        hdr.rtm_flags |= RTF_GATEWAY;
        hdr.rtm_addrs |= RTA_GATEWAY;
        match gateway {
            IpAddress::V4(ref addr) => {
                let octets: [u8; 4] = addr.octets();
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
                offset += mem::size_of::<libc::sockaddr_in>();
            }
            IpAddress::V6(ref addr) => {
                let octets: [u8; 16] = addr.octets();
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
                offset += mem::size_of::<libc::sockaddr_in6>();
            }
        }
    }

    if let Some(interface_index) = route.interface_index {
        hdr.rtm_index = interface_index as u16;
    }

    hdr.rtm_msglen = offset as u16;
    buf.truncate(offset);
    Ok(buf)
}

fn build_delete_message(route: &Route) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; RTM_MAXSIZE];
    let hdr = unsafe { &mut *(buf.as_mut_ptr() as *mut libc::rt_msghdr) };
    hdr.rtm_version = RTM_VERSION;
    hdr.rtm_type = RTM_DELETE;
    hdr.rtm_pid = unsafe { libc::getpid() };
    hdr.rtm_seq = 3;
    hdr.rtm_flags = RTF_UP;
    hdr.rtm_addrs = RTA_DST;
    let mut offset = mem::size_of::<libc::rt_msghdr>();

    let prefix_len = match route.destination {
        Network::V4(ref net) => {
            let addr = net.address();
            let octets: [u8; 4] = addr.octets();
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
            offset += mem::size_of::<libc::sockaddr_in>();
            net.prefix().value()
        }
        Network::V6(ref net) => {
            let addr = net.address();
            let octets: [u8; 16] = addr.octets();
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
            offset += mem::size_of::<libc::sockaddr_in6>();
            net.prefix().value()
        }
    };

    if prefix_len == 32 || prefix_len == 128 {
        hdr.rtm_flags |= RTF_HOST;
    } else {
        hdr.rtm_addrs |= RTA_NETMASK;
        if prefix_len <= 32 {
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
            offset += mem::size_of::<libc::sockaddr_in>();
        } else {
            let mut mask_bytes = [0u8; 16];
            let full_bytes = (prefix_len / 8) as usize;
            let remainder = prefix_len % 8;
            for i in 0..full_bytes {
                mask_bytes[i] = 0xff;
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
            offset += mem::size_of::<libc::sockaddr_in6>();
        }
    }

    if let Some(ref gateway) = route.gateway {
        hdr.rtm_flags |= RTF_GATEWAY;
        hdr.rtm_addrs |= RTA_GATEWAY;
        match gateway {
            IpAddress::V4(ref addr) => {
                let octets: [u8; 4] = addr.octets();
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
                offset += mem::size_of::<libc::sockaddr_in>();
            }
            IpAddress::V6(ref addr) => {
                let octets: [u8; 16] = addr.octets();
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
                offset += mem::size_of::<libc::sockaddr_in6>();
            }
        }
    }

    if let Some(interface_index) = route.interface_index {
        hdr.rtm_index = interface_index as u16;
    }

    hdr.rtm_msglen = offset as u16;
    buf.truncate(offset);
    Ok(buf)
}

impl RouteProvider for DarwinBackend {
    type Route = Route;

    fn routes(&self) -> Result<Vec<Self::Route>> {
        self.runtime.block_on(self.read_routes())
    }

    fn add_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(self.add_route_impl(route))
    }

    fn remove_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(self.remove_route_impl(route))
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
        let _ = routes;
    }

    /// Requires `root` privileges. Not run by default because most
    /// development and CI environments don't grant it, and this test would
    /// otherwise fail with `PermissionDenied` rather than being skipped.
    ///
    /// Uses a documentation-only prefix (RFC 5737 `203.0.113.0/24`,
    /// TEST-NET-3) on `lo0` so it can't collide with or disrupt real
    /// routing, and removes what it added regardless of assertion outcome.
    #[test]
    #[ignore = "requires root; run with `sudo -E cargo test -p net-lattice-backend-darwin -- --ignored`"]
    fn add_then_remove_route_round_trips_through_the_kernel() {
        let backend = DarwinBackend::new().expect("failed to open a route socket");
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
            add_result.expect("add_route failed - are you running as root?");
        }

        let routes = backend
            .routes()
            .expect("routes() failed after add_route succeeded");
        let found = routes
            .iter()
            .any(|r| r.destination == destination && r.interface_index == Some(loopback_index));

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
