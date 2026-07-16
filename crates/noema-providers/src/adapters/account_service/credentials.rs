//! Pathless, serialized access to provider account credentials.

use std::{fmt, future::Future, pin::Pin, sync::Arc};

use noema_home::NoemaPaths;

use crate::adapters::{
    account_files::SecretInputStore,
    codex::oauth::{CodexOAuthClient, CodexTokenStore},
};
use crate::{
    CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, ProviderAccountPersistenceHandle,
    ProviderAccountRecord, ProviderAuthMethod, ProviderError,
};

use super::gates::AccountGateRegistry;

const EXA_PROVIDER: &str = "exa";
const CODEX_PROVIDER: &str = "codex";

/// Future returned by pathless credential-access operations.
pub type ProviderCredentialFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ProviderCredential, ProviderError>> + Send + 'a>>;

/// Credential value whose debug representation never exposes its contents.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderCredential(String);

impl ProviderCredential {
    /// Borrow the credential for immediate provider use.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the credential.
    #[must_use]
    pub fn into_secret(self) -> String {
        self.0
    }
}

impl fmt::Debug for ProviderCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderCredential([REDACTED])")
    }
}

impl From<String> for ProviderCredential {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Runtime-facing provider credential access without filesystem paths.
pub trait ProviderCredentialAccess: Send + Sync {
    /// Load one Exa API key from a durable provider account id.
    fn exa_api_key<'a>(&'a self, provider_account_id: &'a str) -> ProviderCredentialFuture<'a>;

    /// Load a usable Codex access token, refreshing near expiry.
    fn codex_access_token<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a>;

    /// Force one Codex access-token refresh.
    fn refresh_codex_access_token<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a>;
}

/// Shared runtime-facing credential access handle.
pub type ProviderCredentialAccessHandle = Arc<dyn ProviderCredentialAccess>;

/// Provider-owned credential access backed by Noema paths and durable accounts.
#[derive(Clone)]
pub struct ProviderCredentialAccessService {
    paths: NoemaPaths,
    persistence: ProviderAccountPersistenceHandle,
    gates: AccountGateRegistry,
    codex_oauth: CodexOAuthClient,
}

impl fmt::Debug for ProviderCredentialAccessService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderCredentialAccessService")
            .field("paths", &"[REDACTED]")
            .field("persistence", &"[CONFIGURED]")
            .field("gates", &self.gates)
            .field("codex_oauth", &self.codex_oauth)
            .finish()
    }
}

impl ProviderCredentialAccessService {
    /// Build pathless credential access sharing account-service gates.
    #[must_use]
    pub fn new(
        paths: NoemaPaths,
        persistence: ProviderAccountPersistenceHandle,
        gates: AccountGateRegistry,
        codex_oauth: CodexOAuthClient,
    ) -> Self {
        Self {
            paths,
            persistence,
            gates,
            codex_oauth,
        }
    }

    async fn exa_api_key_value(
        &self,
        provider_account_id: &str,
    ) -> Result<ProviderCredential, ProviderError> {
        let gate = self.gates.gate(provider_account_id);
        let _guard = gate.lock().await;
        let identity = self
            .load_account_identity(provider_account_id, EXA_PROVIDER)
            .await?;
        let store = SecretInputStore::new(self.account_home(&identity));
        let secret = store.load_api_key()?;
        self.revalidate_account(&identity).await?;
        Ok(secret.into())
    }

