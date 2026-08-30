//! Role-fenced access to governed artifacts owned by one Task.

use std::io::{Seek, SeekFrom, Write};

use serde::Deserialize;
use serde_json::{Value, json};

use noema_capabilities::ToolSpec;
use noema_store::NoemaStore;

pub(crate) const TASK_READ_ARTIFACT_TOOL: &str = "task.read_artifact";
pub(crate) const TASK_LIST_ARTIFACTS_TOOL: &str = "task.list_artifacts";
pub(crate) const TASK_PARSE_ARTIFACT_TOOL: &str = "task.parse_artifact";
const MAX_ARTIFACT_TEXT_CHARS: usize = 120_000;
const DEFAULT_PARSE_CHARS: usize = 12_000;
const MAX_PARSE_CHARS: usize = 20_000;
const MAX_LISTED_ARTIFACTS: i64 = 50;

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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArtifactsArguments {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParseArtifactArguments {
    artifact_id: String,
    #[serde(default)]
    artifact_version_id: Option<String>,
    #[serde(default)]
    max_chars: Option<usize>,
}

#[must_use]
pub(crate) fn is_task_read_artifact_tool(name: &str) -> bool {
    name == TASK_READ_ARTIFACT_TOOL
}

#[must_use]
pub(crate) fn is_task_list_artifacts_tool(name: &str) -> bool {
    name == TASK_LIST_ARTIFACTS_TOOL
}

#[must_use]
pub(crate) fn is_task_parse_artifact_tool(name: &str) -> bool {
    name == TASK_PARSE_ARTIFACT_TOOL
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

pub(crate) fn task_list_artifacts_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_LIST_ARTIFACTS_TOOL,
        "List bounded artifacts owned by the current Task, including exact version, media type, and size.",
        json!({"type": "object", "properties": {}, "additionalProperties": false}),
    )
}

pub(crate) fn task_parse_artifact_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_PARSE_ARTIFACT_TOOL,
        "Parse one supported local artifact owned by the current Task. Preserve its artifact and version IDs when citing extracted content.",
        json!({
            "type": "object",
            "properties": {
                "artifact_id": {"type": "string", "minLength": 1, "maxLength": 200},
                "artifact_version_id": {"type": "string", "minLength": 1, "maxLength": 200},
                "max_chars": {"type": "integer", "minimum": 1000, "maximum": MAX_PARSE_CHARS}
            },
            "required": ["artifact_id"],
            "additionalProperties": false
        }),
    )
}

pub(crate) async fn execute_task_list_artifacts(
    store: &NoemaStore,
    context: &TaskArtifactReadContext,
    payload: &Value,
) -> Result<Value, String> {
    serde_json::from_value::<ListArtifactsArguments>(payload.clone())
        .map_err(|error| format!("invalid task artifact list arguments: {error}"))?;
    let envelope = active_task_envelope(store, context, true).await?;
    let artifacts = store
        .list_artifacts_for_owner(
            noema_artifacts::ArtifactOwnerRef::task(envelope.task.task_id.as_str()),
            MAX_LISTED_ARTIFACTS,
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "artifacts": artifacts.into_iter().map(|artifact| {
            let version = artifact.current_version;
            json!({
                "artifact_id": artifact.artifact.artifact_id,
                "title": artifact.artifact.title,
                "artifact_kind": artifact.artifact.artifact_kind,
                "metadata": artifact.artifact.metadata,
                "artifact_version_id": version.artifact_version_id,
                "version_index": version.version_index,
                "media_type": version.media_type,
                "byte_size": version.byte_size,
            })
        }).collect::<Vec<_>>()
    }))
}

