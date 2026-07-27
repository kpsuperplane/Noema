//! Provider-neutral OAuth 2.0 authorization-code/PKCE attempts.
//!
//! This module owns the pieces that must be identical for every declarative
//! adapter: reviewed endpoint/callback policy, one-use state and PKCE material,
//! exact revision binding, bounded callback parsing, and the process-local
//! state index used before the sibling hardened token transport publishes a
//! filesystem credential generation.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use oauth2::{
    AuthUrl, ClientId, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    TokenUrl, basic::BasicClient,
};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
use thiserror::Error;
use url::{Host, Url};

use crate::{CompiledAdapterDefinition, Oauth2AuthorizationCodePkceConfig, Oauth2CallbackMode};

const RANDOM_BYTES: usize = 80;
const MAX_ATTEMPT_TTL_SECONDS: u64 = 15 * 60;
const MAX_CALLBACK_BYTES: usize = 8 * 1024;
const MAX_ENDPOINT_BYTES: usize = 2_048;
const MAX_CLIENT_ID_BYTES: usize = 512;
const MAX_AUTH_CODE_BYTES: usize = 16 * 1024;
const MAX_ACTIVE_ATTEMPTS: usize = 64;

/// Safe failure categories from the OAuth preflight boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum AdapterOAuthError {
    /// The reviewed definition does not declare a supported OAuth setup.
    #[error("adapter OAuth setup is unsupported")]
    Unsupported,
    /// The definition, authority, callback, or query was invalid.
    #[error("adapter OAuth setup input is invalid")]
    InvalidInput,
    /// The callback did not match the initiating attempt.
    #[error("adapter OAuth callback does not match the initiating attempt")]
    CallbackMismatch,
    /// The initiating authority changed before callback completion.
    #[error("adapter OAuth setup was superseded")]
    Superseded,
    /// The short-lived authorization attempt expired.
    #[error("adapter OAuth setup attempt expired")]
    Expired,
    /// The authorization server returned a bounded denial or error.
    #[error("adapter OAuth authorization was denied")]
    ProviderDenied,
    /// Secure randomness was unavailable.
    #[error("adapter OAuth setup is unavailable")]
    Unavailable,
}

/// Non-secret authority captured when a human starts OAuth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdapterOAuthAuthorityV1 {
    /// Authenticated human who initiated the attempt.
    pub(crate) human_id: String,
    /// Exact connection identity being created or reauthenticated.
    pub(crate) connection_id: String,
    /// Stable external account identity when already known.
    pub(crate) account_id: Option<String>,
    /// Exact account surface selected by the reviewed definition.
    pub(crate) account_kind: String,
    /// Reviewed definition semantic digest.
    pub(crate) semantic_digest: String,
    /// Connection descriptor revision captured before redirect.
    pub(crate) connection_revision: u64,
    /// Credential generation revision captured before redirect.
    pub(crate) credential_revision: u64,
    /// Provider-grant revision captured before redirect.
    pub(crate) grant_revision: u64,
    /// Reviewed operation/policy revision captured before redirect.
    pub(crate) policy_revision: u64,
}

impl AdapterOAuthAuthorityV1 {
    fn validate(&self, definition: &CompiledAdapterDefinition) -> Result<(), AdapterOAuthError> {
        if self.semantic_digest != definition.semantic_digest.as_str()
            || self.connection_revision == 0
            || self.grant_revision == 0
            || self.policy_revision == 0
            || !valid_component(&self.human_id, 256)
            || !valid_hex_id(&self.connection_id)
            || !valid_component(&self.account_kind, 96)
            || self
                .account_id
                .as_deref()
                .is_some_and(|value| !valid_component(value, 256))
            || crate::SemanticDigest::parse(self.semantic_digest.clone()).is_err()
        {
            return Err(AdapterOAuthError::InvalidInput);
        }
        Ok(())
    }

    /// Compare a callback authority with a fresh under-lock connection read.
    #[must_use]
    pub(crate) fn matches(&self, current: &Self) -> bool {
        self == current
    }
}

/// One short-lived browser authorization attempt.
///
/// State and PKCE verifier never serialize, clone, or appear in `Debug`.
pub(crate) struct AdapterOAuthAttempt {
    attempt_id: String,
    authority: AdapterOAuthAuthorityV1,
    redirect_uri: Url,
    authorization_url: Url,
    expires_at_epoch_seconds: u64,
    state: String,
    pkce_verifier: String,
}

