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
/// A route *does* carry a backend-synthesized identity distinct from
/// caller intent: on Windows and Darwin the observed `RouteProvider::Route`
/// carries an `id` synthesized as a hash of its own observed fields, and on
/// Linux it is a kernel-issued netlink handle — on every platform, no
/// native API accepts that id back as mutation input. This mirrors
/// [`crate::NeighborProvider`]/[`crate::NeighborMutator`], which already
/// keep the observed `NeighborEntry` and the intent `StaticNeighbor` as two
/// distinct types precisely because `NeighborEntry::id` is never accepted
/// back either. `RouteMutator` therefore declares its own associated
/// `RouteConfig` type instead of reusing [`crate::RouteProvider`]'s `Route`:
/// the facade binds it to `net_lattice_model::route::RouteConfig`, a
/// distinct intent type with no id field.
pub trait RouteMutator {
    type RouteConfig;

    fn add_route(&self, route: Self::RouteConfig) -> Result<()>;
    fn remove_route(&self, route: Self::RouteConfig) -> Result<()>;
}
