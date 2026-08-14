use noema_store::{SubmitTaskResult, WorkCommandService, WorkRunFence, WorkRunTerminal};
use noema_tasks::{
    AgentRunItemKind, AgentRunItemStatus, NewAgentRunItem, NewTaskSubmission,
    SubmissionCriterionEvidence,
};
use serde_json::json;

use super::{TaskSubmissionEvidenceContext, execute_task_read_submission_evidence};

#[tokio::test]
async fn reviewer_lists_metadata_then_reads_exact_submitted_item() {
    let fixture = reviewer_fixture().await;

    let listed = execute_task_read_submission_evidence(
        &fixture.store,
        &fixture.context,
        &json!({"first": 1}),
    )
    .await
    .expect("list submitted evidence");
    let items = listed["items"].as_array().expect("evidence items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["item_id"], "run_item:evidence:large");
    assert!(items[0].get("payload").is_none());
    assert_eq!(items[0]["content_truncated"], true);
    assert_eq!(listed["has_more"], true);

    let exact = execute_task_read_submission_evidence(
        &fixture.store,
        &fixture.context,
        &json!({"item_id": "run_item:evidence:large"}),
    )
    .await
    .expect("read exact submitted evidence");
    assert_eq!(exact["executor_run_id"], fixture.executor_run_id);
    assert_eq!(
        exact["item"]["payload"]["ordinary_account_id"],
        "account:123"
    );
    assert_eq!(
        exact["item"]["payload"]["saved_evidence"]
            .as_str()
            .expect("saved evidence")
            .len(),
        4_096
    );
}

#[tokio::test]
async fn reviewer_rejects_items_outside_the_submitted_executor_run() {
    let fixture = reviewer_fixture().await;
    fixture
        .store
        .append_agent_run_item(
            NewAgentRunItem {
                item_id: Some("run_item:evidence:reviewer".to_string()),
                run_id: fixture.context.run_id.clone(),
                round_index: 0,
                kind: AgentRunItemKind::AssistantOutput,
                status: AgentRunItemStatus::Completed,
                correlation_id: None,
                parent_item_id: None,
                content_text: Some("Reviewer-private item".to_string()),
                payload: json!({}),
            },
            &fixture.reviewer_fence,
        )
        .await
        .expect("append reviewer item");

    let error = execute_task_read_submission_evidence(
        &fixture.store,
        &fixture.context,
        &json!({"item_id": "run_item:evidence:reviewer"}),
    )
    .await
    .expect_err("reviewer item must stay outside submission evidence");
    assert_eq!(error, "submission evidence item is unavailable");

    let wrong_role = TaskSubmissionEvidenceContext {
        task_id: fixture.context.task_id,
        run_id: fixture.executor_run_id,
    };
    let error = execute_task_read_submission_evidence(&fixture.store, &wrong_role, &json!({}))
        .await
        .expect_err("Executor run must not access reviewer evidence tool");
    assert_eq!(error, "submission evidence context is unavailable");
}

struct ReviewerEvidenceFixture {
    store: noema_store::NoemaStore,
    context: TaskSubmissionEvidenceContext,
    executor_run_id: String,
    reviewer_fence: WorkRunFence,
}

async fn reviewer_fixture() -> ReviewerEvidenceFixture {
    let store = crate::test_support::test_store().await;
    let (task, queued_run) = crate::test_support::seed_task(&store, "Review saved evidence").await;
    let service = WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );
    let claimed = service
        .claim_next_work_run("worker:evidence:executor", 120, &[])
        .await
        .expect("claim executor")
        .expect("executor run");
    assert_eq!(claimed.run.run_id, queued_run.run_id);
    let executor_fence = WorkRunFence {
        run_id: claimed.run.run_id.clone(),
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
        contract_id: claimed.run.contract_id.clone(),
    };
    service
        .start_work_run(
            &executor_fence,
            "actor:test",
            None,
            "correlation:evidence:executor",
        )
        .await
        .expect("start executor");
    let detail = store
        .get_work_task(&task.task_id)
        .await
        .expect("read task")
        .expect("task detail");
    let contract = detail.current_contract.expect("task contract");
    let criterion_id = contract.criteria[0].criterion_id.clone();
    store
        .append_agent_run_item(
            NewAgentRunItem {
                item_id: Some("run_item:evidence:small".to_string()),
                run_id: claimed.run.run_id.clone(),
                round_index: 0,
                kind: AgentRunItemKind::AssistantOutput,
                status: AgentRunItemStatus::Completed,
                correlation_id: None,
                parent_item_id: None,
                content_text: Some("Prepared connector definition".to_string()),
                payload: json!({}),
            },
            &executor_fence,
        )
        .await
        .expect("append small evidence");
    store
        .append_agent_run_item(
            NewAgentRunItem {
                item_id: Some("run_item:evidence:large".to_string()),
                run_id: claimed.run.run_id.clone(),
                round_index: 0,
                kind: AgentRunItemKind::ToolResult,
                status: AgentRunItemStatus::Completed,
                correlation_id: Some("call:evidence:large".to_string()),
                parent_item_id: None,
                content_text: Some("x".repeat(300)),
                payload: json!({
                    "ordinary_account_id": "account:123",
                    "saved_evidence": "y".repeat(4_096),
                }),
            },
            &executor_fence,
        )
        .await
        .expect("append large evidence");
    let executor_run_id = claimed.run.run_id.clone();
    service
        .record_work_run_terminal(
            WorkRunTerminal::TaskResult(SubmitTaskResult {
                fence: executor_fence,
                submission: NewTaskSubmission {
                    submission_id: Some("submission:evidence".to_string()),
                    task_id: task.task_id.clone(),
                    contract_id: contract.contract_id,
                    executor_run_id: executor_run_id.clone(),
                    review_round: 1,
                    summary: "Connector proposal is ready".to_string(),
                    result_markdown: "The connector proposal is ready for review.".to_string(),
                    citations: Vec::new(),
                    criteria: vec![SubmissionCriterionEvidence {
                        criterion_id,
                        evidence_markdown: "The saved tool result contains the proposal."
                            .to_string(),
                    }],
                    artifact_ids: Vec::new(),
                },
            }),
            "actor:test",
            None,
            "correlation:evidence:submission",
        )
        .await
        .expect("submit executor result");
    let reviewer = service
        .claim_next_work_run("worker:evidence:reviewer", 120, &[])
        .await
        .expect("claim reviewer")
        .expect("reviewer run");
    let reviewer_fence = WorkRunFence {
        run_id: reviewer.run.run_id.clone(),
        lease_token: reviewer.lease_token,
        task_generation: reviewer.run.task_generation,
        contract_id: reviewer.run.contract_id,
    };
    service
        .start_work_run(
            &reviewer_fence,
            "actor:test",
            None,
            "correlation:evidence:reviewer",
        )
        .await
        .expect("start reviewer");

    ReviewerEvidenceFixture {
        store,
        context: TaskSubmissionEvidenceContext {
            task_id: task.task_id.to_string(),
            run_id: reviewer.run.run_id,
        },
        executor_run_id,
        reviewer_fence,
    }
}
