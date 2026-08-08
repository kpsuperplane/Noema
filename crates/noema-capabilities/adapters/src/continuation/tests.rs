use super::*;
use noema_home::NoemaPaths;
use tempfile::tempdir;

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
fn retry_after_stays_bounded() {
    assert_eq!(parse_retry_after("30"), Ok(30));
    assert_eq!(
        parse_retry_after("86401"),
        Err(ContinuationError::RetryAfterInvalid)
    );
    assert_eq!(
        parse_retry_after("tomorrow"),
        Err(ContinuationError::RetryAfterInvalid)
    );
}
