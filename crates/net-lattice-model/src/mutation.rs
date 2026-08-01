//! Typed, inspectable descriptions of imperative network mutations.
//!
//! These types describe work; they do not execute it. A future transaction
//! executor consumes a [`MutationPlan`] only after the caller has inspected
//! each operation's preconditions and limits.

use crate::dns::NewDnsConfig;
use crate::ifaddr::{InterfaceAddress, NewInterfaceAddress};
use crate::route::Route;
use net_lattice_core::Error;

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

/// Privilege level required by the current native operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MutationPrivilege {
    /// The operation changes operating-system network configuration and
    /// requires the platform's elevated networking privilege.
    Elevated,
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
#[non_exhaustive]
pub struct MutationSemantics {
    /// The requested effect.
    pub kind: MutationKind,
    /// Required state before execution.
    pub precondition: MutationPrecondition,
    /// Repetition behavior.
    pub idempotency: MutationIdempotency,
    /// Privilege required to submit the operation.
    pub privilege: MutationPrivilege,
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
                privilege: MutationPrivilege::Elevated,
                confirmation: MutationConfirmation::NativeAcknowledgement,
                reversibility: MutationReversibility::RequiresPriorState,
                may_partially_apply: false,
            },
            Self::RemoveRoute(_) => MutationSemantics {
                kind: MutationKind::RemoveRoute,
                precondition: MutationPrecondition::Present,
                idempotency: MutationIdempotency::Strict,
                privilege: MutationPrivilege::Elevated,
                confirmation: MutationConfirmation::NativeAcknowledgement,
                reversibility: MutationReversibility::RequiresPriorState,
                may_partially_apply: false,
            },
            Self::AddAddress(_) => MutationSemantics {
                kind: MutationKind::AddAddress,
                precondition: MutationPrecondition::Absent,
                idempotency: MutationIdempotency::Strict,
                privilege: MutationPrivilege::Elevated,
                confirmation: MutationConfirmation::ReadAfterWrite,
                reversibility: MutationReversibility::RequiresPriorState,
                may_partially_apply: false,
            },
            Self::RemoveAddress(_) => MutationSemantics {
                kind: MutationKind::RemoveAddress,
                precondition: MutationPrecondition::Present,
                idempotency: MutationIdempotency::Strict,
                privilege: MutationPrivilege::Elevated,
                confirmation: MutationConfirmation::NativeAcknowledgement,
                reversibility: MutationReversibility::RequiresPriorState,
                may_partially_apply: false,
            },
            Self::SetDnsConfig(_) => MutationSemantics {
                kind: MutationKind::SetDnsConfig,
                precondition: MutationPrecondition::Any,
                idempotency: MutationIdempotency::Replace,
                privilege: MutationPrivilege::Elevated,
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

/// Static preflight facts derived from a [`MutationPlan`].
///
/// Preflight is deliberately backend-independent: it does not inspect the
/// operating system, capabilities, privileges, or current state. A plan can
/// pass this analysis and still fail when an executor submits it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MutationPreflight {
    prior_state_indices: Vec<usize>,
    partial_application_indices: Vec<usize>,
}

impl MutationPreflight {
    /// Returns plan-local indices whose compensation needs a prior snapshot.
    pub fn prior_state_indices(&self) -> &[usize] {
        &self.prior_state_indices
    }

    /// Returns plan-local indices whose operation may change state before an
    /// error is returned.
    pub fn partial_application_indices(&self) -> &[usize] {
        &self.partial_application_indices
    }

    /// Whether any operation requires a prior observed state for compensation.
    pub fn requires_prior_state(&self) -> bool {
        !self.prior_state_indices.is_empty()
    }

    /// Whether any operation carries a partial-application risk.
    pub fn may_partially_apply(&self) -> bool {
        !self.partial_application_indices.is_empty()
    }
}

/// The result recorded for one operation in an applied mutation plan.
///
/// These values describe an executor's report; constructing a plan or a
/// report never changes operating-system state. `Failed` deliberately keeps
/// whether the operation may have taken effect separate from the error so a
/// caller can decide whether a fresh read is required.
#[derive(Debug)]
#[non_exhaustive]
pub enum MutationOutcome {
    /// The backend acknowledged the operation according to its contract.
    Applied,
    /// The operation returned an error.
    Failed {
        /// Backend error returned by the operation.
        error: Error,
        /// Whether the operation may have changed state before failing.
        may_have_applied: bool,
    },
    /// The executor did not attempt this operation because an earlier
    /// operation failed or execution was cancelled.
    NotAttempted,
}

