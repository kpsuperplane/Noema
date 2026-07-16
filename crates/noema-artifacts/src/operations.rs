//! Consumer-facing governed artifact operations.

use std::{future::Future, pin::Pin, sync::Arc};

use serde_json::Value;

use crate::{
    ArtifactOperationError, ArtifactOwnerRef, ArtifactRecord, ArtifactSource,
    ArtifactVersionRecord, ArtifactWithVersions,
};

/// Request to create a conversation- or task-owned local file artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateLocalArtifactRequest {
    /// Concrete artifact owner.
    pub owner: ArtifactOwnerRef,
    /// Human-readable artifact title.
    pub title: String,
    /// Optional artifact description.
    pub description: Option<String>,
    /// Product-defined artifact kind.
    pub artifact_kind: String,
    /// Logical safe filename shown to users.
    pub filename: String,
    /// Initial immutable bytes.
    pub bytes: Vec<u8>,
    /// Optional media type.
    pub media_type: Option<String>,
    /// Actor creating the artifact.
    pub created_by_actor_id: String,
    /// Optional transcript provenance.
    pub source: ArtifactSource,
    /// Arbitrary artifact metadata.
    pub metadata: Value,
}

/// Request to append a local file version.
#[derive(Debug, Clone, PartialEq)]
pub struct AppendLocalArtifactVersionRequest {
    /// Existing artifact id.
    pub artifact_id: String,
    /// Optional version-specific title.
    pub title: Option<String>,
    /// Logical safe filename shown to users.
    pub filename: String,
    /// Immutable version bytes.
    pub bytes: Vec<u8>,
    /// Optional media type.
    pub media_type: Option<String>,
    /// Actor creating the version.
    pub created_by_actor_id: String,
    /// Optional transcript provenance.
    pub source: ArtifactSource,
    /// Arbitrary version metadata.
    pub metadata: Value,
}

/// Request to read one already-authorized local artifact version.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadLocalArtifactRequest {
    /// Parent artifact metadata used to validate owner-relative layout.
    pub artifact: ArtifactRecord,
    /// Immutable version metadata containing the relative object path.
    pub version: ArtifactVersionRecord,
}

/// Verified local artifact bytes returned to a consumer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactFileContent {
    /// Logical safe filename suitable for a download response.
    pub filename: String,
    /// Verified immutable bytes.
    pub bytes: Vec<u8>,
}

/// Boxed future returned by object-safe artifact operations.
pub type ArtifactOperationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ArtifactOperationError>> + Send + 'a>>;

/// Governed artifact operations consumed by runtime and API layers.
///
/// The interface deliberately exposes no Noema root, logger, metadata store,
/// or concrete filesystem writer.
pub trait ArtifactOperations: std::fmt::Debug + Send + Sync {
    /// Create a local artifact after publishing its bytes safely.
    fn create_local_file(
        &self,
        request: CreateLocalArtifactRequest,
    ) -> ArtifactOperationFuture<'_, ArtifactWithVersions>;

    /// Append one immutable local artifact version.
    fn append_local_file_version(
        &self,
        request: AppendLocalArtifactVersionRequest,
    ) -> ArtifactOperationFuture<'_, ArtifactVersionRecord>;

    /// Read and verify one already-authorized local artifact version.
    fn read_local_file(
        &self,
        request: ReadLocalArtifactRequest,
    ) -> ArtifactOperationFuture<'_, ArtifactFileContent>;
}

/// Clonable governed artifact operations handle.
pub type ArtifactOperationsHandle = Arc<dyn ArtifactOperations>;
