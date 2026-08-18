//! Role-fenced access to governed artifacts owned by one Task.

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
        "Read bounded UTF-8 content from an artifact version owned by the current Task.",
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
    let (artifact, version) = task_artifact(store, &envelope, &arguments).await?;

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

async fn task_artifact(
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
