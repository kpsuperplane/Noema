use super::*;

async fn cancel_and_reopen(fixture: &ExecutorFixture, key: &str) -> noema_tasks::TaskRecord {
    let current = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load current task")
        .expect("current task exists")
        .task;
    let cancelled = fixture
        .service
        .execute(WorkCommand::CancelTask(CancelTask {
            meta: metadata(&format!("{key}:cancel")),
            precondition: TaskPrecondition {
                task_id: current.task_id,
                expected_revision: current.revision,
                expected_generation: current.generation,
            },
            reason: Some("Advance the task generation for replay coverage.".to_string()),
        }))
        .await
        .expect("cancel task")
        .task
        .expect("cancelled task");
    fixture
        .service
        .execute(WorkCommand::ReopenTask(noema_tasks::ReopenTask {
            meta: metadata(&format!("{key}:reopen")),
            precondition: TaskPrecondition {
                task_id: cancelled.task_id,
                expected_revision: cancelled.revision,
                expected_generation: cancelled.generation,
            },
        }))
        .await
        .expect("reopen task")
        .task
        .expect("reopened task")
}

fn assert_original_result(
    first: &noema_tasks::WorkCommandResult,
    replay: &noema_tasks::WorkCommandResult,
) {
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.contract_id, first.contract_id);
    assert_eq!(replay.gate_id, first.gate_id);
    assert_eq!(replay.run_id, first.run_id);
}

#[tokio::test]
async fn executor_terminal_replays_after_cancel_and_reopen_generation_advance() {
    let fixture = fixture().await;
    let terminal = executor_submission(
        &fixture,
        "generation summary",
        "generation evidence",
        vec![],
    );
    let first = fixture
        .service
        .record_work_run_terminal(
            terminal.clone(),
            ACTOR,
            None,
            "correlation:generation:executor:first",
        )
        .await
        .expect("submit executor terminal");
    let reopened = cancel_and_reopen(&fixture, "generation:executor").await;
    assert!(reopened.generation > fixture.executor_fence.task_generation);

    let replay = fixture
        .service
        .record_work_run_terminal(
            terminal,
            ACTOR,
            None,
            "correlation:generation:executor:replay",
        )
        .await
        .expect("replay executor terminal after generation advance");
    assert_original_result(&first, &replay);

    let error = fixture
        .service
        .record_work_run_terminal(
            executor_submission(
                &fixture,
                "divergent generation summary",
                "generation evidence",
                vec![],
            ),
            ACTOR,
            None,
            "correlation:generation:executor:divergent",
        )
        .await
        .expect_err("divergent executor replay must conflict");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::IdempotencyConflict)
    ));
}

#[tokio::test]
async fn reviewer_terminal_replays_after_request_changes_generation_advance() {
    let fixture = fixture().await;
    let reviewer_fence = running_reviewer(&fixture, "worker:generation:reviewer").await;
    let terminal = review_terminal(
        &fixture,
        &reviewer_fence,
        "review:generation-advance",
        TaskReviewVerdict::Approve,
        None,
    );
    let first = fixture
        .service
        .record_work_run_terminal(
            terminal.clone(),
            ACTOR,
            None,
            "correlation:generation:reviewer:first",
        )
        .await
        .expect("approve reviewer terminal");
    let approved = fixture
        .store
        .get_work_task(&fixture.task.task_id)
        .await
        .expect("load approved task")
        .expect("approved task exists")
        .task;
    let revised = fixture
        .service
        .execute(WorkCommand::RequestTaskChanges(
            noema_tasks::RequestTaskChanges {
                meta: metadata("generation:reviewer:request-changes"),
                precondition: TaskPrecondition {
                    task_id: approved.task_id,
                    expected_revision: approved.revision,
                    expected_generation: approved.generation,
                },
                amendment: noema_tasks::TaskContractAmendment {
                    feedback_markdown: "Revise this approved result.".to_string(),
                    request_markdown: None,
                    replacement_criteria: None,
                    complexity: None,
                },
            },
        ))
        .await
        .expect("request changes")
        .task
        .expect("revised task");
    assert!(revised.generation > reviewer_fence.task_generation);

    let replay = fixture
        .service
        .record_work_run_terminal(
            terminal.clone(),
            ACTOR,
            None,
            "correlation:generation:reviewer:replay",
        )
        .await
        .expect("replay reviewer terminal after request changes");
    assert_original_result(&first, &replay);

    let mut divergent = terminal;
    let WorkRunTerminal::Review(command) = &mut divergent else {
        unreachable!()
    };
    command.review.overall_feedback = "Divergent review feedback.".to_string();
    let error = fixture
        .service
        .record_work_run_terminal(
            divergent,
            ACTOR,
            None,
            "correlation:generation:reviewer:divergent",
        )
        .await
        .expect_err("divergent reviewer replay must conflict");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::IdempotencyConflict)
    ));
}

