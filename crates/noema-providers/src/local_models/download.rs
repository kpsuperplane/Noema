//! Resumable, verified installation into Noema's content-addressed GGUF store.

mod transfer;

use std::{fmt, path::PathBuf};

use thiserror::Error;
use tokio::{fs, io::AsyncReadExt};
use tokio_util::sync::CancellationToken;

use noema_home::{NoemaPathError, NoemaPaths};

use crate::{
    LocalModelBackend, LocalModelInstallationPersistenceHandle, LocalModelInstallationRecord,
    LocalModelInstallationStatus, LocalModelSourceKind, NewLocalModelInstallation,
    ProviderPersistenceError,
};

use super::download_support::{
    hugging_face_url, local_file_installation_id, normalized_model_id, normalized_optional,
    required_text, validate_digest, validate_revision,
};
use super::{LocalModelBuild, LocalModelCatalogEntry};

const BYTES_PER_DECIMAL_GB: f64 = 1_000_000_000.0;

/// Input for importing a public GGUF from an immutable Hugging Face revision.
#[derive(Clone, Debug, PartialEq)]
pub struct HuggingFaceLocalModelImport {
    /// Product-facing model name.
    pub name: String,
    /// Provider-facing model profile.
    pub model_id: String,
    /// Public repository in `owner/repository` form.
    pub repo: String,
    /// Immutable 40-character commit.
    pub revision: String,
    /// GGUF path inside the repository.
    pub file: String,
    /// Expected lowercase SHA-256 digest.
    pub sha256: String,
    /// Optional user-supplied license label.
    pub license: Option<String>,
    /// Backend to use for this advanced import.
    pub backend: LocalModelBackend,
}

/// Input for importing an existing GGUF from the local filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalFileModelImport {
    /// Product-facing model name.
    pub name: String,
    /// Provider-facing model profile.
    pub model_id: String,
    /// Existing local GGUF path.
    pub path: PathBuf,
    /// Optional user-supplied license label.
    pub license: Option<String>,
    /// Backend to use for this advanced import.
    pub backend: LocalModelBackend,
}

/// Local-model installation failures.
#[derive(Debug, Error)]
pub enum LocalModelInstallError {
    /// Installation metadata could not be persisted.
    #[error("local-model persistence operation failed: {0}")]
    Persistence(#[from] ProviderPersistenceError),
    /// A model path could not be resolved safely.
    #[error("local-model path resolution failed: {0}")]
    Path(#[from] NoemaPathError),
    /// A filesystem operation failed.
    #[error("local-model filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// An HTTP request failed.
    #[error("local-model download failed: {0}")]
    Http(#[from] reqwest::Error),
    /// User or catalog input was malformed.
    #[error("invalid local-model installation input: {0}")]
    InvalidInput(String),
    /// The source returned a non-successful response.
    #[error("local-model source returned HTTP {status}: {message}")]
    HttpStatus {
        /// HTTP response status.
        status: u16,
        /// Concise response detail.
        message: String,
    },
    /// Available disk space cannot accommodate the remaining bytes.
    #[error(
        "local-model installation needs {required_bytes} bytes but only {available_bytes} are available"
    )]
    InsufficientDisk {
        /// Remaining bytes required.
        required_bytes: u64,
        /// Bytes currently available.
        available_bytes: u64,
    },
    /// The complete artifact did not match its pinned digest.
    #[error("local-model checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Expected catalog or import digest.
        expected: String,
        /// Computed artifact digest.
        actual: String,
    },
    /// A content-addressed path already contains bytes that do not match its name.
    #[error("existing local-model blob does not match its content digest: {sha256}")]
    BlobDigestConflict {
        /// Digest encoded by the occupied blob path.
        sha256: String,
    },
    /// The user cancelled the operation.
    #[error("local-model installation was cancelled")]
    Cancelled,
}

