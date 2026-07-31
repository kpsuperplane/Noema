//! Provider persistence port transaction and error-contract tests.

use noema_providers::{
    NewProviderAccount, PersistProviderModelCatalogRequest, ProviderAccountPersistence,
    ProviderAccountStatus, ProviderAccountStatusUpdate, ProviderAuthMethod,
    ProviderCapabilityAccountReference, ProviderCapabilityAssignmentKey,
    ProviderCapabilityAssignmentPersistence, ProviderModelCatalogPersistence, ProviderModelProfile,
    ProviderPersistenceError, UpdateProviderAccountRequest,
    UpsertProviderCapabilityAssignmentRequest,
};
use serde_json::json;
use tempfile::TempDir;

use super::{NoemaStore, StoreConfig};

async fn insert_user_managed_codex_account(store: &NoemaStore, account_id: &str) {
    let connection = store.connection_for_tests();
    let connection = connection.lock().await;
    connection
        .execute(
            r#"
            INSERT INTO provider_accounts (
              provider_account_id, provider_kind, account_key, display_name,
              auth_method, is_active, is_default, status, metadata_json
            ) VALUES (?1, 'codex', ?2, 'User managed Codex',
                      'external_manual', 1, 0, 'authenticated', '{}')
            "#,
            rusqlite::params![account_id, account_id],
        )
        .expect("user-managed provider account");
}

fn catalog_request(
    provider_account_id: impl Into<String>,
    profile: Option<(&str, &str)>,
) -> PersistProviderModelCatalogRequest {
    PersistProviderModelCatalogRequest {
        provider_account_id: provider_account_id.into(),
        profiles: profile
            .map(|(id, label)| ProviderModelProfile {
                id: id.to_string(),
                label: label.to_string(),
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
                context_window_tokens: None,
            })
            .into_iter()
            .collect(),
        refreshed_at_unix: 100,
        source: "codex_models_endpoint".to_string(),
        metadata_version: 3,
        client_version: "0.144.1".to_string(),
        client_version_refreshed_at_unix: Some(99),
        resulting_status: ProviderAccountStatus::Authenticated,
    }
}

