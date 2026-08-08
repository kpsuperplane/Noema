//! Typed provider-issued links, opaque cursors, and deferred auth gates.

#[cfg(test)]
mod tests;

use crate::{
    AdapterCompileError, ArgumentDefinition, ContinuationCredentialMode, PaginationPolicy,
};
use noema_home::NoemaPaths;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::PathBuf};
use thiserror::Error;
use url::Url;

const MAX_LINK_BYTES: usize = 8 * 1024;
pub(crate) const MAX_REFERENCE_BYTES: usize = 128;
pub(crate) const CONTINUATION_JSON_OVERHEAD_BYTES: usize = 320;
const MAX_POINTER_BYTES: usize = 512;
const MAX_ALLOWED_ORIGINS: usize = 16;
const MAX_TTL_SECONDS: u32 = 7 * 24 * 60 * 60;
const MAX_RETRY_AFTER_SECONDS: u64 = 24 * 60 * 60;
const MAX_CURSOR_BYTES: usize = 4 * 1024;
const MAX_CURSOR_RECORD_BYTES: u64 = 8 * 1024;
const CURSOR_SECRETS_DIR: &str = "cursor-secrets";

/// Whether a continuation requires an unproven non-personal auth binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationAuthBinding {
    /// A personal account binding that the baseline can represent.
    Personal,
    /// Delegated permissions require a second-company fixture pair.
    Delegated,
    /// Application permissions require a second-company fixture pair.
    Application,
    /// Tenant binding requires a second-company fixture pair.
    Tenant,
    /// Audience binding requires a second-company fixture pair.
    Audience,
}

/// Explicit account/auth eligibility check for a continuation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuationEligibility {
    /// Exact reviewed account surface.
    pub account_kind: String,
    /// Auth binding whose evidence gate must be met.
    pub auth_binding: ContinuationAuthBinding,
}

impl ContinuationEligibility {
    /// Reject an account or auth mode that is not qualified for this slice.
    ///
    /// # Errors
    ///
    /// Returns a typed gate error when the account surface differs or the auth
    /// binding has not crossed its independent-fixture evidence gate.
    pub fn check(&self, actual_account_kind: &str) -> Result<(), ContinuationGateError> {
        if self.account_kind != actual_account_kind {
            return Err(ContinuationGateError::AccountKindMismatch);
        }
        match self.auth_binding {
            ContinuationAuthBinding::Personal => Ok(()),
            ContinuationAuthBinding::Delegated => Err(ContinuationGateError::DelegatedUnproven),
            ContinuationAuthBinding::Application => Err(ContinuationGateError::ApplicationUnproven),
            ContinuationAuthBinding::Tenant => Err(ContinuationGateError::TenantUnproven),
            ContinuationAuthBinding::Audience => Err(ContinuationGateError::AudienceUnproven),
        }
    }
}

/// Deferred continuation eligibility failure.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum ContinuationGateError {
    /// The synthetic or real account surface differs from reviewed data.
    #[error("continuation account kind does not match")]
    AccountKindMismatch,
    /// Delegated behavior lacks two independent-company fixtures.
    #[error("delegated continuation mode is unproven")]
    DelegatedUnproven,
    /// Application behavior lacks two independent-company fixtures.
    #[error("application continuation mode is unproven")]
    ApplicationUnproven,
    /// Tenant binding lacks two independent-company fixtures.
    #[error("tenant continuation binding is unproven")]
    TenantUnproven,
    /// Audience binding lacks two independent-company fixtures.
    #[error("audience continuation binding is unproven")]
    AudienceUnproven,
}

/// A provider-issued link after exact origin, path, and size validation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedProviderLink {
    /// The exact bounded URL; it is never model-supplied.
    pub url: String,
    /// The reviewed credential mode for this link.
    pub credential_mode: ContinuationCredentialMode,
    /// The reviewed workflow kind.
    pub link_kind: crate::ProviderLinkKind,
    /// Local expiry derived from the reviewed TTL.
    pub expires_at_epoch_seconds: u64,
}

impl std::fmt::Debug for ValidatedProviderLink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidatedProviderLink")
            .field("url", &"[REDACTED]")
            .field("credential_mode", &self.credential_mode)
            .field("link_kind", &self.link_kind)
            .field("expires_at_epoch_seconds", &self.expires_at_epoch_seconds)
            .finish()
    }
}

