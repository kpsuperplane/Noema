use super::*;

#[test]
fn terminal_contract_specs_are_typed_and_role_specific() {
    let result = task_submit_result_tool_spec().expect("result spec");
    assert_eq!(result.name.as_str(), TASK_SUBMIT_RESULT_TOOL);
    assert_eq!(
        result.input_schema.as_value()["required"],
        json!(["summary", "result_markdown", "criteria"])
    );

    let review = task_submit_review_tool_spec().expect("review spec");
    assert_eq!(review.name.as_str(), TASK_SUBMIT_REVIEW_TOOL);
    assert_eq!(
        review.input_schema.as_value()["properties"]["overall_verdict"]["enum"],
        json!(["approve", "request_changes", "needs_human"])
    );

    let blocked = task_report_blocked_tool_spec().expect("blocked spec");
    assert_eq!(blocked.name.as_str(), TASK_REPORT_BLOCKED_TOOL);
    assert_eq!(
        blocked.input_schema.as_value()["required"],
        json!(["question", "work_summary"])
    );
}

#[tokio::test]
async fn inspect_and_resume_tools_use_canonical_task_state() {
    let store = crate::test_support::test_store().await;
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
    crate::test_support::initialize_codex_provider_selections(&store).await;
    let registry = crate::test_support::ready_test_provider_registry();
    let pool = store
        .ensure_default_task_model_pool_settings_with_readiness("codex", registry.as_ref())
        .await
        .expect("task model settings")
        .into_iter()
        .find(|entry| entry.complexity == TaskComplexity::Simple)
        .expect("simple task model");
    let (task, run) = store
        .create_task_with_executor_with_readiness(
            NewTask {
                task_id: None,
                title: "Inspectable task".to_string(),
                request_markdown: "Inspect me".to_string(),
                complexity: TaskComplexity::Simple,
                owner_human_id: "human:local".to_string(),
                source: TaskSource::default(),
                created_by_agent_id: "agent:primary".to_string(),
                creation_tool_call_id: None,
                pool_entry_id: pool.pool_entry_id,
                executor_model: pool.model.clone(),
                reviewer_model: pool.model,
                max_review_rounds: None,
                criteria: vec![NewTaskValidationCriterion {
                    criterion_id: None,
                    ordinal: 1,
                    description: "Completes".to_string(),
                    expected_evidence: None,
                }],
            },
            registry.as_ref(),
        )
        .await
        .expect("task");
    let context = TaskAccessRuntimeContext {
        owner_human_id: "human:local".to_string(),
        actor_id: "agent:primary".to_string(),
    };
    let arguments = json!({"task_id": task.task_id});

    let queued = execute_task_inspect(&store, &context, None, &arguments).await;
    assert!(queued.success);
    assert_eq!(queued.payload["status"], "queued");
    assert_eq!(queued.payload["latest_run"]["status"], "queued");
    assert_eq!(queued.payload["can_resume"], false);
    assert_eq!(queued.payload["can_cancel"], true);

    store
        .transition_agent_run(
            &run.run_id,
            noema_tasks::RunStatus::Failed,
            None,
            Some(("provider_error".to_string(), "model missing".to_string())),
        )
        .await
        .expect("failed run");
    let failed = execute_task_inspect(&store, &context, None, &arguments).await;
    assert_eq!(failed.payload["status"], "failed");
    assert_eq!(failed.payload["can_resume"], true);
    assert_eq!(
        failed.payload["latest_run"]["error_message"],
        "model missing"
    );

    let resumed = execute_task_resume(&store, registry.as_ref(), &context, None, &arguments).await;
    assert!(resumed.success);
    assert_eq!(resumed.payload["status"], "queued");
    assert_eq!(resumed.payload["attempt"], 1);

    let cancelled = execute_task_cancel(&store, &context, None, &arguments).await;
    assert!(cancelled.success);
    assert_eq!(cancelled.payload["status"], "cancelled");
}

#[tokio::test]
async fn reviewer_inspection_targets_the_submission_executor_run() {
    let store = crate::test_support::test_store().await;
    let (task, executor_run) = crate::test_support::seed_task(&store, "Review inspection").await;
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
    store
        .transition_task(&task.task_id, noema_tasks::TaskStatus::Executing, None)
        .await
        .expect("execute task");
    store
        .record_agent_run_progress(&executor_run.run_id, "lease:executor", 2, 40)
        .await
        .expect("executor progress");
    let criterion_id = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .expect("criteria")[0]
        .criterion_id
        .clone();
    let provider_registry = crate::test_support::ready_test_provider_registry();
    let (_, reviewer_run) = store
        .create_task_submission_with_readiness(
            noema_tasks::NewTaskSubmission {
                submission_id: None,
                task_id: task.task_id.clone(),
                executor_run_id: executor_run.run_id.clone(),
                revision_index: 0,
                summary: "Done".to_string(),
                result_markdown: "Done".to_string(),
                criteria: vec![noema_tasks::SubmissionCriterionEvidence {
                    criterion_id,
                    evidence_markdown: "Verified".to_string(),
                }],
                artifact_ids: Vec::new(),
            },
            "lease:executor",
            provider_registry.as_ref(),
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
            noema_tasks::RunStatus::Running,
            Some("lease:reviewer"),
            None,
        )
        .await
        .expect("run reviewer");

    let inspected = execute_task_inspect(
        &store,
        &TaskAccessRuntimeContext {
            owner_human_id: "human:local".to_string(),
            actor_id: TASK_REVIEWER_AGENT_ID.to_string(),
        },
        None,
        &json!({"task_id": task.task_id}),
    )
    .await;

    assert!(inspected.success);
    assert_eq!(
        inspected.payload["latest_run"]["run_id"],
        reviewer_run.run_id
    );
    assert_eq!(
        inspected.payload["transcript_run"]["run_id"],
        executor_run.run_id
    );
    assert_eq!(inspected.payload["policy_consumption"]["tool_calls"], 2);
}
