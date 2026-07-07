use serde_json::json;
use surrealdb::types::SurrealValue;
use tempfile::TempDir;

mod claims;
mod mcp;

use super::{NoemaStore, StoreConfig, schema::STORE_SCHEMA_SQL};
use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversation, NewConversationItem,
    NewConversationTurn, ObjectRef, ProviderAccountStatus, ReplayMode, StoreError,
};

#[tokio::test]
async fn opens_embedded_store_under_noema_db_dir() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    let store = NoemaStore::open(&config).await.expect("open store");

    assert!(paths.db_dir().exists());
    assert_eq!(store.schema_version().await.expect("schema version"), 1);
}

#[tokio::test]
async fn embedded_store_config_is_stable_for_reopen() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    assert_eq!(config.path, paths.db_dir());
    assert_eq!(StoreConfig::from_paths(&paths), config);
}

#[tokio::test]
async fn strict_schema_rejects_invalid_sensitivity() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('claims', 'invalid_sensitivity') SET
              claim_id = 'claim:invalid-sensitivity',
              subject_entity_id = 'entity:human-local',
              predicate_id = 'likes',
              fact = 'invalid sensitivity test claim',
              status = 'candidate',
              sensitivity = 'galaxy',
              retrieval_hints = {},
              policy_overrides = {},
              metadata = {},
              updated_at = time::now();
            "#,
        )
        .await
        .expect("invalid claim query")
        .check()
        .expect_err("invalid sensitivity should be rejected");

    assert!(
        error.to_string().contains("sensitivity") || error.to_string().contains("galaxy"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn strict_schema_accepts_minimal_claim_with_datetime_fields() {
    let store = test_store().await;

    store
        .db()
        .query(
            r#"
            CREATE type::record('claims', 'valid_datetime_claim') SET
              claim_id = 'claim:valid-datetime',
              subject_entity_id = 'entity:human-local',
              object_entity_id = 'entity:ice-cream',
              predicate_id = 'likes',
              fact = 'Kevin likes ice cream.',
              status = 'candidate',
              sensitivity = 'normal',
              valid_from = time::now(),
              valid_to = time::now(),
              observed_at = time::now(),
              dedupe_fingerprint = 'claim-fingerprint:valid-datetime',
              retrieval_hints = {},
              policy_overrides = {},
              metadata = {},
              updated_at = time::now();
            "#,
        )
        .await
        .expect("valid datetime claim query")
        .check()
        .expect("valid datetime claim should insert");
}

#[tokio::test]
async fn bootstrap_refreshes_stale_claim_timestamp_field_definitions() {
    let store = test_store().await;
    store
        .db()
        .query(
            r#"
            DEFINE FIELD OVERWRITE valid_from ON TABLE claims TYPE option<string>;
            DEFINE FIELD OVERWRITE valid_to ON TABLE claims TYPE option<string>;
            DEFINE FIELD OVERWRITE observed_at ON TABLE claims TYPE option<string>;
            "#,
        )
        .await
        .expect("stale claim timestamp field query")
        .check()
        .expect("stale claim timestamp fields should define");
    store
        .db()
        .query(STORE_SCHEMA_SQL)
        .await
        .expect("replay schema bootstrap")
        .check()
        .expect("schema bootstrap should replay");

    store
        .db()
        .query(
            r#"
            CREATE type::record('claims', 'refreshed_datetime_claim') SET
              claim_id = 'claim:refreshed-datetime',
              subject_entity_id = 'entity:human-local',
              object_entity_id = 'entity:trains',
              predicate_id = 'likes',
              fact = 'Kevin likes trains.',
              status = 'candidate',
              sensitivity = 'normal',
              valid_from = time::now(),
              valid_to = time::now(),
              observed_at = time::now(),
              dedupe_fingerprint = 'claim-fingerprint:refreshed-datetime',
              retrieval_hints = {},
              policy_overrides = {},
              metadata = {},
              updated_at = time::now();
            "#,
        )
        .await
        .expect("refreshed datetime claim query")
        .check()
        .expect("refreshed datetime fields should accept datetime values");
}

#[tokio::test]
async fn strict_schema_rejects_invalid_claim_timestamp() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('claims', 'invalid_timestamp_claim') SET
              claim_id = 'claim:invalid-timestamp',
              subject_entity_id = 'entity:human-local',
              object_entity_id = 'entity:invalid-timestamp',
              predicate_id = 'likes',
              fact = 'invalid timestamp test claim',
              status = 'candidate',
              sensitivity = 'normal',
              observed_at = 'not-a-timestamp',
              dedupe_fingerprint = 'claim-fingerprint:invalid-timestamp',
              retrieval_hints = {},
              policy_overrides = {},
              metadata = {},
              updated_at = time::now();
            "#,
        )
        .await
        .expect("invalid timestamp query")
        .check()
        .expect_err("invalid timestamp should be rejected");

    assert!(
        error.to_string().contains("observed_at") || error.to_string().contains("datetime"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn strict_schema_rejects_duplicate_claim_dedupe_fingerprint() {
    let store = test_store().await;

    store
        .db()
        .query(
            r#"
            CREATE type::record('claims', 'first_dedupe_claim') SET
              claim_id = 'claim:first-dedupe',
              subject_entity_id = 'entity:human-local',
              object_entity_id = 'entity:first-dedupe',
              predicate_id = 'likes',
              fact = 'first dedupe test claim',
              status = 'candidate',
              sensitivity = 'normal',
              dedupe_fingerprint = 'claim-fingerprint:duplicate-test',
              retrieval_hints = {},
              policy_overrides = {},
              metadata = {},
              updated_at = time::now();
            "#,
        )
        .await
        .expect("first dedupe query")
        .check()
        .expect("first dedupe claim should insert");

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('claims', 'second_dedupe_claim') SET
              claim_id = 'claim:second-dedupe',
              subject_entity_id = 'entity:human-local',
              object_entity_id = 'entity:second-dedupe',
              predicate_id = 'likes',
              fact = 'second dedupe test claim',
              status = 'candidate',
              sensitivity = 'normal',
              dedupe_fingerprint = 'claim-fingerprint:duplicate-test',
              retrieval_hints = {},
              policy_overrides = {},
              metadata = {},
              updated_at = time::now();
            "#,
        )
        .await
        .expect("second dedupe query")
        .check()
        .expect_err("duplicate dedupe fingerprint should be rejected");

    assert!(
        error.to_string().contains("dedupe_fingerprint")
            || error.to_string().contains("claims_dedupe_fingerprint"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn strict_schema_rejects_invalid_evidence_authority() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('supported_by', 'invalid_authority') SET
              relation_id = 'evidence:invalid-authority',
              claim_id = 'claim:test',
              source_kind = 'item',
              source_item_id = 'item:test',
              source_object_type = NONE,
              source_object_id = NONE,
              authority = 'rumor',
              excerpt = 'invalid authority test evidence',
              observed_at = time::now(),
              created_by = 'agent:primary',
              metadata = {};
            "#,
        )
        .await
        .expect("invalid evidence query")
        .check()
        .expect_err("invalid evidence authority should be rejected");

    assert!(
        error.to_string().contains("authority") || error.to_string().contains("rumor"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn strict_schema_rejects_invalid_evidence_observed_at() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('supported_by', 'invalid_observed_at') SET
              relation_id = 'evidence:invalid-observed-at',
              claim_id = 'claim:test',
              source_kind = 'item',
              source_item_id = 'item:test',
              source_object_type = NONE,
              source_object_id = NONE,
              authority = 'explicit_human_statement',
              excerpt = 'invalid observed_at test evidence',
              observed_at = '123',
              created_by = 'agent:primary',
              metadata = {};
            "#,
        )
        .await
        .expect("invalid evidence observed_at query")
        .check()
        .expect_err("invalid evidence observed_at should be rejected");

    assert!(
        error.to_string().contains("observed_at") || error.to_string().contains("datetime"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn strict_schema_rejects_evidence_without_source() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('supported_by', 'missing_source') SET
              relation_id = 'evidence:missing-source',
              claim_id = 'claim:test',
              authority = 'explicit_human_statement',
              excerpt = 'missing source test evidence',
              observed_at = time::now(),
              created_by = 'agent:primary',
              metadata = {};
            "#,
        )
        .await
        .expect("missing source evidence query")
        .check()
        .expect_err("missing evidence source should be rejected");

    assert!(
        error.to_string().contains("source_item_id")
            || error.to_string().contains("source_object")
            || error.to_string().contains("source_kind"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn strict_schema_rejects_evidence_with_both_source_shapes() {
    let store = test_store().await;

    let error = store
        .db()
        .query(
            r#"
            CREATE type::record('supported_by', 'both_sources') SET
              relation_id = 'evidence:both-sources',
              claim_id = 'claim:test',
              source_kind = 'item',
              source_item_id = 'item:test',
              source_object_type = 'conversation',
              source_object_id = 'conversation:test',
              authority = 'explicit_human_statement',
              excerpt = 'both source shapes test evidence',
              observed_at = time::now(),
              created_by = 'agent:primary',
              metadata = {};
            "#,
        )
        .await
        .expect("both sources evidence query")
        .check()
        .expect_err("evidence with both source shapes should be rejected");

    assert!(
        error.to_string().contains("source_item_id") || error.to_string().contains("source_object"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn strict_schema_accepts_valid_evidence_source_shape() {
    let store = test_store().await;

    store
        .db()
        .query(
            r#"
            CREATE type::record('supported_by', 'valid_source') SET
              relation_id = 'evidence:valid-source',
              claim_id = 'claim:test',
              source_kind = 'object',
              source_item_id = NONE,
              source_object_type = 'conversation',
              source_object_id = 'conversation:test',
              authority = 'explicit_human_statement',
              excerpt = 'valid source shape test evidence',
              observed_at = time::now(),
              created_by = 'agent:primary',
              metadata = {};
            "#,
        )
        .await
        .expect("valid evidence source query")
        .check()
        .expect("valid evidence source should insert");
}

#[tokio::test]
async fn built_in_personal_predicates_are_seeded() {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct PredicateRow {
        label: String,
        default_sensitivity: String,
        inverse_behavior: String,
        proactivity_default: i64,
        synonym_hints: Vec<String>,
    }

    let store = test_store().await;
    let mut response = store
        .db()
        .query(
            r#"
            SELECT label, default_sensitivity, inverse_behavior, proactivity_default, synonym_hints
            FROM predicates
            WHERE predicate_id = 'likes'
            LIMIT 1;
            "#,
        )
        .await
        .expect("select predicate");
    let rows: Vec<PredicateRow> = response.take(0).expect("predicate rows");
    let likes = rows.first().expect("likes predicate");

    assert_eq!(likes.label, "likes");
    assert_eq!(likes.default_sensitivity, "normal");
    assert_eq!(likes.inverse_behavior, "none");
    assert_eq!(likes.proactivity_default, 2);
    assert!(likes.synonym_hints.contains(&"enjoys".to_string()));
}

#[tokio::test]
async fn built_in_predicate_seed_is_idempotent_when_bootstrap_replays() {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct PredicateIdRow {
        predicate_id: String,
    }

    let store = test_store().await;
    let first = seeded_predicate_ids(&store).await;

    store
        .db()
        .query(STORE_SCHEMA_SQL)
        .await
        .expect("replay schema bootstrap")
        .check()
        .expect("schema bootstrap should replay");
    let second = seeded_predicate_ids(&store).await;

    assert_eq!(
        first,
        vec![
            "dislikes",
            "has_note",
            "likes",
            "prefers",
            "prefers_interaction_style",
            "uses",
            "works_on",
        ]
    );
    assert_eq!(first, second);

    async fn seeded_predicate_ids(store: &NoemaStore) -> Vec<String> {
        let mut response = store
            .db()
            .query(
                r#"
                SELECT predicate_id
                FROM predicates
                WHERE predicate_id IN ['likes', 'dislikes', 'prefers', 'uses', 'works_on', 'prefers_interaction_style', 'has_note']
                ORDER BY predicate_id ASC;
                "#,
            )
            .await
            .expect("select seeded predicates");
        let rows: Vec<PredicateIdRow> = response.take(0).expect("seeded predicate rows");
        rows.into_iter().map(|row| row.predicate_id).collect()
    }
}

#[tokio::test]
async fn default_provider_account_round_trips_status() {
    let store = test_store().await;

    let account = store
        .ensure_default_provider_account()
        .await
        .expect("default account");
    assert_eq!(
        account.provider_account_id,
        "provider_account:codex:default"
    );
    assert_eq!(account.status, ProviderAccountStatus::Unknown);

    store
        .update_provider_account_status(
            &account.provider_account_id,
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("update status");

    let fetched = store
        .active_provider_account("codex")
        .await
        .expect("active account")
        .expect("account exists");
    assert_eq!(fetched.provider_account_id, account.provider_account_id);
    assert_eq!(fetched.status, ProviderAccountStatus::Authenticated);
}

#[tokio::test]
async fn provider_accounts_include_derived_capabilities() {
    let store = test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("default account");

    assert_eq!(account.provider_kind, "codex");
    assert!(
        account
            .capabilities
            .iter()
            .any(|capability| capability.capability_id.as_str() == "model.generate")
    );
    assert!(
        !account
            .capabilities
            .iter()
            .any(|capability| capability.capability_id.as_str() == "web.search")
    );

    let system_accounts = store.system_provider_accounts();
    assert!(system_accounts.iter().any(|account| {
        account.provider_kind == "duckduckgo_public"
            && account
                .capabilities
                .iter()
                .any(|capability| capability.capability_id.as_str() == "web.search")
    }));
    assert!(system_accounts.iter().any(|account| {
        account.provider_kind == "direct_http"
            && account
                .capabilities
                .iter()
                .any(|capability| capability.capability_id.as_str() == "web.fetch")
    }));
}

#[tokio::test]
async fn provider_account_catalog_lists_exa_without_creating_account() {
    let store = test_store().await;

    let catalog = store.provider_account_catalog();
    let exa = catalog
        .iter()
        .find(|entry| entry.provider_kind == "exa")
        .expect("exa catalog entry");
    assert_eq!(exa.display_name, "Exa");
    assert_eq!(exa.auth_method, crate::ProviderAuthMethod::SecretInput);
    assert!(exa.capabilities.iter().any(|capability| {
        capability.capability_id == crate::provider::CapabilityId::WebSearch
    }));
    assert!(
        store
            .active_provider_account("exa")
            .await
            .expect("read")
            .is_none()
    );
}

#[tokio::test]
async fn create_provider_account_generates_stable_key_and_capabilities() {
    let store = test_store().await;

    let account = store
        .create_provider_account(crate::store::NewProviderAccount {
            provider_kind: "exa".to_string(),
            display_name: Some("Research Search".to_string()),
            auth_method: crate::ProviderAuthMethod::SecretInput,
            status: crate::ProviderAccountStatus::Authenticated,
            metadata: serde_json::json!({ "secretConfigured": true }),
        })
        .await
        .expect("created account");

    assert_eq!(account.provider_kind, "exa");
    assert_eq!(account.display_name, "Research Search");
    assert!(account.account_key.starts_with("acct_"));
    assert_eq!(
        account.provider_account_id,
        format!("provider_account:exa:{}", account.account_key)
    );
    assert!(!account.is_default);
    assert!(
        account.capabilities.iter().any(|capability| {
            capability.capability_id == crate::provider::CapabilityId::WebFetch
        })
    );
}

#[tokio::test]
async fn delete_provider_account_removes_account_and_capability_bindings() {
    let store = test_store().await;
    let account = store
        .create_provider_account(crate::store::NewProviderAccount {
            provider_kind: "exa".to_string(),
            display_name: Some("Research Search".to_string()),
            auth_method: crate::ProviderAuthMethod::SecretInput,
            status: crate::ProviderAccountStatus::Authenticated,
            metadata: serde_json::json!({ "secretConfigured": true }),
        })
        .await
        .expect("created account");

    store
        .upsert_provider_capability_binding(
            "web.search",
            "web.search",
            &account.provider_account_id,
        )
        .await
        .expect("save binding");

    let deleted = store
        .delete_provider_account(&account.provider_account_id)
        .await
        .expect("delete account");

    assert!(deleted);
    assert!(
        store
            .get_provider_account(&account.provider_account_id)
            .await
            .expect("load provider")
            .is_none()
    );
    assert!(
        store
            .provider_capability_binding("web.search", "web.search")
            .await
            .expect("load binding")
            .is_none()
    );
}

#[tokio::test]
async fn delete_provider_account_rejects_default_accounts() {
    let store = test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");

    let error = store
        .delete_provider_account(&account.provider_account_id)
        .await
        .expect_err("default provider accounts are not delete targets");

    assert!(matches!(
        error,
        StoreError::InvalidEnum { kind, value }
            if kind == "provider_account_delete_target"
                && value == account.provider_account_id
    ));
    assert!(
        store
            .get_provider_account(&account.provider_account_id)
            .await
            .expect("load provider")
            .is_some()
    );
}

#[tokio::test]
async fn provider_capability_binding_round_trips_web_search() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");

    let saved = store
        .upsert_provider_capability_binding(
            "web.search",
            "web.search",
            "provider_account:duckduckgo_public:system",
        )
        .await
        .expect("save binding");

    assert_eq!(saved.tool_name, "web.search");
    assert_eq!(saved.capability_id, "web.search");
    assert_eq!(
        saved.provider_account_id,
        "provider_account:duckduckgo_public:system"
    );

    let loaded = store
        .provider_capability_binding("web.search", "web.search")
        .await
        .expect("load binding")
        .expect("binding exists");

    assert_eq!(
        loaded.provider_account_id,
        "provider_account:duckduckgo_public:system"
    );
}

#[tokio::test]
async fn provider_capability_binding_round_trips_web_fetch() {
    let store = test_store().await;

    let saved = store
        .upsert_provider_capability_binding(
            "web.fetch",
            "web.fetch",
            "provider_account:direct_http:system",
        )
        .await
        .expect("save binding");

    assert_eq!(saved.tool_name, "web.fetch");
    assert_eq!(saved.capability_id, "web.fetch");
    assert_eq!(
        saved.provider_account_id,
        "provider_account:direct_http:system"
    );
}

#[tokio::test]
async fn provider_capability_binding_rejects_missing_provider_account() {
    let store = test_store().await;

    let error = store
        .upsert_provider_capability_binding(
            "web.search",
            "web.search",
            "provider_account:missing:default",
        )
        .await
        .expect_err("missing provider account should fail");

    assert!(matches!(
        error,
        StoreError::ProviderAccountNotFound { provider_account_id }
            if provider_account_id == "provider_account:missing:default"
    ));
}

#[tokio::test]
async fn provider_capability_binding_rejects_provider_account_without_capability() {
    let store = test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");

    let error = store
        .upsert_provider_capability_binding(
            "web.search",
            "web.search",
            &account.provider_account_id,
        )
        .await
        .expect_err("wrong provider capability should fail");

    assert!(matches!(
        error,
        StoreError::InvalidEnum { kind, value }
            if kind == "provider_capability_binding_provider_account"
                && value == account.provider_account_id
    ));
}

#[tokio::test]
async fn provider_capability_binding_rejects_mismatched_tool_and_capability() {
    let store = test_store().await;

    let error = store
        .upsert_provider_capability_binding(
            "web.fetch",
            "web.search",
            "provider_account:duckduckgo_public:system",
        )
        .await
        .expect_err("mismatched tool/capability should fail");

    assert!(matches!(
        error,
        StoreError::InvalidEnum { kind, value }
            if kind == "provider_capability_binding_pair" && value == "web.fetch:web.search"
    ));
}

#[tokio::test]
async fn provider_capability_binding_repeated_upsert_is_stable() {
    let store = test_store().await;

    let first = store
        .upsert_provider_capability_binding(
            "web.search",
            "web.search",
            "provider_account:duckduckgo_public:system",
        )
        .await
        .expect("first save");
    let second = store
        .upsert_provider_capability_binding(
            "web.search",
            "web.search",
            "provider_account:duckduckgo_public:system",
        )
        .await
        .expect("second save");

    assert_eq!(first, second);
}

#[test]
fn web_tool_specs_do_not_gain_provider_argument() {
    let search = crate::search::tool::web_search_tool_spec().expect("search spec");
    let fetch = crate::web_fetch::tool::web_fetch_tool_spec().expect("fetch spec");

    assert!(
        search.input_schema.as_value()["properties"]
            .get("provider")
            .is_none()
    );
    assert!(
        fetch.input_schema.as_value()["properties"]
            .get("provider")
            .is_none()
    );
}

#[tokio::test]
async fn default_foundation_local_provider_account_is_available_metadata() {
    let store = test_store().await;

    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation provider account");

    assert_eq!(
        account.provider_account_id,
        "provider_account:foundation_local:default"
    );
    assert_eq!(account.provider_kind, "foundation_local");
    assert_eq!(account.account_key, "default");
    assert_eq!(account.display_name, "Apple Foundation Models");
    assert_eq!(account.auth_method, crate::ProviderAuthMethod::None);
    assert_eq!(account.status, ProviderAccountStatus::Unknown);
}

#[tokio::test]
async fn active_default_provider_accounts_lists_codex_and_foundation() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");

    let accounts = store
        .active_default_provider_accounts()
        .await
        .expect("provider accounts");

    let kinds = accounts
        .iter()
        .map(|account| account.provider_kind.as_str())
        .collect::<Vec<_>>();
    assert_eq!(kinds, vec!["codex", "foundation_local"]);
}

#[tokio::test]
async fn agent_runtime_preference_round_trips() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("default actors");
    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");

    let saved = store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: account.provider_account_id.clone(),
            model_profile: "default".to_string(),
        })
        .await
        .expect("save preference");

    assert_eq!(saved.agent_id, "agent:primary");
    assert_eq!(saved.provider_kind, "foundation_local");
    assert_eq!(saved.provider_account_id, account.provider_account_id);
    assert_eq!(saved.model_profile, "default");

    let loaded = store
        .get_agent_runtime_preference("agent:primary")
        .await
        .expect("load preference")
        .expect("preference exists");

    assert_eq!(loaded, saved);
}

#[tokio::test]
async fn agent_runtime_preference_rejects_unknown_provider_account() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("default actors");

    let error = store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: "provider_account:foundation_local:missing".to_string(),
            model_profile: "default".to_string(),
        })
        .await
        .expect_err("unknown provider account should fail");

    assert!(matches!(
        error,
        StoreError::ProviderAccountNotFound { provider_account_id }
            if provider_account_id == "provider_account:foundation_local:missing"
    ));
}

#[tokio::test]
async fn auxiliary_model_preferences_web_fetch_summarizer_round_trips() {
    let store = test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");

    let saved = store
        .upsert_auxiliary_model_preference(crate::NewAuxiliaryModelPreference {
            task_id: crate::WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id.clone(),
            model_profile: "gpt-5.4-mini".to_string(),
        })
        .await
        .expect("save preference");

    assert_eq!(saved.task_id, crate::WEB_FETCH_SUMMARIZER_TASK_ID);
    assert_eq!(saved.provider_kind, "codex");
    assert_eq!(saved.provider_account_id, account.provider_account_id);
    assert_eq!(saved.model_profile, "gpt-5.4-mini");

    let loaded = store
        .get_auxiliary_model_preference(crate::WEB_FETCH_SUMMARIZER_TASK_ID)
        .await
        .expect("load preference")
        .expect("preference exists");

    assert_eq!(loaded, saved);
}

#[tokio::test]
async fn auxiliary_model_preferences_tool_progress_audit_round_trips() {
    let store = test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("account");

    let saved = store
        .upsert_auxiliary_model_preference(crate::NewAuxiliaryModelPreference {
            task_id: crate::store::TOOL_PROGRESS_AUDIT_TASK_ID.to_string(),
            provider_kind: account.provider_kind.clone(),
            provider_account_id: account.provider_account_id.clone(),
            model_profile: "gpt-5.4-mini".to_string(),
        })
        .await
        .expect("save preference");

    assert_eq!(saved.task_id, crate::store::TOOL_PROGRESS_AUDIT_TASK_ID);
    assert_eq!(saved.provider_kind, account.provider_kind);
    assert_eq!(saved.provider_account_id, account.provider_account_id);
    assert_eq!(saved.model_profile, "gpt-5.4-mini");

    let loaded = store
        .get_auxiliary_model_preference(crate::store::TOOL_PROGRESS_AUDIT_TASK_ID)
        .await
        .expect("load preference")
        .expect("preference exists");
    assert_eq!(loaded, saved);
}

#[tokio::test]
async fn auxiliary_model_preferences_rejects_unknown_provider_account() {
    let store = test_store().await;

    let error = store
        .upsert_auxiliary_model_preference(crate::NewAuxiliaryModelPreference {
            task_id: crate::WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:missing".to_string(),
            model_profile: "gpt-5.4-mini".to_string(),
        })
        .await
        .expect_err("unknown provider account should fail");

    assert!(matches!(
        error,
        StoreError::ProviderAccountNotFound { provider_account_id }
            if provider_account_id == "provider_account:codex:missing"
    ));
}

#[tokio::test]
async fn auxiliary_model_preferences_rejects_provider_kind_mismatch() {
    let store = test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");

    let error = store
        .upsert_auxiliary_model_preference(crate::NewAuxiliaryModelPreference {
            task_id: crate::WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
            provider_kind: "openai".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.4-mini".to_string(),
        })
        .await
        .expect_err("provider mismatch should fail");

    assert!(matches!(
        error,
        StoreError::InvalidEnum {
            kind: "auxiliary_model_preference_provider_kind",
            value
        } if value == "openai"
    ));
}

#[tokio::test]
async fn default_primary_agent_starts_unnamed() {
    let store = test_store().await;

    store.ensure_default_actors().await.expect("actors");

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("primary agent exists");
    assert_eq!(agent.agent_id, "agent:primary");
    assert_eq!(agent.display_name, None);
}

#[tokio::test]
async fn ensure_default_actors_preserves_existing_agent_name() {
    let store = test_store().await;

    store.ensure_default_actors().await.expect("actors");
    store
        .update_agent_display_name("agent:primary", "Mira")
        .await
        .expect("update name");
    store.ensure_default_actors().await.expect("actors again");

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("primary agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Mira"));
}

#[tokio::test]
async fn ensure_default_actors_reuses_primary_agent_created_through_store_api() {
    let store = test_store().await;

    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:primary".to_string(),
            display_name: Some("Mira".to_string()),
        })
        .await
        .expect("create primary agent");
    store.ensure_default_actors().await.expect("actors");

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("primary agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Mira"));
}

