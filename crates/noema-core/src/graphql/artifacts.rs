use async_graphql::{Enum, InputObject, Result, SimpleObject};
use noema_home::{NoemaPaths, SystemErrorEvent, SystemErrorLogger};

use super::{errors::graphql_error, schema::GraphqlState};

const GRAPHQL_ARTIFACT_ACTOR_ID: &str = "human:local";

/// Authorized local artifact bytes prepared for HTTP download adaptation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizedArtifactDownload {
    /// Safe filename for the download response.
    pub filename: String,
    /// Stored media type, or the binary default when absent.
    pub media_type: String,
    /// Validated artifact contents.
    pub bytes: Vec<u8>,
}

/// Internal failure while resolving an authorized artifact download.
#[derive(Debug, thiserror::Error)]
#[error("artifact download unavailable")]
pub struct AuthorizedArtifactDownloadError;

/// Resolve a local artifact only when it belongs to the authenticated principal.
///
/// # Errors
///
/// Returns an error when required runtime state or the authorized store query
/// is unavailable. Missing, unauthorized, or invalid files return `Ok(None)`.
pub async fn authorized_artifact_download(
    state: &GraphqlState,
    principal: &super::RequestPrincipal,
    artifact_version_id: &str,
) -> std::result::Result<Option<AuthorizedArtifactDownload>, AuthorizedArtifactDownloadError> {
    let paths = state.paths().map_err(|_| AuthorizedArtifactDownloadError)?;
    let store = state.store().map_err(|_| {
        record_artifact_download_failure(paths, "store_state");
        AuthorizedArtifactDownloadError
    })?;
    let Some((artifact, version)) = store
        .get_local_artifact_version_for_human(artifact_version_id, principal.subject_id())
        .await
        .map_err(|_| {
            record_artifact_download_failure(paths, "authorized_version_query");
            AuthorizedArtifactDownloadError
        })?
    else {
        return Ok(None);
    };
    let Ok((absolute_path, bytes)) =
        crate::artifacts::read_validated_local_artifact_file(paths, &artifact, &version)
    else {
        return Ok(None);
    };
    let Some(filename) = absolute_path.file_name().and_then(|value| value.to_str()) else {
        return Ok(None);
    };
    Ok(Some(AuthorizedArtifactDownload {
        filename: filename.to_owned(),
        media_type: version
            .media_type
            .unwrap_or_else(|| "application/octet-stream".to_owned()),
        bytes,
    }))
}

fn record_artifact_download_failure(paths: &NoemaPaths, operation: &'static str) {
    let event = SystemErrorEvent::new(
        "artifact_download_failure",
        "artifact download operation failed",
    )
    .with_context(serde_json::json!({ "operation": operation }));
    if SystemErrorLogger::from_paths(paths).append(event).is_err() {
        eprintln!("Noema artifact download failure: diagnostic_write");
    }
}

/// Artifact storage kind exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ArtifactStorageKind")]
pub enum GraphqlArtifactStorageKind {
    /// Artifact bytes live in the local Noema filesystem.
    LocalFile,
    /// Artifact content is referenced by an external durable URL.
    ExternalUrl,
}

impl From<noema_artifacts::ArtifactStorageKind> for GraphqlArtifactStorageKind {
    fn from(kind: noema_artifacts::ArtifactStorageKind) -> Self {
        match kind {
            noema_artifacts::ArtifactStorageKind::LocalFile => Self::LocalFile,
            noema_artifacts::ArtifactStorageKind::ExternalUrl => Self::ExternalUrl,
        }
    }
}

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

pub async fn artifacts(
    state: &GraphqlState,
    owner_object_type: String,
    owner_object_id: String,
    limit: Option<i32>,
) -> Result<Vec<GraphqlArtifact>> {
    let store = state.store()?;
    let artifacts = store
        .list_artifacts_for_owner(
            noema_artifacts::ArtifactOwnerRef {
                object_type: owner_object_type,
                object_id: owner_object_id,
            },
            i64::from(limit.unwrap_or(20)),
        )
        .await
        .map_err(graphql_error)?;
    artifacts
        .into_iter()
        .map(graphql_artifact_from_store)
        .collect()
}

