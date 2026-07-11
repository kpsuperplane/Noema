use tempfile::TempDir;

use super::{NoemaStore, StoreConfig};

mod mcp;

#[tokio::test]
async fn opens_sqlite_store_under_noema_db_dir() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    let store = NoemaStore::open(&config).await.expect("open store");

    assert!(paths.db_dir().exists());
    assert!(paths.sqlite_db_path().exists());
    assert_eq!(store.schema_version().await.expect("schema version"), 1);
}

#[tokio::test]
async fn opening_pre_v1_task_runtime_tables_rebuilds_and_preserves_history() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    let store = NoemaStore::open(&config).await.expect("open store");
    store
        .with_connection(|conn| {
            conn.execute_batch(
                r#"
                DROP TABLE agent_run_items;
                DROP TABLE agent_runs;
                CREATE TABLE agent_runs (
                  run_id TEXT PRIMARY KEY NOT NULL,
                  task_id TEXT NOT NULL,
                  run_kind TEXT NOT NULL CHECK (run_kind IN ('executor', 'reviewer', 'completion_delivery')),
                  agent_id TEXT NOT NULL,
                  attempt_index INTEGER NOT NULL DEFAULT 0,
                  revision_index INTEGER NOT NULL DEFAULT 0,
                  parent_run_id TEXT,
                  triggering_submission_id TEXT,
                  triggering_review_id TEXT,
                  provider_kind TEXT NOT NULL,
                  provider_account_id TEXT NOT NULL,
                  selection_mode TEXT NOT NULL,
                  model_profile TEXT,
                  reasoning_effort TEXT,
                  selection_source TEXT,
                  actual_provider_kind TEXT,
                  actual_model_profile TEXT,
                  status TEXT NOT NULL,
                  priority INTEGER NOT NULL DEFAULT 0,
                  queued_at TEXT NOT NULL DEFAULT 'old',
                  lease_owner TEXT,
                  lease_token TEXT,
                  lease_expires_at TEXT,
                  heartbeat_at TEXT,
                  started_at TEXT,
                  ended_at TEXT,
                  cancellation_requested INTEGER NOT NULL DEFAULT 0,
                  retry_count INTEGER NOT NULL DEFAULT 0,
                  error_code TEXT,
                  error_message TEXT,
                  input_tokens INTEGER,
                  output_tokens INTEGER,
                  created_at TEXT NOT NULL DEFAULT 'old',
                  updated_at TEXT NOT NULL DEFAULT 'old'
                );
                CREATE TABLE agent_run_items (
                  item_id TEXT PRIMARY KEY NOT NULL,
                  run_id TEXT NOT NULL,
                  sequence_index INTEGER NOT NULL,
                  kind TEXT NOT NULL CHECK (kind IN ('model_input', 'assistant_output', 'tool_call', 'tool_result', 'progress_notice', 'task_submission', 'task_review', 'artifact_reference', 'failure', 'cancellation')),
                  content_text TEXT,
                  payload_json TEXT NOT NULL DEFAULT '{}',
                  created_at TEXT NOT NULL DEFAULT 'old',
                  UNIQUE(run_id, sequence_index)
                );
                INSERT INTO agent_runs (run_id, task_id, run_kind, agent_id, provider_kind, provider_account_id, selection_mode, status, input_tokens, output_tokens)
                VALUES ('run:legacy', 'task:legacy', 'executor', 'agent:task-executor', 'codex', 'provider_account:codex:default', 'explicit_profile', 'failed', NULL, NULL);
                INSERT INTO agent_run_items (item_id, run_id, sequence_index, kind, content_text)
                VALUES ('item:legacy', 'run:legacy', 1, 'assistant_output', 'preserved');
                "#,
            )?;
            Ok(())
        })
        .await
        .expect("install legacy schema");
    drop(store);

    let reopened = NoemaStore::open(&config)
        .await
        .expect("upgrade legacy store");
    let run = reopened
        .get_agent_run("run:legacy")
        .await
        .expect("read run")
        .expect("legacy run");
    assert_eq!(run.execution_policy, crate::TaskExecutionPolicy::default());
    assert_eq!(run.input_tokens, 0);
    assert_eq!(run.output_tokens, 0);
    let items = reopened
        .list_agent_run_items("run:legacy")
        .await
        .expect("legacy items");
    assert_eq!(items[0].content_text.as_deref(), Some("preserved"));
    assert_eq!(items[0].status, crate::AgentRunItemStatus::Completed);
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

