//! The contract between the model and platform backends.
//!
//! `net-lattice-platform` depends only on `net-lattice-core` — never on
//! `net-lattice-model`. Its provider traits describe the *shape* of a
//! contract, not the *content* of the model, via associated types. See
//! ARCHITECTURE.md for the full rationale, including how model
//! convergence is enforced one layer up, in `lattice`.
//!
//! Stage 0.4 adds `InterfaceProvider` — `NeighborProvider`, `DnsProvider`,
//! and `EventProvider` are added in later stages per ARCHITECTURE.md's
//! Incremental Delivery Plan.

mod capability;
mod interface_provider;
mod route_provider;

pub use capability::Capability;
pub use interface_provider::InterfaceProvider;
pub use route_provider::RouteProvider;