pub async fn artifact(
    state: &GraphqlState,
    artifact_id: String,
) -> Result<Option<GraphqlArtifact>> {
    let store = state.store()?;
    let artifact = store
        .get_artifact(&artifact_id)
        .await
        .map_err(graphql_error)?;
    artifact.map(graphql_artifact_from_store).transpose()
}

pub async fn artifact_version_detail(
    state: &GraphqlState,
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

    match &version.storage {
        noema_artifacts::ArtifactVersionStorage::ExternalUrl { url } => {
            Ok(Some(GraphqlArtifactVersionDetail {
                artifact_version_id: version.artifact_version_id,
                artifact_id: version.artifact_id,
                version_index,
                title,
                artifact_kind: artifact.artifact.artifact_kind,
                storage_kind: artifact.artifact.storage_kind.into(),
                media_type,
                preview_kind: GraphqlArtifactVersionPreviewKind::External,
                markdown: None,
                plain_text: None,
                download_url: None,
                external_url: Some(url.clone()),
                versions,
            }))
        }
        noema_artifacts::ArtifactVersionStorage::LocalFile { .. } => {
            let download_url = Some(noema_artifacts::artifact_download_url(
                &version.artifact_version_id,
            ));
            let Some(preview_kind) = text_preview_kind(media_type.as_deref()) else {
                return Ok(Some(GraphqlArtifactVersionDetail {
                    artifact_version_id: version.artifact_version_id,
                    artifact_id: version.artifact_id,
                    version_index,
                    title,
                    artifact_kind: artifact.artifact.artifact_kind,
                    storage_kind: artifact.artifact.storage_kind.into(),
                    media_type,
                    preview_kind: GraphqlArtifactVersionPreviewKind::Unsupported,
                    markdown: None,
                    plain_text: None,
                    download_url,
                    external_url: None,
                    versions,
                }));
            };

            let (_, bytes) = crate::artifacts::read_validated_local_artifact_file(
                state.paths()?,
                &artifact.artifact,
                &version,
            )
            .map_err(graphql_error)?;
            let content = String::from_utf8(bytes).map_err(|error| {
                graphql_error(format!("artifact text content is not valid UTF-8: {error}"))
            })?;
            let (markdown, plain_text) = match preview_kind {
                GraphqlArtifactVersionPreviewKind::Markdown => (Some(content), None),
                GraphqlArtifactVersionPreviewKind::PlainText => (None, Some(content)),
                GraphqlArtifactVersionPreviewKind::Unsupported
                | GraphqlArtifactVersionPreviewKind::External => {
                    unreachable!("only local text preview kinds reach content decoding")
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
                external_url: None,
                versions,
            }))
        }
    }
}

pub async fn create_conversation_external_artifact(
    state: &GraphqlState,
    input: GraphqlCreateConversationExternalArtifactInput,
) -> Result<GraphqlArtifact> {
    let store = state.store()?;
    let external_url = noema_artifacts::validate_external_artifact_url(&input.external_url)
        .map_err(graphql_error)?;
    let source = noema_artifacts::ArtifactSource {
        conversation_id: Some(input.conversation_id.clone()),
        ..Default::default()
    };
    let created = store
        .create_artifact_with_initial_version(
            noema_artifacts::NewArtifact {
                artifact_id: None,
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    input.conversation_id.clone(),
                ),
                title: input.title,
                description: input.description,
                artifact_kind: input.artifact_kind,
                storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: GRAPHQL_ARTIFACT_ACTOR_ID.to_string(),
                source: source.clone(),
                metadata: serde_json::json!({}),
            },
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: None,
                storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl { url: external_url },
                media_type: input.media_type,
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: GRAPHQL_ARTIFACT_ACTOR_ID.to_string(),
                source,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .map_err(graphql_error)?;
    graphql_artifact_from_store(created)
}

