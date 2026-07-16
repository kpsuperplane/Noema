//! Governed artifact contracts and semantic models.
//!
//! The default build contains only domain models and object-safe service
//! boundaries. Governed local filesystem implementation is added by the
//! `filesystem` feature in the next extraction checkpoint.

mod domain;
mod error;
#[cfg(feature = "filesystem")]
mod filesystem;
mod operations;
mod paths;
mod ports;

pub use domain::{
    ArtifactOwnerRef, ArtifactRecord, ArtifactSource, ArtifactStorageKind, ArtifactVersionRecord,
    ArtifactVersionStorage, ArtifactWithVersions, NewArtifact, NewArtifactVersion,
    validate_external_artifact_url,
};
pub use error::{ArtifactDomainError, ArtifactMetadataError, ArtifactOperationError};
#[cfg(feature = "filesystem")]
pub use filesystem::LocalArtifactService;
pub use operations::{
    AppendLocalArtifactVersionRequest, ArtifactFileContent, ArtifactOperationFuture,
    ArtifactOperations, ArtifactOperationsHandle, CreateLocalArtifactRequest,
    ReadLocalArtifactRequest,
};
pub use paths::{
    artifact_download_url, artifact_version_dir, artifact_version_id_from_download_slug,
    owner_artifacts_dir, safe_artifact_filename,
};
pub use ports::{
    ArtifactAppendTarget, ArtifactFuture, ArtifactMetadataStore, ArtifactMetadataStoreHandle,
};
