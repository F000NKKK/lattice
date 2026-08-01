//! Internal Stage 0.15 transaction orchestration helpers.
//!
//! The mutation model remains in `net-lattice-model`. This module owns only
//! execution policy shared by the facade methods and deliberately has no
//! dependency on a concrete operating-system backend.

use net_lattice_core::{Error, Result};
use net_lattice_model::{
    Mutation, MutationOutcome, MutationPlan, MutationPlanReport, MutationSnapshot, RollbackStatus,
};

type Cancellation<'a> = &'a mut dyn FnMut(usize, &Mutation) -> bool;
type Snapshot<'a> = &'a mut dyn FnMut(usize, &Mutation) -> Result<MutationSnapshot>;
type Compensation<'a> =
    &'a mut dyn FnMut(usize, &Mutation, Option<&MutationSnapshot>) -> Result<()>;

/// Options controlling one ordered plan execution.
pub struct ExecutionOptions<'a> {
    pub(crate) cancellation: Option<Cancellation<'a>>,
    pub(crate) snapshot: Option<Snapshot<'a>>,
    pub(crate) compensation: Option<Compensation<'a>>,
}

impl<'a> ExecutionOptions<'a> {
    /// Creates options with no cancellation, snapshot, or compensation hooks.
    pub fn new() -> Self {
        Self {
            cancellation: None,
            snapshot: None,
            compensation: None,
        }
    }

    /// Installs an operation-boundary cancellation hook.
    pub fn cancellation(mut self, callback: &'a mut dyn FnMut(usize, &Mutation) -> bool) -> Self {
        self.cancellation = Some(callback);
        self
    }

    /// Installs a typed prior-state capture hook.
    pub fn snapshot(
        mut self,
        callback: &'a mut dyn FnMut(usize, &Mutation) -> Result<MutationSnapshot>,
    ) -> Self {
        self.snapshot = Some(callback);
        self
    }

    /// Installs explicit reverse-order compensation.
    pub fn compensation(
        mut self,
        callback: &'a mut dyn FnMut(usize, &Mutation, Option<&MutationSnapshot>) -> Result<()>,
    ) -> Self {
        self.compensation = Some(callback);
        self
    }
}

impl Default for ExecutionOptions<'_> {
    fn default() -> Self {
        Self::new()
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
