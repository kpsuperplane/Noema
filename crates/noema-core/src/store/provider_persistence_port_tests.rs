//! Provider persistence port transaction and error-contract tests.

use noema_capabilities::{CapabilityId, ToolName};
use noema_providers::{
    NewProviderAccount, PersistProviderModelCatalogRequest, ProviderAccountPersistence,
    ProviderAccountStatus, ProviderAccountStatusUpdate, ProviderAuthMethod,
    ProviderCapabilityAssignmentPersistence, ProviderModelCatalogPersistence, ProviderModelProfile,
    ProviderPersistenceError, UpdateProviderAccountRequest,
    UpsertProviderCapabilityAssignmentRequest,
};
use serde_json::json;
use tempfile::TempDir;

use super::{NoemaStore, StoreConfig};

#[tokio::test]
async fn catalog_commit_merges_latest_metadata_and_status_atomically() {
    let store = super::tests::test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("default provider account");
    store
        .update_provider_account_status(
            &account.provider_account_id,
            ProviderAccountStatus::Unavailable,
            Some("old_error"),
            Some("old message"),
        )
        .await
        .expect("seed unavailable status");
    let stale = store
        .get_provider_account(&account.provider_account_id)
        .await
        .expect("read stale account")
        .expect("account");
    store
        .update_provider_account_metadata(
            &account.provider_account_id,
            json!({
                "base_url": "https://example.test",
                "unrelated": {"generation": 2}
            }),
        )
        .await
        .expect("concurrent unrelated metadata update");

    let updated = ProviderModelCatalogPersistence::persist_provider_model_catalog(
        &store,
        PersistProviderModelCatalogRequest {
            provider_account_id: stale.provider_account_id,
            profiles: vec![ProviderModelProfile {
                id: "gpt-live".to_string(),
                label: "GPT Live".to_string(),
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            }],
            refreshed_at_unix: 100,
            source: "codex_models_endpoint".to_string(),
            metadata_version: 3,
            client_version: "0.144.1".to_string(),
            client_version_refreshed_at_unix: Some(99),
            resulting_status: ProviderAccountStatus::Authenticated,
        },
    )
    .await
    .expect("persist catalog");

    assert_eq!(updated.status, ProviderAccountStatus::Authenticated);
    assert!(updated.last_checked_at.is_some());
    assert!(updated.last_authenticated_at.is_some());
    assert_eq!(updated.last_error_code, None);
    assert_eq!(updated.last_error_message, None);
    assert_eq!(updated.metadata["unrelated"], json!({"generation": 2}));
    assert_eq!(
        updated.metadata["profiles"],
        json!([{"id": "gpt-live", "label": "GPT Live"}])
    );

    let raw_metadata = {
        let connection = store.connection_for_tests();
        let connection = connection.lock().await;
        connection
            .query_row(
                "SELECT metadata_json FROM provider_accounts WHERE provider_account_id = ?1",
                [&account.provider_account_id],
                |row| row.get::<_, String>(0),
            )
            .expect("raw metadata")
    };
    assert_eq!(
        raw_metadata,
        r#"{"base_url":"https://example.test","unrelated":{"generation":2},"profiles":[{"id":"gpt-live","label":"GPT Live"}],"models_refreshed_at":"100","models_source":"codex_models_endpoint","models_metadata_version":3,"models_client_version":"0.144.1","models_client_version_refreshed_at":"99"}"#
    );
}

#[tokio::test]
async fn catalog_commit_failure_rolls_back_metadata_and_status() {
    let store = super::tests::test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("default provider account");
    store
        .update_provider_account_status(
            &account.provider_account_id,
            ProviderAccountStatus::Unavailable,
            Some("old_error"),
            Some("old message"),
        )
        .await
        .expect("seed unavailable status");
    store
        .update_provider_account_metadata(
            &account.provider_account_id,
            json!({"unrelated": "before"}),
        )
        .await
        .expect("seed metadata");
    let before = store
        .get_provider_account(&account.provider_account_id)
        .await
        .expect("read account")
        .expect("account");
    {
        let connection = store.connection_for_tests();
        let connection = connection.lock().await;
        connection
            .execute_batch(
                r#"
                CREATE TEMP TRIGGER fail_provider_catalog_update
                BEFORE UPDATE ON provider_accounts
                WHEN NEW.provider_account_id = 'provider_account:codex:default'
                BEGIN
                  SELECT RAISE(ABORT, 'forced provider catalog failure');
                END;
                "#,
            )
            .expect("install failure trigger");
    }

    let error = ProviderModelCatalogPersistence::persist_provider_model_catalog(
        &store,
        PersistProviderModelCatalogRequest {
            provider_account_id: account.provider_account_id.clone(),
            profiles: vec![ProviderModelProfile {
                id: "new".to_string(),
                label: "New".to_string(),
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            }],
            refreshed_at_unix: 100,
            source: "codex_models_endpoint".to_string(),
            metadata_version: 3,
            client_version: "0.144.1".to_string(),
            client_version_refreshed_at_unix: None,
            resulting_status: ProviderAccountStatus::Authenticated,
        },
    )
    .await
    .expect_err("trigger must abort catalog update");
    assert_eq!(
        error,
        ProviderPersistenceError::Persistence {
            operation: "persist_provider_model_catalog"
        }
    );
    {
        let connection = store.connection_for_tests();
        let connection = connection.lock().await;
        connection
            .execute_batch("DROP TRIGGER fail_provider_catalog_update")
            .expect("drop failure trigger");
    }

    let after = store
        .get_provider_account(&account.provider_account_id)
        .await
        .expect("read account")
        .expect("account");
    assert_eq!(after.metadata, before.metadata);
    assert_eq!(after.status, before.status);
    assert_eq!(after.last_checked_at, before.last_checked_at);
    assert_eq!(after.last_authenticated_at, before.last_authenticated_at);
    assert_eq!(after.last_error_code, before.last_error_code);
    assert_eq!(after.last_error_message, before.last_error_message);
}

