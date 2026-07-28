//! The domain model of operating system networking state.
//!
//! No operating-system dependency. Stage 0.1 includes only the `route`
//! module — `interface`, `neighbor`, `dns`, and `event` are added in later
//! stages per ARCHITECTURE.md's Incremental Delivery Plan.

mod address;
pub mod route;

pub use address::{IpAddress, Network};
pub use route::Route;
