use lattice_core::Result;

/// Lists, adds, and removes routes.
///
/// Generic over an associated `Route` type rather than naming
/// `lattice_model::route::Route` directly — `lattice-platform` does not
/// depend on `lattice-model` (see ARCHITECTURE.md). The facade crate
/// (`lattice`) is what constrains `Route` to the concrete model type.
pub trait RouteProvider {
    type Route;

    fn routes(&self) -> Result<Vec<Self::Route>>;
    fn add_route(&self, route: Self::Route) -> Result<()>;
    fn remove_route(&self, route: Self::Route) -> Result<()>;
}
