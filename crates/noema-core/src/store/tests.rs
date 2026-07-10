use tempfile::TempDir;

use super::{NoemaStore, StoreConfig};

mod mcp;
mod schema_v2;

#[tokio::test]
async fn opens_sqlite_store_under_noema_db_dir() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    let store = NoemaStore::open(&config).await.expect("open store");

    assert!(paths.db_dir().exists());
    assert!(paths.sqlite_db_path().exists());
    assert_eq!(store.schema_version().await.expect("schema version"), 2);
}

#[tokio::test]
async fn sqlite_schema_does_not_create_memory_ingest_jobs() {
    let store = test_store().await;

    let table_count = store
        .with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'memory_ingest_jobs'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("table lookup");

    assert_eq!(table_count, 0);
}

#[tokio::test]
async fn sqlite_schema_creates_artifact_tables() {
    let store = test_store().await;

    let tables = store
        .with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('artifacts', 'artifact_versions') ORDER BY name",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("table lookup");

    assert_eq!(tables, vec!["artifact_versions", "artifacts"]);
}

#[tokio::test]
async fn artifact_external_url_initial_version_round_trips() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");

    let artifact = store
        .create_artifact_with_initial_version(
            crate::NewArtifact {
                artifact_id: None,
                owner: crate::ArtifactOwnerRef::conversation(&conversation.conversation_id),
                title: "Sprint brief".to_string(),
                description: Some("Planning notes".to_string()),
                artifact_kind: "document".to_string(),
                storage_kind: crate::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource {
                    conversation_id: Some(conversation.conversation_id.clone()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({"provider": "notion"}),
            },
            crate::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Initial".to_string()),
                storage: crate::ArtifactVersionStorage::ExternalUrl {
                    url: "https://notion.so/noema-brief".to_string(),
                },
                media_type: Some("text/html".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource {
                    conversation_id: Some(conversation.conversation_id.clone()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("artifact");

    assert_eq!(artifact.artifact.title, "Sprint brief");
    assert_eq!(artifact.versions.len(), 1);
    assert_eq!(
        artifact.current_version.artifact_id,
        artifact.artifact.artifact_id
    );
    assert_eq!(artifact.current_version.version_index, 1);
}

#[tokio::test]
async fn artifact_external_url_initial_version_rejects_non_http_url() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");

    let error = store
        .create_artifact_with_initial_version(
            crate::NewArtifact {
                artifact_id: None,
                owner: crate::ArtifactOwnerRef::conversation(&conversation.conversation_id),
                title: "Unsafe link".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                storage_kind: crate::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
            crate::NewArtifactVersion {
                artifact_version_id: None,
                title: None,
                storage: crate::ArtifactVersionStorage::ExternalUrl {
                    url: "javascript:alert(1)".to_string(),
                },
                media_type: None,
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect_err("unsafe external URL should be rejected");

    assert!(matches!(
        error,
        crate::StoreError::InvalidArtifactExternalUrl { .. }
    ));
}

#[tokio::test]
async fn conversation_local_file_artifact_writes_bytes_and_metadata() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("open store");
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let bytes = b"# report\n".to_vec();

    let artifact = crate::create_conversation_local_file_artifact(
        &store,
        &paths,
        crate::NewConversationLocalFileArtifact {
            conversation_id: conversation.conversation_id.clone(),
            title: "Session report".to_string(),
            description: Some("Local markdown artifact".to_string()),
            artifact_kind: "document".to_string(),
            filename: "report.md".to_string(),
            bytes: bytes.clone(),
            media_type: Some("text/markdown".to_string()),
            created_by_actor_id: "agent:primary".to_string(),
            source: crate::ArtifactSource {
                conversation_id: Some(conversation.conversation_id.clone()),
                turn_id: None,
                item_id: None,
            },
            metadata: serde_json::json!({"origin": "unit-test"}),
        },
    )
    .await
    .expect("create local artifact");

    assert_eq!(
        artifact.artifact.storage_kind,
        crate::ArtifactStorageKind::LocalFile
    );
    assert_eq!(artifact.current_version.version_index, 1);
    assert_eq!(
        artifact.current_version.media_type.as_deref(),
        Some("text/markdown")
    );
    assert_eq!(artifact.current_version.byte_size, Some(bytes.len() as i64));
    assert!(artifact.current_version.content_sha256.is_some());

    let relative_path = match &artifact.current_version.storage {
        crate::ArtifactVersionStorage::LocalFile { relative_path } => relative_path,
        crate::ArtifactVersionStorage::ExternalUrl { .. } => {
            panic!("expected local file storage")
        }
    };
    let absolute_path = paths.root().join(relative_path);
    assert_eq!(
        tokio::fs::read(&absolute_path).await.expect("read bytes"),
        bytes
    );
    assert!(
        absolute_path.starts_with(paths.conversation_artifacts_dir(&conversation.conversation_id))
    );
    assert_eq!(
        artifact.artifact.metadata,
        serde_json::json!({"origin": "unit-test"})
    );
}

#[cfg(unix)]
#[tokio::test]
async fn conversation_local_file_artifact_rejects_symlinked_artifact_root() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("open store");
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let outside = home.path().join("outside-artifacts");
    tokio::fs::create_dir_all(&outside)
        .await
        .expect("outside dir");
    tokio::fs::create_dir_all(paths.conversation_dir(&conversation.conversation_id))
        .await
        .expect("conversation dir");
    std::os::unix::fs::symlink(
        &outside,
        paths.conversation_artifacts_dir(&conversation.conversation_id),
    )
    .expect("symlink artifact root");

    let error = crate::create_conversation_local_file_artifact(
        &store,
        &paths,
        crate::NewConversationLocalFileArtifact {
            conversation_id: conversation.conversation_id.clone(),
            title: "Session report".to_string(),
            description: None,
            artifact_kind: "document".to_string(),
            filename: "report.md".to_string(),
            bytes: b"# report\n".to_vec(),
            media_type: Some("text/markdown".to_string()),
            created_by_actor_id: "agent:primary".to_string(),
            source: crate::ArtifactSource::default(),
            metadata: serde_json::json!({}),
        },
    )
    .await
    .expect_err("symlinked artifact root should be rejected");

    assert!(matches!(
        error,
        crate::ArtifactWriteError::CreateDirectory { .. }
            | crate::ArtifactWriteError::WriteFile { .. }
    ));
}

#[tokio::test]
async fn append_artifact_version_updates_current_version() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let created = seed_external_artifact(&store, &conversation.conversation_id).await;

    let second = store
        .append_artifact_version(
            &created.artifact.artifact_id,
            crate::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Revision".to_string()),
                storage: crate::ArtifactVersionStorage::ExternalUrl {
                    url: "https://notion.so/noema-brief-v2".to_string(),
                },
                media_type: Some("text/html".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource::default(),
                metadata: serde_json::json!({"revision": 2}),
            },
        )
        .await
        .expect("append version");

    assert_eq!(second.version_index, 2);
    let loaded = store
        .get_artifact(&created.artifact.artifact_id)
        .await
        .expect("load artifact")
        .expect("artifact exists");
    assert_eq!(
        loaded.artifact.current_version_id.as_deref(),
        Some(second.artifact_version_id.as_str())
    );
    assert_eq!(loaded.versions.len(), 2);
}

#[tokio::test]
async fn append_artifact_version_rejects_non_http_external_url() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let created = seed_external_artifact(&store, &conversation.conversation_id).await;

    let error = store
        .append_artifact_version(
            &created.artifact.artifact_id,
            crate::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Unsafe revision".to_string()),
                storage: crate::ArtifactVersionStorage::ExternalUrl {
                    url: "file:///private/report.html".to_string(),
                },
                media_type: None,
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect_err("unsafe external URL should be rejected");

    assert!(matches!(
        error,
        crate::StoreError::InvalidArtifactExternalUrl { .. }
    ));
}

#[tokio::test]
async fn artifact_read_rejects_forged_non_http_external_url() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let created = seed_external_artifact(&store, &conversation.conversation_id).await;

    store
        .with_connection(|conn| {
            conn.execute(
                "UPDATE artifact_versions SET external_url = ?1 WHERE artifact_version_id = ?2",
                rusqlite::params![
                    "javascript:alert(1)",
                    created.current_version.artifact_version_id
                ],
            )
            .map(|_| ())
            .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("forge external URL");

    let error = store
        .get_artifact(&created.artifact.artifact_id)
        .await
        .expect_err("forged external URL should be rejected on read");

    assert!(matches!(
        error,
        crate::StoreError::InvalidArtifactExternalUrl { .. }
    ));
}

#[tokio::test]
async fn sqlite_store_config_is_stable_for_reopen() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    assert_eq!(config.path, paths.sqlite_db_path());
    assert_eq!(StoreConfig::from_paths(&paths), config);
}

pub(crate) async fn test_store() -> crate::NoemaStore {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("open store");
    std::mem::forget(home);
    store
}

async fn seed_external_artifact(
    store: &crate::NoemaStore,
    conversation_id: &str,
) -> crate::ArtifactWithVersions {
    store
        .create_artifact_with_initial_version(
            crate::NewArtifact {
                artifact_id: None,
                owner: crate::ArtifactOwnerRef::conversation(conversation_id),
                title: "Sprint brief".to_string(),
                description: Some("Planning notes".to_string()),
                artifact_kind: "document".to_string(),
                storage_kind: crate::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource {
                    conversation_id: Some(conversation_id.to_string()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({"provider": "notion"}),
            },
            crate::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Initial".to_string()),
                storage: crate::ArtifactVersionStorage::ExternalUrl {
                    url: "https://notion.so/noema-brief".to_string(),
                },
                media_type: Some("text/html".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource {
                    conversation_id: Some(conversation_id.to_string()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("seed artifact")
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_provider_account_for_tests(
    store: &crate::NoemaStore,
    provider_account_id: &str,
    provider_kind: &str,
    account_key: &str,
    display_name: &str,
    auth_method: crate::ProviderAuthMethod,
    is_default: bool,
    status: crate::ProviderAccountStatus,
    metadata: serde_json::Value,
) {
    let metadata_json = serde_json::to_string(&metadata).expect("serialize provider metadata");
    store
        .with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO provider_accounts (
                  provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, metadata_json, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(provider_account_id) DO UPDATE SET
                  provider_kind = excluded.provider_kind,
                  account_key = excluded.account_key,
                  display_name = excluded.display_name,
                  auth_method = excluded.auth_method,
                  is_active = excluded.is_active,
                  is_default = excluded.is_default,
                  status = excluded.status,
                  metadata_json = excluded.metadata_json,
                  updated_at = excluded.updated_at
                "#,
                rusqlite::params![
                    provider_account_id,
                    provider_kind,
                    account_key,
                    display_name,
                    auth_method.as_str(),
                    is_default,
                    status.as_str(),
                    metadata_json,
                ],
            )?;
            Ok(())
        })
        .await
        .expect("insert provider account");
}

pub(crate) async fn insert_provider_capability_binding_for_tests(
    store: &crate::NoemaStore,
    tool_name: &str,
    capability_id: &str,
    provider_account_id: &str,
) {
    let binding_id = format!("provider_capability_binding:{tool_name}:{capability_id}");
    store
        .with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO provider_capability_bindings
                  (binding_id, tool_name, capability_id, provider_account_id, updated_at)
                VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(tool_name, capability_id) DO UPDATE SET
                  provider_account_id = excluded.provider_account_id,
                  updated_at = excluded.updated_at
                "#,
                rusqlite::params![binding_id, tool_name, capability_id, provider_account_id],
            )?;
            Ok(())
        })
        .await
        .expect("insert provider capability binding");
}

#[tokio::test]
async fn sqlite_default_actors_round_trip() {
    let store = test_store().await;

    store.ensure_default_actors().await.expect("default actors");

    let human = store
        .get_human("human:local")
        .await
        .expect("get human")
        .expect("human exists");
    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("agent exists");

    assert_eq!(human.human_id, "human:local");
    assert_eq!(agent.agent_id, "agent:primary");
    assert_eq!(agent.display_name, None);
}

#[tokio::test]
async fn sqlite_agent_display_name_updates() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("default actors");

    store
        .update_agent_display_name("agent:primary", "Noema")
        .await
        .expect("update agent");

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Noema"));
}

#[tokio::test]
async fn sqlite_create_agent_rejects_duplicate_agent_id() {
    let store = test_store().await;

    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:duplicate".to_string(),
            display_name: Some("Original".to_string()),
        })
        .await
        .expect("create agent");

    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:duplicate".to_string(),
            display_name: Some("Replacement".to_string()),
        })
        .await
        .expect_err("duplicate agent id should fail");

    let agent = store
        .get_agent("agent:duplicate")
        .await
        .expect("get agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Original"));
}