fn graphql_artifact_from_store(
    artifact: noema_artifacts::ArtifactWithVersions,
) -> Result<GraphqlArtifact> {
    Ok(GraphqlArtifact {
        artifact_id: artifact.artifact.artifact_id,
        owner_object_type: artifact.artifact.owner.object_type,
        owner_object_id: artifact.artifact.owner.object_id,
        title: artifact.artifact.title,
        description: artifact.artifact.description,
        artifact_kind: artifact.artifact.artifact_kind,
        storage_kind: artifact.artifact.storage_kind.into(),
        current_version: graphql_artifact_version_from_store(artifact.current_version)?,
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
    Ok(GraphqlArtifactVersion {
        artifact_version_id: version.artifact_version_id,
        artifact_id: version.artifact_id,
        version_index,
        external_url,
        download_url,
        media_type: version.media_type,
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

    async fn local_artifact_fixture() -> (
        tempfile::TempDir,
        NoemaPaths,
        crate::NoemaStore,
        noema_artifacts::ArtifactWithVersions,
        GraphqlState,
    ) {
        let home = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
            .await
            .expect("store");
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let artifact = crate::create_conversation_local_file_artifact(
            &store,
            &paths,
            crate::NewConversationLocalFileArtifact {
                conversation_id: conversation.conversation_id.clone(),
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
            },
        )
        .await
        .expect("artifact");
        let state = GraphqlState::for_tests_with_store_and_paths(store.clone(), paths.clone());
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
    async fn authorized_download_hides_missing_version() {
        let (_home, _paths, _store, _artifact, state) = local_artifact_fixture().await;
        assert!(
            authorized_artifact_download(
                &state,
                &super::super::RequestPrincipal::local(),
                "artifact-version:missing",
            )
            .await
            .expect("download query")
            .is_none()
        );
    }

    #[tokio::test]
    async fn authorized_download_hides_other_owner() {
        let (_home, _paths, store, artifact, state) = local_artifact_fixture().await;
        store
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE conversations SET owner_object_id = 'human:other', primary_human_id = 'human:other' WHERE conversation_id = ?1",
                        [&artifact.artifact.owner.object_id],
                    )
                    .map(|_| ())
                    .map_err(crate::StoreError::Sqlite)
            })
            .await
            .expect("change owner");
        assert!(
            authorized_artifact_download(
                &state,
                &super::super::RequestPrincipal::local(),
                &artifact.current_version.artifact_version_id,
            )
            .await
            .expect("download query")
            .is_none()
        );
    }

    #[tokio::test]
    async fn authorized_download_hides_missing_file() {
        let (_home, paths, _store, artifact, state) = local_artifact_fixture().await;
        let noema_artifacts::ArtifactVersionStorage::LocalFile { relative_path } =
            &artifact.current_version.storage
        else {
            panic!("expected local file");
        };
        std::fs::remove_file(paths.root().join(relative_path)).expect("remove artifact");
        assert!(
            authorized_artifact_download(
                &state,
                &super::super::RequestPrincipal::local(),
                &artifact.current_version.artifact_version_id,
            )
            .await
            .expect("download query")
            .is_none()
        );
    }

    #[tokio::test]
    async fn authorized_download_refuses_traversal() {
        let (_home, paths, store, artifact, state) = local_artifact_fixture().await;
        store
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE artifact_versions SET local_relative_path = 'providers/secret.txt' WHERE artifact_version_id = ?1",
                        [&artifact.current_version.artifact_version_id],
                    )
                    .map(|_| ())
                    .map_err(crate::StoreError::Sqlite)
            })
            .await
            .expect("forge artifact path");
        std::fs::create_dir_all(paths.providers_dir()).expect("providers dir");
        std::fs::write(paths.providers_dir().join("secret.txt"), b"secret").expect("secret");
        assert!(
            authorized_artifact_download(
                &state,
                &super::super::RequestPrincipal::local(),
                &artifact.current_version.artifact_version_id,
            )
            .await
            .expect("download query")
            .is_none()
        );
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
        assert!(
            authorized_artifact_download(
                &state,
                &super::super::RequestPrincipal::local(),
                &artifact.current_version.artifact_version_id,
            )
            .await
            .expect("download query")
            .is_none()
        );
    }

    #[tokio::test]
    async fn authorized_download_store_failure_writes_redacted_diagnostic() {
        let (_home, paths, store, artifact, state) = local_artifact_fixture().await;
        store
            .with_connection(|connection| {
                connection
                    .execute("DROP TABLE artifact_versions", [])
                    .map(|_| ())
                    .map_err(crate::StoreError::Sqlite)
            })
            .await
            .expect("break artifact query");
        assert!(
            authorized_artifact_download(
                &state,
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
