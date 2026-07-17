use std::{
    collections::BTreeMap,
    fs,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use noema_home::{NoemaPaths, SystemErrorLogger};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
    time::timeout,
};

use super::ProviderAccountService;
use crate::adapters::{
    account_service::{filesystem::atomic_write_private, gates::AccountGateRegistry},
    codex::oauth::CodexTokenStore,
};
use crate::{
    CodexOAuthTokens, CreateSecretProviderAccountRequest, NewProviderAccount,
    PersistProviderModelCatalogRequest, PersistedProviderAccountRecord,
    ProviderAccountOperationError, ProviderAccountOperations, ProviderAccountPersistence,
    ProviderAccountPersistenceHandle, ProviderAccountRecord, ProviderAccountStatus,
    ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderAuthMethod, ProviderCredential,
    ProviderCredentialAccess, ProviderCredentialAccessHandle, ProviderCredentialFuture,
    ProviderError, ProviderModelCatalogPersistence, ProviderModelCatalogPersistenceHandle,
    ProviderPersistenceError, ProviderPersistenceFuture, SaveProviderAccountSecretRequest,
    UpdateProviderAccountRequest, capabilities_for_provider_account,
};

#[path = "tests/auth_failure.rs"]
mod auth_failure;
#[path = "tests/oauth.rs"]
mod oauth;

#[derive(Default)]
struct FakePersistence {
    state: Mutex<FakePersistenceState>,
}

#[derive(Default)]
struct FakePersistenceState {
    accounts: BTreeMap<String, PersistedProviderAccountRecord>,
    fail_next_update: bool,
    fail_next_delete: bool,
    catalog_persist_count: usize,
}

impl FakePersistence {
    fn with_account(account: ProviderAccountRecord) -> Self {
        let provider_account_id = account.provider_account_id.clone();
        Self {
            state: Mutex::new(FakePersistenceState {
                accounts: BTreeMap::from([(provider_account_id, account.into())]),
                ..FakePersistenceState::default()
            }),
        }
    }

    fn fail_next_update(&self) {
        self.state.lock().expect("fake state").fail_next_update = true;
    }

    fn fail_next_delete(&self) {
        self.state.lock().expect("fake state").fail_next_delete = true;
    }

    fn account(&self, provider_account_id: &str) -> Option<PersistedProviderAccountRecord> {
        self.state
            .lock()
            .expect("fake state")
            .accounts
            .get(provider_account_id)
            .cloned()
    }

    fn set_credential_revision(&self, provider_account_id: &str, revision: u64) {
        self.state
            .lock()
            .expect("fake state")
            .accounts
            .get_mut(provider_account_id)
            .expect("provider account")
            .metadata["credentialRevision"] = json!(revision);
    }

    fn catalog_persist_count(&self) -> usize {
        self.state.lock().expect("fake state").catalog_persist_count
    }
}

impl ProviderAccountPersistence for FakePersistence {
    fn provider_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<PersistedProviderAccountRecord>> {
        let account = self.account(provider_account_id);
        Box::pin(async move { Ok(account) })
    }

    fn active_provider_account<'a>(
        &'a self,
        provider_kind: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<PersistedProviderAccountRecord>> {
        let account = self
            .state
            .lock()
            .expect("fake state")
            .accounts
            .values()
            .find(|account| {
                account.provider_kind == provider_kind && account.is_active && account.is_default
            })
            .cloned();
        Box::pin(async move { Ok(account) })
    }

    fn active_default_provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<PersistedProviderAccountRecord>> {
        let accounts = self
            .state
            .lock()
            .expect("fake state")
            .accounts
            .values()
            .filter(|account| account.is_active && account.is_default)
            .cloned()
            .collect();
        Box::pin(async move { Ok(accounts) })
    }

    fn active_provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<PersistedProviderAccountRecord>> {
        let accounts = self
            .state
            .lock()
            .expect("fake state")
            .accounts
            .values()
            .filter(|account| account.is_active)
            .cloned()
            .collect();
        Box::pin(async move { Ok(accounts) })
    }

