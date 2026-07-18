use super::{
    claim_and_start_run, first_criterion_id, ready_codex_registry, ready_provider_registry,
    seed_task, test_store,
};

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
    claim_and_start_run(&store, &run.run_id, "worker:blocked", "lease:blocked").await;
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
    let registry = ready_codex_registry();
    let (resumed, child) = store
        .resume_task_with_readiness(
            &task.task_id,
            "human:local",
            "human:local",
            Some("Use the personal account"),
            &registry,
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
    assert_eq!(
        store
            .get_agent_run(&run.run_id)
            .await
            .expect("parent run")
            .expect("persisted parent")
            .status,
        noema_tasks::RunStatus::Completed
    );
}

#[tokio::test]
async fn queue_leases_distinct_tasks_concurrently_without_overlapping_one_task() {
    let store = test_store().await;
    let (first_task, first_run) = seed_task(&store, "First concurrent task").await;
    let (second_task, _) = seed_task(&store, "Second concurrent task").await;
    let registry = ready_provider_registry(&first_run.model);
    store
        .create_agent_run_with_readiness(
            noema_tasks::NewAgentRun {
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
            },
            &registry,
        )
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
    let registry = ready_codex_registry();
    let child = store
        .claim_next_agent_run_with_readiness("worker:new", "lease:new", 120, &registry)
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
    claim_and_start_run(&store, &run.run_id, "worker:old", "lease:old").await;
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

    let registry = ready_codex_registry();
    let child = store
        .claim_next_agent_run_with_readiness("worker:new", "lease:new", 120, &registry)
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
    let registry = ready_codex_registry();
    claim_and_start_run(&store, &run.run_id, "worker:cancel", "lease:cancel").await;
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
    let criterion_id = first_criterion_id(&store, &task.task_id).await;
    assert!(
        store
            .create_task_submission_with_readiness(
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
                &registry,
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
    claim_and_start_run(&store, &run.run_id, "worker:fenced", "lease:fenced").await;

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
    claim_and_start_run(&store, &run.run_id, "worker:usage", "lease:usage").await;
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
