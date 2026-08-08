//! Store-free verified model materialization for isolated evaluations.

use std::{
    fs::{File, OpenOptions as StdOpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};

use fs2::FileExt;
use futures_util::StreamExt;
use ring::digest::{Context as DigestContext, SHA256};
use tokio::{fs, io::AsyncWriteExt, time::sleep};
use tokio_util::sync::CancellationToken;
use url::Url;

use super::LocalModelEvalError;
use crate::local_models::{
    download::validate_gguf_magic,
    download_support::{hash_file, hugging_face_url, validate_digest},
};

/// Immutable public source for one evaluation artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedEvalModelSource {
    /// Hugging Face repository in `owner/repository` form.
    pub repo: String,
    /// Immutable 40-character revision.
    pub revision: String,
    /// Safe GGUF path inside the repository.
    pub file: String,
}

/// Inputs for store-free content-addressed evaluation materialization.
#[derive(Clone)]
pub struct MaterializeVerifiedEvalModelRequest {
    /// Immutable public artifact provenance.
    pub source: VerifiedEvalModelSource,
    /// Exact expected file size.
    pub expected_bytes: u64,
    /// Expected lowercase SHA-256 digest.
    pub sha256: String,
    /// Explicit directory that owns evaluation blobs and temporary files.
    pub cache_root: PathBuf,
}

impl std::fmt::Debug for MaterializeVerifiedEvalModelRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MaterializeVerifiedEvalModelRequest")
            .field("source", &self.source)
            .field("expected_bytes", &self.expected_bytes)
            .field("sha256", &self.sha256)
            .field("cache_root", &self.cache_root)
            .finish()
    }
}

/// Download, verify, and atomically publish one evaluation GGUF.
///
/// The operation uses only the explicit cache root. It never opens production
/// persistence, creates installation rows, or resolves a Noema home.
///
/// # Errors
///
/// Returns [`LocalModelEvalError`] for invalid immutable provenance,
/// cancellation, download failure, size/digest mismatch, or filesystem failure.
pub async fn materialize_verified_eval_model(
    request: MaterializeVerifiedEvalModelRequest,
    cancellation: CancellationToken,
) -> Result<PathBuf, LocalModelEvalError> {
    validate_request(&request)?;
    let url = hugging_face_url(
        &request.source.repo,
        &request.source.revision,
        &request.source.file,
    )
    .map_err(|error| LocalModelEvalError::InvalidInput(error.to_string()))?;
    materialize_from_url(request, url, cancellation).await
}

async fn materialize_from_url(
    request: MaterializeVerifiedEvalModelRequest,
    url: Url,
    cancellation: CancellationToken,
) -> Result<PathBuf, LocalModelEvalError> {
    if cancellation.is_cancelled() {
        return Err(LocalModelEvalError::Cancelled);
    }
    fs::create_dir_all(&request.cache_root)
        .await
        .map_err(materialization)?;
    let destination = request.cache_root.join(format!("{}.gguf", request.sha256));
    let lock_path = request.cache_root.join(format!(".{}.lock", request.sha256));
    let _lock = acquire_lock(&lock_path, &cancellation).await?;

    if verified_existing(&destination, &request, &cancellation).await? {
        return Ok(destination);
    }
    remove_if_present(&destination).await?;
    let partial = request
        .cache_root
        .join(format!(".{}.partial", request.sha256));
    remove_if_present(&partial).await?;

    let result = download_and_publish(&request, &url, &partial, &destination, &cancellation).await;
    if result.is_err() {
        let _ = fs::remove_file(&partial).await;
    }
    result.map(|()| destination)
}

fn validate_request(
    request: &MaterializeVerifiedEvalModelRequest,
) -> Result<(), LocalModelEvalError> {
    if request.expected_bytes == 0 {
        return Err(LocalModelEvalError::InvalidInput(
            "expected byte count must be greater than zero".to_string(),
        ));
    }
    validate_digest(&request.sha256)
        .map_err(|error| LocalModelEvalError::InvalidInput(error.to_string()))?;
    if request.cache_root.as_os_str().is_empty() {
        return Err(LocalModelEvalError::InvalidInput(
            "cache root is required".to_string(),
        ));
    }
    Ok(())
}