#[tokio::test]
async fn list_agents_returns_primary_first() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("default actors");
    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:zeta".to_string(),
            display_name: Some("Zeta".to_string()),
        })
        .await
        .expect("zeta agent");
    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:alpha".to_string(),
            display_name: Some("Alpha".to_string()),
        })
        .await
        .expect("alpha agent");

    let agents = store.list_agents().await.expect("agents");

    let ids = agents
        .iter()
        .map(|agent| agent.agent_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["agent:primary", "agent:alpha", "agent:zeta"]);
}

#[tokio::test]
async fn list_agents_sorts_unnamed_agents_by_id_after_named_agents() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("default actors");
    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:unnamed-b".to_string(),
            display_name: None,
        })
        .await
        .expect("unnamed b");
    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:named".to_string(),
            display_name: Some("Named".to_string()),
        })
        .await
        .expect("named");
    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:unnamed-a".to_string(),
            display_name: None,
        })
        .await
        .expect("unnamed a");

    let agents = store.list_agents().await.expect("agents");

    let ids = agents
        .iter()
        .map(|agent| agent.agent_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "agent:primary",
            "agent:named",
            "agent:unnamed-a",
            "agent:unnamed-b"
        ]
    );
}

#[tokio::test]
async fn agent_display_name_updates_trim_and_preserve_casing() {
    let store = test_store().await;

    let agent = store
        .create_agent(crate::NewAgent {
            agent_id: "agent:test-naming".to_string(),
            display_name: None,
        })
        .await
        .expect("create agent");
    assert_eq!(agent.display_name, None);

    let updated = store
        .update_agent_display_name("agent:test-naming", "  Mira Sol  ")
        .await
        .expect("update name");

    assert_eq!(updated.display_name.as_deref(), Some("Mira Sol"));
}

