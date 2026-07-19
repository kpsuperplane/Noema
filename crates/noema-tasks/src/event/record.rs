use noema_workspaces::{ProjectId, WorkspaceId};
use serde::Serialize;
use serde_json::Value;

use super::{WorkEventKind, WorkEventPayload, event_validation};
use crate::{TaskId, WorkDomainError, WorkEventId, error::invalid_input};

/// Scope and causal lineage shared by every event in the Work ledger.
/// The record constructor validates these fields together with event identity,
/// timestamp, and payload data before exposing a durable event record.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkEventContext {
    pub workspace_id: WorkspaceId,
    pub project_id: Option<ProjectId>,
    pub task_id: Option<TaskId>,
    pub run_id: Option<String>,
    pub actor_id: String,
    pub causation_id: Option<String>,
    pub correlation_id: String,
}

/// Immutable event appended to the global Work ledger. `non_exhaustive`
/// prevents downstream crates from constructing an arbitrary kind/payload pair
/// with a struct literal; use [`WorkEventRecord::new`] and a typed
/// [`WorkEventPayload`] instead.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkEventRecord {
    pub event_id: WorkEventId,
    pub event_sequence: u64,
    pub kind: WorkEventKind,
    pub workspace_id: WorkspaceId,
    pub project_id: Option<ProjectId>,
    pub task_id: Option<TaskId>,
    pub run_id: Option<String>,
    pub actor_id: String,
    pub causation_id: Option<String>,
    pub correlation_id: String,
    pub safe_payload: Value,
    pub created_at: String,
    /// Constructor provenance retained outside the public wire shape.
    #[serde(skip)]
    constructed_kind: WorkEventKind,
}

impl WorkEventRecord {
    /// Construct an event from its domain context and a typed, kind-matched payload.
    /// # Errors
    /// Returns [`WorkDomainError`] when sequence, actor, correlation,
    /// causation, run identity, timestamp, or the typed payload violates the
    /// durable event record contract.
    pub fn new(
        event_id: WorkEventId,
        event_sequence: u64,
        context: WorkEventContext,
        payload: WorkEventPayload,
        created_at: String,
    ) -> Result<Self, WorkDomainError> {
        let record = Self {
            event_id,
            event_sequence,
            kind: payload.kind,
            workspace_id: context.workspace_id,
            project_id: context.project_id,
            task_id: context.task_id,
            run_id: context.run_id,
            actor_id: context.actor_id,
            causation_id: context.causation_id,
            correlation_id: context.correlation_id,
            safe_payload: payload.value,
            created_at,
            constructed_kind: payload.kind,
        };
        record.validate()?;
        Ok(record)
    }

    /// Validate event identity and causal metadata.
    /// # Errors
    /// Returns [`WorkDomainError`] when the public and constructed kinds
    /// disagree, sequence is zero, actor or causal identifiers are malformed,
    /// timestamp is blank, or the safe payload violates its event schema.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.kind != self.constructed_kind {
            return Err(invalid_input(
                "work_event.kind",
                "public kind does not match the typed constructor kind",
            ));
        }
        if self.event_sequence == 0 {
            return Err(invalid_input(
                "work_event.event_sequence",
                "sequence must be positive",
            ));
        }
        if self.actor_id.trim().is_empty() || self.correlation_id.trim().is_empty() {
            return Err(invalid_input(
                "work_event",
                "actor and correlation cannot be blank",
            ));
        }
        validate_record_identifier(&self.actor_id, "work_event.actor_id", &["actor:"])?;
        validate_record_identifier(
            &self.correlation_id,
            "work_event.correlation_id",
            &["correlation:"],
        )?;
        if let Some(causation_id) = &self.causation_id {
            validate_record_identifier(
                causation_id,
                "work_event.causation_id",
                &["event:", "command:", "run:"],
            )?;
        }
        if let Some(run_id) = &self.run_id {
            validate_record_identifier(run_id, "work_event.run_id", &["run:"])?;
        }
        if self.created_at.trim().is_empty() {
            return Err(invalid_input(
                "work_event.created_at",
                "timestamp cannot be blank",
            ));
        }
        event_validation::validate_payload(self.kind, &self.safe_payload)?;
        Ok(())
    }
}

fn validate_record_identifier(
    value: &str,
    field: &'static str,
    prefixes: &[&str],
) -> Result<(), WorkDomainError> {
    if value.len() > 255
        || value.trim().is_empty()
        || value.chars().any(char::is_control)
        || !prefixes.iter().any(|prefix| value.starts_with(prefix))
    {
        return Err(invalid_input(field, "identifier has an invalid shape"));
    }
    Ok(())
}
