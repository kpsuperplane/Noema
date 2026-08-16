//! Axum route composition and HTTP response adaptation for the daemon web UI.

use std::borrow::Cow;

use async_graphql::{Data, http::ALL_WEBSOCKET_PROTOCOLS};
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket};
use axum::{
    Router,
    body::Body,
    extract::{Extension, Path, RawQuery, State, WebSocketUpgrade},
    http::{HeaderValue, Method, StatusCode, Uri, header},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_http::{limit::RequestBodyLimitLayer, set_header::SetResponseHeaderLayer};
use tower_sessions::{MemoryStore, Session, SessionManagerLayer, cookie::SameSite};

use super::{
    MAX_GRAPHQL_BODY_BYTES, WebState, assets::embedded_asset, authority, clients, passkey, session,
};

const MAX_OAUTH_QUERY_BYTES: usize = 8 * 1024;
const MAX_RECOVERY_BODY_BYTES: usize = 1024;
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

    let graphql_route = post(graphql)
        .head(method_not_found)
        .fallback(method_not_found);
    let graphql_route = if state.graphiql_enabled {
        graphql_route.get(graphiql)
    } else {
        graphql_route.get(method_not_found)
    };

    let router = Router::new()
        .route("/graphql", graphql_route)
        .route("/graphql/schema.graphql", get_only!(graphql_schema))
        .route("/graphql/ws", get_only!(graphql_ws))
        .route("/auth/status", get_only!(passkey::status))
        .route(
            "/auth/recovery",
            post(recover).layer(RequestBodyLimitLayer::new(MAX_RECOVERY_BODY_BYTES)),
        )
        .route(
            "/auth/passkey/register/start",
            post(passkey::start_registration),
        )
        .route(
            "/auth/passkey/register/finish",
            post(passkey::finish_registration),
        )
        .route(
            "/auth/passkey/login/start",
            post(passkey::start_authentication),
        )
        .route(
            "/auth/passkey/login/finish",
            post(passkey::finish_authentication),
        )
        .route("/auth/logout", post(passkey::logout))
        .route("/auth/client/pairing/start", post(clients::start_pairing))
        .route(
            "/auth/client/pairing/complete",
            post(clients::complete_pairing),
        )
        .route("/mcp/oauth/callback", get_only!(mcp_oauth_callback))
        .route(
            "/provider/oauth/callback/{attempt_id}",
            get_only!(provider_oauth_callback),
        )
        .route("/adapter/oauth/callback", get_only!(adapter_oauth_callback))
        .route(
            "/artifacts/versions/{artifact_version_slug}/download",
            get_only!(download_artifact_slug),
        )
        .fallback(asset_or_not_found);
    #[cfg(test)]
    let router = router.route("/__test/authenticate", post(authenticate_test_session));

    router
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            clients::authenticate_bearer,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            passkey::enforce_setup_barrier,
        ))
        .layer(session_layer)
        .layer(middleware::from_fn_with_state(
            authority,
            authority::enforce_authority,
        ))
        .with_state(state)
}

#[cfg(test)]
async fn authenticate_test_session(
    State(state): State<WebState>,
    session_value: Session,
) -> Response {
    if state
        .store
        .insert_local_human_passkey("test-passkey", r#"{"test":true}"#)
        .await
        .is_err()
    {
        return internal_error();
    }
    match session::authenticate(&session_value, "test-passkey").await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => internal_error(),
    }
}

async fn graphql(
    State(state): State<WebState>,
    session: Session,
    principal: Option<Extension<noema_api::RequestPrincipal>>,
    request: GraphQLRequest,
) -> Response {
    let Some(principal) = authenticated_principal(&state, &session, principal).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    GraphQLResponse::from(
        state
            .graphql_schema
            .execute(request.into_inner().data(principal))
            .await,
    )
    .into_response()
}

async fn graphql_ws(
    State(state): State<WebState>,
    session: Session,
    principal: Option<Extension<noema_api::RequestPrincipal>>,
    protocol: GraphQLProtocol,
    upgrade: WebSocketUpgrade,
) -> Response {
    let Some(principal) = authenticated_principal(&state, &session, principal).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let schema = state.graphql_schema.clone();
    let revocations = state.store.subscribe_client_revocations();
    upgrade
        .protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |socket| {
            let mut data = Data::default();
            data.insert(principal.clone());
            let serve = GraphQLWebSocket::new(socket, schema, protocol)
                .with_data(data)
                .serve();
            let client_id = principal.client_id().map(str::to_owned);
            async move {
                let Some(client_id) = client_id else {
                    serve.await;
                    return;
                };
                tokio::pin!(serve);
                let mut revocations = revocations;
                loop {
                    tokio::select! {
                        () = &mut serve => break,
                        event = revocations.recv() => match event {
                            Ok(revoked) if revoked == client_id => break,
                            Ok(_) => continue,
                            Err(_) => break,
                        },
                    }
                }
            }
        })
        .into_response()
}

async fn authenticated_principal(
    state: &WebState,
    session: &Session,
    principal: Option<Extension<noema_api::RequestPrincipal>>,
) -> Option<noema_api::RequestPrincipal> {
    if let Some(principal) = principal {
        return Some(principal.0);
    }
    session::request_principal(session, state.auth_mode.requires_session()).await
}

