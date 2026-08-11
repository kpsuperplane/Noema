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
    PROJECT_UPDATE_TOOL, TASK_ANSWER_TOOL, TASK_CANCEL_TOOL, TASK_CAPTURE_TOOL, TASK_DELEGATE_TOOL,
    TASK_LIST_TOOL, TASK_QUEUE_TOOL, TASK_RECURRENCE_END_TOOL, TASK_RECURRENCE_PAUSE_TOOL,
    TASK_RECURRENCE_RESUME_TOOL, TASK_RECURRENCE_SKIP_NEXT_TOOL, TASK_RECURRENCE_UPDATE_TOOL,
    TASK_REOPEN_TOOL, TASK_REPORT_BLOCKED_TOOL, TASK_RESCHEDULE_TOOL, TASK_RETRY_TOOL,
    TASK_RUN_RECURRENCE_NOW_TOOL, TASK_RUN_SCHEDULED_NOW_TOOL, TASK_SCHEDULE_TOOL,
    TASK_SUBMIT_PLAN_TOOL, TASK_SUBMIT_RESULT_TOOL, TASK_SUBMIT_REVIEW_TOOL, TASK_UNSCHEDULE_TOOL,
    TASK_UPDATE_TOOL,
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
    description: String,
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
    #[serde(default)] description: Option<String>,
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
    description: String,
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
    criteria: Vec<CriterionArguments>,
    #[serde(default)]
    execution_plan_markdown: Option<String>,
} }

