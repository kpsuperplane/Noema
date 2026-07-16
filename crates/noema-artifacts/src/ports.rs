//! Object-safe artifact metadata persistence boundary.

use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    ArtifactMetadataError, ArtifactOwnerRef, ArtifactStorageKind, ArtifactVersionRecord,
    ArtifactWithVersions, NewArtifact, NewArtifactVersion,
};

/// Minimal append target observed before filesystem publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactAppendTarget {
    /// Stable artifact id.
    pub artifact_id: String,
    /// Concrete owner used to derive governed filesystem layout.
    pub owner: ArtifactOwnerRef,
    /// Storage kind shared by every version of the artifact.
    pub storage_kind: ArtifactStorageKind,
    /// Next version index observed before publication.
    pub expected_next_version_index: i64,
}

/// Boxed future returned by object-safe artifact contracts.
pub type ArtifactFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ArtifactMetadataError>> + Send + 'a>>;

/// Atomic metadata operations required by governed artifact writers.
///
/// Implementations own transactionality and repository-specific error
/// conversion. They must not expose a raw database connection. Dropping a
/// create or append future before it returns `Ready(Ok(_))` must leave no
/// committed metadata: the filesystem service treats future cancellation as a
/// failed metadata write and removes its operation-private publication.
pub trait ArtifactMetadataStore: std::fmt::Debug + Send + Sync {
    /// Allocate one canonical artifact id.
    fn new_artifact_id(&self) -> String;

    /// Allocate one canonical artifact-version id.
    fn new_artifact_version_id(&self) -> String;

    /// Load only the metadata required to prepare an append safely.
    fn load_append_target<'a>(
        &'a self,
        artifact_id: &'a str,
    ) -> ArtifactFuture<'a, Option<ArtifactAppendTarget>>;

    /// Atomically insert an artifact and its initial version.
    ///
    /// Cancellation before `Ready(Ok(_))` must not commit either row.
    fn create_artifact_with_initial_version<'a>(
        &'a self,
        artifact: NewArtifact,
        initial_version: NewArtifactVersion,
    ) -> ArtifactFuture<'a, ArtifactWithVersions>;

    /// Atomically append one version if the next index still matches the
    /// caller's pre-publication observation.
    ///
    /// Implementations must re-read the actual next index inside the same
    /// transaction as the insert. A mismatch returns
    /// [`ArtifactMetadataError::AppendConflict`] without writing metadata.
    /// Cancellation before `Ready(Ok(_))` must not commit the version.
    fn append_artifact_version<'a>(
        &'a self,
        artifact_id: &'a str,
        expected_next_version_index: i64,
        version: NewArtifactVersion,
    ) -> ArtifactFuture<'a, ArtifactVersionRecord>;
}

/// Clonable artifact metadata persistence handle.
pub type ArtifactMetadataStoreHandle = Arc<dyn ArtifactMetadataStore>;
