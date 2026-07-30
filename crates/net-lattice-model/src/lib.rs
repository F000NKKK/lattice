//! The domain model of operating system networking state.
//!
//! No operating-system dependency. Stage 0.4 added the `mac` and `interface`
//! modules; Stage 0.5 added `dns`; Stage 0.6 added `neighbor`; Stage 0.7 adds
//! `ifaddr` — `event` is added in a later stage per ARCHITECTURE.md's
//! Incremental Delivery Plan.

mod address;
pub mod dns;
pub mod ifaddr;
pub mod interface;
pub mod mac;
pub mod neighbor;
pub mod route;

pub use address::{IpAddress, Network};
pub use dns::DnsConfig;
pub use ifaddr::{InterfaceAddress, InterfaceAddressId};
pub use interface::{AdminState, Interface, InterfaceId, InterfaceKind, OperationalState};
pub use mac::MacAddress;
pub use neighbor::{NeighborEntry, NeighborId, NeighborState};
pub use route::Route;
