use net_lattice_core::Result;

/// Lists routes.
///
/// Generic over an associated `Route` type rather than naming
/// `net_lattice_model::route::Route` directly — `net-lattice-platform` does not
/// depend on `net-lattice-model` (see ARCHITECTURE.md). The facade crate
/// (`lattice`) is what constrains `Route` to the concrete model type.
///
/// Mutation is deliberately separate, in [`crate::RouteMutator`]: listing is
/// normally unprivileged, while adding or removing a route requires
/// `CAP_NET_ADMIN`, an elevated Windows token, or root on BSD/macOS — the
/// same read/write split used by [`crate::AddressProvider`]/
/// [`crate::AddressMutator`], [`crate::InterfaceProvider`]/
/// [`crate::InterfaceMutator`], and [`crate::NeighborProvider`]/
/// [`crate::NeighborMutator`].
pub trait RouteProvider {
    type Route;

    fn routes(&self) -> Result<Vec<Self::Route>>;
}

/// Adds and removes routes.
///
/// `Route` carries no OS-synthesized identity or derived observation field
/// distinct from caller intent (unlike `NeighborEntry`/`StaticNeighbor`), so
/// the same associated type is reused as both mutation input and the
/// [`crate::RouteProvider`] output.
pub trait RouteMutator {
    type Route;

    fn add_route(&self, route: Self::Route) -> Result<()>;
    fn remove_route(&self, route: Self::Route) -> Result<()>;
}
