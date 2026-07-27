//! Bounded event authenticity and duplicate-delivery authorities.

#[cfg(test)]
mod tests;

use ring::hmac;
use std::collections::BTreeSet;
use thiserror::Error;

const MAX_EVENT_BODY_BYTES: usize = 256 * 1024;
const MAX_EVENT_ID_BYTES: usize = 256;
const MAX_DEDUPLICATION_KEYS: usize = 4_096;

/// Declared authenticity contract for one event ingress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventAuthenticityContract {
    /// HMAC-SHA256 over a timestamp-prefixed exact body.
    HmacSha256Hex(HmacEventPolicy),
    /// A challenge/client-state value must match the setup authority.
    Challenge {
        /// Maximum challenge value size.
        max_value_bytes: usize,
    },
}

/// Bounded HMAC policy with explicit replay identity and clock window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HmacEventPolicy {
    /// Header carrying lowercase hexadecimal HMAC-SHA256.
    pub signature_header: String,
    /// Header carrying decimal epoch seconds included in the signed message.
    pub timestamp_header: String,
    /// Header carrying the bounded delivery identity used for deduplication.
    pub delivery_id_header: String,
    /// Maximum exact raw body size.
    pub max_body_bytes: usize,
    /// Maximum age of an accepted delivery.
    pub max_age_seconds: u64,
    /// Maximum future clock skew accepted.
    pub max_future_skew_seconds: u64,
}

/// Exact raw event request retained only until authenticity verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEventRequest {
    /// Ordered headers, preserving duplicates so smuggling cannot be hidden.
    pub headers: Vec<(String, String)>,
    /// Exact bounded bytes before JSON parsing.
    pub body: Vec<u8>,
}

/// Authenticated delivery metadata, never the event body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedEventRequest {
    /// Delivery identity suitable for duplicate suppression.
    pub delivery_id: String,
    /// Authenticated delivery timestamp.
    pub timestamp_epoch_seconds: u64,
}

/// Safe event identity emitted after authenticity verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedEvent {
    /// Stable event identity used for duplicate suppression.
    pub event_id: String,
    /// Sanitized connection identity, never provider payload content.
    pub connection_id: String,
    /// Opaque provider identity retained for routing/observability only.
    pub provider_id: String,
}

/// Authenticity or duplicate-delivery failure.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum EventError {
    /// The declared body/value bound is outside the local policy.
    #[error("event authenticity bound is invalid")]
    InvalidPolicy,
    /// The request body or event identity exceeds its bound.
    #[error("event payload is oversized")]
    Oversized,
    /// The signature is malformed or does not verify.
    #[error("event signature is invalid")]
    InvalidSignature,
    /// A required header is missing, malformed, or duplicated.
    #[error("event header is invalid")]
    InvalidHeader,
    /// The delivery falls outside its bounded replay window.
    #[error("event timestamp is stale")]
    Stale,
    /// A one-use challenge or delivery identity has already been consumed.
    #[error("event delivery has already been consumed")]
    Replay,
    /// The challenge value does not match the pending authority.
    #[error("event challenge is invalid")]
    InvalidChallenge,
    /// The event identity is empty or contains unsafe bytes.
    #[error("event identity is invalid")]
    InvalidIdentity,
}

/// Verify one exact raw HMAC-SHA256 event request.
///
/// The signed message is `timestamp + "." + body`; required headers are
/// unique, bounded, and checked before any JSON parsing.
///
/// # Errors
///
/// Returns EventError when bounds, replay-window headers, hexadecimal encoding,
/// or the HMAC do not match the declared contract.
pub fn verify_hmac_event(
    request: &RawEventRequest,
    secret: &[u8],
    contract: EventAuthenticityContract,
    now_epoch_seconds: u64,
) -> Result<VerifiedEventRequest, EventError> {
    let EventAuthenticityContract::HmacSha256Hex(policy) = contract else {
        return Err(EventError::InvalidPolicy);
    };
    if policy.max_body_bytes == 0
        || policy.max_body_bytes > MAX_EVENT_BODY_BYTES
        || request.body.len() > policy.max_body_bytes
        || policy.max_age_seconds == 0
        || policy.max_future_skew_seconds > policy.max_age_seconds
    {
        return Err(EventError::Oversized);
    }
    if secret.is_empty()
        || !valid_header_name(&policy.signature_header)
        || !valid_header_name(&policy.timestamp_header)
        || !valid_header_name(&policy.delivery_id_header)
    {
        return Err(EventError::InvalidPolicy);
    }
    let signature_header = unique_header(&request.headers, &policy.signature_header)?;
    let timestamp = unique_header(&request.headers, &policy.timestamp_header)?
        .parse::<u64>()
        .map_err(|_| EventError::InvalidHeader)?;
    let delivery_id = unique_header(&request.headers, &policy.delivery_id_header)?;
    if !valid_identity(delivery_id) || delivery_id.len() > MAX_EVENT_ID_BYTES {
        return Err(EventError::InvalidIdentity);
    }
    if timestamp > now_epoch_seconds.saturating_add(policy.max_future_skew_seconds)
        || now_epoch_seconds.saturating_sub(timestamp) > policy.max_age_seconds
    {
        return Err(EventError::Stale);
    }
    if signature_header.len() != 64 {
        return Err(EventError::InvalidSignature);
    }
    let mut signature = [0_u8; 32];
    for (index, pair) in signature_header.as_bytes().chunks_exact(2).enumerate() {
        signature[index] = hex(pair).ok_or(EventError::InvalidSignature)?;
    }
    let mut signed = timestamp.to_string().into_bytes();
    signed.push(b'.');
    signed.extend_from_slice(&request.body);
    hmac::verify(
        &hmac::Key::new(hmac::HMAC_SHA256, secret),
        &signed,
        &signature,
    )
    .map_err(|_| EventError::InvalidSignature)?;
    Ok(VerifiedEventRequest {
        delivery_id: delivery_id.to_string(),
        timestamp_epoch_seconds: timestamp,
    })
}

