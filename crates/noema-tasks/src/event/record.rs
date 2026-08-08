use noema_workspaces::{ProjectId, WorkspaceId};
use serde::Serialize;
use serde_json::Value;

use super::{WorkEventKind, WorkEventPayload};
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

/// Immutable event appended to the global Work ledger. Use
/// [`WorkEventRecord::new`] and a typed [`WorkEventPayload`] to construct it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkEventRecord {
    event_id: WorkEventId,
    event_sequence: u64,
    kind: WorkEventKind,
    workspace_id: WorkspaceId,
    project_id: Option<ProjectId>,
    task_id: Option<TaskId>,
    run_id: Option<String>,
    actor_id: String,
    causation_id: Option<String>,
    correlation_id: String,
    safe_payload: Value,
    created_at: String,
}

#[allow(
    missing_docs,
    reason = "read-only accessors mirror the stable event vocabulary"
)]
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
        };
        record.validate()?;
        Ok(record)
    }

    #[must_use]
    pub const fn event_id(&self) -> &WorkEventId {
        &self.event_id
    }

    #[must_use]
    pub const fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    #[must_use]
    pub const fn kind(&self) -> WorkEventKind {
        self.kind
    }

    #[must_use]
    pub const fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }

    #[must_use]
    pub const fn project_id(&self) -> Option<&ProjectId> {
        self.project_id.as_ref()
    }

    #[must_use]
    pub const fn task_id(&self) -> Option<&TaskId> {
        self.task_id.as_ref()
    }

    #[must_use]
    pub fn run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
    }

    #[must_use]
    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }

    #[must_use]
    pub fn causation_id(&self) -> Option<&str> {
        self.causation_id.as_deref()
    }

    #[must_use]
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    #[must_use]
    pub const fn safe_payload(&self) -> &Value {
        &self.safe_payload
    }

    #[must_use]
    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    fn validate(&self) -> Result<(), WorkDomainError> {
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
