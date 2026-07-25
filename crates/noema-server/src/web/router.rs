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
    routing::{get, post},
};
use tower_http::{limit::RequestBodyLimitLayer, set_header::SetResponseHeaderLayer};
use tower_sessions::{MemoryStore, Session, SessionManagerLayer, cookie::SameSite};

use super::{WebState, assets::embedded_asset, authority, passkey, session};

const MAX_GRAPHQL_BODY_BYTES: usize = 64 * 1024;
const NOT_FOUND: &str = "not found";
const GRAPHIQL_CSP: &str = "default-src 'none'; script-src 'unsafe-inline' https://unpkg.com; style-src 'unsafe-inline' https://unpkg.com; img-src https://graphql.org; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'";

macro_rules! get_only {
    ($handler:expr) => {
        get($handler)
            .head(method_not_found)
            .fallback(method_not_found)
    };
}

/// Build the single application router served by the daemon.
pub(crate) fn build_router(state: WebState) -> Router {
    let authority = state.authority.clone();
    let session_layer = SessionManagerLayer::new(MemoryStore::default())
        .with_name("noema.sid")
        .with_http_only(true)
        .with_same_site(SameSite::Strict)
        .with_path("/")
        .with_secure(authority.secure())
        .with_private(state.sessions.key());

    Router::new()
        .route(
            "/graphql",
            get(graphiql)
                .post(graphql)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .route("/graphql/schema.graphql", get_only!(graphql_schema))
        .route("/graphql/ws", get_only!(graphql_ws))
        .route("/__noema/bootstrap/{capability}", get_only!(bootstrap))
        .route("/auth/status", get_only!(passkey::status))
        .route("/auth/passkey/register/start", post(passkey::start_registration))
        .route("/auth/passkey/register/finish", post(passkey::finish_registration))
        .route("/auth/passkey/login/start", post(passkey::start_authentication))
        .route("/auth/passkey/login/finish", post(passkey::finish_authentication))
        .route("/auth/logout", post(passkey::logout))
        .route("/mcp/oauth/callback", get_only!(mcp_oauth_callback))
        .route(
            "/artifacts/versions/{artifact_version_slug}/download",
            get_only!(download_artifact_slug),
        )
        .fallback(asset_or_not_found)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
            ),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static(
                "publickey-credentials-create=(self), publickey-credentials-get=(self)",
            ),
        ))
        .layer(RequestBodyLimitLayer::new(MAX_GRAPHQL_BODY_BYTES))
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
    if needs_authentication(&state, &session).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    GraphQLResponse::from(
        state
            .graphql_schema
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
    if needs_authentication(&state, &session).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let schema = state.graphql_schema.clone();
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
    request.data(noema_api::RequestPrincipal::local())
}

fn request_principal_data() -> Data {
    let mut data = Data::default();
    data.insert(noema_api::RequestPrincipal::local());
    data
}

async fn needs_authentication(state: &WebState, session: &Session) -> bool {
    state.auth_mode.requires_session() && !session::is_authenticated(session).await
}

async fn graphiql(State(state): State<WebState>, session: Session) -> Response {
    if needs_authentication(&state, &session).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let mut response = Html(
        async_graphql::http::GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/graphql/ws")
            .finish(),
    )
    .into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(GRAPHIQL_CSP),
    );
    response
}

async fn graphql_schema(State(state): State<WebState>, session: Session) -> Response {
    if needs_authentication(&state, &session).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    plain_response(StatusCode::OK, state.graphql_schema.sdl())
}

async fn bootstrap(
    State(state): State<WebState>,
    Path(capability): Path<String>,
    session_value: Session,
) -> Response {
    match state.store.local_human_passkey().await {
        Ok(Some(_)) => return not_found(),
        Err(_) => return internal_error(),
        Ok(None) => {}
    }
    if !state.sessions.consume(&capability) {
        return not_found();
    }
    #[cfg(test)]
    let result = if state.sessions.test_bootstrap_authenticates() {
        session::authenticate(&session_value).await
    } else {
        session::authorize_setup(&session_value).await
    };
    #[cfg(not(test))]
    let result = session::authorize_setup(&session_value).await;
    if result.is_err() {
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
    let callback_url = oauth_callback_url(&state.authority, &query);

    match noema_api::graphql::complete_mcp_server_oauth_setup(
        &state.graphql_state,
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
    let Some(principal) =
        session::request_principal(&session_value, state.auth_mode.requires_session()).await
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(artifact_version_id) =
        noema_artifacts::artifact_version_id_from_download_slug(&artifact_version_slug)
    else {
        return not_found();
    };
    download_artifact(&state, &principal, &artifact_version_id).await
}

async fn download_artifact(
    state: &WebState,
    principal: &noema_api::RequestPrincipal,
    artifact_version_id: &str,
) -> Response {
    let download = match noema_api::graphql::authorized_artifact_download(
        &state.graphql_state,
        principal,
        artifact_version_id,
    )
    .await
    {
        Ok(Some(download)) => download,
        Ok(None) => return not_found(),
        Err(_) => return internal_error(),
    };
    artifact_download_response(download)
}

fn artifact_download_response(
    download: noema_api::graphql::AuthorizedArtifactDownload,
) -> Response {
    let content_type = safe_header_value(&download.media_type);
    let content_disposition =
        safe_header_value(&attachment_content_disposition(&download.filename));
    let mut response = Body::from(download.bytes).into_response();
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

fn internal_error() -> Response {
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
