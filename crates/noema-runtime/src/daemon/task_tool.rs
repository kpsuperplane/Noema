//! Typed primary-agent Work commands and role terminal tools.
//!
//! Model payloads contain only product fields. Actor, provenance, causation,
//! correlation, and idempotency metadata are filled from this trusted runtime
//! context before the Store command service is called.

use serde_json::Value;

mod catalog;
mod dispatch;

pub(crate) use catalog::{
    primary_task_tool_specs, task_continue_execution_tool_spec, task_file_delete_tool_spec,
    task_file_list_tool_spec, task_file_read_tool_spec, task_file_write_tool_spec,
    task_finish_execution_tool_spec, task_finish_planning_tool_spec, task_finish_review_tool_spec,
    task_list_scoped_tool_spec, task_report_blocked_tool_spec,
};
pub(crate) use dispatch::{
    execute_primary_task_tool, execute_scoped_task_file_tool, execute_scoped_task_list_tool,
};

pub(crate) const TASK_CAPTURE_TOOL: &str = "task.capture";
pub(crate) const TASK_LIST_TOOL: &str = "task.list";
pub(crate) const TASK_UPDATE_TOOL: &str = "task.update";
pub(crate) const TASK_QUEUE_TOOL: &str = "task.queue";
pub(crate) const TASK_SCHEDULE_TOOL: &str = "task.schedule";
pub(crate) const TASK_RESCHEDULE_TOOL: &str = "task.reschedule";
pub(crate) const TASK_UNSCHEDULE_TOOL: &str = "task.unschedule";
pub(crate) const TASK_RUN_SCHEDULED_NOW_TOOL: &str = "task.schedule.run_now";
pub(crate) const TASK_RECURRENCE_UPDATE_TOOL: &str = "task.recurrence.update";
pub(crate) const TASK_RECURRENCE_PAUSE_TOOL: &str = "task.recurrence.pause";
pub(crate) const TASK_RECURRENCE_RESUME_TOOL: &str = "task.recurrence.resume";
pub(crate) const TASK_RECURRENCE_SKIP_NEXT_TOOL: &str = "task.recurrence.skip_next";
pub(crate) const TASK_RECURRENCE_END_TOOL: &str = "task.recurrence.end";
pub(crate) const TASK_RUN_RECURRENCE_NOW_TOOL: &str = "task.recurrence.run_now";
pub(crate) const TASK_DELEGATE_TOOL: &str = "task.delegate";
pub(crate) const TASK_ANSWER_TOOL: &str = "task.answer";
pub(crate) const TASK_RETRY_TOOL: &str = "task.retry";
pub(crate) const TASK_CANCEL_TOOL: &str = "task.cancel";
pub(crate) const TASK_REOPEN_TOOL: &str = "task.reopen";
pub(crate) const PROJECT_CREATE_TOOL: &str = "project.create";
pub(crate) const PROJECT_LIST_TOOL: &str = "project.list";
pub(crate) const PROJECT_UPDATE_TOOL: &str = "project.update";
pub(crate) const PROJECT_ARCHIVE_TOOL: &str = "project.archive";
pub(crate) const PROJECT_REOPEN_TOOL: &str = "project.reopen";
pub(crate) const TASK_FINISH_PLANNING_TOOL: &str = "task.finish_planning";
pub(crate) const TASK_FINISH_EXECUTION_TOOL: &str = "task.finish_execution";
pub(crate) const TASK_CONTINUE_EXECUTION_TOOL: &str = "task.continue_execution";
pub(crate) const TASK_FINISH_REVIEW_TOOL: &str = "task.finish_review";
pub(crate) const TASK_REPORT_BLOCKED_TOOL: &str = "task.report_blocked";
pub(crate) const TASK_FILE_LIST_TOOL: &str = "task.files.list";
pub(crate) const TASK_FILE_READ_TOOL: &str = "task.files.read";
pub(crate) const TASK_FILE_WRITE_TOOL: &str = "task.files.write";
pub(crate) const TASK_FILE_DELETE_TOOL: &str = "task.files.delete";

/// Trusted source context captured from the active primary turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskDelegateRuntimeContext {
    pub conversation_id: String,
    pub turn_id: String,
    pub user_item_id: String,
    pub agent_id: String,
    pub workspace_id: String,
    pub owner_human_id: String,
    pub client_time_zone: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TaskToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

/// Return whether a name is one of the semantic primary task tools.
#[must_use]
pub(crate) fn is_primary_task_tool(name: &str) -> bool {
    matches!(
        name,
        TASK_CAPTURE_TOOL
            | TASK_LIST_TOOL
            | TASK_UPDATE_TOOL
            | TASK_QUEUE_TOOL
            | TASK_SCHEDULE_TOOL
            | TASK_RESCHEDULE_TOOL
            | TASK_UNSCHEDULE_TOOL
            | TASK_RUN_SCHEDULED_NOW_TOOL
            | TASK_RECURRENCE_UPDATE_TOOL
            | TASK_RECURRENCE_PAUSE_TOOL
            | TASK_RECURRENCE_RESUME_TOOL
            | TASK_RECURRENCE_SKIP_NEXT_TOOL
            | TASK_RECURRENCE_END_TOOL
            | TASK_RUN_RECURRENCE_NOW_TOOL
            | TASK_DELEGATE_TOOL
            | TASK_ANSWER_TOOL
            | TASK_RETRY_TOOL
            | TASK_CANCEL_TOOL
            | TASK_REOPEN_TOOL
            | PROJECT_CREATE_TOOL
            | PROJECT_LIST_TOOL
            | PROJECT_UPDATE_TOOL
            | PROJECT_ARCHIVE_TOOL
            | PROJECT_REOPEN_TOOL
    )
}

#[must_use]
pub(crate) fn is_task_delegate_tool(name: &str) -> bool {
    name == TASK_DELEGATE_TOOL
}

#[must_use]
pub(crate) fn is_task_finish_planning_tool(name: &str) -> bool {
    name == TASK_FINISH_PLANNING_TOOL
}

#[must_use]
pub(crate) fn is_task_finish_execution_tool(name: &str) -> bool {
    name == TASK_FINISH_EXECUTION_TOOL
}

#[must_use]
pub(crate) fn is_task_continue_execution_tool(name: &str) -> bool {
    name == TASK_CONTINUE_EXECUTION_TOOL
}

#[must_use]
pub(crate) fn is_task_finish_review_tool(name: &str) -> bool {
    name == TASK_FINISH_REVIEW_TOOL
}

#[must_use]
pub(crate) fn is_task_report_blocked_tool(name: &str) -> bool {
    name == TASK_REPORT_BLOCKED_TOOL
}

#[must_use]
pub(crate) fn is_task_file_tool(name: &str) -> bool {
    matches!(
        name,
        TASK_FILE_LIST_TOOL | TASK_FILE_READ_TOOL | TASK_FILE_WRITE_TOOL | TASK_FILE_DELETE_TOOL
    )
}