#[tokio::test]
async fn sqlite_provider_accounts_seed_and_list() {
    let store = super::tests::test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex");
    store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation");
    store
        .ensure_default_openai_provider_account()
        .await
        .expect("openai");

    let ids = ProviderAccountPersistence::active_provider_accounts(&store)
        .await
        .expect("provider accounts")
        .into_iter()
        .map(|account| account.provider_account_id)
        .collect::<Vec<_>>();
    for expected in [
        "provider_account:codex:default",
        "provider_account:foundation_local:default",
        "provider_account:openai:default",
    ] {
        assert!(ids.iter().any(|id| id == expected));
    }

    let missing_id = "provider_account:missing";
    let error = ProviderModelCatalogPersistence::persist_provider_model_catalog(
        &store,
        catalog_request(missing_id, None),
    )
    .await
    .expect_err("missing catalog account");
    assert_eq!(
        error,
        ProviderPersistenceError::AccountNotFound {
            provider_account_id: missing_id.to_string()
        }
    );
    assert!(
        ProviderAccountPersistence::provider_account(&store, missing_id)
            .await
            .expect("missing account read")
            .is_none(),
        "missing catalog commit must not write"
    );

    let updated_account = ProviderAccountPersistence::create_provider_account(
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
    .expect("update account");
    let updated = ProviderAccountPersistence::update_provider_account(
        &store,
        UpdateProviderAccountRequest {
            provider_account_id: updated_account.provider_account_id,
            auth_method: None,
            status: Some(ProviderAccountStatusUpdate {
                status: ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            }),
            metadata: Some(json!({"secretConfigured": true})),
        },
    )
    .await
    .expect("atomic account update");
    assert_eq!(updated.status, ProviderAccountStatus::Authenticated);
    assert_eq!(updated.metadata, json!({"secretConfigured": true}));
    assert!(updated.last_authenticated_at.is_some());

    assert!(
        ProviderAccountPersistence::delete_provider_account(
            &store,
            "provider_account:foundation_local:default",
        )
        .await
        .expect("unreferenced canonical account can be compensated")
    );

    store.ensure_default_actors().await.expect("actors");
    let referenced_id = "provider_account:codex:port-reference";
    insert_user_managed_codex_account(&store, referenced_id).await;
    let ready = super::tests::ready_provider_selection(super::tests::provider_selection(
        "codex",
        referenced_id,
        "gpt-test",
        "account_delete_port_test",
    ));
    store
        .upsert_agent_runtime_preference_with_ready_selection(
            crate::NewAgentRuntimePreference {
                agent_id: "agent:primary".to_string(),
                provider_kind: "codex".to_string(),
                provider_account_id: referenced_id.to_string(),
                selection: noema_providers::ModelPreferenceSelection::ExplicitProfile {
                    model_profile: "gpt-test".to_string(),
                    reasoning_effort: None,
                },
            },
            &ready,
        )
        .await
        .expect("canonical reference");
    assert_eq!(
        ProviderAccountPersistence::delete_provider_account(&store, referenced_id)
            .await
            .expect_err("referenced port delete"),
        ProviderPersistenceError::AccountInUse {
            provider_account_id: referenced_id.to_string()
        }
    );
    assert!(matches!(
        store
            .delete_provider_account(referenced_id)
            .await
            .expect_err("direct referenced delete"),
        crate::StoreError::ProviderAccountInUse { provider_account_id }
            if provider_account_id == referenced_id
    ));

    let deleted = ProviderAccountPersistence::create_provider_account(
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
    .expect("deleted account");
    assert!(
        ProviderAccountPersistence::delete_provider_account(&store, &deleted.provider_account_id)
            .await
            .expect("delete")
    );
    assert_eq!(
        ProviderAccountPersistence::update_provider_account(
            &store,
            UpdateProviderAccountRequest {
                provider_account_id: deleted.provider_account_id.clone(),
                auth_method: None,
                status: Some(ProviderAccountStatusUpdate {
                    status: ProviderAccountStatus::Authenticated,
                    error_code: None,
                    error_message: None,
                }),
                metadata: None,
            },
        )
        .await
        .expect_err("update after delete"),
        ProviderPersistenceError::AccountNotFound {
            provider_account_id: deleted.provider_account_id
        }
    );

    store
        .update_provider_account_status(
            "provider_account:codex:default",
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated metadata");
    let hosted_selection = super::tests::exact_provider_selection(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        noema_providers::provider_account_instance_key("provider_account:codex:default")
            .expect("hosted instance key"),
        "unregistered_hosted_test",
    );
    assert!(matches!(
        store
            .initialize_missing_provider_selections(&hosted_selection, None)
            .await
            .expect_err("metadata is not registry readiness"),
        crate::StoreError::ConfiguredDefaultUnresolvable { reason }
            if reason == "canonical provider selections are incomplete and the configured instance is not ready"
    ));

    let home = TempDir::new().expect("two-handle root");
    let config = StoreConfig::new(home.path().join("db/noema.sqlite3"));
    let writer = NoemaStore::open(&config).await.expect("writer");
    let reader = NoemaStore::open(&config).await.expect("reader");
    let account = writer
        .ensure_default_provider_account()
        .await
        .expect("account");
    ProviderModelCatalogPersistence::persist_provider_model_catalog(
        &writer,
        catalog_request(&account.provider_account_id, Some(("visible", "Visible"))),
    )
    .await
    .expect("catalog commit");
    let observed = reader
        .get_provider_account(&account.provider_account_id)
        .await
        .expect("second-handle read")
        .expect("account");
    assert_eq!(observed.status, ProviderAccountStatus::Authenticated);
    assert_eq!(observed.metadata["profiles"][0]["id"], "visible");
}

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
        catalog_request(stale.provider_account_id, Some(("gpt-live", "GPT Live"))),
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
        catalog_request(account.provider_account_id.clone(), Some(("new", "New"))),
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
async fn canonical_reference_write_and_account_delete_never_leave_a_dangling_selection() {
    let home = TempDir::new().expect("temp store root");
    let config = StoreConfig::new(home.path().join("db/noema.sqlite3"));
    let writer = NoemaStore::open(&config).await.expect("writer");
    let deleter = NoemaStore::open(&config).await.expect("deleter");
    writer.ensure_default_actors().await.expect("actors");
    let provider_account_id = "provider_account:codex:concurrent-reference";
    insert_user_managed_codex_account(&writer, provider_account_id).await;

    let preference = crate::NewAgentRuntimePreference {
        agent_id: "agent:primary".to_string(),
        provider_kind: "codex".to_string(),
        provider_account_id: provider_account_id.to_string(),
        selection: noema_providers::ModelPreferenceSelection::ExplicitProfile {
            model_profile: "gpt-test".to_string(),
            reasoning_effort: None,
        },
    };
    let ready_selection = super::tests::ready_provider_selection(super::tests::provider_selection(
        "codex",
        provider_account_id,
        "gpt-test",
        "concurrent_account_reference",
    ));
    let (_write_result, _delete_result) = tokio::join!(
        writer.upsert_agent_runtime_preference_with_ready_selection(preference, &ready_selection,),
        ProviderAccountPersistence::delete_provider_account(&deleter, provider_account_id),
    );

    let account_exists = writer
        .get_provider_account(provider_account_id)
        .await
        .expect("account read")
        .is_some();
    let selection_exists = writer
        .get_agent_runtime_preference("agent:primary")
        .await
        .expect("selection read")
        .is_some();
    assert!(!selection_exists || account_exists);
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
        catalog_request(account.provider_account_id.clone(), None),
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
            auth_method: None,
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
}

#[tokio::test]
async fn capability_assignment_and_account_delete_never_leave_a_dangling_row() {
    let home = TempDir::new().expect("temp store root");
    let config = StoreConfig::new(home.path().join("db/noema.sqlite3"));
    let writer = NoemaStore::open(&config).await.expect("writer");
    let deleter = NoemaStore::open(&config).await.expect("deleter");
    let missing_provider_account_id = "provider_account:exa:missing";
    let error = ProviderCapabilityAssignmentPersistence::upsert_provider_capability_assignment(
        &writer,
        UpsertProviderCapabilityAssignmentRequest::from_storage_values(
            "web.search",
            "web.search",
            ProviderCapabilityAccountReference::persisted(missing_provider_account_id),
        )
        .expect("missing persisted account request"),
    )
    .await
    .expect_err("missing persisted account");
    assert_eq!(
        error,
        ProviderPersistenceError::AccountNotFound {
            provider_account_id: missing_provider_account_id.to_string(),
        }
    );

    let system_provider_account_id = "provider_account:direct_http:system";
    let system_assignment =
        ProviderCapabilityAssignmentPersistence::upsert_provider_capability_assignment(
            &writer,
            UpsertProviderCapabilityAssignmentRequest::from_storage_values(
                "web.fetch",
                "web.fetch",
                ProviderCapabilityAccountReference::validated_system(system_provider_account_id)
                    .expect("system account"),
            )
            .expect("system assignment request"),
        )
        .await
        .expect("save system assignment");
    assert_eq!(
        system_assignment.provider_account_id,
        system_provider_account_id
    );

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
    let key = ProviderCapabilityAssignmentKey::from_storage_values("web.search", "web.search")
        .expect("assignment key");
    let request = UpsertProviderCapabilityAssignmentRequest::from_storage_values(
        "web.search",
        "web.search",
        ProviderCapabilityAccountReference::persisted(account.provider_account_id.clone()),
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
        ProviderCapabilityAssignmentPersistence::provider_capability_assignment(&writer, &key)
            .await
            .expect("assignment read")
            .is_some();
    assert!(!assignment_exists || account_exists);
}