#[tokio::test]
async fn sqlite_provider_accounts_seed_and_list() {
    let store = test_store().await;

    store
        .ensure_default_provider_account()
        .await
        .expect("codex");
    store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation");

    let accounts = store
        .list_provider_accounts()
        .await
        .expect("provider accounts");
    let ids = accounts
        .iter()
        .map(|account| account.provider_account_id.as_str())
        .collect::<Vec<_>>();

    assert!(ids.contains(&"provider_account:codex:default"));
    assert!(ids.contains(&"provider_account:foundation_local:default"));
}

#[tokio::test]
async fn sqlite_agent_model_preference_round_trip() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider");

    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::Medium),
        })
        .await
        .expect("save preference");

    let preference = store
        .get_agent_runtime_preference("agent:primary")
        .await
        .expect("get preference")
        .expect("preference exists");
    assert_eq!(preference.model_profile, "gpt-5.5");
    assert_eq!(
        preference.reasoning_effort,
        Some(crate::provider::ReasoningEffort::Medium)
    );
}

#[tokio::test]
async fn sqlite_memory_service_defaults_to_managed() {
    let store = test_store().await;

    let settings = store.memory_service_settings().await.expect("settings");

    assert_eq!(settings.mode, crate::MemoryServiceMode::Managed);
    assert_eq!(settings.base_url, None);
    assert_eq!(settings.port, None);
}