    fn provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<PersistedProviderAccountRecord>> {
        let accounts = self
            .state
            .lock()
            .expect("fake state")
            .accounts
            .values()
            .cloned()
            .collect();
        Box::pin(async move { Ok(accounts) })
    }

    fn create_provider_account(
        &self,
        request: NewProviderAccount,
    ) -> ProviderPersistenceFuture<'_, PersistedProviderAccountRecord> {
        let account = PersistedProviderAccountRecord::from(account(
            &request.provider_kind,
            "created",
            request.auth_method,
            false,
            request.status,
            request.metadata,
        ));
        self.state
            .lock()
            .expect("fake state")
            .accounts
            .insert(account.provider_account_id.clone(), account.clone());
        Box::pin(async move { Ok(account) })
    }

    fn update_provider_account(
        &self,
        request: UpdateProviderAccountRequest,
    ) -> ProviderPersistenceFuture<'_, PersistedProviderAccountRecord> {
        let result = {
            let mut state = self.state.lock().expect("fake state");
            if state.fail_next_update {
                state.fail_next_update = false;
                Err(ProviderPersistenceError::Persistence {
                    operation: "update_provider_account",
                })
            } else {
                let account = state
                    .accounts
                    .get_mut(&request.provider_account_id)
                    .ok_or_else(|| ProviderPersistenceError::AccountNotFound {
                        provider_account_id: request.provider_account_id.clone(),
                    });
                account.map(|account| {
                    if let Some(status) = request.status {
                        account.status = status.status;
                        account.last_error_code = status.error_code;
                        account.last_error_message = status.error_message;
                    }
                    if let Some(metadata) = request.metadata {
                        account.metadata = metadata;
                    }
                    account.clone()
                })
            }
        };
        Box::pin(async move { result })
    }

    fn delete_provider_account<'a>(
        &'a self,
        provider_account_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, bool> {
        let result = {
            let mut state = self.state.lock().expect("fake state");
            if state.fail_next_delete {
                state.fail_next_delete = false;
                Err(ProviderPersistenceError::Persistence {
                    operation: "delete_provider_account",
                })
            } else if state
                .accounts
                .get(provider_account_id)
                .is_some_and(|account| account.is_default)
            {
                Err(ProviderPersistenceError::ProtectedAccount {
                    provider_account_id: provider_account_id.to_string(),
                })
            } else {
                Ok(state.accounts.remove(provider_account_id).is_some())
            }
        };
        Box::pin(async move { result })
    }
}

impl ProviderModelCatalogPersistence for FakePersistence {
    fn persist_provider_model_catalog(
        &self,
        request: PersistProviderModelCatalogRequest,
    ) -> ProviderPersistenceFuture<'_, PersistedProviderAccountRecord> {
        let result = {
            let mut state = self.state.lock().expect("fake state");
            state.catalog_persist_count += 1;
            state
                .accounts
                .get(&request.provider_account_id)
                .cloned()
                .ok_or(ProviderPersistenceError::AccountNotFound {
                    provider_account_id: request.provider_account_id,
                })
        };
        Box::pin(async move { result })
    }
}

#[derive(Debug)]
struct StaticCodexCredentials;

impl ProviderCredentialAccess for StaticCodexCredentials {
    fn exa_api_key<'a>(&'a self, _provider_account_id: &'a str) -> ProviderCredentialFuture<'a> {
        Box::pin(async {
            Err(ProviderError::MissingCredentials {
                provider: "exa".to_string(),
                credential: "provider account".to_string(),
            })
        })
    }

    fn codex_access_token<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a> {
        Box::pin(async { Ok(ProviderCredential::from("catalog-token".to_string())) })
    }

    fn refresh_codex_access_token<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderCredentialFuture<'a> {
        Box::pin(async { Ok(ProviderCredential::from("refreshed-token".to_string())) })
    }
}

struct ServiceFixture {
    _home: tempfile::TempDir,
    paths: NoemaPaths,
    persistence: Arc<FakePersistence>,
    service: ProviderAccountService,
}

impl ServiceFixture {
    fn empty() -> Self {
        Self::new(FakePersistence::default())
    }

    fn with_account(account: ProviderAccountRecord) -> Self {
        Self::new(FakePersistence::with_account(account))
    }

