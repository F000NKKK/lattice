use net_lattice_core::Result;

/// Lists network interfaces.
///
/// Generic over an associated `Interface` type rather than naming
/// `net_lattice_model::interface::Interface` directly — `net-lattice-platform`
/// does not depend on `net-lattice-model` (see ARCHITECTURE.md). The facade
/// crate (`net-lattice`) is what constrains `Interface` to the concrete model
/// type.
///
/// Unlike [`RouteProvider`](crate::RouteProvider), this trait is read-only for
/// now: listing interfaces. Configuring interfaces (setting MTU, bringing them
/// up/down) is a write operation that will be added once the API design for
/// `InterfaceConfig` vs `Interface` state/config split is settled — see
/// ARCHITECTURE.md's State Model section.
pub trait InterfaceProvider {
    type Interface;

    fn interfaces(&self) -> Result<Vec<Self::Interface>>;
}
