//! First-party native OAuth endpoints and protocol adapters.

use std::{collections::HashMap, time::SystemTime};

use axum::{
    body::{Body as AxumBody, Bytes},
    extract::{RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, TimeZone as _, Utc};
use oxide_auth::{
    endpoint::{
        AccessTokenFlow, AuthorizationFlow, OwnerConsent, OwnerSolicitor, RefreshFlow, Solicitation,
    },
    frontends::simple::{
        endpoint::{Generic, Vacant},
        extensions::{AddonList, Extended, Pkce},
        request::{Body, Request as OAuthRequest, Response as OAuthResponse, Status},
    },
    primitives::{
        authorizer::Authorizer,
        grant::{Extensions, Grant, Value},
        issuer::{IssuedToken, Issuer, RefreshedToken, TokenType},
        registrar::{Client, ClientMap, ExactUrl, RegisteredUrl},
    },
};
use ring::{digest, rand::SecureRandom as _};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use super::{WebState, session};

const LOCAL_HUMAN_ID: &str = "human:local";
const SCOPE: &str = "noema";
const PENDING_KEY: &str = "native_oauth_pending";
const ACCESS_TTL_SECONDS: i64 = 15 * 60;
const IDLE_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
const ABSOLUTE_TTL_SECONDS: i64 = 180 * 24 * 60 * 60;

#[derive(Deserialize)]
struct ApprovalForm {
    csrf: String,
    decision: String,
}

#[derive(Serialize, Deserialize)]
struct PendingAuthorization {
    query: String,
    csrf: String,
}

pub(super) async fn authorize(
    State(state): State<WebState>,
    browser: Session,
    RawQuery(query): RawQuery,
) -> Response {
    let query = match query {
        Some(query) if query.len() <= super::router::MAX_OAUTH_QUERY_BYTES => query,
        Some(_) => return oauth_error(StatusCode::BAD_REQUEST, "invalid_request"),
        None => match browser
            .remove::<String>(session::NATIVE_OAUTH_RESUME_KEY)
            .await
        {
            Ok(Some(query)) => query,
            _ => return oauth_error(StatusCode::BAD_REQUEST, "invalid_request"),
        },
    };
    let Ok(parameters) = unique_parameters(query.as_bytes()) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if validate_request_shape(&parameters).is_err()
        || run_authorization(parameters, Consent::Pending).is_err()
    {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    if state.auth_mode.requires_session()
        && (!session::is_authenticated(&browser).await
            || !session::has_recent_passkey(&browser).await)
    {
        if browser
            .insert(session::NATIVE_OAUTH_RESUME_KEY, query)
            .await
            .is_err()
        {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        return Redirect::to("/?native_authorization=resume").into_response();
    }
    let _ = browser
        .remove::<String>(session::NATIVE_OAUTH_RESUME_KEY)
        .await;
    let Ok(csrf) = random_text(32) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if browser
        .insert(
            PENDING_KEY,
            PendingAuthorization {
                query,
                csrf: csrf.clone(),
            },
        )
        .await
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    Html(format!(
        "<!doctype html><html><head><meta charset=utf-8><title>Authorize Noema</title></head><body><main><h1>Authorize native access</h1><p>The native Noema client will receive complete access to this instance.</p><form method=post action=/oauth/authorize><input type=hidden name=csrf value={csrf}><button name=decision value=approve>Authorize</button><button name=decision value=deny>Deny</button></form></main></body></html>"
    ))
    .into_response()
}

pub(super) async fn approve(
    State(state): State<WebState>,
    browser: Session,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !form_content_type(&headers)
        || (state.auth_mode.requires_session()
            && (!session::is_authenticated(&browser).await
                || !session::has_recent_passkey(&browser).await))
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(form) = serde_urlencoded::from_bytes::<ApprovalForm>(&body) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Ok(Some(pending)) = browser.remove::<PendingAuthorization>(PENDING_KEY).await else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if !secret_equal(form.csrf.as_bytes(), pending.csrf.as_bytes()) {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    let Ok(parameters) = unique_parameters(pending.query.as_bytes()) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let consent = match form.decision.as_str() {
        "approve" => Consent::Approve,
        "deny" => Consent::Deny,
        _ => return oauth_error(StatusCode::BAD_REQUEST, "invalid_request"),
    };
    let Ok((response, issued)) = run_authorization(parameters, consent) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if let Some((raw_code, grant)) = issued {
        let Some(pkce_value) = private_pkce(&grant.extensions) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        let display_name = display_name(&grant.client_id);
        let code = noema_store::NewNativeOAuthCode {
            client_id: &grant.client_id,
            display_name,
            redirect_uri: grant.redirect_uri.as_str(),
            pkce_value: &pkce_value,
            expires_at: grant.until.timestamp(),
        };
        if state
            .store
            .insert_native_oauth_code(sha256(&raw_code), code, now())
            .await
            .is_err()
        {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    adapt_response(response)
}

pub(super) async fn token(
    State(state): State<WebState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !form_content_type(&headers) {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    let Ok(parameters) = unique_parameters(&body) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    match parameters.get("grant_type").map(String::as_str) {
        Some("authorization_code") => exchange_code(&state, parameters).await,
        Some("refresh_token") => refresh(&state, parameters).await,
        _ => oauth_error(StatusCode::BAD_REQUEST, "unsupported_grant_type"),
    }
}

pub(super) async fn revoke(
    State(state): State<WebState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !form_content_type(&headers) {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    let Ok(parameters) = unique_parameters(&body) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if let Some(token) = parameters.get("token") {
        if state
            .store
            .revoke_native_oauth_family(sha256(token), now())
            .await
            .is_err()
        {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    StatusCode::OK.into_response()
}

async fn exchange_code(state: &WebState, parameters: HashMap<String, String>) -> Response {
    let Some(raw_code) = parameters.get("code") else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Ok(registrar) = registrar(&parameters) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Ok(Some(stored)) = state
        .store
        .consume_native_oauth_code(sha256(raw_code), now())
        .await
    else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let Some(grant) = stored_grant(stored) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let mut authorizer = OneShotAuthorizer::new(raw_code, grant);
    let mut issuer = TokenIssuer::default();
    let response = {
        let endpoint = Generic {
            registrar: &registrar,
            authorizer: &mut authorizer,
            issuer: &mut issuer,
            solicitor: Vacant,
            scopes: Vacant,
            response: Vacant,
        };
        let mut addons = AddonList::new();
        addons.push_code(Pkce::required());
        let mut endpoint = Extended::extend_with(endpoint, addons);
        let Ok(mut flow) = AccessTokenFlow::prepare(&mut endpoint) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        flow.execute(OAuthRequest {
            urlbody: parameters,
            ..OAuthRequest::default()
        })
    };
    let Ok(response) = response else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if response.status == Status::Ok {
        let Some(tokens) = issuer.issued.take() else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        let Ok(family_id) = random_hex(16) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        let issued_at = now();
        let family = noema_store::NewNativeOAuthFamily {
            family_id: &family_id,
            client_id: &tokens.client_id,
            access_hash: sha256(&tokens.access),
            refresh_hash: sha256(&tokens.refresh),
            issued_at,
            access_expires_at: issued_at + ACCESS_TTL_SECONDS,
            idle_expires_at: issued_at + IDLE_TTL_SECONDS,
            absolute_expires_at: issued_at + ABSOLUTE_TTL_SECONDS,
        };
        if state
            .store
            .insert_native_oauth_family(family)
            .await
            .is_err()
        {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    adapt_response(response)
}

async fn refresh(state: &WebState, parameters: HashMap<String, String>) -> Response {
    let Some(raw_refresh) = parameters.get("refresh_token").cloned() else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let lookup = state
        .store
        .native_oauth_refresh_grant(sha256(&raw_refresh), now())
        .await;
    let Ok(noema_store::NativeOAuthRefreshLookup::Active(stored)) = lookup else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let Some(grant) = refresh_grant(&stored) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let mut registrar = ClientMap::new();
    registrar.register_client(Client::public(
        &stored.client_id,
        RegisteredUrl::Exact(ExactUrl::new("noema:/oauth/grant".to_string()).expect("fixed URL")),
        SCOPE.parse().expect("fixed scope"),
    ));
    let mut issuer = TokenIssuer::recovering(&raw_refresh, grant);
    let response = {
        let endpoint = Generic {
            registrar: &registrar,
            authorizer: Vacant,
            issuer: &mut issuer,
            solicitor: Vacant,
            scopes: Vacant,
            response: Vacant,
        };
        let Ok(mut flow) = RefreshFlow::prepare(endpoint) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        flow.execute(OAuthRequest {
            urlbody: parameters,
            ..OAuthRequest::default()
        })
    };
    let Ok(response) = response else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if response.status == Status::Ok {
        let Some(tokens) = issuer.issued.take() else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        let issued_at = now();
        let outcome = state
            .store
            .rotate_native_oauth_refresh(
                sha256(&raw_refresh),
                noema_store::NativeOAuthRotation {
                    access_hash: sha256(&tokens.access),
                    refresh_hash: sha256(&tokens.refresh),
                    issued_at,
                    access_expires_at: issued_at + ACCESS_TTL_SECONDS,
                    idle_expires_at: issued_at + IDLE_TTL_SECONDS,
                },
                issued_at,
            )
            .await;
        if !matches!(
            outcome,
            Ok(noema_store::NativeOAuthRotationOutcome::Rotated)
        ) {
            return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
        }
    }
    adapt_response(response)
}

#[derive(Clone, Copy)]
enum Consent {
    Pending,
    Approve,
    Deny,
}

fn run_authorization(
    parameters: HashMap<String, String>,
    consent: Consent,
) -> Result<(OAuthResponse, Option<(String, Grant)>), ()> {
    let registrar = registrar(&parameters)?;
    let mut authorizer = CodeAuthorizer::default();
    let mut solicitor = ConsentSolicitor(consent);
    let response = {
        let endpoint = Generic {
            registrar: &registrar,
            authorizer: &mut authorizer,
            issuer: Vacant,
            solicitor: &mut solicitor,
            scopes: Vacant,
            response: Vacant,
        };
        let mut addons = AddonList::new();
        addons.push_code(Pkce::required());
        let mut endpoint = Extended::extend_with(endpoint, addons);
        AuthorizationFlow::<_, OAuthRequest>::prepare(&mut endpoint)
            .map_err(|_| ())?
            .execute(OAuthRequest {
                query: parameters,
                ..OAuthRequest::default()
            })
            .map_err(|_| ())?
    };
    Ok((response, authorizer.issued))
}

struct ConsentSolicitor(Consent);

impl OwnerSolicitor<OAuthRequest> for ConsentSolicitor {
    fn check_consent(
        &mut self,
        _: &mut OAuthRequest,
        _: Solicitation<'_>,
    ) -> OwnerConsent<OAuthResponse> {
        match self.0 {
            Consent::Pending => OwnerConsent::InProgress(OAuthResponse::default()),
            Consent::Approve => OwnerConsent::Authorized(LOCAL_HUMAN_ID.to_string()),
            Consent::Deny => OwnerConsent::Denied,
        }
    }
}

fn registrar(parameters: &HashMap<String, String>) -> Result<ClientMap, ()> {
    validate_request_shape(parameters)?;
    let client_id = parameters.get("client_id").ok_or(())?;
    let redirect = parameters.get("redirect_uri").ok_or(())?;
    validate_redirect(client_id, redirect)?;
    let mut registrar = ClientMap::new();
    registrar.register_client(Client::public(
        client_id,
        RegisteredUrl::Exact(ExactUrl::new(redirect.clone()).map_err(|_| ())?),
        SCOPE.parse().map_err(|_| ())?,
    ));
    Ok(registrar)
}

fn validate_request_shape(parameters: &HashMap<String, String>) -> Result<(), ()> {
    let client_id = parameters.get("client_id").ok_or(())?;
    let suffix = client_id
        .strip_prefix("noema-desktop:")
        .or_else(|| client_id.strip_prefix("noema-ios:"))
        .ok_or(())?;
    if !(16..=96).contains(&suffix.len())
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        || parameters.get("redirect_uri").is_none()
    {
        return Err(());
    }
    if let Some(value) = parameters.get("scope") {
        if value != SCOPE {
            return Err(());
        }
    }
    if parameters.contains_key("response_type") {
        let state = parameters.get("state").ok_or(())?;
        let challenge = parameters.get("code_challenge").ok_or(())?;
        if parameters.get("response_type").map(String::as_str) != Some("code")
            || parameters.get("code_challenge_method").map(String::as_str) != Some("S256")
            || !(32..=256).contains(&state.len())
            || challenge.len() != 43
            || !challenge
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(());
        }
    }
    Ok(())
}

fn validate_redirect(client_id: &str, redirect: &str) -> Result<(), ()> {
    if client_id.starts_with("noema-ios:") {
        return (redirect == "noema://oauth/callback")
            .then_some(())
            .ok_or(());
    }
    let parsed = url::Url::parse(redirect).map_err(|_| ())?;
    let loopback = parsed.host_str() == Some("127.0.0.1") || parsed.host_str() == Some("::1");
    (parsed.scheme() == "http"
        && loopback
        && parsed.port().is_some()
        && parsed.path() == "/oauth/callback"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none())
    .then_some(())
    .ok_or(())
}

#[derive(Default)]
struct CodeAuthorizer {
    issued: Option<(String, Grant)>,
}

impl Authorizer for CodeAuthorizer {
    fn authorize(&mut self, grant: Grant) -> Result<String, ()> {
        let code = random_text(32)?;
        self.issued = Some((code.clone(), grant));
        Ok(code)
    }

    fn extract(&mut self, _: &str) -> Result<Option<Grant>, ()> {
        Ok(None)
    }
}

struct OneShotAuthorizer {
    code: String,
    grant: Option<Grant>,
}

impl OneShotAuthorizer {
    fn new(code: &str, grant: Grant) -> Self {
        Self {
            code: code.to_string(),
            grant: Some(grant),
        }
    }
}

impl Authorizer for OneShotAuthorizer {
    fn authorize(&mut self, _: Grant) -> Result<String, ()> {
        Err(())
    }

    fn extract(&mut self, code: &str) -> Result<Option<Grant>, ()> {
        Ok((code == self.code).then(|| self.grant.take()).flatten())
    }
}

#[derive(Default)]
struct TokenIssuer {
    recover: Option<(String, Grant)>,
    issued: Option<IssuedPair>,
}

struct IssuedPair {
    client_id: String,
    access: String,
    refresh: String,
}

impl TokenIssuer {
    fn recovering(token: &str, grant: Grant) -> Self {
        Self {
            recover: Some((token.to_string(), grant)),
            issued: None,
        }
    }

    fn pair(&mut self, grant: &Grant) -> Result<(String, String), ()> {
        let access = random_text(32)?;
        let refresh = random_text(32)?;
        self.issued = Some(IssuedPair {
            client_id: grant.client_id.clone(),
            access: access.clone(),
            refresh: refresh.clone(),
        });
        Ok((access, refresh))
    }
}

impl Issuer for TokenIssuer {
    fn issue(&mut self, grant: Grant) -> Result<IssuedToken, ()> {
        let (access, refresh) = self.pair(&grant)?;
        Ok(IssuedToken {
            token: access,
            refresh: Some(refresh),
            until: Utc::now() + Duration::seconds(ACCESS_TTL_SECONDS),
            token_type: TokenType::Bearer,
        })
    }

    fn refresh(&mut self, _: &str, grant: Grant) -> Result<RefreshedToken, ()> {
        let (access, refresh) = self.pair(&grant)?;
        Ok(RefreshedToken {
            token: access,
            refresh: Some(refresh),
            until: Utc::now() + Duration::seconds(ACCESS_TTL_SECONDS),
            token_type: TokenType::Bearer,
        })
    }

    fn recover_token<'a>(&'a self, _: &'a str) -> Result<Option<Grant>, ()> {
        Ok(None)
    }

    fn recover_refresh<'a>(&'a self, token: &'a str) -> Result<Option<Grant>, ()> {
        Ok(self
            .recover
            .as_ref()
            .filter(|(expected, _)| expected == token)
            .map(|(_, grant)| grant.clone()))
    }
}

fn stored_grant(stored: noema_store::NativeOAuthGrant) -> Option<Grant> {
    let mut extensions = Extensions::new();
    extensions.set_raw("pkce".to_string(), Value::private(stored.pkce_value));
    Some(Grant {
        owner_id: stored.owner_human_id,
        client_id: stored.client_id,
        scope: stored.scope.parse().ok()?,
        redirect_uri: stored.redirect_uri.parse().ok()?,
        until: Utc.timestamp_opt(stored.expires_at, 0).single()?,
        extensions,
    })
}

fn refresh_grant(stored: &noema_store::NativeOAuthRefreshGrant) -> Option<Grant> {
    Some(Grant {
        owner_id: stored.owner_human_id.clone(),
        client_id: stored.client_id.clone(),
        scope: stored.scope.parse().ok()?,
        redirect_uri: "noema:/oauth/grant".parse().ok()?,
        until: Utc.timestamp_opt(stored.absolute_expires_at, 0).single()?,
        extensions: Extensions::new(),
    })
}

fn private_pkce(extensions: &Extensions) -> Option<String> {
    extensions
        .private()
        .find(|(key, _)| *key == "pkce")
        .and_then(|(_, value)| value.map(str::to_string))
}

fn unique_parameters(input: &[u8]) -> Result<HashMap<String, String>, ()> {
    let mut parameters = HashMap::new();
    for (key, value) in url::form_urlencoded::parse(input) {
        if parameters
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return Err(());
        }
    }
    Ok(parameters)
}

fn form_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|kind| kind.trim() == "application/x-www-form-urlencoded")
        })
}

fn adapt_response(response: OAuthResponse) -> Response {
    let status = match response.status {
        Status::Ok => StatusCode::OK,
        Status::Redirect => StatusCode::FOUND,
        Status::BadRequest => StatusCode::BAD_REQUEST,
        Status::Unauthorized => StatusCode::UNAUTHORIZED,
    };
    let json_body = matches!(response.body, Some(Body::Json(_)));
    let mut output = Response::new(AxumBody::from(response.body.map_or_else(
        String::new,
        |body| match body {
            Body::Text(value) | Body::Json(value) => value,
        },
    )));
    *output.status_mut() = status;
    if let Some(location) = response.location {
        if let Ok(value) = HeaderValue::from_str(location.as_str()) {
            output.headers_mut().insert(header::LOCATION, value);
        }
    }
    if let Some(challenge) = response.www_authenticate {
        if let Ok(value) = HeaderValue::from_str(&challenge) {
            output.headers_mut().insert(header::WWW_AUTHENTICATE, value);
        }
    }
    if json_body {
        output.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    output
}

fn oauth_error(status: StatusCode, error: &str) -> Response {
    let body = serde_json::json!({ "error": error }).to_string();
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

fn random_text(bytes: usize) -> Result<String, ()> {
    let mut value = vec![0_u8; bytes];
    ring::rand::SystemRandom::new()
        .fill(&mut value)
        .map_err(|_| ())?;
    Ok(URL_SAFE_NO_PAD.encode(value))
}

fn random_hex(bytes: usize) -> Result<String, ()> {
    let mut value = vec![0_u8; bytes];
    ring::rand::SystemRandom::new()
        .fill(&mut value)
        .map_err(|_| ())?;
    Ok(value.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn sha256(value: &str) -> [u8; 32] {
    let digest = digest::digest(&digest::SHA256, value.as_bytes());
    let mut output = [0_u8; 32];
    output.copy_from_slice(digest.as_ref());
    output
}

fn secret_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn now() -> i64 {
    SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn display_name(client_id: &str) -> &'static str {
    if client_id.starts_with("noema-ios:") {
        "Noema iOS"
    } else {
        "Noema Desktop"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENT_ID: &str = "noema-desktop:abcdefghijklmnop";
    const REDIRECT: &str = "http://127.0.0.1:49152/oauth/callback";
    const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

    fn authorization_parameters() -> HashMap<String, String> {
        HashMap::from([
            ("client_id".to_string(), CLIENT_ID.to_string()),
            ("redirect_uri".to_string(), REDIRECT.to_string()),
            ("response_type".to_string(), "code".to_string()),
            ("state".to_string(), "s".repeat(32)),
            ("code_challenge".to_string(), CHALLENGE.to_string()),
            ("code_challenge_method".to_string(), "S256".to_string()),
        ])
    }

    #[test]
    fn authorization_requires_s256_pkce_and_client_state() {
        assert!(run_authorization(authorization_parameters(), Consent::Pending).is_ok());
        for missing in ["code_challenge", "code_challenge_method", "state"] {
            let mut parameters = authorization_parameters();
            parameters.remove(missing);
            assert!(run_authorization(parameters, Consent::Pending).is_err());
        }
        let mut plain = authorization_parameters();
        plain.insert("code_challenge_method".to_string(), "plain".to_string());
        assert!(run_authorization(plain, Consent::Pending).is_err());
    }

    #[test]
    fn native_redirects_are_platform_bound() {
        assert!(validate_redirect(CLIENT_ID, REDIRECT).is_ok());
        assert!(validate_redirect(CLIENT_ID, "http://localhost:49152/oauth/callback").is_err());
        assert!(validate_redirect(CLIENT_ID, "http://127.0.0.1:49152/other").is_err());
        assert!(validate_redirect("noema-ios:abcdefghijklmnop", "noema://oauth/callback").is_ok());
        assert!(
            validate_redirect(
                "noema-ios:abcdefghijklmnop",
                "https://attacker.example/callback"
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn code_exchange_refresh_rotation_and_replay_are_end_to_end() {
        let home = tempfile::tempdir().expect("store root");
        let store = noema_store::NoemaStore::open(&noema_store::StoreConfig::new(
            home.path().join("noema.sqlite3"),
        ))
        .await
        .expect("store");
        let state = WebState::new(
            noema_api::graphql::GraphqlState::for_tests(),
            store,
            super::super::authority::CanonicalAuthority::from_public_origin(
                "http://localhost:3737",
                "localhost",
            )
            .expect("authority"),
            super::super::session::SessionSecurity::for_tests("native OAuth"),
            super::super::WebAuthMode::DisabledForDevelopment,
            false,
            None,
        )
        .expect("web state");
        let (_, issued) =
            run_authorization(authorization_parameters(), Consent::Approve).expect("authorization");
        let (code, grant) = issued.expect("issued code");
        let pkce_value = private_pkce(&grant.extensions).expect("PKCE grant");
        state
            .store
            .insert_native_oauth_code(
                sha256(&code),
                noema_store::NewNativeOAuthCode {
                    client_id: &grant.client_id,
                    display_name: "Noema Desktop",
                    redirect_uri: grant.redirect_uri.as_str(),
                    pkce_value: &pkce_value,
                    expires_at: grant.until.timestamp(),
                },
                now(),
            )
            .await
            .expect("persist code");
        let exchange = HashMap::from([
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("client_id".to_string(), CLIENT_ID.to_string()),
            ("redirect_uri".to_string(), REDIRECT.to_string()),
            ("code".to_string(), code),
            ("code_verifier".to_string(), VERIFIER.to_string()),
        ]);
        let response = exchange_code(&state, exchange).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 8 * 1024)
            .await
            .expect("token body");
        let token: serde_json::Value = serde_json::from_slice(&body).expect("token JSON");
        let access = token["access_token"].as_str().expect("access token");
        let old_refresh = token["refresh_token"].as_str().expect("refresh token");
        assert_eq!(
            crate::web::clients::validate_bearer(&state.store, &format!("Bearer {access}"))
                .await
                .expect("access validation")
                .and_then(|principal| principal.client_id().map(str::to_string)),
            Some(CLIENT_ID.to_string())
        );
        let response = refresh(
            &state,
            HashMap::from([
                ("grant_type".to_string(), "refresh_token".to_string()),
                ("refresh_token".to_string(), old_refresh.to_string()),
            ]),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let replay = refresh(
            &state,
            HashMap::from([
                ("grant_type".to_string(), "refresh_token".to_string()),
                ("refresh_token".to_string(), old_refresh.to_string()),
            ]),
        )
        .await;
        assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
        assert!(
            crate::web::clients::validate_bearer(&state.store, &format!("Bearer {access}"))
                .await
                .expect("revoked access validation")
                .is_none()
        );
    }
}