impl LocalModelInstallError {
    pub(super) const fn code(&self) -> &'static str {
        match self {
            Self::Persistence(_) => "store_error",
            Self::Path(_) | Self::Io(_) => "filesystem_error",
            Self::Http(_) | Self::HttpStatus { .. } => "download_error",
            Self::InvalidInput(_) => "invalid_input",
            Self::InsufficientDisk { .. } => "insufficient_disk",
            Self::ChecksumMismatch { .. } | Self::BlobDigestConflict { .. } => "checksum_mismatch",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Installer for catalog downloads and advanced GGUF imports.
#[derive(Clone)]
pub struct LocalModelInstaller {
    pub(super) persistence: LocalModelInstallationPersistenceHandle,
    pub(super) paths: NoemaPaths,
    pub(super) client: reqwest::Client,
}

impl fmt::Debug for LocalModelInstaller {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalModelInstaller")
            .finish_non_exhaustive()
    }
}

impl LocalModelInstaller {
    /// Creates an installer with explicit persistence and Noema-home paths.
    ///
    /// # Errors
    ///
    /// Returns an error if the hardened HTTP client cannot be constructed.
    pub fn new(
        persistence: LocalModelInstallationPersistenceHandle,
        paths: NoemaPaths,
    ) -> Result<Self, LocalModelInstallError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        Ok(Self {
            persistence,
            paths,
            client,
        })
    }

    /// Returns the deterministic installation id for one curated artifact.
    #[must_use]
    pub fn catalog_installation_id(model_id: &str, sha256: &str) -> String {
        format!(
            "local_model_installation:catalog:{model_id}:{}",
            &sha256[..12.min(sha256.len())]
        )
    }

    /// Returns the deterministic installation id for one pinned public GGUF.
    ///
    /// # Errors
    ///
    /// Returns an error when the expected digest is malformed.
    pub fn hugging_face_installation_id(sha256: &str) -> Result<String, LocalModelInstallError> {
        validate_digest(sha256)?;
        Ok(format!(
            "local_model_installation:hugging_face:{}",
            &sha256[..12]
        ))
    }

