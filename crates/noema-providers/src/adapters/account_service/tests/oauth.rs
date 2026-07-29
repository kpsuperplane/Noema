use std::{
    fs,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use noema_home::{NoemaPaths, SystemErrorLogger};
use serde_json::json;
use tokio::{sync::oneshot, time::timeout};

use super::{FakePersistence, ServiceFixture, auth_attempt, codex_account, tokens};
use crate::adapters::{
    auth::ProviderAuthAttemptRuntime,
    codex::oauth::{CodexDeviceAuthOutcome, CodexTokenStore},
    test_support::spawn_scripted_server,
};
use crate::{
    CodexOAuthConfig, ProviderAccountOperationError, ProviderAccountOperations,
    ProviderAccountPersistenceHandle, ProviderAccountStatus, ProviderAuthAttemptStatus,
    ProviderAuthMethod, ProviderModelCatalogPersistenceHandle, StartProviderAuthRequest,
};

#[tokio::test]
async fn service_uses_selected_codex_oauth_endpoints_and_client_id() {
    let home = tempfile::tempdir().expect("temp home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let mut account = codex_account();
    account.status = ProviderAccountStatus::Unauthenticated;
    account.metadata["profiles"] = json!([{"id":"gpt-5.2-codex","label":"Codex"}]);
    account.metadata["models_metadata_version"] = json!(3);
    account.metadata["models_client_version"] = json!("0.1.0");
    account.metadata["models_refreshed_at"] = json!(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_secs()
            .to_string()
    );
    let account_id = account.provider_account_id.clone();
    let persistence = Arc::new(FakePersistence::with_account(account));
    let accounts: ProviderAccountPersistenceHandle = persistence.clone();
    let catalogs: ProviderModelCatalogPersistenceHandle = persistence.clone();
    let (base_url, requests) = spawn_scripted_server([
        (
            200,
            r#"{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":0}"#,
        ),
        (
            200,
            r#"{"authorization_code":"authorization-secret","code_verifier":"verifier-secret"}"#,
        ),
        (
            200,
            r#"{"access_token":"access-secret","refresh_token":"refresh-secret"}"#,
        ),
    ])
    .await;
    let oauth = CodexOAuthConfig {
        issuer: base_url.clone(),
        client_id: "custom-client".to_string(),
        token_url: format!("{base_url}/custom/token"),
        timeout_seconds: 17,
    };

    let service = super::ProviderAccountService::new_with_codex_oauth(
        paths.clone(),
        accounts,
        catalogs,
        SystemErrorLogger::from_paths(&paths),
        oauth,
    )
    .expect("service");

    let attempt = service
        .start_auth(StartProviderAuthRequest {
            provider_kind: "codex".to_string(),
            provider_account_id: account_id.clone(),
            method: ProviderAuthMethod::OauthDeviceCode,
            callback_url: None,
        })
        .await
        .expect("start auth");
    let requests = timeout(Duration::from_secs(8), requests)
        .await
        .expect("OAuth requests timed out")
        .expect("OAuth request capture");
    let completed = wait_for_auth_terminal(&service, &attempt.attempt_id).await;

    assert_eq!(completed.status, ProviderAuthAttemptStatus::Completed);
    assert_eq!(requests[0].path, "/api/accounts/deviceauth/usercode");
    assert!(requests[0].body.contains(r#""client_id":"custom-client""#));
    assert_eq!(requests[1].path, "/api/accounts/deviceauth/token");
    assert_eq!(requests[2].path, "/custom/token");
    assert!(requests[2].body.contains("client_id=custom-client"));
    assert!(requests[2].body.contains("redirect_uri="));
    let updated = persistence.account(&account_id).expect("updated account");
    assert_eq!(updated.status, ProviderAccountStatus::Authenticated);
    assert_eq!(updated.metadata["credentialRevision"], json!(3));
    assert!(
        CodexTokenStore::new(paths.provider_account_home("codex", "default")).has_usable_tokens()
    );
}

#[tokio::test]
async fn start_auth_validates_active_provider_kind_and_method() {
    let mut mismatched_kind = codex_account();
    mismatched_kind.provider_kind = "exa".to_string();
    let mut inactive = codex_account();
    inactive.is_active = false;
    let mut mismatched_method = codex_account();
    mismatched_method.auth_method = ProviderAuthMethod::SecretInput;
    for (account, expected) in [
        (
            mismatched_kind,
            ProviderAccountOperationError::ProviderMismatch,
        ),
        (inactive, ProviderAccountOperationError::AccountInactive),
        (
            mismatched_method,
            ProviderAccountOperationError::AuthMethodMismatch,
        ),
    ] {
        let fixture = ServiceFixture::with_account(account.clone());
        let error = fixture
            .service
            .start_auth(StartProviderAuthRequest {
                provider_kind: "codex".to_string(),
                provider_account_id: account.provider_account_id,
                method: ProviderAuthMethod::OauthDeviceCode,
                callback_url: None,
            })
            .await
            .expect_err("invalid account must be rejected");
        assert_eq!(error, expected);
    }
}

#[tokio::test]
async fn oauth_update_failure_restores_previous_token_bytes() {
    let account = codex_account();
    let fixture = ServiceFixture::with_account(account.clone());
    let token_store = CodexTokenStore::new(fixture.paths.provider_account_home("codex", "default"));
    let old_tokens = tokens("old-access", "old-refresh");
    token_store.write(&old_tokens).expect("old tokens");
    let old_bytes = fs::read(token_store.token_path()).expect("old token bytes");
    fixture.persistence.fail_next_update();

    let error = fixture
        .service
        .publish_codex_tokens(&account, 2, &tokens("new-access", "new-refresh"))
        .await
        .expect_err("status update must fail");

    assert_eq!(error, ProviderAccountOperationError::Persistence);
    assert_eq!(
        fs::read(token_store.token_path()).expect("restored tokens"),
        old_bytes
    );
}

#[tokio::test]
async fn oauth_publication_rejects_a_stale_credential_revision_before_writing() {
    let account = codex_account();
    let fixture = ServiceFixture::with_account(account.clone());
    let token_store = CodexTokenStore::new(fixture.paths.provider_account_home("codex", "default"));
    let old_tokens = tokens("old-access", "old-refresh");
    token_store.write(&old_tokens).expect("old tokens");
    let old_bytes = fs::read(token_store.token_path()).expect("old token bytes");
    fixture
        .persistence
        .set_credential_revision(&account.provider_account_id, 3);

    let error = fixture
        .service
        .publish_codex_tokens(&account, 2, &tokens("stale-access", "stale-refresh"))
        .await
        .expect_err("stale publication must fail");

    assert_eq!(error, ProviderAccountOperationError::Conflict);
    assert_eq!(
        fs::read(token_store.token_path()).expect("unchanged tokens"),
        old_bytes
    );
}

#[tokio::test]
async fn cancelled_attempt_cannot_publish_returned_tokens_and_is_durable() {
    let (fixture, account, attempt) = registered_attempt("cancel-before-publication").await;
    fixture
        .service
        .inner
        .auth
        .cancel_attempt(&attempt.attempt_id)
        .await
        .expect("cancel attempt");

    fixture
        .service
        .complete_auth_attempt(
            attempt.attempt_id.clone(),
            account.clone(),
            2,
            CodexDeviceAuthOutcome::Completed(tokens("late-access", "late-refresh")),
        )
        .await;

    let token_store = CodexTokenStore::new(fixture.paths.provider_account_home("codex", "default"));
    assert!(!token_store.token_path().exists());
    let durable = fixture
        .persistence
        .account(&account.provider_account_id)
        .expect("durable account");
    assert_eq!(durable.status, ProviderAccountStatus::Unauthenticated);
    assert_eq!(
        durable.last_error_code.as_deref(),
        Some("provider_auth_cancelled")
    );
    assert_eq!(
        durable.last_error_message.as_deref(),
        Some("Provider authentication was cancelled")
    );
}

#[tokio::test]
async fn failed_attempt_persists_safe_terminal_account_state() {
    let (fixture, account, attempt) = registered_attempt("failed-terminal").await;

    fixture
        .service
        .complete_auth_attempt(
            attempt.attempt_id.clone(),
            account.clone(),
            2,
            CodexDeviceAuthOutcome::Failed {
                error_code: "codex_device_auth_failed".to_string(),
                error_message: "Codex login request failed".to_string(),
            },
        )
        .await;

    let terminal = fixture
        .service
        .auth_attempt(&attempt.attempt_id)
        .await
        .expect("attempt lookup")
        .expect("attempt");
    assert_eq!(terminal.status, ProviderAuthAttemptStatus::Failed);
    let durable = fixture
        .persistence
        .account(&account.provider_account_id)
        .expect("durable account");
    assert_eq!(durable.status, ProviderAccountStatus::Unauthenticated);
    assert_eq!(
        durable.last_error_code.as_deref(),
        Some("codex_device_auth_failed")
    );
    assert_eq!(
        durable.last_error_message.as_deref(),
        Some("Codex login request failed")
    );
}

#[tokio::test]
async fn service_shutdown_cancels_and_drains_registered_auth_tasks() {
    let account = codex_account();
    let fixture = ServiceFixture::with_account(account.clone());
    let attempt = auth_attempt(&account, "shutdown-drain");
    let (cancel_sender, cancel_receiver) = oneshot::channel();
    assert!(
        fixture
            .service
            .inner
            .auth
            .register_attempt(attempt, ProviderAuthAttemptRuntime::new(cancel_sender),)
            .await
    );
    let (cancel_observed_sender, cancel_observed_receiver) = oneshot::channel();
    let (release_sender, release_receiver) = oneshot::channel();
    fixture
        .service
        .inner
        .auth_tasks
        .lock()
        .await
        .push(tokio::spawn(async move {
            cancel_receiver.await.expect("shutdown cancellation");
            cancel_observed_sender
                .send(())
                .expect("signal cancellation");
            release_receiver.await.expect("release auth task");
        }));

    let service = fixture.service.clone();
    let shutdown = tokio::spawn(async move { service.shutdown().await });
    cancel_observed_receiver
        .await
        .expect("auth task observed cancellation");
    assert!(!shutdown.is_finished());
    release_sender.send(()).expect("release shutdown");
    shutdown.await.expect("shutdown task");
}

#[tokio::test]
async fn stale_terminal_outcome_does_not_clobber_newer_account_state() {
    let (fixture, account, attempt) = registered_attempt("stale-terminal").await;
    fixture
        .persistence
        .set_credential_revision(&account.provider_account_id, 3);
    {
        let mut state = fixture.persistence.state.lock().expect("fake state");
        let current = state
            .account
            .as_mut()
            .filter(|current| current.provider_account_id == account.provider_account_id)
            .expect("account");
        current.status = ProviderAccountStatus::Authenticated;
        current.last_error_code = None;
        current.last_error_message = None;
    }

    let error = fixture
        .service
        .persist_auth_terminal(
            &attempt.attempt_id,
            &account,
            2,
            ProviderAuthAttemptStatus::Failed,
            Some("codex_device_auth_failed".to_string()),
            Some("Codex login request failed".to_string()),
        )
        .await
        .expect_err("stale terminal state must fail");

    assert_eq!(error, ProviderAccountOperationError::Conflict);
    let durable = fixture
        .persistence
        .account(&account.provider_account_id)
        .expect("durable account");
    assert_eq!(durable.status, ProviderAccountStatus::Authenticated);
    assert_eq!(durable.metadata["credentialRevision"], json!(3));
    assert_eq!(durable.last_error_code, None);
}

async fn registered_attempt(
    attempt_id: &str,
) -> (
    ServiceFixture,
    crate::ProviderAccountRecord,
    crate::ProviderAuthAttemptView,
) {
    let mut account = codex_account();
    account.status = ProviderAccountStatus::Unauthenticated;
    let fixture = ServiceFixture::with_account(account.clone());
    let attempt = auth_attempt(&account, attempt_id);
    let (cancel, _cancelled) = oneshot::channel();
    assert!(
        fixture
            .service
            .inner
            .auth
            .register_attempt(attempt.clone(), ProviderAuthAttemptRuntime::new(cancel))
            .await
    );
    (fixture, account, attempt)
}

async fn wait_for_auth_terminal(
    service: &super::ProviderAccountService,
    attempt_id: &str,
) -> crate::ProviderAuthAttemptView {
    timeout(Duration::from_secs(2), async {
        loop {
            let attempt = service
                .auth_attempt(attempt_id)
                .await
                .expect("auth attempt lookup")
                .expect("auth attempt");
            if matches!(
                attempt.status,
                ProviderAuthAttemptStatus::Completed
                    | ProviderAuthAttemptStatus::Failed
                    | ProviderAuthAttemptStatus::Expired
                    | ProviderAuthAttemptStatus::Cancelled
            ) {
                return attempt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("auth attempt did not finish")
}
