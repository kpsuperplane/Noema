use thiserror::Error;

/// A workspace/project identifier or record input is malformed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkspaceInputError {
    /// An identifier is blank, uses the wrong prefix, contains controls, or
    /// exceeds the persisted byte limit.
    #[error("invalid {kind} id: {reason}")]
    InvalidId {
        /// Semantic identifier kind.
        kind: &'static str,
        /// Safe validation explanation.
        reason: &'static str,
    },
    /// A required record field is blank.
    #[error("workspace field cannot be empty: {0}")]
    EmptyField(&'static str),
    /// An archive flag violates the Personal workspace invariant.
    #[error("personal workspace cannot be archived")]
    PersonalWorkspaceArchived,
    /// A revision or timestamp does not satisfy the record contract.
    #[error("invalid workspace record: {0}")]
    InvalidRecord(&'static str),
}