    /// Returns the deterministic installation id for one local-file import.
    ///
    /// # Errors
    ///
    /// Returns an error when the source is not a non-empty regular GGUF.
    pub async fn local_file_installation_id(
        input: &LocalFileModelImport,
    ) -> Result<String, LocalModelInstallError> {
        if !input.path.is_file()
            || !input
                .path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("gguf"))
        {
            return Err(LocalModelInstallError::InvalidInput(
                "local import must reference a regular .gguf file".to_string(),
            ));
        }
        let metadata = fs::metadata(&input.path).await?;
        if metadata.len() == 0 {
            return Err(LocalModelInstallError::InvalidInput(
                "local GGUF file is empty".to_string(),
            ));
        }
        validate_gguf_magic(&input.path).await?;
        Ok(local_file_installation_id(input, &metadata))
    }

    /// Persists the queued projection for one catalog artifact without starting I/O.
    ///
    /// # Errors
    ///
    /// Returns an error when the build/backend pair or installation metadata is invalid.
    pub async fn queue_catalog_model(
        &self,
        model: &LocalModelCatalogEntry,
        build: &LocalModelBuild,
        backend: LocalModelBackend,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        if !build.backends.contains(&backend) {
            return Err(LocalModelInstallError::InvalidInput(format!(
                "build `{}` does not support {}",
                build.file,
                backend.display_name()
            )));
        }
        self.ensure_queued(NewLocalModelInstallation {
            installation_id: Self::catalog_installation_id(&model.id, &build.sha256),
            model_id: model.id.clone(),
            display_name: model.name.clone(),
            source_kind: LocalModelSourceKind::Catalog,
            source_repo: Some(model.repo.clone()),
            source_revision: Some(model.revision.clone()),
            source_file: Some(build.file.clone()),
            sha256: Some(build.sha256.clone()),
            download_gb: build.download_gb,
            expected_bytes: None,
            license: Some(model.license.clone()),
            backend,
        })
        .await
    }

    /// Installs one pinned catalog build, resuming a partial download when possible.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata, transfer, verification, or persistence.
    /// Terminal state is persisted before returning.
    pub async fn install_catalog_model(
        &self,
        model: &LocalModelCatalogEntry,
        build: &LocalModelBuild,
        backend: LocalModelBackend,
        cancellation: CancellationToken,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        let queued = self.queue_catalog_model(model, build, backend).await?;
        if queued.status == LocalModelInstallationStatus::Installed {
            return Ok(queued);
        }
        let url = hugging_face_url(&model.repo, &model.revision, &build.file)?;
        let result = self
            .run_download(&queued, url, &build.sha256, &cancellation)
            .await;
        self.finish_operation(&queued, result).await
    }

    /// Imports a public GGUF from a pinned Hugging Face commit.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid source data, transfer, checksum, or persistence failures.
    pub async fn import_hugging_face(
        &self,
        input: HuggingFaceLocalModelImport,
        cancellation: CancellationToken,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        let url = hugging_face_url(&input.repo, &input.revision, &input.file)?;
        let queued = self.queue_hugging_face(&input).await?;
        if queued.status == LocalModelInstallationStatus::Installed {
            return Ok(queued);
        }
        let result = self
            .run_download(&queued, url, &input.sha256, &cancellation)
            .await;
        self.finish_operation(&queued, result).await
    }

    /// Persists a queued public-GGUF import without starting network I/O.
    ///
    /// # Errors
    ///
    /// Returns an error for mutable revisions, malformed hashes, or invalid metadata.
    pub async fn queue_hugging_face(
        &self,
        input: &HuggingFaceLocalModelImport,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        let installation_id = Self::hugging_face_installation_id(&input.sha256)?;
        validate_revision(&input.revision)?;
        hugging_face_url(&input.repo, &input.revision, &input.file)?;
        self.ensure_queued(NewLocalModelInstallation {
            installation_id,
            model_id: normalized_model_id(&input.model_id)?,
            display_name: required_text(&input.name, "name")?.to_string(),
            source_kind: LocalModelSourceKind::HuggingFace,
            source_repo: Some(input.repo.clone()),
            source_revision: Some(input.revision.clone()),
            source_file: Some(input.file.clone()),
            sha256: Some(input.sha256.clone()),
            download_gb: 0.001,
            expected_bytes: None,
            license: normalized_optional(input.license.clone()),
            backend: input.backend,
        })
        .await
    }

    /// Copies and content-addresses an existing local GGUF.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid files, cancellation, disk, checksum, or persistence failures.
    pub async fn import_local_file(
        &self,
        input: LocalFileModelImport,
        cancellation: CancellationToken,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        let queued = self.queue_local_file(&input).await?;
        let result = self
            .copy_local_file(&queued, input.path, &cancellation)
            .await;
        self.finish_operation(&queued, result).await
    }

    /// Persists a queued local-file import without copying the file.
    ///
    /// # Errors
    ///
    /// Returns an error when the source is not a non-empty regular GGUF.
    pub async fn queue_local_file(
        &self,
        input: &LocalFileModelImport,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        let installation_id = Self::local_file_installation_id(input).await?;
        let metadata = fs::metadata(&input.path).await?;
        self.ensure_queued(NewLocalModelInstallation {
            installation_id,
            model_id: normalized_model_id(&input.model_id)?,
            display_name: required_text(&input.name, "name")?.to_string(),
            source_kind: LocalModelSourceKind::LocalFile,
            source_repo: None,
            source_revision: None,
            source_file: input
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            sha256: None,
            download_gb: (metadata.len() as f64 / BYTES_PER_DECIMAL_GB).max(0.001),
            expected_bytes: Some(metadata.len()),
            license: normalized_optional(input.license.clone()),
            backend: input.backend,
        })
        .await
    }

    async fn ensure_queued(
        &self,
        input: NewLocalModelInstallation,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        if let Some(existing) = self
            .persistence
            .local_model_installation(&input.installation_id)
            .await?
            && matches!(
                existing.status,
                LocalModelInstallationStatus::Queued
                    | LocalModelInstallationStatus::Downloading
                    | LocalModelInstallationStatus::Verifying
                    | LocalModelInstallationStatus::Installed
            )
        {
            return Ok(existing);
        }
        self.persistence
            .upsert_local_model_installation(input)
            .await
            .map_err(Into::into)
    }
}

pub(super) async fn validate_gguf_magic(
    path: &std::path::Path,
) -> Result<(), LocalModelInstallError> {
    let mut input = fs::File::open(path).await?;
    let mut magic = [0_u8; 4];
    if input.read_exact(&mut magic).await.is_err() || magic != *b"GGUF" {
        return Err(LocalModelInstallError::InvalidInput(
            "model artifact does not have a GGUF header".to_string(),
        ));
    }
    Ok(())
}