#[tokio::test]
async fn sqlite_memory_service_settings_round_trip_external() {
    let store = test_store().await;
    store
        .with_connection(|conn| {
            conn.execute("INSERT INTO provider_accounts (provider_account_id, provider_kind, account_key, display_name, auth_method, is_active, is_default, status) VALUES ('provider_account:openai:default', 'openai', 'default', 'OpenAI', 'none', 1, 1, 'authenticated')", [])?;
            Ok(())
        })
        .await
        .expect("provider account");

    store
        .save_memory_service_settings(crate::SaveMemoryServiceSettings {
            mode: crate::MemoryServiceMode::External,
            base_url: Some("http://127.0.0.1:7777".to_string()),
            port: None,
            provider_account_id: Some("provider_account:openai:default".to_string()),
            provider_kind: Some("openai".to_string()),
            model_profile: Some("gpt-5.1".to_string()),
            reasoning_effort: Some(crate::provider::ReasoningEffort::Low),
        })
        .await
        .expect("save settings");

    let settings = store.memory_service_settings().await.expect("settings");
    assert_eq!(settings.mode, crate::MemoryServiceMode::External);
    assert_eq!(settings.base_url.as_deref(), Some("http://127.0.0.1:7777"));
    assert_eq!(
        settings.reasoning_effort,
        Some(crate::provider::ReasoningEffort::Low)
    );
}

