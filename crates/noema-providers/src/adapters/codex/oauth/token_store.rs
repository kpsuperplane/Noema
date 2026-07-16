use std::{fmt, fs, io, path::PathBuf};

use crate::adapters::auth::ensure_provider_account_home;
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
        ensure_provider_account_home(&self.account_home).map_err(|source| {
            ProviderError::ProviderUnavailable {
                provider: CODEX_PROVIDER.to_string(),
                message: format!("failed to prepare Codex token directory: {source}"),
            }
        })?;
        let text = serde_json::to_string_pretty(tokens).map_err(|source| {
            ProviderError::MalformedResponse {
                message: format!("failed to serialize Codex tokens: {source}"),
            }
        })?;
        fs::write(self.token_path(), text).map_err(|source| ProviderError::ProviderUnavailable {
            provider: CODEX_PROVIDER.to_string(),
            message: format!("failed to write Codex token file: {source}"),
        })
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
