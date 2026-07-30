//! The contract between the model and platform backends.
//!
//! `net-lattice-platform` depends only on `net-lattice-core` — never on
//! `net-lattice-model`. Its provider traits describe the *shape* of a
//! contract, not the *content* of the model, via associated types. See
//! ARCHITECTURE.md for the full rationale, including how model
//! convergence is enforced one layer up, in `lattice`.
//!
//! Stage 0.4 added `InterfaceProvider`; Stage 0.5 added `DnsProvider`;
//! Stage 0.6 added `NeighborProvider`; Stage 0.7 added `AddressProvider`;
//! Stage 0.8 adds `EventProvider`.

mod address_provider;
mod capability;
mod dns_provider;
mod event_provider;
mod interface_provider;
mod neighbor_provider;
mod route_provider;

pub use address_provider::AddressProvider;
pub use capability::{Capability, CapabilityProvider};
pub use dns_provider::DnsProvider;
pub use event_provider::{EventProvider, EventReceiver};
pub use interface_provider::InterfaceProvider;
pub use neighbor_provider::NeighborProvider;
pub use route_provider::RouteProvider;
