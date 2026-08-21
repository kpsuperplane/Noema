//! Product-facing Work tool schemas.
//!
//! This module owns only model-visible names, descriptions, and strict input
//! schemas. Runtime metadata and command dispatch stay in the parent/dispatch
//! modules so model payloads cannot provide authority-bearing fields.

use noema_capabilities::ToolSpec;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    PROJECT_ARCHIVE_TOOL, PROJECT_CREATE_TOOL, PROJECT_LIST_TOOL, PROJECT_REOPEN_TOOL,
    PROJECT_UPDATE_TOOL, TASK_ANSWER_TOOL, TASK_CANCEL_TOOL, TASK_CAPTURE_TOOL,
    TASK_CONTINUE_EXECUTION_TOOL, TASK_DELEGATE_TOOL, TASK_FINISH_EXECUTION_TOOL,
    TASK_FINISH_PLANNING_TOOL, TASK_FINISH_REVIEW_TOOL, TASK_LIST_TOOL, TASK_QUEUE_TOOL,
    TASK_RECURRENCE_END_TOOL, TASK_RECURRENCE_PAUSE_TOOL, TASK_RECURRENCE_RESUME_TOOL,
    TASK_RECURRENCE_SKIP_NEXT_TOOL, TASK_RECURRENCE_UPDATE_TOOL, TASK_REOPEN_TOOL,
    TASK_REPORT_BLOCKED_TOOL, TASK_RESCHEDULE_TOOL, TASK_RETRY_TOOL, TASK_RUN_RECURRENCE_NOW_TOOL,
    TASK_RUN_SCHEDULED_NOW_TOOL, TASK_SCHEDULE_TOOL, TASK_UNSCHEDULE_TOOL, TASK_UPDATE_TOOL,
};
use noema_tasks::TaskComplexity;