pub(crate) async fn execute_task_parse_artifact(
    store: &NoemaStore,
    artifact_operations: &noema_artifacts::ArtifactOperationsHandle,
    context: &TaskArtifactReadContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments: ParseArtifactArguments = serde_json::from_value(payload.clone())
        .map_err(|error| format!("invalid task artifact parse arguments: {error}"))?;
    let max_chars = arguments.max_chars.unwrap_or(DEFAULT_PARSE_CHARS);
    if !(1000..=MAX_PARSE_CHARS).contains(&max_chars) {
        return Err("artifact parse character limit is invalid".to_string());
    }
    let envelope = active_task_envelope(store, context, false).await?;
    let (artifact, version) = owned_task_artifact(
        store,
        &envelope,
        &arguments.artifact_id,
        arguments.artifact_version_id.as_deref(),
    )
    .await?;
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
    let mut temporary = tempfile::tempfile()
        .map_err(|_| "artifact parser temporary file is unavailable".to_string())?;
    temporary
        .write_all(&file.bytes)
        .and_then(|()| temporary.seek(SeekFrom::Start(0)).map(|_| ()))
        .map_err(|_| "artifact parser temporary file is unavailable".to_string())?;
    let display_path = artifact
        .metadata
        .get("filename")
        .and_then(Value::as_str)
        .unwrap_or(&artifact.title);
    let parsed = crate::file_tools::parse_open_file(
        temporary,
        display_path,
        version.media_type.as_deref(),
        max_chars,
    )
    .await;
    Ok(json!({
        "artifact_id": artifact.artifact_id,
        "artifact_version_id": version.artifact_version_id,
        "version_index": version.version_index,
        "title": artifact.title,
        "artifact_kind": artifact.artifact_kind,
        "metadata": artifact.metadata,
        "media_type": version.media_type,
        "parse": parsed,
    }))
}

pub(crate) async fn execute_task_read_artifact(
    store: &NoemaStore,
    artifact_operations: &noema_artifacts::ArtifactOperationsHandle,
    context: &TaskArtifactReadContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments: ReadArtifactArguments = serde_json::from_value(payload.clone())
        .map_err(|error| format!("invalid task artifact arguments: {error}"))?;
    let envelope = active_task_envelope(store, context, false).await?;
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
        "metadata": artifact.metadata,
        "version_index": version.version_index,
        "version_metadata": version.metadata,
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
    owned_task_artifact(
        store,
        envelope,
        &arguments.artifact_id,
        arguments.artifact_version_id.as_deref(),
    )
    .await
}

async fn active_task_envelope(
    store: &NoemaStore,
    context: &TaskArtifactReadContext,
    allow_planner: bool,
) -> Result<noema_store::WorkRunExecutionContext, String> {
    store
        .get_work_run_execution_context(&context.run_id)
        .await
        .map_err(|error| error.to_string())?
        .filter(|envelope| {
            envelope.task.task_id.as_str() == context.task_id
                && (matches!(
                    envelope.run.run_kind,
                    noema_tasks::RunKind::Executor | noema_tasks::RunKind::Reviewer
                ) || (allow_planner && envelope.run.run_kind == noema_tasks::RunKind::Planner))
                && envelope.run.status == noema_tasks::RunStatus::Running
                && !envelope.run.cancellation_requested
        })
        .ok_or_else(|| "task artifact context is unavailable".to_string())
}

async fn owned_task_artifact(
    store: &NoemaStore,
    envelope: &noema_store::WorkRunExecutionContext,
    artifact_id: &str,
    artifact_version_id: Option<&str>,
) -> Result<
    (
        noema_artifacts::ArtifactRecord,
        noema_artifacts::ArtifactVersionRecord,
    ),
    String,
> {
    owned_task_artifact_for_task(
        store,
        envelope.task.task_id.as_str(),
        artifact_id,
        artifact_version_id,
    )
    .await
}

pub(crate) async fn owned_task_artifact_for_task(
    store: &NoemaStore,
    task_id: &str,
    artifact_id: &str,
    artifact_version_id: Option<&str>,
) -> Result<
    (
        noema_artifacts::ArtifactRecord,
        noema_artifacts::ArtifactVersionRecord,
    ),
    String,
> {
    let artifact = store
        .get_artifact(artifact_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "artifact is unavailable".to_string())?;
    if artifact.artifact.owner != noema_artifacts::ArtifactOwnerRef::task(task_id) {
        return Err("artifact is not owned by the current task".to_string());
    }
    let version = artifact_version_id.map_or_else(
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
