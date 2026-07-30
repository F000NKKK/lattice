//! The domain model of operating system networking state.
//!
//! No operating-system dependency. Stage 0.4 added the `mac` and `interface`
//! modules; Stage 0.5 adds `dns` — `neighbor` and `event` are added in later
//! stages per ARCHITECTURE.md's Incremental Delivery Plan.

mod address;
pub mod dns;
pub mod interface;
pub mod mac;
pub mod route;

pub use address::{IpAddress, Network};
pub use dns::DnsConfig;
pub use interface::{AdminState, Interface, InterfaceId, InterfaceKind, OperationalState};
pub use mac::MacAddress;
pub use route::Route;
