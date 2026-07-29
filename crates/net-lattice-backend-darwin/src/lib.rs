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

const RTM_VERSION: i32 = 5;

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
    fd: RawFd,
}

impl DarwinBackend {
    pub fn new() -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new().map_err(darwin_error_code)?;
        let fd = unsafe { libc::socket(libc::PF_ROUTE, libc::SOCK_RAW, libc::AF_UNSPEC) };
        if fd < 0 {
            return Err(Error::Platform(PlatformErrorCode::Darwin(0)));
        }
        Ok(Self { runtime, fd })
    }
}

impl Drop for DarwinBackend {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
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

#[repr(C)]
struct RtMsg {
    header: libc::rt_msghdr,
    data: [u8; 1024],
}

impl RtMsg {
    fn for_route(
        destination: Network,
        gateway: Option<IpAddress>,
        interface_index: Option<u32>,
        add: bool,
    ) -> Self {
        let mut msg = Self {
            header: unsafe { mem::zeroed() },
            data: [0u8; 1024],
        };
        // This is a simplified placeholder: real implementation would
        // serialize sockaddr structs into `data` and set header fields
        // accordingly.
        let _ = destination;
        let _ = gateway;
        let _ = interface_index;
        let _ = add;
        msg
    }
}

impl RouteProvider for DarwinBackend {
    type Route = Route;

    fn routes(&self) -> Result<Vec<Self::Route>> {
        self.runtime.block_on(async {
            let _ = self.fd;
            unimplemented!("Darwin route socket reading is not implemented yet")
        })
    }

    fn add_route(&self, _route: Self::Route) -> Result<()> {
        unimplemented!("Darwin route socket add is not implemented yet")
    }

    fn remove_route(&self, _route: Self::Route) -> Result<()> {
        unimplemented!("Darwin route socket remove is not implemented yet")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
