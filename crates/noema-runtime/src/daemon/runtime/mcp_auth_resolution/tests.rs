use super::*;

#[test]
fn superseded_authentication_advises_retry_only_for_an_available_current_capability() {
    let retryable = superseded_authentication_output(true);
    assert_eq!(retryable["payload"]["retry_with_current_capability"], true);
    assert!(retryable["payload"]["guidance"].is_string());

    let stale = superseded_authentication_output(false);
    assert_eq!(
        stale,
        serde_json::json!({
            "success": false,
            "payload": {"code": "capability_changed"}
        })
    );

    let (succeeded, succeeded_failure) = recovered_authentication_output(Some((
        GovernedActionState::Succeeded,
        Some(serde_json::json!({"id": 7})),
        None,
    )));
    assert_eq!(
        succeeded,
        serde_json::json!({"success": true, "payload": {"id": 7}})
    );
    assert_eq!(succeeded_failure, None);

    let (failed, failed_code) = recovered_authentication_output(Some((
        GovernedActionState::Failed,
        None,
        Some("remote_rejected".to_string()),
    )));
    assert_eq!(failed["payload"]["code"], "remote_rejected");
    assert_eq!(failed_code.as_deref(), Some("remote_rejected"));

    let (uncertain, uncertain_code) =
        recovered_authentication_output(Some((GovernedActionState::Executing, None, None)));
    assert_eq!(uncertain["payload"]["code"], "outcome_uncertain");
    assert_eq!(uncertain_code.as_deref(), Some("outcome_uncertain"));
}