    fn new(persistence: FakePersistence) -> Self {
        let home = tempfile::tempdir().expect("temp home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let system_errors = SystemErrorLogger::from_paths(&paths);
        let persistence = Arc::new(persistence);
        let accounts: ProviderAccountPersistenceHandle = persistence.clone();
        let catalogs: ProviderModelCatalogPersistenceHandle = persistence.clone();
        let service = ProviderAccountService::new(paths.clone(), accounts, catalogs, system_errors)
            .expect("service");
        Self {
            _home: home,
            paths,
            persistence,
            service,
        }
    }
}

#[tokio::test]
async fn create_secret_write_failure_deletes_new_durable_account() {
    let fixture = ServiceFixture::empty();
    let account_home = fixture.paths.provider_account_home("exa", "created");
    fs::create_dir_all(account_home.parent().expect("provider parent")).expect("parent");
    fs::write(&account_home, b"not a directory").expect("blocking file");

    let error = fixture
        .service
        .create_secret_account(
            CreateSecretProviderAccountRequest::new("exa", None, "secret").expect("request"),
        )
        .await
        .expect_err("write must fail");

    assert_eq!(error, ProviderAccountOperationError::ProviderUnavailable);
    assert!(
        fixture
            .persistence
            .account("provider_account:exa:created")
            .is_none()
    );
}

#[tokio::test]
async fn create_update_failure_removes_secret_and_new_account() {
    let fixture = ServiceFixture::empty();
    fixture.persistence.fail_next_update();

    let error = fixture
        .service
        .create_secret_account(
            CreateSecretProviderAccountRequest::new("exa", None, "new-secret").expect("request"),
        )
        .await
        .expect_err("update must fail");

    assert_eq!(error, ProviderAccountOperationError::Persistence);
    assert!(
        fixture
            .persistence
            .account("provider_account:exa:created")
            .is_none()
    );
    assert!(
        !fixture
            .paths
            .provider_account_home("exa", "created")
            .join("api_key.json")
            .exists()
    );
}

#[tokio::test]
async fn failed_create_compensation_is_typed_and_keeps_durable_evidence() {
    let fixture = ServiceFixture::empty();
    fixture.persistence.fail_next_update();
    fixture.persistence.fail_next_delete();

    let error = fixture
        .service
        .create_secret_account(
            CreateSecretProviderAccountRequest::new("exa", None, "new-secret").expect("request"),
        )
        .await
        .expect_err("compensation must fail");

    assert_eq!(error, ProviderAccountOperationError::CompensationFailed);
    assert!(
        fixture
            .persistence
            .account("provider_account:exa:created")
            .is_some()
    );
}

#[tokio::test]
async fn save_update_failure_restores_exact_prior_secret_bytes() {
    let account = exa_account("team", false);
    let fixture = ServiceFixture::with_account(account.clone());
    let secret_path = fixture
        .paths
        .provider_account_home("exa", "team")
        .join("api_key.json");
    let original = b"{malformed prior secret}\0\xff";
    atomic_write_private(&secret_path, original).expect("prior secret");
    fixture.persistence.fail_next_update();

    let error = fixture
        .service
        .save_secret(
            SaveProviderAccountSecretRequest::new(
                account.provider_account_id.clone(),
                "replacement",
            )
            .expect("request"),
        )
        .await
        .expect_err("update must fail");

    assert_eq!(error, ProviderAccountOperationError::Persistence);
    assert_eq!(fs::read(secret_path).expect("restored secret"), original);
}

#[tokio::test]
async fn clear_update_failure_restores_exact_prior_secret_bytes() {
    let account = exa_account("team", false);
    let fixture = ServiceFixture::with_account(account.clone());
    let secret_path = fixture
        .paths
        .provider_account_home("exa", "team")
        .join("api_key.json");
    let original = b"{malformed prior secret}\0\xff";
    atomic_write_private(&secret_path, original).expect("prior secret");
    fixture.persistence.fail_next_update();

    let error = fixture
        .service
        .clear_secret(&account.provider_account_id)
        .await
        .expect_err("update must fail");

    assert_eq!(error, ProviderAccountOperationError::Persistence);
    assert_eq!(fs::read(secret_path).expect("restored secret"), original);
}

#[tokio::test]
async fn save_merges_metadata_and_increments_credential_revision() {
    let mut account = exa_account("team", false);
    account.metadata = json!({
        "base_url": "https://example.test",
        "credentialRevision": 4,
    });
    let fixture = ServiceFixture::with_account(account.clone());

    let updated = fixture
        .service
        .save_secret(
            SaveProviderAccountSecretRequest::new(account.provider_account_id, "replacement")
                .expect("request"),
        )
        .await
        .expect("save");

    assert_eq!(updated.metadata["base_url"], json!("https://example.test"));
    assert_eq!(updated.metadata["credentialRevision"], json!(5));
    assert_eq!(updated.metadata["secretConfigured"], json!(true));
}

#[tokio::test]
async fn delete_persistence_failure_restores_quarantined_account_home() {
    let account = exa_account("team", false);
    let fixture = ServiceFixture::with_account(account.clone());
    let account_home = fixture.paths.provider_account_home("exa", "team");
    let secret_path = account_home.join("api_key.json");
    atomic_write_private(&secret_path, b"credential bytes").expect("secret");
    fixture.persistence.fail_next_delete();

    let error = fixture
        .service
        .delete_account(&account.provider_account_id)
        .await
        .expect_err("delete must fail");

    assert_eq!(error, ProviderAccountOperationError::Persistence);
    assert_eq!(
        fs::read(secret_path).expect("restored account home"),
        b"credential bytes"
    );
    assert!(
        fixture
            .persistence
            .account(&account.provider_account_id)
            .is_some()
    );
}

#[tokio::test]
async fn credential_reads_share_the_account_service_gate() {
    let account = exa_account("team", false);
    let fixture = ServiceFixture::with_account(account.clone());
    crate::adapters::SecretInputStore::new(fixture.paths.provider_account_home("exa", "team"))
        .save_api_key("exa-secret")
        .expect("secret");
    let gate = fixture
        .service
        .inner
        .gates
        .gate(&account.provider_account_id);
    let guard = gate.lock().await;
    let credentials = fixture.service.credentials();
    let account_id = account.provider_account_id;
    let reader = tokio::spawn(async move { credentials.exa_api_key(&account_id).await });
    tokio::task::yield_now().await;

    assert!(!reader.is_finished());
    drop(guard);
    assert_eq!(
        reader
            .await
            .expect("reader task")
            .expect("credential")
            .expose_secret(),
        "exa-secret"
    );
}

#[tokio::test]
async fn catalog_refresh_rejects_a_credential_change_during_http() {
    let home = tempfile::tempdir().expect("temp home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let (base_url, request_started, release_response) = spawn_blocking_model_catalog_server().await;
    let mut account = codex_account();
    account.metadata = json!({
        "base_url": base_url,
        "credentialRevision": 1,
        "models_client_version": "0.144.0",
        "models_client_version_refreshed_at": current_timestamp_string(),
    });
    let account_id = account.provider_account_id.clone();
    let persistence = Arc::new(FakePersistence::with_account(account));
    let accounts: ProviderAccountPersistenceHandle = persistence.clone();
    let catalogs: ProviderModelCatalogPersistenceHandle = persistence.clone();
    let gates = AccountGateRegistry::new();
    let credentials: ProviderCredentialAccessHandle = Arc::new(StaticCodexCredentials);
    let service = ProviderAccountService::from_parts(
        paths.clone(),
        accounts,
        catalogs,
        SystemErrorLogger::from_paths(&paths),
        gates.clone(),
        credentials,
    );
    let refresh = tokio::spawn({
        let service = service.clone();
        let account_id = account_id.clone();
        async move { service.refresh_model_catalog(&account_id).await }
    });

    request_started.await.expect("catalog request started");
    let gate = gates.gate(&account_id);
    let guard = timeout(Duration::from_secs(1), gate.lock())
        .await
        .expect("catalog HTTP must not hold the account gate");
    persistence.set_credential_revision(&account_id, 2);
    drop(guard);
    release_response.send(()).expect("release catalog response");

    let error = refresh
        .await
        .expect("refresh task")
        .expect_err("stale catalog must not persist");
    assert_eq!(error, ProviderAccountOperationError::Conflict);
    assert_eq!(persistence.catalog_persist_count(), 0);
}

#[tokio::test]
async fn reconcile_account_marks_existing_codex_tokens_authenticated() {
    let mut account = codex_account();
    account.status = ProviderAccountStatus::Unauthenticated;
    let fixture = ServiceFixture::with_account(account.clone());
    CodexTokenStore::new(fixture.paths.provider_account_home("codex", "default"))
        .write(&tokens("existing-access", "existing-refresh"))
        .expect("write existing tokens");

    let reconciled = fixture
        .service
        .reconcile_account(&account.provider_account_id)
        .await
        .expect("reconcile account");

    assert_eq!(reconciled.status, ProviderAccountStatus::Authenticated);
    assert_eq!(
        fixture
            .persistence
            .account(&account.provider_account_id)
            .expect("durable account")
            .status,
        ProviderAccountStatus::Authenticated
    );
}

fn exa_account(account_key: &str, is_default: bool) -> ProviderAccountRecord {
    account(
        "exa",
        account_key,
        ProviderAuthMethod::SecretInput,
        is_default,
        ProviderAccountStatus::Authenticated,
        json!({
            "secretConfigured": true,
            "credentialRevision": 1,
        }),
    )
}

fn codex_account() -> ProviderAccountRecord {
    account(
        "codex",
        "default",
        ProviderAuthMethod::OauthDeviceCode,
        true,
        ProviderAccountStatus::Authenticated,
        json!({"credentialRevision": 2}),
    )
}

fn account(
    provider_kind: &str,
    account_key: &str,
    auth_method: ProviderAuthMethod,
    is_default: bool,
    status: ProviderAccountStatus,
    metadata: serde_json::Value,
) -> ProviderAccountRecord {
    ProviderAccountRecord {
        provider_account_id: format!("provider_account:{provider_kind}:{account_key}"),
        provider_kind: provider_kind.to_string(),
        account_key: account_key.to_string(),
        display_name: provider_kind.to_string(),
        auth_method,
        is_active: true,
        is_default,
        status,
        last_checked_at: None,
        last_authenticated_at: None,
        last_error_code: None,
        last_error_message: None,
        metadata,
        capabilities: capabilities_for_provider_account(provider_kind, account_key, status),
    }
}

fn tokens(access_token: &str, refresh_token: &str) -> CodexOAuthTokens {
    CodexOAuthTokens {
        access_token: access_token.to_string(),
        refresh_token: refresh_token.to_string(),
        last_refresh: 123,
    }
}

fn auth_attempt(account: &ProviderAccountRecord, attempt_id: &str) -> ProviderAuthAttemptView {
    ProviderAuthAttemptView {
        attempt_id: attempt_id.to_string(),
        provider_kind: account.provider_kind.clone(),
        provider_account_id: account.provider_account_id.clone(),
        method: ProviderAuthMethod::OauthDeviceCode,
        status: ProviderAuthAttemptStatus::WaitingForUser,
        verification_url: Some("https://example.test/device".to_string()),
        user_code: Some("SAFE-TEST-CODE".to_string()),
        instructions: Some("Complete the login in your browser.".to_string()),
        error_code: None,
        error_message: None,
    }
}

fn current_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_secs()
        .to_string()
}

async fn spawn_blocking_model_catalog_server()
-> (String, oneshot::Receiver<()>, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let (request_started_tx, request_started_rx) = oneshot::channel();
    let (release_response_tx, release_response_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut request = [0_u8; 4096];
        let request_bytes = stream.read(&mut request).await.expect("read request");
        assert!(request_bytes > 0);
        request_started_tx.send(()).expect("signal request");
        release_response_rx.await.expect("release response");
        let body = json!({
            "models": [{
                "slug": "gpt-live",
                "display_name": "GPT Live",
                "visibility": "list",
            }],
        })
        .to_string();
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .expect("write response");
    });
    (
        format!("http://{address}"),
        request_started_rx,
        release_response_tx,
    )
}