#[tokio::test]
async fn catalog_commit_reports_missing_account_without_writing() {
    let store = super::tests::test_store().await;
    let error = ProviderModelCatalogPersistence::persist_provider_model_catalog(
        &store,
        PersistProviderModelCatalogRequest {
            provider_account_id: "provider_account:missing".to_string(),
            profiles: Vec::new(),
            refreshed_at_unix: 100,
            source: "test".to_string(),
            metadata_version: 3,
            client_version: "0.144.1".to_string(),
            client_version_refreshed_at_unix: None,
            resulting_status: ProviderAccountStatus::Authenticated,
        },
    )
    .await
    .expect_err("missing account");

    assert_eq!(
        error,
        ProviderPersistenceError::AccountNotFound {
            provider_account_id: "provider_account:missing".to_string()
        }
    );
}

#[tokio::test]
async fn account_update_commits_status_and_metadata_together() {
    let store = super::tests::test_store().await;
    let account = ProviderAccountPersistence::create_provider_account(
        &store,
        NewProviderAccount {
            provider_kind: "exa".to_string(),
            display_name: Some("Search".to_string()),
            auth_method: ProviderAuthMethod::SecretInput,
            status: ProviderAccountStatus::Unauthenticated,
            metadata: json!({"secretConfigured": false}),
        },
    )
    .await
    .expect("create account");

    let updated = ProviderAccountPersistence::update_provider_account(
        &store,
        UpdateProviderAccountRequest {
            provider_account_id: account.provider_account_id,
            status: Some(ProviderAccountStatusUpdate {
                status: ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            }),
            metadata: Some(json!({"secretConfigured": true})),
        },
    )
    .await
    .expect("update account");

    assert_eq!(updated.status, ProviderAccountStatus::Authenticated);
    assert_eq!(updated.metadata, json!({"secretConfigured": true}));
    assert!(updated.last_authenticated_at.is_some());
}

#[tokio::test]
async fn protected_account_delete_has_a_typed_error() {
    let store = super::tests::test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("default account");

    let error =
        ProviderAccountPersistence::delete_provider_account(&store, &account.provider_account_id)
            .await
            .expect_err("default account is protected");

    assert_eq!(
        error,
        ProviderPersistenceError::ProtectedAccount {
            provider_account_id: account.provider_account_id
        }
    );
}

#[tokio::test]
async fn account_update_after_delete_cannot_report_false_success() {
    let store = super::tests::test_store().await;
    let account = ProviderAccountPersistence::create_provider_account(
        &store,
        NewProviderAccount {
            provider_kind: "exa".to_string(),
            display_name: None,
            auth_method: ProviderAuthMethod::SecretInput,
            status: ProviderAccountStatus::Unauthenticated,
            metadata: json!({}),
        },
    )
    .await
    .expect("create account");
    assert!(
        ProviderAccountPersistence::delete_provider_account(&store, &account.provider_account_id)
            .await
            .expect("delete account")
    );

    let error = ProviderAccountPersistence::update_provider_account(
        &store,
        UpdateProviderAccountRequest {
            provider_account_id: account.provider_account_id.clone(),
            status: Some(ProviderAccountStatusUpdate {
                status: ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            }),
            metadata: None,
        },
    )
    .await
    .expect_err("deleted account update must fail");
    assert_eq!(
        error,
        ProviderPersistenceError::AccountNotFound {
            provider_account_id: account.provider_account_id
        }
    );
}

