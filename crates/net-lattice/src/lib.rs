//! Cross-platform inspection, mutation, and monitoring of operating-system
//! networking through a strongly typed Rust API.
//!
//! Start with [`Lattice::connect`] to inspect interfaces, addresses, routes,
//! DNS configuration, and neighbor tables; perform supported mutations; or
//! subscribe to network change events.
//!
//! # Example
//!
//! ```no_run
//! use net_lattice::{Lattice, Result};
//!
//! fn main() -> Result<()> {
//!     let lattice = Lattice::connect()?;
//!     for interface in lattice.interfaces()? {
//!         println!("{interface:?}");
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # Facade design
//!
//! Re-exports the types consumers need from `net-lattice-model` and
//! `net-lattice-ip`, selects a default backend based on `cfg(target_os =
//! "...")`, and enforces model convergence: `net-lattice-platform`'s generic
//! provider traits are constrained here to Net Lattice's own model types,
//! without `net-lattice-platform` ever depending on `net-lattice-model`. See
//! ARCHITECTURE.md for the full rationale.

/// Async event adapters, enabled by the `async` feature.
#[cfg(feature = "async")]
pub use net_lattice_async::EventStream;
pub use net_lattice_core::{Error, Id, PlatformErrorCode, Result};
pub use net_lattice_ip::{
    Ipv4Address, Ipv4Network, Ipv4PrefixLength, Ipv6Address, Ipv6Network, Ipv6PrefixLength,
};
pub use net_lattice_model::dns::{DnsConfig, NewDnsConfig};
pub use net_lattice_model::event::{ChangeKind, Event, EventDomain, EventFilter};
pub use net_lattice_model::ifaddr::{InterfaceAddress, InterfaceAddressId, NewInterfaceAddress};
pub use net_lattice_model::interface::{
    AdminState, Interface, InterfaceId, InterfaceKind, OperationalState,
};
pub use net_lattice_model::mac::MacAddress;
pub use net_lattice_model::neighbor::{NeighborEntry, NeighborId, NeighborState};
pub use net_lattice_model::route::{Route, RouteId};
pub use net_lattice_model::{IpAddress, Network};
#[cfg(feature = "async")]
pub use net_lattice_platform::TokioEventProvider;
pub use net_lattice_platform::{
    AddressMutator, AddressProvider, Capability, CapabilityProvider, DnsMutator, DnsProvider,
    EventProvider, EventReceiver, InterfaceProvider, NeighborProvider, RouteProvider,
};

/// Contracts for implementing a third-party Net Lattice backend.
///
/// These traits are a supported extension API. A backend must preserve the
/// documented read, mutation, event-delivery, and cancellation semantics of
/// each trait it implements. The root re-exports remain available for
/// compatibility; new backend code may import from this module.
pub mod backend {
    pub use crate::LatticeBackend;
    pub use net_lattice_platform::{
        AddressMutator, AddressProvider, CapabilityProvider, DnsMutator, DnsProvider,
        EventProvider, EventReceiver, EventSender, InterfaceProvider, NeighborProvider,
        RouteProvider,
    };
    #[cfg(feature = "async")]
    pub use net_lattice_platform::{TokioEventProvider, TokioEventReceiver, TokioEventSender};
}

/// Bound satisfied by any backend usable with [`Lattice`].
///
/// This is where model convergence is enforced: a backend whose
/// `RouteProvider::Route` (or `InterfaceProvider::Interface`,
/// `DnsProvider::DnsConfig`, `NeighborProvider::NeighborEntry`,
/// `AddressProvider::InterfaceAddress`, `AddressMutator`'s input/output,
/// `EventProvider::Event`) is not
/// literally `net_lattice_model`'s corresponding type fails to satisfy this
/// trait and cannot be used with [`Lattice`] — a compile error at the point
/// the backend is wired in, not a runtime surprise. See ARCHITECTURE.md's
/// `net-lattice` section. `CapabilityProvider` has no associated type to
/// converge — it reports plain runtime facts about the connected system,
/// not domain objects — so it's required as-is.
pub trait LatticeBackend:
    RouteProvider<Route = Route>
    + InterfaceProvider<Interface = Interface>
    + DnsMutator<NewDnsConfig = NewDnsConfig, DnsConfig = DnsConfig>
    + NeighborProvider<NeighborEntry = NeighborEntry>
    + AddressProvider<InterfaceAddress = InterfaceAddress>
    + AddressMutator<NewInterfaceAddress = NewInterfaceAddress, InterfaceAddress = InterfaceAddress>
    + EventProvider<Event = Event, EventFilter = EventFilter>
    + CapabilityProvider
{
}

