use noema_artifacts::{ArtifactOwnerRef, ArtifactSource, CreateLocalArtifactRequest};
use noema_store::{WorkCommandService, WorkRunFence};
use serde_json::json;

use super::{
    TaskArtifactReadContext, execute_task_list_artifacts, execute_task_parse_artifact,
    execute_task_read_artifact,
};
use crate::daemon::artifact_tool::{
    ArtifactToolRuntimeContext, execute_artifact_create_local_file,
};

#[tokio::test]
async fn executor_reads_exact_versions_owned_by_its_task() {
    let fixture = executor_fixture("Executor artifact versions").await;
    let created =
        create_linked_artifact(&fixture, "linked.md", &["version one", "version two"]).await;
    let artifact_id = created["artifact_id"].as_str().expect("artifact id");
    let versions = created["versions"].as_array().expect("artifact versions");
    let first_version_id = versions[0]["artifact_version_id"]
        .as_str()
        .expect("first version id");

    let first = read_artifact(&fixture, artifact_id, Some(first_version_id))
        .await
        .expect("read exact linked version");
    let current = read_artifact(&fixture, artifact_id, None)
        .await
        .expect("read current linked version");

    assert_eq!(first["content"].as_str(), Some("version one"));
    assert_eq!(current["content"].as_str(), Some("version two"));
}

#[tokio::test]
async fn executor_reads_task_owned_artifacts_and_rejects_foreign_artifacts() {
    let fixture = executor_fixture("Executor artifact scope").await;
    let unlinked = fixture
        .artifact_operations
        .create_local_file(CreateLocalArtifactRequest {
            owner: ArtifactOwnerRef::task(fixture.read_context.task_id.clone()),
            title: "Unlinked".to_string(),
            description: None,
            artifact_kind: "document".to_string(),
            filename: "unlinked.md".to_string(),
            bytes: b"unlinked".to_vec(),
            media_type: Some("text/markdown".to_string()),
            created_by_actor_id: fixture.agent_id.clone(),
            source: ArtifactSource::default(),
            metadata: json!({}),
        })
        .await
        .expect("create unlinked artifact");
    let foreign_conversation = fixture
        .store
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .expect("create foreign artifact owner");
    let foreign = fixture
        .artifact_operations
        .create_local_file(CreateLocalArtifactRequest {
            owner: ArtifactOwnerRef::conversation(foreign_conversation.conversation_id),
            title: "Foreign".to_string(),
            description: None,
            artifact_kind: "document".to_string(),
            filename: "foreign.md".to_string(),
            bytes: b"foreign".to_vec(),
            media_type: Some("text/markdown".to_string()),
            created_by_actor_id: fixture.agent_id.clone(),
            source: ArtifactSource::default(),
            metadata: json!({"task_run_id": fixture.read_context.run_id.clone()}),
        })
        .await
        .expect("create foreign artifact");

    let task_owned = read_artifact(&fixture, &unlinked.artifact.artifact_id, None)
        .await
        .expect("task-owned artifact must be readable");
    let foreign_error = read_artifact(&fixture, &foreign.artifact.artifact_id, None)
        .await
        .expect_err("foreign artifact must be rejected");
    assert_eq!(task_owned["content"].as_str(), Some("unlinked"));
    assert!(foreign_error.contains("not owned by the current task"));
}

