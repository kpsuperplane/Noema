use noema_artifacts::ArtifactDomainError;
use noema_conversations::ConversationError;
use thiserror::Error;

/// Errors produced by the embedded canonical store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Store path could not be prepared.
    #[error("failed to prepare store path: {0}")]
    PreparePath(std::io::Error),
    /// SQLite operation failed.
    #[error("sqlite store operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// JSON encoding or decoding failed.
    #[error("store JSON encoding failed: {0}")]
    Json(#[from] serde_json::Error),
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
    /// Store write/read flow observed an unexpected internal inconsistency.
    #[error("store invariant violated: {message}")]
    InvariantViolation {
        /// Human-readable invariant failure message.
        message: String,
    },
    /// A provider account expected to exist was not found.
    #[error("provider account not found: {provider_account_id}")]
    ProviderAccountNotFound {
        /// Missing provider account id.
        provider_account_id: String,
    },
    /// A protected built-in provider account cannot be deleted.
    #[error("protected provider account cannot be deleted: {provider_account_id}")]
    ProtectedProviderAccount {
        /// Protected provider account id.
        provider_account_id: String,
    },
    /// A local-model installation expected to exist was not found.
    #[error("local-model installation not found: {installation_id}")]
    LocalModelInstallationNotFound {
        /// Missing installation id.
        installation_id: String,
    },
    /// A local-model lifecycle transition is not permitted.
    #[error("local-model installation cannot transition from {from} to {to}")]
    InvalidLocalModelTransition {
        /// Current durable status.
        from: String,
        /// Requested durable status.
        to: String,
    },
    /// An active local-model installation cannot be removed.
    #[error("active local-model installation cannot be removed: {installation_id}")]
    ActiveLocalModelInstallation {
        /// Active installation id.
        installation_id: String,
    },
    /// A local-model persistence command contains invalid caller input.
    #[error("invalid local-model request: {kind}")]
    InvalidLocalModelRequest {
        /// Stable invalid-request category.
        kind: &'static str,
    },
    /// A local-model installation is not ready for activation.
    #[error("local-model installation is not ready for activation: {installation_id} ({status})")]
    LocalModelActivationNotReady {
        /// Installation id.
        installation_id: String,
        /// Current durable status.
        status: String,
    },
    /// An agent expected to exist was not found.
    #[error("agent not found: {agent_id}")]
    AgentNotFound {
        /// Missing agent id.
        agent_id: String,
    },
    /// Agent display name input was empty after trimming whitespace.
    #[error("agent display name cannot be empty")]
    AgentDisplayNameEmpty,
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
    /// A turn belongs to a different conversation than the item being written.
    #[error("conversation turn {turn_id} does not belong to conversation {conversation_id}")]
    ConversationTurnConversationMismatch {
        /// Referenced turn id.
        turn_id: String,
        /// Expected conversation id.
        conversation_id: String,
    },
    /// A referenced conversation item expected to exist was not found.
    #[error("conversation item not found: {item_id}")]
    ConversationItemNotFound {
        /// Missing item id.
        item_id: String,
    },
    /// A referenced item belongs to a different conversation than the owner row.
    #[error("conversation item {item_id} does not belong to conversation {conversation_id}")]
    ConversationItemConversationMismatch {
        /// Referenced item id.
        item_id: String,
        /// Expected conversation id.
        conversation_id: String,
    },
    /// An artifact expected to exist was not found.
    #[error("artifact not found: {artifact_id}")]
    ArtifactNotFound {
        /// Missing artifact id.
        artifact_id: String,
    },
    /// Artifact title input was empty after trimming whitespace.
    #[error("artifact title cannot be empty")]
    ArtifactTitleEmpty,
    /// Artifact kind input was empty after trimming whitespace.
    #[error("artifact kind cannot be empty")]
    ArtifactKindEmpty,
    /// Artifact ownership points at an unsupported concrete object type.
    #[error("unsupported artifact owner: {owner_object_type}:{owner_object_id}")]
    UnsupportedArtifactOwner {
        /// Unsupported owner object type.
        owner_object_type: String,
        /// Unsupported owner object id.
        owner_object_id: String,
    },
    /// Artifact version storage does not match the artifact storage kind.
    #[error("artifact version storage kind does not match artifact storage kind")]
    ArtifactStorageKindMismatch,
    /// External artifact URL is not an HTTP(S) URL.
    #[error("artifact external URL must use HTTP or HTTPS: {url}")]
    InvalidArtifactExternalUrl {
        /// Rejected external artifact URL.
        url: String,
    },
}

impl From<ConversationError> for StoreError {
    fn from(error: ConversationError) -> Self {
        match error {
            ConversationError::InvalidEnum { kind, value } => Self::InvalidEnum { kind, value },
            other => Self::Schema(other.to_string()),
        }
    }
}

impl From<ArtifactDomainError> for StoreError {
    fn from(error: ArtifactDomainError) -> Self {
        match error {
            ArtifactDomainError::InvalidStorageKind { value } => Self::InvalidEnum {
                kind: "artifact_storage_kind",
                value,
            },
            ArtifactDomainError::InvalidExternalUrl { url } => {
                Self::InvalidArtifactExternalUrl { url }
            }
            ArtifactDomainError::UnsupportedOwner {
                owner_object_type,
                owner_object_id,
            } => Self::UnsupportedArtifactOwner {
                owner_object_type,
                owner_object_id,
            },
            ArtifactDomainError::TitleEmpty => Self::ArtifactTitleEmpty,
            ArtifactDomainError::KindEmpty => Self::ArtifactKindEmpty,
            ArtifactDomainError::StorageKindMismatch => Self::ArtifactStorageKindMismatch,
            ArtifactDomainError::UnsafeFilename { value } => Self::Schema(format!(
                "unsafe artifact filename reached the metadata store: {value}"
            )),
            ArtifactDomainError::InvalidVersionIndex { version_index } => Self::Schema(format!(
                "invalid artifact version index reached the metadata store: {version_index}"
            )),
        }
    }
}

impl StoreError {
    /// Return true when this store error represents a Noema schema invariant failure.
    #[must_use]
    pub fn is_system_invariant(&self) -> bool {
        matches!(
            self,
            Self::Schema(_) | Self::InvalidEnum { .. } | Self::InvariantViolation { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_errors_build_system_error_events() {
        let error = StoreError::Schema("bad row".to_string());

        // Consuming runtime boundaries now construct diagnostic events; the
        // store only exposes the typed classification they need.
        assert!(error.is_system_invariant());
    }

    #[test]
    fn missing_records_are_not_store_invariants() {
        let error = StoreError::ConversationNotFound {
            conversation_id: "conversation:missing".to_string(),
        };

        assert!(!error.is_system_invariant());
    }

    #[test]
    fn conversation_enum_errors_preserve_store_vocabulary_context() {
        let error = StoreError::from(ConversationError::InvalidEnum {
            kind: "conversation_item_status",
            value: "unknown".to_string(),
        });

        assert!(matches!(
            error,
            StoreError::InvalidEnum {
                kind: "conversation_item_status",
                value,
            } if value == "unknown"
        ));
    }
}
