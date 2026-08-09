//! Focused transactional tests for the semantic Work command writer.

use noema_capabilities::{
    CapabilityAuthenticationAuthorityKind, CapabilityAuthenticationChallenge,
    CapabilityAuthenticationChallengeKind,
};
use noema_conversations::{ConversationItemKind, ReplayMode};
use noema_tasks::{
    AgentRunItemKind, AgentRunItemStatus, AnswerTask, CancelTask, CaptureTask, CommandMeta,
    CreateProject, CriterionOutcome, DelegateExecutionIntent, DelegateTask, MissedRunPolicy,
    NewAgentRunItem, NewTaskRecurrence, NewTaskReview, NewTaskSchedule, NewTaskSubmission,
    NewTaskValidationCriterion, OverlapPolicy, QueueTask, ReopenTask, RetryTask,
    RunScheduledTaskNow, RunStatus, RunTaskRecurrenceNow, SafeErrorCode, ScheduleTask,
    SubmissionCriterionEvidence, TaskAuthorizationContext, TaskComplexity, TaskContractAmendment,
    TaskGateAnswer, TaskGateId, TaskGateKind, TaskMessageKind, TaskPrecondition, TaskProvenance,
    TaskRecoveryReason, TaskReviewCriterion, TaskReviewVerdict, TaskSourceKind, UnscheduleTask,
    UpdateInboxTask, UpdateTaskRecurrence, WorkCommand, WorkDomainError,
};
use noema_workspaces::WorkspaceId;

use crate::{
    CapabilityAuthenticationRequestState, CompleteWorkNotification, ExecutionReviewRoute,
    GovernedActionDecision, GovernedActionState, GovernedAssessmentStatus, GovernedAuthorization,
    GovernedExecutionOutcome, GovernedRisk, NewCapabilityAuthenticationRequest, NewGovernedAction,
    NewGovernedActionAssessment, NewRuntimeDebugSpan, NoemaStore, ReportRunFailure,
    ReportTaskBlocked, RuntimeDebugMetadata, RuntimeDebugScope, RuntimeDebugSpanCategory,
    RuntimeDebugSpanStatus, StoreError, SubmitTaskResult, SubmitTaskReview,
    WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN, WorkCommandService, WorkEventBeforeQuery,
    WorkEventQuery, WorkNotificationLeaseRequest, WorkPageSize, WorkRunFence, WorkRunTerminal,
    test_support::{
        initialize_codex_provider_selections, open_ephemeral_store, ready_hosted_provider_registry,
    },
};

#[tokio::test]
async fn final_run_status_finishes_active_items_and_debug_spans() {
    for (case, run_status, item_status, span_status) in [
        (
            "completed",
            RunStatus::Completed,
            AgentRunItemStatus::Completed,
            RuntimeDebugSpanStatus::Completed,
        ),
        (
            "failed",
            RunStatus::Failed,
            AgentRunItemStatus::Failed,
            RuntimeDebugSpanStatus::Failed,
        ),
        (
            "cancelled",
            RunStatus::Cancelled,
            AgentRunItemStatus::Cancelled,
            RuntimeDebugSpanStatus::Cancelled,
        ),
        (
            "interrupted",
            RunStatus::Interrupted,
            AgentRunItemStatus::Failed,
            RuntimeDebugSpanStatus::Interrupted,
        ),
    ] {
        let (store, service) = fixture().await;
        service
            .execute(direct_delegated(
                &format!("idem:final-records:{case}"),
                case,
            ))
            .await
            .expect("delegate task");
        let claimed = service
            .claim_next_work_run(&format!("worker:{case}"), 60, &[])
            .await
            .expect("claim")
            .expect("run");
        let fence = WorkRunFence {
            run_id: claimed.run.run_id.clone(),
            lease_token: claimed.lease_token,
            task_generation: claimed.run.task_generation,
            contract_id: claimed.run.contract_id,
        };
        service
            .start_work_run(&fence, ACTOR, None, &format!("correlation:{case}"))
            .await
            .expect("start");
        let call_item_id = format!("run_item:test_call:{case}");
        for item in [
            NewAgentRunItem {
                item_id: Some(format!("run_item:test_assistant:{case}")),
                run_id: fence.run_id.clone(),
                round_index: 0,
                kind: AgentRunItemKind::AssistantOutput,
                status: AgentRunItemStatus::Running,
                correlation_id: None,
                parent_item_id: None,
                content_text: Some("partial output".to_string()),
                payload: serde_json::json!({}),
            },
            NewAgentRunItem {
                item_id: Some(call_item_id.clone()),
                run_id: fence.run_id.clone(),
                round_index: 0,
                kind: AgentRunItemKind::ToolCall,
                status: AgentRunItemStatus::Running,
                correlation_id: Some(format!("call:{case}")),
                parent_item_id: None,
                content_text: Some("tool".to_string()),
                payload: serde_json::json!({}),
            },
            NewAgentRunItem {
                item_id: Some(format!("run_item:test_result:{case}")),
                run_id: fence.run_id.clone(),
                round_index: 0,
                kind: AgentRunItemKind::ToolResult,
                status: AgentRunItemStatus::Failed,
                correlation_id: Some(format!("call:{case}")),
                parent_item_id: Some(call_item_id.clone()),
                content_text: Some("tool failed".to_string()),
                payload: serde_json::json!({}),
            },
        ] {
            store
                .append_agent_run_item(item, &fence)
                .await
                .expect("run item");
        }
        store
            .begin_runtime_debug_span(NewRuntimeDebugSpan {
                scope: RuntimeDebugScope::AgentRun(fence.run_id.clone()),
                category: RuntimeDebugSpanCategory::Provider,
                name: "Provider call".to_string(),
                metadata: RuntimeDebugMetadata::default(),
            })
            .await
            .expect("debug span");

        store
            .with_immediate_transaction_retry(|tx| {
                tx.execute(
                    "UPDATE agent_runs SET status = ?2, ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL WHERE run_id = ?1",
                    rusqlite::params![fence.run_id, run_status.as_str()],
                )?;
                crate::run_items::finish_agent_run_records_tx(tx, &fence.run_id, run_status)?;
                crate::run_items::finish_agent_run_records_tx(tx, &fence.run_id, run_status)
            })
            .await
            .expect("finish records");

        let statuses = store
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT kind, status FROM agent_run_items WHERE run_id = ?1 ORDER BY sequence_index",
                )?;
                statement
                    .query_map([fence.run_id.as_str()], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await
            .expect("statuses");
        assert_eq!(statuses[0].1, item_status.as_str());
        assert_eq!(statuses[1].1, AgentRunItemStatus::Failed.as_str());
        assert_eq!(statuses[2].1, AgentRunItemStatus::Failed.as_str());
        let profile = store
            .runtime_debug_profile(RuntimeDebugScope::AgentRun(fence.run_id))
            .await
            .expect("profile")
            .expect("run profile");
        assert_eq!(profile.spans[0].status, span_status);
        assert!(profile.spans[0].ended_at.is_some());
    }
}

const ACTOR: &str = "actor:human:local";

macro_rules! task {
    ($service:expr, $command:expr, $context:literal) => {
        $service
            .execute($command)
            .await
            .expect($context)
            .task
            .expect("task")
    };
}

macro_rules! work_error {
    ($service:expr, $command:expr, $error:pat, $context:literal) => {
        assert!(
            matches!($service.execute($command).await, Err($error)),
            $context
        )
    };
}

async fn fixture() -> (NoemaStore, WorkCommandService) {
    let store = open_ephemeral_store().await.expect("open store");
    store
        .with_connection(|connection| {
            connection.execute_batch(
                r#"
                INSERT INTO conversations (
                  conversation_id, owner_object_type, owner_object_id, primary_human_id,
                  primary_agent_id, provider
                ) VALUES
                  ('conversation:capture-source', 'human', 'human:local', 'human:local', 'agent:primary', 'codex'),
                  ('conversation:delegate-source', 'human', 'human:local', 'human:local', 'agent:primary', 'codex');
                INSERT INTO conversation_turns (turn_id, conversation_id, status) VALUES
                  ('turn:capture-source', 'conversation:capture-source', 'completed'),
                  ('turn:delegate-source', 'conversation:delegate-source', 'completed');
                INSERT INTO conversation_items (
                  item_id, conversation_id, turn_id, sequence_index, kind, status,
                  author_actor_id, content_text
                ) VALUES
                  ('item:capture-source', 'conversation:capture-source', 'turn:capture-source', 1,
                   'user_text', 'completed', 'human:local', 'Capture the requested task.'),
                  ('item:delegate-source', 'conversation:delegate-source', 'turn:delegate-source', 1,
                   'user_text', 'completed', 'human:local', 'Delegate the requested task.');
                "#,
            )?;
            Ok(())
        })
        .await
        .expect("authorization source fixtures");
    initialize_codex_provider_selections(&store)
        .await
        .expect("initialize provider selections");
    let registry = ready_hosted_provider_registry(["provider_account:codex:default"])
        .expect("ready provider registry");
    let service = WorkCommandService::new(store.clone(), registry);
    (store, service)
}

fn metadata(key: &str) -> CommandMeta {
    CommandMeta {
        actor_id: ACTOR.to_string(),
        causation_id: None,
        correlation_id: format!("correlation:{key}"),
        idempotency_key: Some(key.to_string()),
    }
}

fn capture(key: &str, title: &str) -> WorkCommand {
    WorkCommand::CaptureTask(CaptureTask {
        meta: metadata(key),
        workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
        title: title.to_string(),
        description_markdown: "captured description".to_string(),
        project_id: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::WorkUi,
            created_by_actor_id: ACTOR.to_string(),
            ..TaskProvenance::default()
        },
        schedule: None,
        executor_agent_id: None,
        cwd_override: None,
    })
}

fn sourced_capture(key: &str, source: &str) -> WorkCommand {
    let WorkCommand::CaptureTask(mut command) = capture(key, "Sourced capture") else {
        unreachable!()
    };
    command.provenance.conversation_id = Some("conversation:capture-source".to_string());
    command.provenance.turn_id = Some("turn:capture-source".to_string());
    command.provenance.item_id = Some("item:capture-source".to_string());
    command.provenance.source_kind = TaskSourceKind::ChatCapture;
    command.provenance.source_tool_call_id = Some(format!("tool_call:{source}"));
    WorkCommand::CaptureTask(command)
}

fn precondition(task: &noema_tasks::TaskRecord) -> TaskPrecondition {
    TaskPrecondition {
        task_id: task.task_id.clone(),
        expected_revision: task.revision,
        expected_generation: task.generation,
    }
}

fn update(key: &str, task: &noema_tasks::TaskRecord, title: &str) -> WorkCommand {
    WorkCommand::UpdateInboxTask(UpdateInboxTask {
        meta: metadata(key),
        precondition: precondition(task),
        title: Some(title.to_string()),
        description_markdown: None,
        project_id: None,
        executor_agent_id: None,
        cwd_override: None,
    })
}

fn queue(key: &str, task: &noema_tasks::TaskRecord) -> WorkCommand {
    WorkCommand::QueueTask(QueueTask {
        meta: metadata(key),
        precondition: precondition(task),
    })
}

