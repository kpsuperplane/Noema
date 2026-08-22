//! FIDO passkey enrollment and authentication for the built-in local human.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use webauthn_rs::prelude::*;

use super::{WebState, authority::CanonicalAuthority, session};

const LOCAL_HUMAN_UUID: Uuid = Uuid::from_u128(0x6e6f656d_6100_4000_8000_000000000001);
const CEREMONY_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_PENDING_CEREMONIES: usize = 64;

#[derive(Clone)]
pub(super) struct PasskeySecurity {
    webauthn: Webauthn,
    ceremonies: Arc<Mutex<HashMap<String, PendingCeremony>>>,
}

enum CeremonyState {
    Registration(PasskeyRegistration),
    Authentication {
        state: PasskeyAuthentication,
        credentials: Vec<noema_store::HumanPasskeyRecord>,
    },
}

struct PendingCeremony {
    browser_binding: String,
    expires_at: Instant,
    state: CeremonyState,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CeremonyStart<T> {
    ceremony_id: String,
    options: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinishRegistration {
    ceremony_id: String,
    credential: RegisterPublicKeyCredential,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinishAuthentication {
    ceremony_id: String,
    credential: PublicKeyCredential,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum BrowserAuthState {
    Authenticated,
    SetupRequired,
    SetupReady,
    LoginRequired,
}

#[derive(Serialize)]
struct BrowserAuthStatus {
    state: BrowserAuthState,
    mode: BrowserAuthMode,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum BrowserAuthMode {
    Required,
    Disabled,
}

#[derive(Serialize)]
struct AuthError {
    error: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PasskeySummary {
    credential_id: String,
    current: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemovePasskeyRequest {
    credential_id: String,
}

impl PasskeySecurity {
    pub(super) fn new(authority: &CanonicalAuthority) -> Result<Self, String> {
        let origin = Url::parse(authority.origin())
            .map_err(|_| "web public_origin could not configure passkeys")?;
        let webauthn = WebauthnBuilder::new(authority.rp_id(), &origin)
            .map_err(|_| "web public_origin is not valid for its passkey relying-party id")?
            .rp_name("Noema")
            .build()
            .map_err(|_| "passkey verifier configuration failed")?;
        Ok(Self {
            webauthn,
            ceremonies: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn insert(&self, browser_binding: String, state: CeremonyState) -> Result<String, StatusCode> {
        let mut ceremonies = self
            .ceremonies
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let now = Instant::now();
        ceremonies.retain(|_, ceremony| {
            ceremony.expires_at > now && ceremony.browser_binding != browser_binding
        });
        if ceremonies.len() >= MAX_PENDING_CEREMONIES {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        let ceremony_id = random_id().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        ceremonies.insert(
            ceremony_id.clone(),
            PendingCeremony {
                browser_binding,
                expires_at: now + CEREMONY_TTL,
                state,
            },
        );
        Ok(ceremony_id)
    }

    fn take(&self, ceremony_id: &str, browser_binding: &str) -> Result<CeremonyState, StatusCode> {
        let mut ceremonies = self
            .ceremonies
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let Some(ceremony) = ceremonies.get(ceremony_id) else {
            return Err(StatusCode::BAD_REQUEST);
        };
        if ceremony.expires_at <= Instant::now() {
            ceremonies.remove(ceremony_id);
            return Err(StatusCode::BAD_REQUEST);
        }
        if ceremony.browser_binding != browser_binding {
            return Err(StatusCode::BAD_REQUEST);
        }
        Ok(ceremonies
            .remove(ceremony_id)
            .expect("checked ceremony must remain present")
            .state)
    }

    fn clear(&self) {
        if let Ok(mut ceremonies) = self.ceremonies.lock() {
            ceremonies.clear();
        }
    }
}

pub(super) async fn enforce_setup_barrier(
    State(state): State<WebState>,
    request: Request,
    next: Next,
) -> Response {
    if !state.auth_mode.requires_session() {
        return next.run(request).await;
    }
    let has_passkey = match state.store.local_human_has_passkey().await {
        Ok(has_passkey) => has_passkey,
        Err(_) => {
            return auth_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
            );
        }
    };
    if has_passkey || setup_path_allowed(request.method(), request.uri().path()) {
        return next.run(request).await;
    }
    auth_error(StatusCode::FORBIDDEN, "setup_required")
}

fn setup_path_allowed(method: &Method, path: &str) -> bool {
    #[cfg(test)]
    if method == Method::POST && path == "/__test/authenticate" {
        return true;
    }
    if matches!(
        (method, path),
        (&Method::GET, "/auth/status")
            | (&Method::GET, "/")
            | (&Method::POST, "/auth/recovery")
            | (&Method::POST, "/auth/passkey/register/start")
            | (&Method::POST, "/auth/passkey/register/finish")
    ) {
        return true;
    }
    matches!(method, &Method::GET | &Method::HEAD) && path.starts_with("/assets/")
}

pub(super) async fn status(State(state): State<WebState>, browser: Session) -> Response {
    if !state.auth_mode.requires_session() || session::is_authenticated(&browser).await {
        return Json(BrowserAuthStatus {
            state: BrowserAuthState::Authenticated,
            mode: if state.auth_mode.requires_session() {
                BrowserAuthMode::Required
            } else {
                BrowserAuthMode::Disabled
            },
        })
        .into_response();
    }
    let credential_exists = match state.store.local_human_has_passkey().await {
        Ok(exists) => exists,
        Err(_) => {
            return auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_unavailable",
            );
        }
    };
    let auth_state = if credential_exists {
        BrowserAuthState::LoginRequired
    } else if state.sessions.is_setup_authorized(&browser).await {
        BrowserAuthState::SetupReady
    } else {
        BrowserAuthState::SetupRequired
    };
    Json(BrowserAuthStatus {
        state: auth_state,
        mode: BrowserAuthMode::Required,
    })
    .into_response()
}

pub(super) async fn start_registration(
    State(state): State<WebState>,
    browser: Session,
) -> Response {
    if !registration_start_authorized(&state, &browser).await {
        return auth_error(StatusCode::FORBIDDEN, "setup_not_authorized");
    }
    let stored = match state.store.local_human_passkeys().await {
        Ok(stored) => stored,
        Err(_) => {
            return auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_unavailable",
            );
        }
    };
    let passkeys = match deserialize_passkeys(&stored) {
        Ok(passkeys) => passkeys,
        Err(response) => return response,
    };
    let excluded = (!passkeys.is_empty()).then(|| {
        passkeys
            .iter()
            .map(|passkey| passkey.cred_id().clone())
            .collect()
    });
    let (options, registration) = match state.passkeys.webauthn.start_passkey_registration(
        LOCAL_HUMAN_UUID,
        "human:local",
        "You",
        excluded,
    ) {
        Ok(result) => result,
        Err(_) => return auth_error(StatusCode::INTERNAL_SERVER_ERROR, "ceremony_unavailable"),
    };
    let binding = match session::browser_binding(&browser).await {
        Some(binding) => binding,
        None => return auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable"),
    };
    match state
        .passkeys
        .insert(binding, CeremonyState::Registration(registration))
    {
        Ok(ceremony_id) => Json(CeremonyStart {
            ceremony_id,
            options,
        })
        .into_response(),
        Err(status) => auth_error(status, "ceremony_unavailable"),
    }
}

pub(super) async fn finish_registration(
    State(state): State<WebState>,
    browser: Session,
    Json(input): Json<FinishRegistration>,
) -> Response {
    if !registration_authorized(&state, &browser).await {
        return auth_error(StatusCode::FORBIDDEN, "setup_not_authorized");
    }
    let binding = match session::browser_binding(&browser).await {
        Some(binding) => binding,
        None => return auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable"),
    };
    let CeremonyState::Registration(registration) =
        (match state.passkeys.take(&input.ceremony_id, &binding) {
            Ok(ceremony) => ceremony,
            Err(status) => return auth_error(status, "invalid_ceremony"),
        })
    else {
        return auth_error(StatusCode::BAD_REQUEST, "invalid_ceremony");
    };
    let passkey = match state
        .passkeys
        .webauthn
        .finish_passkey_registration(&input.credential, &registration)
    {
        Ok(passkey) => passkey,
        Err(_) => return auth_error(StatusCode::UNAUTHORIZED, "passkey_rejected"),
    };
    let credential_json = match serde_json::to_string(&passkey) {
        Ok(serialized) => serialized,
        Err(_) => {
            return auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_unavailable",
            );
        }
    };
    let credential_id = URL_SAFE_NO_PAD.encode(passkey.cred_id().as_ref());
    match state
        .store
        .insert_local_human_passkey(&credential_id, &credential_json)
        .await
    {
        Ok(true) => {}
        Ok(false) => return auth_error(StatusCode::CONFLICT, "passkey_already_registered"),
        Err(_) => {
            return auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_unavailable",
            );
        }
    }
    state.sessions.consume_setup(&browser).await;
    establish_session(&state, &browser, &credential_id).await
}

pub(super) async fn start_authentication(
    State(state): State<WebState>,
    browser: Session,
) -> Response {
    let credentials = match state.store.local_human_passkeys().await {
        Ok(credentials) if credentials.is_empty() => {
            return auth_error(StatusCode::CONFLICT, "setup_required");
        }
        Ok(credentials) => credentials,
        Err(_) => {
            return auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_unavailable",
            );
        }
    };
    let passkeys = match deserialize_passkeys(&credentials) {
        Ok(passkeys) => passkeys,
        Err(response) => return response,
    };
    let (options, authentication) = match state
        .passkeys
        .webauthn
        .start_passkey_authentication(&passkeys)
    {
        Ok(result) => result,
        Err(_) => return auth_error(StatusCode::INTERNAL_SERVER_ERROR, "ceremony_unavailable"),
    };
    let binding = match session::browser_binding(&browser).await {
        Some(binding) => binding,
        None => return auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable"),
    };
    match state.passkeys.insert(
        binding,
        CeremonyState::Authentication {
            state: authentication,
            credentials,
        },
    ) {
        Ok(ceremony_id) => Json(CeremonyStart {
            ceremony_id,
            options,
        })
        .into_response(),
        Err(status) => auth_error(status, "ceremony_unavailable"),
    }
}

pub(super) async fn finish_authentication(
    State(state): State<WebState>,
    browser: Session,
    Json(input): Json<FinishAuthentication>,
) -> Response {
    let binding = match session::browser_binding(&browser).await {
        Some(binding) => binding,
        None => return auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable"),
    };
    let CeremonyState::Authentication {
        state: authentication,
        credentials,
    } = (match state.passkeys.take(&input.ceremony_id, &binding) {
        Ok(ceremony) => ceremony,
        Err(status) => return auth_error(status, "invalid_ceremony"),
    })
    else {
        return auth_error(StatusCode::BAD_REQUEST, "invalid_ceremony");
    };
    let result = match state
        .passkeys
        .webauthn
        .finish_passkey_authentication(&input.credential, &authentication)
    {
        Ok(result) => result,
        Err(_) => return auth_error(StatusCode::UNAUTHORIZED, "passkey_rejected"),
    };
    let credential_id = URL_SAFE_NO_PAD.encode(result.cred_id().as_ref());
    let Some(credential) = credentials
        .iter()
        .find(|credential| credential.credential_id == credential_id)
    else {
        return auth_error(StatusCode::UNAUTHORIZED, "passkey_rejected");
    };
    let current = match state.store.local_human_passkey(&credential_id).await {
        Ok(Some(current)) => current,
        Ok(None) => return auth_error(StatusCode::CONFLICT, "credential_changed"),
        Err(_) => {
            return auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_unavailable",
            );
        }
    };
    if current.credential_json != credential.credential_json {
        return auth_error(StatusCode::CONFLICT, "credential_changed");
    }
    let mut passkey: Passkey = match serde_json::from_str(&credential.credential_json) {
        Ok(passkey) => passkey,
        Err(_) => {
            return auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_unavailable",
            );
        }
    };
    if passkey.update_credential(&result) == Some(true) {
        let replacement = match serde_json::to_string(&passkey) {
            Ok(replacement) => replacement,
            Err(_) => {
                return auth_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "authentication_unavailable",
                );
            }
        };
        match state
            .store
            .compare_and_swap_local_human_passkey(
                &credential_id,
                &credential.credential_json,
                &replacement,
            )
            .await
        {
            Ok(true) => {}
            Ok(false) => return auth_error(StatusCode::CONFLICT, "credential_changed"),
            Err(_) => {
                return auth_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "authentication_unavailable",
                );
            }
        }
    }
    establish_session(&state, &browser, &credential_id).await
}

pub(super) async fn logout(State(state): State<WebState>, browser: Session) -> Response {
    if let Some(session_hash) = session::session_hash(&browser)
        && state
            .store
            .remove_web_push_for_session(session_hash)
            .await
            .is_err()
    {
        return auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable");
    }
    match session::logout(&browser).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable"),
    }
}

