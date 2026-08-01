//! The domain model of operating system networking state.
//!
//! No operating-system dependency. Stage 0.4 added the `mac` and `interface`
//! modules; Stage 0.5 added `dns`; Stage 0.6 added `neighbor`; Stage 0.7
//! added `ifaddr`; Stage 0.8 added `event`; Stage 0.14 added `mutation`; and
//! Stage 0.16 adds desired interface-configuration intent.

mod address;
pub mod dns;
pub mod event;
pub mod ifaddr;
pub mod interface;
pub mod mac;
pub mod mutation;
pub mod neighbor;
pub mod route;

pub use address::{IpAddress, Network};
pub use dns::{DnsConfig, NewDnsConfig};
pub use event::{ChangeKind, Event, EventDomain, EventFilter};
pub use ifaddr::{InterfaceAddress, InterfaceAddressId, NewInterfaceAddress};
pub use interface::{
    AdminState, DesiredAdminState, Interface, InterfaceConfig, InterfaceId, InterfaceKind,
    OperationalState,
};
pub use mac::MacAddress;
pub use mutation::{
    Mutation, MutationConfirmation, MutationExecutionPhase, MutationIdempotency, MutationKind,
    MutationOperationReport, MutationOutcome, MutationPlan, MutationPlanReport,
    MutationPrecondition, MutationPreflight, MutationPrivilege, MutationReversibility,
    MutationSemantics, MutationSnapshot, MutationStopReason, RollbackStatus,
};
pub use neighbor::{NeighborEntry, NeighborId, NeighborState};
pub use route::Route;
