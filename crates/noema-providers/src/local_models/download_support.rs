//! Validation and hashing helpers for local-model installation.

use std::path::PathBuf;

use ring::digest::{Context as DigestContext, SHA256};
use tokio::{fs, io::AsyncReadExt};
use tokio_util::sync::CancellationToken;
use url::Url;

use super::download::LocalModelInstallError;
use crate::LocalFileModelImport;

#[derive(Debug)]
pub(super) struct InstalledArtifact {
    pub(super) sha256: String,
    pub(super) downloaded_bytes: u64,
    pub(super) expected_bytes: Option<u64>,
    pub(super) blob_relative_path: String,
}

pub(super) async fn hash_file(
    path: &PathBuf,
    cancellation: &CancellationToken,
) -> Result<String, LocalModelInstallError> {
    let mut file = fs::File::open(path).await?;
    let mut digest = DigestContext::new(&SHA256);
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = tokio::select! {
            () = cancellation.cancelled() => return Err(LocalModelInstallError::Cancelled),
            read = file.read(&mut buffer) => read?,
        };
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex_digest(digest.finish().as_ref()))
}

pub(super) fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

pub(super) fn blob_relative_path(sha256: &str) -> String {
    format!("models/blobs/{sha256}.gguf")
}

pub(super) fn hugging_face_url(
    repo: &str,
    revision: &str,
    file: &str,
) -> Result<Url, LocalModelInstallError> {
    validate_revision(revision)?;
    let mut repo_parts = repo.split('/');
    let owner = repo_parts.next().unwrap_or_default();
    let name = repo_parts.next().unwrap_or_default();
    if owner.is_empty() || name.is_empty() || repo_parts.next().is_some() {
        return Err(LocalModelInstallError::InvalidInput(
            "Hugging Face repository must use owner/repository form".to_string(),
        ));
    }
    let file_parts = file.split('/').collect::<Vec<_>>();
    if file_parts.is_empty()
        || file_parts
            .iter()
            .any(|part| part.is_empty() || matches!(*part, "." | ".."))
        || !file.to_ascii_lowercase().ends_with(".gguf")
    {
        return Err(LocalModelInstallError::InvalidInput(
            "Hugging Face artifact must be a safe .gguf path".to_string(),
        ));
    }
    let mut url = Url::parse("https://huggingface.co/")
        .map_err(|error| LocalModelInstallError::InvalidInput(error.to_string()))?;
    url.path_segments_mut()
        .map_err(|()| LocalModelInstallError::InvalidInput("invalid Hugging Face URL".to_string()))?
        .extend([owner, name, "resolve", revision])
        .extend(file_parts);
    url.query_pairs_mut().append_pair("download", "true");
    Ok(url)
}

pub(super) fn validate_revision(revision: &str) -> Result<(), LocalModelInstallError> {
    if revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(LocalModelInstallError::InvalidInput(
            "Hugging Face revision must be an immutable 40-character commit".to_string(),
        ))
    }
}

pub(super) fn validate_digest(sha256: &str) -> Result<(), LocalModelInstallError> {
    if sha256.len() == 64
        && sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(LocalModelInstallError::InvalidInput(
            "SHA-256 must be 64 lowercase hexadecimal characters".to_string(),
        ))
    }
}

pub(super) fn normalized_model_id(model_id: &str) -> Result<String, LocalModelInstallError> {
    let value = required_text(model_id, "model id")?.to_ascii_lowercase();
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        Ok(value)
    } else {
        Err(LocalModelInstallError::InvalidInput(
            "model id contains unsupported characters".to_string(),
        ))
    }
}

pub(super) fn required_text<'a>(
    value: &'a str,
    field: &str,
) -> Result<&'a str, LocalModelInstallError> {
    let value = value.trim();
    if value.is_empty() {
        Err(LocalModelInstallError::InvalidInput(format!(
            "{field} is required"
        )))
    } else {
        Ok(value)
    }
}

pub(super) fn normalized_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

pub(super) fn local_file_installation_id(
    input: &LocalFileModelImport,
    metadata: &std::fs::Metadata,
) -> String {
    let mut digest = DigestContext::new(&SHA256);
    digest.update(input.path.to_string_lossy().as_bytes());
    digest.update(&metadata.len().to_le_bytes());
    if let Ok(modified) = metadata.modified()
        && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
    {
        digest.update(&duration.as_nanos().to_le_bytes());
    }
    let digest = hex_digest(digest.finish().as_ref());
    format!("local_model_installation:local_file:{}", &digest[..12])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hugging_face_urls_require_pinned_safe_gguf_paths() {
        let revision = "a".repeat(40);
        let url =
            hugging_face_url("owner/repo", &revision, "weights/model.gguf").expect("valid URL");
        assert_eq!(url.host_str(), Some("huggingface.co"));
        assert!(
            url.path()
                .ends_with("/resolve/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/weights/model.gguf")
        );
        assert!(hugging_face_url("owner/repo", "main", "model.gguf").is_err());
        assert!(hugging_face_url("owner/repo", &revision, "../model.gguf").is_err());
    }
}
