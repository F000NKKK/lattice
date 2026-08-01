//! Internal Stage 0.15 transaction orchestration helpers.
//!
//! The mutation model remains in `net-lattice-model`. This module owns only
//! execution policy shared by the facade methods and deliberately has no
//! dependency on a concrete operating-system backend.

use net_lattice_core::Error;
use net_lattice_model::{
    Mutation, MutationOutcome, MutationPlan, MutationPlanReport, RollbackStatus,
};

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
