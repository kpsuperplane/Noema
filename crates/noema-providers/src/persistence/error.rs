//! Provider persistence error vocabulary.

use thiserror::Error;

use crate::ProviderInstanceKey;

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
    /// A canonical or future-work selection still names the provider account.
    #[error("provider account is still in use: {provider_account_id}")]
    AccountInUse {
        /// Referenced provider account id.
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
    /// Durable future work still references an exact provider instance.
    #[error("provider instance is still referenced: {provider_instance_key}")]
    ProviderInstanceReferenced {
        /// Referenced immutable instance identity.
        provider_instance_key: ProviderInstanceKey,
    },
    /// A durable selection attempted to reference a claimed instance.
    #[error("provider instance is claimed for retirement: {provider_instance_key}")]
    ProviderInstanceRetiring {
        /// Claimed immutable instance identity.
        provider_instance_key: ProviderInstanceKey,
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
