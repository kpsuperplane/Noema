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
    TASK_LIST_TOOL, TASK_QUEUE_TOOL, TASK_REOPEN_TOOL, TASK_REPORT_BLOCKED_TOOL, TASK_RETRY_TOOL,
    TASK_SUBMIT_PLAN_TOOL, TASK_SUBMIT_RESULT_TOOL, TASK_SUBMIT_REVIEW_TOOL, TASK_UPDATE_TOOL,
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
} }

arguments! { DelegateArguments {
    title: String,
    description: String,
    project: DelegateProjectArguments,
    #[serde(default)]
    complexity_hint: Option<TaskComplexity>,
    #[serde(default)]
    execution_intent: Option<ExecutionIntentArguments>,
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
} }

arguments! { ProjectUpdateArguments {
    #[serde(flatten)]
    precondition: ProjectPreconditionArguments,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
} }

arguments! { ProjectPreconditionArguments {
    project_id: String,
    expected_revision: u64,
} }

pub(crate) fn primary_task_tool_specs()
-> Result<Vec<ToolSpec>, noema_capabilities::ToolContractError> {
    [
        (TASK_CAPTURE_TOOL, "Capture work in Inbox without authorizing execution.", json!({"type":"object","properties":{"title":{"type":"string","minLength":1,"maxLength":200},"description":{"type":"string","maxLength":20000},"project_id":{"type":"string","minLength":1,"maxLength":255}},"required":["title"],"additionalProperties":false})),
        (TASK_LIST_TOOL, "List bounded owner-authorized Work task summaries.", json!({"type":"object","properties":{"project_id":{"type":"string","minLength":1},"stage_behavior":{"type":"string","enum":["intake","dispatch","active","human_gate","terminal_success","terminal_cancelled"]},"attention_only":{"type":"boolean"},"limit":{"type":"integer","minimum":1,"maximum":100}},"additionalProperties":false})),
        (TASK_UPDATE_TOOL, "Update an Inbox task's capture fields with revision and generation fences.", json!({"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"title":{"type":"string","minLength":1,"maxLength":200},"description":{"type":"string","maxLength":20000},"project_id":{"type":"string","minLength":1},"clear_project":{"type":"boolean"}},"required":["task_id","expected_revision","expected_generation"],"additionalProperties":false})),
        (TASK_QUEUE_TOOL, "Authorize an Inbox task for planning/execution.", task_fenced_schema()),
        (TASK_DELEGATE_TOOL, "Atomically capture and authorize autonomous Work. Projects are optional: use project.kind none and proceed when the user does not choose one; do not ask for or create a project only to delegate. Use project.kind existing only with an exact project_id returned by project.list. Use complexity_hint only when execution_intent is omitted and planning is needed; execution_intent supplies its own complexity and skips planning.", json!({"type":"object","properties":{"title":{"type":"string","minLength":1,"maxLength":200},"description":{"type":"string","minLength":1,"maxLength":20000},"project":{"description":"Explicit project placement. Choose none for a projectless task, or existing with an exact project_id returned by project.list.","oneOf":[{"type":"object","properties":{"kind":{"type":"string","enum":["none"]}},"required":["kind"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","enum":["existing"]},"project_id":{"type":"string","minLength":1,"maxLength":255}},"required":["kind","project_id"],"additionalProperties":false}]},"complexity_hint":{"type":"string","description":"Planner selection hint. Omit when execution_intent is provided.","enum":["simple","medium","difficult"]},"execution_intent":{"type":"object","description":"Complete execution contract that skips planning. Omit complexity_hint when provided.","properties":{"request_markdown":{"type":"string","minLength":1,"maxLength":20000},"complexity":{"type":"string","enum":["simple","medium","difficult"]},"criteria":{"type":"array","minItems":1,"maxItems":32,"items":{"type":"object","properties":{"description":{"type":"string","minLength":1,"maxLength":4000},"expected_evidence":{"type":"string","maxLength":4000}},"required":["description"],"additionalProperties":false}},"execution_plan_markdown":{"type":"string","maxLength":20000}},"required":["request_markdown","complexity","criteria"],"additionalProperties":false}},"required":["title","description","project"],"additionalProperties":false})),
        (TASK_ANSWER_TOOL, "Answer the explicitly named clarification or approval gate.", json!({"type":"object","properties":{"task_id":{"type":"string"},"gate_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"answer_markdown":{"type":"string","minLength":1,"maxLength":20000},"approval_decision":{"type":"string","enum":["approved","declined"]}},"required":["task_id","gate_id","expected_revision","expected_generation","answer_markdown"],"additionalProperties":false})),
        (TASK_RETRY_TOOL, "Retry the explicitly named eligible Recovery gate.", json!({"type":"object","properties":{"task_id":{"type":"string"},"gate_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"retry_note":{"type":"string","maxLength":4000}},"required":["task_id","gate_id","expected_revision","expected_generation"],"additionalProperties":false})),
        (TASK_CANCEL_TOOL, "Cancel a nonterminal task and fence active work.", json!({"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"reason":{"type":"string","maxLength":4000}},"required":["task_id","expected_revision","expected_generation"],"additionalProperties":false})),
        (TASK_REOPEN_TOOL, "Reopen a completed task into Queue with new direction.", json!({"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"feedback_markdown":{"type":"string","minLength":1,"maxLength":20000},"request_markdown":{"type":"string","maxLength":20000},"replacement_criteria":{"type":"array","items":{"type":"object","properties":{"description":{"type":"string","minLength":1},"expected_evidence":{"type":"string"}},"required":["description"],"additionalProperties":false}},"complexity":{"type":"string","enum":["simple","medium","difficult"]}},"required":["task_id","expected_revision","expected_generation","feedback_markdown"],"additionalProperties":false})),
        (PROJECT_CREATE_TOOL, "Create a Personal project container.", json!({"type":"object","properties":{"name":{"type":"string","minLength":1,"maxLength":200},"description":{"type":"string","maxLength":20000}},"required":["name"],"additionalProperties":false})),
        (PROJECT_LIST_TOOL, "List bounded Personal projects.", json!({"type":"object","properties":{"include_archived":{"type":"boolean"},"limit":{"type":"integer","minimum":1,"maximum":100}},"additionalProperties":false})),
        (PROJECT_UPDATE_TOOL, "Update a project with its revision fence.", json!({"type":"object","properties":{"project_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"name":{"type":"string","minLength":1},"description":{"type":"string"}},"required":["project_id","expected_revision"],"additionalProperties":false})),
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
        "Open one focused clarification or approval gate and stop at a safe boundary.",
        json!({"type":"object","properties":{"gate_kind":{"type":"string","enum":["clarification","approval"]},"question":{"type":"string","minLength":1,"maxLength":4000},"context_markdown":{"type":"string","maxLength":20000},"suggested_answers":{"type":"array","maxItems":8,"items":{"type":"string","minLength":1,"maxLength":1000}}},"required":["gate_kind","question"],"additionalProperties":false}),
    )
}

pub(crate) fn task_submit_result_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_SUBMIT_RESULT_TOOL,
        "Submit one complete executor result with evidence for every contract criterion.",
        json!({"type":"object","properties":{"summary":{"type":"string","minLength":1,"maxLength":4000},"result_markdown":{"type":"string","minLength":1,"maxLength":100000},"criteria":{"type":"array","minItems":1,"items":{"type":"object","properties":{"criterion_id":{"type":"string","minLength":1,"maxLength":200},"evidence_markdown":{"type":"string","minLength":1,"maxLength":20000}},"required":["criterion_id","evidence_markdown"],"additionalProperties":false}},"artifact_ids":{"type":"array","maxItems":100,"items":{"type":"string","minLength":1,"maxLength":200}}},"required":["summary","result_markdown","criteria","artifact_ids"],"additionalProperties":false}),
    )
}

pub(crate) fn task_submit_review_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_SUBMIT_REVIEW_TOOL,
        "Submit one typed review verdict and one outcome for every criterion.",
        task_review_schema(),
    )
}

fn task_review_schema() -> Value {
    let review = json!({"type":"object","properties":{"overall_verdict":{"type":"string"},"overall_feedback":{"type":"string","minLength":1,"maxLength":20000},"criteria":{"type":"array","minItems":1,"items":{"type":"object","properties":{"criterion_id":{"type":"string","minLength":1,"maxLength":200},"outcome":{"type":"string","enum":["pass","fail","uncertain"]},"evidence_markdown":{"type":"string","maxLength":20000},"feedback":{"type":"string","maxLength":20000}},"required":["criterion_id","outcome"],"additionalProperties":false}}},"required":["overall_verdict","overall_feedback","criteria"],"additionalProperties":false});
    let mut approve = review.clone();
    approve["properties"]["overall_verdict"]["enum"] = json!(["approve"]);
    let mut request_changes = review.clone();
    request_changes["properties"]["overall_verdict"]["enum"] = json!(["request_changes"]);
    let mut needs_human = review;
    needs_human["properties"]["overall_verdict"]["enum"] = json!(["needs_human"]);
    needs_human["properties"]["human_gate_kind"] =
        json!({"type":"string","enum":["clarification","approval"]});
    needs_human["properties"]["human_question"] =
        json!({"type":"string","minLength":1,"maxLength":4000});
    needs_human["required"] = json!([
        "overall_verdict",
        "overall_feedback",
        "criteria",
        "human_gate_kind",
        "human_question"
    ]);
    json!({"type":"object","oneOf":[approve,request_changes,needs_human]})
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
        let schema = task_submit_result_tool_spec().expect("executor result tool");

        assert_eq!(
            schema.input_schema.as_value()["required"],
            json!(["summary", "result_markdown", "criteria", "artifact_ids"])
        );
        assert_eq!(
            schema.input_schema.as_value()["properties"]["artifact_ids"]["type"],
            "array"
        );
    }

    #[test]
    fn reviewer_schema_only_exposes_gate_fields_for_needs_human() {
        let schema = task_submit_review_tool_spec().expect("review tool");
        let variants = schema.input_schema.as_value()["oneOf"]
            .as_array()
            .expect("review variants");

        for variant in &variants[..2] {
            assert!(variant["properties"].get("human_gate_kind").is_none());
            assert!(variant["properties"].get("human_question").is_none());
        }
        assert_eq!(
            variants[2]["properties"]["overall_verdict"]["enum"],
            json!(["needs_human"])
        );
        assert_eq!(
            variants[2]["required"],
            json!([
                "overall_verdict",
                "overall_feedback",
                "criteria",
                "human_gate_kind",
                "human_question"
            ])
        );
    }
}