async fn graphiql(
    State(state): State<WebState>,
    session: Session,
    principal: Option<Extension<noema_api::RequestPrincipal>>,
) -> Response {
    if authenticated_principal(&state, &session, principal)
        .await
        .is_none()
    {
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

async fn graphql_schema(
    State(state): State<WebState>,
    session: Session,
    principal: Option<Extension<noema_api::RequestPrincipal>>,
) -> Response {
    if authenticated_principal(&state, &session, principal)
        .await
        .is_none()
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    plain_response(StatusCode::OK, state.graphql_schema.sdl())
}

#[derive(Deserialize)]
struct RecoveryRequest {
    code: String,
}

async fn recover(
    State(state): State<WebState>,
    session_value: Session,
    axum::Json(input): axum::Json<RecoveryRequest>,
) -> Response {
    if !state.auth_mode.requires_session() {
        return not_found();
    }
    let Some(recovery) = state.recovery.clone() else {
        return not_found();
    };
    let matched = tokio::task::spawn_blocking(move || recovery.attempt(&input.code)).await;
    match matched {
        Ok(Ok(true)) => match state.sessions.authorize_setup(&session_value).await {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(()) => internal_error(),
        },
        Ok(Ok(false)) => plain_response(StatusCode::UNAUTHORIZED, "recovery code was not accepted"),
        Ok(Err(_)) | Err(_) => plain_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "recovery is unavailable until Noema restarts",
        ),
    }
}

async fn mcp_oauth_callback(State(state): State<WebState>, RawQuery(query): RawQuery) -> Response {
    let Some(query) = query else {
        return plain_response(StatusCode::BAD_REQUEST, "missing OAuth callback query");
    };
    if query.len() > MAX_OAUTH_QUERY_BYTES {
        return plain_response(StatusCode::BAD_REQUEST, "invalid OAuth callback query");
    }
    let Some(attempt_id) = query_value(&query, "attemptId") else {
        return plain_response(StatusCode::BAD_REQUEST, "missing MCP OAuth attempt id");
    };
    let callback_url = oauth_callback_url(&state.authority, "/mcp/oauth/callback", &query);

    match noema_api::graphql::complete_mcp_server_oauth_setup(
        &state.graphql_state,
        &attempt_id,
        &callback_url,
    )
    .await
    {
        Ok(attempt) if attempt.status == "completed" => Html(
            "<!doctype html><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Noema MCP OAuth</title><main><p>Authentication completed.</p><p><a href=\"/\">Return to Noema</a></p></main>",
        )
        .into_response(),
        Ok(_) => Html(
            "<!doctype html><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Noema MCP OAuth</title><main><p>Authentication finished, but Noema could not list tools.</p><p><a href=\"/\">Return to Noema to retry</a></p></main>",
        )
        .into_response(),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Html("<!doctype html><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Noema MCP OAuth</title><main><p>Noema could not complete this MCP OAuth setup attempt.</p><p><a href=\"/\">Return to Noema</a></p></main>"),
        )
            .into_response(),
    }
}

async fn adapter_oauth_callback(
    State(state): State<WebState>,
    RawQuery(query): RawQuery,
) -> Response {
    let Some(query) = query else {
        return plain_response(StatusCode::BAD_REQUEST, "missing OAuth callback query");
    };
    if query.len() > MAX_OAUTH_QUERY_BYTES {
        return plain_response(StatusCode::BAD_REQUEST, "invalid OAuth callback query");
    }
    let callback_url = oauth_callback_url(&state.authority, "/adapter/oauth/callback", &query);
    match noema_api::graphql::complete_adapter_oauth_setup(&state.graphql_state, &callback_url).await
    {
        Ok(_) => Html(
            "<!doctype html><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Noema OAuth</title><main><p>Authentication completed.</p><p><a href=\"/\">Return to Noema</a></p></main>",
        )
        .into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Html(format!(
                "<!doctype html><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Noema OAuth</title><main><p>{}</p><p><a href=\"/\">Return to Noema</a></p></main>",
                noema_api::graphql::adapter_oauth_failure_message(error)
            )),
        )
            .into_response(),
    }
}

async fn provider_oauth_callback(
    State(state): State<WebState>,
    Path(attempt_id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let Some(query) = query else {
        return plain_response(StatusCode::BAD_REQUEST, "missing OAuth callback query");
    };
    if query.len() > MAX_OAUTH_QUERY_BYTES {
        return plain_response(StatusCode::BAD_REQUEST, "invalid OAuth callback query");
    }
    if query_value(&query, "code").is_none() {
        return plain_response(StatusCode::BAD_REQUEST, "invalid provider OAuth callback");
    }
    let callback_url = oauth_callback_url(
        &state.authority,
        &format!("/provider/oauth/callback/{attempt_id}"),
        &query,
    );
    match noema_api::graphql::complete_provider_oauth_callback(
        &state.graphql_state,
        &callback_url,
    )
    .await
    {
        Ok(_) => Html(
            "<!doctype html><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Noema Provider OAuth</title><main><p>Authentication completed.</p><p><a href=\"/\">Return to Noema</a></p></main>",
        )
        .into_response(),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Html("<!doctype html><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Noema Provider OAuth</title><main><p>Noema could not complete this provider connection.</p><p><a href=\"/\">Return to Noema</a></p></main>"),
        )
            .into_response(),
    }
}

fn oauth_callback_url(
    authority: &authority::CanonicalAuthority,
    path: &str,
    query: &str,
) -> String {
    format!("{}{path}?{query}", authority.origin())
}

fn query_value(query: &str, key: &str) -> Option<String> {
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}

async fn download_artifact_slug(
    State(state): State<WebState>,
    session_value: Session,
    principal: Option<Extension<noema_api::RequestPrincipal>>,
    Path(artifact_version_slug): Path<String>,
) -> Response {
    let Some(principal) = authenticated_principal(&state, &session_value, principal).await else {
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
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, safe_header_value(asset.content_type));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(asset.cache_control),
    );
    if asset.service_worker_allowed {
        headers.insert(
            header::HeaderName::from_static("service-worker-allowed"),
            HeaderValue::from_static("/"),
        );
    }
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