async fn acquire_lock(
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<EvalFileLock, LocalModelEvalError> {
    let file = StdOpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(materialization)?;
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(EvalFileLock(file)),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::select! {
                    () = cancellation.cancelled() => {
                        return Err(LocalModelEvalError::Cancelled);
                    }
                    () = sleep(Duration::from_millis(25)) => {}
                }
            }
            Err(error) => return Err(materialization(error)),
        }
    }
}

async fn verified_existing(
    path: &PathBuf,
    request: &MaterializeVerifiedEvalModelRequest,
    cancellation: &CancellationToken,
) -> Result<bool, LocalModelEvalError> {
    let metadata = match fs::metadata(path).await {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(materialization(error)),
    };
    if metadata.len() != request.expected_bytes {
        return Ok(false);
    }
    let actual = hash_file(path, cancellation)
        .await
        .map_err(map_install_error)?;
    if actual != request.sha256 {
        return Ok(false);
    }
    validate_gguf_magic(path).await.map_err(map_install_error)?;
    Ok(true)
}

async fn download_and_publish(
    request: &MaterializeVerifiedEvalModelRequest,
    url: &Url,
    partial: &PathBuf,
    destination: &Path,
    cancellation: &CancellationToken,
) -> Result<(), LocalModelEvalError> {
    let available = fs2::available_space(&request.cache_root).map_err(materialization)?;
    if available < request.expected_bytes {
        return Err(LocalModelEvalError::Materialization(format!(
            "evaluation artifact needs {} bytes but only {available} are available",
            request.expected_bytes
        )));
    }
    let client = reqwest::Client::builder()
        .build()
        .map_err(|error| LocalModelEvalError::Materialization(error.to_string()))?;
    let response = tokio::select! {
        () = cancellation.cancelled() => return Err(LocalModelEvalError::Cancelled),
        response = client.get(url.clone()).send() => {
            response.map_err(|error| LocalModelEvalError::Materialization(error.to_string()))?
        }
    };
    if !response.status().is_success() {
        return Err(LocalModelEvalError::Materialization(format!(
            "source returned HTTP {}",
            response.status().as_u16()
        )));
    }
    if response
        .content_length()
        .is_some_and(|length| length != request.expected_bytes)
    {
        return Err(LocalModelEvalError::Materialization(
            "source content length did not match the pinned byte count".to_string(),
        ));
    }

    let mut output = fs::File::create(partial).await.map_err(materialization)?;
    let mut digest = DigestContext::new(&SHA256);
    let mut downloaded = 0_u64;
    let mut stream = response.bytes_stream();
    loop {
        let next = tokio::select! {
            () = cancellation.cancelled() => return Err(LocalModelEvalError::Cancelled),
            next = stream.next() => next,
        };
        let Some(chunk) = next else { break };
        let chunk =
            chunk.map_err(|error| LocalModelEvalError::Materialization(error.to_string()))?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > request.expected_bytes {
            return Err(LocalModelEvalError::Materialization(
                "source exceeded the pinned byte count".to_string(),
            ));
        }
        output.write_all(&chunk).await.map_err(materialization)?;
        digest.update(&chunk);
    }
    output.flush().await.map_err(materialization)?;
    output.sync_all().await.map_err(materialization)?;
    if downloaded != request.expected_bytes {
        return Err(LocalModelEvalError::Materialization(format!(
            "download ended at {downloaded} bytes instead of {}",
            request.expected_bytes
        )));
    }
    let actual = hex_digest(digest.finish().as_ref());
    if actual != request.sha256 {
        return Err(LocalModelEvalError::Materialization(
            "downloaded artifact did not match its pinned SHA-256".to_string(),
        ));
    }
    validate_gguf_magic(partial)
        .await
        .map_err(map_install_error)?;
    if cancellation.is_cancelled() {
        return Err(LocalModelEvalError::Cancelled);
    }
    fs::rename(partial, destination)
        .await
        .map_err(materialization)?;
    Ok(())
}

async fn remove_if_present(path: &Path) -> Result<(), LocalModelEvalError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(materialization(error)),
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn map_install_error(error: crate::local_models::LocalModelInstallError) -> LocalModelEvalError {
    match error {
        crate::local_models::LocalModelInstallError::Cancelled => LocalModelEvalError::Cancelled,
        error => LocalModelEvalError::Materialization(error.to_string()),
    }
}

