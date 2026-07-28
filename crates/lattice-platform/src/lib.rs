//! The contract between the model and platform backends.
//!
//! `lattice-platform` depends only on `lattice-core` — never on
//! `lattice-model`. Its provider traits describe the *shape* of a
//! contract, not the *content* of the model, via associated types. See
//! ARCHITECTURE.md for the full rationale, including how model
//! convergence is enforced one layer up, in `lattice`.
//!
//! Stage 0.1 includes only `RouteProvider` — `InterfaceProvider`,
//! `NeighborProvider`, `DnsProvider`, and `EventProvider` are added in
//! later stages per ARCHITECTURE.md's Incremental Delivery Plan.

mod capability;
mod route_provider;

pub use capability::Capability;
pub use route_provider::RouteProvider;
