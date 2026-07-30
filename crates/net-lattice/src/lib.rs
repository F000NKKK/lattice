//! The public-facing facade of Net Lattice.
//!
//! Re-exports the types consumers need from `net-lattice-model` and
//! `net-lattice-ip`, selects a default backend based on `cfg(target_os =
//! "...")`, and enforces model convergence: `net-lattice-platform`'s generic
//! provider traits are constrained here to Net Lattice's own model types,
//! without `net-lattice-platform` ever depending on `net-lattice-model`. See
//! ARCHITECTURE.md for the full rationale.

pub use net_lattice_core::{Error, Id, PlatformErrorCode, Result};
pub use net_lattice_ip::{
    Ipv4Address, Ipv4Network, Ipv4PrefixLength, Ipv6Address, Ipv6Network, Ipv6PrefixLength,
};
pub use net_lattice_model::dns::DnsConfig;
pub use net_lattice_model::ifaddr::{InterfaceAddress, InterfaceAddressId};
pub use net_lattice_model::interface::{
    AdminState, Interface, InterfaceId, InterfaceKind, OperationalState,
};
pub use net_lattice_model::mac::MacAddress;
pub use net_lattice_model::neighbor::{NeighborEntry, NeighborId, NeighborState};
pub use net_lattice_model::route::{Route, RouteId};
pub use net_lattice_model::{IpAddress, Network};
pub use net_lattice_platform::{
    AddressProvider, Capability, DnsProvider, InterfaceProvider, NeighborProvider, RouteProvider,
};

/// Bound satisfied by any backend usable with [`Lattice`].
///
/// This is where model convergence is enforced: a backend whose
/// `RouteProvider::Route` (or `InterfaceProvider::Interface`,
/// `DnsProvider::DnsConfig`, `NeighborProvider::NeighborEntry`,
/// `AddressProvider::InterfaceAddress`) is not literally
/// `net_lattice_model`'s corresponding type fails to satisfy this trait and
/// cannot be used with [`Lattice`] — a compile error at the point the
/// backend is wired in, not a runtime surprise. See ARCHITECTURE.md's
/// `net-lattice` section.
pub trait LatticeBackend:
    RouteProvider<Route = Route>
    + InterfaceProvider<Interface = Interface>
    + DnsProvider<DnsConfig = DnsConfig>
    + NeighborProvider<NeighborEntry = NeighborEntry>
    + AddressProvider<InterfaceAddress = InterfaceAddress>
{
}

impl<B> LatticeBackend for B where
    B: RouteProvider<Route = Route>
        + InterfaceProvider<Interface = Interface>
        + DnsProvider<DnsConfig = DnsConfig>
        + NeighborProvider<NeighborEntry = NeighborEntry>
        + AddressProvider<InterfaceAddress = InterfaceAddress>
{
}

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

    pub fn interfaces(&self) -> Result<Vec<Interface>> {
        self.backend.interfaces()
    }

    pub fn dns_config(&self) -> Result<DnsConfig> {
        self.backend.dns_config()
    }

    pub fn neighbors(&self) -> Result<Vec<NeighborEntry>> {
        self.backend.neighbors()
    }

    pub fn addresses(&self) -> Result<Vec<InterfaceAddress>> {
        self.backend.addresses()
    }
}

#[cfg(target_os = "linux")]
impl Lattice<net_lattice_backend_linux::LinuxBackend> {
    /// Connects using the default backend for the current platform.
    pub fn connect() -> Result<Self> {
        Ok(Self {
            backend: net_lattice_backend_linux::LinuxBackend::new()?,
        })
    }
}

#[cfg(target_os = "windows")]
impl Lattice<net_lattice_backend_windows::WindowsBackend> {
    /// Connects using the default backend for the current platform.
    pub fn connect() -> Result<Self> {
        Ok(Self {
            backend: net_lattice_backend_windows::WindowsBackend::new()?,
        })
    }
}

#[cfg(target_os = "macos")]
impl Lattice<net_lattice_backend_darwin::DarwinBackend> {
    /// Connects using the default backend for the current platform.
    pub fn connect() -> Result<Self> {
        Ok(Self {
            backend: net_lattice_backend_darwin::DarwinBackend::new()?,
        })
    }
}
