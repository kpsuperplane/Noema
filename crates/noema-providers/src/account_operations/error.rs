//! Safe provider account operation failures.

/// Provider account orchestration failure safe to expose through API adapters.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProviderAccountOperationError {
    /// The requested provider kind does not support this account operation.
    #[error("unsupported provider")]
    UnsupportedProvider,
    /// The requested provider account does not exist.
    #[error("provider account not found")]
    AccountNotFound,
    /// The requested account exists but is inactive.
    #[error("provider account not found")]
    AccountInactive,
    /// The request's provider kind does not match the durable account.
    #[error("provider account mismatch")]
    ProviderMismatch,
    /// The request's authentication method does not match the durable account.
    #[error("provider account auth method mismatch")]
    AuthMethodMismatch,
    /// Default or system-owned accounts cannot be deleted.
    #[error("default provider accounts cannot be deleted")]
    ProtectedAccount,
    /// A supplied secret was empty or otherwise invalid.
    #[error("api key is required")]
    InvalidSecret,
    /// The provider could not complete the requested remote operation.
    #[error("provider unavailable")]
    ProviderUnavailable,
    /// Durable provider account state could not be read or written.
    #[error("provider account state unavailable")]
    Persistence,
    /// Durable state changed while a remote operation was in flight.
    #[error("provider account changed during the operation")]
    Conflict,
    /// The primary operation failed and its compensating action also failed.
    #[error("provider account recovery failed")]
    CompensationFailed,
}
