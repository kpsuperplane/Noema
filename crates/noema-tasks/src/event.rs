use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{TaskDomainError, error::invalid_operation};

/// Validated extensible event name used by mixed task and run streams.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct TaskEventKind(String);

impl TaskEventKind {
    /// Validate and retain an extension event name.
    ///
    /// # Errors
    ///
    /// Returns a task-domain error when the name is blank.
    pub fn new(value: impl Into<String>) -> Result<Self, TaskDomainError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            Err(invalid_operation("task event kind cannot be empty"))
        } else {
            Ok(Self(value.to_string()))
        }
    }

    /// Borrow the exact stable event name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskEventKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for TaskEventKind {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for TaskEventKind {
    type Error = TaskDomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for TaskEventKind {
    fn deserialize<DeserializerT>(deserializer: DeserializerT) -> Result<Self, DeserializerT::Error>
    where
        DeserializerT: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Persisted task lifecycle event used as a durable subscription cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskEventRecord {
    /// Stable event id.
    pub event_id: String,
    /// Owning task id.
    pub task_id: String,
    /// Durable monotonic cursor within the task.
    pub sequence_number: i64,
    /// Extensible event vocabulary name.
    pub event_kind: TaskEventKind,
    /// Actor or component that emitted the event.
    pub actor_id: String,
    /// Direct causation id, when present.
    pub causation_id: Option<String>,
    /// Cross-run correlation id, when present.
    pub correlation_id: Option<String>,
    /// Safe structured payload.
    pub payload: Value,
    /// Creation timestamp.
    pub created_at: String,
}

/// One append-only task lifecycle event.
#[derive(Debug, Clone, PartialEq)]
pub struct NewTaskEvent {
    /// Optional stable event id.
    pub event_id: Option<String>,
    /// Task owning the event.
    pub task_id: String,
    /// Event name.
    pub event_kind: TaskEventKind,
    /// Actor/component responsible.
    pub actor_id: String,
    /// Direct cause, when present.
    pub causation_id: Option<String>,
    /// Cross-run correlation id, when present.
    pub correlation_id: Option<String>,
    /// Safe structured payload.
    pub payload: Value,
}

/// One append-only run event.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRunEvent {
    /// Optional stable event id.
    pub event_id: Option<String>,
    /// Run owning the event.
    pub run_id: String,
    /// Event name.
    pub event_kind: TaskEventKind,
    /// Actor/component responsible.
    pub actor_id: String,
    /// Direct cause, when present.
    pub causation_id: Option<String>,
    /// Cross-run correlation id, when present.
    pub correlation_id: Option<String>,
    /// Safe structured payload.
    pub payload: Value,
}

#[cfg(test)]
mod tests {
    use super::TaskEventKind;

    #[test]
    fn event_kind_trims_extensions_and_rejects_blank_wire_values() {
        let kind = TaskEventKind::new("  extension.ready  ").expect("valid extension");
        assert_eq!(kind.as_str(), "extension.ready");
        assert_eq!(
            serde_json::to_string(&kind).expect("serialize event kind"),
            "\"extension.ready\""
        );
        assert!(serde_json::from_str::<TaskEventKind>("\"  \"").is_err());
    }
}
