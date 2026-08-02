use super::*;
use crate::{ArgumentLocation, ArgumentSource, ArgumentType, PaginationPolicy, ProviderLinkKind};
use noema_home::NoemaPaths;
use tempfile::tempdir;

fn link_policy() -> PaginationPolicy {
    PaginationPolicy::ProviderLink {
        response_pointer: "/next".to_string(),
        request_argument: Some("page_token".to_string()),
        allowed_origins: vec!["https://api.example.test/".to_string()],
        credential_mode: ContinuationCredentialMode::Omit,
        link_kind: ProviderLinkKind::NextPage,
        max_bytes: 8 * 1024,
        ttl_seconds: 300,
    }
}

fn binding(account_kind: &str) -> CursorBinding {
    CursorBinding {
        connection_id: "connection-1".to_string(),
        semantic_digest: "a".repeat(64),
        operation_id: "list_items".to_string(),
        account_kind: account_kind.to_string(),
        grant_revision: 4,
        arguments_sha256: "b".repeat(64),
    }
}

#[test]
fn durable_cursor_secrets_survive_store_recreation_without_metadata_leakage() {
    let home = tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let handle = CursorHandle {
        secret_reference: "cursor-ref".to_string(),
        binding: binding("personal_user"),
        expires_at_epoch_seconds: 100,
    };
    DurableCursorStore::new(paths.clone())
        .put(&handle, "private-token")
        .expect("put");
    let reopened = DurableCursorStore::new(paths);
    assert_eq!(
        reopened
            .resolve("cursor-ref", &binding("personal_user"), 1)
            .expect("resolve")
            .1
            .as_str(),
        "private-token"
    );
    assert!(!format!("{reopened:?}").contains("private-token"));
    reopened.retire("cursor-ref").expect("retire");
}

#[test]
fn durable_cursor_authority_rejects_tampering_expiry_and_every_binding_drift() {
    let home = tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let expected = binding("personal_user");
    let handle = CursorHandle {
        secret_reference: "cursor-ref".to_string(),
        binding: expected.clone(),
        expires_at_epoch_seconds: 100,
    };
    let store = DurableCursorStore::new(paths);
    store.put(&handle, "private-token").expect("put");

    let mut variants = Vec::new();
    let mut cross_connection = expected.clone();
    cross_connection.connection_id = "connection-2".to_string();
    variants.push(cross_connection);
    let mut stale_definition = expected.clone();
    stale_definition.semantic_digest = "c".repeat(64);
    variants.push(stale_definition);
    let mut stale_grant = expected.clone();
    stale_grant.grant_revision += 1;
    variants.push(stale_grant);
    let mut mismatched_arguments = expected.clone();
    mismatched_arguments.arguments_sha256 = "d".repeat(64);
    variants.push(mismatched_arguments);
    for variant in variants {
        assert!(matches!(
            store.resolve("cursor-ref", &variant, 1),
            Err(DurableCursorError::BindingMismatch)
        ));
    }
    assert!(store.resolve("tampered", &expected, 1).is_err());
    assert!(matches!(
        store.resolve("cursor-ref", &expected, 100),
        Err(DurableCursorError::Expired)
    ));
}

#[test]
fn provider_links_are_exact_origin_bounded_and_expiring() {
    let validated = validate_provider_link(
        "https://api.example.test/v1/items?cursor=opaque",
        &link_policy(),
        100,
    )
    .expect("link");
    assert_eq!(validated.expires_at_epoch_seconds, 400);
    assert!(!format!("{validated:?}").contains("opaque"));
    for value in [
        "http://api.example.test/v1/items",
        "https://other.example.test/v1/items",
        "https://user:secret@api.example.test/v1/items",
        "https://api.example.test/v1/%2e%2e/admin",
    ] {
        assert!(
            validate_provider_link(value, &link_policy(), 100).is_err(),
            "{value}"
        );
    }
}