#[tokio::test]
async fn task_lifecycle_queues_review_and_completes_without_delivery_run() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            crate::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider account");
    let model = crate::ModelConfigSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6",
        None,
        Some("test".to_string()),
    );
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("task model settings")
        .into_iter()
        .find(|entry| entry.complexity == crate::TaskComplexity::Simple)
        .expect("simple task model");
    let (task, executor_run) = store
        .create_task_with_executor(crate::NewTask {
            task_id: None,
            title: "Lifecycle task".to_string(),
            request_markdown: "Produce a short result".to_string(),
            complexity: crate::TaskComplexity::Simple,
            owner_human_id: "human:local".to_string(),
            source: crate::TaskSource::default(),
            created_by_agent_id: "agent:primary".to_string(),
            creation_tool_call_id: None,
            pool_entry_id: pool.pool_entry_id,
            executor_model: pool.model.clone(),
            reviewer_model: model,
            max_review_rounds: None,
            criteria: vec![crate::NewTaskValidationCriterion {
                criterion_id: None,
                ordinal: 1,
                description: "Result is present".to_string(),
                expected_evidence: None,
            }],
        })
        .await
        .expect("task");
    assert_eq!(executor_run.run_kind, crate::RunKind::Executor);
    store
        .transition_task(&task.task_id, crate::TaskStatus::Executing, Some("test"))
        .await
        .expect("executing");
    store
        .claim_next_agent_run("worker:executor", "lease:executor", 120)
        .await
        .expect("claim executor")
        .expect("executor run");
    store
        .transition_agent_run(
            &executor_run.run_id,
            crate::RunStatus::Running,
            Some("lease:executor"),
            None,
        )
        .await
        .expect("run executor");
    let (submission, reviewer_run) = store
        .create_task_submission(
            crate::NewTaskSubmission {
                submission_id: None,
                task_id: task.task_id.clone(),
                executor_run_id: executor_run.run_id,
                revision_index: 0,
                summary: "Done".to_string(),
                result_markdown: "# Result\n\nDone".to_string(),
                criteria: vec![crate::SubmissionCriterionEvidence {
                    criterion_id: store
                        .list_task_validation_criteria(&task.task_id)
                        .await
                        .expect("criteria")[0]
                        .criterion_id
                        .clone(),
                    evidence_markdown: "The result is present".to_string(),
                }],
                artifact_ids: Vec::new(),
            },
            "lease:executor",
        )
        .await
        .expect("submission");
    store
        .claim_next_agent_run("worker:reviewer", "lease:reviewer", 120)
        .await
        .expect("claim reviewer")
        .expect("reviewer run");
    store
        .transition_agent_run(
            &reviewer_run.run_id,
            crate::RunStatus::Running,
            Some("lease:reviewer"),
            None,
        )
        .await
        .expect("run reviewer");
    let completed = store
        .create_task_review(
            crate::NewTaskReview {
                review_id: None,
                task_id: task.task_id.clone(),
                reviewer_run_id: reviewer_run.run_id.clone(),
                reviewed_submission_id: submission.submission_id,
                overall_verdict: crate::TaskReviewVerdict::Approve,
                overall_feedback: "All criteria pass".to_string(),
                criteria: vec![crate::TaskReviewCriterion {
                    criterion_id: store
                        .list_task_validation_criteria(&task.task_id)
                        .await
                        .expect("criteria")[0]
                        .criterion_id
                        .clone(),
                    outcome: crate::CriterionOutcome::Pass,
                    evidence_markdown: Some("Verified".to_string()),
                    feedback: None,
                }],
            },
            "lease:reviewer",
        )
        .await
        .expect("review");
    assert_eq!(completed.status, crate::TaskStatus::Completed);
    assert_eq!(
        completed.latest_run_id.as_deref(),
        Some(reviewer_run.run_id.as_str())
    );
}

