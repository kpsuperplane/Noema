use super::*;
use crate::{NoemaStore, StoreConfig};
use noema_capability_adapters::{
    AdapterConnectionRevisions, AdapterConnectionStatus, AdapterConnectionStore,
    AdapterConnectionV3, AdapterCredentialGenerationV1, AdapterCredentialMaterial,
    AdapterDefinitionStore, AdapterManifestV3,
};
use noema_home::NoemaPaths;

fn fixture_manifest(authentication: serde_json::Value) -> AdapterManifestV3 {
    serde_json::from_value(serde_json::json!({
        "schema_version": 3,
        "definition_id": "definition:offline_fixture",
        "adapter_id": "offline_fixture",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": authentication,
        "quota": {"cost_class":"free","request_units":1},
        "operations": [{
            "operation_id":"list",
            "method":"GET",
            "path":"/v1/items",
            "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
            "retry":"transport_safe_read",
            "pagination":{"kind":"none"}
        }]
    }))
    .expect("fixture manifest")
}

fn remove_database_family(paths: &NoemaPaths) {
    for suffix in ["", "-wal", "-shm"] {
        let path =
            std::path::PathBuf::from(format!("{}{suffix}", paths.sqlite_db_path().display()));
        if path.exists() {
            std::fs::remove_file(path).expect("remove database family");
        }
    }
}

#[tokio::test]
async fn fresh_sqlite_rebuilds_exact_definition_projection_from_files() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let definitions = AdapterDefinitionStore::new(paths.clone());
    definitions
        .install(
            &fixture_manifest(serde_json::json!({"mode":"none"})),
            "fixture://independent-company-a/openapi.json",
            None,
            Some((br#"{"openapi":"3.0.3"}"#, "json")),
        )
        .expect("install");
    let scan = definitions.scan().expect("scan");
    let store = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path()))
        .await
        .expect("store");
    store
        .reconcile_adapter_definitions(&scan.projections())
        .await
        .expect("reconcile");
    let before = store.adapter_definitions().await.expect("rows");
    drop(store);
    remove_database_family(&paths);
    let recreated = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path()))
        .await
        .expect("recreated store");
    let rediscovered = definitions.scan().expect("rescan");
    recreated
        .reconcile_adapter_definitions(&rediscovered.projections())
        .await
        .expect("reconcile recreated");
    assert_eq!(recreated.adapter_definitions().await.expect("rows"), before);
}

#[tokio::test]
async fn fresh_sqlite_rebuilds_connection_projection_without_secret_bytes() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let definitions = AdapterDefinitionStore::new(paths.clone());
    let definition = definitions
        .install(
            &fixture_manifest(serde_json::json!({
                "mode":"oauth2_authorization_code_pkce",
                "scopes":["https://scope.example/items.read"]
            })),
            "fixture://independent-company-b/openapi.json",
            None,
            None,
        )
        .expect("definition");
    let connection_id = "a".repeat(32);
    let generation_id = "b".repeat(32);
    let descriptor = AdapterConnectionV3 {
        schema_version: 3,
        connection_id: connection_id.clone(),
        connection_slug: "personal".to_string(),
        semantic_digest: definition.compiled.semantic_digest.to_string(),
        account_id: Some("account:synthetic".to_string()),
        connection_label: Some("person@example.test".to_string()),
        account_kind: "personal".to_string(),
        status: AdapterConnectionStatus::Active,
        revisions: AdapterConnectionRevisions {
            connection: 4,
            credential: 3,
            grant: 2,
            policy: 7,
        },
        credential_generation: Some(generation_id.clone()),
        granted_scopes: vec!["https://scope.example/items.read".to_string()],
        allowed_operations: vec!["list".to_string()],
        policy: Some(noema_capabilities::CapabilityConnectionPolicy {
            data_sharing: noema_capabilities::CapabilityDataSharingPolicy::AllowAutomatically,
            unsafe_actions: noema_capabilities::CapabilityUnsafeActionPolicy::ReviewerMayApprove,
            revision: 7,
        }),
        tool_overrides: Vec::new(),
    };
    let credential = AdapterCredentialGenerationV1 {
        schema_version: 1,
        generation_id,
        material: AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
            client_id: "synthetic-client".to_string(),
            client_secret: Some("client-secret-marker".to_string()),
            access_token: "access-secret-marker".to_string(),
            refresh_token: Some("refresh-secret-marker".to_string()),
            expires_at_epoch_seconds: Some(4_000_000_000),
        },
    };
    let connections = AdapterConnectionStore::new(paths.clone());
    connections
        .install(&descriptor, Some(&credential), &definition.compiled)
        .expect("connection");
    let definition_scan = definitions.scan().expect("definition scan");
    let connection_scan = connections
        .scan(&definition_scan.definitions)
        .expect("connection scan");
    let store = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path()))
        .await
        .expect("store");
    store
        .reconcile_adapter_definitions(&definition_scan.projections())
        .await
        .expect("definitions projection");
    store
        .reconcile_adapter_connections(&connection_scan.projections())
        .await
        .expect("connections projection");
    let before = store.adapter_connections().await.expect("connections");
    let database_bytes = std::fs::read(paths.sqlite_db_path()).expect("database bytes");
    for secret in [
        "client-secret-marker",
        "access-secret-marker",
        "refresh-secret-marker",
    ] {
        assert!(
            !database_bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes())
        );
    }
    drop(store);
    remove_database_family(&paths);

    let recreated = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path()))
        .await
        .expect("recreated store");
    let definition_scan = definitions.scan().expect("definition rescan");
    let connection_scan = connections
        .scan(&definition_scan.definitions)
        .expect("connection rescan");
    recreated
        .reconcile_adapter_definitions(&definition_scan.projections())
        .await
        .expect("definitions rebuild");
    recreated
        .reconcile_adapter_connections(&connection_scan.projections())
        .await
        .expect("connections rebuild");
    assert_eq!(
        recreated.adapter_connections().await.expect("connections"),
        before
    );

    connections.quarantine(&connection_id).expect("quarantine");
    let after_quarantine = connections
        .scan(&definition_scan.definitions)
        .expect("scan after quarantine");
    recreated
        .reconcile_adapter_connections(&after_quarantine.projections())
        .await
        .expect("quarantine projection");
    assert!(
        recreated
            .adapter_connections()
            .await
            .expect("connections")
            .is_empty()
    );
}

#[test]
fn projection_allows_revisions_but_rejects_duplicate_content_authority() {
    let row = DefinitionProjection {
        semantic_digest: "a".repeat(64),
        definition_id: Some("definition:duplicate".to_string()),
        adapter_id: Some("fixture".to_string()),
        source_digest: None,
        manifest_relative_path: format!("adapters/definitions/{}/manifest.json", "a".repeat(64)),
        provenance_relative_path: format!(
            "adapters/definitions/{}/provenance.json",
            "a".repeat(64)
        ),
        compile_status: "compiled",
        review_status: "reviewed",
        diagnostic_code: None,
        operation_count: 1,
        compiler_version: "test",
    };
    let mut other = row.clone();
    other.semantic_digest = "b".repeat(64);
    other.manifest_relative_path = format!(
        "adapters/definitions/{}/manifest.json",
        other.semantic_digest
    );
    other.provenance_relative_path = format!(
        "adapters/definitions/{}/provenance.json",
        other.semantic_digest
    );
    assert!(validate_snapshot(&[row.clone(), other]).is_ok());
    assert!(validate_snapshot(&[row.clone(), row]).is_err());
}
