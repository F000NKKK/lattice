//! Typed, inspectable descriptions of imperative network mutations.
//!
//! These types describe work; they do not execute it. A future transaction
//! executor consumes a [`MutationPlan`] only after the caller has inspected
//! each operation's preconditions and limits.

use crate::dns::NewDnsConfig;
use crate::ifaddr::{InterfaceAddress, NewInterfaceAddress};
use crate::route::Route;

/// One existing imperative network mutation expressed as data.
///
/// Operations deliberately use the same input types accepted by today's
/// provider methods. Stage 0.14 makes their current semantics inspectable;
/// later stages may add more specific intent types where an observed type is
/// still being used as a mutation input.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Mutation {
    /// Adds a route using the route's currently supported defining fields.
    AddRoute(Route),
    /// Removes a route according to the backend's current matching rules.
    RemoveRoute(Route),
    /// Assigns an interface address.
    AddAddress(NewInterfaceAddress),
    /// Removes an observed interface address.
    RemoveAddress(InterfaceAddress),
    /// Replaces the portable resolver configuration.
    SetDnsConfig(NewDnsConfig),
}

/// The broad effect an operation requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MutationKind {
    AddRoute,
    RemoveRoute,
    AddAddress,
    RemoveAddress,
    SetDnsConfig,
}

/// State that must hold for an operation to be meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MutationPrecondition {
    /// The target must not already exist.
    Absent,
    /// The target must exist and match the backend's removal rule.
    Present,
    /// The operation replaces configuration regardless of its previous value.
    Any,
}

/// Whether repeating an operation with the same input is expected to succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MutationIdempotency {
    /// Repetition is not a successful no-op: duplicate or absent-object
    /// errors remain observable.
    Strict,
    /// Repetition requests the same replacement state.
    Replace,
}

/// How much completion evidence the current imperative API returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MutationConfirmation {
    /// The native platform operation acknowledged the request.
    NativeAcknowledgement,
    /// Net Lattice re-read the corresponding observed state after mutation.
    ReadAfterWrite,
}

/// Whether an operation can be safely compensated without prior state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MutationReversibility {
    /// A compensating operation needs a captured prior observed state and is
    /// still subject to concurrent external changes.
    RequiresPriorState,
    /// The current primitive may affect multiple native settings or lose
    /// unmodelled state, so no rollback promise is made.
    NotGuaranteed,
}

/// Static metadata describing one [`Mutation`]'s current contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MutationSemantics {
    /// The requested effect.
    pub kind: MutationKind,
    /// Required state before execution.
    pub precondition: MutationPrecondition,
    /// Repetition behavior.
    pub idempotency: MutationIdempotency,
    /// How successful completion is confirmed.
    pub confirmation: MutationConfirmation,
    /// Whether rollback can be promised by this primitive.
    pub reversibility: MutationReversibility,
    /// Whether a failed operation may already have changed some state.
    pub may_partially_apply: bool,
}

impl Mutation {
    /// Returns the static contract of this operation in the current API.
    pub const fn semantics(&self) -> MutationSemantics {
        match self {
            Self::AddRoute(_) => MutationSemantics {
                kind: MutationKind::AddRoute,
                precondition: MutationPrecondition::Absent,
                idempotency: MutationIdempotency::Strict,
                confirmation: MutationConfirmation::NativeAcknowledgement,
                reversibility: MutationReversibility::RequiresPriorState,
                may_partially_apply: false,
            },
            Self::RemoveRoute(_) => MutationSemantics {
                kind: MutationKind::RemoveRoute,
                precondition: MutationPrecondition::Present,
                idempotency: MutationIdempotency::Strict,
                confirmation: MutationConfirmation::NativeAcknowledgement,
                reversibility: MutationReversibility::RequiresPriorState,
                may_partially_apply: false,
            },
            Self::AddAddress(_) => MutationSemantics {
                kind: MutationKind::AddAddress,
                precondition: MutationPrecondition::Absent,
                idempotency: MutationIdempotency::Strict,
                confirmation: MutationConfirmation::ReadAfterWrite,
                reversibility: MutationReversibility::RequiresPriorState,
                may_partially_apply: false,
            },
            Self::RemoveAddress(_) => MutationSemantics {
                kind: MutationKind::RemoveAddress,
                precondition: MutationPrecondition::Present,
                idempotency: MutationIdempotency::Strict,
                confirmation: MutationConfirmation::NativeAcknowledgement,
                reversibility: MutationReversibility::RequiresPriorState,
                may_partially_apply: false,
            },
            Self::SetDnsConfig(_) => MutationSemantics {
                kind: MutationKind::SetDnsConfig,
                precondition: MutationPrecondition::Any,
                idempotency: MutationIdempotency::Replace,
                confirmation: MutationConfirmation::ReadAfterWrite,
                reversibility: MutationReversibility::NotGuaranteed,
                may_partially_apply: true,
            },
        }
    }
}

