use noema_capabilities::{ToolContractError, ToolSpec};
use noema_store::NoemaStore;

use noema_artifacts::{
    AppendLocalArtifactVersionRequest, ArtifactDomainError, ArtifactOperationError,
    ArtifactOperationsHandle, ArtifactOwnerRef, ArtifactSource, CreateLocalArtifactRequest,
    artifact_download_url, safe_artifact_filename,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) const ARTIFACT_CREATE_LOCAL_FILE_TOOL: &str = "artifact.create_local_file";
const MAX_ARTIFACT_TITLE_CHARS: usize = 160;
const MAX_ARTIFACT_DESCRIPTION_CHARS: usize = 1_000;
const MAX_ARTIFACT_KIND_CHARS: usize = 80;
const MAX_ARTIFACT_VERSIONS: usize = 5;
const MAX_ARTIFACT_VERSION_CHARS: usize = 200_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ArtifactToolRuntimeContext {
    pub conversation_id: String,
    pub turn_id: String,
    pub user_item_id: String,
    pub created_by_actor_id: String,
    pub task_id: Option<String>,
    pub task_run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ArtifactToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ArtifactToolError {
    #[error("{0}")]
    InvalidArguments(String),
    #[error(transparent)]
    Path(#[from] ArtifactDomainError),
    #[error(transparent)]
    Operation(#[from] ArtifactOperationError),
    #[error(transparent)]
    Store(#[from] noema_store::StoreError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateLocalFileArtifactArguments {
    title: String,
    #[serde(default)]
    description: Option<String>,
    artifact_kind: String,
    filename: String,
    #[serde(default)]
    media_type: Option<String>,
    versions: Vec<CreateLocalFileArtifactVersionArguments>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateLocalFileArtifactVersionArguments {
    #[serde(default)]
    title: Option<String>,
    content: String,
}

pub(super) fn is_artifact_create_local_file_tool(name: &str) -> bool {
    name == ARTIFACT_CREATE_LOCAL_FILE_TOOL
}

pub(super) fn artifact_create_local_file_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        ARTIFACT_CREATE_LOCAL_FILE_TOOL,
        "Create a durable local file artifact owned by the current conversation or task execution scope, optionally with multiple immutable text versions.",
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": MAX_ARTIFACT_TITLE_CHARS,
                    "pattern": ".*\\S.*",
                    "description": "Human-readable artifact title."
                },
                "description": {
                    "type": "string",
                    "maxLength": MAX_ARTIFACT_DESCRIPTION_CHARS,
                    "description": "Optional short description of the artifact."
                },
                "artifact_kind": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": MAX_ARTIFACT_KIND_CHARS,
                    "pattern": ".*\\S.*",
                    "description": "Product kind label such as document, report, code, data, image, or note."
                },
                "filename": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 160,
                    "description": "Safe single-segment filename used for each local file version."
                },
                "media_type": {
                    "type": "string",
                    "maxLength": 120,
                    "description": "Optional media type such as text/markdown, text/plain, application/json, or text/csv."
                },
                "versions": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": MAX_ARTIFACT_VERSIONS,
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": {
                                "type": "string",
                                "maxLength": MAX_ARTIFACT_TITLE_CHARS,
                                "description": "Optional label for this version."
                            },
                            "content": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": MAX_ARTIFACT_VERSION_CHARS,
                                "description": "Complete UTF-8 text content for this artifact version."
                            }
                        },
                        "required": ["content"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["title", "artifact_kind", "filename", "versions"],
            "additionalProperties": false
        }),
    )
}

