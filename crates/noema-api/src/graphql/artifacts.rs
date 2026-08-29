use async_graphql::{Enum, InputObject, Result, SimpleObject};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use noema_artifacts::ArtifactMetadataStore;
use ring::digest::{SHA256, digest};

use super::{errors::graphql_error, schema::GraphqlState};

mod download;

pub use download::{
    AuthorizedArtifactDownload, AuthorizedArtifactDownloadError, authorized_artifact_download,
};
#[cfg(test)]
use download::{
    AuthorizedArtifactDownloadRepository, AuthorizedArtifactDownloadRepositoryFuture,
    authorized_artifact_download_with_repository,
};

/// Artifact storage kind exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ArtifactStorageKind")]
pub enum GraphqlArtifactStorageKind {
    /// Artifact bytes live in the local Noema filesystem.
    LocalFile,
    /// Artifact content is referenced by an external durable URL.
    ExternalUrl,
}

graphql_enum_from!(noema_artifacts::ArtifactStorageKind => GraphqlArtifactStorageKind {
    LocalFile => LocalFile,
    ExternalUrl => ExternalUrl,
});

/// Artifact version metadata safe to expose to GraphQL clients.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ArtifactVersion")]
pub struct GraphqlArtifactVersion {
    /// Stable artifact version id.
    pub artifact_version_id: String,
    /// Parent artifact id.
    pub artifact_id: String,
    /// Monotonic version index within the artifact.
    pub version_index: i32,
    /// Durable external URL when the version is externally hosted.
    pub external_url: Option<String>,
    /// Local download route when the version is stored in Noema.
    pub download_url: Option<String>,
    /// Optional media type for the version payload.
    pub media_type: Option<String>,
    /// Exact byte count when known.
    pub byte_size: Option<i32>,
    /// SHA-256 of local file bytes when known.
    pub content_sha256: Option<String>,
}

/// Preview renderer selected for an artifact version detail panel.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ArtifactVersionPreviewKind")]
pub enum GraphqlArtifactVersionPreviewKind {
    /// Local Markdown bytes are available as UTF-8 text.
    Markdown,
    /// Local plain-text bytes are available as UTF-8 text.
    PlainText,
    /// The version exists but this first slice cannot render it inline.
    Unsupported,
    /// The version points at an external URL.
    External,
}

/// Artifact version detail payload for the chat detail rail.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ArtifactVersionDetail")]
pub struct GraphqlArtifactVersionDetail {
    /// Stable artifact version id.
    pub artifact_version_id: String,
    /// Parent artifact id.
    pub artifact_id: String,
    /// Monotonic version index within the artifact.
    pub version_index: i32,
    /// Display title inherited from version title or artifact title.
    pub title: String,
    /// Product-defined artifact kind label.
    pub artifact_kind: String,
    /// Durable storage family shared by the artifact.
    pub storage_kind: GraphqlArtifactStorageKind,
    /// Optional media type for the version payload.
    pub media_type: Option<String>,
    /// Preview renderer selected by the server.
    pub preview_kind: GraphqlArtifactVersionPreviewKind,
    /// Markdown content when previewKind is MARKDOWN.
    pub markdown: Option<String>,
    /// Literal text content when previewKind is PLAIN_TEXT.
    pub plain_text: Option<String>,
    /// Local download route when the version is stored in Noema.
    pub download_url: Option<String>,
    /// External durable URL when the version is externally hosted.
    pub external_url: Option<String>,
    /// Full immutable version history in ascending version order.
    pub versions: Vec<GraphqlArtifactVersion>,
}

/// Artifact metadata safe to expose to GraphQL clients.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "Artifact")]
pub struct GraphqlArtifact {
    /// Stable artifact id.
    pub artifact_id: String,
    /// Concrete owner object type.
    pub owner_object_type: String,
    /// Concrete owner object id.
    pub owner_object_id: String,
    /// Human-readable artifact title.
    pub title: String,
    /// Optional artifact description.
    pub description: Option<String>,
    /// Product-defined artifact kind label.
    pub artifact_kind: String,
    /// Durable storage family shared by every version.
    pub storage_kind: GraphqlArtifactStorageKind,
    /// Current immutable version.
    pub current_version: GraphqlArtifactVersion,
}