impl fmt::Debug for AdapterOAuthAttempt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdapterOAuthAttempt")
            .field("attempt_id", &"[REDACTED]")
            .field("authority", &self.authority)
            .field("redirect_uri", &self.redirect_uri)
            .field("authorization_url", &"[REDACTED]")
            .field("expires_at_epoch_seconds", &self.expires_at_epoch_seconds)
            .field("state", &"[REDACTED]")
            .field("pkce_verifier", &"[REDACTED]")
            .finish()
    }
}

impl AdapterOAuthAttempt {
    /// Start one bounded authorization-code/PKCE attempt with system entropy.
    ///
    /// # Errors
    ///
    /// Returns a safe category if the reviewed definition, authority, callback,
    /// or TTL is invalid.
    pub(crate) fn start(
        definition: &CompiledAdapterDefinition,
        client_id: &str,
        authority: AdapterOAuthAuthorityV1,
        callback_mode: Oauth2CallbackMode,
        redirect_uri: &str,
        now_epoch_seconds: u64,
        ttl_seconds: u64,
    ) -> Result<Self, AdapterOAuthError> {
        let mut random = [0_u8; RANDOM_BYTES];
        SystemRandom::new()
            .fill(&mut random)
            .map_err(|_| AdapterOAuthError::Unavailable)?;
        Self::start_with_random(
            definition,
            client_id,
            authority,
            callback_mode,
            redirect_uri,
            now_epoch_seconds,
            ttl_seconds,
            &random,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn start_with_random(
        definition: &CompiledAdapterDefinition,
        client_id: &str,
        authority: AdapterOAuthAuthorityV1,
        callback_mode: Oauth2CallbackMode,
        redirect_uri: &str,
        now_epoch_seconds: u64,
        ttl_seconds: u64,
        random: &[u8; RANDOM_BYTES],
    ) -> Result<Self, AdapterOAuthError> {
        if !definition.reviewed
            || definition.authentication.mode
                != crate::AuthenticationMode::Oauth2AuthorizationCodePkce
        {
            return Err(AdapterOAuthError::Unsupported);
        }
        let Some(config) = definition.authentication.oauth2.as_ref() else {
            return Err(AdapterOAuthError::Unsupported);
        };
        validate_oauth_config(config).map_err(|_| AdapterOAuthError::InvalidInput)?;
        authority.validate(definition)?;
        if !valid_secret(client_id, MAX_CLIENT_ID_BYTES)
            || ttl_seconds == 0
            || ttl_seconds > MAX_ATTEMPT_TTL_SECONDS
            || !config.callback_modes.contains(&callback_mode)
        {
            return Err(AdapterOAuthError::InvalidInput);
        }
        let redirect_uri = validate_redirect(callback_mode, redirect_uri)?;
        let expires_at_epoch_seconds = now_epoch_seconds
            .checked_add(ttl_seconds)
            .ok_or(AdapterOAuthError::InvalidInput)?;

        let attempt_id = random[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let state = URL_SAFE_NO_PAD.encode(&random[16..48]);
        let pkce_verifier = URL_SAFE_NO_PAD.encode(&random[48..80]);
        let verifier = PkceCodeVerifier::new(pkce_verifier.clone());
        let challenge = PkceCodeChallenge::from_code_verifier_sha256(&verifier);
        let client = BasicClient::new(ClientId::new(client_id.to_string()))
            .set_auth_uri(
                AuthUrl::new(config.authorization_endpoint.clone())
                    .map_err(|_| AdapterOAuthError::InvalidInput)?,
            )
            .set_token_uri(
                TokenUrl::new(config.token_endpoint.clone())
                    .map_err(|_| AdapterOAuthError::InvalidInput)?,
            )
            .set_redirect_uri(
                RedirectUrl::new(redirect_uri.as_str().to_string())
                    .map_err(|_| AdapterOAuthError::InvalidInput)?,
            );
        let mut request = client
            .authorize_url(|| CsrfToken::new(state.clone()))
            .add_scopes(
                definition
                    .authentication
                    .scopes
                    .iter()
                    .cloned()
                    .map(Scope::new),
            )
            .set_pkce_challenge(challenge);
        for (name, value) in &config.extra_authorization_parameters {
            request = request.add_extra_param(name.clone(), value.clone());
        }
        let (authorization_url, _) = request.url();
        Ok(Self {
            attempt_id,
            authority,
            redirect_uri,
            authorization_url,
            expires_at_epoch_seconds,
            state,
            pkce_verifier,
        })
    }

    /// Return the opaque attempt identity.
    #[must_use]
    pub(crate) fn attempt_id(&self) -> &str {
        &self.attempt_id
    }

    /// Return the initiating authority for an under-lock recheck.
    #[must_use]
    pub(crate) fn authority(&self) -> &AdapterOAuthAuthorityV1 {
        &self.authority
    }

    /// Return the exact non-secret authorization URL to open in the system browser.
    #[must_use]
    pub(crate) fn authorization_url(&self) -> &str {
        self.authorization_url.as_str()
    }

    /// Return the expiry bound used by the callback check.
    #[must_use]
    pub(crate) const fn expires_at_epoch_seconds(&self) -> u64 {
        self.expires_at_epoch_seconds
    }

    fn state_key(&self) -> [u8; digest::SHA256_OUTPUT_LEN] {
        state_key(&self.state)
    }

    /// Validate and consume the callback exactly once.
    ///
    /// The returned code keeps the verifier private to the adapter crate so a
    /// future token transport can exchange it without durable serialization.
    ///
    /// # Errors
    ///
    /// Returns a safe category for expiry, authority replacement, callback
    /// mismatch, duplicate fields, provider denial, or missing code.
    pub(crate) fn complete(
        self,
        callback_url: &str,
        now_epoch_seconds: u64,
        current_authority: &AdapterOAuthAuthorityV1,
    ) -> Result<AdapterOAuthAuthorizationCode, AdapterOAuthError> {
        let code = self.validate_callback(callback_url, now_epoch_seconds, current_authority)?;
        Ok(AdapterOAuthAuthorizationCode {
            authority: self.authority,
            redirect_uri: self.redirect_uri,
            code,
            pkce_verifier: self.pkce_verifier,
        })
    }

    fn validate_callback(
        &self,
        callback_url: &str,
        now_epoch_seconds: u64,
        current_authority: &AdapterOAuthAuthorityV1,
    ) -> Result<String, AdapterOAuthError> {
        if now_epoch_seconds >= self.expires_at_epoch_seconds {
            return Err(AdapterOAuthError::Expired);
        }
        if !self.authority.matches(current_authority) {
            return Err(AdapterOAuthError::Superseded);
        }
        if callback_url.len() > MAX_CALLBACK_BYTES {
            return Err(AdapterOAuthError::InvalidInput);
        }
        let callback = Url::parse(callback_url).map_err(|_| AdapterOAuthError::InvalidInput)?;
        if !same_callback_target(&callback, &self.redirect_uri)
            || callback.fragment().is_some()
            || callback.query().is_none()
        {
            return Err(AdapterOAuthError::CallbackMismatch);
        }

        let mut query_names = BTreeSet::new();
        let mut state = None;
        let mut code = None;
        let mut provider_error = None;
        for (name, value) in callback.query_pairs() {
            if !query_names.insert(name.to_string()) {
                return Err(AdapterOAuthError::InvalidInput);
            }
            match name.as_ref() {
                "state" if state.is_none() => state = Some(value.into_owned()),
                "state" => return Err(AdapterOAuthError::InvalidInput),
                "code" if code.is_none() => code = Some(value.into_owned()),
                "code" => return Err(AdapterOAuthError::InvalidInput),
                "error" if provider_error.is_none() => provider_error = Some(value.into_owned()),
                "error" => return Err(AdapterOAuthError::InvalidInput),
                _ => {}
            }
        }
        let state = state.ok_or(AdapterOAuthError::CallbackMismatch)?;
        if !constant_time_equal(state.as_bytes(), self.state.as_bytes()) {
            return Err(AdapterOAuthError::CallbackMismatch);
        }
        if provider_error
            .as_deref()
            .is_some_and(|error| !valid_secret(error, 256))
        {
            return Err(AdapterOAuthError::InvalidInput);
        }
        if provider_error.is_some() {
            if code.is_some() {
                return Err(AdapterOAuthError::InvalidInput);
            }
            return Err(AdapterOAuthError::ProviderDenied);
        }
        let code = code.ok_or(AdapterOAuthError::InvalidInput)?;
        if !valid_secret(&code, MAX_AUTH_CODE_BYTES) {
            return Err(AdapterOAuthError::InvalidInput);
        }
        Ok(code)
    }
}

/// Bounded process-local OAuth attempts indexed by a digest of returned
/// `state`. Restart intentionally discards attempts and leaves the canonical
/// connection in `authentication_required` so the human can start again.
#[derive(Default)]
pub(crate) struct AdapterOAuthAttemptRegistry {
    attempts: BTreeMap<[u8; digest::SHA256_OUTPUT_LEN], RegisteredOAuthAttempt>,
}

struct RegisteredOAuthAttempt {
    authority: AdapterOAuthAuthorityV1,
    attempt: Option<AdapterOAuthAttempt>,
}

pub(crate) struct AdapterOAuthAttemptReservation {
    state_key: [u8; digest::SHA256_OUTPUT_LEN],
    attempt: AdapterOAuthAttempt,
}

impl AdapterOAuthAttemptReservation {
    pub(crate) fn state_key(&self) -> [u8; digest::SHA256_OUTPUT_LEN] {
        self.state_key
    }

    pub(crate) fn complete(
        self,
        callback_url: &str,
        now_epoch_seconds: u64,
        current_authority: &AdapterOAuthAuthorityV1,
    ) -> Result<AdapterOAuthAuthorizationCode, AdapterOAuthError> {
        self.attempt
            .complete(callback_url, now_epoch_seconds, current_authority)
    }
}

impl AdapterOAuthAttemptRegistry {
    pub(crate) fn insert(
        &mut self,
        attempt: AdapterOAuthAttempt,
        now_epoch_seconds: u64,
    ) -> Result<(), AdapterOAuthError> {
        self.attempts.retain(|_, candidate| {
            candidate
                .attempt
                .as_ref()
                .is_none_or(|attempt| now_epoch_seconds < attempt.expires_at_epoch_seconds())
        });
        let authority = attempt.authority().clone();
        let replacement_key = self.attempts.iter().find_map(|(key, candidate)| {
            (candidate.authority.connection_id == authority.connection_id).then_some(*key)
        });
        if replacement_key
            .and_then(|key| self.attempts.get(&key))
            .is_some_and(|candidate| candidate.attempt.is_none())
        {
            return Err(AdapterOAuthError::Unavailable);
        }
        let state_key = attempt.state_key();
        if self.attempts.contains_key(&state_key) {
            return Err(AdapterOAuthError::Unavailable);
        }
        if replacement_key.is_none() && self.attempts.len() >= MAX_ACTIVE_ATTEMPTS {
            return Err(AdapterOAuthError::Unavailable);
        }
        if let Some(key) = replacement_key {
            self.attempts.remove(&key);
        }
        self.attempts.insert(
            state_key,
            RegisteredOAuthAttempt {
                authority,
                attempt: Some(attempt),
            },
        );
        Ok(())
    }

    pub(crate) fn authority_for_callback(
        &self,
        callback_url: &str,
    ) -> Result<AdapterOAuthAuthorityV1, AdapterOAuthError> {
        let state = callback_state(callback_url)?;
        self.attempts
            .get(&state_key(&state))
            .filter(|registered| registered.attempt.is_some())
            .map(|registered| registered.authority.clone())
            .ok_or(AdapterOAuthError::CallbackMismatch)
    }

    pub(crate) fn reserve_for_callback(
        &mut self,
        callback_url: &str,
        now_epoch_seconds: u64,
        current_authority: &AdapterOAuthAuthorityV1,
    ) -> Result<AdapterOAuthAttemptReservation, AdapterOAuthError> {
        let state = callback_state(callback_url)?;
        let state_key = state_key(&state);
        let validation = self
            .attempts
            .get(&state_key)
            .and_then(|registered| registered.attempt.as_ref())
            .ok_or(AdapterOAuthError::CallbackMismatch)?
            .validate_callback(callback_url, now_epoch_seconds, current_authority);
        if let Err(error) = validation {
            if matches!(
                error,
                AdapterOAuthError::Expired
                    | AdapterOAuthError::ProviderDenied
                    | AdapterOAuthError::Superseded
            ) {
                self.attempts.remove(&state_key);
            }
            return Err(error);
        }
        let attempt = self
            .attempts
            .get_mut(&state_key)
            .and_then(|registered| registered.attempt.take())
            .ok_or(AdapterOAuthError::CallbackMismatch)?;
        Ok(AdapterOAuthAttemptReservation { state_key, attempt })
    }

    pub(crate) fn finish(&mut self, state_key: [u8; digest::SHA256_OUTPUT_LEN]) {
        if self
            .attempts
            .get(&state_key)
            .is_some_and(|registered| registered.attempt.is_none())
        {
            self.attempts.remove(&state_key);
        }
    }
}

/// Authorization code plus transient PKCE material for a future token exchange.
pub(crate) struct AdapterOAuthAuthorizationCode {
    authority: AdapterOAuthAuthorityV1,
    redirect_uri: Url,
    code: String,
    pkce_verifier: String,
}

impl fmt::Debug for AdapterOAuthAuthorizationCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdapterOAuthAuthorizationCode")
            .field("authority", &self.authority)
            .field("redirect_uri", &self.redirect_uri)
            .field("code", &"[REDACTED]")
            .field("pkce_verifier", &"[REDACTED]")
            .finish()
    }
}

impl AdapterOAuthAuthorizationCode {
    pub(crate) fn token_exchange_parts(&self) -> (&str, &str, &str) {
        (&self.code, self.redirect_uri.as_str(), &self.pkce_verifier)
    }
}

/// Validate the complete reviewed OAuth policy at both compile and use time.
pub(crate) fn validate_oauth_config(
    config: &Oauth2AuthorizationCodePkceConfig,
) -> Result<(), &'static str> {
    for (field, value) in [
        (
            "oauth2_authorization_endpoint",
            config.authorization_endpoint.as_str(),
        ),
        ("oauth2_token_endpoint", config.token_endpoint.as_str()),
    ] {
        if value.len() > MAX_ENDPOINT_BYTES || !valid_endpoint(value) {
            return Err(field);
        }
    }
    if config.callback_modes.is_empty() || config.callback_modes.len() > 2 {
        return Err("oauth2_callback_modes");
    }
    let mut callback_modes = BTreeSet::new();
    if config
        .callback_modes
        .iter()
        .any(|mode| !callback_modes.insert(*mode))
    {
        return Err("oauth2_callback_modes");
    }
    if config.extra_authorization_parameters.len() > 32 {
        return Err("oauth2_extra_parameters");
    }
    const RESERVED: [&str; 14] = [
        "response_type",
        "client_id",
        "client_secret",
        "redirect_uri",
        "scope",
        "state",
        "code",
        "code_challenge",
        "code_challenge_method",
        "code_verifier",
        "grant_type",
        "error",
        "error_description",
        "error_uri",
    ];
    for (key, value) in &config.extra_authorization_parameters {
        if !valid_component(key, 128) || !valid_secret(value, 1_024) {
            return Err("oauth2_extra_parameter");
        }
        if RESERVED.iter().any(|reserved| *reserved == key) {
            return Err("oauth2_reserved_parameter");
        }
    }
    Ok(())
}