#[tokio::test]
async fn sqlite_memory_service_settings_validate_provider_and_clear_deleted_account() {
    let store = test_store().await;
    store
        .with_connection(|conn| {
            conn.execute("INSERT INTO provider_accounts (provider_account_id, provider_kind, account_key, display_name, auth_method, is_active, is_default, status) VALUES ('provider_account:memory', 'openai', 'memory', 'Memory', 'none', 1, 0, 'authenticated')", [])?;
            Ok(())
        })
        .await
        .expect("provider account");
    let settings = crate::SaveMemoryServiceSettings {
        mode: crate::MemoryServiceMode::External,
        base_url: Some("http://127.0.0.1:7777".to_string()),
        port: None,
        provider_account_id: Some("provider_account:memory".to_string()),
        provider_kind: Some("codex".to_string()),
        model_profile: Some("model".to_string()),
        reasoning_effort: None,
    };
    assert!(
        store
            .save_memory_service_settings(settings.clone())
            .await
            .is_err()
    );
    let saved = store
        .save_memory_service_settings(crate::SaveMemoryServiceSettings {
            provider_kind: Some("openai".to_string()),
            ..settings
        })
        .await
        .expect("matching provider");
    assert_eq!(
        saved.provider_account_id.as_deref(),
        Some("provider_account:memory")
    );
    store
        .with_connection(|conn| {
            conn.execute("DELETE FROM provider_accounts WHERE provider_account_id = 'provider_account:memory'", [])?;
            Ok(())
        })
        .await
        .expect("delete provider");
    assert_eq!(
        store
            .memory_service_settings()
            .await
            .expect("settings")
            .provider_account_id,
        None
    );
}

#[tokio::test]
async fn sqlite_memory_article_cache_round_trip() {
    let store = test_store().await;

    store
        .save_memory_article_cache(crate::SaveMemoryArticleCache {
            scope_id: "human:local".to_string(),
            fact_fingerprint: "facts-v1".to_string(),
            article_markdown: "# Kevin\n\nLittle is currently known about Kevin.".to_string(),
            generated_at: "2026-07-08T20:00:00.000Z".to_string(),
        })
        .await
        .expect("save article cache");

    let cached = store
        .memory_article_cache("human:local")
        .await
        .expect("cache")
        .expect("cache row");
    assert_eq!(cached.fact_fingerprint, "facts-v1");
    assert_eq!(
        cached.article_markdown,
        "# Kevin\n\nLittle is currently known about Kevin."
    );
}

#[tokio::test]
async fn sqlite_conversation_items_page_in_sequence_order() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");

    let turn = store
        .create_conversation_turn(crate::NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("turn");

    store
        .append_conversation_item(crate::NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: crate::ConversationItemKind::UserText,
            status: crate::ConversationItemStatus::Completed,
            author: crate::ActorRef::human("human:local"),
            content_text: Some("hello".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("append user");

    let page = store
        .list_visible_conversation_item_page(&conversation.conversation_id, None, 20)
        .await
        .expect("page");

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].content_text.as_deref(), Some("hello"));
    assert_eq!(page.items[0].sequence_index, 1);
}