#[tokio::test]
async fn executor_reads_task_owned_artifact_from_prior_run() {
    let fixture = executor_fixture("Executor prior artifact scope").await;
    let created = create_linked_artifact(&fixture, "prior.md", &["prior run output"]).await;
    let artifact_id = created["artifact_id"]
        .as_str()
        .expect("prior artifact id")
        .to_string();
    let service = WorkCommandService::new(
        fixture.store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    service
        .report_work_run_failure(
            noema_store::ReportRunFailure {
                fence: fixture.fence.clone(),
                status: noema_tasks::RunStatus::Failed,
                error_code: noema_tasks::SafeErrorCode::new("prior_executor_failed")
                    .expect("safe code"),
                error_message: Some("retry with the same contract".to_string()),
                retryable: true,
            },
            "actor:test",
            None,
            "correlation:artifact-retry",
        )
        .await
        .expect("fail prior Executor");
    let successor = service
        .claim_next_work_run("worker:artifact-successor", 120, &[])
        .await
        .expect("claim successor")
        .expect("successor Executor");
    assert_ne!(successor.run.run_id, fixture.read_context.run_id);
    let successor_fence = WorkRunFence {
        run_id: successor.run.run_id.clone(),
        lease_token: successor.lease_token,
        task_generation: successor.run.task_generation,
    };
    service
        .start_work_run(
            &successor_fence,
            "actor:test",
            None,
            "correlation:artifact-successor",
        )
        .await
        .expect("start successor Executor");

    let successor_fixture = ExecutorArtifactFixture {
        store: fixture.store,
        read_context: TaskArtifactReadContext {
            task_id: fixture.read_context.task_id,
            run_id: successor.run.run_id,
        },
        artifact_operations: fixture.artifact_operations,
        agent_id: fixture.agent_id,
        fence: successor_fence,
    };
    let artifact = read_artifact(&successor_fixture, &artifact_id, None)
        .await
        .expect("prior task-owned artifact must be readable");

    assert_eq!(artifact["content"].as_str(), Some("prior run output"));
}

#[tokio::test]
async fn executor_lists_versions_owned_by_its_task() {
    let fixture = executor_fixture("Executor artifact receipts").await;
    fixture
        .artifact_operations
        .create_local_file(CreateLocalArtifactRequest {
            owner: ArtifactOwnerRef::task(fixture.read_context.task_id.clone()),
            title: "Private statement".to_string(),
            description: None,
            artifact_kind: "source_file".to_string(),
            filename: "statement.csv".to_string(),
            bytes: b"item,amount\nTransit,12.50\n".to_vec(),
            media_type: Some("text/csv".to_string()),
            created_by_actor_id: "human:local".to_string(),
            source: ArtifactSource::default(),
            metadata: json!({"filename": "statement.csv"}),
        })
        .await
        .expect("create source artifact");

    let listed = execute_task_list_artifacts(&fixture.store, &fixture.read_context, &json!({}))
        .await
        .expect("list artifacts");

    assert_eq!(listed["artifacts"][0]["title"], "Private statement");
    assert_eq!(listed["artifacts"][0]["byte_size"], 26);
    assert!(listed["artifacts"][0]["artifact_version_id"].is_string());
}

#[tokio::test]
async fn executor_parses_csv_artifact() {
    let fixture = executor_fixture("Executor artifact parse").await;
    let artifact = fixture
        .artifact_operations
        .create_local_file(CreateLocalArtifactRequest {
            owner: ArtifactOwnerRef::task(fixture.read_context.task_id.clone()),
            title: "Expense rows".to_string(),
            description: None,
            artifact_kind: "source_file".to_string(),
            filename: "expenses.csv".to_string(),
            bytes: b"item,amount\nTransit,12.50\n".to_vec(),
            media_type: Some("text/csv".to_string()),
            created_by_actor_id: "human:local".to_string(),
            source: ArtifactSource::default(),
            metadata: json!({"filename": "expenses.csv"}),
        })
        .await
        .expect("create CSV artifact");

    let parsed = execute_task_parse_artifact(
        &fixture.store,
        &fixture.artifact_operations,
        &fixture.read_context,
        &json!({"artifact_id": artifact.artifact.artifact_id}),
    )
    .await
    .expect("parse artifact");

    assert_eq!(parsed["metadata"]["filename"], "expenses.csv");
    assert_eq!(parsed["parse"]["status"], "converted");
    assert_eq!(parsed["parse"]["content"], "item,amount\nTransit,12.50\n");
}

struct ExecutorArtifactFixture {
    store: noema_store::NoemaStore,
    artifact_operations: noema_artifacts::ArtifactOperationsHandle,
    read_context: TaskArtifactReadContext,
    agent_id: String,
    fence: WorkRunFence,
}

async fn executor_fixture(title: &str) -> ExecutorArtifactFixture {
    let store = crate::test_support::test_store().await;
    let (task, queued_run) = crate::test_support::seed_task(&store, title).await;
    let service = WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    let claimed = service
        .claim_next_work_run("worker:artifact-test", 120, &[])
        .await
        .expect("claim executor")
        .expect("executor run");
    assert_eq!(claimed.run.run_id, queued_run.run_id);
    let fence = WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
    };
    service
        .start_work_run(&fence, "actor:test", None, "correlation:artifact-test")
        .await
        .expect("start executor");
    ExecutorArtifactFixture {
        artifact_operations: crate::test_support::artifact_operations(&store)
            .expect("artifact operations"),
        read_context: TaskArtifactReadContext {
            task_id: task.task_id.to_string(),
            run_id: claimed.run.run_id,
        },
        agent_id: claimed.run.agent_id,
        fence,
        store,
    }
}

async fn create_linked_artifact(
    fixture: &ExecutorArtifactFixture,
    filename: &str,
    contents: &[&str],
) -> serde_json::Value {
    let result = execute_artifact_create_local_file(
        &fixture.store,
        &fixture.artifact_operations,
        &ArtifactToolRuntimeContext {
            conversation_id: "conversation:artifact-test".to_string(),
            turn_id: "turn:artifact-test".to_string(),
            user_item_id: "item:artifact-test".to_string(),
            created_by_actor_id: fixture.agent_id.clone(),
            task_id: Some(fixture.read_context.task_id.clone()),
            task_run_id: Some(fixture.read_context.run_id.clone()),
        },
        None,
        &json!({
            "title": "Linked artifact",
            "artifact_kind": "document",
            "filename": filename,
            "media_type": "text/markdown",
            "versions": contents
                .iter()
                .map(|content| json!({"content": content}))
                .collect::<Vec<_>>(),
        }),
    )
    .await;
    assert!(
        result.success,
        "artifact creation failed: {}",
        result.payload
    );
    result.payload
}

async fn read_artifact(
    fixture: &ExecutorArtifactFixture,
    artifact_id: &str,
    artifact_version_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut arguments = json!({"artifact_id": artifact_id});
    if let Some(version_id) = artifact_version_id {
        arguments["artifact_version_id"] = json!(version_id);
    }
    execute_task_read_artifact(
        &fixture.store,
        &fixture.artifact_operations,
        &fixture.read_context,
        &arguments,
    )
    .await
}
