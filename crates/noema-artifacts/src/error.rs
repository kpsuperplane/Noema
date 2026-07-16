//! Artifact-owned error vocabulary.

use std::path::PathBuf;

use thiserror::Error;

/// Artifact semantic validation failures.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ArtifactDomainError {
    /// A stored storage-kind label is outside the closed artifact vocabulary.
    #[error("invalid artifact storage kind: {value}")]
    InvalidStorageKind {
        /// Rejected storage-kind label.
        value: String,
    },
    /// An external artifact URL was not a valid HTTP(S) URL.
    #[error("artifact external URL must use HTTP or HTTPS")]
    InvalidExternalUrl {
        /// Rejected URL.
        url: String,
    },
    /// An artifact filename was not one safe portable path segment.
    #[error("artifact filename must be a single safe path segment")]
    UnsafeFilename {
        /// Rejected raw filename.
        value: String,
    },
    /// Artifact ownership did not name a supported concrete owner.
    #[error("unsupported artifact owner: {owner_object_type}:{owner_object_id}")]
    UnsupportedOwner {
        /// Unsupported owner object type.
        owner_object_type: String,
        /// Unsupported owner object id.
        owner_object_id: String,
    },
    /// The artifact title was empty after trimming whitespace.
    #[error("artifact title cannot be empty")]
    TitleEmpty,
    /// The artifact kind was empty after trimming whitespace.
    #[error("artifact kind cannot be empty")]
    KindEmpty,
    /// The version storage kind did not match its artifact.
    #[error("artifact version storage kind does not match artifact storage kind")]
    StorageKindMismatch,
    /// A version index was not positive.
    #[error("artifact version index must be positive: {version_index}")]
    InvalidVersionIndex {
        /// Rejected version index.
        version_index: i64,
    },
}

/// Failures returned by an artifact metadata persistence port.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ArtifactMetadataError {
    /// Artifact semantic validation failed before persistence.
    #[error(transparent)]
    Domain(#[from] ArtifactDomainError),
    /// The requested artifact did not exist.
    #[error("artifact not found: {artifact_id}")]
    NotFound {
        /// Missing artifact id.
        artifact_id: String,
    },
    /// A concurrent appender already claimed the expected version index.
    #[error(
        "artifact append conflict for {artifact_id}: expected next index {expected_next_version_index}, actual next index {actual_next_version_index}"
    )]
    AppendConflict {
        /// Artifact whose version history changed.
        artifact_id: String,
        /// Next index observed before filesystem publication.
        expected_next_version_index: i64,
        /// Next index observed inside the metadata transaction.
        actual_next_version_index: i64,
    },
    /// Persisted artifact metadata violated a closed invariant.
    #[error("artifact metadata invariant violated")]
    Invariant {
        /// Safe invariant detail.
        message: String,
    },
    /// The backing metadata repository failed.
    #[error("artifact metadata persistence failed")]
    Persistence {
        /// Safe persistence failure detail.
        message: String,
    },
}

/// Failures returned by consumer-facing artifact operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ArtifactOperationError {
    /// Artifact metadata work failed.
    #[error(transparent)]
    Metadata(#[from] ArtifactMetadataError),
    /// Artifact path validation failed.
    #[error(transparent)]
    Domain(#[from] ArtifactDomainError),
    /// A governed filesystem operation failed.
    #[error("artifact filesystem operation failed: {operation}")]
    Filesystem {
        /// Stable operation name.
        operation: &'static str,
        /// Governed path involved in the failure.
        path: PathBuf,
        /// Safe filesystem failure detail.
        message: String,
    },
    /// Published bytes did not match their expected size or digest.
    #[error("artifact content verification failed")]
    Integrity {
        /// Path whose bytes failed verification.
        path: PathBuf,
    },
    /// Metadata failed and cleanup of this operation's private object also failed.
    #[error("artifact metadata failed after publication: {metadata_error}; cleanup failed")]
    MetadataRollback {
        /// Private published object that could not be removed.
        path: PathBuf,
        /// Metadata failure which triggered cleanup.
        #[source]
        metadata_error: ArtifactMetadataError,
        /// Safe cleanup failure detail.
        cleanup_message: String,
    },
}