/// Input for creating a conversation-owned external URL artifact.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CreateConversationExternalArtifactInput")]
pub struct GraphqlCreateConversationExternalArtifactInput {
    /// Owning conversation id.
    pub conversation_id: String,
    /// Human-readable artifact title.
    pub title: String,
    /// Optional artifact description.
    pub description: Option<String>,
    /// Product-defined artifact kind label.
    pub artifact_kind: String,
    /// Durable external HTTP(S) URL for the initial version.
    pub external_url: String,
    /// Optional media type for the external payload.
    pub media_type: Option<String>,
}

/// Input for one bounded private file attached to an Inbox Task.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CreateTaskLocalArtifactInput")]
pub struct GraphqlCreateTaskLocalArtifactInput {
    /// Task that owns this private source file.
    pub task_id: String,
    /// Expected Task revision.
    pub expected_revision: i64,
    /// Expected Task generation.
    pub expected_generation: i64,
    /// Human-readable source title.
    pub title: String,
    /// Safe single-segment filename.
    pub filename: String,
    /// Source media type.
    pub media_type: String,
    /// Base64 file bytes. The decoded file limit is 40 KiB.
    pub content_base64: String,
    /// Stable source identifier from the supplied packet.
    pub source_id: String,
    /// Exact source version or source date label.
    pub source_version: String,
    /// Person or organization that owns the source.
    pub source_owner: String,
    /// Intended audience for this source.
    pub disclosure_scope: String,
}

/// Receipt for one immutable Task source upload.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskLocalArtifactReceipt")]
pub struct GraphqlTaskLocalArtifactReceipt {
    /// Created artifact.
    pub artifact: GraphqlArtifact,
    /// Stable source identifier.
    pub source_id: String,
    /// Exact source version.
    pub source_version: String,
    /// Source owner.
    pub source_owner: String,
    /// Intended audience.
    pub disclosure_scope: String,
    /// SHA-256 of the received bytes.
    pub content_sha256: String,
    /// Received byte count.
    pub byte_size: i32,
}

const MAX_TASK_UPLOAD_BYTES: usize = 40 * 1024;

/// Create one immutable local artifact owned by an Inbox Task.
pub async fn create_task_local_artifact(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlCreateTaskLocalArtifactInput,
) -> Result<GraphqlTaskLocalArtifactReceipt> {
    crate::graphql::tasks::require_owner(principal_subject)?;
    let task_id = crate::graphql::tasks::parse_task_id(&input.task_id)?;
    let detail = state
        .store()?
        .get_work_task(&task_id)
        .await
        .map_err(|_| crate::graphql::tasks::unavailable())?
        .ok_or_else(crate::graphql::tasks::unavailable)?;
    crate::graphql::tasks::require_personal_workspace(&detail.workspace.workspace_id)?;
    let expected_revision =
        crate::graphql::tasks::positive(input.expected_revision, "expectedRevision")?;
    let expected_generation =
        crate::graphql::tasks::positive(input.expected_generation, "expectedGeneration")?;
    if detail.task.revision != expected_revision
        || detail.task.generation != expected_generation
        || detail.stage.system_behavior != noema_tasks::WorkflowStageBehavior::Intake
    {
        return Err(crate::graphql::tasks::unavailable());
    }
    validate_upload_label(&input.title, 160, "title")?;
    validate_upload_label(&input.media_type, 120, "mediaType")?;
    validate_upload_label(&input.source_id, 200, "sourceId")?;
    validate_upload_label(&input.source_version, 200, "sourceVersion")?;
    validate_upload_label(&input.source_owner, 200, "sourceOwner")?;
    validate_upload_label(&input.disclosure_scope, 200, "disclosureScope")?;
    let filename = noema_artifacts::safe_artifact_filename(&input.filename)
        .map_err(|_| crate::graphql::tasks::invalid_input_error("filename"))?
        .to_string();
    let bytes = BASE64_STANDARD
        .decode(input.content_base64.as_bytes())
        .map_err(|_| crate::graphql::tasks::invalid_input_error("contentBase64"))?;
    if bytes.is_empty() || bytes.len() > MAX_TASK_UPLOAD_BYTES {
        return Err(crate::graphql::tasks::invalid_input_error("contentBase64"));
    }
    let content_sha256 = digest(&SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let byte_size = i32::try_from(bytes.len())
        .map_err(|_| crate::graphql::tasks::invalid_input_error("contentBase64"))?;
    let metadata = serde_json::json!({
        "source_id": &input.source_id,
        "source_version": &input.source_version,
        "source_owner": &input.source_owner,
        "disclosure_scope": &input.disclosure_scope,
        "content_sha256": &content_sha256,
        "byte_size": byte_size,
        "media_type": &input.media_type,
        "filename": &filename,
    });
    let artifact = state
        .artifact_operations()?
        .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
            owner: noema_artifacts::ArtifactOwnerRef::task(task_id.as_str()),
            title: input.title,
            description: Some("Private Task source file".to_string()),
            artifact_kind: "source_file".to_string(),
            filename,
            bytes,
            media_type: Some(input.media_type),
            created_by_actor_id: principal_subject.to_string(),
            source: noema_artifacts::ArtifactSource::default(),
            metadata,
        })
        .await
        .map_err(graphql_error)?;
    let receipt = GraphqlTaskLocalArtifactReceipt {
        artifact: graphql_artifact_from_store(artifact)?,
        source_id: input.source_id,
        source_version: input.source_version,
        source_owner: input.source_owner,
        disclosure_scope: input.disclosure_scope,
        content_sha256,
        byte_size,
    };
    Ok(receipt)
}