#[tokio::test]
async fn agent_display_name_update_rejects_whitespace_only_and_preserves_existing_name() {
    let store = test_store().await;

    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:test-empty-update".to_string(),
            display_name: Some("Mira".to_string()),
        })
        .await
        .expect("create agent");

    let error = store
        .update_agent_display_name("agent:test-empty-update", "   ")
        .await
        .expect_err("empty display name should be rejected");

    assert!(matches!(error, StoreError::AgentDisplayNameEmpty));
    let agent = store
        .get_agent("agent:test-empty-update")
        .await
        .expect("get agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Mira"));
}

#[tokio::test]
async fn create_agent_rejects_whitespace_only_display_name() {
    let store = test_store().await;

    let error = store
        .create_agent(crate::NewAgent {
            agent_id: "agent:test-empty-create".to_string(),
            display_name: Some("   ".to_string()),
        })
        .await
        .expect_err("empty display name should be rejected");

    assert!(matches!(error, StoreError::AgentDisplayNameEmpty));
    assert!(
        store
            .get_agent("agent:test-empty-create")
            .await
            .expect("get agent")
            .is_none()
    );
}

#[tokio::test]
async fn create_agent_distinguishes_ids_with_punctuation_differences() {
    let store = test_store().await;

    let dashed = store
        .create_agent(crate::NewAgent {
            agent_id: "agent:a-b".to_string(),
            display_name: Some("Dash".to_string()),
        })
        .await
        .expect("create dashed agent");
    let underscored = store
        .create_agent(crate::NewAgent {
            agent_id: "agent:a_b".to_string(),
            display_name: Some("  Under  ".to_string()),
        })
        .await
        .expect("create underscored agent");

    assert_eq!(dashed.agent_id, "agent:a-b");
    assert_eq!(underscored.agent_id, "agent:a_b");
    assert_eq!(
        store
            .get_agent("agent:a-b")
            .await
            .expect("get dashed")
            .expect("dashed exists")
            .display_name
            .as_deref(),
        Some("Dash")
    );
    assert_eq!(
        store
            .get_agent("agent:a_b")
            .await
            .expect("get underscored")
            .expect("underscored exists")
            .display_name
            .as_deref(),
        Some("Under")
    );
}

