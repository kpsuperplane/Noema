use super::*;
use crate::{
    AdapterCatalogCompiler, AdapterConnectionRevisions, AdapterConnectionStatus,
    AdapterCredentialMaterial, AdapterDefinitionStore, AdapterManifestV1, import_client_json,
};

fn definition(paths: &NoemaPaths) -> DefinitionInstall {
    let manifest: AdapterManifestV1 = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "definition_id": "definition:synthetic_calendar",
        "adapter_id": "synthetic_calendar",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {
            "mode": "oauth2_authorization_code_pkce",
            "scopes": ["https://scope.example/calendar.read"],
            "credential_import": {
                "kind": "oauth_client_json",
                "alternatives": [
                    {"client_id_pointer": "/desktop/client_id", "client_secret_pointer": "/desktop/client_secret"},
                    {"client_id_pointer": "/browser/client_id", "client_secret_pointer": "/browser/client_secret"}
                ]
            }
        },
        "provider_data_policy": {"retention_allowed": false, "deletion_supported": true},
        "quota": {"cost_class": "free", "request_units": 1},
        "operations": [{
            "operation_id": "list_events",
            "method": "GET",
            "path": "/v1/events",
            "effect": "read_only",
            "admission": "direct",
            "result": {
                "classification": "private",
                "model_route": "local_only",
                "model_payload": "full",
                "provider_retention": "deny",
                "persistence": "omit"
            },
            "retry": "transport_safe_read",
            "pagination": {"kind": "none"}
        }]
    }))
    .expect("manifest");
    AdapterDefinitionStore::new(paths.clone())
        .install(&manifest, "fixture://company-a/calendar.json", None, None)
        .expect("definition")
}

fn connection(
    definition: &DefinitionInstall,
    connection_id: &str,
    generation_id: &str,
) -> (AdapterConnectionV1, AdapterCredentialGenerationV1) {
    (
        AdapterConnectionV1 {
            schema_version: 1,
            connection_id: connection_id.to_string(),
            connection_slug: format!("calendar_{}", &connection_id[..6]),
            semantic_digest: definition.compiled.semantic_digest.to_string(),
            account_id: Some(format!("account:{}", &connection_id[..6])),
            account_kind: "personal".to_string(),
            status: AdapterConnectionStatus::Active,
            revisions: AdapterConnectionRevisions {
                connection: 1,
                credential: 1,
                grant: 1,
                policy: 1,
            },
            credential_generation: Some(generation_id.to_string()),
            granted_scopes: vec!["https://scope.example/calendar.read".to_string()],
            allowed_operations: vec!["list_events".to_string()],
        },
        AdapterCredentialGenerationV1 {
            schema_version: 1,
            generation_id: generation_id.to_string(),
            material: AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
                client_id: "synthetic-client".to_string(),
                client_secret: Some("client-secret-marker".to_string()),
                access_token: "access-secret-marker".to_string(),
                refresh_token: Some("refresh-secret-marker".to_string()),
                expires_at_epoch_seconds: Some(4_000_000_000),
            },
        },
    )
}

#[test]
fn install_and_scan_preserve_non_secret_authority_and_private_credentials() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let definition = definition(&paths);
    let store = AdapterConnectionStore::new(paths.clone());
    let (descriptor, credential) = connection(&definition, &"a".repeat(32), &"b".repeat(32));

    let installed = store
        .install(&descriptor, Some(&credential), &definition.compiled)
        .expect("install");
    let mut conflicting = credential.clone();
    if let AdapterCredentialMaterial::Oauth2AuthorizationCodePkce { access_token, .. } =
        &mut conflicting.material
    {
        *access_token = "different-secret".to_string();
    }
    assert!(
        store
            .install(&descriptor, Some(&conflicting), &definition.compiled)
            .is_err(),
        "one immutable generation cannot accept different bytes"
    );
    let scan = store.scan(&[definition]).expect("scan");
    assert_eq!(scan.connections.len(), 1);
    assert_eq!(scan.projections(), [installed.projection]);

    let credential_path = paths
        .adapter_connection_dir(&descriptor.connection_id)
        .expect("connection path")
        .join("credentials")
        .join(format!("{}.json", credential.generation_id));
    let descriptor_bytes = std::fs::read(
        paths
            .adapter_connection_dir(&descriptor.connection_id)
            .expect("connection path")
            .join("connection.json"),
    )
    .expect("descriptor bytes");
    for secret in [
        "client-secret-marker",
        "access-secret-marker",
        "refresh-secret-marker",
    ] {
        assert!(!String::from_utf8_lossy(&descriptor_bytes).contains(secret));
        assert!(!format!("{credential:?}").contains(secret));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(credential_path)
                .expect("credential metadata")
                .permissions()
                .mode()
                & 0o077,
            0
        );
    }
}

#[test]
fn invalid_connection_isolated_and_quarantine_prevents_rediscovery() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let definition = definition(&paths);
    let store = AdapterConnectionStore::new(paths.clone());
    let (valid, valid_credential) = connection(&definition, &"c".repeat(32), &"d".repeat(32));
    let (invalid, invalid_credential) = connection(&definition, &"e".repeat(32), &"f".repeat(32));
    store
        .install(&valid, Some(&valid_credential), &definition.compiled)
        .expect("valid install");
    store
        .install(&invalid, Some(&invalid_credential), &definition.compiled)
        .expect("invalid-before-tamper install");
    std::fs::write(
        paths
            .adapter_connection_dir(&invalid.connection_id)
            .expect("path")
            .join("connection.json"),
        b"{}",
    )
    .expect("tamper");

    let scan = store
        .scan(std::slice::from_ref(&definition))
        .expect("isolated scan");
    assert_eq!(scan.connections.len(), 1);
    assert_eq!(scan.diagnostics.len(), 1);
    assert_eq!(
        scan.connections[0].descriptor.connection_id,
        valid.connection_id
    );

    store.quarantine(&valid.connection_id).expect("quarantine");
    let scan = store.scan(&[definition]).expect("post-quarantine scan");
    assert!(scan.connections.is_empty());
    assert_eq!(
        scan.diagnostics.len(),
        1,
        "only tampered connection remains"
    );
    assert!(
        paths
            .quarantined_adapter_connection_dir(&valid.connection_id)
            .expect("quarantine path")
            .exists()
    );
}