fn validate_upload_label(value: &str, max_chars: usize, field: &str) -> Result<()> {
    let count = value.chars().count();
    if count == 0 || count > max_chars || value.trim().is_empty() {
        Err(crate::graphql::tasks::invalid_input_error(field))
    } else {
        Ok(())
    }
}

pub async fn artifacts(
    state: &GraphqlState,
    human_id: &str,
    owner_object_type: String,
    owner_object_id: String,
    limit: Option<i32>,
) -> Result<Vec<GraphqlArtifact>> {
    let store = state.store()?;
    let owner = noema_artifacts::ArtifactOwnerRef {
        object_type: owner_object_type,
        object_id: owner_object_id,
    };
    if !store
        .artifact_owner_is_authorized_for_human(&owner, human_id)
        .await
        .map_err(graphql_error)?
    {
        return Ok(Vec::new());
    }
    let artifacts = store
        .list_artifacts_for_owner(owner, i64::from(limit.unwrap_or(20)))
        .await
        .map_err(graphql_error)?;
    artifacts
        .into_iter()
        .map(graphql_artifact_from_store)
        .collect()
}

pub async fn artifact_version_detail(
    state: &GraphqlState,
    human_id: &str,
    artifact_version_id: String,
) -> Result<Option<GraphqlArtifactVersionDetail>> {
    let store = state.store()?;
    let Some(version) = store
        .get_artifact_version(&artifact_version_id)
        .await
        .map_err(graphql_error)?
    else {
        return Ok(None);
    };

    let Some(artifact) = store
        .get_artifact(&version.artifact_id)
        .await
        .map_err(graphql_error)?
    else {
        return Ok(None);
    };
    if !store
        .artifact_owner_is_authorized_for_human(&artifact.artifact.owner, human_id)
        .await
        .map_err(graphql_error)?
    {
        return Ok(None);
    }

    let title = version
        .title
        .clone()
        .unwrap_or_else(|| artifact.artifact.title.clone());
    let media_type = version.media_type.clone();
    let version_index = i32::try_from(version.version_index)
        .map_err(|_| graphql_error("artifact version index exceeds GraphQL Int range"))?;
    let versions = artifact
        .versions
        .clone()
        .into_iter()
        .map(graphql_artifact_version_from_store)
        .collect::<Result<Vec<_>>>()?;

    let (preview_kind, markdown, plain_text, download_url, external_url) = match &version.storage {
        noema_artifacts::ArtifactVersionStorage::ExternalUrl { url } => (
            GraphqlArtifactVersionPreviewKind::External,
            None,
            None,
            None,
            Some(url.clone()),
        ),
        noema_artifacts::ArtifactVersionStorage::LocalFile { .. } => {
            let download_url = Some(noema_artifacts::artifact_download_url(
                &version.artifact_version_id,
            ));
            let (preview_kind, markdown, plain_text) =
                match text_preview_kind(media_type.as_deref()) {
                    None => (GraphqlArtifactVersionPreviewKind::Unsupported, None, None),
                    Some(preview_kind) => {
                        let file = state
                            .artifact_operations()?
                            .read_local_file(noema_artifacts::ReadLocalArtifactRequest {
                                artifact: artifact.artifact.clone(),
                                version: version.clone(),
                            })
                            .await
                            .map_err(graphql_error)?;
                        let content = String::from_utf8(file.bytes).map_err(|error| {
                            graphql_error(format!(
                                "artifact text content is not valid UTF-8: {error}"
                            ))
                        })?;
                        match preview_kind {
                            GraphqlArtifactVersionPreviewKind::Markdown => {
                                (preview_kind, Some(content), None)
                            }
                            GraphqlArtifactVersionPreviewKind::PlainText => {
                                (preview_kind, None, Some(content))
                            }
                            GraphqlArtifactVersionPreviewKind::Unsupported
                            | GraphqlArtifactVersionPreviewKind::External => {
                                unreachable!("only local text preview kinds reach content decoding")
                            }
                        }
                    }
                };
            (preview_kind, markdown, plain_text, download_url, None)
        }
    };
    Ok(Some(GraphqlArtifactVersionDetail {
        artifact_version_id: version.artifact_version_id,
        artifact_id: version.artifact_id,
        version_index,
        title,
        artifact_kind: artifact.artifact.artifact_kind,
        storage_kind: artifact.artifact.storage_kind.into(),
        media_type,
        preview_kind,
        markdown,
        plain_text,
        download_url,
        external_url,
        versions,
    }))
}

