//! BSD/macOS backend for Net Lattice: implements `net-lattice-platform`'s provider
//! traits via route sockets.
//!
//! Only ever compiled for `target_os = "macos"`. See ARCHITECTURE.md for how
//! this crate binds `net-lattice-platform`'s generic `RouteProvider::Route`
//! associated type to the concrete `net_lattice_model::route::Route`.

#![cfg(target_os = "macos")]

use std::net::IpAddr;

use net_lattice_core::{Error, PlatformErrorCode, Result};
use net_lattice_model::route::{Route, RouteId};
use net_lattice_model::{IpAddress, Network};
use net_lattice_platform::RouteProvider;

/// The BSD/macOS route socket-backed implementation of Net Lattice's provider
/// traits.
pub struct DarwinBackend {
    runtime: tokio::runtime::Runtime,
}

impl DarwinBackend {
    pub fn new() -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new().map_err(darwin_error_code)?;
        Ok(Self { runtime })
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

impl RouteProvider for DarwinBackend {
    type Route = Route;

    fn routes(&self) -> Result<Vec<Self::Route>> {
        let _ = self.runtime;
        unimplemented!("Darwin route socket reading is not implemented yet")
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
