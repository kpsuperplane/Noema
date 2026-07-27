use super::*;
use ring::hmac;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn hmac_verification_is_bounded_and_body_free() {
    let body = br#"{"event_id":"evt-1","private":"omitted"}"#;
    let timestamp = "100";
    let mut signed = timestamp.as_bytes().to_vec();
    signed.push(b'.');
    signed.extend_from_slice(body);
    let key = hmac::Key::new(hmac::HMAC_SHA256, b"synthetic-secret");
    let signature = hex(hmac::sign(&key, &signed).as_ref());
    let policy = EventAuthenticityContract::HmacSha256Hex(HmacEventPolicy {
        signature_header: "x-signature".to_string(),
        timestamp_header: "x-timestamp".to_string(),
        delivery_id_header: "x-delivery-id".to_string(),
        max_body_bytes: 1024,
        max_age_seconds: 60,
        max_future_skew_seconds: 5,
    });
    let request = RawEventRequest {
        headers: vec![
            ("x-signature".to_string(), signature),
            ("x-timestamp".to_string(), timestamp.to_string()),
            ("x-delivery-id".to_string(), "event-1".to_string()),
        ],
        body: body.to_vec(),
    };
    assert_eq!(
        verify_hmac_event(&request, b"synthetic-secret", policy.clone(), 100)
            .expect("signature")
            .delivery_id,
        "event-1"
    );
    assert_eq!(
        verify_hmac_event(&request, b"synthetic-secret", policy.clone(), 200),
        Err(EventError::Stale)
    );
    assert_eq!(
        verify_hmac_event(
            &RawEventRequest {
                body: request.body.clone(),
                headers: {
                    let mut headers = request.headers.clone();
                    headers.push(("X-SIGNATURE".to_string(), "duplicate".to_string()));
                    headers
                },
            },
            b"synthetic-secret",
            policy.clone(),
            100,
        ),
        Err(EventError::InvalidHeader)
    );
    assert_eq!(
        verify_hmac_event(
            &request,
            b"synthetic-secret",
            EventAuthenticityContract::HmacSha256Hex(HmacEventPolicy {
                max_body_bytes: 4,
                ..HmacEventPolicy {
                    signature_header: "x-signature".to_string(),
                    timestamp_header: "x-timestamp".to_string(),
                    delivery_id_header: "x-delivery-id".to_string(),
                    max_body_bytes: 1024,
                    max_age_seconds: 60,
                    max_future_skew_seconds: 5,
                }
            }),
            100,
        ),
        Err(EventError::Oversized)
    );
    assert!(!format!("{request:?}").contains("synthetic-secret"));
}

#[test]
fn challenge_contract_is_exact_and_separate_from_hmac() {
    let contract = EventAuthenticityContract::Challenge {
        max_value_bytes: 64,
    };
    verify_challenge("state-1", "state-1", contract.clone()).expect("challenge");
    assert_eq!(
        verify_challenge("state-1", "state-2", contract.clone()),
        Err(EventError::InvalidChallenge)
    );
    assert_eq!(
        verify_challenge(
            "state-1",
            "state-1",
            EventAuthenticityContract::HmacSha256Hex(HmacEventPolicy {
                signature_header: "x-signature".to_string(),
                timestamp_header: "x-timestamp".to_string(),
                delivery_id_header: "x-delivery-id".to_string(),
                max_body_bytes: 1024,
                max_age_seconds: 60,
                max_future_skew_seconds: 5,
            })
        ),
        Err(EventError::InvalidPolicy)
    );
    let mut verifier = ChallengeVerifier::new("state-1");
    verifier
        .verify_once("state-1", contract.clone())
        .expect("one use");
    assert_eq!(
        verifier.verify_once("state-1", contract),
        Err(EventError::Replay)
    );
}

#[test]
fn deduplication_and_sanitized_trigger_never_retain_event_body() {
    let mut deduplicator = EventDeduplicator::default();
    assert_eq!(deduplicator.accept("event-1"), Ok(true));
    assert_eq!(deduplicator.accept("event-1"), Ok(false));
    assert_eq!(deduplicator.len(), 1);
    assert_eq!(
        verified_event("event-1", "connection-1", "fixture"),
        Ok(VerifiedEvent {
            event_id: "event-1".to_string(),
            connection_id: "connection-1".to_string(),
            provider_id: "fixture".to_string(),
        })
    );
    assert_eq!(
        verified_event("event with body", "connection-1", "fixture"),
        Err(EventError::InvalidIdentity)
    );
    assert!(!format!("{deduplicator:?}").contains("private"));
}
