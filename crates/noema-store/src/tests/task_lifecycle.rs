use super::{ready_codex_registry, ready_provider_selection, seed_task, test_store};

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
    let ready_selection =
        ready_provider_selection(noema_providers::ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "gpt-5.6-luna",
            None,
            Some("test_configured_default".to_string()),
        ));
    store
        .initialize_missing_provider_selections(ready_selection.selection(), Some(&ready_selection))
        .await
        .expect("initialized provider selections");
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
    let registry = ready_codex_registry();
    let (task, executor_run) = store
        .create_task_with_executor_with_readiness(
            noema_tasks::NewTask {
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
            },
            &registry,
        )
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
        store.create_task_submission_with_readiness(first_submission, "lease:executor", &registry,),
        store.create_task_submission_with_readiness(
            concurrent_submission,
            "lease:executor",
            &registry,
        ),
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
    let ready_selection =
        ready_provider_selection(noema_providers::ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "gpt-5.6-luna",
            None,
            Some("test_configured_default".to_string()),
        ));
    store
        .initialize_missing_provider_selections(ready_selection.selection(), Some(&ready_selection))
        .await
        .expect("initialized provider selections");
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("task model settings")
        .into_iter()
        .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
        .expect("simple task model");
    let registry = ready_codex_registry();
    let (task, failed_run) = store
        .create_task_with_executor_with_readiness(
            noema_tasks::NewTask {
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
            },
            &registry,
        )
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
        .resume_task_with_readiness(
            &task.task_id,
            "human:local",
            "human:local",
            Some("Continue with the corrected configuration"),
            &registry,
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
    let registry = ready_codex_registry();
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
        .create_task_submission_with_readiness(
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
            &registry,
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
        .resume_task_with_readiness(
            &task.task_id,
            "human:local",
            "human:local",
            Some("Use the clarified interpretation"),
            &registry,
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