#[tokio::test]
async fn update_agent_display_name_rejects_missing_agent() {
    let store = test_store().await;

    let error = store
        .update_agent_display_name("agent:missing", "Mira")
        .await
        .expect_err("missing agent should be rejected");

    assert!(matches!(
        error,
        StoreError::AgentNotFound { agent_id } if agent_id == "agent:missing"
    ));
}

#[tokio::test]
async fn primary_conversation_reuses_existing_home_conversation() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");

    let first = store
        .get_or_create_primary_conversation("human:local", Some("gpt-test".to_string()), None)
        .await
        .expect("first primary conversation");
    let second = store
        .get_or_create_primary_conversation("human:local", Some("other-model".to_string()), None)
        .await
        .expect("second primary conversation");

    assert_eq!(first.conversation_id, second.conversation_id);
}

#[tokio::test]
async fn primary_conversation_reuses_existing_home_conversation_across_provider_switch() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");

    let first = store
        .get_or_create_primary_conversation_for_provider(
            "human:local",
            "codex",
            Some("gpt-test".to_string()),
            None,
        )
        .await
        .expect("first primary conversation");
    let second = store
        .get_or_create_primary_conversation_for_provider(
            "human:local",
            "foundation_local",
            Some("default".to_string()),
            None,
        )
        .await
        .expect("second primary conversation");

    assert_eq!(first.conversation_id, second.conversation_id);
}

