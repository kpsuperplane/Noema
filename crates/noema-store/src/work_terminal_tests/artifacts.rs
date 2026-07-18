use noema_artifacts::{
    ArtifactOwnerRef, ArtifactSource, ArtifactStorageKind, ArtifactVersionStorage, NewArtifact,
    NewArtifactVersion,
};
use noema_tasks::{
    RequestTaskChanges, RunStatus, TaskContractAmendment, TaskReviewVerdict, WorkCommand,
    WorkDomainError,
};
use serde_json::{Value, json};

use super::*;

async fn create_artifact(
    fixture: &ExecutorFixture,
    artifact_id: &str,
    creator_id: &str,
    metadata: Value,
) -> noema_artifacts::ArtifactWithVersions {
    let suffix = artifact_id
        .strip_prefix("artifact:")
        .expect("artifact test id namespace");
    fixture
        .store
        .create_artifact_with_initial_version(
            NewArtifact {
                artifact_id: Some(artifact_id.to_string()),
                owner: ArtifactOwnerRef::task(fixture.task.task_id.as_str()),
                title: format!("Artifact {suffix}"),
                description: None,
                artifact_kind: "document".to_string(),
                storage_kind: ArtifactStorageKind::LocalFile,
                created_by_actor_id: creator_id.to_string(),
                source: ArtifactSource::default(),
                metadata,
            },
            NewArtifactVersion {
                artifact_version_id: Some(format!("artifact_version:{suffix}:1")),
                title: None,
                storage: ArtifactVersionStorage::LocalFile {
                    relative_path: format!("work-terminal-tests/{suffix}.md"),
                },
                media_type: Some("text/markdown".to_string()),
                byte_size: Some(1),
                content_sha256: None,
                created_by_actor_id: creator_id.to_string(),
                source: ArtifactSource::default(),
                metadata: json!({}),
            },
        )
        .await
        .expect("create task artifact")
}

async fn assert_executor_write_rolled_back(fixture: &ExecutorFixture) {
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM task_submissions WHERE executor_run_id = ?1",
            &fixture.executor_fence.run_id,
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM agent_runs WHERE parent_run_id = ?1 AND run_kind = 'reviewer'",
            &fixture.executor_fence.run_id,
        )
        .await,
        0
    );
    let run = fixture
        .store
        .get_work_run_record(&fixture.executor_fence.run_id)
        .await
        .expect("load executor")
        .expect("executor exists");
    assert_eq!(run.status, RunStatus::Running);
}

#[tokio::test]
async fn executor_submission_snapshots_an_artifact_from_the_exact_run() {
    let fixture = fixture().await;
    let executor = fixture
        .store
        .get_work_run_record(&fixture.executor_fence.run_id)
        .await
        .expect("load executor")
        .expect("executor exists");
    let artifact = create_artifact(
        &fixture,
        "artifact:exact-executor",
        &executor.agent_id,
        json!({
            "task_id": fixture.task.task_id.as_str(),
            "task_run_id": fixture.executor_fence.run_id,
        }),
    )
    .await;

    fixture
        .service
        .record_work_run_terminal(
            executor_submission(
                &fixture,
                "artifact result",
                "artifact evidence",
                vec![artifact.artifact.artifact_id.clone()],
            ),
            ACTOR,
            None,
            "correlation:terminal:artifact:exact",
        )
        .await
        .expect("submit exact run artifact");

    fixture
        .store
        .append_artifact_version(
            &artifact.artifact.artifact_id,
            NewArtifactVersion {
                artifact_version_id: Some("artifact_version:exact-executor:2".to_string()),
                title: None,
                storage: ArtifactVersionStorage::LocalFile {
                    relative_path: "work-terminal-tests/exact-executor-v2.md".to_string(),
                },
                media_type: Some("text/markdown".to_string()),
                byte_size: Some(2),
                content_sha256: None,
                created_by_actor_id: executor.agent_id,
                source: ArtifactSource::default(),
                metadata: json!({}),
            },
        )
        .await
        .expect("append later artifact version");
    let linked_version: String = fixture
        .store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT artifact_version_id FROM task_submission_artifacts WHERE artifact_id = ?1",
                    [artifact.artifact.artifact_id],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("load submission artifact snapshot");
    assert_eq!(linked_version, artifact.current_version.artifact_version_id);
}

