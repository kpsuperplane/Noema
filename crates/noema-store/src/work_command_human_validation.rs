//! Transaction-local validation for human gates and approved review lineage.

use crate::StoreError;
use noema_tasks::{GateResolutionKind, WorkDomainError};

pub(super) fn validate_gate_resolution(
    gate: &noema_tasks::TaskGateRecord,
    resolution: GateResolutionKind,
) -> Result<(), StoreError> {
    if gate
        .kind
        .allows_resolution(gate.recovery_reason, gate.retry_run_kind, resolution)
    {
        Ok(())
    } else {
        Err(StoreError::Work(WorkDomainError::InvalidTransition))
    }
}