#[tokio::test]
async fn primary_conversation_ignores_pointer_to_other_human_conversation() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let mut other_human_conversation = NewConversation::local_chat(None, None);
    other_human_conversation.owner = ObjectRef::human("human:other");
    other_human_conversation.primary_human_id = Some("human:other".to_string());
    let other = store
        .create_conversation(other_human_conversation)
        .await
        .expect("other human conversation");
    store
        .db()
        .query(
            r#"
            UPDATE humans SET
              primary_conversation_id = $conversation_id,
              updated_at = time::now()
            WHERE human_id = 'human:local';
            "#,
        )
        .bind(("conversation_id", other.conversation_id.clone()))
        .await
        .expect("update human pointer")
        .check()
        .expect("checked pointer update");

    let local = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("local primary conversation");
    let local_again = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("local primary conversation again");

    assert_ne!(local.conversation_id, other.conversation_id);
    assert_eq!(local.conversation_id, local_again.conversation_id);
}

#[tokio::test]
async fn conversation_items_replay_in_append_order() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("turn");

    store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("first".to_string()),
            payload_json: json!({}),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("first item");
    store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::agent("agent:primary"),
            content_text: Some("second".to_string()),
            payload_json: json!({}),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("second item");

    let items = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("items");
    let content = items
        .into_iter()
        .map(|item| item.content_text)
        .collect::<Vec<_>>();

    assert_eq!(
        content,
        vec![Some("first".to_string()), Some("second".to_string())]
    );
}

