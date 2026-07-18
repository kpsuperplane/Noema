use super::*;

#[tokio::test]
async fn blocked_terminal_replays_after_the_gate_is_answered() {
    let fixture = fixture().await;
    let blocked = WorkRunTerminal::Blocked(ReportTaskBlocked {
        fence: fixture.executor_fence.clone(),
        gate_kind: TaskGateKind::Clarification,
        prompt_markdown: "Which durable choice should be used?".to_string(),
        context_markdown: "The executor needs one explicit choice.".to_string(),
    });
    let first = fixture
        .service
        .record_work_run_terminal(blocked.clone(), ACTOR, None, "correlation:terminal:blocked")
        .await
        .expect("report blocked");
    let waiting = fixture
        .service
        .store()
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load waiting task")
        .expect("waiting task exists");
    let gate = waiting.active_gate.expect("active clarification gate");
    fixture
        .service
        .execute(WorkCommand::AnswerTask(AnswerTask {
            meta: metadata("terminal:answer-blocked"),
            precondition: TaskPrecondition {
                task_id: waiting.task.task_id.clone(),
                expected_revision: waiting.task.revision,
                expected_generation: waiting.task.generation,
            },
            gate_id: gate.gate_id,
            answer: TaskGateAnswer {
                message_markdown: "Use the durable choice.".to_string(),
                approval_decision: None,
            },
        }))
        .await
        .expect("answer blocked gate");

    let replay = fixture
        .service
        .record_work_run_terminal(blocked, ACTOR, None, "correlation:terminal:blocked-replay")
        .await
        .expect("replay blocked terminal after answer");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.run_id, first.run_id);
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM task_gates WHERE originating_run_id = ?1",
            &fixture.executor_fence.run_id,
        )
        .await,
        1
    );
}

#[tokio::test]
async fn blocked_terminal_replays_after_answer_opens_configuration_recovery() {
    let fixture = fixture().await;
    let blocked = WorkRunTerminal::Blocked(ReportTaskBlocked {
        fence: fixture.executor_fence.clone(),
        gate_kind: TaskGateKind::Clarification,
        prompt_markdown: "Which unavailable route should be restored?".to_string(),
        context_markdown: "The executor needs the configured provider route.".to_string(),
    });
    let first = fixture
        .service
        .record_work_run_terminal(
            blocked.clone(),
            ACTOR,
            None,
            "correlation:terminal:blocked-config",
        )
        .await
        .expect("report blocked");
    let waiting = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load waiting task")
        .expect("waiting task exists");
    let original_gate = waiting.active_gate.expect("original gate");
    let unavailable_registry = ready_hosted_provider_registry(std::iter::empty::<&str>())
        .expect("empty provider registry");
    let unavailable_service = WorkCommandService::new(fixture.store.clone(), unavailable_registry);
    unavailable_service
        .execute(WorkCommand::AnswerTask(AnswerTask {
            meta: metadata("terminal:answer-blocked-config"),
            precondition: TaskPrecondition {
                task_id: waiting.task.task_id,
                expected_revision: waiting.task.revision,
                expected_generation: waiting.task.generation,
            },
            gate_id: original_gate.gate_id,
            answer: TaskGateAnswer {
                message_markdown: "Restore the exact configured provider route.".to_string(),
                approval_decision: None,
            },
        }))
        .await
        .expect("answer opens configuration recovery");
    let recovery = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load configuration recovery")
        .expect("configuration recovery exists")
        .active_gate
        .expect("configuration recovery gate");
    assert_eq!(recovery.kind, TaskGateKind::Recovery);
    assert_eq!(
        recovery.originating_run_id.as_deref(),
        Some(fixture.executor_fence.run_id.as_str())
    );
    assert_eq!(
        count(
            &fixture.store,
            "SELECT COUNT(*) FROM task_gates WHERE originating_run_id = ?1",
            &fixture.executor_fence.run_id,
        )
        .await,
        2
    );

    let replay = fixture
        .service
        .record_work_run_terminal(
            blocked,
            ACTOR,
            None,
            "correlation:terminal:blocked-config-replay",
        )
        .await
        .expect("replay original blocked terminal");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.gate_id, first.gate_id);
    assert_eq!(replay.run_id, first.run_id);
}

