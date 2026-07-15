//! Resumable, verified installation into Noema's content-addressed GGUF store.

use std::path::PathBuf;

use futures_util::StreamExt;
use ring::digest::{Context as DigestContext, SHA256};
use thiserror::Error;
use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{NoemaPathError, NoemaPaths, NoemaStore, StoreError};

use super::download_support::{
    InstalledArtifact, hash_file, hex_digest, hugging_face_url, local_file_installation_id,
    normalized_model_id, normalized_optional, remove_file_if_present, required_text,
    validate_digest, validate_revision,
};
use super::{
    LocalModelBackend, LocalModelBuild, LocalModelCatalogEntry, LocalModelInstallationRecord,
    LocalModelInstallationStatus, LocalModelInstallationUpdate, LocalModelSourceKind,
    NewLocalModelInstallation, RemovedLocalModelInstallation,
};

const BYTES_PER_DECIMAL_GB: f64 = 1_000_000_000.0;
const PROGRESS_INTERVAL_BYTES: u64 = 8 * 1024 * 1024;

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
    #[error("local-model store operation failed: {0}")]
    Store(#[from] StoreError),
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
    /// The user cancelled the operation.
    #[error("local-model installation was cancelled")]
    Cancelled,
}

impl LocalModelInstallError {
    fn code(&self) -> &'static str {
        match self {
            Self::Store(_) => "store_error",
            Self::Path(_) | Self::Io(_) => "filesystem_error",
            Self::Http(_) | Self::HttpStatus { .. } => "download_error",
            Self::InvalidInput(_) => "invalid_input",
            Self::InsufficientDisk { .. } => "insufficient_disk",
            Self::ChecksumMismatch { .. } => "checksum_mismatch",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Installer for catalog downloads and advanced GGUF imports.
#[derive(Clone, Debug)]
pub struct LocalModelInstaller {
    store: NoemaStore,
    paths: NoemaPaths,
    client: reqwest::Client,
}

impl LocalModelInstaller {
    /// Creates an installer rooted in the current Noema home.
    ///
    /// # Errors
    ///
    /// Returns an error if the hardened HTTP client cannot be constructed.
    pub fn new(store: NoemaStore, paths: NoemaPaths) -> Result<Self, LocalModelInstallError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        Ok(Self {
            store,
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
    /// Returns an error for invalid metadata, transfer, verification, persistence,
    /// or activation failures. Terminal state is persisted before returning.
    pub async fn install_catalog_model(
        &self,
        model: &LocalModelCatalogEntry,
        build: &LocalModelBuild,
        backend: LocalModelBackend,
        activate_on_complete: bool,
        cancellation: CancellationToken,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        let queued = self.queue_catalog_model(model, build, backend).await?;
        if queued.status == LocalModelInstallationStatus::Installed {
            if activate_on_complete && !queued.is_active {
                self.store
                    .activate_local_model_as_system_default(&queued.installation_id)
                    .await?;
            }
            return self.required_installation(&queued.installation_id).await;
        }
        let url = hugging_face_url(&model.repo, &model.revision, &build.file)?;
        let result = self
            .run_download(&queued, url, &build.sha256, &cancellation)
            .await;
        let installed = self.finish_operation(&queued, result).await?;
        if activate_on_complete {
            self.store
                .activate_local_model_as_system_default(&installed.installation_id)
                .await?;
            return self.required_installation(&installed.installation_id).await;
        }
        Ok(installed)
    }

    /// Imports a public GGUF from a pinned Hugging Face commit.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid source data, transfer, checksum, or storage failures.
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
    /// Returns an error for invalid files, cancellation, disk, checksum, or storage failures.
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

    /// Removes installation metadata and its unreferenced content-addressed blob.
    ///
    /// # Errors
    ///
    /// Returns an error when the installation is active, persistence fails, or
    /// an unreferenced blob cannot be removed.
    pub async fn remove(
        &self,
        installation_id: &str,
    ) -> Result<RemovedLocalModelInstallation, LocalModelInstallError> {
        let removed = self
            .store
            .remove_local_model_installation(installation_id)
            .await?;
        if removed.unreferenced_blob_relative_path.is_some()
            && let Some(sha256) = removed.installation.sha256.as_deref()
        {
            let path = self.paths.local_model_blob_path(sha256)?;
            remove_file_if_present(path).await?;
        }
        if let Some(sha256) = removed.installation.sha256.as_deref() {
            remove_file_if_present(self.paths.local_model_partial_path(sha256)?).await?;
        }
        remove_file_if_present(self.paths.local_model_import_partial_path(installation_id)).await?;
        Ok(removed)
    }

    async fn ensure_queued(
        &self,
        input: NewLocalModelInstallation,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        if let Some(existing) = self
            .store
            .get_local_model_installation(&input.installation_id)
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
        self.store
            .upsert_local_model_installation(input)
            .await
            .map_err(Into::into)
    }

    async fn run_download(
        &self,
        installation: &LocalModelInstallationRecord,
        url: Url,
        expected_sha256: &str,
        cancellation: &CancellationToken,
    ) -> Result<InstalledArtifact, LocalModelInstallError> {
        self.ensure_directories().await?;
        let partial = self.paths.local_model_partial_path(expected_sha256)?;
        let existing_bytes = fs::metadata(&partial)
            .await
            .map_or(0, |metadata| metadata.len());
        let mut request = self.client.get(url);
        if existing_bytes > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={existing_bytes}-"));
        }
        let response = request.send().await?;
        if existing_bytes > 0 && response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            return self
                .verify_downloaded_file(
                    installation,
                    partial,
                    expected_sha256,
                    existing_bytes,
                    Some(existing_bytes),
                    cancellation,
                )
                .await;
        }
        let append =
            existing_bytes > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        if !response.status().is_success() {
            return Err(LocalModelInstallError::HttpStatus {
                status: response.status().as_u16(),
                message: response
                    .text()
                    .await
                    .unwrap_or_else(|error| error.to_string())
                    .chars()
                    .take(512)
                    .collect(),
            });
        }
        let starting_bytes = if append { existing_bytes } else { 0 };
        let expected_bytes = response
            .content_length()
            .and_then(|remaining| remaining.checked_add(starting_bytes));
        self.preflight_disk(
            expected_bytes
                .unwrap_or((installation.download_gb * BYTES_PER_DECIMAL_GB) as u64)
                .saturating_sub(starting_bytes),
        )?;
        self.persist_state(
            installation,
            LocalModelInstallationStatus::Downloading,
            starting_bytes,
            expected_bytes,
            None,
            None,
            None,
        )
        .await?;
        let mut output = OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&partial)
            .await?;
        let mut stream = response.bytes_stream();
        let mut downloaded = starting_bytes;
        let mut last_reported = downloaded;
        loop {
            let next = tokio::select! {
                () = cancellation.cancelled() => return Err(LocalModelInstallError::Cancelled),
                next = stream.next() => next,
            };
            let Some(chunk) = next else { break };
            let chunk = chunk?;
            output.write_all(&chunk).await?;
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            if downloaded.saturating_sub(last_reported) >= PROGRESS_INTERVAL_BYTES {
                self.persist_state(
                    installation,
                    LocalModelInstallationStatus::Downloading,
                    downloaded,
                    expected_bytes,
                    None,
                    None,
                    None,
                )
                .await?;
                last_reported = downloaded;
            }
        }
        output.flush().await?;
        if expected_bytes.is_some_and(|expected| expected != downloaded) {
            return Err(LocalModelInstallError::InvalidInput(format!(
                "download ended at {downloaded} bytes; expected {expected_bytes:?}"
            )));
        }
        self.verify_downloaded_file(
            installation,
            partial,
            expected_sha256,
            downloaded,
            expected_bytes,
            cancellation,
        )
        .await
    }

    async fn verify_downloaded_file(
        &self,
        installation: &LocalModelInstallationRecord,
        partial: PathBuf,
        expected_sha256: &str,
        downloaded_bytes: u64,
        expected_bytes: Option<u64>,
        cancellation: &CancellationToken,
    ) -> Result<InstalledArtifact, LocalModelInstallError> {
        self.persist_state(
            installation,
            LocalModelInstallationStatus::Verifying,
            downloaded_bytes,
            expected_bytes,
            None,
            None,
            None,
        )
        .await?;
        let actual = hash_file(&partial, cancellation).await?;
        if actual != expected_sha256 {
            let _ = fs::remove_file(&partial).await;
            return Err(LocalModelInstallError::ChecksumMismatch {
                expected: expected_sha256.to_string(),
                actual,
            });
        }
        validate_gguf_magic(&partial).await?;
        self.install_verified(
            partial,
            actual,
            downloaded_bytes,
            expected_bytes,
            cancellation,
        )
        .await
    }

    async fn copy_local_file(
        &self,
        installation: &LocalModelInstallationRecord,
        source: PathBuf,
        cancellation: &CancellationToken,
    ) -> Result<InstalledArtifact, LocalModelInstallError> {
        self.ensure_directories().await?;
        let expected_bytes = fs::metadata(&source).await?.len();
        self.preflight_disk(expected_bytes)?;
        let partial = self
            .paths
            .local_model_import_partial_path(&installation.installation_id);
        let mut input = fs::File::open(source).await?;
        let mut output = fs::File::create(&partial).await?;
        let mut digest = DigestContext::new(&SHA256);
        let mut buffer = vec![0_u8; 1024 * 1024];
        let mut copied = 0_u64;
        let mut last_reported = 0_u64;
        self.persist_state(
            installation,
            LocalModelInstallationStatus::Downloading,
            0,
            Some(expected_bytes),
            None,
            None,
            None,
        )
        .await?;
        loop {
            let read = tokio::select! {
                () = cancellation.cancelled() => return Err(LocalModelInstallError::Cancelled),
                read = input.read(&mut buffer) => read?,
            };
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read]).await?;
            digest.update(&buffer[..read]);
            copied = copied.saturating_add(read as u64);
            if copied.saturating_sub(last_reported) >= PROGRESS_INTERVAL_BYTES {
                self.persist_state(
                    installation,
                    LocalModelInstallationStatus::Downloading,
                    copied,
                    Some(expected_bytes),
                    None,
                    None,
                    None,
                )
                .await?;
                last_reported = copied;
            }
        }
        output.flush().await?;
        self.persist_state(
            installation,
            LocalModelInstallationStatus::Verifying,
            copied,
            Some(expected_bytes),
            None,
            None,
            None,
        )
        .await?;
        let sha256 = hex_digest(digest.finish().as_ref());
        self.install_verified(partial, sha256, copied, Some(expected_bytes), cancellation)
            .await
    }

    async fn install_verified(
        &self,
        partial: PathBuf,
        sha256: String,
        downloaded_bytes: u64,
        expected_bytes: Option<u64>,
        cancellation: &CancellationToken,
    ) -> Result<InstalledArtifact, LocalModelInstallError> {
        if cancellation.is_cancelled() {
            return Err(LocalModelInstallError::Cancelled);
        }
        let blob = self.paths.local_model_blob_path(&sha256)?;
        if fs::try_exists(&blob).await? {
            let existing_sha256 = hash_file(&blob, cancellation).await?;
            if existing_sha256 == sha256 {
                fs::remove_file(&partial).await?;
            } else {
                fs::remove_file(&blob).await?;
                fs::rename(&partial, &blob).await?;
            }
        } else {
            fs::rename(&partial, &blob).await?;
        }
        Ok(InstalledArtifact {
            sha256: sha256.clone(),
            downloaded_bytes,
            expected_bytes,
            blob_relative_path: format!("models/blobs/{sha256}.gguf"),
        })
    }

    async fn finish_operation(
        &self,
        installation: &LocalModelInstallationRecord,
        result: Result<InstalledArtifact, LocalModelInstallError>,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        match result {
            Ok(artifact) => {
                self.persist_state(
                    installation,
                    LocalModelInstallationStatus::Installed,
                    artifact.downloaded_bytes,
                    artifact.expected_bytes,
                    Some(artifact.sha256),
                    Some(artifact.blob_relative_path),
                    None,
                )
                .await
            }
            Err(LocalModelInstallError::Cancelled) => {
                let current = self
                    .required_installation(&installation.installation_id)
                    .await?;
                self.persist_state(
                    &current,
                    LocalModelInstallationStatus::Cancelled,
                    current.downloaded_bytes,
                    current.expected_bytes,
                    None,
                    None,
                    None,
                )
                .await?;
                Err(LocalModelInstallError::Cancelled)
            }
            Err(error) => {
                let current = self
                    .required_installation(&installation.installation_id)
                    .await?;
                let message = error.to_string();
                self.persist_state(
                    &current,
                    LocalModelInstallationStatus::Failed,
                    current.downloaded_bytes,
                    current.expected_bytes,
                    None,
                    None,
                    Some((error.code(), message.as_str())),
                )
                .await?;
                Err(error)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn persist_state(
        &self,
        installation: &LocalModelInstallationRecord,
        status: LocalModelInstallationStatus,
        downloaded_bytes: u64,
        expected_bytes: Option<u64>,
        sha256: Option<String>,
        blob_relative_path: Option<String>,
        error: Option<(&str, &str)>,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        self.store
            .update_local_model_installation(
                &installation.installation_id,
                LocalModelInstallationUpdate {
                    status,
                    downloaded_bytes,
                    expected_bytes,
                    sha256,
                    blob_relative_path,
                    error_code: error.map(|(code, _)| code.to_string()),
                    error_message: error.map(|(_, message)| message.to_string()),
                },
            )
            .await
            .map_err(Into::into)
    }

    async fn required_installation(
        &self,
        installation_id: &str,
    ) -> Result<LocalModelInstallationRecord, LocalModelInstallError> {
        self.store
            .get_local_model_installation(installation_id)
            .await?
            .ok_or_else(|| {
                LocalModelInstallError::InvalidInput(format!(
                    "installation disappeared: {installation_id}"
                ))
            })
    }

    async fn ensure_directories(&self) -> Result<(), LocalModelInstallError> {
        fs::create_dir_all(self.paths.local_model_blobs_dir()).await?;
        fs::create_dir_all(self.paths.local_model_downloads_dir()).await?;
        Ok(())
    }

    fn preflight_disk(&self, required_bytes: u64) -> Result<(), LocalModelInstallError> {
        let available_bytes = fs2::available_space(self.paths.local_models_dir())?;
        if available_bytes < required_bytes {
            return Err(LocalModelInstallError::InsufficientDisk {
                required_bytes,
                available_bytes,
            });
        }
        Ok(())
    }
}

async fn validate_gguf_magic(path: &std::path::Path) -> Result<(), LocalModelInstallError> {
    let mut input = fs::File::open(path).await?;
    let mut magic = [0_u8; 4];
    if input.read_exact(&mut magic).await.is_err() || magic != *b"GGUF" {
        return Err(LocalModelInstallError::InvalidInput(
            "model artifact does not have a GGUF header".to_string(),
        ));
    }
    Ok(())
}