/// Verify a challenge/client-state value without retaining the event body.
///
/// # Errors
///
/// Returns EventError when the bound is invalid or values differ exactly.
pub fn verify_challenge(
    expected: &str,
    observed: &str,
    contract: EventAuthenticityContract,
) -> Result<(), EventError> {
    let EventAuthenticityContract::Challenge { max_value_bytes } = contract else {
        return Err(EventError::InvalidPolicy);
    };
    if max_value_bytes == 0
        || max_value_bytes > MAX_EVENT_ID_BYTES
        || expected.is_empty()
        || expected.len() > max_value_bytes
        || observed.len() > max_value_bytes
    {
        return Err(EventError::InvalidChallenge);
    }
    if expected == observed {
        Ok(())
    } else {
        Err(EventError::InvalidChallenge)
    }
}

/// One-use challenge authority for a setup or event handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeVerifier {
    expected: String,
    consumed: bool,
}

impl ChallengeVerifier {
    /// Create a verifier for one bounded challenge value.
    #[must_use]
    pub fn new(expected: impl Into<String>) -> Self {
        Self {
            expected: expected.into(),
            consumed: false,
        }
    }

    /// Verify and consume the challenge exactly once.
    ///
    /// # Errors
    ///
    /// Returns EventError for mismatch, invalid bounds, or replay.
    pub fn verify_once(
        &mut self,
        observed: &str,
        contract: EventAuthenticityContract,
    ) -> Result<(), EventError> {
        if self.consumed {
            return Err(EventError::Replay);
        }
        verify_challenge(&self.expected, observed, contract)?;
        self.consumed = true;
        Ok(())
    }
}

/// Accept an event identity once, retaining no event body or headers.
#[derive(Debug, Default)]
pub struct EventDeduplicator {
    keys: BTreeSet<String>,
}

impl EventDeduplicator {
    /// Record one bounded event identity.
    ///
    /// # Errors
    ///
    /// Returns EventError::InvalidIdentity or EventError::Oversized when the
    /// key is not a safe bounded reference.
    pub fn accept(&mut self, event_id: &str) -> Result<bool, EventError> {
        if event_id.is_empty() || !valid_identity(event_id) {
            return Err(EventError::InvalidIdentity);
        }
        if event_id.len() > MAX_EVENT_ID_BYTES {
            return Err(EventError::Oversized);
        }
        if self.keys.contains(event_id) {
            return Ok(false);
        }
        if self.keys.len() >= MAX_DEDUPLICATION_KEYS {
            self.keys.clear();
        }
        self.keys.insert(event_id.to_string());
        Ok(true)
    }

    /// Return the number of retained event identities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Return whether no delivery identities are retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// Create a sanitized trigger after the caller has verified the event.
///
/// # Errors
///
/// Returns EventError when one routing identity is not a bounded reference.
pub fn verified_event(
    event_id: &str,
    connection_id: &str,
    provider_id: &str,
) -> Result<VerifiedEvent, EventError> {
    for value in [event_id, connection_id, provider_id] {
        if value.is_empty() || !valid_identity(value) {
            return Err(EventError::InvalidIdentity);
        }
        if value.len() > MAX_EVENT_ID_BYTES {
            return Err(EventError::Oversized);
        }
    }
    Ok(VerifiedEvent {
        event_id: event_id.to_string(),
        connection_id: connection_id.to_string(),
        provider_id: provider_id.to_string(),
    })
}

fn valid_identity(value: &str) -> bool {
    value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
    })
}

fn valid_header_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn unique_header<'a>(
    headers: &'a [(String, String)],
    expected: &str,
) -> Result<&'a str, EventError> {
    let mut found = None;
    for (name, value) in headers {
        if name.eq_ignore_ascii_case(expected) {
            if found.is_some() || value.is_empty() || value.len() > 256 {
                return Err(EventError::InvalidHeader);
            }
            found = Some(value.as_str());
        }
    }
    found.ok_or(EventError::InvalidHeader)
}

fn hex(pair: &[u8]) -> Option<u8> {
    Some((digit(pair[0])? << 4) | digit(pair[1])?)
}

fn digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