#[tokio::test]
async fn conversation_items_replay_exposes_sequence_index() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("turn");

    let first = store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("first".to_string()),
            payload_json: json!({}),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("first item");
    let second = store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id),
            parent_item_id: Some(first.item_id.clone()),
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::agent("agent:primary"),
            content_text: Some("second".to_string()),
            payload_json: json!({}),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("second item");

    assert_eq!(first.sequence_index, 1);
    assert_eq!(second.sequence_index, 2);
    let replay = store
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("replay");
    assert_eq!(
        replay
            .iter()
            .map(|item| item.sequence_index)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[tokio::test]
async fn conversation_items_latest_page_returns_newest_visible_items_in_ascending_order() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("turn");

    for label in ["one", "two", "three", "four"] {
        store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::human("human:local"),
                content_text: Some(label.to_string()),
                payload_json: json!({}),
                metadata: json!({ "turn_index": 1 }),
            })
            .await
            .expect("item");
    }

    let page = store
        .list_visible_conversation_item_page(&conversation.conversation_id, None, 2)
        .await
        .expect("latest page");

    assert_eq!(
        page.items
            .iter()
            .map(|item| item.content_text.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("three"), Some("four")]
    );
    assert!(page.has_more_before);
    assert_eq!(
        page.before_cursor,
        page.items.first().map(|item| item.cursor.clone())
    );
    assert_eq!(page.limit, 2);
}

