use thiserror::Error;

/// Errors produced by the embedded canonical store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Store path could not be prepared.
    #[error("failed to prepare store path: {0}")]
    PreparePath(std::io::Error),
    /// Embedded SurrealDB operation failed.
    #[error("embedded store operation failed: {0}")]
    Surreal(Box<surrealdb::Error>),
    /// Schema bootstrap returned an invalid result.
    #[error("store schema bootstrap failed: {0}")]
    Schema(String),
    /// A stored value did not match a closed Noema vocabulary.
    #[error("invalid {kind} value in embedded store: {value}")]
    InvalidEnum {
        /// Vocabulary kind.
        kind: &'static str,
        /// Stored value.
        value: String,
    },
    /// A provider account expected to exist was not found.
    #[error("provider account not found: {provider_account_id}")]
    ProviderAccountNotFound {
        /// Missing provider account id.
        provider_account_id: String,
    },
    /// A conversation expected to exist was not found.
    #[error("conversation not found: {conversation_id}")]
    ConversationNotFound {
        /// Missing conversation id.
        conversation_id: String,
    },
    /// A turn expected to exist was not found.
    #[error("conversation turn not found: {turn_id}")]
    ConversationTurnNotFound {
        /// Missing turn id.
        turn_id: String,
    },
}

impl From<surrealdb::Error> for StoreError {
    fn from(source: surrealdb::Error) -> Self {
        Self::Surreal(Box::new(source))
    }
}