fn schedule(
    key: &str,
    task: &noema_tasks::TaskRecord,
    scheduled_for: i64,
    recurrence: Option<NewTaskRecurrence>,
) -> WorkCommand {
    WorkCommand::ScheduleTask(ScheduleTask {
        meta: metadata(key),
        precondition: precondition(task),
        schedule: NewTaskSchedule {
            scheduled_for,
            time_zone: "UTC".to_string(),
            missed_run_policy: MissedRunPolicy::RunOnce,
            recurrence,
        },
        requires_existing: false,
    })
}

fn delegated(key: &str, source: &str, complexity_hint: Option<TaskComplexity>) -> WorkCommand {
    WorkCommand::DelegateTask(DelegateTask {
        meta: CommandMeta {
            actor_id: ACTOR.to_string(),
            causation_id: None,
            correlation_id: format!("correlation:delegate:{source}"),
            idempotency_key: Some(key.to_string()),
        },
        workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
        title: format!("Delegated {source}"),
        description_markdown: "Durable delegated payload".to_string(),
        project_id: None,
        executor_agent_id: None,
        cwd_override: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::ChatDelegate,
            conversation_id: Some("conversation:delegate-source".to_string()),
            turn_id: Some("turn:delegate-source".to_string()),
            item_id: Some("item:delegate-source".to_string()),
            source_tool_call_id: Some(format!("tool_call:{source}")),
            created_by_actor_id: ACTOR.to_string(),
        },
        complexity_hint,
        execution_intent: None,
    })
}

fn direct_delegated(key: &str, source: &str) -> WorkCommand {
    let WorkCommand::DelegateTask(mut command) = delegated(key, source, None) else {
        unreachable!()
    };
    command.execution_intent = Some(DelegateExecutionIntent {
        request_markdown: "Perform the exact delegated work".to_string(),
        criteria: vec![NewTaskValidationCriterion {
            criterion_id: None,
            ordinal: 1,
            description: "Return a result".to_string(),
            expected_evidence: None,
        }],
        complexity: TaskComplexity::Simple,
        execution_plan_markdown: None,
    });
    WorkCommand::DelegateTask(command)
}