#[tokio::test]
async fn conversation_items_cursor_page_returns_items_before_cursor() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("turn");

    for label in ["one", "two", "three", "four"] {
        store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::human("human:local"),
                content_text: Some(label.to_string()),
                payload_json: json!({}),
                metadata: json!({ "turn_index": 1 }),
            })
            .await
            .expect("item");
    }

    let latest = store
        .list_visible_conversation_item_page(&conversation.conversation_id, None, 2)
        .await
        .expect("latest page");
    let older = store
        .list_visible_conversation_item_page(
            &conversation.conversation_id,
            latest.before_cursor.as_deref(),
            2,
        )
        .await
        .expect("older page");

    assert_eq!(
        older
            .items
            .iter()
            .map(|item| item.content_text.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("one"), Some("two")]
    );
    assert!(!older.has_more_before);
    assert_eq!(
        older.before_cursor,
        older.items.first().map(|item| item.cursor.clone())
    );
}

#[tokio::test]
async fn conversation_items_page_excludes_deleted_items() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("turn");

    let deleted = store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("deleted".to_string()),
            payload_json: json!({}),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("deleted item");
    store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("visible".to_string()),
            payload_json: json!({}),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("visible item");

    store
        .db()
        .query(
            "UPDATE conversation_items SET deleted_at = 'test-deleted' WHERE item_id = $item_id;",
        )
        .bind(("item_id", deleted.item_id))
        .await
        .expect("soft delete")
        .check()
        .expect("checked soft delete");

    let page = store
        .list_visible_conversation_item_page(&conversation.conversation_id, None, 10)
        .await
        .expect("page");

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].content_text.as_deref(), Some("visible"));
}

