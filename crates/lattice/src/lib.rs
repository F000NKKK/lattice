//! The public-facing facade of Lattice.
//!
//! Re-exports the types consumers need from `lattice-model` and
//! `lattice-ip`, selects a default backend based on `cfg(target_os =
//! "...")`, and enforces model convergence: `lattice-platform`'s generic
//! provider traits are constrained here to Lattice's own model types,
//! without `lattice-platform` ever depending on `lattice-model`. See
//! ARCHITECTURE.md for the full rationale.

pub use lattice_core::{Error, Id, PlatformErrorCode, Result};
pub use lattice_ip::{
    Ipv4Address, Ipv4Network, Ipv4PrefixLength, Ipv6Address, Ipv6Network, Ipv6PrefixLength,
};
pub use lattice_model::route::{Route, RouteId};
pub use lattice_model::{IpAddress, Network};
pub use lattice_platform::{Capability, RouteProvider};

/// Bound satisfied by any backend usable with [`Lattice`].
///
/// This is where model convergence is enforced: a backend whose
/// `RouteProvider::Route` is not literally `lattice_model::route::Route`
/// fails to satisfy this trait and cannot be used with [`Lattice`] — a
/// compile error at the point the backend is wired in, not a runtime
/// surprise. See ARCHITECTURE.md's `lattice` section.
pub trait LatticeBackend: RouteProvider<Route = Route> {}

impl<B> LatticeBackend for B where B: RouteProvider<Route = Route> {}

/// The top-level entry point: a connected backend for the current system.
pub struct Lattice<B: LatticeBackend> {
    backend: B,
}

impl<B: LatticeBackend> Lattice<B> {
    pub fn routes(&self) -> Result<Vec<Route>> {
        self.backend.routes()
    }

    pub fn add_route(&self, route: Route) -> Result<()> {
        self.backend.add_route(route)
    }

    pub fn remove_route(&self, route: Route) -> Result<()> {
        self.backend.remove_route(route)
    }
}

#[cfg(target_os = "linux")]
impl Lattice<lattice_backend_linux::LinuxBackend> {
    /// Connects using the default backend for the current platform.
    pub fn connect() -> Result<Self> {
        Ok(Self {
            backend: lattice_backend_linux::LinuxBackend::new()?,
        })
    }
}
