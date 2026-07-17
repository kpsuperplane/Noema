use std::{future::Future, pin::Pin};

use super::GraphqlState;

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

pub(super) type AuthorizedArtifactDownloadRepositoryFuture<'a> = Pin<
    Box<
        dyn Future<
                Output = std::result::Result<
                    Option<(
                        noema_artifacts::ArtifactRecord,
                        noema_artifacts::ArtifactVersionRecord,
                    )>,
                    AuthorizedArtifactDownloadError,
                >,
            > + Send
            + 'a,
    >,
>;

pub(super) trait AuthorizedArtifactDownloadRepository {
    fn get_local_version_for_human<'a>(
        &'a self,
        artifact_version_id: &'a str,
        human_id: &'a str,
    ) -> AuthorizedArtifactDownloadRepositoryFuture<'a>;
}

impl AuthorizedArtifactDownloadRepository for noema_store::NoemaStore {
    fn get_local_version_for_human<'a>(
        &'a self,
        artifact_version_id: &'a str,
        human_id: &'a str,
    ) -> AuthorizedArtifactDownloadRepositoryFuture<'a> {
        Box::pin(async move {
            self.get_local_artifact_version_for_human(artifact_version_id, human_id)
                .await
                .map_err(|_| AuthorizedArtifactDownloadError)
        })
    }
}

/// Resolve a local artifact only when it belongs to the authenticated principal.
///
/// # Errors
///
/// Returns an error when required runtime state or the authorized store query
/// is unavailable. Missing, unauthorized, or invalid files return `Ok(None)`.
pub async fn authorized_artifact_download(
    state: &GraphqlState,
    principal: &crate::graphql::RequestPrincipal,
    artifact_version_id: &str,
) -> std::result::Result<Option<AuthorizedArtifactDownload>, AuthorizedArtifactDownloadError> {
    let artifact_operations = state
        .artifact_operations()
        .map_err(|_| AuthorizedArtifactDownloadError)?;
    let store = state.store().map_err(|_| {
        state.record_artifact_download_failure("store_state");
        AuthorizedArtifactDownloadError
    })?;
    authorized_artifact_download_with_repository(
        state,
        artifact_operations,
        store,
        principal,
        artifact_version_id,
    )
    .await
}

pub(super) async fn authorized_artifact_download_with_repository(
    state: &GraphqlState,
    artifact_operations: &noema_artifacts::ArtifactOperationsHandle,
    repository: &impl AuthorizedArtifactDownloadRepository,
    principal: &crate::graphql::RequestPrincipal,
    artifact_version_id: &str,
) -> std::result::Result<Option<AuthorizedArtifactDownload>, AuthorizedArtifactDownloadError> {
    let Some((artifact, version)) = repository
        .get_local_version_for_human(artifact_version_id, principal.subject_id())
        .await
        .map_err(|_| {
            state.record_artifact_download_failure("authorized_version_query");
            AuthorizedArtifactDownloadError
        })?
    else {
        return Ok(None);
    };
    let Ok(content) = artifact_operations
        .read_local_file(noema_artifacts::ReadLocalArtifactRequest {
            artifact,
            version: version.clone(),
        })
        .await
    else {
        return Ok(None);
    };
    Ok(Some(AuthorizedArtifactDownload {
        filename: content.filename,
        media_type: version
            .media_type
            .unwrap_or_else(|| "application/octet-stream".to_owned()),
        bytes: content.bytes,
    }))
}
