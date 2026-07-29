//! BSD/macOS backend for Net Lattice: implements `net-lattice-platform`'s provider
//! traits via route sockets.
//!
//! Only ever compiled for `target_os = "macos"`. See ARCHITECTURE.md for how
//! this crate binds `net-lattice-platform`'s generic `RouteProvider::Route`
//! associated type to the concrete `net_lattice_model::route::Route`.

#![cfg(target_os = "macos")]

use std::net::IpAddr;
use std::os::unix::io::{AsRawFd, RawFd};
use std::{io, mem, ptr};

use net_lattice_core::{Error, PlatformErrorCode, Result};
use net_lattice_model::route::{Route, RouteId};
use net_lattice_model::{IpAddress, Network};
use net_lattice_platform::RouteProvider;
use tokio::io::unix::AsyncFd;

const RTM_VERSION: i32 = 5;
const RTM_GET: i32 = 4;
const RTM_ADD: i32 = 1;
const RTM_DELETE: i32 = 2;

bitflags::bitflags! {
    struct Flags: i32 {
        const RTF_UP = 0x0001;
        const RTF_GATEWAY = 0x0002;
        const RTF_HOST = 0x0004;
        const RTF_REJECT = 0x0020;
        const RTF_IFSCOPE = 0x0080;
    }
}

/// The BSD/macOS route socket-backed implementation of Net Lattice's provider
/// traits.
pub struct DarwinBackend {
    runtime: tokio::runtime::Runtime,
    fd: AsyncFd<RawFd>,
}

impl DarwinBackend {
    pub fn new() -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new().map_err(darwin_error_code)?;
        let fd = unsafe { libc::socket(libc::PF_ROUTE, libc::SOCK_RAW, libc::AF_UNSPEC) };
        if fd < 0 {
            return Err(Error::Platform(PlatformErrorCode::Darwin(0)));
        }
        let async_fd = AsyncFd::new(fd).map_err(darwin_error_code)?;
        Ok(Self {
            runtime,
            fd: async_fd,
        })
    }

    async fn read_routes(&self) -> Result<Vec<Route>> {
        let mut buf = [0u8; 65536];
        loop {
            let mut guard = self.fd.readable().await.map_err(darwin_error_code)?;
            match guard.try_io(|inner| unsafe {
                let n = libc::recv(inner.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len(), 0);
                if n < 0 {
                    Err(Error::Platform(PlatformErrorCode::Darwin(*libc::errno())))
                } else {
                    Ok(n as usize)
                }
            }) {
                Ok(Ok(n)) => {
                    let mut routes = Vec::new();
                    let mut offset = 0usize;
                    while offset < n {
                        let hdr = &*(buf.as_ptr().add(offset) as *const libc::rt_msghdr);
                        if hdr.rtm_version != RTM_VERSION {
                            break;
                        }
                        if let Some(route) = unsafe { message_to_route(hdr) } {
                            routes.push(route);
                        }
                        offset += hdr.rtm_msglen as usize;
                    }
                    return Ok(routes);
                }
                Ok(Err(err)) => return Err(err),
                Err(_) => continue,
            }
        }
    }
}

impl Drop for DarwinBackend {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd.as_raw_fd()) };
    }
}

fn darwin_error_code(err: std::io::Error) -> PlatformErrorCode {
    PlatformErrorCode::Darwin(0)
}

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
    let mut interface_index = None;

    let mut ptr = (hdr as *const libc::rt_msghdr).add(1) as *const u8;
    let mut remaining = hdr.rtm_msglen as usize - mem::size_of::<libc::rt_msghdr>();
    let mut count = 0;
    while count < hdr.rtm_addrs && remaining >= mem::size_of::<libc::sockaddr>() {
        let sa = ptr as *const libc::sockaddr;
        let len = (*sa).sa_len as usize;
        if len == 0 || len > remaining {
            break;
        }
        if (hdr.rtm_flags & libc::RTF_GATEWAY as i32) != 0 {
            if gateway.is_none() {
                gateway = sockaddr_to_ip(sa);
            }
        } else if destination.is_none() {
            destination = sockaddr_to_ip(sa);
        }
        if interface_index.is_none() {
            interface_index = Some(hdr.rtm_index as u32);
        }
        ptr = ptr.add(len);
        remaining -= len;
        count += 1;
    }

    let destination = destination?;
    let destination = match destination {
        IpAddr::V4(addr) => {
            let ipv4 = net_lattice_ip::Ipv4Address::from(addr);
            let prefix = net_lattice_ip::Ipv4PrefixLength::new(32).ok()?;
            Network::from(net_lattice_ip::Ipv4Network::new(ipv4, prefix))
        }
        _ => return None,
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

impl RouteProvider for DarwinBackend {
    type Route = Route;

    fn routes(&self) -> Result<Vec<Self::Route>> {
        self.runtime.block_on(self.read_routes())
    }

    fn add_route(&self, _route: Self::Route) -> Result<()> {
        unimplemented!("Darwin RTM_ADD/RTM_DELETE are placeholders pending macOS-side verification")
    }

    fn remove_route(&self, _route: Self::Route) -> Result<()> {
        unimplemented!("Darwin RTM_ADD/RTM_DELETE are placeholders pending macOS-side verification")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