/// An ordered, inspectable list of mutations.
///
/// Creating a plan has no side effects. Stage 0.15 will define execution,
/// outcomes, cancellation, and best-effort compensation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct MutationPlan {
    operations: Vec<Mutation>,
}

impl MutationPlan {
    /// Creates an empty plan.
    pub const fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// Creates a plan from operations in execution order.
    pub fn from_operations(operations: impl IntoIterator<Item = Mutation>) -> Self {
        Self {
            operations: operations.into_iter().collect(),
        }
    }

    /// Appends an operation after all existing operations.
    pub fn push(&mut self, operation: Mutation) {
        self.operations.push(operation);
    }

    /// Returns the operations in their declared order.
    pub fn operations(&self) -> &[Mutation] {
        &self.operations
    }

    /// Whether the plan has no operations.
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Number of operations in the plan.
    pub fn len(&self) -> usize {
        self.operations.len()
    }
}

impl IntoIterator for MutationPlan {
    type Item = Mutation;
    type IntoIter = std::vec::IntoIter<Mutation>;

    fn into_iter(self) -> Self::IntoIter {
        self.operations.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IpAddress, Network};
    use net_lattice_ip::{Ipv4Address, Ipv4Network, Ipv4PrefixLength};

    fn network() -> Network {
        Network::from(Ipv4Network::new(
            Ipv4Address::new(192, 0, 2, 0),
            Ipv4PrefixLength::new(24).expect("valid prefix"),
        ))
    }

    #[test]
    fn dns_replacement_exposes_partial_application_risk() {
        let operation = Mutation::SetDnsConfig(NewDnsConfig::with(
            vec![IpAddress::from(Ipv4Address::new(1, 1, 1, 1))],
            Vec::new(),
        ));
        assert_eq!(
            operation.semantics().precondition,
            MutationPrecondition::Any
        );
        assert_eq!(
            operation.semantics().idempotency,
            MutationIdempotency::Replace
        );
        assert!(operation.semantics().may_partially_apply);
        assert_eq!(
            operation.semantics().reversibility,
            MutationReversibility::NotGuaranteed
        );
    }

    #[test]
    fn address_addition_has_an_observed_readback_contract() {
        let operation = Mutation::AddAddress(NewInterfaceAddress::new(
            crate::interface::InterfaceId::new(2),
            network(),
        ));
        assert_eq!(
            operation.semantics().confirmation,
            MutationConfirmation::ReadAfterWrite
        );
        assert_eq!(
            operation.semantics().precondition,
            MutationPrecondition::Absent
        );
    }

    #[test]
    fn plan_keeps_declared_order_without_executing_operations() {
        let first = Mutation::AddRoute(Route::new(crate::route::RouteId::new(1), network()));
        let second = Mutation::RemoveRoute(Route::new(crate::route::RouteId::new(2), network()));
        let plan = MutationPlan::from_operations([first.clone(), second.clone()]);
        assert_eq!(plan.operations(), [first, second]);
        assert_eq!(plan.len(), 2);
    }
}