async fn event_count(store: &NoemaStore) -> i64 {
    store
        .with_connection(|connection| {
            connection
                .query_row("SELECT COUNT(*) FROM work_events", [], |row| row.get(0))
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("event count")
}

#[tokio::test]
async fn reconciliation_uses_only_the_current_resolved_gate() {
    let (store, service) = fixture().await;
    let task = task!(
        service,
        direct_delegated("idem:current-gate", "current-gate"),
        "direct task"
    );
    let task_id = task.task_id.to_string();
    let contract_id = task
        .current_contract_id
        .expect("current contract")
        .to_string();
    store
        .with_connection(move |connection| {
            connection.execute(
                "UPDATE tasks SET generation = 2, latest_run_id = NULL WHERE task_id = ?1",
                [&task_id],
            )?;
            connection.execute(
                "UPDATE task_execution_contracts SET task_generation = 2 WHERE contract_id = ?1",
                [&contract_id],
            )?;
            for (name, generation, contract, run_kind, resolved_at) in [
                ("old", 1, Some(contract_id.as_str()), "planner", "2026-01-01T00:00:01Z"),
                ("current", 2, Some(contract_id.as_str()), "executor", "2026-01-01T00:00:02Z"),
                ("replaced", 2, None, "reviewer", "2026-01-01T00:00:03Z"),
            ] {
                let gate_id = format!("gate:{name}");
                let message_id = format!("task_message:{name}");
                connection.execute(
                    "INSERT INTO task_gates (gate_id, task_id, task_generation, contract_id, gate_kind, gate_state, recovery_reason, retry_run_kind, prompt_markdown, opened_by_actor_id) VALUES (?1, ?2, ?3, ?4, 'recovery', 'open', 'configuration_unavailable', ?5, 'Retry current work', ?6)",
                    rusqlite::params![gate_id, task_id, generation, contract, run_kind, ACTOR],
                )?;
                connection.execute(
                    "INSERT INTO task_messages (message_id, task_id, task_generation, contract_id, gate_id, message_kind, body_markdown, author_actor_id) VALUES (?1, ?2, ?3, ?4, ?5, 'retry_note', 'Retry', ?6)",
                    rusqlite::params![message_id, task_id, generation, contract, gate_id, ACTOR],
                )?;
                connection.execute(
                    "UPDATE task_gates SET gate_state = 'resolved', resolved_by_actor_id = ?2, resolution_message_id = ?3, resolved_at = ?4 WHERE gate_id = ?1",
                    rusqlite::params![gate_id, ACTOR, message_id, resolved_at],
                )?;
            }
            Ok(())
        })
        .await
        .expect("resolved gate history");

    let envelope = store
        .load_work_reconciliation_snapshot(&task.task_id)
        .await
        .expect("load snapshot")
        .expect("task snapshot");
    let action = crate::plan_work_reconciliation(&envelope).expect("valid recovery step");
    assert_eq!(
        crate::action_run_kind(&action),
        Some(noema_tasks::RunKind::Executor)
    );
}

#[tokio::test]
async fn event_pagination_rejects_malformed_rows_in_both_directions() {
    let (store, service) = fixture().await;
    service
        .execute(capture("event-page-first", "First event"))
        .await
        .expect("first capture");
    let workspace_id = WorkspaceId::new("workspace:personal").expect("workspace id");
    let first = WorkPageSize::new(1).expect("page size");
    let after = WorkEventQuery {
        workspace_id: workspace_id.clone(),
        project_id: None,
        task_id: None,
        run_id: None,
        after: None,
        first,
    };
    let before = WorkEventBeforeQuery {
        workspace_id,
        project_id: None,
        task_id: None,
        run_id: None,
        before: None,
        first,
    };
    let oldest = store
        .list_work_events_after(after.clone())
        .await
        .expect("oldest event page");
    let newest = store
        .list_work_events_before(before.clone())
        .await
        .expect("newest event page");
    assert!(oldest.edges[0].node.event_sequence() < newest.edges[0].node.event_sequence());

    store
        .with_connection(|connection| {
            connection.execute("UPDATE work_events SET payload_json = '{}'", [])?;
            Ok(())
        })
        .await
        .expect("corrupt persisted payloads");
    assert!(store.list_work_events_after(after).await.is_err());
    assert!(store.list_work_events_before(before).await.is_err());
}

async fn count_without_id(store: &NoemaStore, sql: &str) -> i64 {
    store
        .with_connection(|connection| {
            connection
                .query_row(sql, [], |row| row.get(0))
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("count durable rows")
}

async fn assert_one_durable_row(store: &NoemaStore, sql: &str) {
    assert_eq!(count_without_id(store, sql).await, 1);
}

#[derive(Clone, Copy)]
enum SourceReplayKind {
    Capture,
    Delegate,
}

async fn assert_source_replay(kind: SourceReplayKind) {
    let (store, service) = fixture().await;
    let command = |key| match kind {
        SourceReplayKind::Capture => sourced_capture(key, "same-source"),
        SourceReplayKind::Delegate => delegated(key, "same-source", None),
    };
    let first = service
        .execute(command("idem:source:first"))
        .await
        .expect("create task");
    let replay = service
        .execute(command("idem:source:second"))
        .await
        .expect("source replay under another idempotency key");
    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.task, first.task);
    let TaskAuthorizationContext::ConversationExcerpt { messages } = &first
        .task
        .as_ref()
        .expect("created task")
        .authorization_context
    else {
        panic!("chat-created task must retain a conversation excerpt");
    };
    assert_eq!(
        messages[0].role,
        noema_tasks::TaskAuthorizationMessageRole::Human
    );
    if matches!(kind, SourceReplayKind::Delegate) {
        assert_eq!(replay.run_id, first.run_id);
    }

    let mut divergent = command("idem:source:third");
    match &mut divergent {
        WorkCommand::CaptureTask(command) => command.description_markdown = "divergent".to_string(),
        WorkCommand::DelegateTask(command) => command.title = "Changed durable payload".to_string(),
        _ => unreachable!(),
    }
    work_error!(
        service,
        divergent,
        StoreError::Work(WorkDomainError::IdempotencyConflict),
        "source replay with divergent payload must fail"
    );
    let source_kind = match kind {
        SourceReplayKind::Capture => "capture-source",
        SourceReplayKind::Delegate => "delegate-source",
    };
    assert_one_durable_row(&store, &format!("SELECT COUNT(*) FROM tasks WHERE source_conversation_id = 'conversation:{source_kind}' AND source_tool_call_id = 'tool_call:same-source'")).await;
}

#[tokio::test]
async fn task_notification_suppresses_near_term_same_task_references() {
    let (store, service) = fixture().await;
    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE humans SET primary_conversation_id = 'conversation:capture-source' WHERE human_id = 'human:local'",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("use capture source as primary conversation");

    let chat_task = task!(
        service,
        sourced_capture("idem:notification:chat", "notification-chat"),
        "capture chat task"
    );
    let work_ui_task = task!(
        service,
        capture("idem:notification:work-ui", "Work UI notification"),
        "capture Work UI task"
    );
    let notifications = store
        .claim_work_notifications(WorkNotificationLeaseRequest {
            worker_id: "worker:notification-turn-test".to_string(),
            lease_seconds: 30,
            limit: 2,
        })
        .await
        .expect("claim creation notifications");
    assert_eq!(notifications.len(), 2);
    for notification in notifications {
        store
            .complete_work_notification(CompleteWorkNotification {
                notification_id: notification.notification_id,
                lease_token: notification.lease_token,
                conversation_id: "conversation:capture-source".to_string(),
            })
            .await
            .expect("deliver creation notification")
            .expect("creation reference");
    }

    let references = store
        .list_conversation_items("conversation:capture-source", ReplayMode::Visible)
        .await
        .expect("load delivered references")
        .into_iter()
        .filter(|item| item.kind == ConversationItemKind::TaskReference)
        .collect::<Vec<_>>();
    assert_eq!(references.len(), 2);
    let reference_for = |task_id: &noema_tasks::TaskId| {
        references
            .iter()
            .find(|item| item.payload_json["task_id"] == task_id.as_str())
            .expect("task reference")
    };
    assert_eq!(
        reference_for(&chat_task.task_id).turn_id.as_deref(),
        Some("turn:capture-source")
    );
    assert!(reference_for(&work_ui_task.task_id).turn_id.is_none());

    let task_id = chat_task.task_id.to_string();
    store
        .with_connection(move |connection| {
            let event_sequence: i64 = connection.query_row(
                "SELECT event_sequence FROM work_notification_outbox
                 WHERE notification_kind = 'task_created'
                   AND json_extract(payload_json, '$.task_id') = ?1",
                [&task_id],
                |row| row.get(0),
            )?;
            connection.execute_batch(&format!(
                r#"
                INSERT INTO conversation_items
                  (item_id, conversation_id, sequence_index, kind, status, author_actor_id, content_text)
                VALUES ('item:waiting-gap-one', 'conversation:capture-source',
                  (SELECT MAX(sequence_index) + 1 FROM conversation_items WHERE conversation_id = 'conversation:capture-source'),
                  'user_text', 'completed', 'human:local', 'Intervening message.');
                INSERT INTO conversation_items
                  (item_id, conversation_id, sequence_index, kind, status, author_actor_id, content_text)
                VALUES ('item:waiting-gap-two', 'conversation:capture-source',
                  (SELECT MAX(sequence_index) + 1 FROM conversation_items WHERE conversation_id = 'conversation:capture-source'),
                  'assistant_text', 'completed', 'agent:primary', 'Another intervening message.');
                INSERT INTO conversation_items
                  (item_id, conversation_id, sequence_index, kind, status, author_actor_id, payload_json, metadata_json)
                VALUES ('item:recent-recovery-reference', 'conversation:capture-source',
                  (SELECT MAX(sequence_index) + 1 FROM conversation_items WHERE conversation_id = 'conversation:capture-source'),
                  'task_reference', 'completed', 'actor:store:notification', '{{"task_id":"{task_id}"}}',
                  '{{"notification_kind":"task_recovery"}}');
                INSERT INTO conversation_items
                  (item_id, conversation_id, sequence_index, kind, status, author_actor_id, content_text, metadata_json)
                VALUES ('item:near-waiting-message', 'conversation:capture-source',
                  (SELECT MAX(sequence_index) + 1 FROM conversation_items WHERE conversation_id = 'conversation:capture-source'),
                  'assistant_text', 'completed', 'agent:primary', 'Task waiting again.',
                  '{{"source":"work_notification","notification_id":"notification:near-waiting"}}');
                INSERT INTO work_notification_outbox
                  (notification_id, event_sequence, destination_kind, destination_id, notification_kind,
                   payload_json, status, lease_owner, lease_token, lease_expires_at, attempt_count)
                VALUES ('notification:near-waiting', {event_sequence}, 'human_primary_conversation',
                  'human:local', 'task_waiting', '{{"task_id":"{task_id}"}}', 'leased',
                  'worker:test', 'lease:near-waiting', '9999-12-31T23:59:59.999Z', 1);
                "#
            ))?;
            Ok(())
        })
        .await
        .expect("seed near-term waiting notification");
    assert!(
        store
            .complete_work_notification(CompleteWorkNotification {
                notification_id: "notification:near-waiting".to_string(),
                lease_token: "lease:near-waiting".to_string(),
                conversation_id: "conversation:capture-source".to_string(),
            })
            .await
            .expect("deliver near-term waiting notification")
            .is_none()
    );

    let task_id = work_ui_task.task_id.to_string();
    store
        .with_connection(move |connection| {
            let event_sequence: i64 = connection.query_row(
                "SELECT event_sequence FROM work_notification_outbox
                 WHERE notification_kind = 'task_created'
                   AND json_extract(payload_json, '$.task_id') = ?1",
                [&task_id],
                |row| row.get(0),
            )?;
            connection.execute_batch(&format!(
                r#"
                INSERT INTO conversation_items
                  (item_id, conversation_id, sequence_index, kind, status, author_actor_id, content_text, metadata_json)
                VALUES ('item:distant-completion-message', 'conversation:capture-source',
                  (SELECT MAX(sequence_index) + 1 FROM conversation_items WHERE conversation_id = 'conversation:capture-source'),
                  'assistant_text', 'completed', 'agent:primary', 'Another task completed.',
                  '{{"source":"work_notification","notification_id":"notification:distant-completion"}}');
                INSERT INTO work_notification_outbox
                  (notification_id, event_sequence, destination_kind, destination_id, notification_kind,
                   payload_json, status, lease_owner, lease_token, lease_expires_at, attempt_count)
                VALUES ('notification:distant-completion', {event_sequence}, 'human_primary_conversation',
                  'human:local', 'task_completed', '{{"task_id":"{task_id}"}}', 'leased',
                  'worker:test', 'lease:distant-completion', '9999-12-31T23:59:59.999Z', 1);
                "#
            ))?;
            Ok(())
        })
        .await
        .expect("seed distant completion");
    assert!(
        store
            .complete_work_notification(CompleteWorkNotification {
                notification_id: "notification:distant-completion".to_string(),
                lease_token: "lease:distant-completion".to_string(),
                conversation_id: "conversation:capture-source".to_string(),
            })
            .await
            .expect("deliver distant completion notification")
            .is_some()
    );
}

#[tokio::test]
async fn receipt_replay_is_exact_and_divergent_replay_is_rejected() {
    let (_store, service) = fixture().await;
    let command = capture("idem:capture", "first title");

    let first = service.execute(command.clone()).await.expect("capture");
    let returned_snapshot = first.task.clone().expect("captured task");
    assert_eq!(
        returned_snapshot.authorization_context,
        TaskAuthorizationContext::ManualTaskBody {
            title: "first title".to_string(),
            description_markdown: "captured description".to_string(),
        }
    );
    let replay = service.execute(command).await.expect("idempotent replay");

    assert_eq!(replay.event_id, first.event_id);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.task, Some(returned_snapshot.clone()));

    work_error!(
        service,
        capture("idem:capture", "different title"),
        StoreError::Work(WorkDomainError::IdempotencyConflict),
        "same receipt key with a changed request must fail"
    );

    // The value returned by the first command is an immutable snapshot owned
    // by the caller; later writes cannot mutate it through shared store state.
    let updated = task!(
        service,
        update("idem:update", &returned_snapshot, "edited title"),
        "inbox update"
    );
    assert_eq!(returned_snapshot.title, "first title");
    assert_eq!(updated.title, "edited title");
    assert_eq!(
        updated.authorization_context,
        TaskAuthorizationContext::ManualTaskBody {
            title: "edited title".to_string(),
            description_markdown: "captured description".to_string(),
        }
    );
}

#[tokio::test]
async fn agent_inbox_edit_preserves_existing_authorization_context() {
    let (_store, service) = fixture().await;
    let mut forged_manual = capture("idem:forged-manual", "Forged title");
    let WorkCommand::CaptureTask(command) = &mut forged_manual else {
        unreachable!()
    };
    command.provenance.created_by_actor_id = "actor:agent:primary".to_string();
    assert!(service.execute(forged_manual).await.is_err());

    let original = task!(
        service,
        capture("idem:agent-edit:capture", "Human title"),
        "manual capture"
    );
    let original_context = original.authorization_context.clone();
    let updated = task!(
        service,
        WorkCommand::UpdateInboxTask(UpdateInboxTask {
            meta: CommandMeta {
                actor_id: "actor:agent:primary".to_string(),
                causation_id: None,
                correlation_id: "correlation:agent-edit".to_string(),
                idempotency_key: Some("idem:agent-edit:update".to_string()),
            },
            precondition: precondition(&original),
            title: Some("Agent rewrite".to_string()),
            description_markdown: None,
            project_id: None,
            executor_agent_id: None,
            cwd_override: None,
        }),
        "agent edit"
    );

    assert_eq!(updated.title, "Agent rewrite");
    assert_eq!(updated.authorization_context, original_context);
}

#[tokio::test]
async fn committed_detail_replay_does_not_reread_later_task_state() {
    let (_store, service) = fixture().await;
    let command = capture("idem:committed-detail", "receipt title");
    let first = service
        .execute_committed(command.clone())
        .await
        .expect("capture committed detail");
    let first_detail = first.task_detail.clone().expect("task detail snapshot");
    assert_eq!(first_detail.stage.stage_id, first_detail.task.stage_id);
    assert!(first_detail.completed_submission.is_none());

    let updated = task!(
        service,
        update(
            "idem:committed-detail:update",
            &first_detail.task,
            "later title",
        ),
        "mutate task after original receipt"
    );
    assert_eq!(updated.title, "later title");

    let replay = service
        .execute_committed(command)
        .await
        .expect("replay original receipt");
    assert_eq!(replay.result.event_id, first.result.event_id);
    assert_eq!(replay.result.event_sequence, first.result.event_sequence);
    assert_eq!(replay.task_detail, Some(first_detail));
}

#[tokio::test]
async fn stale_revision_is_atomic_and_inbox_edits_stop_at_queue() {
    let (store, service) = fixture().await;
    let original = task!(
        service,
        capture("idem:stale:capture", "captured"),
        "capture"
    );

    let current = task!(
        service,
        update("idem:stale:update", &original, "edited"),
        "first update"
    );
    let before_events = event_count(&store).await;

    work_error!(
        service,
        update("idem:stale:second", &original, "must not write"),
        StoreError::Work(WorkDomainError::StaleRevision),
        "old revision must be fenced"
    );
    assert_eq!(event_count(&store).await, before_events);
    assert_eq!(
        service
            .store()
            .get_work_task(&current.task_id)
            .await
            .expect("read task")
            .expect("task exists")
            .task
            .title,
        "edited"
    );

    let queued = service
        .execute(queue("idem:queue", &current))
        .await
        .expect("queue task");
    let queued_task = queued.task.expect("queued task");
    assert_eq!(queued_task.stage_id.as_str(), "stage:personal:queue");
    assert!(queued.run_id.is_some(), "queue must create one planner run");

    work_error!(
        service,
        update("idem:after-queue", &queued_task, "must fail"),
        StoreError::Work(WorkDomainError::InvalidTransition),
        "capture fields are Inbox-only"
    );
}

#[tokio::test]
async fn stale_generation_is_rejected_even_when_revision_matches() {
    let (_store, service) = fixture().await;
    let captured = task!(
        service,
        capture("idem:generation:capture", "captured"),
        "capture"
    );

    let task_id = captured.task_id.clone();
    service
        .store()
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE tasks SET generation = generation + 1 WHERE task_id = ?1",
                    [task_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("advance generation in test fixture");

    work_error!(
        service,
        update("idem:generation:stale", &captured, "must fail"),
        StoreError::Work(WorkDomainError::StaleGeneration),
        "old generation must be fenced"
    );
}

#[tokio::test]
async fn inbox_project_update_distinguishes_omitted_replacement_and_explicit_clear() {
    let (_store, service) = fixture().await;
    let project = service
        .execute(WorkCommand::CreateProject(CreateProject {
            meta: metadata("idem:project:create"),
            workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
            name: "Project association".to_string(),
            description: String::new(),
            folder: None,
        }))
        .await
        .expect("create project")
        .project
        .expect("project");
    let captured = task!(
        service,
        WorkCommand::CaptureTask(CaptureTask {
            meta: metadata("idem:project:capture"),
            workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
            title: "Associated task".to_string(),
            description_markdown: String::new(),
            project_id: Some(project.project_id.clone()),
            provenance: TaskProvenance {
                source_kind: TaskSourceKind::WorkUi,
                created_by_actor_id: ACTOR.to_string(),
                ..TaskProvenance::default()
            },
            schedule: None,
            executor_agent_id: None,
            cwd_override: None,
        }),
        "capture associated task"
    );

    let preserved = task!(
        service,
        WorkCommand::UpdateInboxTask(UpdateInboxTask {
            meta: metadata("idem:project:preserve"),
            precondition: precondition(&captured),
            title: Some("Still associated".to_string()),
            description_markdown: None,
            project_id: None,
            executor_agent_id: None,
            cwd_override: None,
        }),
        "omitted project replacement"
    );
    assert_eq!(preserved.project_id.as_ref(), Some(&project.project_id));

    let cleared = task!(
        service,
        WorkCommand::UpdateInboxTask(UpdateInboxTask {
            meta: metadata("idem:project:clear"),
            precondition: precondition(&preserved),
            title: None,
            description_markdown: None,
            project_id: Some(None),
            executor_agent_id: None,
            cwd_override: None,
        }),
        "explicit project clear"
    );
    assert_eq!(cleared.project_id, None);
}

#[tokio::test]
async fn acp_executor_contract_freezes_launch_revision_and_exact_cwd_precedence() {
    let (store, service) = fixture().await;
    let agent = store
        .create_acp_agent("Fake ACP", "/bin/false", &["--safe".to_string()])
        .await
        .expect("create ACP agent");
    let project = service
        .execute(WorkCommand::CreateProject(CreateProject {
            meta: metadata("idem:acp:project"),
            workspace_id: WorkspaceId::new("workspace:personal").unwrap(),
            name: "ACP project".to_string(),
            description: String::new(),
            folder: Some("/project/worktree".to_string()),
        }))
        .await
        .unwrap()
        .project
        .unwrap();

    let mut project_command = direct_delegated("idem:acp:project-task", "acp-project-task");
    let WorkCommand::DelegateTask(command) = &mut project_command else {
        unreachable!()
    };
    command.project_id = Some(project.project_id.clone());
    command.executor_agent_id = Some(agent.agent_id.clone());
    let project_task = task!(service, project_command, "delegate project task");
    let project_contract = store
        .get_work_task(&project_task.task_id)
        .await
        .unwrap()
        .unwrap()
        .current_contract
        .unwrap();
    assert_eq!(
        project_contract.effective_cwd.as_deref(),
        Some("/project/worktree")
    );
    assert_eq!(project_contract.executor.agent_id, agent.agent_id);
    assert_eq!(
        project_contract
            .executor
            .acp
            .as_ref()
            .unwrap()
            .connection_revision,
        1
    );

    let mut override_command = direct_delegated("idem:acp:override-task", "acp-override-task");
    let WorkCommand::DelegateTask(command) = &mut override_command else {
        unreachable!()
    };
    command.project_id = Some(project.project_id);
    command.executor_agent_id = Some(agent.agent_id.clone());
    command.cwd_override = Some("/task/override".to_string());
    let override_task = task!(service, override_command, "delegate override task");
    let override_contract = store
        .get_work_task(&override_task.task_id)
        .await
        .unwrap()
        .unwrap()
        .current_contract
        .unwrap();
    assert_eq!(
        override_contract.effective_cwd.as_deref(),
        Some("/task/override")
    );

    let mut default_command = direct_delegated("idem:acp:default-task", "acp-default-task");
    let WorkCommand::DelegateTask(command) = &mut default_command else {
        unreachable!()
    };
    command.executor_agent_id = Some(agent.agent_id.clone());
    let default_task = task!(service, default_command, "delegate default task");
    let expected_default = store.default_task_cwd(default_task.task_id.as_str());
    let default_contract = store
        .get_work_task(&default_task.task_id)
        .await
        .unwrap()
        .unwrap()
        .current_contract
        .unwrap();
    assert_eq!(
        default_contract.effective_cwd.as_deref(),
        expected_default.to_str()
    );
    assert!(expected_default.is_dir());

    let updated = store
        .update_acp_agent(&agent.agent_id, 1, "Fake ACP", "/bin/true", &[], true)
        .await
        .unwrap();
    assert_eq!(updated.connection_revision, 2);
    assert_eq!(project_contract.executor.acp.unwrap().command, "/bin/false");
}

#[tokio::test]
async fn missing_or_disabled_acp_executors_are_rejected_before_capture() {
    let (store, service) = fixture().await;
    for (source, agent_id) in [
        ("missing", "agent:missing".to_string()),
        ("disabled", {
            let agent = store
                .create_acp_agent("Disabled", "/bin/false", &[])
                .await
                .unwrap();
            store
                .update_acp_agent(&agent.agent_id, 1, "Disabled", "/bin/false", &[], false)
                .await
                .unwrap();
            agent.agent_id
        }),
    ] {
        let mut command = direct_delegated(&format!("idem:acp:{source}"), source);
        let WorkCommand::DelegateTask(delegate) = &mut command else {
            unreachable!()
        };
        delegate.executor_agent_id = Some(agent_id);
        assert!(matches!(
            service.execute(command).await,
            Err(StoreError::Work(WorkDomainError::ConfigurationUnavailable))
        ));
    }
}

#[tokio::test]
async fn acp_permission_decisions_match_exactly_and_approvals_are_consumed_once() {
    let (store, service) = fixture().await;
    let task = task!(
        service,
        direct_delegated("idem:acp:permission", "acp-permission"),
        "delegate permission task"
    );
    let claimed = service
        .claim_next_work_run("worker:acp:permission", 60, &[])
        .await
        .unwrap()
        .unwrap();
    let fence = WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id,
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:acp:permission")
        .await
        .unwrap();
    let exact = serde_json::json!({
        "agent_id": "agent:acp:test",
        "task_generation": task.generation,
        "contract_id": fence.contract_id,
        "tool_call": {"toolCallId": "tool:exact", "rawInput": {"path": "/tmp/exact"}},
        "options": [{"optionId": "allow", "kind": "allow_once"}],
        "allow_once_option_id": "allow",
    });
    let action = store
        .create_governed_action(acp_permission_action(&task, &fence.run_id, exact.clone()))
        .await
        .unwrap();
    store
        .record_governed_action_assessment(
            &action.action_id,
            action.revision,
            acp_permission_assessment(),
            Some(&fence),
        )
        .await
        .unwrap();
    store
        .decide_governed_action(
            &action.action_id,
            action.revision,
            "human:local",
            GovernedActionDecision::Approve,
        )
        .await
        .unwrap();
    store
        .claim_governed_action_execution(&action.action_id, action.revision, None)
        .await
        .unwrap();
    store
        .finish_governed_action_execution(
            &action.action_id,
            action.revision,
            GovernedExecutionOutcome::Succeeded,
            Some(&serde_json::json!({"option_id": "allow"})),
            None,
        )
        .await
        .unwrap();
    assert!(
        store
            .consume_succeeded_acp_permission(&task.task_id.to_string(), &fence.run_id, &exact)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .consume_succeeded_acp_permission(&task.task_id.to_string(), &fence.run_id, &exact)
            .await
            .unwrap()
            .is_none()
    );
    let mut changed = exact.clone();
    changed["tool_call"]["rawInput"]["path"] = serde_json::json!("/tmp/changed");
    assert!(
        store
            .consume_succeeded_acp_permission(&task.task_id.to_string(), &fence.run_id, &changed)
            .await
            .unwrap()
            .is_none()
    );

    service
        .resume_after_governed_action(&action.action_id, action.revision, ACTOR)
        .await
        .unwrap()
        .expect("approval continuation run");
    let denied_claim = service
        .claim_next_work_run("worker:acp:denial", 60, &[])
        .await
        .unwrap()
        .unwrap();
    let denied_fence = WorkRunFence {
        run_id: denied_claim.run.run_id,
        lease_token: denied_claim.lease_token,
        task_generation: denied_claim.run.task_generation,
        contract_id: denied_claim.run.contract_id,
    };
    service
        .start_work_run(&denied_fence, ACTOR, None, "correlation:acp:denial")
        .await
        .unwrap();
    let denied = store
        .create_governed_action(acp_permission_action(
            &task,
            &denied_fence.run_id,
            changed.clone(),
        ))
        .await
        .unwrap();
    store
        .record_governed_action_assessment(
            &denied.action_id,
            denied.revision,
            acp_permission_assessment(),
            Some(&denied_fence),
        )
        .await
        .unwrap();
    store
        .decide_governed_action(
            &denied.action_id,
            denied.revision,
            "human:local",
            GovernedActionDecision::Decline,
        )
        .await
        .unwrap();
    assert!(
        store
            .has_declined_acp_permission(&task.task_id.to_string(), &changed)
            .await
            .unwrap()
    );
    assert!(
        !store
            .has_declined_acp_permission(&task.task_id.to_string(), &exact)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn inline_governed_action_cannot_resume_a_later_task_gate() {
    let (store, service) = fixture().await;
    let task = task!(
        service,
        direct_delegated("idem:inline-action-gate", "inline-action-gate"),
        "delegate task"
    );
    let claimed = service
        .claim_next_work_run("worker:inline-action-gate", 60, &[])
        .await
        .expect("claim executor")
        .expect("executor run");
    let fence = WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id,
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:inline-action-gate")
        .await
        .expect("start executor");
    let action = store
        .create_governed_action(NewGovernedAction {
            owner_human_id: "human:local".to_string(),
            conversation_id: None,
            turn_id: None,
            task_id: Some(task.task_id.to_string()),
            run_id: Some(fence.run_id.clone()),
            requesting_agent_id: "agent:task-executor".to_string(),
            capability_name: "web.browse.open".to_string(),
            operation_token: "web.browse.open".to_string(),
            review_route: ExecutionReviewRoute::LlmReview,
            behavior: crate::StoredToolBehavior {
                read_only: true,
                idempotent: true,
                destructive: false,
                open_world: true,
            },
            arguments: serde_json::json!({"url": "https://example.com"}),
            input_schema: serde_json::json!({"type": "object"}),
            authorization_context: serde_json::json!({"origin": "task"}),
            safe_summary: "open the requested page".to_string(),
        })
        .await
        .expect("create inline action");
    store
        .record_governed_action_assessment(
            &action.action_id,
            action.revision,
            NewGovernedActionAssessment {
                status: GovernedAssessmentStatus::Completed,
                reviewer_selection: Some(serde_json::json!({"model_profile": "reviewer"})),
                authorization: Some(GovernedAuthorization::Explicit),
                risk: Some(GovernedRisk::Low),
                reason_codes: vec!["action_matches_request".to_string()],
                explanation: "the requested read is authorized".to_string(),
            },
            Some(&fence),
        )
        .await
        .expect("auto-authorize action");
    store
        .claim_governed_action_execution(&action.action_id, action.revision, Some(&fence))
        .await
        .expect("claim inline action");
    store
        .finish_governed_action_execution(
            &action.action_id,
            action.revision,
            GovernedExecutionOutcome::Succeeded,
            Some(&serde_json::json!({"opened": true})),
            None,
        )
        .await
        .expect("finish inline action");
    service
        .record_work_run_terminal(
            WorkRunTerminal::Blocked(ReportTaskBlocked {
                fence: fence.clone(),
                gate_kind: TaskGateKind::Clarification,
                prompt_markdown: "What value should I use?".to_string(),
                context_markdown: String::new(),
                suggested_answers: Vec::new(),
            }),
            ACTOR,
            None,
            "correlation:inline-action-gate:blocked",
        )
        .await
        .expect("open task gate");

    assert!(
        store
            .list_interrupted_governed_action_resumptions()
            .await
            .expect("list interrupted action resumptions")
            .is_empty()
    );
    assert_eq!(
        service
            .resume_after_governed_action(&action.action_id, action.revision, ACTOR)
            .await
            .expect("ignore inline action"),
        None
    );
    assert_eq!(
        store
            .get_work_run_record(&fence.run_id)
            .await
            .expect("load originating run")
            .expect("originating run")
            .status,
        noema_tasks::RunStatus::WaitingForApproval
    );

    let run_id = fence.run_id.clone();
    store
        .with_connection(move |connection| {
            connection.execute(
                "UPDATE agent_runs SET status = 'queued' WHERE run_id = ?1",
                [run_id],
            )?;
            Ok(())
        })
        .await
        .expect("restore persisted invalid runnable state");
    service
        .apply_work_reconciliation_action(crate::ApplyReconciliation {
            task_id: task.task_id,
            actor_id: "actor:store:reconciliation".to_string(),
            correlation_id: "correlation:inline-action-gate:reconcile".to_string(),
            causation_id: None,
        })
        .await
        .expect("fence invalid runnable work");
    assert_eq!(
        store
            .get_work_run_record(&fence.run_id)
            .await
            .expect("load reconciled run")
            .expect("reconciled run")
            .status,
        noema_tasks::RunStatus::Cancelled
    );
}

#[tokio::test]
async fn uncertain_capability_authentication_opens_typed_recovery_gate() {
    let (store, service) = fixture().await;
    crate::test_support::insert_mcp_server(&store, "mcp:auth-uncertain")
        .await
        .expect("insert MCP server");
    let task = task!(
        service,
        direct_delegated("idem:auth-uncertain", "auth-uncertain"),
        "delegate task"
    );
    let claimed = service
        .claim_next_work_run("worker:auth-uncertain", 60, &[])
        .await
        .expect("claim executor")
        .expect("executor run");
    let requesting_agent_id = claimed.run.agent_id.clone();
    let fence = WorkRunFence {
        run_id: claimed.run.run_id,
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id,
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:auth-uncertain")
        .await
        .expect("start executor");
    let request = store
        .create_capability_authentication_request(
            NewCapabilityAuthenticationRequest {
                owner_human_id: "human:local".to_string(),
                conversation_id: None,
                turn_id: None,
                task_id: Some(task.task_id.to_string()),
                run_id: Some(fence.run_id.clone()),
                task_generation: Some(task.generation),
                requesting_agent_id,
                challenge: CapabilityAuthenticationChallenge::new(
                    CapabilityAuthenticationChallengeKind::Reauthenticate,
                    CapabilityAuthenticationAuthorityKind::McpServer,
                    "mcp:auth-uncertain",
                    "generation:1",
                )
                .expect("authentication challenge"),
                capability_name: "mcp.auth.read".to_string(),
                operation_token: "exact-token".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
                protected_arguments_ref: "a".repeat(32),
                arguments_sha256: "b".repeat(64),
                provider_selection_digest: "c".repeat(64),
                output_index: 0,
                call_id: Some("call:auth-uncertain".to_string()),
                provider_call_id: None,
                provider_name: Some("auth_read".to_string()),
                governed_action: None,
                result_context: serde_json::json!({"route": "synthetic"}),
            },
            Some(&fence),
        )
        .await
        .expect("create authentication request");
    store
        .begin_capability_authentication(
            &request.request_id,
            request.revision,
            "human:local",
            "attempt:auth-uncertain",
        )
        .await
        .expect("begin authentication");
    store
        .claim_capability_authentication_resumption(&request.request_id, request.revision)
        .await
        .expect("claim authentication resumption");
    store
        .finish_capability_authentication_request(
            &request.request_id,
            request.revision,
            CapabilityAuthenticationRequestState::Completed,
            Some(&serde_json::json!({
                "success": false,
                "payload": {"code": "outcome_uncertain"},
            })),
            Some("outcome_uncertain"),
        )
        .await
        .expect("finish uncertain authentication");

    assert_eq!(
        service
            .resume_after_capability_authentication(&request.request_id, request.revision, ACTOR)
            .await
            .expect("resume uncertain authentication"),
        None
    );
    let snapshot = store
        .load_work_reconciliation_snapshot(&task.task_id)
        .await
        .expect("load task snapshot")
        .expect("task snapshot");
    let gate = snapshot.active_gate.expect("typed recovery gate");
    assert_eq!(gate.kind, TaskGateKind::Recovery);
    assert_eq!(
        gate.recovery_reason,
        Some(TaskRecoveryReason::UnsafeEffectUncertain)
    );
    assert_eq!(gate.retry_run_kind, Some(noema_tasks::RunKind::Executor));
    assert_eq!(
        gate.originating_run_id.as_deref(),
        Some(fence.run_id.as_str())
    );
    assert_ne!(
        gate.prompt_markdown,
        "Reconciliation requires a recovery decision."
    );
    assert_eq!(
        store
            .get_work_run_record(&fence.run_id)
            .await
            .expect("load executor")
            .expect("executor run")
            .status,
        noema_tasks::RunStatus::Completed
    );
    assert_eq!(
        count_without_id(
            &store,
            "SELECT COUNT(*) FROM agent_runs WHERE parent_run_id IS NOT NULL",
        )
        .await,
        0
    );
}

fn acp_permission_action(
    task: &noema_tasks::TaskRecord,
    run_id: &str,
    arguments: serde_json::Value,
) -> NewGovernedAction {
    NewGovernedAction {
        owner_human_id: "human:local".to_string(),
        conversation_id: None,
        turn_id: None,
        task_id: Some(task.task_id.to_string()),
        run_id: Some(run_id.to_string()),
        requesting_agent_id: "agent:acp:test".to_string(),
        capability_name: "acp.permission".to_string(),
        operation_token: "acp.permission".to_string(),
        review_route: ExecutionReviewRoute::HumanReview,
        behavior: crate::StoredToolBehavior {
            read_only: false,
            idempotent: false,
            destructive: false,
            open_world: true,
        },
        arguments,
        input_schema: serde_json::json!({"type": "object"}),
        authorization_context: serde_json::json!({"origin": "acp"}),
        safe_summary: "exact ACP request".to_string(),
    }
}

fn acp_permission_assessment() -> NewGovernedActionAssessment {
    NewGovernedActionAssessment {
        status: GovernedAssessmentStatus::ReviewerUnavailable,
        reviewer_selection: None,
        authorization: None,
        risk: None,
        reason_codes: vec!["acp_permission_requires_approval".to_string()],
        explanation: "one-time human approval required".to_string(),
    }
}

#[tokio::test]
async fn delegate_source_replay_is_exact_across_idempotency_namespaces() {
    assert_source_replay(SourceReplayKind::Delegate).await;
}

#[tokio::test]
async fn capture_source_replay_is_exact_across_idempotency_namespaces() {
    assert_source_replay(SourceReplayKind::Capture).await;
}

#[tokio::test]
async fn capture_source_replay_is_durable_without_an_idempotency_key() {
    let (store, service) = fixture().await;
    let mut original = sourced_capture("unused:first", "no-key-source");
    let WorkCommand::CaptureTask(command) = &mut original else {
        unreachable!()
    };
    command.meta.idempotency_key = None;
    let first = service
        .execute_committed(original.clone())
        .await
        .expect("source-owned capture without an idempotency key");

    let replay = service
        .execute_committed(original.clone())
        .await
        .expect("exact source replay without an idempotency key");
    assert_eq!(replay, first);
    assert_one_durable_row(&store, "SELECT COUNT(*) FROM work_command_receipts WHERE command_name = 'task.capture' AND idempotency_key LIKE 'source-replay:%'").await;

    let WorkCommand::CaptureTask(command) = &mut original else {
        unreachable!()
    };
    command.title = "Divergent source payload".to_string();
    work_error!(
        service,
        original,
        StoreError::Work(WorkDomainError::IdempotencyConflict),
        "source replay with a changed payload must fail"
    );
}

#[tokio::test]
async fn delegate_planner_complexity_hint_selects_the_matching_pool_tier() {
    let (store, service) = fixture().await;
    store
        .with_connection(|connection| {
            connection.execute_batch(
                "UPDATE task_model_pool_entries SET selection_mode = 'explicit_profile', model_profile = CASE complexity WHEN 'difficult' THEN 'gpt-5.6-terra' ELSE 'gpt-5.6-luna' END, reasoning_effort = CASE complexity WHEN 'simple' THEN 'low' WHEN 'medium' THEN 'medium' WHEN 'difficult' THEN 'high' END;",
            ).map_err(StoreError::Sqlite)?;
            Ok(())
        })
        .await
        .expect("differentiate task pool tiers");

    for (source, hint, expected_effort) in [
        ("simple", Some(TaskComplexity::Simple), "low"),
        ("difficult", Some(TaskComplexity::Difficult), "high"),
        ("fallback", None, "medium"),
    ] {
        let result = service
            .execute(delegated(&format!("idem:delegate:{source}"), source, hint))
            .await
            .expect("delegate planner tier");
        let run_id = result.run_id.expect("planner run id");
        let (kind, effort, source_kind): (String, Option<String>, Option<String>) = store
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT run_kind, reasoning_effort, selection_source FROM agent_runs WHERE run_id = ?1",
                        [run_id.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(StoreError::Sqlite)
            })
            .await
            .expect("load planner snapshot");
        assert_eq!(kind, "planner");
        assert_eq!(effort.as_deref(), Some(expected_effort));
        assert_eq!(source_kind.as_deref(), Some("task_model_pool_setting"));
    }
}

#[tokio::test]
async fn governed_action_approval_releases_and_resumes_a_task_run_once() {
    let (store, service) = fixture().await;
    let captured = task!(
        service,
        capture("idem:governed:capture", "Governed task"),
        "capture task"
    );
    task!(
        service,
        queue("idem:governed:queue", &captured),
        "queue task"
    );
    let claimed = service
        .claim_next_work_run("worker:governed", 60, &[])
        .await
        .expect("claim run")
        .expect("queued run");
    let fence = WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id.clone(),
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:governed")
        .await
        .expect("start run");
    let failed_run_id = fence.run_id.clone();
    store
        .with_connection(move |connection| {
            connection.execute(
                "UPDATE agent_runs SET max_automatic_retries = 0 WHERE run_id = ?1",
                [failed_run_id],
            )?;
            Ok(())
        })
        .await
        .expect("exhaust automatic retries");
    let failed = service
        .report_work_run_failure(
            ReportRunFailure {
                fence,
                status: noema_tasks::RunStatus::Failed,
                error_code: SafeErrorCode::new("work_runtime_failed").expect("safe error code"),
                error_message: Some("retryable test failure".to_string()),
                retryable: true,
            },
            ACTOR,
            None,
            "correlation:governed:failure",
        )
        .await
        .expect("open recovery gate");
    let failed_task = failed.task.expect("failed task");
    let gate_id = failed.gate_id.expect("recovery gate");
    service
        .execute(WorkCommand::RetryTask(RetryTask {
            meta: metadata("idem:governed:retry"),
            precondition: precondition(&failed_task),
            gate_id,
            note: None,
        }))
        .await
        .expect("retry failed run");
    let claimed = service
        .claim_next_work_run("worker:governed:retry", 60, &[])
        .await
        .expect("claim retry")
        .expect("retry run");
    let fence = WorkRunFence {
        run_id: claimed.run.run_id,
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id,
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:governed:retry")
        .await
        .expect("start retry");
    service
        .admit_work_run_execution_context(&fence, ACTOR, None, "correlation:governed")
        .await
        .expect("admit parent checkpoint");
    let parent_item_limit =
        i64::try_from(WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN).expect("lineage limit fits i64");
    let parent_run_id = fence.run_id.clone();
    store
        .with_connection(move |connection| {
            connection.execute(
                "WITH RECURSIVE item(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM item WHERE n < ?2)
                 INSERT INTO agent_run_items (item_id, run_id, sequence_index, round_index, kind, status, content_text)
                 SELECT 'run_item:governed:' || n, ?1, n + 1, 0, 'progress_notice', 'completed', replace(printf('%6000c', 'x'), ' ', 'x') || ' parent item ' || n FROM item",
                rusqlite::params![parent_run_id, parent_item_limit],
            )?;
            Ok(())
        })
        .await
        .expect("append oversized parent transcript");
    let action = store
        .create_governed_action(NewGovernedAction {
            owner_human_id: "human:local".to_string(),
            conversation_id: None,
            turn_id: None,
            task_id: Some(captured.task_id.to_string()),
            run_id: Some(fence.run_id.clone()),
            requesting_agent_id: "agent:task-executor".to_string(),
            capability_name: "mcp.example.write".to_string(),
            operation_token: "exact-token".to_string(),
            review_route: ExecutionReviewRoute::LlmReview,
            behavior: crate::StoredToolBehavior {
                read_only: false,
                idempotent: false,
                destructive: false,
                open_world: true,
            },
            arguments: serde_json::json!({"record_id": "42"}),
            input_schema: serde_json::json!({"type": "object"}),
            authorization_context: serde_json::json!({"origin": "task"}),
            safe_summary: "write an external record".to_string(),
        })
        .await
        .expect("create action");
    let waiting = store
        .record_governed_action_assessment(
            &action.action_id,
            action.revision,
            NewGovernedActionAssessment {
                status: GovernedAssessmentStatus::ReviewerUnavailable,
                reviewer_selection: None,
                authorization: None,
                risk: None,
                reason_codes: vec!["authorization_ambiguous".to_string()],
                explanation: "human approval required".to_string(),
            },
            Some(&fence),
        )
        .await
        .expect("request approval");
    assert_eq!(waiting.state, GovernedActionState::AwaitingApproval);
    assert_eq!(
        store
            .get_work_run_record(&fence.run_id)
            .await
            .expect("load waiting run")
            .expect("run")
            .status,
        noema_tasks::RunStatus::WaitingForApproval
    );
    let snapshot = store
        .load_work_reconciliation_snapshot(&captured.task_id)
        .await
        .expect("load waiting reconciliation snapshot")
        .expect("task snapshot");
    assert_eq!(
        crate::plan_work_reconciliation(&snapshot).expect("plan waiting reconciliation"),
        noema_tasks::WorkReconciliationAction::Idle
    );

    let declined = store
        .decide_governed_action(
            &action.action_id,
            action.revision,
            "human:local",
            GovernedActionDecision::Decline,
        )
        .await
        .expect("decline action");
    assert_eq!(declined.state, GovernedActionState::Declined);
    let child = service
        .resume_after_governed_action(&action.action_id, action.revision, ACTOR)
        .await
        .expect("resume task")
        .expect("child run");
    assert_eq!(
        service
            .resume_after_governed_action(&action.action_id, action.revision, ACTOR)
            .await
            .expect("idempotent resume"),
        Some(child.clone())
    );
    let claimed_child = service
        .claim_next_work_run("worker:governed:child", 60, &[])
        .await
        .expect("claim child")
        .expect("child run");
    assert_eq!(claimed_child.run.run_id, child);
    let child_fence = WorkRunFence {
        run_id: claimed_child.run.run_id,
        lease_token: claimed_child.lease_token,
        task_generation: claimed_child.run.task_generation,
        contract_id: claimed_child.run.contract_id,
    };
    service
        .start_work_run(&child_fence, ACTOR, None, "correlation:governed:child")
        .await
        .expect("start child");
    let admitted_child = service
        .admit_work_run_execution_context(&child_fence, ACTOR, None, "correlation:governed:child")
        .await
        .expect("admit child from bounded parent transcript");
    assert!(admitted_child.context.lineage.len() < WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN);
    assert!(admitted_child.context.lineage.iter().any(|item| {
        item.content_text
            .as_deref()
            .is_some_and(|content| content.ends_with("parent item 24"))
    }));
    assert_eq!(
        store
            .get_work_run_record(&fence.run_id)
            .await
            .expect("load parent")
            .expect("parent run")
            .status,
        noema_tasks::RunStatus::Completed
    );
}

fn delegated_review_case(key: &str, complexity: TaskComplexity) -> WorkCommand {
    let WorkCommand::DelegateTask(mut command) = delegated(key, "review-case", None) else {
        unreachable!()
    };
    command.execution_intent = Some(DelegateExecutionIntent {
        request_markdown: "Complete the review fixture.".to_string(),
        criteria: vec![NewTaskValidationCriterion {
            criterion_id: Some("criterion:review-case".to_string()),
            ordinal: 1,
            description: "The fixture has a complete result.".to_string(),
            expected_evidence: None,
        }],
        complexity,
        execution_plan_markdown: Some("Use the fixture result.".to_string()),
    });
    WorkCommand::DelegateTask(command)
}

async fn run_review_case(
    complexity: TaskComplexity,
    verdict: TaskReviewVerdict,
) -> (
    NoemaStore,
    WorkCommandService,
    noema_tasks::TaskRecord,
    serde_json::Value,
    Option<TaskGateId>,
) {
    let (store, service) = fixture().await;
    let created = service
        .execute(delegated_review_case(
            &format!("idem:review-case:{}", complexity.as_str()),
            complexity,
        ))
        .await
        .expect("delegate review case");
    let task = created.task.expect("delegated task");
    let claimed_executor = service
        .claim_next_work_run("worker:review-case:executor", 60, &[])
        .await
        .expect("claim executor")
        .expect("executor run");
    let executor_run_id = claimed_executor.run.run_id.clone();
    let executor_fence = WorkRunFence {
        run_id: executor_run_id.clone(),
        lease_token: claimed_executor.lease_token,
        task_generation: claimed_executor.run.task_generation,
        contract_id: claimed_executor.run.contract_id.clone(),
    };
    let contract_id = executor_fence.contract_id.clone().expect("contract");
    service
        .start_work_run(
            &executor_fence,
            ACTOR,
            None,
            "correlation:review-case:executor",
        )
        .await
        .expect("start executor");
    service
        .record_work_run_terminal(
            WorkRunTerminal::TaskResult(SubmitTaskResult {
                fence: executor_fence,
                submission: NewTaskSubmission {
                    submission_id: Some("submission:review-case".to_string()),
                    task_id: task.task_id.clone(),
                    contract_id: contract_id.clone(),
                    executor_run_id: executor_run_id.clone(),
                    review_round: 1,
                    summary: "Fixture result".to_string(),
                    result_markdown: "The fixture completed.".to_string(),
                    criteria: vec![SubmissionCriterionEvidence {
                        criterion_id: "criterion:review-case".to_string(),
                        evidence_markdown: "The fixture result is present.".to_string(),
                    }],
                    artifact_ids: Vec::new(),
                },
            }),
            ACTOR,
            None,
            "correlation:review-case:submission",
        )
        .await
        .expect("submit executor result");
    let executor_run_for_items = executor_run_id.clone();
    store
        .with_connection(move |connection| {
            connection.execute(
                r#"INSERT INTO agent_run_items (item_id, run_id, sequence_index, round_index, kind, status, content_text, payload_json)
                 VALUES ('run_item:review-case:executor-text', ?1, (SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM agent_run_items WHERE run_id = ?1), 0, 'assistant_output', 'completed', 'Authorized account label: Personal', '{"source_id":"source:ordinary:1"}')"#,
                [executor_run_for_items.as_str()],
            )?;
            connection.execute(
                r#"INSERT INTO agent_run_items (item_id, run_id, sequence_index, round_index, kind, status, correlation_id, content_text, payload_json)
                 VALUES ('run_item:review-case:executor-result', ?1, (SELECT COALESCE(MAX(sequence_index), 0) + 1 FROM agent_run_items WHERE run_id = ?1), 0, 'tool_result', 'failed', 'call:uncertain:1', 'The external outcome is uncertain.', '{"outcome":"uncertain","receipt_id":"receipt:ordinary:1"}')"#,
                [executor_run_for_items.as_str()],
            )?;
            Ok(())
        })
        .await
        .expect("append executor transcript");
    let claimed_reviewer = service
        .claim_next_work_run("worker:review-case:reviewer", 60, &[])
        .await
        .expect("claim reviewer")
        .expect("reviewer run");
    let reviewer_fence = WorkRunFence {
        run_id: claimed_reviewer.run.run_id.clone(),
        lease_token: claimed_reviewer.lease_token,
        task_generation: claimed_reviewer.run.task_generation,
        contract_id: claimed_reviewer.run.contract_id.clone(),
    };
    service
        .start_work_run(
            &reviewer_fence,
            ACTOR,
            None,
            "correlation:review-case:reviewer",
        )
        .await
        .expect("start reviewer");
    let reviewer_context = service
        .admit_work_run_execution_context(
            &reviewer_fence,
            ACTOR,
            None,
            "correlation:review-case:reviewer-context",
        )
        .await
        .expect("admit reviewer context");
    assert_eq!(reviewer_context.context.lineage.len(), 2);
    assert!(
        reviewer_context
            .context
            .lineage
            .iter()
            .all(|item| item.run_id == executor_run_id)
    );
    assert_eq!(
        reviewer_context.context.lineage[0].content_text.as_deref(),
        Some("Authorized account label: Personal")
    );
    assert_eq!(
        reviewer_context.context.lineage[1].status,
        AgentRunItemStatus::Failed
    );
    assert_eq!(
        reviewer_context.context.lineage[1].payload["outcome"],
        "uncertain"
    );
    assert!(reviewer_context.context.latest_submission.is_some());
    let result = service
        .record_work_run_terminal(
            WorkRunTerminal::Review(SubmitTaskReview {
                fence: reviewer_fence,
                review: NewTaskReview {
                    review_id: Some("review:review-case".to_string()),
                    task_id: task.task_id.clone(),
                    contract_id,
                    reviewer_run_id: claimed_reviewer.run.run_id,
                    reviewed_submission_id: "submission:review-case".to_string(),
                    review_attempt_index: 1,
                    supersedes_review_id: None,
                    overall_verdict: verdict,
                    human_gate_kind: (verdict == TaskReviewVerdict::NeedsHuman)
                        .then_some(TaskGateKind::Clarification),
                    overall_feedback: "All fixture evidence passes.".to_string(),
                    criteria: vec![TaskReviewCriterion {
                        criterion_id: "criterion:review-case".to_string(),
                        outcome: match verdict {
                            TaskReviewVerdict::Approve => CriterionOutcome::Pass,
                            TaskReviewVerdict::RequestChanges => CriterionOutcome::Fail,
                            TaskReviewVerdict::NeedsHuman => CriterionOutcome::Uncertain,
                        },
                        evidence_markdown: Some("The submitted evidence is complete.".to_string()),
                        feedback: (verdict == TaskReviewVerdict::RequestChanges)
                            .then(|| "The result must include the required value.".to_string()),
                    }],
                },
            }),
            ACTOR,
            None,
            "correlation:review-case:review",
        )
        .await
        .expect("submit reviewer result");
    let gate_id = result.gate_id.clone();
    let payload = if verdict == TaskReviewVerdict::RequestChanges {
        serde_json::json!({})
    } else {
        let notification_kind = if verdict == TaskReviewVerdict::Approve {
            "task_completed"
        } else {
            "task_waiting"
        };
        let payload_json: String = store
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT payload_json FROM work_notification_outbox WHERE notification_kind = ?1 ORDER BY notification_id DESC LIMIT 1",
                        [notification_kind],
                        |row| row.get(0),
                    )
                    .map_err(StoreError::Sqlite)
            })
            .await
            .expect("review notification");
        serde_json::from_str(&payload_json).expect("notification payload")
    };
    (
        store,
        service,
        result.task.expect("review task"),
        payload,
        gate_id,
    )
}

#[tokio::test]
async fn approved_reviews_complete_tasks_at_every_complexity() {
    for complexity in [TaskComplexity::Simple, TaskComplexity::Medium] {
        let (_, _, task, notification, _) =
            run_review_case(complexity, TaskReviewVerdict::Approve).await;
        assert_eq!(task.stage_id.as_str(), noema_tasks::PERSONAL_DONE_STAGE_ID);
        assert_eq!(
            task.completed_submission_id.as_deref(),
            Some("submission:review-case")
        );
        assert_eq!(notification["action_needed"], false);
    }
}

#[tokio::test]
async fn executor_context_uses_only_the_review_saved_on_a_correction_run() {
    let (first_store, first_service) = fixture().await;
    let first_task = first_service
        .execute(direct_delegated("idem:first-review-input", "first"))
        .await
        .expect("delegate first task")
        .task
        .expect("first task");
    let first_claim = first_service
        .claim_next_work_run("worker:first-review-input", 60, &[])
        .await
        .expect("claim first run")
        .expect("first run");
    let first_fence = WorkRunFence {
        run_id: first_claim.run.run_id.clone(),
        lease_token: first_claim.lease_token,
        task_generation: first_claim.run.task_generation,
        contract_id: first_claim.run.contract_id,
    };
    first_service
        .start_work_run(&first_fence, ACTOR, None, "correlation:first-review-input")
        .await
        .expect("start first run");
    first_store
        .with_connection(move |connection| {
            connection.execute(
                "UPDATE tasks SET latest_review_id = 'review:not-for-first-run' WHERE task_id = ?1",
                [first_task.task_id.as_str()],
            )?;
            Ok(())
        })
        .await
        .expect("change latest review");
    let first_context = first_store
        .get_work_run_execution_context(&first_fence.run_id)
        .await
        .expect("first context")
        .expect("first context row");
    assert!(first_context.latest_review.is_none());

    let (store, service, task, _, _) =
        run_review_case(TaskComplexity::Medium, TaskReviewVerdict::RequestChanges).await;
    let correction = service
        .claim_next_work_run("worker:correction-review-input", 60, &[])
        .await
        .expect("claim correction")
        .expect("correction run");
    assert_eq!(correction.run.review_round, 2);
    assert_eq!(
        correction.run.triggering_review_id.as_deref(),
        Some("review:review-case")
    );
    let correction_fence = WorkRunFence {
        run_id: correction.run.run_id.clone(),
        lease_token: correction.lease_token,
        task_generation: correction.run.task_generation,
        contract_id: correction.run.contract_id,
    };
    service
        .start_work_run(
            &correction_fence,
            ACTOR,
            None,
            "correlation:correction-review-input",
        )
        .await
        .expect("start correction");
    let correction_run_id = correction_fence.run_id.clone();
    let task_id = task.task_id.clone();
    store
        .with_connection(move |connection| {
            connection.execute(
                "UPDATE tasks SET latest_review_id = 'review:not-the-trigger' WHERE task_id = ?1",
                [task_id.as_str()],
            )?;
            Ok(())
        })
        .await
        .expect("replace latest review pointer");
    let exact = store
        .get_work_run_execution_context(&correction_run_id)
        .await
        .expect("correction context")
        .expect("correction context row");
    assert_eq!(
        exact
            .latest_review
            .as_ref()
            .map(|review| review.review_id.as_str()),
        Some("review:review-case")
    );

    store
        .with_connection({
            let correction_run_id = correction_run_id.clone();
            move |connection| {
                connection.execute(
                    "UPDATE agent_runs SET triggering_review_id = NULL WHERE run_id = ?1",
                    [correction_run_id],
                )?;
                Ok(())
            }
        })
        .await
        .expect("remove trigger");
    let missing = store
        .get_work_run_execution_context(&correction_run_id)
        .await
        .expect_err("missing correction review must fail");
    assert!(matches!(
        missing,
        StoreError::InvariantViolation { message }
            if message.contains("has no triggering review")
    ));

    let other_task = service
        .execute(capture("idem:unrelated-review-task", "Other task"))
        .await
        .expect("capture other task")
        .task
        .expect("other task");
    store
        .with_connection({
            let correction_run_id = correction_run_id.clone();
            move |connection| {
                connection.execute(
                    "UPDATE agent_runs SET triggering_review_id = 'review:review-case' WHERE run_id = ?1",
                    [correction_run_id],
                )?;
                connection.execute(
                    "UPDATE task_reviews SET task_id = ?1 WHERE review_id = 'review:review-case'",
                    [other_task.task_id.as_str()],
                )?;
                Ok(())
            }
        })
        .await
        .expect("make review unrelated");
    let unrelated = store
        .get_work_run_execution_context(&correction_run_id)
        .await
        .expect_err("unrelated correction review must fail");
    assert!(matches!(
        unrelated,
        StoreError::InvariantViolation { message }
            if message.contains("foreign review")
    ));
}

#[tokio::test]
async fn reviewer_answer_carries_prior_review_into_continuation_context() {
    let (_, service, waiting, _, gate_id) =
        run_review_case(TaskComplexity::Medium, TaskReviewVerdict::NeedsHuman).await;
    service
        .execute(WorkCommand::AnswerTask(AnswerTask {
            meta: metadata("idem:review-case:answer"),
            precondition: precondition(&waiting),
            gate_id: gate_id.expect("reviewer gate"),
            answer: TaskGateAnswer {
                message_markdown: "The secondary evidence is acceptable.".to_string(),
                approval_decision: None,
            },
        }))
        .await
        .expect("answer reviewer gate");
    let claimed = service
        .claim_next_work_run("worker:review-case:continuation", 60, &[])
        .await
        .expect("claim reviewer continuation")
        .expect("reviewer continuation");
    assert!(claimed.run.triggering_review_id.is_none());
    let fence = WorkRunFence {
        run_id: claimed.run.run_id,
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id,
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:review-case:continuation")
        .await
        .expect("start reviewer continuation");
    let admitted = service
        .admit_work_run_execution_context(
            &fence,
            ACTOR,
            None,
            "correlation:review-case:continuation-context",
        )
        .await
        .expect("admit reviewer continuation");
    assert_eq!(
        admitted
            .context
            .latest_review
            .as_ref()
            .map(|review| review.review_id.as_str()),
        Some("review:review-case")
    );
}

#[tokio::test]
async fn reopen_requires_direction_and_queues_a_fresh_contract_generation() {
    let (store, service, completed, _, _) =
        run_review_case(TaskComplexity::Medium, TaskReviewVerdict::Approve).await;
    let command = |feedback_markdown: &str, key: &str| {
        WorkCommand::ReopenTask(ReopenTask {
            meta: metadata(key),
            precondition: TaskPrecondition {
                task_id: completed.task_id.clone(),
                expected_revision: completed.revision,
                expected_generation: completed.generation,
            },
            amendment: TaskContractAmendment {
                feedback_markdown: feedback_markdown.to_string(),
                request_markdown: None,
                replacement_criteria: None,
                complexity: None,
            },
        })
    };
    work_error!(
        service,
        command("  ", "reopen-blank"),
        StoreError::Work(WorkDomainError::InvalidInput { .. }),
        "blank reopen direction must fail before mutation"
    );
    let reopened = task!(
        service,
        command(
            "Include the newly discovered edge case.",
            "reopen-completed"
        ),
        "reopen completed task"
    );
    assert_eq!(
        reopened.stage_id.as_str(),
        noema_tasks::PERSONAL_QUEUE_STAGE_ID
    );
    assert_eq!(reopened.generation, completed.generation + 1);
    assert_ne!(reopened.current_contract_id, completed.current_contract_id);
    assert!(reopened.completed_submission_id.is_none());
    assert!(reopened.completed_at.is_none());

    let detail = store
        .get_work_task(&reopened.task_id)
        .await
        .expect("load reopened detail")
        .expect("reopened detail");
    assert_eq!(
        detail.current_run.as_ref().map(|run| run.run_kind),
        Some(noema_tasks::RunKind::Executor)
    );
    assert!(detail.messages.iter().any(|message| {
        message.body_markdown == "Include the newly discovered edge case."
            && message.task_generation == reopened.generation
    }));

    let claimed = service
        .claim_next_work_run("worker:reopen-context", 60, &[])
        .await
        .expect("claim reopened executor")
        .expect("reopened executor");
    assert!(claimed.run.triggering_review_id.is_none());
    let fence = WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id.clone(),
    };
    service
        .start_work_run(&fence, ACTOR, None, "correlation:reopen-context")
        .await
        .expect("start reopened executor");
    let admitted = service
        .admit_work_run_execution_context(
            &fence,
            ACTOR,
            None,
            "correlation:reopen-context-admission",
        )
        .await
        .expect("admit reopened executor context");
    assert_eq!(
        admitted
            .context
            .contract
            .as_ref()
            .map(|contract| &contract.contract_id),
        reopened.current_contract_id.as_ref()
    );
    assert!(admitted.context.latest_submission.is_none());
    assert!(admitted.context.latest_review.is_none());
    let amendment = admitted
        .context
        .messages
        .iter()
        .find(|message| message.kind == TaskMessageKind::HumanChangeRequest)
        .expect("reopen amendment");
    assert_eq!(amendment.contract_id, reopened.current_contract_id);
    assert_eq!(amendment.review_id, completed.latest_review_id);
    assert_eq!(
        amendment.consumed_by_run_id.as_deref(),
        Some(claimed.run.run_id.as_str())
    );
}

#[tokio::test]
async fn scheduling_preserves_task_identity_and_fences_schedule_state() {
    let (_, service) = fixture().await;
    let captured = task!(service, capture("schedule:capture", "Scheduled"), "capture");
    let scheduled = task!(
        service,
        schedule("schedule:set", &captured, 2_000_000_000, None),
        "schedule"
    );
    assert_eq!(scheduled.task_id, captured.task_id);
    assert_eq!(scheduled.scheduled_for, Some(2_000_000_000));
    work_error!(
        service,
        schedule("schedule:stale", &captured, 2_000_000_100, None),
        StoreError::Work(WorkDomainError::StaleRevision),
        "stale schedule fence"
    );
    let unscheduled = task!(
        service,
        WorkCommand::UnscheduleTask(UnscheduleTask {
            meta: metadata("schedule:clear"),
            precondition: precondition(&scheduled),
        }),
        "unschedule"
    );
    assert_eq!(unscheduled.task_id, captured.task_id);
    assert!(unscheduled.scheduled_for.is_none());
}

#[tokio::test]
async fn run_now_preserves_scheduled_task_identity_and_blocks_plain_queue() {
    let (store, service) = fixture().await;
    let captured = task!(service, capture("run-now:capture", "Run now"), "capture");
    let scheduled = task!(
        service,
        schedule("run-now:schedule", &captured, 2_000_000_000, None),
        "schedule"
    );
    assert!(
        store
            .get_work_task(&scheduled.task_id)
            .await
            .unwrap()
            .unwrap()
            .valid_actions
            .contains(&crate::WorkTaskValidAction::RunNow)
    );
    work_error!(
        service,
        queue("run-now:plain-queue", &scheduled),
        StoreError::Work(WorkDomainError::InvalidTransition),
        "plain Queue cannot bypass future authorization"
    );
    let command = WorkCommand::RunScheduledTaskNow(RunScheduledTaskNow {
        meta: metadata("run-now:start"),
        precondition: precondition(&scheduled),
    });
    let started = task!(service, command.clone(), "run scheduled task now");
    let replay = task!(service, command, "replay run now");
    assert_eq!(started.task_id, scheduled.task_id);
    assert_eq!(replay.task_id, started.task_id);
    assert_eq!(
        started.stage_id.as_str(),
        noema_tasks::PERSONAL_QUEUE_STAGE_ID
    );
    assert_eq!(started.scheduled_for, scheduled.scheduled_for);
}

#[tokio::test]
async fn recurrence_keeps_first_task_snapshot_and_future_template_authority_separate() {
    let (store, service) = fixture().await;
    let captured = task!(service, capture("repeat:capture", "Original"), "capture");
    let scheduled_for = noema_tasks::parse_utc_instant("2030-01-01T08:00:00Z", "start").unwrap();
    let first = task!(
        service,
        schedule(
            "repeat:set",
            &captured,
            scheduled_for,
            Some(NewTaskRecurrence {
                starts_at: scheduled_for,
                cron_expression: "0 8 * * *".to_string(),
                overlap_policy: OverlapPolicy::Skip,
            })
        ),
        "enable recurrence"
    );
    let recurrence_id = first.recurrence_id.clone().expect("recurrence id");
    let changed = service
        .execute(WorkCommand::UpdateTaskRecurrence(UpdateTaskRecurrence {
            meta: metadata("repeat:update"),
            precondition: noema_tasks::RecurrencePrecondition {
                recurrence_id: recurrence_id.clone(),
                expected_revision: 1,
            },
            title: Some("Future title".to_string()),
            description_markdown: None,
            project_id: None,
            starts_at: None,
            cron_expression: None,
            time_zone: None,
            missed_run_policy: None,
            overlap_policy: None,
        }))
        .await
        .expect("update recurrence");
    assert_eq!(changed.task.expect("first task").title, "Original");
    assert_eq!(
        store
            .get_task_recurrence(&recurrence_id)
            .await
            .unwrap()
            .expect("recurrence")
            .title,
        "Future title"
    );
}

#[tokio::test]
async fn recurrence_run_now_materializes_manual_history_without_advancing_schedule() {
    let (store, service) = fixture().await;
    let captured = task!(
        service,
        capture("repeat-now:capture", "Recurring"),
        "capture"
    );
    let first_at = noema_tasks::parse_utc_instant("2030-01-01T08:00:00Z", "start").unwrap();
    let first = task!(
        service,
        schedule(
            "repeat-now:schedule",
            &captured,
            first_at,
            Some(NewTaskRecurrence {
                starts_at: first_at,
                cron_expression: "0 8 * * *".to_string(),
                overlap_policy: OverlapPolicy::Skip,
            })
        ),
        "schedule recurrence"
    );
    let recurrence_id = first.recurrence_id.clone().expect("recurrence id");
    let first = task!(
        service,
        WorkCommand::RunScheduledTaskNow(RunScheduledTaskNow {
            meta: metadata("repeat-now:first"),
            precondition: precondition(&first),
        }),
        "run first occurrence now"
    );
    let cancelled = task!(
        service,
        WorkCommand::CancelTask(CancelTask {
            meta: metadata("repeat-now:cancel-first"),
            precondition: precondition(&first),
            reason: None,
        }),
        "settle first occurrence"
    );
    let recurrence_before = store
        .get_task_recurrence(&recurrence_id)
        .await
        .unwrap()
        .unwrap();
    let command = WorkCommand::RunTaskRecurrenceNow(RunTaskRecurrenceNow {
        meta: metadata("repeat-now:manual"),
        precondition: noema_tasks::RecurrencePrecondition {
            recurrence_id: recurrence_id.clone(),
            expected_revision: recurrence_before.revision,
        },
    });
    let manual = task!(service, command.clone(), "run recurrence now");
    let replay = task!(service, command, "replay recurrence run now");
    assert_ne!(manual.task_id, cancelled.task_id);
    assert_eq!(manual.task_id, replay.task_id);
    assert_eq!(manual.recurrence_id.as_ref(), Some(&recurrence_id));
    assert_eq!(
        manual.stage_id.as_str(),
        noema_tasks::PERSONAL_QUEUE_STAGE_ID
    );
    let recurrence_after = store
        .get_task_recurrence(&recurrence_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recurrence_after.revision, recurrence_before.revision);
    assert_eq!(recurrence_after.next_run_at, recurrence_before.next_run_at);
    let occurrences = store
        .list_task_recurrence_occurrences(&recurrence_id, 10)
        .await
        .unwrap();
    let manual_occurrence = occurrences
        .iter()
        .find(|occurrence| occurrence.task_id.as_ref() == Some(&manual.task_id))
        .expect("manual occurrence history");
    assert_eq!(
        manual_occurrence.trigger,
        noema_tasks::RecurrenceOccurrenceTrigger::Manual
    );
    work_error!(
        service,
        WorkCommand::RunTaskRecurrenceNow(RunTaskRecurrenceNow {
            meta: metadata("repeat-now:overlap"),
            precondition: noema_tasks::RecurrencePrecondition {
                recurrence_id,
                expected_revision: recurrence_after.revision,
            },
        }),
        StoreError::Work(WorkDomainError::InvalidTransition),
        "manual recurrence overlap is rejected"
    );
}

#[tokio::test]
async fn due_processing_is_idempotent_and_applies_one_time_missed_policy() {
    let (store, service) = fixture().await;
    let now = 2_000_000_000;
    let captured = task!(service, capture("due:capture", "Due"), "capture");
    let due = task!(
        service,
        schedule("due:set", &captured, now - 60, None),
        "schedule"
    );
    assert_eq!(
        service.process_due_work_schedules(now, true).await.unwrap(),
        vec![due.task_id.clone()]
    );
    assert!(
        service
            .process_due_work_schedules(now, true)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .get_work_task(&due.task_id)
            .await
            .unwrap()
            .unwrap()
            .task
            .stage_id
            .as_str(),
        noema_tasks::PERSONAL_QUEUE_STAGE_ID
    );

    let skipped = task!(service, capture("due:skip:capture", "Skipped"), "capture");
    let WorkCommand::ScheduleTask(mut command) = schedule("due:skip:set", &skipped, now - 60, None)
    else {
        unreachable!()
    };
    command.schedule.missed_run_policy = MissedRunPolicy::Skip;
    let skipped = task!(
        service,
        WorkCommand::ScheduleTask(command),
        "schedule skipped"
    );
    service.process_due_work_schedules(now, true).await.unwrap();
    assert_eq!(
        store
            .get_work_task(&skipped.task_id)
            .await
            .unwrap()
            .unwrap()
            .task
            .stage_id
            .as_str(),
        noema_tasks::PERSONAL_CANCELLED_STAGE_ID
    );
}

#[tokio::test]
async fn overlap_policies_skip_coalesce_or_materialize_a_due_slot() {
    let now = 1_999_999_980;
    for policy in [
        OverlapPolicy::Skip,
        OverlapPolicy::QueueOne,
        OverlapPolicy::Allow,
    ] {
        let (store, service) = fixture().await;
        let captured = task!(service, capture("overlap:capture", "Overlap"), "capture");
        let first = task!(
            service,
            schedule(
                "overlap:set",
                &captured,
                now - 60,
                Some(NewTaskRecurrence {
                    starts_at: now - 60,
                    cron_expression: "* * * * *".to_string(),
                    overlap_policy: policy,
                })
            ),
            "schedule recurrence"
        );
        let recurrence_id = first.recurrence_id.clone().unwrap();
        service
            .process_due_work_schedules(now, false)
            .await
            .unwrap();
        let occurrences = store
            .list_task_recurrence_occurrences(&recurrence_id, 5)
            .await
            .unwrap();
        let expected = match policy {
            OverlapPolicy::Skip => noema_tasks::RecurrenceOccurrenceResolution::Skipped,
            OverlapPolicy::QueueOne => noema_tasks::RecurrenceOccurrenceResolution::Coalesced,
            OverlapPolicy::Allow => noema_tasks::RecurrenceOccurrenceResolution::Materialized,
        };
        assert_eq!(occurrences[0].resolution, expected);
        if policy == OverlapPolicy::QueueOne {
            let query_id = recurrence_id.clone();
            store
                .with_connection(move |connection| {
                    connection.execute(
                        "UPDATE tasks SET stage_id = ?2 WHERE recurrence_id = ?1",
                        rusqlite::params![query_id.as_str(), noema_tasks::PERSONAL_DONE_STAGE_ID],
                    )?;
                    Ok(())
                })
                .await
                .unwrap();
            service
                .process_due_work_schedules(now, false)
                .await
                .unwrap();
            assert_eq!(
                store
                    .list_task_recurrence_occurrences(&recurrence_id, 5)
                    .await
                    .unwrap()[0]
                    .resolution,
                noema_tasks::RecurrenceOccurrenceResolution::Materialized
            );
        }
    }
}
