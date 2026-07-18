use noema_artifacts::ArtifactDomainError;
use noema_conversations::ConversationError;
use noema_tasks::WorkDomainError;
use noema_workspaces::WorkspaceInputError;
use thiserror::Error;

/// Machine-readable reason an existing SQLite schema was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaIncompatibility {
    /// SQLite schema objects do not exactly match the canonical bootstrap.
    Shape {
        /// Number of schema objects required by this binary.
        expected_object_count: usize,
        /// Number of schema objects found in the database.
        found_object_count: usize,
    },
    /// The exact schema objects exist, but the marker row set is not current.
    Marker {
        /// Marker name required by this binary.
        expected_name: &'static str,
        /// Marker names found in the database, including unexpected extras.
        found_names: Vec<String>,
        /// Schema version required by this binary.
        expected_version: i64,
        /// Found version when the database contains exactly one marker row.
        found_version: Option<i64>,
    },
    /// SQLite could not read the schema metadata as a valid schema.
    Unreadable,
    /// The file changed between immutable inspection and writable open.
    ChangedDuringOpen,
}

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
    /// A semantic Work command, fence, or domain invariant was rejected.
    #[error("{0}")]
    Work(#[from] WorkDomainError),
    /// A workspace/project identity or record input was rejected.
    #[error("{0}")]
    Workspace(#[from] WorkspaceInputError),
    /// Schema bootstrap returned an invalid result.
    #[error("store schema bootstrap failed: {0}")]
    Schema(String),
    /// The SQLite file is not the exact schema understood by this binary.
    #[error("incompatible store schema ({kind:?}): {reason}")]
    IncompatibleSchema {
        /// Machine-readable incompatibility classification.
        kind: SchemaIncompatibility,
        /// Diagnostic explanation of the rejected schema state.
        reason: String,
    },
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
    /// A canonical or future-work selection still names the provider account.
    #[error("provider account is still in use: {provider_account_id}")]
    ProviderAccountInUse {
        /// Referenced provider account id.
        provider_account_id: String,
    },
    /// A durable provider selection omitted its exact process identity.
    #[error("provider selection is missing an exact instance key")]
    ProviderInstanceKeyMissing,
    /// A durable provider selection's exact key disagrees with its account or profile.
    #[error(
        "provider instance key does not match the selected account/profile: {provider_instance_key}"
    )]
    ProviderInstanceKeyMismatch {
        /// Exact key rejected by the authoritative store.
        provider_instance_key: String,
    },
    /// A durable provider selection names an instance already claimed for retirement.
    #[error("provider instance has been claimed for retirement: {provider_instance_key}")]
    ProviderInstanceClaimed {
        /// Claimed exact key.
        provider_instance_key: String,
    },
    /// A provider selection names an instance that cannot currently accept new references.
    #[error("provider instance is unavailable for selection: {provider_instance_key}")]
    ProviderInstanceUnavailable {
        /// Unavailable exact key.
        provider_instance_key: String,
    },
    /// A local provider instance is still named by durable future work.
    #[error("provider instance is still referenced: {provider_instance_key}")]
    ProviderInstanceReferenced {
        /// Referenced exact key.
        provider_instance_key: String,
    },
    /// A local-model retirement compare-and-set lost its guard.
    #[error("local-model retirement persistence conflict during {operation}")]
    LocalModelRetirementConflict {
        /// Stable operation identifier.
        operation: &'static str,
    },
    /// The configured startup default could not be resolved to an exact provider instance.
    #[error("configured default provider selection is unresolvable: {reason}")]
    ConfiguredDefaultUnresolvable {
        /// Stable non-secret diagnostic.
        reason: String,
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
            Self::Schema(_)
                | Self::IncompatibleSchema { .. }
                | Self::InvalidEnum { .. }
                | Self::InvariantViolation { .. }
                | Self::ProviderInstanceKeyMissing
                | Self::ProviderInstanceKeyMismatch { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_invariant_classification_is_explicit_and_fail_closed() {
        for error in [
            StoreError::Schema("bad row".to_string()),
            StoreError::IncompatibleSchema {
                kind: SchemaIncompatibility::Unreadable,
                reason: "future marker".to_string(),
            },
            StoreError::ProviderInstanceKeyMissing,
            StoreError::ProviderInstanceKeyMismatch {
                provider_instance_key: "bad-key".to_string(),
            },
        ] {
            assert!(error.is_system_invariant(), "{error:?}");
        }
        for error in [
            StoreError::ConversationNotFound {
                conversation_id: "conversation:missing".to_string(),
            },
            StoreError::ProviderInstanceUnavailable {
                provider_instance_key: "temporarily-down".to_string(),
            },
        ] {
            assert!(!error.is_system_invariant(), "{error:?}");
        }

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
