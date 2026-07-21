use noema_tasks::{
    ContractOrigin, RunKind, RunTerminalKind, TaskStageChangeReason, WorkDomainError,
    WorkEventPayload, WorkflowStageBehavior, WorkflowStageId,
};
use rusqlite::Transaction;

use super::super::{PlanTerminal, SubmitPlan};
use super::terminal_helpers::{
    OpenGate, bump_task_to_waiting_tx, insert_gate_tx, load_running_fence_tx,
    load_terminal_run_identity_tx, mark_run_completed_tx, mark_run_waiting_tx,
    normalize_new_criteria_for_store, replay_plan_terminal_tx, scope,
};
use crate::{
    StoreError,
    work_commands::{
        WorkCommandService, helpers,
        tasks::{CreateContract, create_contract_tx},
    },
    work_events::append_work_event_tx,
    work_notifications::enqueue_work_notification_tx,
};

pub(super) fn submit_plan_tx(
    transaction: &Transaction<'_>,
    service: &WorkCommandService,
    command: &SubmitPlan,
    actor_id: &str,
    causation_id: Option<&str>,
    correlation_id: &str,
) -> Result<helpers::CommandWrite, StoreError> {
    let (run, mut task) = load_running_fence_tx(transaction, &command.fence, RunKind::Planner)?;
    if task.stage_behavior != WorkflowStageBehavior::Active {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    let PlanTerminal::Complete(plan) = &command.terminal else {
        return block_for_human_tx(
            transaction,
            command,
            &run,
            &mut task,
            actor_id,
            causation_id,
            correlation_id,
        );
    };
    let event_context = || helpers::CommandEventContext {
        actor_id,
        causation_id,
        correlation_id,
    };

    let criteria = normalize_new_criteria_for_store(&plan.criteria)?;
    let (contract_id, _contract_event) = create_contract_tx(
        transaction,
        service.provider_registry.as_ref(),
        &task,
        CreateContract {
            origin: ContractOrigin::Planned,
            request_markdown: &plan.request_markdown,
            execution_plan_markdown: Some(&plan.execution_plan_markdown),
            criteria: &criteria,
            complexity: plan.complexity,
            supersedes_contract_id: None,
            event: event_context(),
        },
    )?;
    mark_run_completed_tx(transaction, &run, &command.fence)?;
    let mut task_for_child = task.clone();
    task_for_child.current_contract_id = Some(contract_id.clone());
    let _completed_event = append_work_event_tx(
        transaction,
        scope(
            &task,
            (actor_id, causation_id, correlation_id),
            Some(&run.run_id),
        ),
        WorkEventPayload::run_completed(run.run_kind, run.task_generation, RunTerminalKind::Plan)
            .map_err(StoreError::Work)?,
    )?;
    let (child_run_id, child_event) = helpers::queue_run_tx(
        transaction,
        service.provider_registry.as_ref(),
        &task_for_child,
        helpers::QueueRun {
            run_kind: RunKind::Executor,
            contract_id: Some(&contract_id),
            planner_complexity: None,
            review_round: 1,
            attempt_index: 0,
            parent_run_id: Some(&run.run_id),
            triggering_submission_id: None,
            triggering_review_id: None,
            event: event_context(),
        },
    )?;
    Ok(helpers::task_write(child_event, task.task_id)
        .contract(Some(contract_id))
        .run(Some(child_run_id)))
}

pub(super) fn replay_tx(
    transaction: &Transaction<'_>,
    command: &SubmitPlan,
) -> Result<Option<helpers::CommandWrite>, StoreError> {
    let run = load_terminal_run_identity_tx(transaction, &command.fence, Some(RunKind::Planner))?;
    let Some(replay) = replay_plan_terminal_tx(transaction, &run, &command.terminal)? else {
        return Ok(None);
    };
    Ok(Some(
        helpers::task_write(replay.event, run.task_id)
            .contract(replay.contract_id)
            .gate(replay.gate_id)
            .run(Some(replay.run_id)),
    ))
}

fn block_for_human_tx(
    transaction: &Transaction<'_>,
    command: &SubmitPlan,
    run: &noema_tasks::AgentRunRecord,
    task: &mut helpers::TaskState,
    actor_id: &str,
    causation_id: Option<&str>,
    correlation_id: &str,
) -> Result<helpers::CommandWrite, StoreError> {
    let PlanTerminal::BlockingQuestion {
        prompt_markdown,
        context_markdown,
        gate_kind,
    } = &command.terminal
    else {
        unreachable!()
    };
    let gate_id = insert_gate_tx(
        transaction,
        task,
        OpenGate {
            gate_kind: *gate_kind,
            prompt: prompt_markdown,
            context: context_markdown,
            actor_id,
            originating_run_id: Some(&run.run_id),
            recovery_reason: None,
            retry_run_kind: None,
        },
    )?;
    let revision = bump_task_to_waiting_tx(transaction, task)?;
    mark_run_waiting_tx(transaction, run, &command.fence)?;
    let run_scope = || {
        scope(
            task,
            (actor_id, causation_id, correlation_id),
            Some(&run.run_id),
        )
    };
    let _gate_event = append_work_event_tx(
        transaction,
        run_scope(),
        WorkEventPayload::gate_opened(
            gate_id.clone(),
            task.generation,
            *gate_kind,
            Some(run.run_id.clone()),
            None,
            None,
        )
        .map_err(StoreError::Work)?,
    )?;
    let _stage_event = append_work_event_tx(
        transaction,
        run_scope(),
        WorkEventPayload::task_stage_changed(
            revision,
            task.generation,
            WorkflowStageId::new("stage:personal:doing").map_err(StoreError::Work)?,
            task.stage_id.clone(),
            TaskStageChangeReason::GateOpened,
        )
        .map_err(StoreError::Work)?,
    )?;
    let waiting_event = append_work_event_tx(
        transaction,
        run_scope(),
        WorkEventPayload::run_waiting_for_approval(
            run.run_kind,
            run.task_generation,
            gate_id.clone(),
            *gate_kind,
        )
        .map_err(StoreError::Work)?,
    )?;
    let event = enqueue_work_notification_tx(
        transaction,
        &waiting_event,
        noema_tasks::NotificationKind::TaskWaiting,
        &serde_json::json!({
            "task_id": task.task_id.as_str(),
            "gate_id": gate_id.as_str(),
            "action_needed": true,
        }),
    )?
    .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    Ok(helpers::task_write(event, task.task_id.clone())
        .gate(Some(gate_id))
        .run(Some(run.run_id.clone())))
}