pub async fn create_conversation_external_artifact(
    state: &GraphqlState,
    human_id: &str,
    input: GraphqlCreateConversationExternalArtifactInput,
) -> Result<GraphqlArtifact> {
    let store = state.store()?;
    if !store
        .conversation_is_owned_by_human(&input.conversation_id, human_id)
        .await
        .map_err(graphql_error)?
    {
        return Err(async_graphql::Error::new("conversation is unavailable"));
    }
    let source = noema_artifacts::ArtifactSource {
        conversation_id: Some(input.conversation_id.clone()),
        ..Default::default()
    };
    let created = ArtifactMetadataStore::create_artifact_with_initial_version(
        store,
        noema_artifacts::NewArtifact {
            artifact_id: None,
            owner: noema_artifacts::ArtifactOwnerRef::conversation(input.conversation_id.clone()),
            title: input.title,
            description: input.description,
            artifact_kind: input.artifact_kind,
            storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
            created_by_actor_id: human_id.to_string(),
            source: source.clone(),
            metadata: serde_json::json!({}),
        },
        noema_artifacts::NewArtifactVersion {
            artifact_version_id: None,
            title: None,
            storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                url: input.external_url,
            },
            media_type: input.media_type,
            byte_size: None,
            content_sha256: None,
            created_by_actor_id: human_id.to_string(),
            source,
            metadata: serde_json::json!({}),
        },
    )
    .await
    .map_err(graphql_error)?;
    graphql_artifact_from_store(created)
}

pub(crate) fn graphql_artifact_from_store(
    artifact: noema_artifacts::ArtifactWithVersions,
) -> Result<GraphqlArtifact> {
    graphql_artifact_from_current(artifact.artifact, artifact.current_version)
}

pub(crate) fn graphql_artifact_from_current(
    artifact: noema_artifacts::ArtifactRecord,
    current_version: noema_artifacts::ArtifactVersionRecord,
) -> Result<GraphqlArtifact> {
    Ok(GraphqlArtifact {
        artifact_id: artifact.artifact_id,
        owner_object_type: artifact.owner.object_type,
        owner_object_id: artifact.owner.object_id,
        title: artifact.title,
        description: artifact.description,
        artifact_kind: artifact.artifact_kind,
        storage_kind: artifact.storage_kind.into(),
        current_version: graphql_artifact_version_from_store(current_version)?,
    })
}

