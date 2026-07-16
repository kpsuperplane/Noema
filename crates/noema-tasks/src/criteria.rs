use serde::{Deserialize, Serialize};

/// Immutable validation criterion supplied when a task is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskValidationCriterion {
    /// Stable criterion id.
    pub criterion_id: String,
    /// One-based display/evaluation order.
    pub ordinal: i64,
    /// Precise condition that must be true for approval.
    pub description: String,
    /// Optional evidence or validation method guidance.
    pub expected_evidence: Option<String>,
}

/// Input criterion; the store allocates an id when one is not provided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTaskValidationCriterion {
    /// Optional caller-supplied stable criterion id.
    pub criterion_id: Option<String>,
    /// One-based display/evaluation order.
    pub ordinal: i64,
    /// Precise condition that must be true for approval.
    pub description: String,
    /// Optional evidence or validation method guidance.
    pub expected_evidence: Option<String>,
}
