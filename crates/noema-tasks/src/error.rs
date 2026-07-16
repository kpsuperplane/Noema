use thiserror::Error;

use noema_providers::ProviderSelectionError;

/// Domain input or persisted task vocabulary was malformed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TaskDomainError {
    /// A closed enum value was not recognized.
    #[error("invalid {kind}: {value}")]
    InvalidEnum {
        /// Closed vocabulary being parsed.
        kind: &'static str,
        /// Stored or supplied value.
        value: String,
    },
    /// A required field is blank.
    #[error("task field cannot be empty: {0}")]
    EmptyField(&'static str),
    /// No validation criteria were provided.
    #[error("task must include at least one validation criterion")]
    NoCriteria,
    /// A criterion was not one-based.
    #[error("criterion ordinal must be positive: {0}")]
    InvalidCriterionOrdinal(i64),
    /// Two criteria share an ordinal.
    #[error("duplicate criterion ordinal: {0}")]
    DuplicateCriterionOrdinal(i64),
    /// Two criteria have the same description.
    #[error("duplicate criterion description: {0}")]
    DuplicateCriterionDescription(String),
    /// A task's review-round bound was not positive.
    #[error("invalid maximum review rounds: {0}")]
    InvalidReviewRoundLimit(i64),
    /// A provider-independent task execution policy was malformed.
    #[error("invalid task execution policy: {message}")]
    InvalidExecutionPolicy {
        /// Actionable validation failure.
        message: String,
    },
    /// An operation-specific task input or state combination is invalid.
    #[error("invalid task operation: {message}")]
    InvalidOperation {
        /// Stable safe explanation.
        message: String,
    },
    /// A model snapshot was malformed.
    #[error("invalid task model snapshot: {0}")]
    Model(#[from] ProviderSelectionError),
}

pub(crate) fn invalid_operation(message: impl Into<String>) -> TaskDomainError {
    TaskDomainError::InvalidOperation {
        message: message.into(),
    }
}