impl<B> LatticeBackend for B where
    B: RouteProvider<Route = Route>
        + InterfaceProvider<Interface = Interface>
        + DnsMutator<NewDnsConfig = NewDnsConfig, DnsConfig = DnsConfig>
        + NeighborProvider<NeighborEntry = NeighborEntry>
        + AddressProvider<InterfaceAddress = InterfaceAddress>
        + AddressMutator<
            NewInterfaceAddress = NewInterfaceAddress,
            InterfaceAddress = InterfaceAddress,
        > + EventProvider<Event = Event, EventFilter = EventFilter>
        + CapabilityProvider
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

    /// Replaces resolver configuration and returns the resulting observed
    /// resolver view.
    pub fn set_dns_config(&self, config: NewDnsConfig) -> Result<DnsConfig> {
        self.backend.set_dns_config(config)
    }

    pub fn neighbors(&self) -> Result<Vec<NeighborEntry>> {
        self.backend.neighbors()
    }

    pub fn addresses(&self) -> Result<Vec<InterfaceAddress>> {
        self.backend.addresses()
    }

    /// Assigns an address to an interface and returns the canonical record
    /// observed from the operating system after creation.
    pub fn add_address(&self, address: NewInterfaceAddress) -> Result<InterfaceAddress> {
        self.backend.add_address(address)
    }

    /// Removes the observed interface address.
    pub fn remove_address(&self, address: InterfaceAddress) -> Result<()> {
        self.backend.remove_address(address)
    }

    /// The full set of runtime-dependent [`Capability`] flags the connected
    /// backend currently has available.
    pub fn capabilities(&self) -> Capability {
        self.backend.capabilities()
    }

    /// Whether the connected backend currently has `capability` available.
    /// Shorthand for `self.capabilities().contains(capability)`.
    pub fn supports(&self, capability: Capability) -> bool {
        self.backend.capabilities().contains(capability)
    }

    /// Subscribes to change notifications. See [`EventReceiver`] for how to
    /// consume the returned events. Prefer `recv`/`try_recv`/`recv_timeout`
    /// when errors must be handled explicitly; `Iterator` terminates on any
    /// receiver error.
    ///
    /// ```no_run
    /// use net_lattice::{Lattice, Result};
    ///
    /// fn main() -> Result<()> {
    ///     let lattice = Lattice::connect()?;
    ///     let events = lattice.watch()?;
    ///     loop {
    ///         println!("{:?}", events.recv()?);
    ///     }
    /// }
    /// ```
    pub fn watch(&self) -> Result<EventReceiver<Event>> {
        self.ensure_monitoring()?;
        self.backend.watch()
    }

    /// Subscribes to async change notifications selected by `filter`.
    ///
    /// This is the Stage 0.11 async watcher API. It has the same filter
    /// semantics as [`Self::watch_filtered`].
    ///
    /// ```no_run
    /// use futures::StreamExt;
    /// use net_lattice::{EventFilter, Lattice, Result};
    ///
    /// async fn monitor() -> Result<()> {
    ///     let lattice = Lattice::connect()?;
    ///     let mut events = lattice.watch_async(EventFilter::ALL)?;
    ///     while let Some(event) = events.next().await {
    ///         println!("{:?}", event?);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "async")]
    pub fn watch_async(&self, filter: EventFilter) -> Result<EventStream<Event>>
    where
        B: TokioEventProvider<Event = Event, EventFilter = EventFilter>,
    {
        self.ensure_monitoring()?;
        Ok(net_lattice_async::from_tokio_receiver(
            self.backend.watch_tokio(filter)?,
        ))
    }

    /// Subscribes to change notifications selected by `filter`.
    pub fn watch_filtered(&self, filter: EventFilter) -> Result<EventReceiver<Event>> {
        self.ensure_monitoring()?;
        self.backend.watch_filtered(filter)
    }

    fn ensure_monitoring(&self) -> Result<()> {
        if self.supports(Capability::MONITORING) {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
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
