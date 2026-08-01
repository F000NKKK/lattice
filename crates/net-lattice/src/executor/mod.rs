//! Internal Stage 0.15 transaction orchestration helpers.
//!
//! The mutation model remains in `net-lattice-model`. This module owns only
//! execution policy shared by the facade methods and deliberately has no
//! dependency on a concrete operating-system backend.

use net_lattice_core::Error;
use net_lattice_model::{
    Mutation, MutationOutcome, MutationPlan, MutationPlanReport, RollbackStatus,
};

/// Internal execution policy shared by the facade convenience methods.
///
/// Snapshot capture remains generic over the caller's state type and is kept
/// on the dedicated snapshot entry point until the policy object grows a
/// typed callback contract.
pub(crate) struct ExecutionOptions<'a> {
    pub(crate) cancellation: &'a mut dyn FnMut(usize, &Mutation) -> bool,
    pub(crate) compensation:
        Option<&'a mut dyn FnMut(usize, &Mutation) -> net_lattice_core::Result<()>>,
}

impl<'a> ExecutionOptions<'a> {
    pub(crate) fn new(cancellation: &'a mut dyn FnMut(usize, &Mutation) -> bool) -> Self {
        Self {
            cancellation,
            compensation: None,
        }
    }

    pub(crate) fn with_compensation(
        cancellation: &'a mut dyn FnMut(usize, &Mutation) -> bool,
        compensation: &'a mut dyn FnMut(usize, &Mutation) -> net_lattice_core::Result<()>,
    ) -> Self {
        Self {
            cancellation,
            compensation: Some(compensation),
        }
    }
}

/// Returns whether an operation requires the resolver mutation capability.
pub(crate) fn requires_dns_capability(operation: &Mutation) -> bool {
    matches!(operation, Mutation::SetDnsConfig(_))
}

/// Builds the complete report for a plan rejected before native submission.
pub(crate) fn unsupported_plan_report(plan: &MutationPlan, error: Error) -> MutationPlanReport {
    let mut outcomes = Vec::with_capacity(plan.len());
    if !plan.is_empty() {
        outcomes.push(MutationOutcome::Failed {
            error,
            may_have_applied: false,
        });
        outcomes.extend((0..plan.len() - 1).map(|_| MutationOutcome::NotAttempted));
    }
    MutationPlanReport::new(outcomes, RollbackStatus::NotNeeded)
}