#[tokio::test]
async fn failed_task_resume_queues_a_linked_attempt_with_current_snapshots() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            crate::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider account");
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("task model settings")
        .into_iter()
        .find(|entry| entry.complexity == crate::TaskComplexity::Simple)
        .expect("simple task model");
    let (task, failed_run) = store
        .create_task_with_executor(crate::NewTask {
            task_id: None,
            title: "Retry task".to_string(),
            request_markdown: "Try once more".to_string(),
            complexity: crate::TaskComplexity::Simple,
            owner_human_id: "human:local".to_string(),
            source: crate::TaskSource::default(),
            created_by_agent_id: "agent:primary".to_string(),
            creation_tool_call_id: None,
            pool_entry_id: pool.pool_entry_id,
            executor_model: pool.model.clone(),
            reviewer_model: pool.model,
            max_review_rounds: None,
            criteria: vec![crate::NewTaskValidationCriterion {
                criterion_id: None,
                ordinal: 1,
                description: "Completes".to_string(),
                expected_evidence: None,
            }],
        })
        .await
        .expect("task");
    store
        .transition_agent_run(
            &failed_run.run_id,
            crate::RunStatus::Failed,
            None,
            Some((
                "provider_error".to_string(),
                "model unavailable".to_string(),
            )),
        )
        .await
        .expect("failed run");
    let (retried_task, retried_run) = store
        .resume_task(
            &task.task_id,
            "human:local",
            "human:local",
            Some("Continue with the corrected configuration"),
        )
        .await
        .expect("retry task");

    assert_eq!(retried_task.status, crate::TaskStatus::Queued);
    assert_eq!(
        retried_task.latest_run_id.as_deref(),
        Some(retried_run.run_id.as_str())
    );
    assert_eq!(retried_task.error_code, None);
    assert_eq!(retried_task.error_message, None);
    assert_eq!(
        retried_run.parent_run_id.as_deref(),
        Some(failed_run.run_id.as_str())
    );
    assert_eq!(retried_run.attempt_index, 1);
    assert_eq!(retried_run.model, failed_run.model);
    assert_eq!(
        retried_run.execution_policy,
        crate::TaskExecutionPolicy::default()
    );
    assert_eq!(
        retried_run.resume_message.as_deref(),
        Some("Continue with the corrected configuration")
    );
    assert!(
        store
            .resume_task(&task.task_id, "human:local", "human:local", None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn agent_run_items_round_trip_in_sequence_order() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let run = store
        .create_agent_run(crate::NewAgentRun {
            run_id: Some("run:test".to_string()),
            task_id: "task:test".to_string(),
            run_kind: crate::RunKind::Executor,
            agent_id: crate::TASK_EXECUTOR_AGENT_ID.to_string(),
            revision_index: 0,
            attempt_index: 0,
            parent_run_id: None,
            triggering_submission_id: None,
            triggering_review_id: None,
            model: crate::ModelConfigSnapshot::explicit(
                "codex",
                "provider_account:codex:default",
                "gpt-test",
                None,
                Some("test".to_string()),
            ),
            execution_policy: crate::TaskExecutionPolicy::default(),
            priority: 0,
        })
        .await
        .expect("run");
    let leased = store
        .claim_next_agent_run("worker:test", "lease:test", 120)
        .await
        .expect("claim")
        .expect("leased run");
    assert_eq!(leased.run_id, run.run_id);
    store
        .transition_agent_run(
            &run.run_id,
            crate::RunStatus::Running,
            Some("lease:test"),
            None,
        )
        .await
        .expect("running");
    store
        .append_agent_run_item(
            crate::NewAgentRunItem {
                item_id: Some("run_item:1".to_string()),
                run_id: "run:test".to_string(),
                round_index: 0,
                kind: "assistant_output".to_string(),
                status: crate::AgentRunItemStatus::Completed,
                correlation_id: None,
                parent_item_id: None,
                content_text: Some("first".to_string()),
                payload: serde_json::json!({"response_index": 0}),
            },
            "lease:test",
        )
        .await
        .expect("first item");
    store
        .append_agent_run_item(
            crate::NewAgentRunItem {
                item_id: Some("run_item:2".to_string()),
                run_id: "run:test".to_string(),
                round_index: 0,
                kind: "tool_call".to_string(),
                status: crate::AgentRunItemStatus::Completed,
                correlation_id: Some("call:1".to_string()),
                parent_item_id: None,
                content_text: Some("web.fetch".to_string()),
                payload: serde_json::json!({"output_index": 1}),
            },
            "lease:test",
        )
        .await
        .expect("second item");
    for index in 3..=5 {
        store
            .append_agent_run_item(
                crate::NewAgentRunItem {
                    item_id: Some(format!("run_item:{index}")),
                    run_id: "run:test".to_string(),
                    round_index: 1,
                    kind: "assistant_output".to_string(),
                    status: crate::AgentRunItemStatus::Completed,
                    correlation_id: None,
                    parent_item_id: None,
                    content_text: Some(format!("item {index}")),
                    payload: serde_json::json!({"response_index": index - 1}),
                },
                "lease:test",
            )
            .await
            .expect("later item");
    }

    let items = store.list_agent_run_items("run:test").await.expect("items");
    assert_eq!(items.len(), 5);
    assert_eq!(items[0].sequence_index, 1);
    assert_eq!(items[0].content_text.as_deref(), Some("first"));
    assert_eq!(items[1].kind, "tool_call");

    let newest = store
        .list_agent_run_items_before_page("run:test", None, 2)
        .await
        .expect("newest page");
    assert_eq!(
        newest
            .iter()
            .map(|item| item.sequence_index)
            .collect::<Vec<_>>(),
        vec![4, 5]
    );
    let older = store
        .list_agent_run_items_before_page("run:test", Some(4), 2)
        .await
        .expect("older page");
    assert_eq!(
        older
            .iter()
            .map(|item| item.sequence_index)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
}

#[tokio::test]
async fn task_execution_policy_is_global_and_snapshotted_on_new_runs() {
    let store = test_store().await;
    let defaults = store
        .get_task_execution_policy()
        .await
        .expect("default policy");
    assert_eq!(defaults, crate::TaskExecutionPolicy::default());
    let updated = crate::TaskExecutionPolicy {
        max_provider_continuations: 42,
        max_tool_calls: 210,
        max_active_minutes: 90,
        progress_audit_interval: 14,
    };
    assert_eq!(
        store
            .update_task_execution_policy(updated)
            .await
            .expect("updated policy"),
        updated
    );
    let (_, run) = seed_task(&store, "Policy snapshot").await;
    assert_eq!(run.execution_policy, updated);
}

#[tokio::test]
async fn blocked_task_persists_context_and_resumes_as_a_child_run() {
    let store = test_store().await;
    let (task, run) = seed_task(&store, "Blocked task").await;
    let leased = store
        .claim_next_agent_run("worker:blocked", "lease:blocked", 120)
        .await
        .expect("claim")
        .expect("leased");
    assert_eq!(leased.run_id, run.run_id);
    store
        .transition_agent_run(
            &run.run_id,
            crate::RunStatus::Running,
            Some("lease:blocked"),
            None,
        )
        .await
        .expect("running");
    store
        .transition_task(&task.task_id, crate::TaskStatus::Executing, None)
        .await
        .expect("executing");
    let blocked = store
        .report_task_blocked(
            &task.task_id,
            &run.run_id,
            "lease:blocked",
            "Which account should I use?",
            "Research is complete except for account selection.",
        )
        .await
        .expect("blocked");
    assert_eq!(blocked.status, crate::TaskStatus::WaitingForHuman);
    assert_eq!(
        blocked.blocked_question.as_deref(),
        Some("Which account should I use?")
    );
    let (resumed, child) = store
        .resume_task(
            &task.task_id,
            "human:local",
            "human:local",
            Some("Use the personal account"),
        )
        .await
        .expect("resume");
    assert_eq!(resumed.status, crate::TaskStatus::Queued);
    assert_eq!(resumed.blocked_question, None);
    assert_eq!(child.parent_run_id.as_deref(), Some(run.run_id.as_str()));
    assert_eq!(
        child.resume_message.as_deref(),
        Some("Use the personal account")
    );
}

#[tokio::test]
async fn expired_lease_interrupts_parent_and_claims_automatic_child() {
    let store = test_store().await;
    let (task, run) = seed_task(&store, "Lease recovery").await;
    store
        .claim_next_agent_run("worker:old", "lease:old", 120)
        .await
        .expect("initial claim")
        .expect("leased");
    store
        .transition_task(&task.task_id, crate::TaskStatus::Executing, None)
        .await
        .expect("executing");
    store
        .with_connection(|conn| {
            conn.execute(
                "UPDATE agent_runs SET lease_expires_at = '0' WHERE run_id = ?1",
                [&run.run_id],
            )?;
            Ok(())
        })
        .await
        .expect("expire lease");
    let child = store
        .claim_next_agent_run("worker:new", "lease:new", 120)
        .await
        .expect("recovery claim")
        .expect("child run");
    assert_ne!(child.run_id, run.run_id);
    assert_eq!(child.parent_run_id.as_deref(), Some(run.run_id.as_str()));
    assert_eq!(child.retry_count, 1);
    assert_eq!(
        store
            .get_agent_run(&run.run_id)
            .await
            .expect("parent")
            .expect("parent run")
            .status,
        crate::RunStatus::Interrupted
    );
}

#[tokio::test]
async fn shutdown_interruption_is_recovered_as_a_linked_child() {
    let store = test_store().await;
    let (task, run) = seed_task(&store, "Shutdown recovery").await;
    store
        .claim_next_agent_run("worker:old", "lease:old", 120)
        .await
        .expect("initial claim")
        .expect("leased");
    store
        .transition_agent_run(
            &run.run_id,
            crate::RunStatus::Running,
            Some("lease:old"),
            None,
        )
        .await
        .expect("running");
    store
        .transition_task(&task.task_id, crate::TaskStatus::Executing, None)
        .await
        .expect("executing");
    store
        .transition_agent_run(
            &run.run_id,
            crate::RunStatus::Interrupted,
            Some("lease:old"),
            None,
        )
        .await
        .expect("shutdown interruption");

    let child = store
        .claim_next_agent_run("worker:new", "lease:new", 120)
        .await
        .expect("recovery claim")
        .expect("child run");
    assert_ne!(child.run_id, run.run_id);
    assert_eq!(child.parent_run_id.as_deref(), Some(run.run_id.as_str()));
    assert_eq!(child.retry_count, 1);
}

#[tokio::test]
async fn cancellation_fences_failure_and_terminal_submission() {
    let store = test_store().await;
    let (task, run) = seed_task(&store, "Cancellation fence").await;
    store
        .claim_next_agent_run("worker:cancel", "lease:cancel", 120)
        .await
        .expect("claim")
        .expect("leased");
    store
        .transition_agent_run(
            &run.run_id,
            crate::RunStatus::Running,
            Some("lease:cancel"),
            None,
        )
        .await
        .expect("running");
    store
        .transition_task(&task.task_id, crate::TaskStatus::Executing, None)
        .await
        .expect("executing");
    store
        .cancel_task(&task.task_id, "human:local", "human:local")
        .await
        .expect("cancel");

    assert!(
        store
            .transition_agent_run(
                &run.run_id,
                crate::RunStatus::Failed,
                Some("lease:cancel"),
                Some(("provider_error".to_string(), "late failure".to_string())),
            )
            .await
            .is_err()
    );
    let criterion_id = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .expect("criteria")[0]
        .criterion_id
        .clone();
    assert!(
        store
            .create_task_submission(
                crate::NewTaskSubmission {
                    submission_id: None,
                    task_id: task.task_id.clone(),
                    executor_run_id: run.run_id.clone(),
                    revision_index: 0,
                    summary: "Late result".to_string(),
                    result_markdown: "Late result".to_string(),
                    criteria: vec![crate::SubmissionCriterionEvidence {
                        criterion_id,
                        evidence_markdown: "Late evidence".to_string(),
                    }],
                    artifact_ids: Vec::new(),
                },
                "lease:cancel",
            )
            .await
            .is_err()
    );
    let cancelled = store
        .get_task(&task.task_id)
        .await
        .expect("task")
        .expect("cancelled task");
    assert_eq!(cancelled.status, crate::TaskStatus::Cancelled);
    assert!(
        store
            .list_agent_runs_for_task(&task.task_id)
            .await
            .expect("runs")
            .iter()
            .all(|candidate| candidate.run_kind != crate::RunKind::Reviewer)
    );
}

#[tokio::test]
async fn run_usage_and_progress_accumulate_across_provider_calls() {
    let store = test_store().await;
    let (_, run) = seed_task(&store, "Usage accounting").await;
    store
        .claim_next_agent_run("worker:usage", "lease:usage", 120)
        .await
        .expect("claim")
        .expect("leased");
    store
        .transition_agent_run(
            &run.run_id,
            crate::RunStatus::Running,
            Some("lease:usage"),
            None,
        )
        .await
        .expect("running");
    for usage in [
        crate::TokenUsage {
            input_tokens: 100,
            output_tokens: 20,
            total_tokens: 120,
            cached_input_tokens: Some(40),
        },
        crate::TokenUsage {
            input_tokens: 70,
            output_tokens: 10,
            total_tokens: 80,
            cached_input_tokens: None,
        },
    ] {
        store
            .record_agent_run_observation(
                &run.run_id,
                "lease:usage",
                "codex",
                "gpt-test",
                Some(&usage),
            )
            .await
            .expect("usage");
    }
    store
        .record_agent_run_progress(&run.run_id, "lease:usage", 3, 250)
        .await
        .expect("progress");
    let heartbeat = store
        .heartbeat_agent_run(&run.run_id, "lease:usage", 120)
        .await
        .expect("heartbeat");
    assert!(!heartbeat.cancellation_requested);
    let updated = store
        .get_agent_run(&run.run_id)
        .await
        .expect("run")
        .expect("updated run");
    assert_eq!(updated.provider_call_count, 2);
    assert_eq!(updated.tool_call_count, 3);
    assert_eq!(updated.input_tokens, 170);
    assert_eq!(updated.cached_input_tokens, 40);
    assert_eq!(updated.output_tokens, 30);
    assert_eq!(updated.active_milliseconds, 250);
}

async fn seed_task(
    store: &crate::NoemaStore,
    title: &str,
) -> (crate::TaskRecord, crate::AgentRunRecord) {
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            crate::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider account");
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("task model settings")
        .into_iter()
        .find(|entry| entry.complexity == crate::TaskComplexity::Simple)
        .expect("simple task model");
    store
        .create_task_with_executor(crate::NewTask {
            task_id: None,
            title: title.to_string(),
            request_markdown: "Complete the task".to_string(),
            complexity: crate::TaskComplexity::Simple,
            owner_human_id: "human:local".to_string(),
            source: crate::TaskSource::default(),
            created_by_agent_id: "agent:primary".to_string(),
            creation_tool_call_id: None,
            pool_entry_id: pool.pool_entry_id,
            executor_model: pool.model.clone(),
            reviewer_model: pool.model,
            max_review_rounds: None,
            criteria: vec![crate::NewTaskValidationCriterion {
                criterion_id: None,
                ordinal: 1,
                description: "Task is complete".to_string(),
                expected_evidence: None,
            }],
        })
        .await
        .expect("task")
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
async fn sqlite_memory_article_cache_round_trip() {
    let store = test_store().await;

    store
        .save_memory_article_cache(crate::SaveMemoryArticleCache {
            scope_id: "human:local".to_string(),
            fact_fingerprint: "facts-v1".to_string(),
            article_markdown: "# Kevin\n\nLittle is currently known about Kevin.".to_string(),
            generated_at: "2026-07-08T20:00:00Z".to_string(),
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

#[tokio::test]
async fn idempotent_conversation_item_id_prevents_duplicate_task_delivery() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let item = || crate::NewConversationItem {
        conversation_id: conversation.conversation_id.clone(),
        turn_id: None,
        parent_item_id: None,
        kind: crate::ConversationItemKind::TaskReference,
        status: crate::ConversationItemStatus::Completed,
        author: crate::ActorRef::system("system:task-runtime"),
        content_text: Some("Task update".to_string()),
        payload_json: serde_json::json!({"task_id": "task:1", "status": "completed"}),
        metadata: serde_json::json!({"task_event_id": "event:1"}),
    };

    let first = store
        .append_conversation_item_with_id("item:task_status:event:1".to_string(), item())
        .await
        .expect("first delivery");
    let second = store
        .append_conversation_item_with_id("item:task_status:event:1".to_string(), item())
        .await
        .expect("idempotent delivery");
    let rows = store
        .list_conversation_items(&conversation.conversation_id, crate::ReplayMode::Audit)
        .await
        .expect("conversation items");

    assert_eq!(first.item_id, second.item_id);
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn durable_task_events_expose_pending_status_deliveries_until_materialized() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let (task, _) = seed_task(&store, "Pending delivery").await;
    store
        .with_connection(|conn| {
            conn.execute(
                "UPDATE tasks SET source_conversation_id = ?2, status = 'failed' WHERE task_id = ?1",
                rusqlite::params![task.task_id, conversation.conversation_id],
            )?;
            Ok(())
        })
        .await
        .expect("terminal task");
    let event_id = store
        .append_task_event(super::task_events::NewTaskEvent {
            event_id: Some("event:pending-delivery".to_string()),
            task_id: task.task_id.clone(),
            event_kind: "task.failed".to_string(),
            actor_id: "system:task-runtime".to_string(),
            causation_id: None,
            correlation_id: None,
            payload: serde_json::json!({}),
        })
        .await
        .expect("status event");
    assert_eq!(
        store
            .list_pending_task_status_deliveries(10)
            .await
            .expect("pending"),
        vec![task.task_id.clone()]
    );

    store
        .append_conversation_item_with_id(
            format!("item:task_status:{event_id}"),
            crate::NewConversationItem {
                conversation_id: conversation.conversation_id,
                turn_id: None,
                parent_item_id: None,
                kind: crate::ConversationItemKind::TaskReference,
                status: crate::ConversationItemStatus::Completed,
                author: crate::ActorRef::system("system:task-runtime"),
                content_text: Some("Pending delivery".to_string()),
                payload_json: serde_json::json!({"task_id": task.task_id}),
                metadata: serde_json::json!({"task_event_id": event_id}),
            },
        )
        .await
        .expect("delivery");
    assert!(
        store
            .list_pending_task_status_deliveries(10)
            .await
            .expect("drained")
            .is_empty()
    );
}