/// Status of compensation attempted after a plan failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum RollbackStatus {
    /// No operation failed, so rollback was not needed.
    NotNeeded,
    /// A rollback boundary exists, but compensation was not attempted.
    NotAttempted,
    /// Compensation completed for the operations selected by the executor.
    Completed,
    /// Compensation itself failed.
    Failed {
        /// Index of the operation whose compensation failed.
        operation_index: usize,
        /// Error returned by the compensation attempt.
        error: Error,
    },
}

/// Executor report for an ordered [`MutationPlan`].
///
/// A report is intentionally not called a transaction: operations may have
/// been partially applied, and rollback is reported separately rather than
/// implied. The outcome at index `n` corresponds to
/// `MutationPlan::operation(n)`. Callers should re-read affected state
/// whenever an outcome says that application was possible or rollback was not
/// completed.
#[derive(Debug)]
pub struct MutationPlanReport {
    outcomes: Vec<MutationOutcome>,
    rollback: RollbackStatus,
}

impl MutationPlanReport {
    /// Creates a report for outcomes in the plan's declared order.
    pub fn new(
        outcomes: impl IntoIterator<Item = MutationOutcome>,
        rollback: RollbackStatus,
    ) -> Self {
        Self {
            outcomes: outcomes.into_iter().collect(),
            rollback,
        }
    }

    /// Returns one outcome for each operation attempted or skipped.
    pub fn outcomes(&self) -> &[MutationOutcome] {
        &self.outcomes
    }

    /// Returns the outcome at a plan-local operation index, if present.
    pub fn outcome(&self, index: usize) -> Option<&MutationOutcome> {
        self.outcomes.get(index)
    }

