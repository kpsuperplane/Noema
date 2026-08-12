use super::*;
use crate::{NoemaStore, StoreConfig};
use noema_capability_adapters::{
    AdapterConnectionAuthenticationV1, AdapterConnectionStatus, AdapterConnectionStore,
    AdapterConnectionV4, AdapterCredentialGenerationV2, AdapterCredentialMaterial,
    AdapterDefinitionStore, AdapterManifest, AuthorizationGrantStatus, AuthorizationGrantV1,
    ConnectionProjection, ExternalAccountV1, Oauth2CallbackMode, Oauth2ClientAuthentication,
    OauthApplicationStatus, OauthApplicationV1, OauthAuthoritySnapshot, OauthProfileInstall,
    OauthProfileV1, OauthScopeResponsePolicy,
};
use noema_home::NoemaPaths;

fn fixture_manifest(authentication: serde_json::Value) -> AdapterManifest {
    serde_json::from_value(serde_json::json!({
        "schema_version": 9,
        "definition_id": "definition:offline_fixture",
        "adapter_id": "offline_fixture",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": authentication,
        "operations": [{
            "operation_id":"list",
            "description":"List available items.",
            "method":"GET",
            "path":"/v1/items",
            "authorization":{"kind":"none"},
            "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
            "retry":"transport_safe_read",
            "pagination":{"kind":"none"},
            "response":{"accepted_content_types":["application/json"],"transform":{"language":"luau","source":"return function(response) return nil end"},"output_schema":{"type":"null"}}
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
            &fixture_manifest(serde_json::json!({"kind":"none"})),
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
                "kind":"credential",
                "setup":{"credential_type":"API token","setup_url":"https://developers.example.test/tokens","instructions":["Create an API token."],"input":{"kind":"fields","fields":[{"id":"token","label":"API token"}]}},
                "request_auth":{"language":"luau","source":"return function(input) return { headers = { Authorization = 'Bearer ' .. input.credentials.token } } end"}
            })),
            "fixture://independent-company-b/openapi.json",
            None,
            None,
        )
        .expect("definition");
    let connection_id = "a".repeat(32);
    let generation_id = "b".repeat(32);
    let descriptor = AdapterConnectionV4 {
        schema_version: 4,
        connection_id: connection_id.clone(),
        connection_slug: "personal".to_string(),
        semantic_digest: definition.compiled.semantic_digest.to_string(),
        connection_label: Some("person@example.test".to_string()),
        status: AdapterConnectionStatus::Active,
        connection_revision: 4,
        policy_revision: 7,
        authentication: AdapterConnectionAuthenticationV1::Credential {
            generation_id: generation_id.clone(),
            revision: 3,
        },
        allowed_operations: vec!["list".to_string()],
        policy: Some(noema_capabilities::CapabilityConnectionPolicy {
            data_sharing: noema_capabilities::CapabilityDataSharingPolicy::AllowAutomatically,
            unsafe_actions: noema_capabilities::CapabilityUnsafeActionPolicy::ReviewerMayApprove,
            revision: 7,
        }),
        tool_overrides: Vec::new(),
    };
    let credential = AdapterCredentialGenerationV2 {
        schema_version: 2,
        generation_id,
        material: AdapterCredentialMaterial::Credential {
            fields: std::collections::BTreeMap::from([(
                "token".to_string(),
                "access-secret-marker".to_string(),
            )]),
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
    for secret in ["access-secret-marker"] {
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

#[tokio::test]
async fn sqlite_rebuilds_the_complete_public_oauth_authority_hierarchy() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let profile_digest = "a".repeat(64);
    let application_id = "b".repeat(32);
    let account_id = "c".repeat(32);
    let grant_id = "d".repeat(32);
    let snapshot = OauthAuthoritySnapshot {
        profiles: vec![OauthProfileInstall {
            profile_digest: profile_digest.clone(),
            profile: OauthProfileV1 {
                schema_version: 1,
                profile_id: "google".into(),
                display_name: "Google".into(),
                authorization_endpoint: "https://accounts.example.test/authorize".into(),
                token_endpoint: "https://accounts.example.test/token".into(),
                client_authentication: Oauth2ClientAuthentication::None,
                setups: Vec::new(),
                authorization_parameters: Default::default(),
                account_selection_parameters: Default::default(),
                grant_audience: "google-apis".into(),
                omitted_scope_policy: OauthScopeResponsePolicy::RequireScope,
                preserve_refresh_token_on_expansion: true,
                account_identity: None,
            },
        }],
        applications: vec![OauthApplicationV1 {
            schema_version: 1,
            application_id: application_id.clone(),
            profile_digest: profile_digest.clone(),
            callback_mode: Oauth2CallbackMode::Hosted,
            client_id: "public-client".into(),
            project_label: Some("Personal".into()),
            credential_generation: "e".repeat(32),
            revision: 1,
            status: OauthApplicationStatus::Active,
        }],
        accounts: vec![ExternalAccountV1 {
            schema_version: 1,
            account_id: account_id.clone(),
            profile_digest,
            provider_subject: "subject-a".into(),
            account_label: Some("person@example.test".into()),
            revision: 1,
        }],
        grants: vec![AuthorizationGrantV1 {
            schema_version: 1,
            grant_id: grant_id.clone(),
            application_id,
            account_id: Some(account_id),
            account_label: None,
            audience: "google-apis".into(),
            desired_scopes: vec!["gmail.readonly".into(), "gmail.send".into()],
            granted_scopes: vec!["gmail.readonly".into()],
            authority_revision: 2,
            token_generation: Some("f".repeat(32)),
            token_revision: 4,
            status: AuthorizationGrantStatus::Active,
        }],
    };
    let store = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path()))
        .await
        .expect("store");
    store
        .reconcile_adapter_oauth_authorities(&snapshot)
        .await
        .expect("reconcile");
    let connections = vec![ConnectionProjection {
        connection_id: "1".repeat(32),
        connection_slug: Some("personal-oauth".into()),
        semantic_digest: Some("2".repeat(64)),
        connection_label: Some("Personal Google".into()),
        status: "active",
        connection_revision: Some(1),
        credential_revision: None,
        policy_revision: Some(1),
        credential_generation: None,
        grant_id: Some(grant_id),
        allowed_operations: vec!["list".into()],
        descriptor_relative_path: format!(
            "adapters/connections/{}/connection.json",
            "1".repeat(32)
        ),
        credential_relative_path: None,
        diagnostic_code: None,
    }];
    store
        .reconcile_complete_adapter_state(&snapshot, &connections)
        .await
        .expect("first complete reconcile");
    store
        .reconcile_complete_adapter_state(&snapshot, &connections)
        .await
        .expect("rebuild with referenced grant");
    store
        .with_connection(|connection| {
            assert_eq!(
                connection.query_row("SELECT COUNT(*) FROM adapter_oauth_profiles", [], |row| {
                    row.get::<_, usize>(0)
                })?,
                1
            );
            assert_eq!(
                connection.query_row(
                    "SELECT client_id FROM adapter_oauth_applications",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                "public-client"
            );
            assert_eq!(
                connection.query_row(
                    "SELECT account_label FROM adapter_external_accounts",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                "person@example.test"
            );
            assert_eq!(
                connection.query_row(
                    "SELECT json_array_length(desired_scopes_json), granted_scopes_json FROM adapter_oauth_grants",
                    [],
                    |row| Ok((row.get::<_, usize>(0)?, row.get::<_, String>(1)?)),
                )?,
                (2, "[\"gmail.readonly\"]".to_string())
            );
            assert_eq!(
                connection.query_row("SELECT COUNT(*) FROM adapter_connections", [], |row| {
                    row.get::<_, usize>(0)
                })?,
                1
            );
            assert!(connection
                .prepare("PRAGMA foreign_key_check")?
                .query([])?
                .next()?
                .is_none());
            Ok(())
        })
        .await
        .expect("projected rows");
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
