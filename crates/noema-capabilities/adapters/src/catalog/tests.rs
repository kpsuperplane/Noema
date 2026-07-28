use super::*;
use crate::{
    AdapterConnectionRevisions, AdapterConnectionStatus, AdapterConnectionStore,
    AdapterConnectionV2, AdapterCredentialGenerationV1, AdapterCredentialMaterial,
    AdapterDefinitionStore, AdapterManifestV3, ConnectionInstall,
};
use noema_capabilities::CapabilityExecutionDecision;
use noema_home::NoemaPaths;

fn fixture() -> (
    tempfile::TempDir,
    NoemaPaths,
    DefinitionInstall,
    AdapterConnectionStore,
) {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let manifest: AdapterManifestV3 = serde_json::from_value(serde_json::json!({
        "schema_version": 3,
        "definition_id": "definition:synthetic_tasks",
        "adapter_id": "synthetic_tasks",
        "display_name": "Synthetic Tasks",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {
            "mode": "oauth2_authorization_code_pkce",
            "scopes": ["https://scope.example/tasks.read"]
        },
        "quota": {"cost_class": "free", "request_units": 1},
        "operations": [
            {
                "operation_id": "list_items",
                "method": "GET",
                "path": "/v1/items",
                "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"}
            },
            {
                "operation_id": "create_item",
                "method": "POST",
                "path": "/v1/items",
                "behavior": {"readOnly": {"value": false, "source": "model"}, "idempotent": {"value": false, "source": "model"}, "destructive": {"value": true, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                "retry": "never",
                "pagination": {"kind": "none"}
            }
        ]
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
    let descriptor = AdapterConnectionV2 {
        schema_version: 2,
        connection_id,
        connection_slug: slug.to_string(),
        semantic_digest: definition.compiled.semantic_digest.to_string(),
        account_id: Some(format!("account:{id}")),
        account_label: Some(format!("person-{id}@example.test")),
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
        policy: Some(noema_capabilities::CapabilityConnectionPolicy {
            data_sharing: noema_capabilities::CapabilityDataSharingPolicy::AllowAutomatically,
            unsafe_actions: noema_capabilities::CapabilityUnsafeActionPolicy::ReviewerMayApprove,
            revision: 11,
        }),
        tool_overrides: Vec::new(),
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
    assert!(
        binding
            .destination()
            .expect("destination")
            .revision()
            .contains("credential:5")
    );
    assert!(
        !binding
            .destination()
            .expect("destination")
            .authentication_revision()
            .contains("credential")
    );
    let service = binding.service_context().expect("service context");
    assert_eq!(service.display_name(), "Synthetic Tasks");
    assert_eq!(service.account_label(), Some("person-a@example.test"));
    let authority: AdapterOperationAuthorityV1 =
        serde_json::from_str(binding.target().operation_token().as_str()).expect("authority");
    assert_eq!(authority.connection_revision, 3);
    assert_eq!(authority.account_id.as_deref(), Some("account:a"));
    assert_eq!(authority.credential_revision, 5);
    assert_eq!(authority.grant_revision, 7);
    assert_eq!(authority.policy_revision, 11);
    let encoded = binding.target().operation_token().as_str();
    assert_eq!(
        AdapterOperationAuthorityV1::from_operation_token(&OperationToken::new(format!(
            " {encoded}"
        ))),
        Err(AdapterCatalogError)
    );
    for secret in [
        "client-secret-marker",
        "access-secret-marker",
        "refresh-secret-marker",
    ] {
        assert!(!encoded.contains(secret));
    }
    assert_eq!(
        binding.persist_arguments(&json!({"marker": "ordinary"})),
        Some(json!({"marker": "ordinary"}))
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

#[test]
fn risky_tool_uses_the_connection_review_policy() {
    let (_home, _paths, definition, store) = fixture();
    let installed = install_connection(
        &store,
        &definition,
        'a',
        "personal",
        AdapterConnectionStatus::Active,
    );
    let mut descriptor = installed.descriptor;
    descriptor.allowed_operations = vec!["create_item".to_string(), "list_items".to_string()];
    descriptor.policy = Some(noema_capabilities::CapabilityConnectionPolicy {
        data_sharing: noema_capabilities::CapabilityDataSharingPolicy::AllowAutomatically,
        unsafe_actions: noema_capabilities::CapabilityUnsafeActionPolicy::AlwaysAsk,
        revision: descriptor.revisions.policy,
    });
    let generation_id = descriptor
        .credential_generation
        .clone()
        .expect("credential generation");
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
    let other_home = tempfile::tempdir().expect("other home");
    let other_paths = NoemaPaths::from_noema_home(other_home.path()).expect("paths");
    let other_store = AdapterConnectionStore::new(other_paths);
    let connection = other_store
        .install(&descriptor, Some(&credential), &definition.compiled)
        .expect("connection");
    let scan = ConnectionScan {
        connections: vec![connection],
        diagnostics: Vec::new(),
    };

    let catalog = AdapterCatalogCompiler::compile(&[definition], &scan).expect("catalog");
    assert!(catalog.availability_notices.is_empty());
    let binding = catalog
        .snapshot
        .resolve("synthetic_tasks_personal.create_item")
        .expect("reviewed binding");
    assert!(!binding.behavior().read_only);
    assert_eq!(
        binding.execution_decision(),
        CapabilityExecutionDecision::HumanReview
    );
}
