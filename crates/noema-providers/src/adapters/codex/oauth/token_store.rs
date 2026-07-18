use std::{fmt, fs, io, path::PathBuf};

use crate::adapters::account_service::filesystem::{
    FileSnapshot, atomic_write_private, restore_file, snapshot_file,
};
use crate::{CODEX_PROVIDER, CodexOAuthTokens, ProviderError};

use super::claims::token_needs_refresh;

const TOKEN_FILE_NAME: &str = "codex_tokens.json";

/// Filesystem store for Noema-owned Codex OAuth tokens.
#[derive(Clone, PartialEq, Eq)]
pub struct CodexTokenStore {
    account_home: PathBuf,
}

impl fmt::Debug for CodexTokenStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexTokenStore")
            .field("account_home", &"[REDACTED]")
            .finish()
    }
}

impl CodexTokenStore {
    /// Build a token store under the provider account home.
    #[must_use]
    pub fn new(account_home: impl Into<PathBuf>) -> Self {
        Self {
            account_home: account_home.into(),
        }
    }

    /// Return the token file path.
    #[must_use]
    pub fn token_path(&self) -> PathBuf {
        self.account_home.join(TOKEN_FILE_NAME)
    }

    /// Return true when a Noema-owned token file exists and has required fields.
    #[must_use]
    pub fn has_usable_tokens(&self) -> bool {
        self.read().is_ok_and(|tokens| tokens.has_required_fields())
    }

    /// Read Codex OAuth tokens.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::MissingCredentials`] when tokens are absent or
    /// incomplete, and [`ProviderError::MalformedResponse`] for invalid JSON.
    pub fn read(&self) -> Result<CodexOAuthTokens, ProviderError> {
        let token_path = self.token_path();
        let text = fs::read_to_string(&token_path).map_err(|source| match source.kind() {
            io::ErrorKind::NotFound => ProviderError::MissingCredentials {
                provider: CODEX_PROVIDER.to_string(),
                credential: TOKEN_FILE_NAME.to_string(),
            },
            _ => ProviderError::ProviderUnavailable {
                provider: CODEX_PROVIDER.to_string(),
                message: format!("failed to read Codex token file: {source}"),
            },
        })?;
        let tokens: CodexOAuthTokens =
            serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
                message: format!("failed to parse Codex token file: {source}"),
            })?;
        if !tokens.has_required_fields() {
            return Err(ProviderError::MissingCredentials {
                provider: CODEX_PROVIDER.to_string(),
                credential: "access_token and refresh_token".to_string(),
            });
        }
        Ok(tokens)
    }

    /// Write Codex OAuth tokens atomically enough for a single local daemon.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::ProviderUnavailable`] if the account home or
    /// token file cannot be written.
    pub fn write(&self, tokens: &CodexOAuthTokens) -> Result<(), ProviderError> {
        let bytes = serde_json::to_vec_pretty(tokens).map_err(|source| {
            ProviderError::MalformedResponse {
                message: format!("failed to serialize Codex tokens: {source}"),
            }
        })?;
        atomic_write_private(&self.token_path(), &bytes).map_err(|source| {
            ProviderError::ProviderUnavailable {
                provider: CODEX_PROVIDER.to_string(),
                message: format!("failed to write Codex token file: {source}"),
            }
        })
    }

    pub(crate) fn snapshot(&self) -> Result<FileSnapshot, ProviderError> {
        snapshot_file(&self.token_path()).map_err(|source| ProviderError::ProviderUnavailable {
            provider: CODEX_PROVIDER.to_string(),
            message: format!("failed to snapshot Codex token file: {source}"),
        })
    }

    pub(crate) fn restore(&self, snapshot: &FileSnapshot) -> Result<(), ProviderError> {
        restore_file(&self.token_path(), snapshot).map_err(|source| {
            ProviderError::ProviderUnavailable {
                provider: CODEX_PROVIDER.to_string(),
                message: format!("failed to restore Codex token file: {source}"),
            }
        })
    }

    pub(crate) fn tokens_need_refresh(
        tokens: &CodexOAuthTokens,
        refresh_skew_seconds: u64,
    ) -> bool {
        token_needs_refresh(tokens, refresh_skew_seconds)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn written_tokens_have_private_unix_permissions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = CodexTokenStore::new(dir.path().join("providers/codex/default"));
        store
            .write(&CodexOAuthTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: 1,
            })
            .expect("write");

        let token_path = store.token_path();
        for (label, path, expected) in [
            ("account home", store.account_home.as_path(), 0o700),
            ("token file", token_path.as_path(), 0o600),
        ] {
            assert_eq!(
                fs::metadata(path).expect(label).permissions().mode() & 0o777,
                expected,
                "{label} permissions"
            );
        }
    }
}
