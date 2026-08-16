//! Native bearer validation at the public HTTP boundary.

use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use ring::digest;

use super::WebState;

#[derive(Clone, Debug)]
pub(super) struct NativeBearerAuth {
    pub client_id: String,
    pub expires_at: i64,
}

#[cfg(test)]
pub(super) async fn validate_bearer(
    store: &noema_store::NoemaStore,
    raw: &str,
) -> Result<Option<noema_api::RequestPrincipal>, noema_store::StoreError> {
    validate_bearer_access(store, raw)
        .await
        .map(|access| access.map(|access| noema_api::RequestPrincipal::client(access.client_id)))
}

async fn validate_bearer_access(
    store: &noema_store::NoemaStore,
    raw: &str,
) -> Result<Option<NativeBearerAuth>, noema_store::StoreError> {
    let Some(token) = raw.strip_prefix("Bearer ") else {
        return Ok(None);
    };
    if token.is_empty() || token.contains('.') || token.len() > 128 {
        return Ok(None);
    }
    let candidate = digest::digest(&digest::SHA256, token.as_bytes());
    let mut hash = [0_u8; 32];
    hash.copy_from_slice(candidate.as_ref());
    store
        .active_native_oauth_client(hash, unix_timestamp())
        .await
        .map(|access| {
            access.map(|access| NativeBearerAuth {
                client_id: access.client_id,
                expires_at: access.expires_at,
            })
        })
}

pub(super) fn bearer_header(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
}

pub(super) async fn authenticate_bearer(
    State(state): State<WebState>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(raw) = bearer_header(request.headers()) else {
        return next.run(request).await;
    };
    match validate_bearer_access(&state.store, raw).await {
        Ok(Some(access)) => {
            let principal = noema_api::RequestPrincipal::client(&access.client_id);
            request.extensions_mut().insert(access);
            request.extensions_mut().insert(principal);
            next.run(request).await
        }
        Ok(None) => StatusCode::UNAUTHORIZED.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(super) fn unix_timestamp() -> i64 {
    std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX)
}