#[tokio::test]
async fn conversation_items_page_rejects_malformed_cursor() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");

    let error = store
        .list_visible_conversation_item_page(
            &conversation.conversation_id,
            Some("not-a-cursor"),
            10,
        )
        .await
        .expect_err("malformed cursor should fail");

    assert!(
        matches!(error, StoreError::Schema(ref message) if message.contains("invalid conversation item cursor")),
        "{error:?}"
    );
}

#[tokio::test]
async fn context_summary_lifecycle_supersedes_previous_active_checkpoint() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat_for_provider(
            "foundation_local",
            Some("default".to_string()),
            None,
        ))
        .await
        .expect("conversation");

    let first = store
        .insert_conversation_context_summary(crate::NewConversationContextSummary {
            conversation_id: conversation.conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: Some("default".to_string()),
            summary_text: "The user is exploring local model support.".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 4,
            source_item_ids: vec!["item:1".to_string(), "item:4".to_string()],
            input_token_estimate: 900,
            summary_token_estimate: 64,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: Some("default".to_string()),
            status: crate::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("first summary");
    let second = store
        .insert_conversation_context_summary(crate::NewConversationContextSummary {
            conversation_id: conversation.conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: Some("default".to_string()),
            summary_text: "The user wants durable compaction checkpoints.".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 8,
            source_item_ids: vec!["item:1".to_string(), "item:8".to_string()],
            input_token_estimate: 1_800,
            summary_token_estimate: 82,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: Some("default".to_string()),
            status: crate::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("second summary");

    let active = store
        .latest_active_context_summary(
            &conversation.conversation_id,
            "foundation_local",
            Some("default"),
        )
        .await
        .expect("active summary")
        .expect("active summary exists");

    assert_eq!(active.summary_id, second.summary_id);
    let first_reloaded = store
        .get_conversation_context_summary(&first.summary_id)
        .await
        .expect("first reload")
        .expect("first summary exists");
    assert_eq!(
        first_reloaded.status,
        crate::ConversationContextSummaryStatus::Superseded
    );

    let stale = store
        .insert_conversation_context_summary(crate::NewConversationContextSummary {
            conversation_id: conversation.conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: Some("default".to_string()),
            summary_text: "stale background summary".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 6,
            source_item_ids: vec!["item:1".to_string(), "item:6".to_string()],
            input_token_estimate: 1_200,
            summary_token_estimate: 72,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: Some("default".to_string()),
            status: crate::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("stale summary insert");
    let active_after_stale = store
        .latest_active_context_summary(
            &conversation.conversation_id,
            "foundation_local",
            Some("default"),
        )
        .await
        .expect("active after stale")
        .expect("active summary exists");
    assert_eq!(stale.summary_id, second.summary_id);
    assert_eq!(active_after_stale.summary_id, second.summary_id);
}

#[tokio::test]
async fn append_conversation_item_rejects_cross_conversation_turn() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let first = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("first conversation");
    let second = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("second conversation");
    let first_turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: first.conversation_id,
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("first turn");

    let error = store
        .append_conversation_item(NewConversationItem {
            conversation_id: second.conversation_id.clone(),
            turn_id: Some(first_turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("wrong turn".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect_err("cross-conversation turn should be rejected");

    assert!(matches!(
        error,
        StoreError::ConversationTurnConversationMismatch {
            turn_id,
            conversation_id,
        } if turn_id == first_turn.turn_id && conversation_id == second.conversation_id
    ));
}

#[tokio::test]
async fn create_conversation_turn_rejects_cross_conversation_trigger_item() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let first = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("first conversation");
    let second = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("second conversation");
    let first_item = store
        .append_conversation_item(NewConversationItem {
            conversation_id: first.conversation_id,
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("wrong trigger".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("first conversation item");

    let error = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: second.conversation_id.clone(),
            trigger_item_id: Some(first_item.item_id.clone()),
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect_err("cross-conversation trigger item should be rejected");

    assert!(matches!(
        error,
        StoreError::ConversationItemConversationMismatch {
            item_id,
            conversation_id,
        } if item_id == first_item.item_id && conversation_id == second.conversation_id
    ));
}

#[tokio::test]
async fn append_conversation_item_rejects_cross_conversation_parent() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let first = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("first conversation");
    let second = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("second conversation");
    let first_turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: first.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("first turn");
    let second_turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: second.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("second turn");
    let first_item = store
        .append_conversation_item(NewConversationItem {
            conversation_id: first.conversation_id,
            turn_id: Some(first_turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("first parent".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("first item");

    let error = store
        .append_conversation_item(NewConversationItem {
            conversation_id: second.conversation_id.clone(),
            turn_id: Some(second_turn.turn_id),
            parent_item_id: Some(first_item.item_id.clone()),
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::agent("agent:primary"),
            content_text: Some("wrong parent".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect_err("cross-conversation parent should be rejected");

    assert!(matches!(
        error,
        StoreError::ConversationItemConversationMismatch {
            item_id,
            conversation_id,
        } if item_id == first_item.item_id && conversation_id == second.conversation_id
    ));
}

pub(crate) async fn test_store() -> NoemaStore {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    let store = NoemaStore::open(&config).await.expect("open store");
    std::mem::forget(home);
    store
}
