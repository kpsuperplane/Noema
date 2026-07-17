//! Transfer, verification, and lifecycle-state mechanics for local-model installation.

use std::path::PathBuf;

use futures_util::StreamExt;
use ring::digest::{Context as DigestContext, SHA256};
use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{
    LocalModelInstallationRecord, LocalModelInstallationStatus, LocalModelInstallationUpdate,
};

use super::{
    BYTES_PER_DECIMAL_GB, LocalModelInstallError, LocalModelInstaller, validate_gguf_magic,
};
use crate::local_models::download_support::{InstalledArtifact, hash_file, hex_digest};

const PROGRESS_INTERVAL_BYTES: u64 = 8 * 1024 * 1024;

impl LocalModelInstaller {
    pub(super) async fn run_download(
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

    pub(super) async fn copy_local_file(
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
                let is_referenced = self
                    .persistence
                    .local_model_installations()
                    .await?
                    .iter()
                    .any(|installation| {
                        installation.sha256.as_deref() == Some(sha256.as_str())
                            && installation.blob_relative_path.is_some()
                    });
                if is_referenced {
                    return Err(LocalModelInstallError::BlobDigestConflict { sha256 });
                }
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

    pub(super) async fn finish_operation(
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
        self.persistence
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
        self.persistence
            .local_model_installation(installation_id)
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