fn graphql_artifact_version_from_store(
    version: noema_artifacts::ArtifactVersionRecord,
) -> Result<GraphqlArtifactVersion> {
    let (external_url, download_url) = match version.storage {
        noema_artifacts::ArtifactVersionStorage::LocalFile { .. } => (
            None,
            Some(noema_artifacts::artifact_download_url(
                &version.artifact_version_id,
            )),
        ),
        noema_artifacts::ArtifactVersionStorage::ExternalUrl { url } => (Some(url), None),
    };
    let version_index = i32::try_from(version.version_index)
        .map_err(|_| graphql_error("artifact version index exceeds GraphQL Int range"))?;
    let byte_size = version
        .byte_size
        .map(i32::try_from)
        .transpose()
        .map_err(|_| graphql_error("artifact byte size exceeds GraphQL Int range"))?;
    Ok(GraphqlArtifactVersion {
        artifact_version_id: version.artifact_version_id,
        artifact_id: version.artifact_id,
        version_index,
        external_url,
        download_url,
        media_type: version.media_type,
        byte_size,
        content_sha256: version.content_sha256,
    })
}

fn text_preview_kind(media_type: Option<&str>) -> Option<GraphqlArtifactVersionPreviewKind> {
    let normalized = media_type?
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "text/markdown" | "text/x-markdown" => Some(GraphqlArtifactVersionPreviewKind::Markdown),
        "text/plain" => Some(GraphqlArtifactVersionPreviewKind::PlainText),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestEnvironment;

    #[derive(Debug)]
    enum TestArtifactDownloadRepository {
        Found(
            Box<(
                noema_artifacts::ArtifactRecord,
                noema_artifacts::ArtifactVersionRecord,
            )>,
        ),
        Failure,
    }

    impl AuthorizedArtifactDownloadRepository for TestArtifactDownloadRepository {
        fn get_local_version_for_human<'a>(
            &'a self,
            _artifact_version_id: &'a str,
            _human_id: &'a str,
        ) -> AuthorizedArtifactDownloadRepositoryFuture<'a> {
            let result = match self {
                Self::Found(found) => Ok(Some((found.0.clone(), found.1.clone()))),
                Self::Failure => Err(AuthorizedArtifactDownloadError),
            };
            Box::pin(async move { result })
        }
    }

    async fn local_artifact_fixture() -> (
        tempfile::TempDir,
        TestEnvironment,
        noema_store::NoemaStore,
        noema_artifacts::ArtifactWithVersions,
        GraphqlState,
    ) {
        let home = tempfile::tempdir().expect("temp dir");
        let paths = TestEnvironment::from_root(home.path()).expect("paths");
        let store = crate::test_support::test_store_for_environment(&paths).await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let artifact_operations =
            crate::test_support::artifact_operations_for_environment(&store, &paths)
                .expect("artifact operations");
        let artifact = artifact_operations
            .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    &conversation.conversation_id,
                ),
                title: "Report".to_owned(),
                description: None,
                artifact_kind: "document".to_owned(),
                filename: "report.md".to_owned(),
                bytes: b"hello download".to_vec(),
                media_type: Some("text/markdown".to_owned()),
                created_by_actor_id: "agent:primary".to_owned(),
                source: noema_artifacts::ArtifactSource {
                    conversation_id: Some(conversation.conversation_id),
                    ..Default::default()
                },
                metadata: serde_json::json!({}),
            })
            .await
            .expect("artifact");
        let state =
            GraphqlState::for_tests_with_store_and_environment(store.clone(), paths.clone());
        (home, paths, store, artifact, state)
    }

    #[tokio::test]
    async fn authorized_download_returns_validated_payload() {
        let (_home, _paths, _store, artifact, state) = local_artifact_fixture().await;
        let download = authorized_artifact_download(
            &state,
            &super::super::RequestPrincipal::local(),
            &artifact.current_version.artifact_version_id,
        )
        .await
        .expect("download query")
        .expect("download");
        assert_eq!(download.filename, "report.md");
        assert_eq!(download.media_type, "text/markdown");
        assert_eq!(download.bytes, b"hello download");
    }

    #[tokio::test]
    async fn authorized_download_hides_other_owner() {
        let (_home, _paths, store, _artifact, state) = local_artifact_fixture().await;
        let mut other_conversation = noema_conversations::NewConversation::local_chat(None, None);
        other_conversation.owner = noema_conversations::ConversationOwnerRef::human("human:other")
            .expect("other human owner");
        other_conversation.primary_human_id = Some("human:other".to_owned());
        let other_conversation = store
            .create_conversation(other_conversation)
            .await
            .expect("other conversation");
        let other_artifact = state
            .artifact_operations()
            .expect("artifact operations")
            .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    &other_conversation.conversation_id,
                ),
                title: "Other report".to_owned(),
                description: None,
                artifact_kind: "document".to_owned(),
                filename: "other-report.md".to_owned(),
                bytes: b"private to another human".to_vec(),
                media_type: Some("text/markdown".to_owned()),
                created_by_actor_id: "agent:primary".to_owned(),
                source: noema_artifacts::ArtifactSource {
                    conversation_id: Some(other_conversation.conversation_id),
                    ..Default::default()
                },
                metadata: serde_json::json!({}),
            })
            .await
            .expect("other artifact");
        assert!(
            authorized_artifact_download(
                &state,
                &super::super::RequestPrincipal::local(),
                &other_artifact.current_version.artifact_version_id,
            )
            .await
            .expect("download query")
            .is_none()
        );
    }

    #[tokio::test]
    async fn authorized_download_hides_unreadable_local_payload() {
        let (_home, paths, _store, artifact, state) = local_artifact_fixture().await;
        let noema_artifacts::ArtifactVersionStorage::LocalFile { relative_path } =
            &artifact.current_version.storage
        else {
            panic!("expected local file");
        };
        std::fs::remove_file(paths.root().join(relative_path)).expect("remove artifact");

        let download = authorized_artifact_download(
            &state,
            &super::super::RequestPrincipal::local(),
            &artifact.current_version.artifact_version_id,
        )
        .await
        .expect("download query");

        assert!(download.is_none());
    }

    #[tokio::test]
    async fn authorized_download_refuses_traversal() {
        let (_home, paths, _store, artifact, state) = local_artifact_fixture().await;
        let mut forged_version = artifact.current_version.clone();
        forged_version.storage = noema_artifacts::ArtifactVersionStorage::LocalFile {
            relative_path: "providers/secret.txt".to_owned(),
        };
        let repository = TestArtifactDownloadRepository::Found(Box::new((
            artifact.artifact.clone(),
            forged_version,
        )));
        std::fs::create_dir_all(paths.providers_dir()).expect("providers dir");
        std::fs::write(paths.providers_dir().join("secret.txt"), b"secret").expect("secret");

        let download = authorized_artifact_download_with_repository(
            &state,
            state.artifact_operations().expect("artifact operations"),
            &repository,
            &super::super::RequestPrincipal::local(),
            &artifact.current_version.artifact_version_id,
        )
        .await
        .expect("download query");

        assert!(download.is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn authorized_download_refuses_symlink() {
        let (_home, paths, _store, artifact, state) = local_artifact_fixture().await;
        let noema_artifacts::ArtifactVersionStorage::LocalFile { relative_path } =
            &artifact.current_version.storage
        else {
            panic!("expected local file");
        };
        let artifact_path = paths.root().join(relative_path);
        std::fs::remove_file(&artifact_path).expect("remove artifact");
        let secret_path = paths.root().join("secret.txt");
        std::fs::write(&secret_path, b"secret").expect("secret");
        std::os::unix::fs::symlink(secret_path, artifact_path).expect("symlink");

        let download = authorized_artifact_download(
            &state,
            &super::super::RequestPrincipal::local(),
            &artifact.current_version.artifact_version_id,
        )
        .await
        .expect("download query");

        assert!(download.is_none());
    }

    #[tokio::test]
    async fn authorized_download_store_failure_writes_redacted_diagnostic() {
        let (_home, paths, _store, artifact, state) = local_artifact_fixture().await;
        let repository = TestArtifactDownloadRepository::Failure;
        assert!(
            authorized_artifact_download_with_repository(
                &state,
                state.artifact_operations().expect("artifact operations"),
                &repository,
                &super::super::RequestPrincipal::local(),
                &artifact.current_version.artifact_version_id,
            )
            .await
            .is_err()
        );
        let diagnostics = std::fs::read_to_string(paths.errors_log_path()).expect("diagnostics");
        assert!(diagnostics.contains("artifact_download_failure"));
        assert!(diagnostics.contains("authorized_version_query"));
        assert!(!diagnostics.contains("report.md"));
        assert!(!diagnostics.contains("hello download"));
    }
}
