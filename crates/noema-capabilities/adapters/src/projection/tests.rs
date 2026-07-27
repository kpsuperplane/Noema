use super::*;
use crate::{
    ModelPayload, ModelRoute, PersistenceMode, ProviderRetention, ResultClassification,
    ResultDefinition,
};
use serde_json::json;

fn result(classification: ResultClassification) -> ResultDefinition {
    ResultDefinition {
        classification,
        model_route: ModelRoute::LocalOnly,
        model_payload: ModelPayload::Full,
        provider_retention: ProviderRetention::Deny,
        persistence: PersistenceMode::Redacted,
    }
}

fn policy() -> RestrictedDataPolicy {
    RestrictedDataPolicy {
        model_omit_pointers: vec!["/message/attachments".to_string()],
        persistence_omit_pointers: vec!["/message/body".to_string()],
        max_depth: 8,
        max_nodes: 100,
        max_string_bytes: 1024,
    }
}

#[test]
fn recursive_projection_omits_explicit_restricted_pointers() {
    let value = json!({
        "message": {
            "subject": "hello",
            "body": "private body",
            "attachments": [{"bytes": "private artifact"}]
        }
    });
    let projected = project_result(&value, &result(ResultClassification::Private), &policy())
        .expect("projection");
    assert_eq!(
        projected.model.as_ref().expect("model")["message"]["subject"],
        "hello"
    );
    assert!(
        projected.model.as_ref().expect("model")["message"]
            .get("attachments")
            .is_none()
    );
    assert!(
        projected.persisted.as_ref().expect("persisted")["message"]
            .get("body")
            .is_none()
    );
    assert!(
        !projected
            .model
            .as_ref()
            .expect("model")
            .to_string()
            .contains("private artifact")
    );
}

#[test]
fn metadata_policy_never_leaks_private_values() {
    let mut metadata = result(ResultClassification::Private);
    metadata.model_payload = ModelPayload::MetadataOnly;
    metadata.persistence = PersistenceMode::MetadataOnly;
    let value = json!({"body": "private body"});
    let projected = project_result(&value, &metadata, &policy()).expect("metadata");
    assert_eq!(
        projected.model.as_ref().expect("model")["kind"],
        "restricted_result_metadata"
    );
    assert!(
        !projected
            .model
            .as_ref()
            .expect("model")
            .to_string()
            .contains("private body")
    );
}

#[test]
fn eligibility_requires_exact_grant_and_review_state() {
    let eligibility = RestrictedDataEligibility {
        account_kind: "personal_user".to_string(),
        required_scopes: vec!["mail.read".to_string()],
        granted_scopes: vec![],
        testing_required: true,
        testing_complete: false,
        assessment_required: false,
        assessment_complete: false,
        allowlisted: true,
    };
    assert_eq!(
        eligibility.check("personal_user"),
        Err(ProjectionError::ScopeMissing)
    );
    let mut ready = eligibility.clone();
    ready.granted_scopes = vec!["mail.read".to_string()];
    assert_eq!(
        ready.check("personal_user"),
        Err(ProjectionError::TestingIncomplete)
    );
    ready.testing_complete = true;
    assert_eq!(ready.check("personal_user"), Ok(()));
}

#[test]
fn recursive_bounds_and_pointer_syntax_fail_closed() {
    let mut bounded = policy();
    bounded.max_depth = 1;
    assert_eq!(
        project_result(
            &json!({"one": {"two": true}}),
            &result(ResultClassification::Public),
            &bounded,
        ),
        Err(ProjectionError::Oversized)
    );
    let mut invalid = policy();
    invalid.model_omit_pointers = vec!["/bad~2pointer".to_string()];
    assert_eq!(invalid.validate(), Err(ProjectionError::PolicyInvalid));
}
