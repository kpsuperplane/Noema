//! Provider persistence error vocabulary.

use thiserror::Error;

/// Transport- and repository-neutral provider persistence failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderPersistenceError {
    /// A provider account expected by the operation does not exist.
    #[error("provider account not found: {provider_account_id}")]
    AccountNotFound {
        /// Missing provider account id.
        provider_account_id: String,
    },
    /// A local-model installation expected by the operation does not exist.
    #[error("local-model installation not found: {installation_id}")]
    InstallationNotFound {
        /// Missing installation id.
        installation_id: String,
    },
    /// A protected built-in provider account cannot be deleted.
    #[error("protected provider account cannot be deleted: {provider_account_id}")]
    ProtectedAccount {
        /// Protected provider account id.
        provider_account_id: String,
    },
    /// The caller supplied an invalid provider persistence request.
    #[error("invalid provider persistence request: {kind}")]
    InvalidRequest {
        /// Stable invalid-request category.
        kind: &'static str,
    },
    /// A local-model lifecycle transition is not permitted.
    #[error("invalid local-model transition from {from} to {to}")]
    InvalidInstallationTransition {
        /// Current durable status.
        from: String,
        /// Requested durable status.
        to: String,
    },
    /// An active local-model installation cannot be removed.
    #[error("active local-model installation cannot be removed: {installation_id}")]
    ActiveInstallationConflict {
        /// Active installation id.
        installation_id: String,
    },
    /// Concurrent persistence state prevented a guarded operation.
    #[error("provider persistence conflict during {operation}")]
    Conflict {
        /// Stable operation identifier.
        operation: &'static str,
    },
    /// Stored provider data violated a closed Noema invariant.
    #[error("provider persistence invariant violated during {operation}")]
    Invariant {
        /// Stable operation identifier.
        operation: &'static str,
    },
    /// The repository operation failed without exposing implementation details.
    #[error("provider persistence operation failed: {operation}")]
    Persistence {
        /// Stable operation identifier.
        operation: &'static str,
    },
}