pub(super) async fn logout_all(State(state): State<WebState>, browser: Session) -> Response {
    if state.auth_mode.requires_session() && !session::is_authenticated(&browser).await {
        return auth_error(StatusCode::UNAUTHORIZED, "authentication_required");
    }
    if state
        .store
        .remove_all_web_push_subscriptions()
        .await
        .is_err()
    {
        return auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable");
    }
    if state.sessions.revoke_all().await.is_err() {
        return auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable");
    }
    match session::logout(&browser).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable"),
    }
}

pub(super) async fn list_passkeys(State(state): State<WebState>, browser: Session) -> Response {
    if state.auth_mode.requires_session() && !session::is_authenticated(&browser).await {
        return auth_error(StatusCode::UNAUTHORIZED, "authentication_required");
    }
    let current = session::authenticating_passkey(&browser).await;
    match state.store.local_human_passkeys().await {
        Ok(passkeys) => Json(
            passkeys
                .into_iter()
                .map(|passkey| PasskeySummary {
                    current: current.as_deref() == Some(passkey.credential_id.as_str()),
                    credential_id: passkey.credential_id,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(_) => auth_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "authentication_unavailable",
        ),
    }
}

pub(super) async fn remove_passkey(
    State(state): State<WebState>,
    browser: Session,
    Json(input): Json<RemovePasskeyRequest>,
) -> Response {
    if state.auth_mode.requires_session()
        && (!session::is_authenticated(&browser).await
            || !session::has_recent_passkey(&browser).await)
    {
        return auth_error(StatusCode::FORBIDDEN, "recent_passkey_required");
    }
    match state
        .store
        .remove_local_human_passkey(&input.credential_id)
        .await
    {
        Ok(noema_store::HumanPasskeyRemoval::Removed { browser_sessions }) => {
            state.passkeys.clear();
            state
                .sessions
                .announce_passkey_revocations(browser_sessions);
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(noema_store::HumanPasskeyRemoval::NotFound) => {
            auth_error(StatusCode::NOT_FOUND, "passkey_not_found")
        }
        Ok(noema_store::HumanPasskeyRemoval::FinalPasskey) => {
            auth_error(StatusCode::CONFLICT, "final_passkey_required")
        }
        Err(_) => auth_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "authentication_unavailable",
        ),
    }
}

async fn establish_session(state: &WebState, browser: &Session, credential_id: &str) -> Response {
    if let Some(session_hash) = session::session_hash(browser)
        && state
            .store
            .remove_web_push_for_session(session_hash)
            .await
            .is_err()
    {
        return auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable");
    }
    match session::authenticate(browser, credential_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => auth_error(StatusCode::INTERNAL_SERVER_ERROR, "session_unavailable"),
    }
}

async fn registration_authorized(state: &WebState, browser: &Session) -> bool {
    if !state.auth_mode.requires_session() || state.sessions.is_setup_authorized(browser).await {
        return true;
    }
    session::is_authenticated(browser).await && session::has_recent_passkey(browser).await
}

async fn registration_start_authorized(state: &WebState, browser: &Session) -> bool {
    if !state.auth_mode.requires_session() {
        return true;
    }
    if session::is_authenticated(browser).await && session::has_recent_passkey(browser).await {
        return true;
    }
    state.sessions.claim_setup_registration(browser).await
}

fn deserialize_passkeys(
    stored: &[noema_store::HumanPasskeyRecord],
) -> Result<Vec<Passkey>, Response> {
    stored
        .iter()
        .map(|credential| serde_json::from_str(&credential.credential_json))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_unavailable",
            )
        })
}

