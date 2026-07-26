use super::*;

#[test]
fn challenge_round_trips_only_bounded_non_secret_identity() {
    let challenge = CapabilityAuthenticationChallenge::new(
        CapabilityAuthenticationChallengeKind::Reauthenticate,
        CapabilityAuthenticationAuthorityKind::McpServer,
        "mcp:docs",
        "authority:7",
    )
    .expect("challenge");
    let encoded = serde_json::to_value(&challenge).expect("serialize");

    assert_eq!(
        serde_json::from_value::<CapabilityAuthenticationChallenge>(encoded.clone())
            .expect("deserialize"),
        challenge
    );
    assert_eq!(encoded["authority_id"], "mcp:docs");
    assert_eq!(encoded["authority_revision"], "authority:7");
}

#[test]
fn challenge_rejects_display_text_controls_and_oversize_values() {
    for invalid in ["", "display label", "line\nbreak", &"a".repeat(513)] {
        assert!(
            CapabilityAuthenticationChallenge::new(
                CapabilityAuthenticationChallengeKind::ReplaceCredential,
                CapabilityAuthenticationAuthorityKind::AdapterConnection,
                invalid,
                "revision:1",
            )
            .is_err(),
            "{invalid:?}"
        );
    }
}