pub(super) async fn execute_artifact_create_local_file(
    store: &NoemaStore,
    artifact_operations: &ArtifactOperationsHandle,
    context: &ArtifactToolRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> ArtifactToolResult {
    match execute_artifact_create_local_file_inner(store, artifact_operations, context, payload)
        .await
    {
        Ok(payload) => ArtifactToolResult {
            call_id,
            name: ARTIFACT_CREATE_LOCAL_FILE_TOOL.to_string(),
            success: true,
            payload,
        },
        Err(error) => ArtifactToolResult {
            call_id,
            name: ARTIFACT_CREATE_LOCAL_FILE_TOOL.to_string(),
            success: false,
            payload: json!({
                "error": safe_error_message(&error),
            }),
        },
    }
}

async fn execute_artifact_create_local_file_inner(
    store: &NoemaStore,
    artifact_operations: &ArtifactOperationsHandle,
    context: &ArtifactToolRuntimeContext,
    payload: &Value,
) -> Result<Value, ArtifactToolError> {
    let arguments = parse_arguments(payload)?;
    let task = match (&context.task_id, &context.task_run_id) {
        (Some(task_id), Some(run_id)) => {
            let task = store.get_task(task_id).await?.ok_or_else(|| {
                ArtifactToolError::InvalidArguments("task context is unavailable".to_string())
            })?;
            let run = store.get_agent_run(run_id).await?.ok_or_else(|| {
                ArtifactToolError::InvalidArguments("task run context is unavailable".to_string())
            })?;
            if run.task_id != task.task_id
                || run.agent_id != context.created_by_actor_id
                || run.run_kind != noema_tasks::RunKind::Executor
                || run.status != noema_tasks::RunStatus::Running
                || run.cancellation_requested
                || !matches!(
                    task.status,
                    noema_tasks::TaskStatus::Executing | noema_tasks::TaskStatus::RevisionRequested
                )
            {
                return Err(ArtifactToolError::InvalidArguments(
                    "task artifact context does not match the active executor".to_string(),
                ));
            }
            Some(task)
        }
        (None, None) => None,
        _ => {
            return Err(ArtifactToolError::InvalidArguments(
                "task artifact context is incomplete".to_string(),
            ));
        }
    };
    let source = task.as_ref().map_or_else(
        || ArtifactSource {
            conversation_id: Some(context.conversation_id.clone()),
            turn_id: Some(context.turn_id.clone()),
            item_id: Some(context.user_item_id.clone()),
        },
        |task| ArtifactSource {
            conversation_id: task.source.conversation_id.clone(),
            turn_id: task.source.turn_id.clone(),
            item_id: task.source.item_id.clone(),
        },
    );
    let mut versions = arguments.versions.into_iter();
    let first_version = versions.next().ok_or_else(|| {
        ArtifactToolError::InvalidArguments("versions must include at least one item".to_string())
    })?;
    let owner = task.as_ref().map_or_else(
        || ArtifactOwnerRef::conversation(&context.conversation_id),
        |task| ArtifactOwnerRef::task(&task.task_id),
    );
    let metadata = task.as_ref().map_or_else(
        || json!({"created_by_tool": ARTIFACT_CREATE_LOCAL_FILE_TOOL}),
        |task| {
            json!({
                "created_by_tool": ARTIFACT_CREATE_LOCAL_FILE_TOOL,
                "task_id": task.task_id,
                "task_run_id": context.task_run_id,
            })
        },
    );
    let artifact = artifact_operations
        .create_local_file(CreateLocalArtifactRequest {
            owner,
            title: arguments.title.clone(),
            description: arguments.description.clone(),
            artifact_kind: arguments.artifact_kind.clone(),
            filename: arguments.filename.clone(),
            bytes: first_version.content.into_bytes(),
            media_type: arguments.media_type.clone(),
            created_by_actor_id: context.created_by_actor_id.clone(),
            source: source.clone(),
            metadata,
        })
        .await?;

    let artifact_id = artifact.artifact.artifact_id.clone();
    for version in versions {
        artifact_operations
            .append_local_file_version(AppendLocalArtifactVersionRequest {
                artifact_id: artifact_id.clone(),
                title: version.title,
                filename: arguments.filename.clone(),
                bytes: version.content.into_bytes(),
                media_type: arguments.media_type.clone(),
                created_by_actor_id: context.created_by_actor_id.clone(),
                source: source.clone(),
                metadata: json!({"created_by_tool": ARTIFACT_CREATE_LOCAL_FILE_TOOL}),
            })
            .await?;
    }

    let artifact = store.get_artifact(&artifact_id).await?.ok_or(
        noema_store::StoreError::ArtifactNotFound {
            artifact_id: artifact_id.clone(),
        },
    )?;

    Ok(json!({
        "artifact_id": artifact.artifact.artifact_id,
        "title": artifact.artifact.title,
        "artifact_kind": artifact.artifact.artifact_kind,
        "storage_kind": artifact.artifact.storage_kind.as_str(),
        "current_version_id": artifact.current_version.artifact_version_id,
        "current_version_index": artifact.current_version.version_index,
        "download_url": artifact_download_url(&artifact.current_version.artifact_version_id),
        "media_type": artifact.current_version.media_type,
        "versions": artifact.versions.iter().map(|version| {
            json!({
                "artifact_version_id": version.artifact_version_id,
                "version_index": version.version_index,
                "download_url": artifact_download_url(&version.artifact_version_id),
                "media_type": version.media_type,
                "byte_size": version.byte_size,
                "content_sha256": version.content_sha256,
            })
        }).collect::<Vec<_>>(),
    }))
}

fn parse_arguments(payload: &Value) -> Result<CreateLocalFileArtifactArguments, ArtifactToolError> {
    let argument_value = if let Some(arguments) = payload.get("arguments") {
        reject_nested_outer_fields(payload)?;
        arguments.clone()
    } else {
        payload.clone()
    };
    let mut arguments: CreateLocalFileArtifactArguments = serde_json::from_value(argument_value)
        .map_err(|error| {
            ArtifactToolError::InvalidArguments(format!("invalid arguments: {error}"))
        })?;

    arguments.title = trim_required(arguments.title, "title", MAX_ARTIFACT_TITLE_CHARS)?;
    arguments.description = trim_optional(
        arguments.description,
        "description",
        MAX_ARTIFACT_DESCRIPTION_CHARS,
    )?;
    arguments.artifact_kind = trim_required(
        arguments.artifact_kind,
        "artifact_kind",
        MAX_ARTIFACT_KIND_CHARS,
    )?;
    arguments.media_type = trim_optional(arguments.media_type, "media_type", 120)?;
    if arguments.versions.is_empty() {
        return Err(ArtifactToolError::InvalidArguments(
            "versions must include at least one item".to_string(),
        ));
    }
    if arguments.versions.len() > MAX_ARTIFACT_VERSIONS {
        return Err(ArtifactToolError::InvalidArguments(format!(
            "versions must include {MAX_ARTIFACT_VERSIONS} items or fewer"
        )));
    }
    for version in &mut arguments.versions {
        version.title = trim_optional(
            version.title.take(),
            "version.title",
            MAX_ARTIFACT_TITLE_CHARS,
        )?;
        if version.content.is_empty() {
            return Err(ArtifactToolError::InvalidArguments(
                "version content is required".to_string(),
            ));
        }
        if version.content.chars().count() > MAX_ARTIFACT_VERSION_CHARS {
            return Err(ArtifactToolError::InvalidArguments(format!(
                "version content must be {MAX_ARTIFACT_VERSION_CHARS} characters or fewer"
            )));
        }
    }
    safe_artifact_filename(&arguments.filename)?;
    Ok(arguments)
}

fn reject_nested_outer_fields(payload: &Value) -> Result<(), ArtifactToolError> {
    let Some(object) = payload.as_object() else {
        return Ok(());
    };
    if object.keys().all(|key| key == "arguments") {
        Ok(())
    } else {
        Err(ArtifactToolError::InvalidArguments(
            "nested arguments payload cannot include outer fields".to_string(),
        ))
    }
}

fn trim_required(
    value: String,
    field: &str,
    max_chars: usize,
) -> Result<String, ArtifactToolError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(ArtifactToolError::InvalidArguments(format!(
            "{field} is required"
        )));
    }
    if trimmed.chars().count() > max_chars {
        return Err(ArtifactToolError::InvalidArguments(format!(
            "{field} must be {max_chars} characters or fewer"
        )));
    }
    Ok(trimmed)
}

fn trim_optional(
    value: Option<String>,
    field: &str,
    max_chars: usize,
) -> Result<Option<String>, ArtifactToolError> {
    value
        .map(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if trimmed.chars().count() > max_chars {
                return Err(ArtifactToolError::InvalidArguments(format!(
                    "{field} must be {max_chars} characters or fewer"
                )));
            }
            Ok(Some(trimmed))
        })
        .transpose()
        .map(Option::flatten)
}

fn safe_error_message(error: &ArtifactToolError) -> String {
    match error {
        ArtifactToolError::InvalidArguments(message) => message.clone(),
        ArtifactToolError::Path(_) => "artifact path validation failed".to_string(),
        ArtifactToolError::Operation(_) => "artifact write failed".to_string(),
        ArtifactToolError::Store(_) => "artifact metadata update failed".to_string(),
    }
}