#[test]
fn connection_rejects_definition_scope_operation_and_credential_mismatch() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let definition = definition(&paths);
    let store = AdapterConnectionStore::new(paths);
    let (mut descriptor, credential) = connection(&definition, &"1".repeat(32), &"2".repeat(32));

    descriptor.granted_scopes = vec!["calendar.write".to_string()];
    assert!(
        store
            .install(&descriptor, Some(&credential), &definition.compiled)
            .is_err()
    );
    descriptor.granted_scopes = Vec::new();
    assert!(
        store
            .install(&descriptor, Some(&credential), &definition.compiled)
            .is_err()
    );
    descriptor.granted_scopes = vec!["https://scope.example/calendar.read".to_string()];
    descriptor.allowed_operations = vec!["missing".to_string()];
    assert!(
        store
            .install(&descriptor, Some(&credential), &definition.compiled)
            .is_err()
    );
    descriptor.allowed_operations = vec!["list_events".to_string()];
    let mut mismatched = credential;
    mismatched.generation_id = "3".repeat(32);
    assert!(
        store
            .install(&descriptor, Some(&mismatched), &definition.compiled)
            .is_err()
    );
}

#[test]
fn transient_client_json_publishes_only_metadata_and_rebuilds_auth_required_state() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let definition = definition(&paths);
    let store = AdapterConnectionStore::new(paths.clone());
    let generation_id = "9".repeat(32);
    let upload = br#"{"desktop":{"client_id":"client-marker","client_secret":"secret-marker","raw_upload_marker":"discard-me"}}"#;
    let credential =
        import_client_json(&definition.compiled, upload, generation_id).expect("import");
    let AdapterCredentialMaterial::Oauth2ClientMetadata {
        client_id,
        client_secret,
    } = &credential.material
    else {
        panic!("client metadata");
    };
    assert_eq!(client_id, "client-marker");
    assert_eq!(client_secret.as_deref(), Some("secret-marker"));

    let descriptor = AdapterConnectionV1 {
        schema_version: 1,
        connection_id: "8".repeat(32),
        connection_slug: "pending".to_string(),
        semantic_digest: definition.compiled.semantic_digest.to_string(),
        account_id: None,
        account_kind: "personal".to_string(),
        status: AdapterConnectionStatus::AuthenticationRequired,
        revisions: AdapterConnectionRevisions {
            connection: 1,
            credential: 1,
            grant: 1,
            policy: 1,
        },
        credential_generation: Some(credential.generation_id.clone()),
        granted_scopes: Vec::new(),
        allowed_operations: vec!["list_events".to_string()],
    };
    store
        .install(&descriptor, Some(&credential), &definition.compiled)
        .expect("publish");
    let mut active_with_metadata = descriptor.clone();
    active_with_metadata.connection_id = "7".repeat(32);
    active_with_metadata.status = AdapterConnectionStatus::Active;
    active_with_metadata.granted_scopes = vec!["https://scope.example/calendar.read".to_string()];
    assert!(
        store
            .install(
                &active_with_metadata,
                Some(&credential),
                &definition.compiled,
            )
            .is_err(),
        "pre-authorization metadata cannot become an active bearer credential"
    );
    let mut definition_without_import = definition.compiled.clone();
    definition_without_import.authentication.credential_import = None;
    let mut pending_without_import = descriptor.clone();
    pending_without_import.connection_id = "5".repeat(32);
    assert!(
        store
            .install(
                &pending_without_import,
                Some(&credential),
                &definition_without_import,
            )
            .is_err(),
        "metadata generations require the definition import authority"
    );
    let credential_path = paths
        .adapter_connection_dir(&descriptor.connection_id)
        .expect("path")
        .join("credentials")
        .join(format!("{}.json", credential.generation_id));
    let stored = std::fs::read_to_string(credential_path).expect("stored credential");
    assert!(stored.contains("client-marker"));
    assert!(stored.contains("secret-marker"));
    assert!(!stored.contains("discard-me"));
    assert!(!stored.contains(std::str::from_utf8(upload).expect("upload text")));

    let scan = store.scan(std::slice::from_ref(&definition)).expect("scan");
    assert_eq!(scan.connections.len(), 1);
    assert_eq!(scan.connections[0].descriptor, descriptor);
    let catalog = AdapterCatalogCompiler::compile(&[definition], &scan).expect("catalog");
    assert_eq!(catalog.snapshot.len(), 0);
    assert_eq!(catalog.availability_notices.len(), 1);
    assert_eq!(
        catalog.availability_notices[0].status,
        noema_capabilities::CapabilityAvailabilityStatus::AuthenticationRequired
    );

    let abandoned = paths
        .adapter_connections_dir()
        .join(".staging")
        .join("interrupted");
    std::fs::create_dir_all(abandoned.join("credentials")).expect("staging");
    std::fs::write(
        abandoned.join("credentials").join("deadbeef.json"),
        b"discard-me",
    )
    .expect("staged secret");
    store.recover().expect("recover");
    assert!(!abandoned.exists());
    assert!(
        paths
            .adapter_connection_dir(&descriptor.connection_id)
            .expect("active path")
            .exists()
    );
}