/// Validate one provider-returned absolute URL against its reviewed policy.
///
/// # Errors
///
/// Returns [`ContinuationError`] when the URL is malformed, expired by policy,
/// or outside the reviewed origin/credential contract.
pub fn validate_provider_link(
    value: &str,
    policy: &PaginationPolicy,
    now_epoch_seconds: u64,
) -> Result<ValidatedProviderLink, ContinuationError> {
    let PaginationPolicy::ProviderLink {
        allowed_origins,
        credential_mode,
        link_kind,
        max_bytes,
        ttl_seconds,
        ..
    } = policy
    else {
        return Err(ContinuationError::PolicyMismatch);
    };
    if value.is_empty() || value.len() > MAX_LINK_BYTES || value.len() > *max_bytes as usize {
        return Err(ContinuationError::LinkInvalid);
    }
    let lower_value = value.to_ascii_lowercase();
    if lower_value.contains("%2e") || lower_value.contains("%2f") || lower_value.contains("%5c") {
        return Err(ContinuationError::LinkInvalid);
    }
    let parsed = Url::parse(value).map_err(|_| ContinuationError::LinkInvalid)?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
        || parsed.port() == Some(0)
        || parsed
            .path()
            .split('/')
            .any(|part| matches!(part, "." | ".."))
    {
        return Err(ContinuationError::LinkInvalid);
    }
    let lower_path = parsed.path().to_ascii_lowercase();
    if lower_path.contains("%2e") || lower_path.contains("%2f") || lower_path.contains("%5c") {
        return Err(ContinuationError::LinkInvalid);
    }
    let allowed = allowed_origins
        .iter()
        .map(|origin| Url::parse(origin).map_err(|_| ContinuationError::PolicyMismatch))
        .collect::<Result<Vec<_>, _>>()?;
    if !allowed
        .iter()
        .any(|origin| parsed.origin() == origin.origin())
    {
        return Err(ContinuationError::OriginNotAllowed);
    }
    Ok(ValidatedProviderLink {
        url: value.to_string(),
        credential_mode: *credential_mode,
        link_kind: *link_kind,
        expires_at_epoch_seconds: now_epoch_seconds.saturating_add(u64::from(*ttl_seconds)),
    })
}

/// Typed continuation validation failure.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum ContinuationError {
    /// The link policy was not a provider-link policy.
    #[error("continuation policy does not describe a provider link")]
    PolicyMismatch,
    /// The provider-returned URL is malformed or exceeds bounds.
    #[error("provider continuation link is invalid")]
    LinkInvalid,
    /// The URL origin is outside the reviewed set.
    #[error("provider continuation link origin is not allowed")]
    OriginNotAllowed,
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
    /// Exact reviewed account surface.
    pub account_kind: String,
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
        && valid_reference(&binding.account_kind)
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
        PaginationPolicy::ProviderLink {
            response_pointer,
            request_argument,
            allowed_origins,
            max_bytes,
            ttl_seconds,
            ..
        } => {
            if !pointer(response_pointer)
                || request_argument
                    .as_deref()
                    .is_some_and(|argument| !runtime_argument(argument))
                || allowed_origins.is_empty()
                || allowed_origins.len() > MAX_ALLOWED_ORIGINS
                || *max_bytes == 0
                || *max_bytes as usize > MAX_LINK_BYTES
                || *ttl_seconds == 0
                || *ttl_seconds > MAX_TTL_SECONDS
            {
                return Err(AdapterCompileError::Invalid("pagination"));
            }
            let mut origins = BTreeSet::new();
            for origin in allowed_origins {
                if !valid_origin(origin) || !origins.insert(origin) {
                    return Err(AdapterCompileError::Invalid("pagination_origin"));
                }
            }
            Ok(())
        }
        PaginationPolicy::DeltaCursor {
            response_pointer,
            request_argument,
            baseline_operation,
            max_age_seconds,
        } => {
            if pointer(response_pointer)
                && runtime_argument(request_argument)
                && valid_id(baseline_operation)
                && *max_age_seconds > 0
                && *max_age_seconds <= MAX_TTL_SECONDS
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

fn valid_origin(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some()
        && url.path() == "/"
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.port() != Some(0)
}
