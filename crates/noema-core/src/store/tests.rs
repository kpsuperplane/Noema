use tempfile::TempDir;

use super::{NoemaStore, StoreConfig};

mod mcp;

fn store_config(root: &std::path::Path) -> StoreConfig {
    StoreConfig::new(root.join("db/noema.sqlite3"))
}

#[tokio::test]
async fn opens_sqlite_store_under_noema_db_dir() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());

    let store = NoemaStore::open(&config).await.expect("open store");

    assert!(home.path().join("db").exists());
    assert!(config.path.exists());
    assert_eq!(store.schema_version().await.expect("schema version"), 1);
}

#[tokio::test]
async fn opening_pre_v1_task_runtime_tables_rebuilds_and_preserves_history() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
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
    assert_eq!(
        run.execution_policy,
        noema_tasks::TaskExecutionPolicy::default()
    );
    assert_eq!(run.input_tokens, 0);
    assert_eq!(run.output_tokens, 0);
    let items = reopened
        .list_agent_run_items("run:legacy")
        .await
        .expect("legacy items");
    assert_eq!(items[0].content_text.as_deref(), Some("preserved"));
    assert_eq!(items[0].status, noema_tasks::AgentRunItemStatus::Completed);
}

#[tokio::test]
async fn opening_legacy_tasks_adds_blocking_columns() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    let store = NoemaStore::open(&config).await.expect("open store");
    store
        .with_connection(|conn| {
            conn.execute_batch(
                "ALTER TABLE tasks DROP COLUMN blocked_question; ALTER TABLE tasks DROP COLUMN blocked_context;",
            )?;
            Ok(())
        })
        .await
        .expect("install legacy task schema");
    drop(store);

    let reopened = NoemaStore::open(&config)
        .await
        .expect("upgrade legacy task schema");
    let columns = reopened
        .with_connection(|conn| {
            let mut statement = conn.prepare("PRAGMA table_info(tasks)")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("read task columns");
    assert!(columns.iter().any(|column| column == "blocked_question"));
    assert!(columns.iter().any(|column| column == "blocked_context"));
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
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");

    let artifact = store
        .create_artifact_with_initial_version(
            noema_artifacts::NewArtifact {
                artifact_id: None,
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    &conversation.conversation_id,
                ),
                title: "Sprint brief".to_string(),
                description: Some("Planning notes".to_string()),
                artifact_kind: "document".to_string(),
                storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource {
                    conversation_id: Some(conversation.conversation_id.clone()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({"provider": "notion"}),
            },
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Initial".to_string()),
                storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                    url: "https://notion.so/noema-brief".to_string(),
                },
                media_type: Some("text/html".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource {
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
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");

    let error = store
        .create_artifact_with_initial_version(
            noema_artifacts::NewArtifact {
                artifact_id: None,
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    &conversation.conversation_id,
                ),
                title: "Unsafe link".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: None,
                storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                    url: "javascript:alert(1)".to_string(),
                },
                media_type: None,
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
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
async fn append_artifact_version_updates_current_version() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let created = seed_external_artifact(&store, &conversation.conversation_id).await;

    let second = store
        .append_artifact_version(
            &created.artifact.artifact_id,
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Revision".to_string()),
                storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                    url: "https://notion.so/noema-brief-v2".to_string(),
                },
                media_type: Some("text/html".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
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
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let created = seed_external_artifact(&store, &conversation.conversation_id).await;

    let error = store
        .append_artifact_version(
            &created.artifact.artifact_id,
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Unsafe revision".to_string()),
                storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                    url: "file:///private/report.html".to_string(),
                },
                media_type: None,
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
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
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
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
    let home = TempDir::new().expect("temp store root");
    let database_path = home.path().join("db/noema.sqlite3");
    let config = StoreConfig::new(database_path.clone());

    assert_eq!(config.path, database_path);
    assert_eq!(StoreConfig::new(config.path.clone()), config);
}

pub(crate) async fn test_store() -> crate::NoemaStore {
    let home = TempDir::new().expect("temp store root");
    let store = crate::NoemaStore::open(&store_config(home.path()))
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
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider account");
    let model = noema_providers::ProviderSelectionSnapshot::explicit(
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
        .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
        .expect("simple task model");
    let (task, executor_run) = store
        .create_task_with_executor(noema_tasks::NewTask {
            task_id: None,
            title: "Lifecycle task".to_string(),
            request_markdown: "Produce a short result".to_string(),
            complexity: noema_tasks::TaskComplexity::Simple,
            owner_human_id: "human:local".to_string(),
            source: noema_tasks::TaskSource::default(),
            created_by_agent_id: "agent:primary".to_string(),
            creation_tool_call_id: None,
            pool_entry_id: pool.pool_entry_id,
            executor_model: pool.model.clone(),
            reviewer_model: model,
            max_review_rounds: None,
            criteria: vec![noema_tasks::NewTaskValidationCriterion {
                criterion_id: None,
                ordinal: 1,
                description: "Result is present".to_string(),
                expected_evidence: None,
            }],
        })
        .await
        .expect("task");
    assert_eq!(executor_run.run_kind, noema_tasks::RunKind::Executor);
    let executing_task = store
        .transition_task(
            &task.task_id,
            noema_tasks::TaskStatus::Executing,
            Some("test"),
        )
        .await
        .expect("executing");
    assert_eq!(executing_task.terminal_reason, None);
    store
        .claim_next_agent_run("worker:executor", "lease:executor", 120)
        .await
        .expect("claim executor")
        .expect("executor run");
    store
        .transition_agent_run(
            &executor_run.run_id,
            noema_tasks::RunStatus::Running,
            Some("lease:executor"),
            None,
        )
        .await
        .expect("run executor");
    let task_artifact = seed_local_artifact_metadata(
        &store,
        noema_artifacts::ArtifactOwnerRef::task(&task.task_id),
        "Task report",
        "document",
        "test/task-report/report.md",
        noema_tasks::TASK_EXECUTOR_AGENT_ID,
    )
    .await;
    let data_artifact = seed_local_artifact_metadata(
        &store,
        noema_artifacts::ArtifactOwnerRef::task(&task.task_id),
        "Task data",
        "data",
        "test/task-data/data.csv",
        noema_tasks::TASK_EXECUTOR_AGENT_ID,
    )
    .await;
    let criterion_id = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .expect("criteria")[0]
        .criterion_id
        .clone();
    let conversation = store
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let foreign_artifact = seed_local_artifact_metadata(
        &store,
        noema_artifacts::ArtifactOwnerRef::conversation(conversation.conversation_id),
        "Foreign artifact",
        "document",
        "test/foreign/foreign.txt",
        "agent:primary",
    )
    .await;
    assert!(
        store
            .create_task_submission(
                noema_tasks::NewTaskSubmission {
                    submission_id: None,
                    task_id: task.task_id.clone(),
                    executor_run_id: executor_run.run_id.clone(),
                    revision_index: 0,
                    summary: "Invalid".to_string(),
                    result_markdown: "Invalid".to_string(),
                    criteria: vec![noema_tasks::SubmissionCriterionEvidence {
                        criterion_id: criterion_id.clone(),
                        evidence_markdown: "Invalid".to_string(),
                    }],
                    artifact_ids: vec![foreign_artifact.artifact.artifact_id],
                },
                "lease:executor",
            )
            .await
            .is_err()
    );
    let submission_input = noema_tasks::NewTaskSubmission {
        submission_id: None,
        task_id: task.task_id.clone(),
        executor_run_id: executor_run.run_id.clone(),
        revision_index: 0,
        summary: "Done".to_string(),
        result_markdown: "# Result\n\nDone".to_string(),
        criteria: vec![noema_tasks::SubmissionCriterionEvidence {
            criterion_id: criterion_id.clone(),
            evidence_markdown: "The result is present".to_string(),
        }],
        artifact_ids: vec![
            task_artifact.artifact.artifact_id.clone(),
            data_artifact.artifact.artifact_id.clone(),
        ],
    };
    let first_submission = submission_input.clone();
    let concurrent_submission = submission_input.clone();
    let (first_result, concurrent_result) = tokio::join!(
        store.create_task_submission(first_submission, "lease:executor"),
        store.create_task_submission(concurrent_submission, "lease:executor"),
    );
    let (submission, reviewer_run) = first_result.expect("submission");
    let (concurrent_submission, concurrent_reviewer) =
        concurrent_result.expect("concurrent exact submission");
    assert_eq!(concurrent_submission, submission);
    assert_eq!(concurrent_reviewer.run_id, reviewer_run.run_id);
    assert_eq!(submission.artifacts.len(), 2);
    assert_eq!(
        submission.artifacts[0].artifact.artifact_id,
        task_artifact.artifact.artifact_id
    );
    assert_eq!(
        submission.artifacts[0].version.artifact_version_id,
        task_artifact.current_version.artifact_version_id
    );
    assert_eq!(
        submission.artifacts[1].artifact.artifact_id,
        data_artifact.artifact.artifact_id
    );
    let (replayed_submission, replayed_reviewer) = store
        .create_task_submission(submission_input.clone(), "lease:executor")
        .await
        .expect("exact submission replay");
    assert_eq!(replayed_submission, submission);
    assert_eq!(replayed_reviewer.run_id, reviewer_run.run_id);
    let conflicting_submission = store
        .create_task_submission(
            noema_tasks::NewTaskSubmission {
                summary: "Different summary".to_string(),
                ..submission_input
            },
            "lease:executor",
        )
        .await
        .expect_err("conflicting submission replay");
    assert!(
        conflicting_submission
            .to_string()
            .contains("different submission")
    );
    store
        .claim_next_agent_run("worker:reviewer", "lease:reviewer", 120)
        .await
        .expect("claim reviewer")
        .expect("reviewer run");
    store
        .transition_agent_run(
            &reviewer_run.run_id,
            noema_tasks::RunStatus::Running,
            Some("lease:reviewer"),
            None,
        )
        .await
        .expect("run reviewer");
    let review_input = noema_tasks::NewTaskReview {
        review_id: None,
        task_id: task.task_id.clone(),
        reviewer_run_id: reviewer_run.run_id.clone(),
        reviewed_submission_id: submission.submission_id.clone(),
        overall_verdict: noema_tasks::TaskReviewVerdict::Approve,
        overall_feedback: "All criteria pass".to_string(),
        criteria: vec![noema_tasks::TaskReviewCriterion {
            criterion_id: store
                .list_task_validation_criteria(&task.task_id)
                .await
                .expect("criteria")[0]
                .criterion_id
                .clone(),
            outcome: noema_tasks::CriterionOutcome::Pass,
            evidence_markdown: Some("Verified".to_string()),
            feedback: None,
        }],
    };
    let first_review = review_input.clone();
    let concurrent_review = review_input.clone();
    let (first_result, concurrent_result) = tokio::join!(
        store.create_task_review(first_review, "lease:reviewer"),
        store.create_task_review(concurrent_review, "lease:reviewer"),
    );
    let completed = first_result.expect("review");
    let concurrent_completed = concurrent_result.expect("concurrent exact review");
    assert_eq!(concurrent_completed, completed);
    assert_eq!(completed.status, noema_tasks::TaskStatus::Completed);
    assert_eq!(
        completed.latest_run_id.as_deref(),
        Some(reviewer_run.run_id.as_str())
    );
    let replayed_completed = store
        .create_task_review(review_input.clone(), "lease:reviewer")
        .await
        .expect("exact review replay");
    assert_eq!(replayed_completed, completed);
    let conflicting_review = store
        .create_task_review(
            noema_tasks::NewTaskReview {
                overall_feedback: "Different feedback".to_string(),
                ..review_input
            },
            "lease:reviewer",
        )
        .await
        .expect_err("conflicting review replay");
    assert!(
        conflicting_review
            .to_string()
            .contains("different completed review")
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
            noema_providers::ProviderAccountStatus::Authenticated,
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
        .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
        .expect("simple task model");
    let (task, failed_run) = store
        .create_task_with_executor(noema_tasks::NewTask {
            task_id: None,
            title: "Retry task".to_string(),
            request_markdown: "Try once more".to_string(),
            complexity: noema_tasks::TaskComplexity::Simple,
            owner_human_id: "human:local".to_string(),
            source: noema_tasks::TaskSource::default(),
            created_by_agent_id: "agent:primary".to_string(),
            creation_tool_call_id: None,
            pool_entry_id: pool.pool_entry_id,
            executor_model: pool.model.clone(),
            reviewer_model: pool.model,
            max_review_rounds: None,
            criteria: vec![noema_tasks::NewTaskValidationCriterion {
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
            noema_tasks::RunStatus::Failed,
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

    assert_eq!(retried_task.status, noema_tasks::TaskStatus::Queued);
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
        noema_tasks::TaskExecutionPolicy::default()
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
async fn human_continuation_after_a_completed_review_queues_a_new_executor_revision() {
    let store = test_store().await;
    let (task, executor_run) = seed_task(&store, "Human-guided revision").await;
    store
        .with_connection(|conn| {
            conn.execute(
                "UPDATE tasks SET max_review_rounds = 1 WHERE task_id = ?1",
                [&task.task_id],
            )?;
            Ok(())
        })
        .await
        .expect("one automatic review round");
    let leased_executor = store
        .claim_next_agent_run("worker:executor", "lease:executor", 120)
        .await
        .expect("claim executor")
        .expect("executor run");
    assert_eq!(leased_executor.run_id, executor_run.run_id);
    store
        .transition_agent_run(
            &executor_run.run_id,
            noema_tasks::RunStatus::Running,
            Some("lease:executor"),
            None,
        )
        .await
        .expect("run executor");
    store
        .transition_task(&task.task_id, noema_tasks::TaskStatus::Executing, None)
        .await
        .expect("execute task");
    let criterion_id = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .expect("criteria")[0]
        .criterion_id
        .clone();
    let (submission, reviewer_run) = store
        .create_task_submission(
            noema_tasks::NewTaskSubmission {
                submission_id: None,
                task_id: task.task_id.clone(),
                executor_run_id: executor_run.run_id,
                revision_index: 0,
                summary: "First attempt".to_string(),
                result_markdown: "Needs one human-guided revision".to_string(),
                criteria: vec![noema_tasks::SubmissionCriterionEvidence {
                    criterion_id: criterion_id.clone(),
                    evidence_markdown: "Incomplete evidence".to_string(),
                }],
                artifact_ids: Vec::new(),
            },
            "lease:executor",
        )
        .await
        .expect("submission");
    let leased_reviewer = store
        .claim_next_agent_run("worker:reviewer", "lease:reviewer", 120)
        .await
        .expect("claim reviewer")
        .expect("reviewer run");
    assert_eq!(leased_reviewer.run_id, reviewer_run.run_id);
    store
        .transition_agent_run(
            &reviewer_run.run_id,
            noema_tasks::RunStatus::Running,
            Some("lease:reviewer"),
            None,
        )
        .await
        .expect("run reviewer");
    let review_input = noema_tasks::NewTaskReview {
        review_id: None,
        task_id: task.task_id.clone(),
        reviewer_run_id: reviewer_run.run_id.clone(),
        reviewed_submission_id: submission.submission_id.clone(),
        overall_verdict: noema_tasks::TaskReviewVerdict::RequestChanges,
        overall_feedback: "Ask the human before another revision".to_string(),
        criteria: vec![noema_tasks::TaskReviewCriterion {
            criterion_id,
            outcome: noema_tasks::CriterionOutcome::Fail,
            evidence_markdown: Some("The evidence is incomplete".to_string()),
            feedback: Some("Apply the human clarification".to_string()),
        }],
    };
    let waiting = store
        .create_task_review(review_input.clone(), "lease:reviewer")
        .await
        .expect("review");
    assert_eq!(waiting.status, noema_tasks::TaskStatus::WaitingForHuman);
    let committed_review = store
        .list_task_reviews(&task.task_id)
        .await
        .expect("reviews")
        .pop()
        .expect("committed review");

    let duplicate_error = store
        .create_task_review(
            noema_tasks::NewTaskReview {
                reviewer_run_id: "run:redundant-reviewer".to_string(),
                ..review_input
            },
            "lease:redundant",
        )
        .await
        .expect_err("a different reviewer cannot review the same submission again");
    assert!(
        duplicate_error
            .to_string()
            .contains("continue with a new executor revision")
    );

    let (resumed_task, child) = store
        .resume_task(
            &task.task_id,
            "human:local",
            "human:local",
            Some("Use the clarified interpretation"),
        )
        .await
        .expect("resume with human guidance");
    assert_eq!(resumed_task.status, noema_tasks::TaskStatus::Queued);
    assert_eq!(
        resumed_task.latest_run_id.as_deref(),
        Some(child.run_id.as_str())
    );
    assert_eq!(child.run_kind, noema_tasks::RunKind::Executor);
    assert_eq!(child.agent_id, noema_tasks::TASK_EXECUTOR_AGENT_ID);
    assert_eq!(child.revision_index, 1);
    assert_eq!(child.attempt_index, 0);
    assert_eq!(
        child.parent_run_id.as_deref(),
        Some(reviewer_run.run_id.as_str())
    );
    assert_eq!(
        child.triggering_review_id.as_deref(),
        Some(committed_review.review_id.as_str())
    );
    assert_eq!(child.triggering_submission_id, None);
    assert_eq!(
        child.resume_message.as_deref(),
        Some("Use the clarified interpretation")
    );
}

#[tokio::test]
async fn agent_run_items_round_trip_in_sequence_order() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let run = store
        .create_agent_run(noema_tasks::NewAgentRun {
            run_id: Some("run:test".to_string()),
            task_id: "task:test".to_string(),
            run_kind: noema_tasks::RunKind::Executor,
            agent_id: noema_tasks::TASK_EXECUTOR_AGENT_ID.to_string(),
            revision_index: 0,
            attempt_index: 0,
            parent_run_id: None,
            triggering_submission_id: None,
            triggering_review_id: None,
            model: noema_providers::ProviderSelectionSnapshot::explicit(
                "codex",
                "provider_account:codex:default",
                "gpt-test",
                None,
                Some("test".to_string()),
            ),
            execution_policy: noema_tasks::TaskExecutionPolicy::default(),
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
            noema_tasks::RunStatus::Running,
            Some("lease:test"),
            None,
        )
        .await
        .expect("running");
    store
        .append_agent_run_item(
            noema_tasks::NewAgentRunItem {
                item_id: Some("run_item:1".to_string()),
                run_id: "run:test".to_string(),
                round_index: 0,
                kind: noema_tasks::AgentRunItemKind::AssistantOutput,
                status: noema_tasks::AgentRunItemStatus::Completed,
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
            noema_tasks::NewAgentRunItem {
                item_id: Some("run_item:2".to_string()),
                run_id: "run:test".to_string(),
                round_index: 0,
                kind: noema_tasks::AgentRunItemKind::ToolCall,
                status: noema_tasks::AgentRunItemStatus::Completed,
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
                noema_tasks::NewAgentRunItem {
                    item_id: Some(format!("run_item:{index}")),
                    run_id: "run:test".to_string(),
                    round_index: 1,
                    kind: noema_tasks::AgentRunItemKind::AssistantOutput,
                    status: noema_tasks::AgentRunItemStatus::Completed,
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
    assert_eq!(items[1].kind, noema_tasks::AgentRunItemKind::ToolCall);

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
async fn create_agent_run_rejects_unpersistable_provider_instance_identity() {
    let store = test_store().await;
    let mut model = noema_providers::ProviderSelectionSnapshot::explicit(
        "openai",
        "provider_account:openai:default",
        "gpt-5.5",
        None,
        None,
    );
    model.provider_instance_key =
        Some(noema_providers::ProviderInstanceKey::new("openai:default:1").unwrap());

    let error = store
        .create_agent_run(noema_tasks::NewAgentRun {
            run_id: Some("run:instance-key".to_string()),
            task_id: "task:instance-key".to_string(),
            run_kind: noema_tasks::RunKind::Executor,
            agent_id: noema_tasks::TASK_EXECUTOR_AGENT_ID.to_string(),
            revision_index: 0,
            attempt_index: 0,
            parent_run_id: None,
            triggering_submission_id: None,
            triggering_review_id: None,
            model,
            execution_policy: noema_tasks::TaskExecutionPolicy::default(),
            priority: 0,
        })
        .await
        .expect_err("legacy storage must reject exact instance identity");

    assert!(matches!(
        error,
        crate::StoreError::InvariantViolation { message }
            if message.contains("cannot store an instance key")
    ));
    assert!(
        store
            .get_agent_run("run:instance-key")
            .await
            .expect("read rejected run")
            .is_none()
    );
}

#[tokio::test]
async fn task_execution_policy_is_global_and_snapshotted_on_new_runs() {
    let store = test_store().await;
    let defaults = store
        .get_task_execution_policy()
        .await
        .expect("default policy");
    assert_eq!(defaults, noema_tasks::TaskExecutionPolicy::default());
    let updated = noema_tasks::TaskExecutionPolicy {
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
            noema_tasks::RunStatus::Running,
            Some("lease:blocked"),
            None,
        )
        .await
        .expect("running");
    store
        .transition_task(&task.task_id, noema_tasks::TaskStatus::Executing, None)
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
    assert_eq!(blocked.status, noema_tasks::TaskStatus::WaitingForHuman);
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
    assert_eq!(resumed.status, noema_tasks::TaskStatus::Queued);
    assert_eq!(resumed.blocked_question, None);
    assert_eq!(child.parent_run_id.as_deref(), Some(run.run_id.as_str()));
    assert_eq!(
        child.resume_message.as_deref(),
        Some("Use the personal account")
    );
}

#[tokio::test]
async fn queue_leases_distinct_tasks_concurrently_without_overlapping_one_task() {
    let store = test_store().await;
    let (first_task, first_run) = seed_task(&store, "First concurrent task").await;
    let (second_task, _) = seed_task(&store, "Second concurrent task").await;
    store
        .create_agent_run(noema_tasks::NewAgentRun {
            run_id: None,
            task_id: first_task.task_id.clone(),
            run_kind: noema_tasks::RunKind::Executor,
            agent_id: noema_tasks::TASK_EXECUTOR_AGENT_ID.to_string(),
            revision_index: first_run.revision_index,
            attempt_index: first_run.attempt_index + 1,
            parent_run_id: Some(first_run.run_id.clone()),
            triggering_submission_id: None,
            triggering_review_id: None,
            model: first_run.model.clone(),
            execution_policy: first_run.execution_policy,
            priority: first_run.priority,
        })
        .await
        .expect("same-task queued run");

    let first_claim = store
        .claim_next_agent_run("worker:concurrent", "lease:first", 120)
        .await
        .expect("first claim")
        .expect("first leased run");
    let second_claim = store
        .claim_next_agent_run("worker:concurrent", "lease:second", 120)
        .await
        .expect("second claim")
        .expect("second leased run");

    assert_ne!(first_claim.task_id, second_claim.task_id);
    assert!(
        [first_claim.task_id.as_str(), second_claim.task_id.as_str()]
            .contains(&first_task.task_id.as_str())
    );
    assert!(
        [first_claim.task_id.as_str(), second_claim.task_id.as_str()]
            .contains(&second_task.task_id.as_str())
    );
    assert!(
        store
            .claim_next_agent_run("worker:concurrent", "lease:blocked-sibling", 120)
            .await
            .expect("same-task overlap check")
            .is_none()
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
        .transition_task(&task.task_id, noema_tasks::TaskStatus::Executing, None)
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
        noema_tasks::RunStatus::Interrupted
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
            noema_tasks::RunStatus::Running,
            Some("lease:old"),
            None,
        )
        .await
        .expect("running");
    store
        .transition_task(&task.task_id, noema_tasks::TaskStatus::Executing, None)
        .await
        .expect("executing");
    store
        .transition_agent_run(
            &run.run_id,
            noema_tasks::RunStatus::Interrupted,
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
            noema_tasks::RunStatus::Running,
            Some("lease:cancel"),
            None,
        )
        .await
        .expect("running");
    store
        .transition_task(&task.task_id, noema_tasks::TaskStatus::Executing, None)
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
                noema_tasks::RunStatus::Failed,
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
                noema_tasks::NewTaskSubmission {
                    submission_id: None,
                    task_id: task.task_id.clone(),
                    executor_run_id: run.run_id.clone(),
                    revision_index: 0,
                    summary: "Late result".to_string(),
                    result_markdown: "Late result".to_string(),
                    criteria: vec![noema_tasks::SubmissionCriterionEvidence {
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
    assert_eq!(cancelled.status, noema_tasks::TaskStatus::Cancelled);
    assert!(
        store
            .list_agent_runs_for_task(&task.task_id)
            .await
            .expect("runs")
            .iter()
            .all(|candidate| candidate.run_kind != noema_tasks::RunKind::Reviewer)
    );
}

#[tokio::test]
async fn leased_agent_run_rejects_tokenless_transition() {
    let store = test_store().await;
    let (_task, run) = seed_task(&store, "Lease fencing").await;
    store
        .claim_next_agent_run("worker:fenced", "lease:fenced", 120)
        .await
        .expect("claim")
        .expect("leased run");
    store
        .transition_agent_run(
            &run.run_id,
            noema_tasks::RunStatus::Running,
            Some("lease:fenced"),
            None,
        )
        .await
        .expect("running");

    assert!(
        store
            .transition_agent_run(
                &run.run_id,
                noema_tasks::RunStatus::Failed,
                None,
                Some(("stale_worker".to_string(), "stale".to_string())),
            )
            .await
            .is_err()
    );
    assert_eq!(
        store
            .get_agent_run(&run.run_id)
            .await
            .expect("read run")
            .expect("run")
            .status,
        noema_tasks::RunStatus::Running
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
            noema_tasks::RunStatus::Running,
            Some("lease:usage"),
            None,
        )
        .await
        .expect("running");
    for usage in [
        noema_providers::TokenUsage {
            input_tokens: 100,
            output_tokens: 20,
            total_tokens: 120,
            cached_input_tokens: Some(40),
        },
        noema_providers::TokenUsage {
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

pub(crate) async fn seed_task(
    store: &crate::NoemaStore,
    title: &str,
) -> (noema_tasks::TaskRecord, noema_tasks::AgentRunRecord) {
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
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
        .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
        .expect("simple task model");
    store
        .create_task_with_executor(noema_tasks::NewTask {
            task_id: None,
            title: title.to_string(),
            request_markdown: "Complete the task".to_string(),
            complexity: noema_tasks::TaskComplexity::Simple,
            owner_human_id: "human:local".to_string(),
            source: noema_tasks::TaskSource::default(),
            created_by_agent_id: "agent:primary".to_string(),
            creation_tool_call_id: None,
            pool_entry_id: pool.pool_entry_id,
            executor_model: pool.model.clone(),
            reviewer_model: pool.model,
            max_review_rounds: None,
            criteria: vec![noema_tasks::NewTaskValidationCriterion {
                criterion_id: None,
                ordinal: 1,
                description: "Task is complete".to_string(),
                expected_evidence: None,
            }],
        })
        .await
        .expect("task")
}

async fn seed_local_artifact_metadata(
    store: &crate::NoemaStore,
    owner: noema_artifacts::ArtifactOwnerRef,
    title: &str,
    artifact_kind: &str,
    relative_path: &str,
    created_by_actor_id: &str,
) -> noema_artifacts::ArtifactWithVersions {
    store
        .create_artifact_with_initial_version(
            noema_artifacts::NewArtifact {
                artifact_id: None,
                owner,
                title: title.to_string(),
                description: None,
                artifact_kind: artifact_kind.to_string(),
                storage_kind: noema_artifacts::ArtifactStorageKind::LocalFile,
                created_by_actor_id: created_by_actor_id.to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: None,
                storage: noema_artifacts::ArtifactVersionStorage::LocalFile {
                    relative_path: relative_path.to_string(),
                },
                media_type: None,
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: created_by_actor_id.to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("seed local artifact metadata")
}

async fn seed_external_artifact(
    store: &crate::NoemaStore,
    conversation_id: &str,
) -> noema_artifacts::ArtifactWithVersions {
    store
        .create_artifact_with_initial_version(
            noema_artifacts::NewArtifact {
                artifact_id: None,
                owner: noema_artifacts::ArtifactOwnerRef::conversation(conversation_id),
                title: "Sprint brief".to_string(),
                description: Some("Planning notes".to_string()),
                artifact_kind: "document".to_string(),
                storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource {
                    conversation_id: Some(conversation_id.to_string()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({"provider": "notion"}),
            },
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Initial".to_string()),
                storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                    url: "https://notion.so/noema-brief".to_string(),
                },
                media_type: Some("text/html".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource {
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
            reasoning_effort: Some(noema_providers::ReasoningEffort::Medium),
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
        Some(noema_providers::ReasoningEffort::Medium)
    );
}

#[tokio::test]
async fn sqlite_memory_service_defaults_to_managed() {
    let store = test_store().await;

    let settings = store.memory_service_settings().await.expect("settings");

    assert_eq!(settings.mode, noema_memory::MemoryServiceMode::Managed);
    assert_eq!(settings.base_url, None);
    assert_eq!(settings.port, None);
}

#[tokio::test]
async fn sqlite_memory_service_settings_round_trip_external() {
    let store = test_store().await;

    store
        .save_memory_service_settings(noema_memory::SaveMemoryServiceSettings {
            mode: noema_memory::MemoryServiceMode::External,
            base_url: Some("http://127.0.0.1:7777".to_string()),
            port: None,
            provider_account_id: Some("provider_account:openai:default".to_string()),
            provider_kind: Some("openai".to_string()),
            model_profile: Some("gpt-5.1".to_string()),
            reasoning_effort: Some(noema_providers::ReasoningEffort::Low),
        })
        .await
        .expect("save settings");

    let settings = store.memory_service_settings().await.expect("settings");
    assert_eq!(settings.mode, noema_memory::MemoryServiceMode::External);
    assert_eq!(settings.base_url.as_deref(), Some("http://127.0.0.1:7777"));
    assert_eq!(
        settings.reasoning_effort,
        Some(noema_providers::ReasoningEffort::Low)
    );
}

#[tokio::test]
async fn sqlite_memory_article_cache_round_trip() {
    let store = test_store().await;

    store
        .save_memory_article_cache(noema_memory::SaveMemoryArticleCache {
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
        .create_conversation_turn(noema_conversations::NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("turn");

    store
        .append_conversation_item(noema_conversations::NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: noema_conversations::ConversationItemKind::UserText,
            status: noema_conversations::ConversationItemStatus::Completed,
            author: noema_conversations::ActorRef::human("human:local")
                .expect("static local human actor id must be valid"),
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

    store
        .update_conversation_agent_status(
            &conversation.conversation_id,
            noema_conversations::AgentStatus::Thinking,
        )
        .await
        .expect("mark conversation active");
    store
        .recover_shutdown_cancelled_work(&conversation.conversation_id)
        .await
        .expect("recover interrupted conversation");
    let status = store
        .conversation_runtime_status(&conversation.conversation_id)
        .await
        .expect("read recovered conversation state")
        .expect("recovered conversation status");
    assert_eq!(
        status.turn_status,
        noema_conversations::ConversationTurnStatus::Cancelled
    );
    assert_eq!(status.agent_status, noema_conversations::AgentStatus::Idle);
}

#[tokio::test]
async fn idempotent_conversation_item_id_prevents_duplicate_task_delivery() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    let item = || noema_conversations::NewConversationItem {
        conversation_id: conversation.conversation_id.clone(),
        turn_id: None,
        parent_item_id: None,
        kind: noema_conversations::ConversationItemKind::TaskReference,
        status: noema_conversations::ConversationItemStatus::Completed,
        author: noema_conversations::ActorRef::system("system:task-runtime")
            .expect("static task runtime actor id must be valid"),
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
        .list_conversation_items(
            &conversation.conversation_id,
            noema_conversations::ReplayMode::Audit,
        )
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
        .append_task_event(noema_tasks::NewTaskEvent {
            event_id: Some("event:pending-delivery".to_string()),
            task_id: task.task_id.clone(),
            event_kind: noema_tasks::TaskEventKind::new("task.failed").expect("valid event kind"),
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
    assert_eq!(
        store
            .list_pending_task_completion_deliveries(10)
            .await
            .expect("pending completion"),
        vec![(task.task_id.clone(), event_id.clone())]
    );

    store
        .append_conversation_item_with_id(
            format!("item:task_status:{event_id}"),
            noema_conversations::NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: None,
                parent_item_id: None,
                kind: noema_conversations::ConversationItemKind::TaskReference,
                status: noema_conversations::ConversationItemStatus::Completed,
                author: noema_conversations::ActorRef::system("system:task-runtime")
                    .expect("static task runtime actor id must be valid"),
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

    store
        .append_conversation_item_with_id(
            format!("item:task_completion:{event_id}"),
            noema_conversations::NewConversationItem {
                conversation_id: conversation.conversation_id,
                turn_id: None,
                parent_item_id: None,
                kind: noema_conversations::ConversationItemKind::AssistantText,
                status: noema_conversations::ConversationItemStatus::Completed,
                author: noema_conversations::ActorRef::agent("agent:primary")
                    .expect("static primary agent id must be valid"),
                content_text: Some("The task failed.".to_string()),
                payload_json: serde_json::json!({"task_id": task.task_id}),
                metadata: serde_json::json!({"delivery_id": event_id}),
            },
        )
        .await
        .expect("completion delivery");
    assert!(
        store
            .list_pending_task_completion_deliveries(10)
            .await
            .expect("completion drained")
            .is_empty()
    );
}