#[tokio::test]
async fn corrupted_account_rows_are_persistence_invariants() {
    let store = super::tests::test_store().await;
    let account = ProviderAccountPersistence::create_provider_account(
        &store,
        NewProviderAccount {
            provider_kind: "exa".to_string(),
            display_name: None,
            auth_method: ProviderAuthMethod::SecretInput,
            status: ProviderAccountStatus::Authenticated,
            metadata: json!({}),
        },
    )
    .await
    .expect("create account");
    {
        let connection = store.connection_for_tests();
        let connection = connection.lock().await;
        connection
            .execute(
                "UPDATE provider_accounts SET metadata_json = '{' WHERE provider_account_id = ?1",
                [&account.provider_account_id],
            )
            .expect("corrupt row");
    }
    let read_error =
        ProviderAccountPersistence::provider_account(&store, &account.provider_account_id)
            .await
            .expect_err("corrupt row");
    assert_eq!(
        read_error,
        ProviderPersistenceError::Invariant {
            operation: "provider_account"
        }
    );
    let catalog_error = ProviderModelCatalogPersistence::persist_provider_model_catalog(
        &store,
        PersistProviderModelCatalogRequest {
            provider_account_id: account.provider_account_id.clone(),
            profiles: Vec::new(),
            refreshed_at_unix: 100,
            source: "test".to_string(),
            metadata_version: 3,
            client_version: "1.0.0".to_string(),
            client_version_refreshed_at_unix: None,
            resulting_status: ProviderAccountStatus::Authenticated,
        },
    )
    .await
    .expect_err("corrupt row");
    assert_eq!(
        catalog_error,
        ProviderPersistenceError::Invariant {
            operation: "persist_provider_model_catalog"
        }
    );
    let update_error = ProviderAccountPersistence::update_provider_account(
        &store,
        UpdateProviderAccountRequest {
            provider_account_id: account.provider_account_id.clone(),
            status: None,
            metadata: Some(json!({"changed": true})),
        },
    )
    .await
    .expect_err("corrupt row");
    assert_eq!(
        update_error,
        ProviderPersistenceError::Invariant {
            operation: "update_provider_account"
        }
    );
    let assignment_error =
        ProviderCapabilityAssignmentPersistence::upsert_provider_capability_assignment(
            &store,
            UpsertProviderCapabilityAssignmentRequest::new(
                ToolName::new("web.search").expect("tool"),
                CapabilityId::WebSearch,
                account.provider_account_id,
            )
            .expect("request"),
        )
        .await
        .expect_err("corrupt row");
    assert_eq!(
        assignment_error,
        ProviderPersistenceError::Invariant {
            operation: "upsert_provider_capability_assignment"
        }
    );
}

#[tokio::test]
async fn capability_assignment_and_account_delete_never_leave_a_dangling_row() {
    let home = TempDir::new().expect("temp noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    let writer = NoemaStore::open(&config).await.expect("writer");
    let deleter = NoemaStore::open(&config).await.expect("deleter");
    let account = ProviderAccountPersistence::create_provider_account(
        &writer,
        NewProviderAccount {
            provider_kind: "exa".to_string(),
            display_name: None,
            auth_method: ProviderAuthMethod::SecretInput,
            status: ProviderAccountStatus::Authenticated,
            metadata: json!({}),
        },
    )
    .await
    .expect("account");
    let tool = ToolName::new("web.search").expect("tool");
    let request = UpsertProviderCapabilityAssignmentRequest::new(
        tool.clone(),
        CapabilityId::WebSearch,
        account.provider_account_id.clone(),
    )
    .expect("request");

    let _ = tokio::join!(
        ProviderCapabilityAssignmentPersistence::upsert_provider_capability_assignment(
            &writer, request
        ),
        ProviderAccountPersistence::delete_provider_account(&deleter, &account.provider_account_id)
    );

    let account_exists =
        ProviderAccountPersistence::provider_account(&writer, &account.provider_account_id)
            .await
            .expect("account read")
            .is_some();
    let assignment_exists =
        ProviderCapabilityAssignmentPersistence::provider_capability_assignment(
            &writer,
            &tool,
            CapabilityId::WebSearch,
        )
        .await
        .expect("assignment read")
        .is_some();
    assert!(!assignment_exists || account_exists);
}

#[tokio::test]
async fn catalog_commit_is_visible_through_a_second_store_handle() {
    let home = TempDir::new().expect("temp noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    let writer = NoemaStore::open(&config).await.expect("writer");
    let reader = NoemaStore::open(&config).await.expect("reader");
    let account = writer
        .ensure_default_provider_account()
        .await
        .expect("account");

    ProviderModelCatalogPersistence::persist_provider_model_catalog(
        &writer,
        PersistProviderModelCatalogRequest {
            provider_account_id: account.provider_account_id.clone(),
            profiles: vec![ProviderModelProfile {
                id: "visible".to_string(),
                label: "Visible".to_string(),
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            }],
            refreshed_at_unix: 100,
            source: "test".to_string(),
            metadata_version: 3,
            client_version: "1.0.0".to_string(),
            client_version_refreshed_at_unix: None,
            resulting_status: ProviderAccountStatus::Authenticated,
        },
    )
    .await
    .expect("persist catalog");

    let observed = reader
        .get_provider_account(&account.provider_account_id)
        .await
        .expect("read account")
        .expect("account");
    assert_eq!(observed.status, ProviderAccountStatus::Authenticated);
    assert_eq!(observed.metadata["profiles"][0]["id"], "visible");
}