#[tokio::test]
async fn executor_submission_rejects_metadata_free_foreign_and_prior_run_artifacts_atomically() {
    let fixture = fixture().await;
    let executor = fixture
        .store
        .get_work_run_record(&fixture.executor_fence.run_id)
        .await
        .expect("load executor")
        .expect("executor exists");
    let planner = fixture
        .store
        .get_work_run_record(
            executor
                .parent_run_id
                .as_deref()
                .expect("executor has planner parent"),
        )
        .await
        .expect("load planner")
        .expect("planner exists");
    let artifacts = [
        create_artifact(
            &fixture,
            "artifact:metadata-free",
            &executor.agent_id,
            json!({}),
        )
        .await,
        create_artifact(
            &fixture,
            "artifact:foreign-creator",
            "agent:foreign",
            json!({
                "task_id": fixture.task.task_id.as_str(),
                "task_run_id": fixture.executor_fence.run_id,
            }),
        )
        .await,
        create_artifact(
            &fixture,
            "artifact:prior-run",
            &planner.agent_id,
            json!({
                "task_id": fixture.task.task_id.as_str(),
                "task_run_id": planner.run_id,
            }),
        )
        .await,
    ];

    for artifact in artifacts {
        let error = fixture
            .service
            .record_work_run_terminal(
                executor_submission(
                    &fixture,
                    "invalid artifact",
                    "invalid artifact evidence",
                    vec![artifact.artifact.artifact_id],
                ),
                ACTOR,
                None,
                "correlation:terminal:artifact:rejected",
            )
            .await
            .expect_err("artifact outside the exact executor run must fail");
        assert!(matches!(
            error,
            StoreError::Work(WorkDomainError::RunFenced)
        ));
        assert_executor_write_rolled_back(&fixture).await;
    }
}

#[tokio::test]
async fn executor_submission_rejects_an_artifact_from_the_previous_contract() {
    let fixture = fixture().await;
    let first_executor = fixture
        .store
        .get_work_run_record(&fixture.executor_fence.run_id)
        .await
        .expect("load first executor")
        .expect("first executor exists");
    let old_artifact = create_artifact(
        &fixture,
        "artifact:old-contract",
        &first_executor.agent_id,
        json!({
            "task_id": fixture.task.task_id.as_str(),
            "task_run_id": first_executor.run_id,
        }),
    )
    .await;
    let reviewer = running_reviewer(&fixture, "worker:terminal:artifact:reviewer").await;
    fixture
        .service
        .record_work_run_terminal(
            review_terminal(
                &fixture,
                &reviewer,
                "review:artifact-old-contract",
                TaskReviewVerdict::Approve,
                None,
            ),
            ACTOR,
            None,
            "correlation:terminal:artifact:approve",
        )
        .await
        .expect("approve first contract");
    let approved = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load approved task")
        .expect("approved task exists")
        .task;
    fixture
        .service
        .execute(WorkCommand::RequestTaskChanges(RequestTaskChanges {
            meta: metadata("terminal:artifact:request-changes"),
            precondition: TaskPrecondition {
                task_id: approved.task_id.clone(),
                expected_revision: approved.revision,
                expected_generation: approved.generation,
            },
            amendment: TaskContractAmendment {
                feedback_markdown: "Revise the approved result.".to_string(),
                request_markdown: Some("Execute the revised artifact test.".to_string()),
                replacement_criteria: None,
                complexity: None,
            },
        }))
        .await
        .expect("create revised contract");
    let revised = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load revised task")
        .expect("revised task exists");
    let revised_contract = revised.current_contract.expect("revised contract");
    let claim = fixture
        .service
        .claim_next_work_run("worker:terminal:artifact:revised-executor", 60, &[])
        .await
        .expect("claim revised executor")
        .expect("revised executor exists");
    let revised_fence = WorkRunFence {
        run_id: claim.run.run_id.clone(),
        lease_token: claim.lease_token,
        task_generation: revised.task.generation,
        contract_id: Some(revised_contract.contract_id.clone()),
    };
    fixture
        .service
        .start_work_run(
            &revised_fence,
            ACTOR,
            None,
            "correlation:terminal:artifact:revised-start",
        )
        .await
        .expect("start revised executor");
    let error = fixture
        .service
        .record_work_run_terminal(
            WorkRunTerminal::TaskResult(SubmitTaskResult {
                fence: revised_fence.clone(),
                submission: NewTaskSubmission {
                    submission_id: Some("submission:revised-artifact-test".to_string()),
                    task_id: revised.task.task_id,
                    contract_id: revised_contract.contract_id,
                    executor_run_id: revised_fence.run_id.clone(),
                    review_round: claim.run.review_round,
                    summary: "Revised result".to_string(),
                    result_markdown: "Revised result body".to_string(),
                    criteria: revised_contract
                        .criteria
                        .iter()
                        .map(|criterion| SubmissionCriterionEvidence {
                            criterion_id: criterion.criterion_id.clone(),
                            evidence_markdown: "Revised evidence".to_string(),
                        })
                        .collect(),
                    artifact_ids: vec![old_artifact.artifact.artifact_id],
                },
            }),
            ACTOR,
            None,
            "correlation:terminal:artifact:old-contract",
        )
        .await
        .expect_err("old-contract artifact must fail");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::RunFenced)
    ));
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM task_submissions WHERE executor_run_id = ?1",
            &revised_fence.run_id,
        )
        .await,
        0
    );
}