#[tokio::test]
async fn cancel_supersedes_open_gate_and_cancels_its_waiting_run() {
    let fixture = fixture().await;
    fixture
        .service
        .record_work_run_terminal(
            WorkRunTerminal::Blocked(ReportTaskBlocked {
                fence: fixture.executor_fence.clone(),
                gate_kind: TaskGateKind::Clarification,
                prompt_markdown: "Cancel this blocked task?".to_string(),
                context_markdown: "Cancellation must close the gate and its run.".to_string(),
            }),
            ACTOR,
            None,
            "correlation:terminal:cancel-blocked",
        )
        .await
        .expect("report blocked");
    let waiting = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load waiting task")
        .expect("waiting task exists");
    let gate_id = waiting.active_gate.expect("open gate").gate_id;
    let cancelled = fixture
        .service
        .execute(WorkCommand::CancelTask(CancelTask {
            meta: metadata("terminal:cancel-waiting"),
            precondition: TaskPrecondition {
                task_id: waiting.task.task_id,
                expected_revision: waiting.task.revision,
                expected_generation: waiting.task.generation,
            },
            reason: Some("No longer needed".to_string()),
        }))
        .await
        .expect("cancel waiting task")
        .task
        .expect("cancelled task");
    assert_eq!(cancelled.stage_id.as_str(), "stage:personal:cancelled");
    let (run_status, cancellation_requested, gate_state): (String, bool, String) = fixture
        .store
        .with_connection(|connection| {
            let run = connection.query_row(
                "SELECT status, cancellation_requested FROM agent_runs WHERE run_id = ?1",
                [fixture.executor_fence.run_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let gate_state = connection.query_row(
                "SELECT gate_state FROM task_gates WHERE gate_id = ?1",
                [gate_id.as_str()],
                |row| row.get(0),
            )?;
            Ok((run.0, run.1, gate_state))
        })
        .await
        .expect("load cancelled branch");
    assert_eq!(run_status, "cancelled");
    assert!(cancellation_requested);
    assert_eq!(gate_state, "superseded");
    let detail = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load cancelled detail")
        .expect("cancelled detail exists");
    assert!(detail.current_run.is_none());
    assert!(detail.active_gate.is_none());
}

#[tokio::test]
async fn cancel_fails_closed_when_active_gate_is_not_open() {
    let fixture = fixture().await;
    fixture
        .service
        .record_work_run_terminal(
            WorkRunTerminal::Blocked(ReportTaskBlocked {
                fence: fixture.executor_fence.clone(),
                gate_kind: TaskGateKind::Clarification,
                prompt_markdown: "Preserve atomic cancellation?".to_string(),
                context_markdown: "The task pointer will be made inconsistent.".to_string(),
            }),
            ACTOR,
            None,
            "correlation:terminal:cancel-missing-gate",
        )
        .await
        .expect("report blocked");
    let waiting = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load waiting task")
        .expect("waiting task exists");
    let gate_id = waiting.active_gate.expect("open gate").gate_id;
    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE task_gates SET gate_state = 'superseded', resolved_by_actor_id = ?2, resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE gate_id = ?1",
                    rusqlite::params![gate_id.as_str(), ACTOR],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("make active gate pointer inconsistent");
    let error = fixture
        .service
        .execute(WorkCommand::CancelTask(CancelTask {
            meta: metadata("terminal:cancel-missing-gate-command"),
            precondition: TaskPrecondition {
                task_id: waiting.task.task_id.clone(),
                expected_revision: waiting.task.revision,
                expected_generation: waiting.task.generation,
            },
            reason: None,
        }))
        .await
        .expect_err("cancel must fail closed");
    assert!(matches!(error, StoreError::InvariantViolation { .. }));
    let after: (String, i64, i64, Option<String>, String) = fixture
        .store
        .with_connection(|connection| {
            let task = connection.query_row(
                "SELECT stage_id, generation, revision, active_gate_id FROM tasks WHERE task_id = ?1",
                [waiting.task.task_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
            let run_status = connection.query_row(
                "SELECT status FROM agent_runs WHERE run_id = ?1",
                [fixture.executor_fence.run_id.as_str()],
                |row| row.get(0),
            )?;
            Ok((task.0, task.1, task.2, task.3, run_status))
        })
        .await
        .expect("load raw state after rollback");
    assert_eq!(after.0, waiting.task.stage_id.as_str());
    assert_eq!(after.1, i64::try_from(waiting.task.generation).unwrap());
    assert_eq!(after.2, i64::try_from(waiting.task.revision).unwrap());
    assert_eq!(after.3.as_deref(), Some(gate_id.as_str()));
    assert_eq!(after.4, "waiting_for_approval");
}

#[tokio::test]
async fn exhausted_failure_replays_its_original_gate_after_human_retry() {
    let fixture = fixture().await;
    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE agent_runs SET max_automatic_retries = 0 WHERE run_id = ?1",
                    [fixture.executor_fence.run_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("exhaust the run's frozen retry budget");
    let report = ReportRunFailure {
        fence: fixture.executor_fence.clone(),
        status: noema_tasks::RunStatus::Failed,
        error_code: noema_tasks::SafeErrorCode::new("terminal_exhausted").expect("safe error code"),
        error_message: Some("Safe exhausted failure".to_string()),
        retryable: true,
    };
    let first = fixture
        .service
        .report_work_run_failure(report.clone(), ACTOR, None, "correlation:terminal:failure")
        .await
        .expect("report exhausted failure");
    let waiting = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load recovery task")
        .expect("recovery task exists");
    let gate = waiting.active_gate.expect("recovery gate");
    fixture
        .service
        .execute(WorkCommand::RetryTask(RetryTask {
            meta: metadata("terminal:retry-failure"),
            precondition: TaskPrecondition {
                task_id: waiting.task.task_id,
                expected_revision: waiting.task.revision,
                expected_generation: waiting.task.generation,
            },
            gate_id: gate.gate_id,
            note: Some("Retry safely".to_string()),
        }))
        .await
        .expect("retry failure gate");

    let replay = fixture
        .service
        .report_work_run_failure(report, ACTOR, None, "correlation:terminal:failure-replay")
        .await
        .expect("late exact failure replay");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.gate_id, first.gate_id);
    assert_eq!(replay.run_id, first.run_id);
}

#[tokio::test]
async fn configuration_recovery_retry_keeps_all_six_parent_policy_fields() {
    let fixture = fixture().await;
    let unavailable_registry = ready_hosted_provider_registry(std::iter::empty::<&str>())
        .expect("empty provider registry");
    let unavailable_service = WorkCommandService::new(fixture.store.clone(), unavailable_registry);
    unavailable_service
        .report_work_run_failure(
            ReportRunFailure {
                fence: fixture.executor_fence.clone(),
                status: noema_tasks::RunStatus::Failed,
                error_code: noema_tasks::SafeErrorCode::new("provider_route_test")
                    .expect("safe error code"),
                error_message: Some("Configured provider route is unavailable".to_string()),
                retryable: true,
            },
            ACTOR,
            None,
            "correlation:terminal:provider-route-failure",
        )
        .await
        .expect("open configuration recovery");
    let waiting = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load configuration recovery")
        .expect("configuration recovery exists");
    let gate = waiting.active_gate.expect("configuration recovery gate");
    assert_eq!(gate.kind, TaskGateKind::Recovery);
    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE task_execution_contracts
                     SET max_provider_continuations = 9, max_tool_calls = 10,
                         max_active_minutes = 11, progress_audit_interval = 3,
                         max_automatic_retries = 0, max_review_rounds = 1
                     WHERE contract_id = ?1",
                    [fixture.contract.contract_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("mutate live contract policy after failure");
    fixture
        .service
        .execute(WorkCommand::RetryTask(RetryTask {
            meta: metadata("terminal:retry-provider-route"),
            precondition: TaskPrecondition {
                task_id: waiting.task.task_id,
                expected_revision: waiting.task.revision,
                expected_generation: waiting.task.generation,
            },
            gate_id: gate.gate_id,
            note: Some("The original route is available again.".to_string()),
        }))
        .await
        .expect("retry with restored provider route");
    let child = fixture
        .service
        .claim_next_work_run("worker:terminal:pinned-provider-retry", 60, &[])
        .await
        .expect("claim pinned retry")
        .expect("pinned retry exists");
    assert_eq!(child.run.parent_run_id, Some(fixture.executor_fence.run_id));
    assert_eq!(
        child.run.execution_policy,
        fixture.contract.execution_policy
    );
    assert_eq!(child.run.model, fixture.contract.executor_model);
}

#[tokio::test]
async fn needs_human_review_replays_its_gate_after_answer() {
    let fixture = fixture().await;
    let reviewer_fence = running_reviewer(&fixture, "worker:terminal:needs-human").await;
    let terminal = review_terminal(
        &fixture,
        &reviewer_fence,
        "review:needs-human-replay",
        TaskReviewVerdict::NeedsHuman,
        Some(TaskGateKind::Clarification),
    );
    let first = fixture
        .service
        .record_work_run_terminal(
            terminal.clone(),
            ACTOR,
            None,
            "correlation:terminal:needs-human",
        )
        .await
        .expect("review needs human");
    let waiting = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load waiting review")
        .expect("waiting review exists");
    let gate = waiting.active_gate.expect("review gate");
    fixture
        .service
        .execute(WorkCommand::AnswerTask(AnswerTask {
            meta: metadata("terminal:answer-review"),
            precondition: TaskPrecondition {
                task_id: waiting.task.task_id,
                expected_revision: waiting.task.revision,
                expected_generation: waiting.task.generation,
            },
            gate_id: gate.gate_id,
            answer: TaskGateAnswer {
                message_markdown: "Proceed with the reviewed choice.".to_string(),
                approval_decision: None,
            },
        }))
        .await
        .expect("answer review gate");
    let replay = fixture
        .service
        .record_work_run_terminal(
            terminal,
            ACTOR,
            None,
            "correlation:terminal:needs-human-replay",
        )
        .await
        .expect("late needs-human replay");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.gate_id, first.gate_id);
    assert_eq!(replay.run_id, first.run_id);
}

#[tokio::test]
async fn exhausted_request_changes_replays_its_gate_after_retry() {
    let fixture = fixture().await;
    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE task_execution_contracts SET max_review_rounds = 1 WHERE contract_id = ?1",
                    [fixture.contract.contract_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("exhaust review round at one");
    let reviewer_fence = running_reviewer(&fixture, "worker:terminal:review-exhausted").await;
    let terminal = review_terminal(
        &fixture,
        &reviewer_fence,
        "review:request-changes-exhausted",
        TaskReviewVerdict::RequestChanges,
        None,
    );
    let first = fixture
        .service
        .record_work_run_terminal(
            terminal.clone(),
            ACTOR,
            None,
            "correlation:terminal:review-exhausted",
        )
        .await
        .expect("exhaust review rounds");
    let waiting = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load exhausted review")
        .expect("exhausted review exists");
    let gate = waiting.active_gate.expect("review recovery gate");
    fixture
        .service
        .execute(WorkCommand::RetryTask(RetryTask {
            meta: metadata("terminal:retry-review-exhausted"),
            precondition: TaskPrecondition {
                task_id: waiting.task.task_id,
                expected_revision: waiting.task.revision,
                expected_generation: waiting.task.generation,
            },
            gate_id: gate.gate_id,
            note: None,
        }))
        .await
        .expect("retry exhausted review");
    let replay = fixture
        .service
        .record_work_run_terminal(
            terminal,
            ACTOR,
            None,
            "correlation:terminal:review-exhausted-replay",
        )
        .await
        .expect("late exhausted review replay");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.gate_id, first.gate_id);
    assert_eq!(replay.run_id, first.run_id);
}