#[tokio::test]
async fn blocked_terminal_replays_after_cancel_and_reopen_generation_advance() {
    let fixture = fixture().await;
    let terminal = WorkRunTerminal::Blocked(ReportTaskBlocked {
        fence: fixture.executor_fence.clone(),
        gate_kind: TaskGateKind::Clarification,
        prompt_markdown: "Which generation-safe choice should be used?".to_string(),
        context_markdown: "The original terminal remains immutable.".to_string(),
    });
    let first = fixture
        .service
        .record_work_run_terminal(
            terminal.clone(),
            ACTOR,
            None,
            "correlation:generation:blocked:first",
        )
        .await
        .expect("record blocked terminal");
    let reopened = cancel_and_reopen(&fixture, "generation:blocked").await;
    assert!(reopened.generation > fixture.executor_fence.task_generation);

    let replay = fixture
        .service
        .record_work_run_terminal(
            terminal.clone(),
            ACTOR,
            None,
            "correlation:generation:blocked:replay",
        )
        .await
        .expect("replay blocked terminal after generation advance");
    assert_original_result(&first, &replay);

    let mut divergent = terminal;
    let WorkRunTerminal::Blocked(report) = &mut divergent else {
        unreachable!()
    };
    report.prompt_markdown = "A divergent blocked prompt.".to_string();
    let error = fixture
        .service
        .record_work_run_terminal(
            divergent,
            ACTOR,
            None,
            "correlation:generation:blocked:divergent",
        )
        .await
        .expect_err("divergent blocked replay must conflict");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::IdempotencyConflict)
    ));
}

#[tokio::test]
async fn failure_terminal_replays_after_cancel_and_reopen_generation_advance() {
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
        .expect("exhaust automatic retries");
    let report = ReportRunFailure {
        fence: fixture.executor_fence.clone(),
        status: noema_tasks::RunStatus::Failed,
        error_code: noema_tasks::SafeErrorCode::new("generation_failure").expect("safe error code"),
        error_message: Some("The original safe failure.".to_string()),
        retryable: true,
    };
    let first = fixture
        .service
        .report_work_run_failure(
            report.clone(),
            ACTOR,
            None,
            "correlation:generation:failure:first",
        )
        .await
        .expect("report terminal failure");
    let reopened = cancel_and_reopen(&fixture, "generation:failure").await;
    assert!(reopened.generation > fixture.executor_fence.task_generation);

    let replay = fixture
        .service
        .report_work_run_failure(
            report.clone(),
            ACTOR,
            None,
            "correlation:generation:failure:replay",
        )
        .await
        .expect("replay failure after generation advance");
    assert_original_result(&first, &replay);

    let mut divergent = report;
    divergent.error_message = Some("A divergent safe failure.".to_string());
    let error = fixture
        .service
        .report_work_run_failure(
            divergent,
            ACTOR,
            None,
            "correlation:generation:failure:divergent",
        )
        .await
        .expect_err("divergent failure replay must conflict");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::IdempotencyConflict)
    ));
}

#[tokio::test]
async fn first_terminal_write_after_generation_advance_is_fenced() {
    let fixture = fixture().await;
    let reopened = cancel_and_reopen(&fixture, "generation:first-write").await;
    assert!(reopened.generation > fixture.executor_fence.task_generation);

    let error = fixture
        .service
        .record_work_run_terminal(
            executor_submission(&fixture, "stale summary", "stale evidence", vec![]),
            ACTOR,
            None,
            "correlation:generation:first-write",
        )
        .await
        .expect_err("first stale terminal must remain fenced");
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::RunFenced | WorkDomainError::StaleGeneration)
    ));

    let blocked_error = fixture
        .service
        .record_work_run_terminal(
            WorkRunTerminal::Blocked(ReportTaskBlocked {
                fence: fixture.executor_fence.clone(),
                gate_kind: TaskGateKind::Clarification,
                prompt_markdown: "A stale first blocked prompt.".to_string(),
                context_markdown: "This terminal was never committed.".to_string(),
            }),
            ACTOR,
            None,
            "correlation:generation:first-blocked-write",
        )
        .await
        .expect_err("first stale blocked terminal must remain fenced");
    assert!(matches!(
        blocked_error,
        StoreError::Work(WorkDomainError::RunFenced | WorkDomainError::StaleGeneration)
    ));
}
