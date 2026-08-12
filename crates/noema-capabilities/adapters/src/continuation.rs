//! Durable opaque continuation cursors.

#[cfg(test)]
mod tests;

use crate::{AdapterCompileError, ArgumentDefinition, PaginationPolicy};
use noema_home::NoemaPaths;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use thiserror::Error;

pub(crate) const MAX_REFERENCE_BYTES: usize = 128;
pub(crate) const CONTINUATION_JSON_OVERHEAD_BYTES: usize = 320;
const MAX_POINTER_BYTES: usize = 512;
const MAX_RETRY_AFTER_SECONDS: u64 = 24 * 60 * 60;
const MAX_CURSOR_BYTES: usize = 4 * 1024;
const MAX_CURSOR_RECORD_BYTES: u64 = 8 * 1024;
const CURSOR_SECRETS_DIR: &str = "cursor-secrets";

/// Typed continuation validation failure.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum ContinuationError {
    /// A retry-after value is malformed or outside the bounded delay.
    #[error("retry-after value is invalid")]
    RetryAfterInvalid,
}

/// Parse the bounded seconds form of HTTP `Retry-After`.
///
/// # Errors
///
/// Returns [`ContinuationError::RetryAfterInvalid`] for malformed or oversized
/// delays.
pub fn parse_retry_after(value: &str) -> Result<u64, ContinuationError> {
    let seconds = value
        .trim()
        .parse::<u64>()
        .map_err(|_| ContinuationError::RetryAfterInvalid)?;
    (seconds <= MAX_RETRY_AFTER_SECONDS)
        .then_some(seconds)
        .ok_or(ContinuationError::RetryAfterInvalid)
}

/// Non-secret identity binding for an opaque cursor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CursorBinding {
    /// Exact filesystem connection identity.
    pub connection_id: String,
    /// Exact definition semantic digest.
    pub semantic_digest: String,
    /// Exact operation identity.
    pub operation_id: String,
    /// Exact reusable grant identity, when OAuth authorizes the operation.
    pub grant_id: Option<String>,
    /// Stable external account identity, when the reviewed provider exposes it.
    pub account_id: Option<String>,
    /// Provider grant revision captured at issue time.
    pub grant_revision: u64,
    /// SHA-256 of the original model-controlled arguments.
    pub arguments_sha256: String,
}

/// Secret-free cursor reference retained in durable metadata/checkpoints.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CursorHandle {
    /// Opaque reference into the private cursor secret store.
    pub secret_reference: String,
    /// Exact authority binding.
    pub binding: CursorBinding,
    /// Cursor expiration time.
    pub expires_at_epoch_seconds: u64,
}

/// A secret-bearing cursor value with redacted formatting.
#[derive(Clone, PartialEq, Eq)]
pub struct CursorSecret(String);

impl CursorSecret {
    /// Borrow the secret only at the transport boundary.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for CursorSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CursorSecret([REDACTED])")
    }
}

/// Filesystem-owned bearer-like cursor material keyed by a secret-free handle.
#[derive(Debug, Clone)]
pub struct DurableCursorStore {
    paths: NoemaPaths,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DurableCursorRecord {
    handle: CursorHandle,
    token: String,
}

/// Filesystem failure while reading or publishing cursor material.
#[derive(Debug, Error)]
pub enum DurableCursorError {
    /// Filesystem operation failed.
    #[error("durable cursor filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// A private cursor invariant failed.
    #[error("durable cursor filesystem invariant failed: {0}")]
    Integrity(&'static str),
    /// Cursor authority did not match.
    #[error("durable cursor authority does not match")]
    BindingMismatch,
    /// Cursor is expired or requires a baseline.
    #[error("durable cursor is expired")]
    Expired,
}

impl From<crate::private_fs::PrivateFsError> for DurableCursorError {
    fn from(error: crate::private_fs::PrivateFsError) -> Self {
        match error {
            crate::private_fs::PrivateFsError::Io(error) => Self::Io(error),
            crate::private_fs::PrivateFsError::Integrity(code) => Self::Integrity(code),
        }
    }
}

impl DurableCursorStore {
    /// Create a durable cursor store under one Noema home.
    #[must_use]
    pub const fn new(paths: NoemaPaths) -> Self {
        Self { paths }
    }

    /// Publish cursor bytes behind an already validated handle.
    ///
    /// # Errors
    ///
    /// Returns DurableCursorError for invalid bindings, oversized bytes, or a
    /// conflicting immutable secret reference.
    pub fn put(&self, handle: &CursorHandle, token: &str) -> Result<(), DurableCursorError> {
        if !valid_cursor_handle(handle) {
            return Err(DurableCursorError::Integrity("cursor_binding"));
        }
        if token.is_empty() || token.len() > MAX_CURSOR_BYTES {
            return Err(DurableCursorError::Integrity("cursor_oversized"));
        }
        let root = self.prepare_root()?;
        let path = root.join(&handle.secret_reference);
        let bytes = crate::digest::canonical_json_bytes(
            &serde_json::to_value(DurableCursorRecord {
                handle: handle.clone(),
                token: token.to_string(),
            })
            .map_err(|_| DurableCursorError::Integrity("cursor_record"))?,
        )
        .map_err(|_| DurableCursorError::Integrity("cursor_record"))?;
        if bytes.len() as u64 > MAX_CURSOR_RECORD_BYTES {
            return Err(DurableCursorError::Integrity("cursor_oversized"));
        }
        if path.exists() {
            let existing =
                crate::private_fs::read_bounded_regular_file(&path, MAX_CURSOR_RECORD_BYTES)?;
            if existing != bytes {
                return Err(DurableCursorError::Integrity("cursor_conflict"));
            }
            return Ok(());
        }
        crate::private_fs::write_new_file(&path, &bytes)?;
        crate::private_fs::sync_directory(&root)?;
        Ok(())
    }

