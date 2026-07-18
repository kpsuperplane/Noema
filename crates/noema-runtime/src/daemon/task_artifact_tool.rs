//! Role-fenced access to governed artifacts linked to one task contract.

use serde::Deserialize;
use serde_json::{Value, json};

use noema_capabilities::ToolSpec;
use noema_store::NoemaStore;

pub(crate) const TASK_READ_ARTIFACT_TOOL: &str = "task.read_artifact";
const MAX_ARTIFACT_TEXT_CHARS: usize = 120_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskArtifactReadContext {
    pub task_id: String,
    pub run_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArtifactArguments {
    artifact_id: String,
    #[serde(default)]
    artifact_version_id: Option<String>,
}

#[must_use]
pub(crate) fn is_task_read_artifact_tool(name: &str) -> bool {
    name == TASK_READ_ARTIFACT_TOOL
}

pub(crate) fn task_read_artifact_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_READ_ARTIFACT_TOOL,
        "Read bounded UTF-8 content from an artifact version linked to the current task contract. Reviewers are restricted to the submitted manifest; Executors are restricted to task-owned artifacts created by an Executor run on the same contract revision.",
        json!({
            "type": "object",
            "properties": {
                "artifact_id": {"type": "string", "minLength": 1, "maxLength": 200},
                "artifact_version_id": {"type": "string", "minLength": 1, "maxLength": 200}
            },
            "required": ["artifact_id"],
            "additionalProperties": false
        }),
    )
}

pub(crate) async fn execute_task_read_artifact(
    store: &NoemaStore,
    artifact_operations: &noema_artifacts::ArtifactOperationsHandle,
    context: &TaskArtifactReadContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments: ReadArtifactArguments = serde_json::from_value(payload.clone())
        .map_err(|error| format!("invalid task artifact arguments: {error}"))?;
    let envelope = store
        .get_work_run_execution_context(&context.run_id)
        .await
        .map_err(|error| error.to_string())?
        .filter(|envelope| {
            envelope.task.task_id.as_str() == context.task_id
                && matches!(
                    envelope.run.run_kind,
                    noema_tasks::RunKind::Executor | noema_tasks::RunKind::Reviewer
                )
                && envelope.run.status == noema_tasks::RunStatus::Running
                && !envelope.run.cancellation_requested
        })
        .ok_or_else(|| "task artifact context is unavailable".to_string())?;
    let (artifact, version) = match envelope.run.run_kind {
        noema_tasks::RunKind::Reviewer => reviewer_artifact(&envelope, &arguments)?,
        noema_tasks::RunKind::Executor => executor_artifact(store, &envelope, &arguments).await?,
        noema_tasks::RunKind::Planner => {
            return Err("Planner cannot read task artifacts".to_string());
        }
    };

    if !matches!(
        version.storage,
        noema_artifacts::ArtifactVersionStorage::LocalFile { .. }
    ) {
        return Err("external artifact content is unavailable to the task role".to_string());
    }
    let file = artifact_operations
        .read_local_file(noema_artifacts::ReadLocalArtifactRequest {
            artifact: artifact.clone(),
            version: version.clone(),
        })
        .await
        .map_err(|_| "artifact content is unavailable".to_string())?;
    let text = String::from_utf8(file.bytes)
        .map_err(|_| "artifact is not valid UTF-8 text".to_string())?;
    if text.chars().count() > MAX_ARTIFACT_TEXT_CHARS {
        return Err("artifact exceeds the task-role text limit".to_string());
    }
    Ok(json!({
        "artifact_id": artifact.artifact_id,
        "artifact_version_id": version.artifact_version_id,
        "title": artifact.title,
        "artifact_kind": artifact.artifact_kind,
        "media_type": version.media_type,
        "content": text,
    }))
}

fn reviewer_artifact(
    envelope: &noema_store::WorkRunExecutionContext,
    arguments: &ReadArtifactArguments,
) -> Result<
    (
        noema_artifacts::ArtifactRecord,
        noema_artifacts::ArtifactVersionRecord,
    ),
    String,
> {
    let submission_id = envelope
        .run
        .triggering_submission_id
        .as_deref()
        .ok_or_else(|| "reviewer run has no submission".to_string())?;
    let submission = envelope
        .latest_submission
        .as_ref()
        .filter(|submission| submission.submission_id == submission_id)
        .ok_or_else(|| "submission is unavailable".to_string())?;
    let linked = submission
        .artifacts
        .iter()
        .find(|linked| {
            linked.artifact.artifact_id == arguments.artifact_id
                && arguments
                    .artifact_version_id
                    .as_deref()
                    .is_none_or(|version_id| linked.version.artifact_version_id == version_id)
        })
        .ok_or_else(|| {
            "artifact version is not linked to the submission under review".to_string()
        })?;
    Ok((linked.artifact.clone(), linked.version.clone()))
}

async fn executor_artifact(
    store: &NoemaStore,
    envelope: &noema_store::WorkRunExecutionContext,
    arguments: &ReadArtifactArguments,
) -> Result<
    (
        noema_artifacts::ArtifactRecord,
        noema_artifacts::ArtifactVersionRecord,
    ),
    String,
> {
    let artifact = store
        .get_artifact(&arguments.artifact_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "artifact is unavailable".to_string())?;
    if artifact.artifact.owner
        != noema_artifacts::ArtifactOwnerRef::task(envelope.task.task_id.as_str())
    {
        return Err("artifact is not owned by the current task".to_string());
    }
    let linked_run_id = artifact
        .artifact
        .metadata
        .get("task_run_id")
        .and_then(Value::as_str)
        .filter(|run_id| !run_id.trim().is_empty())
        .ok_or_else(|| "artifact is not linked to an Executor run".to_string())?;
    if linked_run_id != envelope.run.run_id {
        return Err("artifact is not linked to the current Executor run".to_string());
    }
    let linked_run = store
        .get_work_run_record(linked_run_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "artifact-linked Executor run is unavailable".to_string())?;
    if linked_run.task_id != envelope.task.task_id
        || linked_run.task_generation != envelope.run.task_generation
        || linked_run.contract_id != envelope.run.contract_id
        || linked_run.run_kind != noema_tasks::RunKind::Executor
        || linked_run.agent_id != artifact.artifact.created_by_actor_id
    {
        return Err("artifact is not linked to this task contract revision".to_string());
    }
    let version = arguments.artifact_version_id.as_deref().map_or_else(
        || Ok(artifact.current_version.clone()),
        |version_id| {
            artifact
                .versions
                .iter()
                .find(|version| version.artifact_version_id == version_id)
                .cloned()
                .ok_or_else(|| "artifact version is unavailable".to_string())
        },
    )?;
    Ok((artifact.artifact, version))
}

#[cfg(test)]
#[path = "task_artifact_tool/tests.rs"]
mod tests;