fn materialization(error: impl std::fmt::Display) -> LocalModelEvalError {
    LocalModelEvalError::Materialization(error.to_string())
}

struct EvalFileLock(File);

impl Drop for EvalFileLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ring::digest;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;

    fn request(cache_root: PathBuf, bytes: &[u8]) -> MaterializeVerifiedEvalModelRequest {
        MaterializeVerifiedEvalModelRequest {
            source: VerifiedEvalModelSource {
                repo: "owner/repository".to_string(),
                revision: "a".repeat(40),
                file: "model.gguf".to_string(),
            },
            expected_bytes: bytes.len() as u64,
            sha256: hex_digest(digest::digest(&SHA256, bytes).as_ref()),
            cache_root,
        }
    }

    async fn serve_once(bytes: Arc<Vec<u8>>) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = vec![0_u8; 4096];
            let _ = stream.read(&mut request).await;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            );
            stream.write_all(header.as_bytes()).await.expect("header");
            stream.write_all(&bytes).await.expect("body");
        });
        Url::parse(&format!("http://{address}/model.gguf")).expect("url")
    }

    #[tokio::test]
    async fn cache_hit_returns_verified_path_without_network() {
        let directory = tempfile::tempdir().expect("cache");
        let bytes = b"GGUF cached evaluation model";
        let request = request(directory.path().to_path_buf(), bytes);
        let destination = request.cache_root.join(format!("{}.gguf", request.sha256));
        assert!(
            format!("{request:?}").contains(&request.cache_root.display().to_string()),
            "debug omitted the ordinary cache path"
        );
        fs::write(&destination, bytes).await.expect("cache fixture");

        let path = materialize_verified_eval_model(request, CancellationToken::new())
            .await
            .expect("cache hit");

        assert_eq!(path, destination);
    }

    #[tokio::test]
    async fn digest_mismatch_removes_partial_and_publishes_nothing() {
        let directory = tempfile::tempdir().expect("cache");
        let expected = b"GGUF expected evaluation model";
        let request = request(directory.path().to_path_buf(), expected);
        let destination = request.cache_root.join(format!("{}.gguf", request.sha256));
        let url = serve_once(Arc::new(b"GGUF incorrect evaluation bytes".to_vec())).await;

        let error = materialize_from_url(request, url, CancellationToken::new())
            .await
            .expect_err("checksum mismatch");

        assert!(matches!(error, LocalModelEvalError::Materialization(_)));
        assert!(!destination.exists());
        assert!(
            std::fs::read_dir(directory.path())
                .expect("cache entries")
                .all(|entry| !entry
                    .expect("cache entry")
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".partial"))
        );
    }

    #[tokio::test]
    async fn cancellation_leaves_no_partial_or_published_file() {
        let directory = tempfile::tempdir().expect("cache");
        let bytes = b"GGUF cancelled evaluation model";
        let request = request(directory.path().to_path_buf(), bytes);
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = materialize_verified_eval_model(request, cancellation)
            .await
            .expect_err("cancelled");

        assert!(matches!(error, LocalModelEvalError::Cancelled));
        assert_eq!(
            std::fs::read_dir(directory.path())
                .expect("cache entries")
                .count(),
            0
        );
    }

    #[tokio::test]
    async fn successful_download_is_atomic_and_store_free() {
        let directory = tempfile::tempdir().expect("cache");
        let bytes = Arc::new(b"GGUF successful evaluation model".to_vec());
        let request = request(directory.path().to_path_buf(), &bytes);
        let url = serve_once(Arc::clone(&bytes)).await;

        let path = materialize_from_url(request, url, CancellationToken::new())
            .await
            .expect("materialized");

        assert_eq!(fs::read(path).await.expect("published bytes"), *bytes);
        let names = std::fs::read_dir(directory.path())
            .expect("cache entries")
            .map(|entry| entry.expect("cache entry").file_name())
            .collect::<Vec<_>>();
        assert!(
            names
                .iter()
                .all(|name| !name.to_string_lossy().ends_with(".partial"))
        );
        assert!(
            names
                .iter()
                .all(|name| !name.to_string_lossy().contains("sqlite"))
        );
    }
}