    /// Resolve secret bytes only for the exact current authority and expiry.
    ///
    /// # Errors
    ///
    /// Returns DurableCursorError when the handle, binding, or expiry is stale.
    pub fn resolve(
        &self,
        secret_reference: &str,
        expected: &CursorBinding,
        now_epoch_seconds: u64,
    ) -> Result<(CursorHandle, CursorSecret), DurableCursorError> {
        if !valid_reference(secret_reference) {
            return Err(DurableCursorError::Integrity("cursor_reference"));
        }
        let path = self.prepare_root()?.join(secret_reference);
        let bytes = crate::private_fs::read_bounded_regular_file(&path, MAX_CURSOR_RECORD_BYTES)?;
        let record: DurableCursorRecord = serde_json::from_slice(&bytes)
            .map_err(|_| DurableCursorError::Integrity("cursor_record"))?;
        if !valid_cursor_handle(&record.handle) {
            return Err(DurableCursorError::Integrity("cursor_binding"));
        }
        if record.handle.secret_reference != secret_reference || record.handle.binding != *expected
        {
            return Err(DurableCursorError::BindingMismatch);
        }
        if now_epoch_seconds >= record.handle.expires_at_epoch_seconds {
            return Err(DurableCursorError::Expired);
        }
        if record.token.is_empty() || record.token.len() > MAX_CURSOR_BYTES {
            return Err(DurableCursorError::Integrity("cursor_oversized"));
        }
        Ok((record.handle, CursorSecret(record.token)))
    }

    /// Retire a superseded cursor secret after a replacement is durable.
    ///
    /// # Errors
    ///
    /// Returns DurableCursorError when the reference is invalid or cannot be
    /// removed safely.
    pub fn retire(&self, secret_reference: &str) -> Result<(), DurableCursorError> {
        if !valid_reference(secret_reference) {
            return Err(DurableCursorError::Integrity("cursor_reference"));
        }
        let root = self.prepare_root()?;
        let path = root.join(secret_reference);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(DurableCursorError::Integrity("cursor_file"));
        }
        fs::remove_file(path)?;
        crate::private_fs::sync_directory(&root)?;
        Ok(())
    }

    /// Quarantine all cursor secrets created before the connection cutover.
    pub(crate) fn quarantine_for_connection_cutover(&self) -> Result<(), DurableCursorError> {
        let root = self.prepare_root()?;
        let mut entries = fs::read_dir(&root)?.collect::<Result<Vec<_>, _>>()?;
        if entries.is_empty() {
            return Ok(());
        }
        if entries.len() > 4_096 {
            return Err(DurableCursorError::Integrity("cursor_root_oversized"));
        }
        entries.sort_by_key(fs::DirEntry::file_name);
        let quarantine_root = self
            .paths
            .adapter_quarantine_dir()
            .join("legacy-cursor-secrets");
        crate::private_fs::create_private_dir(&self.paths.adapter_quarantine_dir())?;
        crate::private_fs::create_private_dir(&quarantine_root)?;
        for entry in entries {
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(DurableCursorError::Integrity("cursor_file"));
            }
            let target = quarantine_root.join(entry.file_name());
            if target.exists() {
                return Err(DurableCursorError::Integrity("cursor_quarantine_conflict"));
            }
            fs::rename(entry.path(), target)?;
        }
        crate::private_fs::sync_directory(&root)?;
        crate::private_fs::sync_directory(&quarantine_root)?;
        Ok(())
    }

    fn prepare_root(&self) -> Result<PathBuf, DurableCursorError> {
        let adapters = self.paths.adapters_dir();
        crate::private_fs::create_private_dir(&adapters)?;
        let root = adapters.join(CURSOR_SECRETS_DIR);
        crate::private_fs::create_private_dir(&root)?;
        Ok(root)
    }
}

fn valid_cursor_handle(handle: &CursorHandle) -> bool {
    valid_reference(&handle.secret_reference)
        && valid_cursor_binding(&handle.binding)
        && handle.expires_at_epoch_seconds != 0
}

fn valid_cursor_binding(binding: &CursorBinding) -> bool {
    valid_reference(&binding.connection_id)
        && binding.semantic_digest.len() == 64
        && binding
            .semantic_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        && valid_id(&binding.operation_id)
        && binding.grant_id.as_deref().is_none_or(valid_reference)
        && binding.account_id.as_deref().is_none_or(valid_reference)
        && binding.arguments_sha256.len() == 64
        && binding
            .arguments_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// Validate typed pagination metadata and keep runtime arguments out of input schemas.
pub(crate) fn validate_pagination(
    policy: &PaginationPolicy,
    arguments: &[ArgumentDefinition],
) -> Result<(), AdapterCompileError> {
    let runtime_argument =
        |name: &str| valid_id(name) && !arguments.iter().any(|argument| argument.name == name);
    let pointer = |value: &str| {
        !value.is_empty()
            && value.len() <= MAX_POINTER_BYTES
            && value.starts_with('/')
            && value.bytes().all(|byte| !byte.is_ascii_control())
    };
    match policy {
        PaginationPolicy::None => Ok(()),
        PaginationPolicy::ResponseToken {
            response_pointer,
            request_argument,
            page_size,
        } => {
            if pointer(response_pointer)
                && runtime_argument(request_argument)
                && page_size.as_ref().is_none_or(|page_size| {
                    page_size.value > 0
                        && page_size.value <= 1_000
                        && page_size.request_argument != *request_argument
                        && runtime_argument(&page_size.request_argument)
                })
            {
                Ok(())
            } else {
                Err(AdapterCompileError::Invalid("pagination"))
            }
        }
    }
}

fn valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REFERENCE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}