    async fn codex_access_token_value(
        &self,
        provider_account_id: &str,
        force_refresh: bool,
    ) -> Result<ProviderCredential, ProviderError> {
        let gate = self.gates.gate(provider_account_id);
        let (identity, original_tokens) = {
            let _guard = gate.lock().await;
            let identity = self
                .load_account_identity(provider_account_id, CODEX_PROVIDER)
                .await?;
            let store = CodexTokenStore::new(self.account_home(&identity));
            let tokens = store.read()?;
            self.revalidate_account(&identity).await?;
            if !force_refresh
                && !CodexTokenStore::tokens_need_refresh(
                    &tokens,
                    CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS,
                )
            {
                return Ok(tokens.access_token.into());
            }
            (identity, tokens)
        };

        // OAuth HTTP must not hold the account gate. Publication is guarded and
        // revalidates both durable identity and the token version afterward.
        let refreshed = self
            .codex_oauth
            .refresh_tokens(&original_tokens.refresh_token)
            .await?;

        let _guard = gate.lock().await;
        self.revalidate_account(&identity).await?;
        let store = CodexTokenStore::new(self.account_home(&identity));
        let current_tokens = store.read()?;
        if current_tokens != original_tokens {
            if !CodexTokenStore::tokens_need_refresh(
                &current_tokens,
                CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS,
            ) {
                return Ok(current_tokens.access_token.into());
            }
            return Err(ProviderError::ProviderUnavailable {
                provider: CODEX_PROVIDER.to_string(),
                message: "Codex credentials changed during token refresh".to_string(),
            });
        }
        store.write(&refreshed)?;
        Ok(refreshed.access_token.into())
    }

    async fn load_account_identity(
        &self,
        provider_account_id: &str,
        expected_provider: &str,
    ) -> Result<AccountIdentity, ProviderError> {
        let account = self
            .persistence
            .provider_account(provider_account_id)
            .await
            .map_err(|_| credential_state_unavailable(expected_provider))?
            .ok_or_else(|| missing_account_credentials(expected_provider))?;
        AccountIdentity::from_account(account, provider_account_id, expected_provider)
    }

    async fn revalidate_account(&self, expected: &AccountIdentity) -> Result<(), ProviderError> {
        let current = self
            .load_account_identity(&expected.provider_account_id, &expected.provider_kind)
            .await?;
        if current == *expected {
            Ok(())
        } else {
            Err(ProviderError::ProviderUnavailable {
                provider: expected.provider_kind.clone(),
                message: "provider account identity changed during credential access".to_string(),
            })
        }
    }

    fn account_home(&self, identity: &AccountIdentity) -> std::path::PathBuf {
        self.paths
            .provider_account_home(&identity.provider_kind, &identity.account_key)
    }
}

impl ProviderCredentialAccess for ProviderCredentialAccessService {
    fn exa_api_key<'a>(&'a self, provider_account_id: &'a str) -> ProviderCredentialFuture<'a> {
        Box::pin(self.exa_api_key_value(provider_account_id))
    }

    fn codex_access_token<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a> {
        Box::pin(self.codex_access_token_value(provider_account_id, false))
    }

    fn refresh_codex_access_token<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a> {
        Box::pin(self.codex_access_token_value(provider_account_id, true))
    }
}

#[derive(Clone, PartialEq, Eq)]
struct AccountIdentity {
    provider_account_id: String,
    provider_kind: String,
    account_key: String,
}

impl AccountIdentity {
    fn from_account(
        account: ProviderAccountRecord,
        requested_id: &str,
        expected_provider: &str,
    ) -> Result<Self, ProviderError> {
        if account.provider_account_id != requested_id
            || account.provider_kind != expected_provider
            || account.account_key.trim().is_empty()
            || account.auth_method != expected_auth_method(expected_provider)
        {
            return Err(ProviderError::InvalidRequest {
                message: format!(
                    "provider account is not a valid {expected_provider} credential account"
                ),
            });
        }
        if !account.is_active {
            return Err(ProviderError::ProviderUnavailable {
                provider: expected_provider.to_string(),
                message: "provider account is inactive".to_string(),
            });
        }
        Ok(Self {
            provider_account_id: account.provider_account_id,
            provider_kind: account.provider_kind,
            account_key: account.account_key,
        })
    }
}

fn expected_auth_method(provider: &str) -> ProviderAuthMethod {
    match provider {
        CODEX_PROVIDER => ProviderAuthMethod::OauthDeviceCode,
        _ => ProviderAuthMethod::SecretInput,
    }
}

fn missing_account_credentials(provider: &str) -> ProviderError {
    ProviderError::MissingCredentials {
        provider: provider.to_string(),
        credential: "provider account".to_string(),
    }
}

