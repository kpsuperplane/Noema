use noema_capabilities::{ToolContractError, ToolSpec};
use noema_store::NoemaStore;

use noema_artifacts::{
    AppendLocalArtifactVersionRequest, ArtifactDomainError, ArtifactOperationError,
    ArtifactOperationsHandle, ArtifactOwnerRef, ArtifactSource, CreateLocalArtifactRequest,
    artifact_download_url, safe_artifact_filename,
};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) const ARTIFACT_CREATE_LOCAL_FILE_TOOL: &str = "artifact.create_local_file";
const MAX_ARTIFACT_TITLE_CHARS: usize = 160;
const MAX_ARTIFACT_DESCRIPTION_CHARS: usize = 1_000;
const MAX_ARTIFACT_KIND_CHARS: usize = 80;
const MAX_ARTIFACT_VERSIONS: usize = 5;
const MAX_ARTIFACT_VERSION_CHARS: usize = 200_000;
const MAX_ARTIFACT_SOURCES: usize = 50;
const MAX_SOURCE_ID_CHARS: usize = 500;
const MAX_SOURCE_VERSION_CHARS: usize = 200;
const MAX_SOURCE_OWNER_CHARS: usize = 160;
const MAX_DISCLOSURE_SCOPE_CHARS: usize = 500;

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
    sources: Vec<CreateLocalFileArtifactSourceArguments>,
    versions: Vec<CreateLocalFileArtifactVersionArguments>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CreateLocalFileArtifactSourceArguments {
    source_id: String,
    source_version: String,
    source_owner: String,
    disclosure_scope: String,
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
                "sources": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": MAX_ARTIFACT_SOURCES,
                    "description": "Exact provider-neutral source records retained with every artifact version.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "source_id": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": MAX_SOURCE_ID_CHARS,
                                "pattern": ".*\\S.*",
                                "description": "Stable source identity, such as an artifact, message, record, file, or URL."
                            },
                            "source_version": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": MAX_SOURCE_VERSION_CHARS,
                                "pattern": ".*\\S.*",
                                "description": "Exact source version, revision, date, or retrieval cutoff."
                            },
                            "source_owner": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": MAX_SOURCE_OWNER_CHARS,
                                "pattern": ".*\\S.*",
                                "description": "Person or organization that owns the source."
                            },
                            "disclosure_scope": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": MAX_DISCLOSURE_SCOPE_CHARS,
                                "pattern": ".*\\S.*",
                                "description": "Audience allowed to receive information from this source."
                            }
                        },
                        "required": ["source_id", "source_version", "source_owner", "disclosure_scope"],
                        "additionalProperties": false
                    }
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
            "required": ["title", "artifact_kind", "filename", "sources", "versions"],
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
            let envelope = store
                .get_work_run_execution_context(run_id)
                .await?
                .ok_or_else(|| {
                    ArtifactToolError::InvalidArguments(
                        "task run context is unavailable".to_string(),
                    )
                })?;
            let task = envelope.task;
            let run = envelope.run;
            if run.task_id.as_str() != task_id
                || run.agent_id != context.created_by_actor_id
                || run.run_kind != noema_tasks::RunKind::Executor
                || run.status != noema_tasks::RunStatus::Running
                || run.cancellation_requested
                || envelope.stage.system_behavior != noema_tasks::WorkflowStageBehavior::Active
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
            conversation_id: task.provenance.conversation_id.clone(),
            turn_id: task.provenance.turn_id.clone(),
            item_id: task.provenance.item_id.clone(),
        },
    );
    let version_metadata = arguments
        .versions
        .iter()
        .map(|version| {
            version_metadata(
                &arguments.sources,
                arguments.media_type.as_deref(),
                &arguments.filename,
                &version.content,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let accessibility = version_metadata
        .iter()
        .enumerate()
        .filter_map(|(index, metadata)| {
            metadata.get("accessibility").map(|report| {
                json!({
                    "version_index": index + 1,
                    "report": report,
                })
            })
        })
        .collect::<Vec<_>>();
    let mut version_metadata = version_metadata.into_iter();
    let mut versions = arguments.versions.into_iter();
    let first_version = versions.next().ok_or_else(|| {
        ArtifactToolError::InvalidArguments("versions must include at least one item".to_string())
    })?;
    version_metadata.next().ok_or_else(|| {
        ArtifactToolError::InvalidArguments("version metadata is unavailable".to_string())
    })?;
    let owner = task.as_ref().map_or_else(
        || ArtifactOwnerRef::conversation(&context.conversation_id),
        |task| ArtifactOwnerRef::task(task.task_id.as_str()),
    );
    let metadata = task.as_ref().map_or_else(
        || {
            json!({
                "created_by_tool": ARTIFACT_CREATE_LOCAL_FILE_TOOL,
                "sources": arguments.sources,
                "version_accessibility": accessibility,
            })
        },
        |task| {
            json!({
                "created_by_tool": ARTIFACT_CREATE_LOCAL_FILE_TOOL,
                "task_id": task.task_id,
                "task_run_id": context.task_run_id,
                "sources": arguments.sources,
                "version_accessibility": accessibility,
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
    for (version, metadata) in versions.zip(version_metadata) {
        artifact_operations
            .append_local_file_version(AppendLocalArtifactVersionRequest {
                artifact_id: artifact_id.clone(),
                title: version.title,
                filename: arguments.filename.clone(),
                bytes: version.content.into_bytes(),
                media_type: arguments.media_type.clone(),
                created_by_actor_id: context.created_by_actor_id.clone(),
                source: source.clone(),
                metadata,
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
        "sources": artifact.artifact.metadata["sources"],
        "accessibility": artifact.artifact.metadata["version_accessibility"],
        "versions": artifact.versions.iter().map(|version| {
            json!({
                "artifact_version_id": version.artifact_version_id,
                "version_index": version.version_index,
                "download_url": artifact_download_url(&version.artifact_version_id),
                "media_type": version.media_type,
                "byte_size": version.byte_size,
                "content_sha256": version.content_sha256,
                "sources": artifact.artifact.metadata["sources"],
                "metadata": version.metadata,
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
    if arguments.sources.is_empty() {
        return Err(ArtifactToolError::InvalidArguments(
            "sources must include at least one item".to_string(),
        ));
    }
    if arguments.sources.len() > MAX_ARTIFACT_SOURCES {
        return Err(ArtifactToolError::InvalidArguments(format!(
            "sources must include {MAX_ARTIFACT_SOURCES} items or fewer"
        )));
    }
    for source in &mut arguments.sources {
        source.source_id = trim_required(
            std::mem::take(&mut source.source_id),
            "source.source_id",
            MAX_SOURCE_ID_CHARS,
        )?;
        source.source_version = trim_required(
            std::mem::take(&mut source.source_version),
            "source.source_version",
            MAX_SOURCE_VERSION_CHARS,
        )?;
        source.source_owner = trim_required(
            std::mem::take(&mut source.source_owner),
            "source.source_owner",
            MAX_SOURCE_OWNER_CHARS,
        )?;
        source.disclosure_scope = trim_required(
            std::mem::take(&mut source.disclosure_scope),
            "source.disclosure_scope",
            MAX_DISCLOSURE_SCOPE_CHARS,
        )?;
    }
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

fn version_metadata(
    sources: &[CreateLocalFileArtifactSourceArguments],
    media_type: Option<&str>,
    filename: &str,
    content: &str,
) -> Result<Value, ArtifactToolError> {
    let mut metadata = json!({
        "created_by_tool": ARTIFACT_CREATE_LOCAL_FILE_TOOL,
        "sources": sources,
    });
    let html_media_type = media_type
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| {
            value.trim().eq_ignore_ascii_case("text/html")
                || value.trim().eq_ignore_ascii_case("application/xhtml+xml")
        });
    let html_filename = std::path::Path::new(filename)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "htm" | "html" | "xhtml"
            )
        });
    if html_media_type || html_filename {
        metadata["accessibility"] = check_accessible_html(content)?;
    }
    Ok(metadata)
}

fn check_accessible_html(content: &str) -> Result<Value, ArtifactToolError> {
    let document = Html::parse_document(content);
    let mut issues = Vec::new();
    if document
        .select(&selector("html"))
        .next()
        .and_then(|element| element.value().attr("lang"))
        .is_none_or(|lang| lang.trim().is_empty())
    {
        issues.push("the document language is missing");
    }
    if !document.select(&selector("title")).any(has_text) {
        issues.push("the document title is missing");
    }
    if document.select(&selector("main")).count() != 1 {
        issues.push("the document must contain one main landmark");
    }
    if document
        .select(&selector("h1"))
        .filter(|element| has_text(*element))
        .count()
        != 1
    {
        issues.push("the document must contain one nonempty h1");
    }
    if document
        .select(&selector("img"))
        .any(|image| image.value().attr("alt").is_none())
    {
        issues.push("each image must have an alt attribute");
    }
    let caption = selector("caption");
    let header = selector("th");
    if document
        .select(&selector("table"))
        .any(|table| !table.select(&caption).any(has_text) || !table.select(&header).any(has_text))
    {
        issues.push("each table must have a caption and a nonempty header cell");
    }
    if document.select(&selector("a[href]")).any(|link| {
        !has_text(link)
            && link
                .value()
                .attr("aria-label")
                .is_none_or(|label| label.trim().is_empty())
    }) {
        issues.push("each link must have a text or aria-label name");
    }
    if !issues.is_empty() {
        return Err(ArtifactToolError::InvalidArguments(format!(
            "HTML accessibility check failed: {}",
            issues.join("; ")
        )));
    }
    Ok(json!({
        "standard": "Noema accessible HTML baseline 1",
        "passed": true,
        "checks": [
            "document_language",
            "document_title",
            "main_landmark",
            "single_primary_heading",
            "image_text_alternatives",
            "table_structure",
            "link_names"
        ]
    }))
}

fn selector(value: &str) -> Selector {
    Selector::parse(value).expect("static accessibility selector")
}

fn has_text(element: ElementRef<'_>) -> bool {
    element.text().any(|text| !text.trim().is_empty())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn creates_all_requested_versions_in_conversation_scope() {
        let store = crate::test_support::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .create_conversation(
                noema_conversations::NewConversation::local_chat_for_provider("codex", None, None),
            )
            .await
            .expect("conversation");
        let operations =
            crate::test_support::artifact_operations(&store).expect("artifact operations");

        let result = execute_artifact_create_local_file(
            &store,
            &operations,
            &ArtifactToolRuntimeContext {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: "turn:test".to_string(),
                user_item_id: "item:test".to_string(),
                created_by_actor_id: "agent:primary".to_string(),
                task_id: None,
                task_run_id: None,
            },
            Some("call:test".to_string()),
            &json!({
                "title": "Agent note",
                "artifact_kind": "document",
                "filename": "agent-note.md",
                "media_type": "text/markdown",
                "sources": [{
                    "source_id": "item:test",
                    "source_version": "turn:test",
                    "source_owner": "human:local",
                    "disclosure_scope": "this conversation"
                }],
                "versions": [
                    {"title": "Draft", "content": "version one"},
                    {"title": "Revision", "content": "version two"}
                ]
            }),
        )
        .await;

        assert!(result.success, "{}", result.payload);
        assert_eq!(result.payload["current_version_index"], 2);
        assert_eq!(result.payload["versions"].as_array().map(Vec::len), Some(2));
        assert_eq!(result.payload["sources"][0]["source_id"], "item:test");
        let artifacts = store
            .list_artifacts_for_owner(
                noema_artifacts::ArtifactOwnerRef::conversation(&conversation.conversation_id),
                10,
            )
            .await
            .expect("artifacts");
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].versions.len(), 2);
        assert_eq!(
            artifacts[0].artifact.metadata["sources"][0]["source_version"],
            "turn:test"
        );
        assert_eq!(
            artifacts[0].versions[1].metadata["sources"][0]["source_owner"],
            "human:local"
        );
    }

    #[test]
    fn source_records_are_required() {
        let error = parse_arguments(&json!({
            "title": "No sources",
            "artifact_kind": "document",
            "filename": "no-sources.md",
            "sources": [],
            "versions": [{"content": "body"}]
        }))
        .expect_err("missing sources must fail");
        assert!(error.to_string().contains("at least one item"));
    }

    #[test]
    fn accessible_html_check_rejects_missing_structure() {
        let error = check_accessible_html("<html><body><img src='x.png'></body></html>")
            .expect_err("inaccessible HTML must fail");
        let message = error.to_string();
        assert!(message.contains("document language"));
        assert!(message.contains("document title"));
        assert!(message.contains("main landmark"));
        assert!(message.contains("alt attribute"));
    }

    #[test]
    fn accessible_html_check_returns_a_complete_receipt() {
        let content = "<!doctype html><html lang='en'><head><title>Move inventory</title></head>\
             <body><main><h1>Move inventory</h1><img src='desk.png' alt='Desk label'>\
             <table><caption>Assets</caption><tr><th>Item</th></tr></table>\
             <a href='sources.html'>Sources</a></main></body></html>";
        let report = check_accessible_html(content).expect("accessible HTML");
        assert_eq!(report["passed"], true);
        assert_eq!(report["checks"].as_array().map(Vec::len), Some(7));
        let metadata = version_metadata(
            &[CreateLocalFileArtifactSourceArguments {
                source_id: "source:1".to_string(),
                source_version: "1".to_string(),
                source_owner: "Kevin".to_string(),
                disclosure_scope: "private".to_string(),
            }],
            None,
            "inventory.html",
            content,
        )
        .expect("filename-routed check");
        assert_eq!(metadata["accessibility"]["passed"], true);
    }
}