arguments! { CriterionArguments {
    description: String,
    #[serde(default)]
    expected_evidence: Option<String>,
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
    description: Option<String>,
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
    replacement_criteria: Option<Vec<CriterionArguments>>,
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
        (TASK_CAPTURE_TOOL, "Capture work in Inbox, optionally with future execution and Repeat.", json!({"type":"object","properties":{"title":{"type":"string","minLength":1,"maxLength":200},"description":{"type":"string","maxLength":20000},"project_id":{"type":"string","minLength":1,"maxLength":255},"executor_agent_id":{"type":"string","minLength":1,"maxLength":255},"cwd_override":{"type":"string","minLength":1,"maxLength":4096},"schedule":schedule_schema()},"required":["title"],"additionalProperties":false})),
        (TASK_LIST_TOOL, "List bounded owner-authorized Task summaries. Each active_gate contains the exact authority for task.answer or task.retry. Each recurrence_authority contains the current future template and revision.", json!({"type":"object","properties":{"project_id":{"type":"string","minLength":1},"stage_behavior":{"type":"string","enum":["intake","dispatch","active","human_gate","terminal_success","terminal_cancelled"]},"attention_only":{"type":"boolean"},"limit":{"type":"integer","minimum":1,"maximum":100}},"additionalProperties":false})),
        (TASK_UPDATE_TOOL, "Update an Inbox task's capture fields with revision and generation fences.", json!({"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"title":{"type":"string","minLength":1,"maxLength":200},"description":{"type":"string","maxLength":20000},"project_id":{"type":"string","minLength":1},"clear_project":{"type":"boolean"},"executor_agent_id":{"type":"string","minLength":1,"maxLength":255},"cwd_override":{"type":"string","minLength":1,"maxLength":4096},"clear_cwd_override":{"type":"boolean"}},"required":["task_id","expected_revision","expected_generation"],"additionalProperties":false})),
        (TASK_QUEUE_TOOL, "Authorize an Inbox task for planning/execution.", task_fenced_schema()),
        (TASK_SCHEDULE_TOOL, "Schedule an ordinary Inbox task. Use exact RFC3339 instants and the user's IANA timezone; include recurrence only when Repeat is requested.", scheduled_task_schema()),
        (TASK_RESCHEDULE_TOOL, "Replace timing for a scheduled Inbox task; this may enable or remove Repeat before execution begins.", scheduled_task_schema()),
        (TASK_UNSCHEDULE_TOOL, "Remove future execution from a one-time Inbox task.", task_fenced_schema()),
        (TASK_RUN_SCHEDULED_NOW_TOOL, "Start an already scheduled Inbox task immediately. For the pending first occurrence of a recurring series, use this command so the existing task identity is preserved.", task_fenced_schema()),
        (TASK_RECURRENCE_UPDATE_TOOL, "Edit the future template for a recurring Task. This never changes an active or historical occurrence. If the human also answered an active occurrence gate, update recurrence first and then call task.answer.", json!({"type":"object","properties":{"recurrence_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"title":{"type":"string","minLength":1},"description":{"type":"string"},"project_id":{"type":"string"},"clear_project":{"type":"boolean"},"starts_at":{"type":"string","format":"date-time"},"cron_expression":{"type":"string"},"time_zone":{"type":"string"},"missed_run_policy":{"type":"string","enum":["skip","run_once"]},"overlap_policy":{"type":"string","enum":["skip","queue_one","allow"]}},"required":["recurrence_id","expected_revision"],"additionalProperties":false})),
        (TASK_RECURRENCE_PAUSE_TOOL, "Pause future recurring materialization.", recurrence_fenced_schema()),
        (TASK_RECURRENCE_RESUME_TOOL, "Resume a paused recurrence using its missed-run policy.", recurrence_fenced_schema()),
        (TASK_RECURRENCE_SKIP_NEXT_TOOL, "Skip the next exact recurring slot.", recurrence_fenced_schema()),
        (TASK_RECURRENCE_END_TOOL, "End all future recurrence without changing active work.", recurrence_fenced_schema()),
        (TASK_RUN_RECURRENCE_NOW_TOOL, "Create and start one extra occurrence now without advancing the next scheduled run. This is unavailable while another occurrence is nonterminal.", recurrence_fenced_schema()),
        (TASK_DELEGATE_TOOL, "Atomically capture and authorize autonomous Work. Preserve the human's requested outcome, scope, and delivery depth in the title and description; do not add optional deliverables or research requirements. Projects are optional: use project.kind none and proceed when the user does not choose one; do not ask for or create a project only to delegate. Use project.kind existing only with an exact project_id returned by project.list. Use complexity_hint only when execution_intent is omitted and planning is needed; execution_intent supplies its own complexity and skips planning.", json!({"type":"object","properties":{"title":{"type":"string","minLength":1,"maxLength":200},"description":{"type":"string","minLength":1,"maxLength":20000},"project":{"description":"Explicit project placement. Choose none for a projectless task, or existing with an exact project_id returned by project.list.","oneOf":[{"type":"object","properties":{"kind":{"type":"string","enum":["none"]}},"required":["kind"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","enum":["existing"]},"project_id":{"type":"string","minLength":1,"maxLength":255}},"required":["kind","project_id"],"additionalProperties":false}]},"executor_agent_id":{"type":"string","minLength":1,"maxLength":255},"cwd_override":{"type":"string","minLength":1,"maxLength":4096},"complexity_hint":{"type":"string","description":"Planner selection hint. Omit when execution_intent is provided.","enum":["simple","medium","difficult"]},"execution_intent":{"type":"object","description":"Complete execution contract that skips planning. Omit complexity_hint when provided.","properties":{"request_markdown":{"type":"string","minLength":1,"maxLength":20000},"complexity":{"type":"string","enum":["simple","medium","difficult"]},"criteria":{"type":"array","minItems":1,"maxItems":32,"items":{"type":"object","properties":{"description":{"type":"string","minLength":1,"maxLength":4000},"expected_evidence":{"type":"string","maxLength":4000}},"required":["description"],"additionalProperties":false}},"execution_plan_markdown":{"type":"string","maxLength":20000}},"required":["request_markdown","complexity","criteria"],"additionalProperties":false}},"required":["title","description","project"],"additionalProperties":false})),
        (TASK_ANSWER_TOOL, "Answer the exact active_gate returned by task.list. This resolves only that occurrence and never edits future recurring authority.", json!({"type":"object","properties":{"task_id":{"type":"string"},"gate_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"answer_markdown":{"type":"string","minLength":1,"maxLength":20000},"approval_decision":{"type":"string","enum":["approved","declined"]}},"required":["task_id","gate_id","expected_revision","expected_generation","answer_markdown"],"additionalProperties":false})),
        (TASK_RETRY_TOOL, "Retry the explicitly named eligible Recovery gate.", json!({"type":"object","properties":{"task_id":{"type":"string"},"gate_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"retry_note":{"type":"string","maxLength":4000}},"required":["task_id","gate_id","expected_revision","expected_generation"],"additionalProperties":false})),
        (TASK_CANCEL_TOOL, "Cancel a nonterminal task and fence active work.", json!({"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"reason":{"type":"string","maxLength":4000}},"required":["task_id","expected_revision","expected_generation"],"additionalProperties":false})),
        (TASK_REOPEN_TOOL, "Reopen a completed task into Queue with new direction.", json!({"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"feedback_markdown":{"type":"string","minLength":1,"maxLength":20000},"request_markdown":{"type":"string","maxLength":20000},"replacement_criteria":{"type":"array","items":{"type":"object","properties":{"description":{"type":"string","minLength":1},"expected_evidence":{"type":"string"}},"required":["description"],"additionalProperties":false}},"complexity":{"type":"string","enum":["simple","medium","difficult"]}},"required":["task_id","expected_revision","expected_generation","feedback_markdown"],"additionalProperties":false})),
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

pub(crate) fn task_submit_plan_tool_spec() -> Result<ToolSpec, noema_capabilities::ToolContractError>
{
    ToolSpec::new(
        TASK_SUBMIT_PLAN_TOOL,
        "Submit one complete immutable execution contract for the task planner.",
        json!({"type":"object","properties":{"request_markdown":{"type":"string","minLength":1,"maxLength":20000},"complexity":{"type":"string","enum":["simple","medium","difficult"]},"criteria":{"type":"array","minItems":1,"maxItems":32,"items":{"type":"object","properties":{"description":{"type":"string","minLength":1,"maxLength":4000},"expected_evidence":{"type":"string","maxLength":4000}},"required":["description"],"additionalProperties":false}},"execution_plan_markdown":{"type":"string","minLength":1,"maxLength":20000}},"required":["request_markdown","complexity","criteria","execution_plan_markdown"],"additionalProperties":false}),
    )
}

pub(crate) fn task_report_blocked_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_REPORT_BLOCKED_TOOL,
        "Open one focused clarification or approval gate and stop at a safe boundary. Keep the copy brief. Suggested answers render as separate controls, so do not repeat them in the question or context.",
        json!({"type":"object","properties":{"gate_kind":{"type":"string","enum":["clarification","approval"]},"question":{"type":"string","description":"One brief standalone question. When suggested_answers is non-empty, do not quote, enumerate, or otherwise repeat those choices here.","minLength":1,"maxLength":4000},"context_markdown":{"type":"string","description":"Optional brief context needed to answer. Do not restate the question or suggested_answers.","maxLength":20000},"suggested_answers":{"type":"array","description":"Optional concise direct answers rendered as separate controls. Do not duplicate them in question or context_markdown.","maxItems":8,"items":{"type":"string","minLength":1,"maxLength":1000}}},"required":["gate_kind","question"],"additionalProperties":false}),
    )
}

pub(crate) fn task_submit_result_tool_spec(
    criterion_ids: &[String],
) -> Result<ToolSpec, noema_capabilities::ToolContractError> {
    let criterion_count = criterion_ids.len();
    ToolSpec::new(
        TASK_SUBMIT_RESULT_TOOL,
        "Submit one complete user-facing executor result with separate evidence for every contract criterion.",
        json!({"type":"object","properties":{"summary":{"type":"string","minLength":1,"maxLength":4000},"result_markdown":{"type":"string","description":"Complete user-facing result for this submission. Match the requested delivery depth, incorporate any revisions into the full work, and avoid duplicating exhaustive criterion evidence.","minLength":1,"maxLength":100000},"criteria":{"type":"array","description":"Structured validation evidence for the reviewer; it identifies evidence but does not replace required user-facing work.","minItems":criterion_count,"maxItems":criterion_count,"items":{"type":"object","properties":{"criterion_id":{"type":"string","enum":criterion_ids},"evidence_markdown":{"type":"string","minLength":1,"maxLength":20000}},"required":["criterion_id","evidence_markdown"],"additionalProperties":false}},"artifact_ids":{"type":"array","description":"Complete artifact manifest for this submission; prior submission artifacts are not inherited implicitly.","maxItems":100,"items":{"type":"string","minLength":1,"maxLength":200}}},"required":["summary","result_markdown","criteria","artifact_ids"],"additionalProperties":false}),
    )
}

pub(crate) fn task_submit_review_tool_spec(
    criterion_ids: &[String],
) -> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_SUBMIT_REVIEW_TOOL,
        "Submit one typed review verdict and one outcome for every criterion.",
        task_review_schema(criterion_ids),
    )
}

fn task_review_schema(criterion_ids: &[String]) -> Value {
    let criterion_count = criterion_ids.len();
    json!({"type":"object","properties":{"overall_feedback":{"type":"string","minLength":1,"maxLength":20000},"criteria":{"type":"array","minItems":criterion_count,"maxItems":criterion_count,"items":{"type":"object","properties":{"criterion_id":{"type":"string","enum":criterion_ids},"outcome":{"type":"string","enum":["pass","fail","uncertain"]},"evidence_markdown":{"type":"string","maxLength":20000},"feedback":{"type":"string","maxLength":20000}},"required":["criterion_id","outcome"],"additionalProperties":false}},"decision":{"oneOf":[{"type":"object","properties":{"verdict":{"type":"string","enum":["approve"]}},"required":["verdict"],"additionalProperties":false},{"type":"object","properties":{"verdict":{"type":"string","enum":["request_changes"]}},"required":["verdict"],"additionalProperties":false},{"type":"object","properties":{"verdict":{"type":"string","enum":["needs_human"]},"human_gate_kind":{"type":"string","enum":["clarification","approval"]},"human_question":{"type":"string","minLength":1,"maxLength":4000}},"required":["verdict","human_gate_kind","human_question"],"additionalProperties":false}]}},"required":["overall_feedback","criteria","decision"],"additionalProperties":false})
}

/// Build the background-role task inspection contract. The runtime supplies
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_result_schema_requires_artifact_array() {
        let schema = task_submit_result_tool_spec(&[
            "criterion:one".to_string(),
            "criterion:two".to_string(),
        ])
        .expect("executor result tool");
        let criteria = &schema.input_schema.as_value()["properties"]["criteria"];

        assert_eq!(
            schema.input_schema.as_value()["required"],
            json!(["summary", "result_markdown", "criteria", "artifact_ids"])
        );
        assert_eq!(criteria["minItems"], 2);
        assert_eq!(criteria["maxItems"], 2);
        assert_eq!(
            criteria["items"]["properties"]["criterion_id"]["enum"],
            json!(["criterion:one", "criterion:two"])
        );
        assert_eq!(
            schema.input_schema.as_value()["properties"]["artifact_ids"]["type"],
            "array"
        );
    }

    #[test]
    fn reviewer_schema_nests_conditional_decision_under_root_object() {
        let schema = task_submit_review_tool_spec(&[
            "criterion:one".to_string(),
            "criterion:two".to_string(),
        ])
        .expect("review tool");
        let schema = schema.input_schema.as_value();
        let criteria = &schema["properties"]["criteria"];
        let decisions = schema["properties"]["decision"]["oneOf"]
            .as_array()
            .expect("review decisions");

        assert_eq!(schema["type"], "object");
        assert_eq!(criteria["minItems"], 2);
        assert_eq!(criteria["maxItems"], 2);
        assert_eq!(
            criteria["items"]["properties"]["criterion_id"]["enum"],
            json!(["criterion:one", "criterion:two"])
        );
        assert!(schema.get("oneOf").is_none());
        assert_eq!(
            schema["required"],
            json!(["overall_feedback", "criteria", "decision"])
        );
        assert_eq!(
            decisions[0]["properties"]["verdict"]["enum"],
            json!(["approve"])
        );
        assert!(decisions[0]["properties"].get("human_gate_kind").is_none());
        assert!(decisions[1]["properties"].get("human_gate_kind").is_none());
        assert_eq!(
            decisions[2]["required"],
            json!(["verdict", "human_gate_kind", "human_question"])
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
