use std::{fmt, fs, io, path::PathBuf};

use crate::adapters::account_service::filesystem::{
    FileSnapshot, atomic_write_private, restore_file, snapshot_file,
};
use crate::{CODEX_PROVIDER, CodexOAuthTokens, ProviderError};

use super::{claims::token_needs_refresh, client::CodexOAuthClient};

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

    /// Resolve a usable access token, refreshing when expiry is near.
    ///
    /// # Errors
    ///
    /// Returns provider/auth errors when credentials are missing or refresh
    /// fails.
    pub async fn access_token(
        &self,
        client: &CodexOAuthClient,
        refresh_skew_seconds: u64,
    ) -> Result<String, ProviderError> {
        let tokens = self.read()?;
        if !token_needs_refresh(&tokens, refresh_skew_seconds) {
            return Ok(tokens.access_token);
        }
        let refreshed = client.refresh_tokens(&tokens.refresh_token).await?;
        self.write(&refreshed)?;
        Ok(refreshed.access_token)
    }

    /// Force-refresh tokens and persist the result.
    ///
    /// # Errors
    ///
    /// Returns provider/auth errors when credentials are missing or refresh
    /// fails.
    pub async fn refresh_access_token(
        &self,
        client: &CodexOAuthClient,
    ) -> Result<String, ProviderError> {
        let tokens = self.read()?;
        let refreshed = client.refresh_tokens(&tokens.refresh_token).await?;
        self.write(&refreshed)?;
        Ok(refreshed.access_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_snapshot_restores_exact_prior_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = CodexTokenStore::new(dir.path().join("providers/codex/default"));
        atomic_write_private(&store.token_path(), b"{malformed-token}\0")
            .expect("write malformed prior file");
        let snapshot = store.snapshot().expect("snapshot");
        store
            .write(&CodexOAuthTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: 1,
            })
            .expect("replace");

        store.restore(&snapshot).expect("restore");

        assert_eq!(
            fs::read(store.token_path()).expect("read restored"),
            b"{malformed-token}\0"
        );
    }

    #[cfg(unix)]
    #[test]
    fn written_tokens_have_private_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let store = CodexTokenStore::new(dir.path().join("providers/codex/default"));
        store
            .write(&CodexOAuthTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: 1,
            })
            .expect("write");

        let account_home_mode = fs::metadata(&store.account_home)
            .expect("account metadata")
            .permissions()
            .mode()
            & 0o777;
        let token_file_mode = fs::metadata(store.token_path())
            .expect("token metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(account_home_mode, 0o700);
        assert_eq!(token_file_mode, 0o600);
    }
}
