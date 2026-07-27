use super::*;
use crate::{
    AdapterConnectionRevisions, AdapterConnectionStatus, AdapterConnectionStore,
    AdapterConnectionV1, AdapterCredentialGenerationV1, AdapterCredentialMaterial,
    AdapterDefinitionStore, AdapterManifestV1, ConnectionInstall,
};
use noema_home::NoemaPaths;

fn fixture() -> (
    tempfile::TempDir,
    NoemaPaths,
    DefinitionInstall,
    AdapterConnectionStore,
) {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let manifest: AdapterManifestV1 = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "definition_id": "definition:synthetic_tasks",
        "adapter_id": "synthetic_tasks",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {
            "mode": "oauth2_authorization_code_pkce",
            "scopes": ["https://scope.example/tasks.read"]
        },
        "provider_data_policy": {"retention_allowed": false, "deletion_supported": true},
        "quota": {"cost_class": "free", "request_units": 1},
        "operations": [{
            "operation_id": "list_items",
            "method": "GET",
            "path": "/v1/items",
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
    let definition = AdapterDefinitionStore::new(paths.clone())
        .install(&manifest, "fixture://company-a/tasks.json", None, None)
        .expect("definition");
    let connections = AdapterConnectionStore::new(paths.clone());
    (home, paths, definition, connections)
}

fn install_connection(
    store: &AdapterConnectionStore,
    definition: &DefinitionInstall,
    id: char,
    slug: &str,
    status: AdapterConnectionStatus,
) -> ConnectionInstall {
    let connection_id = id.to_string().repeat(32);
    let generation_id = id
        .to_ascii_uppercase()
        .to_ascii_lowercase()
        .to_string()
        .repeat(32);
    let generation_id = if generation_id == connection_id {
        match id {
            'a' => "b".repeat(32),
            'c' => "d".repeat(32),
            _ => "f".repeat(32),
        }
    } else {
        generation_id
    };
    let descriptor = AdapterConnectionV1 {
        schema_version: 1,
        connection_id,
        connection_slug: slug.to_string(),
        semantic_digest: definition.compiled.semantic_digest.to_string(),
        account_id: Some(format!("account:{id}")),
        account_kind: "personal".to_string(),
        status,
        revisions: AdapterConnectionRevisions {
            connection: 3,
            credential: 5,
            grant: 7,
            policy: 11,
        },
        credential_generation: Some(generation_id.clone()),
        granted_scopes: vec!["https://scope.example/tasks.read".to_string()],
        allowed_operations: vec!["list_items".to_string()],
    };
    let credential = AdapterCredentialGenerationV1 {
        schema_version: 1,
        generation_id,
        material: AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
            client_id: "client-secret-marker".to_string(),
            client_secret: None,
            access_token: "access-secret-marker".to_string(),
            refresh_token: Some("refresh-secret-marker".to_string()),
            expires_at_epoch_seconds: None,
        },
    };
    store
        .install(&descriptor, Some(&credential), &definition.compiled)
        .expect("connection")
}

#[test]
fn active_connection_compiles_exact_non_secret_binding_authority() {
    let (_home, _paths, definition, store) = fixture();
    let connection = install_connection(
        &store,
        &definition,
        'a',
        "personal",
        AdapterConnectionStatus::Active,
    );
    let scan = ConnectionScan {
        connections: vec![connection],
        diagnostics: Vec::new(),
    };

    let catalog =
        AdapterCatalogCompiler::compile(std::slice::from_ref(&definition), &scan).expect("catalog");
    let binding = catalog.snapshot.iter().next().expect("binding");
    assert_eq!(
        binding.spec().name.as_str(),
        "synthetic_tasks_personal.list_items"
    );
    assert_eq!(
        binding.destination().expect("destination").service_id(),
        "adapter"
    );
    let authority: AdapterOperationAuthorityV1 =
        serde_json::from_str(binding.target().operation_token().as_str()).expect("authority");
    assert_eq!(authority.connection_revision, 3);
    assert_eq!(authority.credential_revision, 5);
    assert_eq!(authority.grant_revision, 7);
    assert_eq!(authority.policy_revision, 11);
    let encoded = binding.target().operation_token().as_str();
    for secret in [
        "client-secret-marker",
        "access-secret-marker",
        "refresh-secret-marker",
    ] {
        assert!(!encoded.contains(secret));
    }
    assert!(
        binding
            .persist_arguments(&json!({"marker": "private"}))
            .is_none()
    );
}

#[test]
fn inactive_connections_emit_typed_notices_without_bindings() {
    let (_home, _paths, definition, store) = fixture();
    let suspended = install_connection(
        &store,
        &definition,
        'c',
        "paused",
        AdapterConnectionStatus::Suspended,
    );
    let auth = install_connection(
        &store,
        &definition,
        'e',
        "reauth",
        AdapterConnectionStatus::AuthenticationRequired,
    );
    let scan = ConnectionScan {
        connections: vec![suspended, auth],
        diagnostics: Vec::new(),
    };

    let catalog =
        AdapterCatalogCompiler::compile(std::slice::from_ref(&definition), &scan).expect("catalog");
    assert_eq!(catalog.snapshot.len(), 0);
    assert_eq!(
        catalog
            .availability_notices
            .iter()
            .map(|notice| notice.status)
            .collect::<Vec<_>>(),
        [
            CapabilityAvailabilityStatus::Disabled,
            CapabilityAvailabilityStatus::AuthenticationRequired,
        ]
    );
}

#[test]
fn duplicate_account_bound_name_rejects_the_complete_catalog() {
    let (_home, _paths, definition, store) = fixture();
    let first = install_connection(
        &store,
        &definition,
        'a',
        "same",
        AdapterConnectionStatus::Active,
    );
    let second = install_connection(
        &store,
        &definition,
        'c',
        "same",
        AdapterConnectionStatus::Active,
    );
    let scan = ConnectionScan {
        connections: vec![first, second],
        diagnostics: Vec::new(),
    };
    assert_eq!(
        AdapterCatalogCompiler::compile(std::slice::from_ref(&definition), &scan)
            .expect_err("duplicate must fail"),
        AdapterCatalogError
    );
}
