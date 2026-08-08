use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use noema_home::{NoemaPaths, SystemErrorLogger};
use serde_json::json;
use tokio::time::timeout;

use super::ProviderAccountService;
use crate::adapters::{
    account_service::{filesystem::atomic_write_private, gates::AccountGateRegistry},
    codex::oauth::CodexTokenStore,
    test_support::{spawn_blocking_server, static_codex_credentials},
};
use crate::{
    CodexOAuthTokens, CreateSecretProviderAccountRequest, NewProviderAccount,
    PersistProviderModelCatalogRequest, ProviderAccountOperationError, ProviderAccountOperations,
    ProviderAccountPersistence, ProviderAccountPersistenceHandle, ProviderAccountRecord,
    ProviderAccountStatus, ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderAuthMethod,
    ProviderCredentialAccessHandle, ProviderModelCatalogPersistence,
    ProviderModelCatalogPersistenceHandle, ProviderPersistenceError, ProviderPersistenceFuture,
    ProviderReadySelection, ProviderSelectionSnapshot, SaveProviderAccountSecretRequest,
    UpdateProviderAccountRequest, capabilities_for_provider_account,
};

#[path = "tests/auth_failure.rs"]
mod auth_failure;
#[path = "tests/oauth.rs"]
mod oauth;

#[derive(Default)]
pub(in crate::adapters::account_service) struct FakePersistence {
    state: Mutex<FakePersistenceState>,
}

#[derive(Default)]
struct FakePersistenceState {
    account: Option<ProviderAccountRecord>,
    fail_next_update: bool,
    fail_next_delete: bool,
    catalog_persist_count: usize,
    reads: usize,
    delete_on_read: Option<(usize, PathBuf)>,
}

impl FakePersistence {
    pub(in crate::adapters::account_service) fn with_account(
        account: ProviderAccountRecord,
    ) -> Self {
        Self {
            state: Mutex::new(FakePersistenceState {
                account: Some(account),
                ..FakePersistenceState::default()
            }),
        }
    }

    pub(in crate::adapters::account_service) fn deleting_account_on_read(
        account: ProviderAccountRecord,
        read_number: usize,
        account_home: PathBuf,
    ) -> Self {
        let persistence = Self::with_account(account);
        persistence.state.lock().expect("fake state").delete_on_read =
            Some((read_number, account_home));
        persistence
    }

    fn fail_next_update(&self) {
        self.state.lock().expect("fake state").fail_next_update = true;
    }

    fn fail_next_delete(&self) {
        self.state.lock().expect("fake state").fail_next_delete = true;
    }

    pub(in crate::adapters::account_service) fn account(
        &self,
        provider_account_id: &str,
    ) -> Option<ProviderAccountRecord> {
        self.state
            .lock()
            .expect("fake state")
            .account
            .as_ref()
            .filter(|account| account.provider_account_id == provider_account_id)
            .cloned()
    }

    fn set_credential_revision(&self, provider_account_id: &str, revision: u64) {
        self.state
            .lock()
            .expect("fake state")
            .account
            .as_mut()
            .filter(|account| account.provider_account_id == provider_account_id)
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
    ) -> ProviderPersistenceFuture<'a, Option<ProviderAccountRecord>> {
        let mut state = self.state.lock().expect("fake state");
        state.reads += 1;
        if let Some((read_number, account_home)) = state.delete_on_read.as_ref()
            && state.reads == *read_number
        {
            fs::remove_dir_all(account_home).expect("remove account home");
            state.account = None;
        }
        ready(Ok(state
            .account
            .as_ref()
            .filter(|account| account.provider_account_id == provider_account_id)
            .cloned()))
    }

    fn active_provider_accounts(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<ProviderAccountRecord>> {
        let accounts = self
            .state
            .lock()
            .expect("fake state")
            .account
            .iter()
            .filter(|account| account.is_active)
            .cloned()
            .collect();
        ready(Ok(accounts))
    }

    fn create_provider_account(
        &self,
        request: NewProviderAccount,
    ) -> ProviderPersistenceFuture<'_, ProviderAccountRecord> {
        let account = account(
            &request.provider_kind,
            "created",
            request.auth_method,
            false,
            request.status,
            request.metadata,
        );
        self.state.lock().expect("fake state").account = Some(account.clone());
        ready(Ok(account))
    }

    fn update_provider_account(
        &self,
        request: UpdateProviderAccountRequest,
    ) -> ProviderPersistenceFuture<'_, ProviderAccountRecord> {
        let result = {
            let mut state = self.state.lock().expect("fake state");
            if state.fail_next_update {
                state.fail_next_update = false;
                Err(ProviderPersistenceError::Persistence {
                    operation: "update_provider_account",
                })
            } else {
                let account = state
                    .account
                    .as_mut()
                    .filter(|account| account.provider_account_id == request.provider_account_id)
                    .ok_or_else(|| ProviderPersistenceError::AccountNotFound {
                        provider_account_id: request.provider_account_id.clone(),
                    });
                account.map(|account| {
                    if let Some(auth_method) = request.auth_method {
                        account.auth_method = auth_method;
                    }
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
        ready(result)
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
                .account
                .as_ref()
                .filter(|account| account.provider_account_id == provider_account_id)
                .is_some_and(|account| account.is_default)
            {
                Err(ProviderPersistenceError::ProtectedAccount {
                    provider_account_id: provider_account_id.to_string(),
                })
            } else {
                Ok(state
                    .account
                    .take_if(|account| account.provider_account_id == provider_account_id)
                    .is_some())
            }
        };
        ready(result)
    }

    fn initialize_missing_provider_selections<'a>(
        &'a self,
        _selection: &'a ProviderSelectionSnapshot,
        _ready: &'a ProviderReadySelection,
    ) -> ProviderPersistenceFuture<'a, ()> {
        ready(Err(ProviderPersistenceError::Persistence {
            operation: "unexpected_account_selection_initialization",
        }))
    }
}