fn auth_error(status: StatusCode, error: &'static str) -> Response {
    (status, Json(AuthError { error })).into_response()
}

fn random_id() -> Result<String, ring::error::Unspecified> {
    let mut bytes = [0_u8; 32];
    SystemRandom::new().fill(&mut bytes)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registration(security: &PasskeySecurity) -> CeremonyState {
        let (_, registration) = security
            .webauthn
            .start_passkey_registration(LOCAL_HUMAN_UUID, "human:local", "You", None)
            .expect("registration");
        CeremonyState::Registration(registration)
    }

    #[test]
    fn ceremony_is_session_bound_and_consumed_once() {
        let authority =
            CanonicalAuthority::from_public_origin("https://noema.example", "noema.example")
                .expect("authority");
        let security = PasskeySecurity::new(&authority).expect("passkey security");
        let ceremony_id = security
            .insert("expected-browser".to_string(), registration(&security))
            .expect("insert ceremony");

        assert!(security.take(&ceremony_id, "different-browser").is_err());
        assert!(matches!(
            security
                .take(&ceremony_id, "expected-browser")
                .expect("bound browser consumes ceremony"),
            CeremonyState::Registration(_)
        ));
        assert!(security.take(&ceremony_id, "expected-browser").is_err());
    }

    #[test]
    fn full_ceremony_registry_rejects_new_work_without_evicting_active_work() {
        let authority =
            CanonicalAuthority::from_public_origin("https://noema.example", "noema.example")
                .expect("authority");
        let security = PasskeySecurity::new(&authority).expect("passkey security");
        let mut first = None;
        for index in 0..MAX_PENDING_CEREMONIES {
            let id = security
                .insert(format!("browser-{index}"), registration(&security))
                .expect("ceremony");
            first.get_or_insert(id);
        }
        assert_eq!(
            security.insert("extra-browser".to_string(), registration(&security)),
            Err(StatusCode::SERVICE_UNAVAILABLE)
        );
        assert!(
            security
                .take(first.as_deref().expect("first"), "browser-0")
                .is_ok()
        );
    }
}