macro_rules! arguments {
    ($name:ident { $($(#[$meta:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Debug, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub(in crate::daemon::task_tool) struct $name {
            $($(#[$meta])* pub(in crate::daemon::task_tool) $field: $ty),*
        }
    };
}

arguments! { CaptureArguments {
    title: String,
    #[serde(default)]
    task_document: String,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    schedule: Option<ScheduleFieldsArguments>,
    #[serde(default)]
    executor_agent_id: Option<String>,
    #[serde(default)]
    cwd_override: Option<String>,
} }

arguments! { ScheduleFieldsArguments {
    scheduled_for: String,
    #[serde(default)] time_zone: Option<String>,
    #[serde(default)] missed_run_policy: Option<noema_tasks::MissedRunPolicy>,
    #[serde(default)] recurrence: Option<RecurrenceFieldsArguments>,
} }

arguments! { RecurrenceFieldsArguments {
    starts_at: String,
    cron_expression: String,
    #[serde(default)] overlap_policy: Option<noema_tasks::OverlapPolicy>,
} }

arguments! { ScheduleArguments {
    #[serde(flatten)] precondition: TaskPreconditionArguments,
    #[serde(flatten)] schedule: ScheduleFieldsArguments,
} }

arguments! { RecurrencePreconditionArguments {
    recurrence_id: String,
    expected_revision: u64,
} }

arguments! { RecurrenceUpdateArguments {
    #[serde(flatten)] precondition: RecurrencePreconditionArguments,
    #[serde(default)] title: Option<String>,
    #[serde(default)] task_document: Option<String>,
    #[serde(default)] project_id: Option<String>,
    #[serde(default)] clear_project: bool,
    #[serde(default)] starts_at: Option<String>,
    #[serde(default)] cron_expression: Option<String>,
    #[serde(default)] time_zone: Option<String>,
    #[serde(default)] missed_run_policy: Option<noema_tasks::MissedRunPolicy>,
    #[serde(default)] overlap_policy: Option<noema_tasks::OverlapPolicy>,
} }

arguments! { DelegateArguments {
    title: String,
    task_document: String,
    project: DelegateProjectArguments,
    #[serde(default)]
    complexity_hint: Option<TaskComplexity>,
    #[serde(default)]
    execution_intent: Option<ExecutionIntentArguments>,
    #[serde(default)]
    executor_agent_id: Option<String>,
    #[serde(default)]
    cwd_override: Option<String>,
} }

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::daemon::task_tool) enum DelegateProjectArguments {
    None,
    Existing { project_id: String },
}

arguments! { ExecutionIntentArguments {
    request_markdown: String,
    complexity: TaskComplexity,
} }

arguments! { TaskPreconditionArguments {
    task_id: String,
    expected_revision: u64,
    expected_generation: u64,
} }

arguments! { UpdateArguments {
    #[serde(flatten)]
    precondition: TaskPreconditionArguments,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    task_document: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    clear_project: bool,
    #[serde(default)]
    executor_agent_id: Option<String>,
    #[serde(default)]
    cwd_override: Option<String>,
    #[serde(default)]
    clear_cwd_override: bool,
} }

arguments! { GateArguments {
    #[serde(flatten)]
    precondition: TaskPreconditionArguments,
    gate_id: String,
    answer_markdown: String,
    #[serde(default)]
    approval_decision: Option<noema_tasks::ApprovalDecision>,
} }

arguments! { RetryArguments {
    #[serde(flatten)]
    precondition: TaskPreconditionArguments,
    gate_id: String,
    #[serde(default)]
    retry_note: Option<String>,
} }

arguments! { ReopenArguments {
    #[serde(flatten)]
    precondition: TaskPreconditionArguments,
    feedback_markdown: String,
    #[serde(default)]
    request_markdown: Option<String>,
    #[serde(default)]
    complexity: Option<TaskComplexity>,
} }

arguments! { CancelArguments {
    #[serde(flatten)]
    precondition: TaskPreconditionArguments,
    #[serde(default)]
    reason: Option<String>,
} }

arguments! { ProjectCreateArguments {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    folder: Option<String>,
} }

arguments! { ProjectUpdateArguments {
    #[serde(flatten)]
    precondition: ProjectPreconditionArguments,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    clear_folder: bool,
} }

arguments! { ProjectPreconditionArguments {
    project_id: String,
    expected_revision: u64,
} }

pub(crate) fn primary_task_tool_specs()
-> Result<Vec<ToolSpec>, noema_capabilities::ToolContractError> {
    [
        (TASK_CAPTURE_TOOL, "Capture work in Inbox, optionally with future execution and Repeat.", json!({"type":"object","properties":{"title":{"type":"string","minLength":1,"maxLength":200},"task_document":{"type":"string","maxLength":65536},"project_id":{"type":"string","minLength":1,"maxLength":255},"executor_agent_id":{"type":"string","minLength":1,"maxLength":255},"cwd_override":{"type":"string","minLength":1,"maxLength":4096},"schedule":schedule_schema()},"required":["title"],"additionalProperties":false})),
        (TASK_LIST_TOOL, "List bounded owner-authorized Task summaries. Each active_gate contains the exact authority for task.answer or task.retry. Each recurrence_authority contains the current future template and revision.", json!({"type":"object","properties":{"project_id":{"type":"string","minLength":1},"stage_behavior":{"type":"string","enum":["intake","dispatch","active","human_gate","terminal_success","terminal_cancelled"]},"attention_only":{"type":"boolean"},"limit":{"type":"integer","minimum":1,"maximum":100}},"additionalProperties":false})),
        (TASK_UPDATE_TOOL, "Update an Inbox Task and its document with revision fences.", json!({"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"title":{"type":"string","minLength":1,"maxLength":200},"task_document":{"type":"string","maxLength":65536},"project_id":{"type":"string","minLength":1},"clear_project":{"type":"boolean"},"executor_agent_id":{"type":"string","minLength":1,"maxLength":255},"cwd_override":{"type":"string","minLength":1,"maxLength":4096},"clear_cwd_override":{"type":"boolean"}},"required":["task_id","expected_revision","expected_generation"],"additionalProperties":false})),
        (TASK_QUEUE_TOOL, "Authorize an Inbox task for planning/execution.", task_fenced_schema()),
        (TASK_SCHEDULE_TOOL, "Schedule an ordinary Inbox task. Use exact RFC3339 instants and the user's IANA timezone; include recurrence only when Repeat is requested.", scheduled_task_schema()),
        (TASK_RESCHEDULE_TOOL, "Replace timing for a scheduled Inbox task; this may enable or remove Repeat before execution begins.", scheduled_task_schema()),
        (TASK_UNSCHEDULE_TOOL, "Remove future execution from a one-time Inbox task.", task_fenced_schema()),
        (TASK_RUN_SCHEDULED_NOW_TOOL, "Start an already scheduled Inbox task immediately. For the pending first occurrence of a recurring series, use this command so the existing task identity is preserved.", task_fenced_schema()),
        (TASK_RECURRENCE_UPDATE_TOOL, "Edit the future template for a recurring Task. This never changes an active or historical occurrence. If the human also answered an active occurrence gate, update recurrence first and then call task.answer.", json!({"type":"object","properties":{"recurrence_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"title":{"type":"string","minLength":1},"task_document":{"type":"string","maxLength":65536},"project_id":{"type":"string"},"clear_project":{"type":"boolean"},"starts_at":{"type":"string","format":"date-time"},"cron_expression":{"type":"string"},"time_zone":{"type":"string"},"missed_run_policy":{"type":"string","enum":["skip","run_once"]},"overlap_policy":{"type":"string","enum":["skip","queue_one","allow"]}},"required":["recurrence_id","expected_revision"],"additionalProperties":false})),
        (TASK_RECURRENCE_PAUSE_TOOL, "Pause future recurring materialization.", recurrence_fenced_schema()),
        (TASK_RECURRENCE_RESUME_TOOL, "Resume a paused recurrence using its missed-run policy.", recurrence_fenced_schema()),
        (TASK_RECURRENCE_SKIP_NEXT_TOOL, "Skip the next exact recurring slot.", recurrence_fenced_schema()),
        (TASK_RECURRENCE_END_TOOL, "End all future recurrence without changing active work.", recurrence_fenced_schema()),
        (TASK_RUN_RECURRENCE_NOW_TOOL, "Create and start one extra occurrence now without advancing the next scheduled run. This is unavailable while another occurrence is nonterminal.", recurrence_fenced_schema()),
        (TASK_DELEGATE_TOOL, "Capture and authorize one autonomous Task. Preserve the human's requested outcome, scope, and delivery depth in the title and Task document. Normally structure the Task document with `## Objective`, `## Requirements`, and `## Expected result`. Omit empty or irrelevant sections. Do not add optional deliverables. Projects are optional. Use complexity_hint only when execution_intent is omitted.", json!({"type":"object","properties":{"title":{"type":"string","minLength":1,"maxLength":200},"task_document":{"type":"string","minLength":1,"maxLength":65536},"project":{"description":"Explicit project placement. Choose none for a projectless task, or existing with an exact project_id returned by project.list.","oneOf":[{"type":"object","properties":{"kind":{"type":"string","enum":["none"]}},"required":["kind"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","enum":["existing"]},"project_id":{"type":"string","minLength":1,"maxLength":255}},"required":["kind","project_id"],"additionalProperties":false}]},"executor_agent_id":{"type":"string","minLength":1,"maxLength":255},"cwd_override":{"type":"string","minLength":1,"maxLength":4096},"complexity_hint":{"type":"string","description":"Planner selection hint. Omit when execution_intent is provided.","enum":["simple","medium","difficult"]},"execution_intent":{"type":"object","description":"Complete execution intent that skips planning. Omit complexity_hint when provided.","properties":{"request_markdown":{"type":"string","minLength":1,"maxLength":20000},"complexity":{"type":"string","enum":["simple","medium","difficult"]}},"required":["request_markdown","complexity"],"additionalProperties":false}},"required":["title","task_document","project"],"additionalProperties":false})),
        (TASK_ANSWER_TOOL, "Answer the exact active_gate returned by task.list. This resolves only that occurrence and never edits future recurring authority.", json!({"type":"object","properties":{"task_id":{"type":"string"},"gate_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"answer_markdown":{"type":"string","minLength":1,"maxLength":20000},"approval_decision":{"type":"string","enum":["approved","declined"]}},"required":["task_id","gate_id","expected_revision","expected_generation","answer_markdown"],"additionalProperties":false})),
        (TASK_RETRY_TOOL, "Retry the explicitly named eligible Recovery gate.", json!({"type":"object","properties":{"task_id":{"type":"string"},"gate_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"retry_note":{"type":"string","maxLength":4000}},"required":["task_id","gate_id","expected_revision","expected_generation"],"additionalProperties":false})),
        (TASK_CANCEL_TOOL, "Cancel a nonterminal task and fence active work.", json!({"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"reason":{"type":"string","maxLength":4000}},"required":["task_id","expected_revision","expected_generation"],"additionalProperties":false})),
        (TASK_REOPEN_TOOL, "Reopen a completed task into Queue with new direction.", json!({"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"feedback_markdown":{"type":"string","minLength":1,"maxLength":20000},"request_markdown":{"type":"string","maxLength":20000},"complexity":{"type":"string","enum":["simple","medium","difficult"]}},"required":["task_id","expected_revision","expected_generation","feedback_markdown"],"additionalProperties":false})),
        (PROJECT_CREATE_TOOL, "Create a Personal project container.", json!({"type":"object","properties":{"name":{"type":"string","minLength":1,"maxLength":200},"description":{"type":"string","maxLength":20000},"folder":{"type":"string","minLength":1,"maxLength":4096}},"required":["name"],"additionalProperties":false})),
        (PROJECT_LIST_TOOL, "List bounded Personal projects.", json!({"type":"object","properties":{"include_archived":{"type":"boolean"},"limit":{"type":"integer","minimum":1,"maximum":100}},"additionalProperties":false})),
        (PROJECT_UPDATE_TOOL, "Update a project with its revision fence.", json!({"type":"object","properties":{"project_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"name":{"type":"string","minLength":1},"description":{"type":"string"},"folder":{"type":"string","minLength":1,"maxLength":4096},"clear_folder":{"type":"boolean"}},"required":["project_id","expected_revision"],"additionalProperties":false})),
        (PROJECT_ARCHIVE_TOOL, "Archive a project without changing task stages.", project_fenced_schema()),
        (PROJECT_REOPEN_TOOL, "Reopen an archived project.", project_fenced_schema()),
    ]
    .into_iter()
    .map(|(name, description, schema)| ToolSpec::new(name, description, schema))
    .collect()
}

fn task_fenced_schema() -> Value {
    json!({"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1}},"required":["task_id","expected_revision","expected_generation"],"additionalProperties":false})
}

fn schedule_schema() -> Value {
    json!({"type":"object","properties":{"scheduled_for":{"type":"string","format":"date-time"},"time_zone":{"type":"string"},"missed_run_policy":{"type":"string","enum":["skip","run_once"]},"recurrence":{"type":"object","properties":{"starts_at":{"type":"string","format":"date-time"},"cron_expression":{"type":"string"},"overlap_policy":{"type":"string","enum":["skip","queue_one","allow"]}},"required":["starts_at","cron_expression"],"additionalProperties":false}},"required":["scheduled_for"],"additionalProperties":false})
}

fn scheduled_task_schema() -> Value {
    let mut schema = schedule_schema();
    let properties = schema["properties"]
        .as_object_mut()
        .expect("schedule properties");
    properties.insert("task_id".to_string(), json!({"type":"string"}));
    properties.insert(
        "expected_revision".to_string(),
        json!({"type":"integer","minimum":1}),
    );
    properties.insert(
        "expected_generation".to_string(),
        json!({"type":"integer","minimum":1}),
    );
    schema["required"] = json!([
        "task_id",
        "expected_revision",
        "expected_generation",
        "scheduled_for"
    ]);
    schema
}

fn recurrence_fenced_schema() -> Value {
    json!({"type":"object","properties":{"recurrence_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1}},"required":["recurrence_id","expected_revision"],"additionalProperties":false})
}

fn project_fenced_schema() -> Value {
    json!({"type":"object","properties":{"project_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1}},"required":["project_id","expected_revision"],"additionalProperties":false})
}

pub(crate) fn task_finish_planning_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_FINISH_PLANNING_TOOL,
        "Finish planning after the current plan is saved in TASK.md.",
        json!({"type":"object","properties":{"complexity":{"type":"string","enum":["simple","medium","difficult"]}},"required":["complexity"],"additionalProperties":false}),
    )
}

pub(crate) fn task_report_blocked_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_REPORT_BLOCKED_TOOL,
        "Open one focused clarification or approval gate and stop at a safe boundary. Use a clarification gate when unavailable core evidence requires an alternate source or reduced scope. Keep the copy brief. Suggested answers render as separate controls, so do not repeat them in the question or context.",
        json!({"type":"object","properties":{"gate_kind":{"type":"string","enum":["clarification","approval"]},"question":{"type":"string","description":"One brief standalone question. When suggested_answers is non-empty, do not quote, enumerate, or otherwise repeat those choices here.","minLength":1,"maxLength":4000},"context_markdown":{"type":"string","description":"Optional brief context needed to answer. Do not restate the question or suggested_answers.","maxLength":20000},"suggested_answers":{"type":"array","description":"Optional concise direct answers rendered as separate controls. Do not duplicate them in question or context_markdown.","maxItems":8,"items":{"type":"string","minLength":1,"maxLength":1000}}},"required":["gate_kind","question"],"additionalProperties":false}),
    )
}

pub(crate) fn task_finish_execution_tool_spec(
    _criterion_ids: &[String],
) -> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_FINISH_EXECUTION_TOOL,
        "Finish execution after the current result is saved in RESULT.md.",
        json!({"type":"object","properties":{},"additionalProperties":false}),
    )
}

pub(crate) fn task_continue_execution_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_CONTINUE_EXECUTION_TOOL,
        "Finish this run and continue execution in a new run from current Task files.",
        json!({"type":"object","properties":{},"additionalProperties":false}),
    )
}

pub(crate) fn task_finish_review_tool_spec(
    _criterion_ids: &[String],
) -> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_FINISH_REVIEW_TOOL,
        "Finish review and replace REVIEW.md with the current feedback.",
        json!({"type":"object","properties":{"decision":{"type":"string","enum":["approve","request_changes","needs_human"]},"feedback":{"type":"string","minLength":1,"maxLength":20000}},"required":["decision","feedback"],"additionalProperties":false}),
    )
}

/// Build the background-role Task inspection tool. The runtime supplies
/// the current task id from the leased run, so the model cannot widen this
/// read to another task by changing arguments.
pub(crate) fn task_list_scoped_tool_spec() -> Result<ToolSpec, noema_capabilities::ToolContractError>
{
    ToolSpec::new(
        TASK_LIST_TOOL,
        "Inspect the current task's bounded owner-scoped summary. The runtime supplies the task identity.",
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    )
}

pub(crate) fn task_file_list_tool_spec() -> Result<ToolSpec, noema_capabilities::ToolContractError>
{
    ToolSpec::new(
        super::TASK_FILE_LIST_TOOL,
        "List one directory inside the current Task's authorized filesystem boundary.",
        json!({"type":"object","properties":{"path":{"type":"string","maxLength":4096,"default":"."}},"additionalProperties":false}),
    )
}

pub(crate) fn task_file_read_tool_spec() -> Result<ToolSpec, noema_capabilities::ToolContractError>
{
    ToolSpec::new(
        super::TASK_FILE_READ_TOOL,
        "Read one UTF-8 text file inside the current Task's authorized filesystem boundary.",
        json!({"type":"object","properties":{"path":{"type":"string","minLength":1,"maxLength":4096}},"required":["path"],"additionalProperties":false}),
    )
}

pub(crate) fn task_file_write_tool_spec() -> Result<ToolSpec, noema_capabilities::ToolContractError>
{
    ToolSpec::new(
        super::TASK_FILE_WRITE_TOOL,
        "Atomically create or replace one UTF-8 text file inside the current Task directory.",
        json!({"type":"object","properties":{"path":{"type":"string","minLength":1,"maxLength":4096},"content":{"type":"string","maxLength":65536}},"required":["path","content"],"additionalProperties":false}),
    )
}

pub(crate) fn task_file_delete_tool_spec() -> Result<ToolSpec, noema_capabilities::ToolContractError>
{
    ToolSpec::new(
        super::TASK_FILE_DELETE_TOOL,
        "Delete one text file inside the current Task directory. TASK.md and RESULT.md cannot be deleted.",
        json!({"type":"object","properties":{"path":{"type":"string","minLength":1,"maxLength":4096}},"required":["path"],"additionalProperties":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegation_hints_flexible_task_document_sections() {
        let tools = primary_task_tool_specs().expect("primary Task tools");
        let delegate = tools
            .iter()
            .find(|tool| tool.name.as_str() == TASK_DELEGATE_TOOL)
            .expect("delegation tool");

        for heading in [
            "`## Objective`",
            "`## Requirements`",
            "`## Expected result`",
        ] {
            assert!(delegate.description.contains(heading));
        }
        assert!(!delegate.description.contains("`# "));
        assert!(
            delegate
                .description
                .contains("Omit empty or irrelevant sections")
        );
    }

    #[test]
    fn executor_finish_schema_contains_no_task_content() {
        let schema = task_finish_execution_tool_spec(&[
            "criterion:one".to_string(),
            "criterion:two".to_string(),
        ])
        .expect("executor result tool");
        assert_eq!(schema.name.as_str(), "task.finish_execution");
        assert_eq!(schema.input_schema.as_value()["properties"], json!({}));
        assert!(schema.input_schema.as_value().get("required").is_none());
    }

    #[test]
    fn reviewer_finish_schema_contains_only_current_decision_and_feedback() {
        let schema = task_finish_review_tool_spec(&[
            "criterion:one".to_string(),
            "criterion:two".to_string(),
        ])
        .expect("review tool");
        let schema = schema.input_schema.as_value();
        assert_eq!(schema["type"], "object");
        assert_eq!(
            schema["properties"]["decision"]["enum"],
            json!(["approve", "request_changes", "needs_human"])
        );
        assert_eq!(schema["required"], json!(["decision", "feedback"]));
        assert_eq!(
            schema["properties"].as_object().map(|value| value.len()),
            Some(2)
        );
    }

    #[test]
    fn blocked_tool_keeps_structured_choices_out_of_question_copy() {
        let tool = task_report_blocked_tool_spec().expect("blocked tool");
        let properties = &tool.input_schema.as_value()["properties"];

        assert!(tool.description.contains("separate controls"));
        assert!(
            properties["question"]["description"]
                .as_str()
                .is_some_and(|description| description.contains("do not quote, enumerate"))
        );
        assert!(
            properties["context_markdown"]["description"]
                .as_str()
                .is_some_and(|description| description.contains("Do not restate"))
        );
    }
}