fn credential_state_unavailable(provider: &str) -> ProviderError {
    ProviderError::ProviderUnavailable {
        provider: provider.to_string(),
        message: "provider account state is unavailable".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;
    use crate::{
        CodexOAuthConfig, CodexOAuthTokens, NewProviderAccount, ProviderAccountStatus,
        ProviderPersistenceError, ProviderPersistenceFuture, UpdateProviderAccountRequest,
        capabilities_for_provider_account,
    };

    #[derive(Default)]
    struct FakeAccountPersistence {
        account: Mutex<Option<ProviderAccountRecord>>,
        reads: AtomicUsize,
        delete_on_read: Option<(usize, PathBuf)>,
    }

    impl FakeAccountPersistence {
        fn with_account(account: ProviderAccountRecord) -> Self {
            Self {
                account: Mutex::new(Some(account)),
                reads: AtomicUsize::new(0),
                delete_on_read: None,
            }
        }

        fn deleting_account_on_read(
            account: ProviderAccountRecord,
            read_number: usize,
            account_home: PathBuf,
        ) -> Self {
            Self {
                account: Mutex::new(Some(account)),
                reads: AtomicUsize::new(0),
                delete_on_read: Some((read_number, account_home)),
            }
        }
    }

    impl crate::ProviderAccountPersistence for FakeAccountPersistence {
        fn provider_account<'a>(
            &'a self,
            provider_account_id: &'a str,
        ) -> ProviderPersistenceFuture<'a, Option<ProviderAccountRecord>> {
            let read_number = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
            let mut account = self.account.lock().expect("fake account lock");
            if let Some((delete_on_read, account_home)) = self.delete_on_read.as_ref()
                && read_number == *delete_on_read
            {
                fs::remove_dir_all(account_home).expect("remove account home");
                *account = None;
            }
            let account = account
                .clone()
                .filter(|account| account.provider_account_id == provider_account_id);
            Box::pin(async move { Ok(account) })
        }

        fn active_provider_account<'a>(
            &'a self,
            _provider_kind: &'a str,
        ) -> ProviderPersistenceFuture<'a, Option<ProviderAccountRecord>> {
            unsupported("active_provider_account")
        }

        fn active_default_provider_accounts(
            &self,
        ) -> ProviderPersistenceFuture<'_, Vec<ProviderAccountRecord>> {
            unsupported("active_default_provider_accounts")
        }

        fn active_provider_accounts(
            &self,
        ) -> ProviderPersistenceFuture<'_, Vec<ProviderAccountRecord>> {
            unsupported("active_provider_accounts")
        }

        fn provider_accounts(&self) -> ProviderPersistenceFuture<'_, Vec<ProviderAccountRecord>> {
            unsupported("provider_accounts")
        }

        fn create_provider_account(
            &self,
            _request: NewProviderAccount,
        ) -> ProviderPersistenceFuture<'_, ProviderAccountRecord> {
            unsupported("create_provider_account")
        }

        fn update_provider_account(
            &self,
            _request: UpdateProviderAccountRequest,
        ) -> ProviderPersistenceFuture<'_, ProviderAccountRecord> {
            unsupported("update_provider_account")
        }

        fn delete_provider_account<'a>(
            &'a self,
            _provider_account_id: &'a str,
        ) -> ProviderPersistenceFuture<'a, bool> {
            unsupported("delete_provider_account")
        }
    }

    fn unsupported<T>(operation: &'static str) -> ProviderPersistenceFuture<'static, T> {
        Box::pin(async move { Err(ProviderPersistenceError::Persistence { operation }) })
    }

    #[test]
    fn provider_credential_debug_redacts_secret() {
        let credential = ProviderCredential::from("credential-secret".to_string());

        let debug = format!("{credential:?}");

        assert!(!debug.contains("credential-secret"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn exa_access_resolves_path_from_durable_identity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let account = account("exa", "team");
        SecretInputStore::new(paths.provider_account_home("exa", "team"))
            .save_api_key("exa-secret")
            .expect("save secret");
        let persistence: ProviderAccountPersistenceHandle =
            Arc::new(FakeAccountPersistence::with_account(account.clone()));
        let service = ProviderCredentialAccessService::new(
            paths,
            persistence,
            AccountGateRegistry::new(),
            CodexOAuthClient::new(crate::CodexOAuthConfig::default()).expect("oauth"),
        );

        let credential = service
            .exa_api_key(&account.provider_account_id)
            .await
            .expect("credential");

        assert_eq!(credential.expose_secret(), "exa-secret");
    }

    #[tokio::test]
    async fn exa_access_rejects_mismatched_provider_identity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let account = account("codex", "default");
        let persistence: ProviderAccountPersistenceHandle =
            Arc::new(FakeAccountPersistence::with_account(account.clone()));
        let service = ProviderCredentialAccessService::new(
            paths,
            persistence,
            AccountGateRegistry::new(),
            CodexOAuthClient::new(crate::CodexOAuthConfig::default()).expect("oauth"),
        );

        let error = service
            .exa_api_key(&account.provider_account_id)
            .await
            .expect_err("provider mismatch");

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    #[tokio::test]
    async fn codex_access_returns_usable_token_without_refreshing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let account = account("codex", "default");
        CodexTokenStore::new(paths.provider_account_home("codex", "default"))
            .write(&CodexOAuthTokens {
                access_token: "usable-access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: 1,
            })
            .expect("write tokens");
        let persistence: ProviderAccountPersistenceHandle =
            Arc::new(FakeAccountPersistence::with_account(account.clone()));
        let service = ProviderCredentialAccessService::new(
            paths,
            persistence,
            AccountGateRegistry::new(),
            CodexOAuthClient::new(CodexOAuthConfig::default()).expect("oauth"),
        );

        let credential = service
            .codex_access_token(&account.provider_account_id)
            .await
            .expect("credential");

        assert_eq!(credential.expose_secret(), "usable-access");
    }

    #[tokio::test]
    async fn codex_refresh_does_not_recreate_account_deleted_during_http() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let account = account("codex", "default");
        let account_home = paths.provider_account_home("codex", "default");
        CodexTokenStore::new(&account_home)
            .write(&CodexOAuthTokens {
                access_token: "old-access".to_string(),
                refresh_token: "old-refresh".to_string(),
                last_refresh: 1,
            })
            .expect("write tokens");
        let (issuer, token_url) = spawn_token_server().await;
        let persistence: ProviderAccountPersistenceHandle =
            Arc::new(FakeAccountPersistence::deleting_account_on_read(
                account.clone(),
                3,
                account_home.clone(),
            ));
        let service = ProviderCredentialAccessService::new(
            paths,
            persistence,
            AccountGateRegistry::new(),
            CodexOAuthClient::new(CodexOAuthConfig {
                issuer,
                client_id: "client".to_string(),
                token_url,
                timeout_seconds: 5,
            })
            .expect("oauth"),
        );

        let error = service
            .refresh_codex_access_token(&account.provider_account_id)
            .await
            .expect_err("deleted account");

        assert!(matches!(error, ProviderError::MissingCredentials { .. }));
        assert!(!account_home.exists());
    }

    fn account(provider_kind: &str, account_key: &str) -> ProviderAccountRecord {
        ProviderAccountRecord {
            provider_account_id: format!("provider_account:{provider_kind}:{account_key}"),
            provider_kind: provider_kind.to_string(),
            account_key: account_key.to_string(),
            display_name: provider_kind.to_string(),
            auth_method: expected_auth_method(provider_kind),
            is_active: true,
            is_default: true,
            status: ProviderAccountStatus::Authenticated,
            last_checked_at: None,
            last_authenticated_at: None,
            last_error_code: None,
            last_error_message: None,
            metadata: json!({}),
            capabilities: capabilities_for_provider_account(
                provider_kind,
                account_key,
                ProviderAccountStatus::Authenticated,
            ),
        }
    }

    async fn spawn_token_server() -> (String, String) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).await.expect("read");
            let body = r#"{"access_token":"new-access","refresh_token":"new-refresh"}"#;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .expect("write");
        });
        let issuer = format!("http://{addr}");
        (issuer.clone(), format!("{issuer}/token"))
    }
}
