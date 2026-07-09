use async_graphql::{Enum, InputObject, Result, SimpleObject};

use super::{errors::graphql_error, schema::GraphqlState};

const GRAPHQL_ARTIFACT_ACTOR_ID: &str = "human:local";

/// Artifact storage kind exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ArtifactStorageKind")]
pub enum GraphqlArtifactStorageKind {
    /// Artifact bytes live in the local Noema filesystem.
    LocalFile,
    /// Artifact content is referenced by an external durable URL.
    ExternalUrl,
}

impl From<crate::ArtifactStorageKind> for GraphqlArtifactStorageKind {
    fn from(kind: crate::ArtifactStorageKind) -> Self {
        match kind {
            crate::ArtifactStorageKind::LocalFile => Self::LocalFile,
            crate::ArtifactStorageKind::ExternalUrl => Self::ExternalUrl,
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
            crate::ArtifactOwnerRef {
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

pub async fn create_conversation_external_artifact(
    state: &GraphqlState,
    input: GraphqlCreateConversationExternalArtifactInput,
) -> Result<GraphqlArtifact> {
    let store = state.store()?;
    let external_url = validated_external_url(&input.external_url)?;
    let source = crate::ArtifactSource {
        conversation_id: Some(input.conversation_id.clone()),
        ..Default::default()
    };
    let created = store
        .create_artifact_with_initial_version(
            crate::NewArtifact {
                artifact_id: None,
                owner: crate::ArtifactOwnerRef::conversation(input.conversation_id.clone()),
                title: input.title,
                description: input.description,
                artifact_kind: input.artifact_kind,
                storage_kind: crate::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: GRAPHQL_ARTIFACT_ACTOR_ID.to_string(),
                source: source.clone(),
                metadata: serde_json::json!({}),
            },
            crate::NewArtifactVersion {
                artifact_version_id: None,
                title: None,
                storage: crate::ArtifactVersionStorage::ExternalUrl { url: external_url },
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

fn graphql_artifact_from_store(artifact: crate::ArtifactWithVersions) -> Result<GraphqlArtifact> {
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
    version: crate::ArtifactVersionRecord,
) -> Result<GraphqlArtifactVersion> {
    let (external_url, download_url) = match version.storage {
        crate::ArtifactVersionStorage::LocalFile { .. } => (
            None,
            Some(crate::artifact_download_url(&version.artifact_version_id)),
        ),
        crate::ArtifactVersionStorage::ExternalUrl { url } => (Some(url), None),
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

fn validated_external_url(value: &str) -> Result<String> {
    let url = url::Url::parse(value)
        .map_err(|_| graphql_error("external URL must be a valid HTTP or HTTPS URL"))?;
    match url.scheme() {
        "http" | "https" => Ok(url.into()),
        _ => Err(graphql_error("external URL must use HTTP or HTTPS")),
    }
}