impl ProviderModelCatalogPersistence for FakePersistence {
    fn persist_provider_model_catalog(
        &self,
        request: PersistProviderModelCatalogRequest,
    ) -> ProviderPersistenceFuture<'_, ProviderAccountRecord> {
        let result = {
            let mut state = self.state.lock().expect("fake state");
            state.catalog_persist_count += 1;
            state
                .account
                .as_ref()
                .filter(|account| account.provider_account_id == request.provider_account_id)
                .cloned()
                .ok_or(ProviderPersistenceError::AccountNotFound {
                    provider_account_id: request.provider_account_id,
                })
        };
        ready(result)
    }
}

fn ready<T>(result: Result<T, ProviderPersistenceError>) -> ProviderPersistenceFuture<'static, T>
where
    T: Send + 'static,
{
    Box::pin(std::future::ready(result))
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
async fn secret_account_creation_compensation_contracts() {
    #[derive(Clone, Copy, Debug)]
    enum Case {
        SecretWrite,
        DurableUpdate,
        DurableCompensation,
    }

    for (case, expected) in [
        (
            Case::SecretWrite,
            ProviderAccountOperationError::ProviderUnavailable,
        ),
        (
            Case::DurableUpdate,
            ProviderAccountOperationError::Persistence,
        ),
        (
            Case::DurableCompensation,
            ProviderAccountOperationError::CompensationFailed,
        ),
    ] {
        eprintln!("case: {case:?}");
        let fixture = ServiceFixture::empty();
        match case {
            Case::SecretWrite => {
                let account_home = fixture.paths.provider_account_home("exa", "created");
                fs::create_dir_all(account_home.parent().expect("provider parent"))
                    .expect("parent");
                fs::write(account_home, b"not a directory").expect("blocking file");
            }
            Case::DurableUpdate => fixture.persistence.fail_next_update(),
            Case::DurableCompensation => {
                fixture.persistence.fail_next_update();
                fixture.persistence.fail_next_delete();
            }
        }

        let error = fixture
            .service
            .create_secret_account(
                CreateSecretProviderAccountRequest::new("exa", None, "new-secret")
                    .expect("request"),
            )
            .await
            .expect_err("injected create failure");
        assert_eq!(error, expected, "{case:?}");
        let account = fixture.persistence.account("provider_account:exa:created");
        assert_eq!(account.is_some(), matches!(case, Case::DurableCompensation));
        if matches!(case, Case::DurableUpdate) {
            assert!(
                !fixture
                    .paths
                    .provider_account_home("exa", "created")
                    .join("api_key.json")
                    .exists()
            );
        }
    }
}

#[tokio::test]
async fn secret_mutation_update_failure_restores_exact_prior_bytes() {
    for clear in [false, true] {
        let account = exa_account("team", false);
        let fixture = ServiceFixture::with_account(account.clone());
        let secret_path = fixture
            .paths
            .provider_account_home("exa", "team")
            .join("api_key.json");
        let original = b"{malformed prior secret}\0\xff";
        atomic_write_private(&secret_path, original).expect("prior secret");
        fixture.persistence.fail_next_update();

        let error = if clear {
            fixture
                .service
                .clear_secret(&account.provider_account_id)
                .await
        } else {
            fixture
                .service
                .save_secret(
                    SaveProviderAccountSecretRequest::new(
                        account.provider_account_id,
                        "replacement",
                    )
                    .expect("request"),
                )
                .await
        }
        .expect_err("update must fail");

        assert_eq!(error, ProviderAccountOperationError::Persistence);
        assert_eq!(fs::read(secret_path).expect("restored secret"), original);
    }
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
    let reader = tokio::spawn(async move { credentials.api_key("exa", &account_id).await });
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
    let credentials: ProviderCredentialAccessHandle =
        static_codex_credentials("catalog-token", "refreshed-token");
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

#[tokio::test]
async fn codex_config_for_provider_account_uses_account_home() {
    let account = codex_account();
    let fixture = ServiceFixture::with_account(account.clone());
    CodexTokenStore::new(fixture.paths.provider_account_home("codex", "default"))
        .write(&tokens("account-home-access", "refresh"))
        .expect("write account tokens");

    let credential = fixture
        .service
        .credentials()
        .codex_access_token(&account.provider_account_id)
        .await
        .expect("credential from canonical account home");
    assert_eq!(credential.expose_secret(), "account-home-access");
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

async fn spawn_blocking_model_catalog_server() -> (
    String,
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    spawn_blocking_server(
        Vec::new(),
        200,
        json!({
            "models": [{
                "slug": "gpt-live",
                "display_name": "GPT Live",
                "visibility": "list",
            }],
        })
        .to_string(),
    )
    .await
}