#[test]
fn cursor_binding_expiry_and_full_resync_are_fenced() {
    let mut store = CursorStore::default();
    let original = store
        .issue("cursor-a", binding("personal_user"), "opaque-token", 10)
        .expect("issue");
    assert_eq!(
        store
            .resolve(&original, &binding("other_account"), 1)
            .expect_err("wrong account"),
        ContinuationError::CursorBindingMismatch
    );
    assert_eq!(
        store.resolve(&original, &binding("personal_user"), 10),
        Err(ContinuationError::Expired)
    );
    assert_eq!(
        store.resolve(&original, &binding("personal_user"), 11),
        Err(ContinuationError::FullResyncRequired)
    );
    let replacement = store
        .commit_resync(&original, "cursor-b", "fresh-token", 50)
        .expect("baseline committed");
    assert_eq!(
        store
            .resolve(&replacement, &binding("personal_user"), 12)
            .expect("replacement"),
        CursorSecret("fresh-token".to_string())
    );
    assert!(!format!("{store:?}").contains("fresh-token"));
}

#[test]
fn explicit_invalidation_requires_a_bounded_baseline_before_commit() {
    let mut store = CursorStore::default();
    let handle = store
        .issue("cursor-a", binding("personal_user"), "token", 100)
        .expect("issue");
    store.require_full_resync(&handle).expect("invalidate");
    assert_eq!(
        store.resolve(&handle, &binding("personal_user"), 1),
        Err(ContinuationError::FullResyncRequired)
    );
    assert_eq!(
        store.commit_resync(&handle, "cursor-b", "new", 200),
        Ok(CursorHandle {
            secret_reference: "cursor-b".to_string(),
            binding: binding("personal_user"),
            expires_at_epoch_seconds: 200,
        })
    );
}

#[test]
fn retry_after_and_eligibility_stay_bounded_and_unproven_modes_block() {
    assert_eq!(parse_retry_after("30"), Ok(30));
    assert_eq!(
        parse_retry_after("86401"),
        Err(ContinuationError::RetryAfterInvalid)
    );
    assert_eq!(
        parse_retry_after("tomorrow"),
        Err(ContinuationError::RetryAfterInvalid)
    );
    let personal = ContinuationEligibility {
        account_kind: "personal_user".to_string(),
        auth_binding: ContinuationAuthBinding::Personal,
    };
    assert_eq!(personal.check("personal_user"), Ok(()));
    assert_eq!(
        personal.check("workspace"),
        Err(ContinuationGateError::AccountKindMismatch)
    );
    for (mode, error) in [
        (
            ContinuationAuthBinding::Delegated,
            ContinuationGateError::DelegatedUnproven,
        ),
        (
            ContinuationAuthBinding::Application,
            ContinuationGateError::ApplicationUnproven,
        ),
        (
            ContinuationAuthBinding::Tenant,
            ContinuationGateError::TenantUnproven,
        ),
        (
            ContinuationAuthBinding::Audience,
            ContinuationGateError::AudienceUnproven,
        ),
    ] {
        assert_eq!(
            ContinuationEligibility {
                account_kind: "personal_user".to_string(),
                auth_binding: mode,
            }
            .check("personal_user"),
            Err(error)
        );
    }
}

#[test]
fn pagination_runtime_arguments_are_hidden_and_policy_is_typed() {
    let arguments = vec![crate::ArgumentDefinition {
        name: "limit".to_string(),
        source: ArgumentSource::ModelInput,
        location: ArgumentLocation::Query,
        argument_type: ArgumentType::Integer,
        required: false,
        enum_values: vec![],
    }];
    assert!(
        validate_pagination(
            &PaginationPolicy::DeltaCursor {
                response_pointer: "/delta".to_string(),
                request_argument: "cursor".to_string(),
                baseline_operation: "list_items".to_string(),
                max_age_seconds: 900,
            },
            &arguments,
        )
        .is_ok()
    );
    assert!(matches!(
        validate_pagination(
            &PaginationPolicy::DeltaCursor {
                response_pointer: "/delta".to_string(),
                request_argument: "limit".to_string(),
                baseline_operation: "list_items".to_string(),
                max_age_seconds: 900,
            },
            &arguments,
        ),
        Err(crate::AdapterCompileError::Invalid("pagination"))
    ));
}
