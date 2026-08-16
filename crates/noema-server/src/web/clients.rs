//! Pairing, bearer validation, and client credential HTTP boundaries.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};

use super::{WebState, authority::CanonicalAuthority, session};

const PAIRING_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_PENDING_PAIRINGS: usize = 128;
const MAX_CLIENT_NAME_BYTES: usize = 128;

#[derive(Clone)]
pub(super) struct ClientAuth {
    pairings: Arc<Mutex<HashMap<String, PendingPairing>>>,
}

struct PendingPairing {
    secret: [u8; 32],
    expires_at: Instant,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairingStartResponse {
    pairing_uri: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PairingCompleteRequest {
    pairing_id: String,
    secret: String,
    display_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairingCompleteResponse {
    client_id: String,
    token: String,
}

impl ClientAuth {
    pub(super) fn new() -> Self {
        Self {
            pairings: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn start(&self, authority: &CanonicalAuthority) -> Result<String, StatusCode> {
        if !authority.secure()
            || authority.as_str() == "localhost"
            || authority.as_str().starts_with("localhost:")
        {
            return Err(StatusCode::NOT_IMPLEMENTED);
        }
        let mut secret = [0_u8; 32];
        SystemRandom::new()
            .fill(&mut secret)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let secret_text = URL_SAFE_NO_PAD.encode(secret);
        let now = Instant::now();
        let mut pairings = self
            .pairings
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        pairings.retain(|_, pairing| pairing.expires_at > now);
        if pairings.len() >= MAX_PENDING_PAIRINGS {
            let oldest = pairings
                .iter()
                .min_by_key(|(_, pairing)| pairing.expires_at)
                .map(|(id, _)| id.clone())
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
            pairings.remove(&oldest);
        }
        let pairing_id = random_text()?;
        pairings.insert(
            pairing_id.clone(),
            PendingPairing {
                secret,
                expires_at: now + PAIRING_TTL,
            },
        );
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.append_pair("origin", authority.origin());
        query.append_pair("pairingId", &pairing_id);
        query.append_pair("secret", &secret_text);
        let mut pairing_uri = String::from("noema://pair?");
        pairing_uri.push_str(&query.finish());
        Ok(pairing_uri)
    }

    fn consume(&self, pairing_id: &str, secret_text: &str) -> Result<(), StatusCode> {
        let candidate = decode_secret(secret_text);
        let now = Instant::now();
        let mut pairings = self
            .pairings
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        pairings.retain(|_, pairing| pairing.expires_at > now);
        let Some(pairing) = pairings.get(pairing_id) else {
            return Err(StatusCode::BAD_REQUEST);
        };
        if !constant_time_equal(&pairing.secret, &candidate) {
            return Err(StatusCode::BAD_REQUEST);
        }
        pairings.remove(pairing_id);
        Ok(())
    }

    pub(super) async fn validate_bearer(
        &self,
        store: &noema_store::NoemaStore,
        raw: &str,
    ) -> Result<Option<noema_api::RequestPrincipal>, noema_store::StoreError> {
        let Some(token) = raw.strip_prefix("Bearer ") else {
            return Ok(None);
        };
        if !token.contains('.') && token.len() <= 128 {
            let candidate_hash = digest::digest(&digest::SHA256, token.as_bytes());
            let mut hash = [0_u8; 32];
            hash.copy_from_slice(candidate_hash.as_ref());
            return store
                .active_native_oauth_client(hash, unix_timestamp())
                .await
                .map(|client_id| client_id.map(|id| noema_api::RequestPrincipal::client(&id)));
        }
        let (client_id, secret_text, shape_valid) = token
            .split_once('.')
            .map_or(("", token, false), |(client_id, secret_text)| {
                (client_id, secret_text, true)
            });
        let decoded = URL_SAFE_NO_PAD.decode(secret_text).ok();
        let secret_bytes = decoded.as_deref().unwrap_or(secret_text.as_bytes());
        let candidate_hash = digest::digest(&digest::SHA256, secret_bytes);
        let stored_hash = store
            .active_client_token_hash(client_id)
            .await?
            .unwrap_or([0_u8; 32]);
        let mut candidate_hash_bytes = [0_u8; 32];
        candidate_hash_bytes.copy_from_slice(candidate_hash.as_ref());
        let equal = constant_time_equal(&candidate_hash_bytes, &stored_hash);
        let shape_valid = shape_valid
            && client_id.len() <= 128
            && !client_id.is_empty()
            && decoded.as_ref().is_some_and(|secret| secret.len() == 32);
        Ok((equal && shape_valid).then(|| noema_api::RequestPrincipal::client(client_id)))
    }
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX)
}

pub(super) async fn start_pairing(
    State(state): State<WebState>,
    session_value: tower_sessions::Session,
    principal: Option<axum::extract::Extension<noema_api::RequestPrincipal>>,
) -> Response {
    if principal.is_none()
        && state.auth_mode.requires_session()
        && !session::is_authenticated(&session_value).await
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.client_auth.start(&state.authority) {
        Ok(pairing_uri) => Json(PairingStartResponse { pairing_uri }).into_response(),
        Err(status) => status.into_response(),
    }
}

pub(super) async fn complete_pairing(
    State(state): State<WebState>,
    Json(input): Json<PairingCompleteRequest>,
) -> Response {
    let display_name = input.display_name.trim();
    if display_name.is_empty()
        || display_name.len() > MAX_CLIENT_NAME_BYTES
        || input.pairing_id.is_empty()
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    // Consume before touching SQLite. A failed insertion cannot make the
    // short-lived pairing secret usable a second time.
    if let Err(status) = state.client_auth.consume(&input.pairing_id, &input.secret) {
        return status.into_response();
    }
    let secret = match random_secret() {
        Ok(secret) => secret,
        Err(status) => return status.into_response(),
    };
    let client_id = match random_text() {
        Ok(client_id) => client_id,
        Err(status) => return status.into_response(),
    };
    let token = format!("{client_id}.{}", URL_SAFE_NO_PAD.encode(secret));
    let token_hash = digest::digest(&digest::SHA256, &secret);
    match state
        .store
        .insert_client(
            &client_id,
            "human:local",
            display_name,
            token_hash
                .as_ref()
                .try_into()
                .expect("SHA-256 has 32 bytes"),
        )
        .await
    {
        Ok(_) => Json(PairingCompleteResponse { client_id, token }).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
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
    match state.client_auth.validate_bearer(&state.store, raw).await {
        Ok(Some(principal)) => {
            request.extensions_mut().insert(principal);
            next.run(request).await
        }
        Ok(None) => StatusCode::UNAUTHORIZED.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn decode_secret(secret_text: &str) -> [u8; 32] {
    let mut secret = [0_u8; 32];
    if let Ok(decoded) = URL_SAFE_NO_PAD.decode(secret_text)
        && decoded.len() == secret.len()
    {
        secret.copy_from_slice(&decoded);
    }
    secret
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut difference = 0_u8;
    for index in 0..32 {
        difference |= left[index] ^ right[index];
    }
    difference == 0
}

fn random_secret() -> Result<[u8; 32], StatusCode> {
    let mut bytes = [0_u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(bytes)
}

fn random_text() -> Result<String, StatusCode> {
    let mut bytes = [0_u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use noema_store::StoreConfig;

    #[test]
    fn pairing_secret_is_single_use_and_constant_time_checked() {
        let authority =
            CanonicalAuthority::from_public_origin("https://noema.example", "noema.example")
                .expect("authority");
        let auth = ClientAuth::new();
        let uri = auth.start(&authority).expect("pairing start");
        assert!(uri.starts_with("noema://pair?origin="));
        let parsed = url::Url::parse(&uri).expect("pair uri");
        let query = parsed.query_pairs().collect::<HashMap<_, _>>();
        let pairing_id = query.get("pairingId").expect("pairing id").to_string();
        let secret = query.get("secret").expect("secret").to_string();
        assert!(auth.consume(&pairing_id, "invalid").is_err());
        assert!(auth.consume(&pairing_id, &secret).is_ok());
        let uri = auth.start(&authority).expect("second pairing start");
        let parsed = url::Url::parse(&uri).expect("second pair uri");
        let query = parsed.query_pairs().collect::<HashMap<_, _>>();
        let pairing_id = query
            .get("pairingId")
            .expect("second pairing id")
            .to_string();
        let secret = query.get("secret").expect("second secret").to_string();
        let mut expired_pairing = auth.pairings.lock().expect("pairing registry");
        expired_pairing
            .get_mut(&pairing_id)
            .expect("pending second pairing")
            .expires_at = Instant::now() - Duration::from_secs(1);
        drop(expired_pairing);
        assert!(auth.consume(&pairing_id, &secret).is_err());
    }

    #[tokio::test]
    async fn bearer_validation_rejects_invalid_and_revoked_credentials() {
        let home = TempDir::new().expect("store root");
        let store =
            noema_store::NoemaStore::open(&StoreConfig::new(home.path().join("db.sqlite3")))
                .await
                .expect("store");
        let secret = [9_u8; 32];
        let hash = digest::digest(&digest::SHA256, &secret);
        store
            .insert_client(
                "client-one",
                "human:local",
                "Test client",
                hash.as_ref().try_into().expect("digest length"),
            )
            .await
            .expect("insert client");
        let auth = ClientAuth::new();
        let bearer = format!("Bearer client-one.{}", URL_SAFE_NO_PAD.encode(secret));
        assert_eq!(
            auth.validate_bearer(&store, &bearer)
                .await
                .expect("validate")
                .and_then(|principal| principal.client_id().map(str::to_owned)),
            Some("client-one".to_string())
        );
        let invalid = format!("Bearer client-one.{}", URL_SAFE_NO_PAD.encode([8_u8; 32]));
        assert!(
            auth.validate_bearer(&store, &invalid)
                .await
                .expect("invalid validation")
                .is_none()
        );
        store
            .revoke_client("human:local", "client-one")
            .await
            .expect("revoke");
        assert!(
            auth.validate_bearer(&store, &bearer)
                .await
                .expect("revoked validation")
                .is_none()
        );
    }
}