    /// Returns the number of recorded outcomes.
    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    /// Whether no outcomes have been recorded.
    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }

    /// Returns the rollback status recorded by the executor.
    pub fn rollback(&self) -> &RollbackStatus {
        &self.rollback
    }

    /// Whether every recorded operation was applied successfully.
    pub fn is_success(&self) -> bool {
        self.outcomes
            .iter()
            .all(|outcome| matches!(outcome, MutationOutcome::Applied))
    }

    /// Number of operations recorded as applied.
    pub fn applied_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| matches!(outcome, MutationOutcome::Applied))
            .count()
    }

    /// Number of operations the executor did not attempt.
    pub fn not_attempted_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| matches!(outcome, MutationOutcome::NotAttempted))
            .count()
    }
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

    /// Returns the operation at `index`, if present.
    ///
    /// Executors use this stable plan-local index to associate an operation
    /// with the corresponding entry in [`MutationPlanReport::outcomes`].
    pub fn operation(&self, index: usize) -> Option<&Mutation> {
        self.operations.get(index)
    }

    /// Computes backend-independent execution risks for this plan.
    ///
    /// This method has no side effects and does not validate capabilities,
    /// privileges, or current networking state. Those checks belong to the
    /// executor at submission time.
    pub fn preflight(&self) -> MutationPreflight {
        let mut prior_state_indices = Vec::new();
        let mut partial_application_indices = Vec::new();

        for (index, operation) in self.operations.iter().enumerate() {
            let semantics = operation.semantics();
            if matches!(
                semantics.reversibility,
                MutationReversibility::RequiresPriorState
            ) {
                prior_state_indices.push(index);
            }
            if semantics.may_partially_apply {
                partial_application_indices.push(index);
            }
        }

        MutationPreflight {
            prior_state_indices,
            partial_application_indices,
        }
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
            operation.semantics().confirmation,
            MutationConfirmation::ReadAfterWrite
        );
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
        assert_eq!(
            operation.semantics().reversibility,
            MutationReversibility::RequiresPriorState
        );
        assert!(!operation.semantics().may_partially_apply);
    }

    #[test]
    fn plan_keeps_declared_order_without_executing_operations() {
        let first = Mutation::AddRoute(Route::new(crate::route::RouteId::new(1), network()));
        let second = Mutation::RemoveRoute(Route::new(crate::route::RouteId::new(2), network()));
        let plan = MutationPlan::from_operations([first.clone(), second.clone()]);
        assert_eq!(plan.operations(), [first, second]);
        assert!(plan.operation(0).is_some());
        assert!(plan.operation(2).is_none());
        assert_eq!(plan.len(), 2);
        assert!(!plan.is_empty());
    }

    #[test]
    fn route_operations_expose_strict_native_acknowledgement_contracts() {
        let route = Route::new(crate::route::RouteId::new(1), network());

        let added = Mutation::AddRoute(route.clone()).semantics();
        assert_eq!(added.kind, MutationKind::AddRoute);
        assert_eq!(added.precondition, MutationPrecondition::Absent);
        assert_eq!(added.idempotency, MutationIdempotency::Strict);
        assert_eq!(added.privilege, MutationPrivilege::Elevated);
        assert_eq!(
            added.confirmation,
            MutationConfirmation::NativeAcknowledgement
        );
        assert_eq!(
            added.reversibility,
            MutationReversibility::RequiresPriorState
        );
        assert!(!added.may_partially_apply);

        let removed = Mutation::RemoveRoute(route).semantics();
        assert_eq!(removed.kind, MutationKind::RemoveRoute);
        assert_eq!(removed.precondition, MutationPrecondition::Present);
        assert_eq!(removed.idempotency, MutationIdempotency::Strict);
        assert_eq!(removed.privilege, MutationPrivilege::Elevated);
        assert_eq!(
            removed.confirmation,
            MutationConfirmation::NativeAcknowledgement
        );
        assert_eq!(
            removed.reversibility,
            MutationReversibility::RequiresPriorState
        );
        assert!(!removed.may_partially_apply);
    }

    #[test]
    fn address_removal_exposes_its_observed_record_contract() {
        let address =
            InterfaceAddress::new(crate::ifaddr::InterfaceAddressId::new(1), 1, network());
        let semantics = Mutation::RemoveAddress(address).semantics();

        assert_eq!(semantics.kind, MutationKind::RemoveAddress);
        assert_eq!(semantics.precondition, MutationPrecondition::Present);
        assert_eq!(semantics.idempotency, MutationIdempotency::Strict);
        assert_eq!(semantics.privilege, MutationPrivilege::Elevated);
        assert_eq!(
            semantics.confirmation,
            MutationConfirmation::NativeAcknowledgement
        );
        assert_eq!(
            semantics.reversibility,
            MutationReversibility::RequiresPriorState
        );
        assert!(!semantics.may_partially_apply);
    }

    #[test]
    fn empty_plan_can_be_built_appended_and_consumed() {
        let mut plan = MutationPlan::new();
        assert!(plan.is_empty());
        assert_eq!(plan.len(), 0);

        let operation = Mutation::AddRoute(Route::new(crate::route::RouteId::new(1), network()));
        plan.push(operation.clone());
        assert_eq!(plan.operations(), std::slice::from_ref(&operation));
        assert_eq!(plan.into_iter().collect::<Vec<_>>(), vec![operation]);
    }

    #[test]
    fn plan_report_preserves_partial_failure_and_rollback_boundary() {
        let report = MutationPlanReport::new(
            [
                MutationOutcome::Applied,
                MutationOutcome::Failed {
                    error: Error::PermissionDenied,
                    may_have_applied: true,
                },
                MutationOutcome::NotAttempted,
            ],
            RollbackStatus::NotAttempted,
        );

        assert!(!report.is_success());
        assert_eq!(report.applied_count(), 1);
        assert_eq!(report.not_attempted_count(), 1);
        assert_eq!(report.len(), 3);
        assert!(!report.is_empty());
        assert!(report.outcome(3).is_none());
        assert!(matches!(report.rollback(), RollbackStatus::NotAttempted));
        assert!(matches!(
            &report.outcomes()[1],
            MutationOutcome::Failed {
                may_have_applied: true,
                ..
            }
        ));
    }

    #[test]
    fn preflight_identifies_snapshot_and_partial_application_risks() {
        let plan = MutationPlan::from_operations([
            Mutation::AddRoute(Route::new(crate::route::RouteId::new(1), network())),
            Mutation::SetDnsConfig(NewDnsConfig::new()),
        ]);
        let preflight = plan.preflight();

        assert_eq!(preflight.prior_state_indices(), &[0]);
        assert_eq!(preflight.partial_application_indices(), &[1]);
        assert!(preflight.requires_prior_state());
        assert!(preflight.may_partially_apply());
    }
}