fn valid_endpoint(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.port() != Some(0)
        && url.query().is_none()
        && url.fragment().is_none()
}

fn validate_redirect(mode: Oauth2CallbackMode, value: &str) -> Result<Url, AdapterOAuthError> {
    if value.len() > MAX_CALLBACK_BYTES {
        return Err(AdapterOAuthError::InvalidInput);
    }
    let url = Url::parse(value).map_err(|_| AdapterOAuthError::InvalidInput)?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|port| port == 0)
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AdapterOAuthError::InvalidInput);
    }
    match mode {
        Oauth2CallbackMode::Loopback
            if is_loopback(&url)
                && url.port().is_some_and(|port| port != 0)
                && matches!(url.scheme(), "http" | "https") => {}
        Oauth2CallbackMode::Hosted if url.scheme() == "https" && !is_loopback(&url) => {}
        _ => return Err(AdapterOAuthError::InvalidInput),
    }
    Ok(url)
}

fn same_callback_target(left: &Url, right: &Url) -> bool {
    left.username().is_empty()
        && left.password().is_none()
        && right.username().is_empty()
        && right.password().is_none()
        && left.scheme() == right.scheme()
        && left.host() == right.host()
        && left.port_or_known_default() == right.port_or_known_default()
        && left.path() == right.path()
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

fn valid_hex_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_component(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.trim() == value
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn valid_secret(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let difference = left
        .iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        });
    difference == 0
}

fn callback_state(callback_url: &str) -> Result<String, AdapterOAuthError> {
    if callback_url.len() > MAX_CALLBACK_BYTES {
        return Err(AdapterOAuthError::InvalidInput);
    }
    let callback = Url::parse(callback_url).map_err(|_| AdapterOAuthError::InvalidInput)?;
    let mut names = BTreeSet::new();
    let mut state = None;
    for (name, value) in callback.query_pairs() {
        if !names.insert(name.to_string()) {
            return Err(AdapterOAuthError::InvalidInput);
        }
        if name == "state" {
            state = Some(value.into_owned());
        }
    }
    state
        .filter(|value| valid_secret(value, 1_024))
        .ok_or(AdapterOAuthError::CallbackMismatch)
}

fn state_key(state: &str) -> [u8; digest::SHA256_OUTPUT_LEN] {
    let value = digest::digest(&digest::SHA256, state.as_bytes());
    let mut key = [0_u8; digest::SHA256_OUTPUT_LEN];
    key.copy_from_slice(value.as_ref());
    key
}

#[cfg(test)]
mod tests;
