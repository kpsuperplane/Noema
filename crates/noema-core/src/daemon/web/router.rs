//! Axum route composition and HTTP response adaptation for the daemon web UI.

use std::borrow::Cow;

use async_graphql::{Data, http::ALL_WEBSOCKET_PROTOCOLS};
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket};
use axum::{
    Router,
    body::Body,
    extract::{Path, RawQuery, State, WebSocketUpgrade},
    http::{HeaderValue, Method, StatusCode, Uri, header},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use tower_http::{limit::RequestBodyLimitLayer, set_header::SetResponseHeaderLayer};
use tower_sessions::{MemoryStore, Session, SessionManagerLayer, cookie::SameSite};

use super::{WebState, assets::embedded_asset, authority, session};

const MAX_GRAPHQL_BODY_BYTES: usize = 64 * 1024;
const NOT_FOUND: &str = "not found";

/// Build the single application router served by the daemon.
pub(crate) fn build_router(state: WebState) -> Router {
    let authority = state.authority().clone();
    let session_layer = SessionManagerLayer::new(MemoryStore::default())
        .with_name("noema.sid")
        .with_http_only(true)
        .with_same_site(SameSite::Strict)
        .with_path("/")
        .with_secure(false)
        .with_private(state.sessions().key());

    Router::new()
        .route(
            "/graphql",
            get(graphiql)
                .post(graphql)
                .head(method_not_found)
                .fallback(method_not_found)
                .layer(RequestBodyLimitLayer::new(MAX_GRAPHQL_BODY_BYTES)),
        )
        .route(
            "/graphql/schema.graphql",
            get(graphql_schema)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .route(
            "/graphql/ws",
            get(graphql_ws)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .route(
            "/__noema/bootstrap/{capability}",
            get(bootstrap)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .route(
            "/mcp/oauth/callback",
            get(mcp_oauth_callback)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .route(
            "/artifacts/versions/{artifact_version_slug}/download",
            get(download_artifact_slug)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .route(
            "/artifacts/{artifact_version_id}/download",
            get(download_artifact_id)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .fallback(asset_or_not_found)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(session_layer)
        .layer(middleware::from_fn_with_state(
            authority,
            authority::enforce_authority,
        ))
        .with_state(state)
}

async fn graphql(
    State(state): State<WebState>,
    session: Session,
    request: GraphQLRequest,
) -> Response {
    if !session::is_authenticated(&session).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    GraphQLResponse::from(
        state
            .graphql_schema()
            .execute(with_request_principal(request.into_inner()))
            .await,
    )
    .into_response()
}

async fn graphql_ws(
    State(state): State<WebState>,
    session: Session,
    protocol: GraphQLProtocol,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !session::is_authenticated(&session).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let schema = state.graphql_schema().clone();
    upgrade
        .protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |socket| {
            GraphQLWebSocket::new(socket, schema, protocol)
                .with_data(request_principal_data())
                .serve()
        })
        .into_response()
}

fn with_request_principal(request: async_graphql::Request) -> async_graphql::Request {
    request.data(crate::graphql::RequestPrincipal::local())
}

fn request_principal_data() -> Data {
    let mut data = Data::default();
    data.insert(crate::graphql::RequestPrincipal::local());
    data
}

async fn graphiql(session: Session) -> Response {
    if !session::is_authenticated(&session).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Html(
        async_graphql::http::GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/graphql/ws")
            .finish(),
    )
    .into_response()
}

async fn graphql_schema(State(state): State<WebState>, session: Session) -> Response {
    if !session::is_authenticated(&session).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    plain_response(StatusCode::OK, state.graphql_schema().sdl())
}

async fn bootstrap(
    State(state): State<WebState>,
    Path(capability): Path<String>,
    session_value: Session,
) -> Response {
    if !state.sessions().consume(&capability) {
        return not_found();
    }
    if session::authenticate(&session_value).await.is_err() {
        return plain_response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error");
    }
    (StatusCode::SEE_OTHER, [(header::LOCATION, "/")]).into_response()
}

async fn mcp_oauth_callback(State(state): State<WebState>, RawQuery(query): RawQuery) -> Response {
    let Some(query) = query else {
        return plain_response(StatusCode::BAD_REQUEST, "missing OAuth callback query");
    };
    let Some(attempt_id) = query_value(&query, "attemptId") else {
        return plain_response(StatusCode::BAD_REQUEST, "missing MCP OAuth attempt id");
    };
    let callback_url = oauth_callback_url(state.authority(), &query);

    match crate::graphql::complete_mcp_server_oauth_setup(
        state.graphql_state(),
        &attempt_id,
        &callback_url,
    )
    .await
    {
        Ok(attempt) if attempt.status == "completed" => Html(
            "<!doctype html><title>Noema MCP OAuth</title><p>Authentication completed. You can return to Noema.</p>",
        )
        .into_response(),
        Ok(_) => Html(
            "<!doctype html><title>Noema MCP OAuth</title><p>Authentication finished, but Noema could not list tools. Return to Noema to retry.</p>",
        )
        .into_response(),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Html("<!doctype html><title>Noema MCP OAuth</title><p>Noema could not complete this MCP OAuth setup attempt.</p>"),
        )
            .into_response(),
    }
}

fn oauth_callback_url(authority: &authority::CanonicalAuthority, query: &str) -> String {
    format!("{}/mcp/oauth/callback?{query}", authority.origin())
}

fn query_value(query: &str, key: &str) -> Option<String> {
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}

async fn download_artifact_slug(
    State(state): State<WebState>,
    session_value: Session,
    Path(artifact_version_slug): Path<String>,
) -> Response {
    if !session::is_authenticated(&session_value).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(artifact_version_id) =
        crate::artifact_version_id_from_download_slug(&artifact_version_slug)
    else {
        return not_found();
    };
    download_artifact(&state, &artifact_version_id).await
}

async fn download_artifact_id(
    State(state): State<WebState>,
    session_value: Session,
    Path(artifact_version_id): Path<String>,
) -> Response {
    if !session::is_authenticated(&session_value).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    download_artifact(&state, &artifact_version_id).await
}

async fn download_artifact(state: &WebState, artifact_version_id: &str) -> Response {
    let store = match state.graphql_state().store() {
        Ok(store) => store,
        Err(_) => return internal_error(state, "store_state"),
    };
    let paths = match state.graphql_state().paths() {
        Ok(paths) => paths,
        Err(_) => return internal_error(state, "path_state"),
    };
    let version = match store.get_artifact_version(artifact_version_id).await {
        Ok(Some(version)) => version,
        Ok(None) => return not_found(),
        Err(_) => return internal_error(state, "version_query"),
    };
    let artifact = match store.get_artifact(&version.artifact_id).await {
        Ok(Some(artifact)) => artifact,
        Ok(None) => return not_found(),
        Err(_) => return internal_error(state, "artifact_query"),
    };
    if !matches!(
        version.storage,
        crate::ArtifactVersionStorage::LocalFile { .. }
    ) {
        return not_found();
    }
    let Ok((absolute_path, bytes)) =
        crate::artifacts::read_validated_local_artifact_file(paths, &artifact.artifact, &version)
    else {
        return not_found();
    };
    let Some(filename) = absolute_path.file_name().and_then(|value| value.to_str()) else {
        return not_found();
    };
    let content_type = safe_header_value(
        version
            .media_type
            .as_deref()
            .unwrap_or("application/octet-stream"),
    );
    let content_disposition = safe_header_value(&attachment_content_disposition(filename));
    let mut response = Body::from(bytes).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    response
        .headers_mut()
        .insert(header::CONTENT_DISPOSITION, content_disposition);
    response
}

async fn asset_or_not_found(method: Method, uri: Uri) -> Response {
    if method != Method::GET {
        return not_found();
    }
    embedded_asset(uri.path()).map_or_else(not_found, asset_response)
}

fn asset_response(asset: super::assets::EmbeddedAsset) -> Response {
    let mut response = Body::from(asset.body).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, safe_header_value(asset.content_type));
    response
}

fn plain_response(status: StatusCode, body: impl Into<Cow<'static, str>>) -> Response {
    let mut response = (status, body.into()).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn not_found() -> Response {
    plain_response(StatusCode::NOT_FOUND, NOT_FOUND)
}

async fn method_not_found() -> Response {
    not_found()
}

fn internal_error(state: &WebState, operation: &'static str) -> Response {
    state.record_artifact_failure(operation);
    plain_response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
}

fn safe_header_value(value: &str) -> HeaderValue {
    HeaderValue::from_str(value)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"))
}

fn attachment_content_disposition(filename: &str) -> String {
    let escaped = filename
        .chars()
        .map(|character| match character {
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            _ if character.is_control() => "_".to_string(),
            _ => character.to_string(),
        })
        .collect::<String>();
    format!("attachment; filename=\"{escaped}\"")
}

#[cfg(test)]
mod tests;
