use thiserror::Error;

/// Validation and parsing failures in the conversation domain.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConversationError {
    /// A stored value did not match a closed conversation vocabulary.
    #[error("invalid {kind} value: {value}")]
    InvalidEnum {
        /// Vocabulary kind.
        kind: &'static str,
        /// Rejected value.
        value: String,
    },
    /// An actor or owner reference id was empty after trimming whitespace.
    #[error("{reference_kind} reference id cannot be empty")]
    EmptyReferenceId {
        /// Reference vocabulary kind.
        reference_kind: &'static str,
    },
}
