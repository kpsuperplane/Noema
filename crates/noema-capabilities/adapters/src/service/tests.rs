use super::*;
use crate::{
    AdapterManifestV3, HttpMethod, RetryPolicy,
    network::{
        AdapterBearerCredential, AdapterHttpError, AdapterHttpFuture, AdapterHttpResponse,
        AdapterOAuthTokenFuture, AdapterOAuthTokenOutcome,
    },
    request::EncodedAdapterRequest,
};
use noema_home::NoemaPaths;
use serde_json::json;
use std::{
    fs,
    sync::{Arc, Mutex},
};
use tokio::{sync::Notify, time::Duration};
use url::Url;

#[derive(Default)]
struct SyntheticOAuthHttp {
    exchanges: Mutex<usize>,
    gate: Option<Arc<ExchangeGate>>,
    identity_response: Option<AdapterHttpResponse>,
}

#[derive(Default)]
struct ExchangeGate {
    entered: Notify,
    release: Notify,
}

impl AdapterHttpExecutor for SyntheticOAuthHttp {
    fn execute(
        &self,
        _method: HttpMethod,
        _retry: RetryPolicy,
        _request: EncodedAdapterRequest,
        _credential: Option<AdapterBearerCredential>,
    ) -> AdapterHttpFuture<'_> {
        let response = self.identity_response.clone();
        Box::pin(async move { response.ok_or(AdapterHttpError::Unavailable) })
    }

    fn exchange_oauth_token(
        &self,
        request: AdapterOAuthTokenRequest,
    ) -> AdapterOAuthTokenFuture<'_> {
        let gate = self.gate.clone();
        Box::pin(async move {
            assert_eq!(
                request.token_endpoint.as_str(),
                "https://auth.example.test/token"
            );
            assert_eq!(
                request.client_authentication,
                crate::Oauth2ClientAuthentication::ClientSecretPost
            );
            assert_eq!(request.requested_scopes, ["calendar.read"]);
            *self.exchanges.lock().expect("exchanges") += 1;
            if let Some(gate) = gate {
                gate.entered.notify_one();
                gate.release.notified().await;
            }
            Ok(AdapterOAuthTokenOutcome {
                access_token: "access-secret-marker".to_string(),
                refresh_token: Some("refresh-secret-marker".to_string()),
                expires_at_epoch_seconds: Some(request.now_epoch_seconds + 3_600),
                granted_scopes: vec!["calendar.read".to_string()],
            })
        })
    }
}

fn manifest() -> AdapterManifestV3 {
    serde_json::from_value(json!({
        "schema_version": 3,
        "definition_id": "definition:service_oauth",
        "adapter_id": "service_oauth",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {
            "mode": "oauth2_authorization_code_pkce",
            "scopes": ["calendar.read"],
            "credential_import": {
                "kind": "oauth_client_json",
                "alternatives": [{
                    "client_id_pointer": "/installed/client_id",
                    "client_secret_pointer": "/installed/client_secret"
                }]
            },
            "oauth2": {
                "authorization_endpoint": "https://auth.example.test/authorize",
                "token_endpoint": "https://auth.example.test/token",
                "client_authentication": "client_secret_post",
                "callback_modes": ["loopback"],
                "extra_authorization_parameters": {}
            },
            "account_identity": {
                "operation_id": "get_profile",
                "arguments": {"user_id": "me"},
                "output_pointer": "/emailAddress"
            }
        },
        "quota": {"cost_class": "free"},
        "operations": [
            {
                "operation_id": "list_events",
                "method": "GET",
                "path": "/v1/events",
                "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"}
            },
            {
                "operation_id": "get_profile",
                "method": "GET",
                "path": "/v1/users/{user_id}/profile",
                "arguments": [{"name": "user_id", "source": "model_input", "location": "path", "type": "string", "required": true}],
                "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": false, "source": "model"}},
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"}
            }
        ]
    }))
    .expect("manifest")
}

#[tokio::test]
async fn management_writes_fence_stale_state_and_allow_disabling_every_tool() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let definition = AdapterDefinitionStore::new(paths.clone())
        .install(
            &manifest(),
            "https://developers.example.test/oauth",
            None,
            None,
        )
        .expect("definition");
    let service = AdapterCapabilityService::new(paths);
    let pending = service
        .import_oauth_client_json(
            definition.compiled.semantic_digest.as_str(),
            br#"{"installed":{"client_id":"client-marker","client_secret":"secret-marker"}}"#,
        )
        .await
        .expect("connection");
    let sibling = service
        .import_oauth_client_json(
            definition.compiled.semantic_digest.as_str(),
            br#"{"installed":{"client_id":"second-client","client_secret":"second-secret"}}"#,
        )
        .await
        .expect("independent sibling connection");
    assert_ne!(
        pending.descriptor.connection_id,
        sibling.descriptor.connection_id
    );
    assert_ne!(
        pending.descriptor.connection_slug,
        sibling.descriptor.connection_slug
    );
    assert_ne!(
        pending.descriptor.credential_generation,
        sibling.descriptor.credential_generation
    );
    let credential_generation = pending.descriptor.credential_generation.clone();
    let initial_fence = AdapterManagementFence {
        connection_id: pending.descriptor.connection_id.clone(),
        expected_connection_revision: pending.descriptor.revisions.connection,
        expected_policy_revision: pending.descriptor.revisions.policy,
    };
    let saved = service
        .save_management_policy(
            initial_fence.clone(),
            noema_capabilities::CapabilityDataSharingPolicy::AllowAutomatically,
            noema_capabilities::CapabilityUnsafeActionPolicy::ReviewerMayApprove,
        )
        .await
        .expect("policy");
    assert_eq!(
        service
            .save_management_policy(
                initial_fence,
                noema_capabilities::CapabilityDataSharingPolicy::AllowAutomatically,
                noema_capabilities::CapabilityUnsafeActionPolicy::ReviewerMayApprove,
            )
            .await
            .expect_err("stale write"),
        AdapterManagementError::Conflict
    );
    let operation = &definition.compiled.operations[0];
    let partly_disabled = service
        .set_management_tool_enabled(
            AdapterManagementFence {
                connection_id: saved.descriptor.connection_id.clone(),
                expected_connection_revision: saved.descriptor.revisions.connection,
                expected_policy_revision: saved.descriptor.revisions.policy,
            },
            operation.operation_id.clone(),
            operation.operation_digest.to_string(),
            false,
        )
        .await
        .expect("disable first tool");
    let operation = definition
        .compiled
        .operations
        .iter()
        .find(|operation| operation.operation_id == "list_events")
        .expect("remaining operation");
    let disabled = service
        .set_management_tool_enabled(
            AdapterManagementFence {
                connection_id: partly_disabled.descriptor.connection_id.clone(),
                expected_connection_revision: partly_disabled.descriptor.revisions.connection,
                expected_policy_revision: partly_disabled.descriptor.revisions.policy,
            },
            operation.operation_id.clone(),
            operation.operation_digest.to_string(),
            false,
        )
        .await
        .expect("disable last tool");
    assert_eq!(disabled.descriptor.allowed_operations, Vec::<String>::new());
    assert_eq!(
        disabled.descriptor.credential_generation,
        credential_generation
    );
}

#[test]
fn legacy_rewrite_preserves_active_credentials_and_is_idempotent() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let definitions = AdapterDefinitionStore::new(paths.clone());
    let v2 = definitions
        .install(
            &manifest(),
            "https://developers.example.test/oauth",
            None,
            None,
        )
        .expect("v2 definition");

    let mut legacy = serde_json::to_value(manifest()).expect("legacy value");
    let object = legacy.as_object_mut().expect("manifest object");
    object.insert("schema_version".to_string(), json!(1));
    object.insert(
        "provider_data_policy".to_string(),
        json!({"retention_allowed": true, "deletion_supported": true}),
    );
    object["authentication"]
        .as_object_mut()
        .expect("authentication")
        .remove("account_identity");
    for operation in object["operations"].as_array_mut().expect("operations") {
        let operation = operation.as_object_mut().expect("operation");
        operation.remove("behavior");
        operation.insert("effect".to_string(), json!("read_only"));
        operation.insert("admission".to_string(), json!("direct"));
        operation.insert(
            "result".to_string(),
            json!({
                "classification": "private",
                "model_route": "local_only",
                "model_payload": "full",
                "provider_retention": "deny",
                "persistence": "omit"
            }),
        );
    }
    let old_digest = crate::SemanticDigest::compute(
        &crate::digest::canonical_json_bytes(&crate::digest::semantic_manifest_json_value(
            legacy.clone(),
        ))
        .expect("legacy semantic bytes"),
    );
    let legacy_dir = paths
        .adapter_definition_dir(old_digest.as_str())
        .expect("legacy path");
    crate::private_fs::create_private_dir(&legacy_dir).expect("legacy directory");
    crate::private_fs::write_new_file(
        &legacy_dir.join("manifest.json"),
        &crate::digest::canonical_json_bytes(&legacy).expect("legacy manifest bytes"),
    )
    .expect("legacy manifest");
    crate::private_fs::write_new_file(
        &legacy_dir.join("provenance.json"),
        &crate::digest::canonical_json_bytes(&json!({
            "source_reference": "https://developers.example.test/oauth"
        }))
        .expect("legacy provenance bytes"),
    )
    .expect("legacy provenance");
    let credential = AdapterCredentialGenerationV1 {
        schema_version: 1,
        generation_id: "a".repeat(32),
        material: AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
            client_id: "client-marker".to_string(),
            client_secret: Some("client-secret-marker".to_string()),
            access_token: "access-secret-marker".to_string(),
            refresh_token: Some("refresh-secret-marker".to_string()),
            expires_at_epoch_seconds: Some(4_000),
        },
    };
    let mut descriptor = AdapterConnectionV2 {
        schema_version: 2,
        connection_id: "b".repeat(32),
        connection_slug: "personal".to_string(),
        semantic_digest: v2.compiled.semantic_digest.to_string(),
        account_id: None,
        account_label: None,
        account_kind: "personal_user".to_string(),
        status: AdapterConnectionStatus::Active,
        revisions: AdapterConnectionRevisions {
            connection: 2,
            credential: 2,
            grant: 2,
            policy: 1,
        },
        credential_generation: Some(credential.generation_id.clone()),
        granted_scopes: vec!["calendar.read".to_string()],
        allowed_operations: vec!["list_events".to_string()],
        policy: Some(noema_capabilities::CapabilityConnectionPolicy {
            data_sharing: noema_capabilities::CapabilityDataSharingPolicy::AllowAutomatically,
            unsafe_actions: noema_capabilities::CapabilityUnsafeActionPolicy::ReviewerMayApprove,
            revision: 1,
        }),
        tool_overrides: Vec::new(),
    };
    let connections = AdapterConnectionStore::new(paths.clone());
    connections
        .install(&descriptor, Some(&credential), &v2.compiled)
        .expect("active connection");
    descriptor.semantic_digest = old_digest.to_string();
    descriptor.schema_version = 1;
    descriptor.policy = None;
    descriptor.tool_overrides.clear();
    let connection_dir = paths
        .adapter_connection_dir(&descriptor.connection_id)
        .expect("connection path");
    fs::write(
        connection_dir.join("connection.json"),
        crate::digest::canonical_json_bytes(
            &serde_json::to_value(&descriptor).expect("descriptor value"),
        )
        .expect("descriptor bytes"),
    )
    .expect("legacy descriptor");
    let credential_path = connection_dir
        .join("credentials")
        .join(format!("{}.json", credential.generation_id));
    let credential_bytes = fs::read(&credential_path).expect("credential bytes");

    let service = AdapterCapabilityService::new(paths.clone());
    service.prepare_filesystem().expect("legacy rewrite");
    let connection_scan = connections
        .scan(&definitions.scan().expect("definitions").definitions)
        .expect("connections");
    assert!(
        connection_scan.diagnostics.is_empty(),
        "migration diagnostics: {:?}",
        connection_scan.diagnostics
    );
    let migrated = connection_scan
        .connections
        .into_iter()
        .next()
        .expect("migrated connection")
        .descriptor;
    assert_ne!(migrated.semantic_digest, old_digest.as_str());
    assert_eq!(migrated.schema_version, 2);
    assert!(migrated.policy.is_none());
    assert!(migrated.tool_overrides.is_empty());
    assert_eq!(migrated.revisions.connection, 4);
    assert_eq!(migrated.revisions.policy, 3);
    assert_eq!(migrated.revisions.credential, 2);
    assert_eq!(migrated.revisions.grant, 2);
    assert_eq!(
        fs::read(&credential_path).expect("credential after rewrite"),
        credential_bytes
    );
    assert!(!legacy_dir.exists());
    assert!(
        paths
            .adapter_quarantine_dir()
            .join("definitions")
            .join(old_digest.as_str())
            .is_dir()
    );

    service.prepare_filesystem().expect("idempotent rewrite");
    let stable: AdapterConnectionV2 = serde_json::from_slice(
        &fs::read(connection_dir.join("connection.json")).expect("stable descriptor"),
    )
    .expect("stable descriptor JSON");
    assert_eq!(stable.revisions, migrated.revisions);
}

#[tokio::test]
async fn setup_callback_exchanges_once_and_publishes_active_token_generation() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let mut transformed_manifest = manifest();
    transformed_manifest.operations[1].response = Some(
        serde_json::from_value(json!({
            "accepted_content_types": ["application/json"],
            "transform": {
                "language": "luau",
                "source": "return function(response)\n  local body = json.decode(response.body)\n  return { emailAddress = body.profile.email }\nend"
            },
            "output_schema": {
                "type": "object",
                "properties": {"emailAddress": {"type": "string"}},
                "required": ["emailAddress"],
                "additionalProperties": false
            }
        }))
        .expect("identity response contract"),
    );
    let definition = AdapterDefinitionStore::new(paths.clone())
        .install(
            &transformed_manifest,
            "https://developers.example.test/oauth",
            None,
            None,
        )
        .expect("definition");
    let http = Arc::new(SyntheticOAuthHttp {
        exchanges: Mutex::new(0),
        gate: None,
        identity_response: Some(AdapterHttpResponse {
            status: 200,
            content_type: Some("application/json".to_string()),
            body: br#"{"profile":{"email":"person@example.test"}}"#.to_vec(),
        }),
    });
    let service = AdapterCapabilityService::new_with_http_for_tests(paths.clone(), http.clone());
    let pending = service
        .import_oauth_client_json(
            definition.compiled.semantic_digest.as_str(),
            br#"{"installed":{"client_id":"client-marker","client_secret":"secret-marker"}}"#,
        )
        .await
        .expect("client metadata");
    let redirect_uri = "http://127.0.0.1:43123/adapter/oauth/callback";
    let mut stale_revisions = pending.descriptor.revisions;
    stale_revisions.policy += 1;
    assert_eq!(
        service
            .start_oauth_setup_at(
                "human:local",
                &pending.descriptor.connection_id,
                stale_revisions,
                Oauth2CallbackMode::Loopback,
                redirect_uri,
                100,
            )
            .await
            .expect_err("stale UI revision"),
        AdapterOAuthSetupError::Superseded
    );
    let started = service
        .start_oauth_setup_at(
            "human:local",
            &pending.descriptor.connection_id,
            pending.descriptor.revisions,
            Oauth2CallbackMode::Loopback,
            redirect_uri,
            100,
        )
        .await
        .expect("start");
    assert!(!format!("{started:?}").contains("state="));
    let state = Url::parse(&started.authorization_url)
        .expect("authorization URL")
        .query_pairs()
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.into_owned())
        .expect("state");
    let callback = format!("{redirect_uri}?code=code-marker&state={state}");
    let active = service
        .complete_oauth_callback_at(&callback, 101)
        .await
        .expect("complete");
    assert_eq!(active.descriptor.status, AdapterConnectionStatus::Active);
    assert_eq!(active.descriptor.revisions.connection, 2);
    assert_eq!(active.descriptor.revisions.credential, 2);
    assert_eq!(active.descriptor.revisions.grant, 2);
    assert_eq!(
        active.descriptor.account_label.as_deref(),
        Some("person@example.test")
    );
    assert_eq!(*http.exchanges.lock().expect("exchanges"), 1);
    assert!(matches!(
        service.complete_oauth_callback_at(&callback, 102).await,
        Err(AdapterOAuthSetupError::Invalid)
    ));

    let (_, credential) = AdapterConnectionStore::new(paths)
        .load_for_invocation(&active.descriptor.connection_id, &definition.compiled)
        .expect("active credential");
    let Some(AdapterCredentialGenerationV1 {
        material:
            AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
                access_token,
                refresh_token,
                ..
            },
        ..
    }) = credential
    else {
        panic!("active OAuth token")
    };
    assert_eq!(access_token, "access-secret-marker");
    assert_eq!(refresh_token.as_deref(), Some("refresh-secret-marker"));
}

#[tokio::test]
async fn token_exchange_releases_connection_lock_and_reserves_attempt() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let definition = AdapterDefinitionStore::new(paths.clone())
        .install(
            &manifest(),
            "https://developers.example.test/oauth",
            None,
            None,
        )
        .expect("definition");
    let gate = Arc::new(ExchangeGate::default());
    let http = Arc::new(SyntheticOAuthHttp {
        exchanges: Mutex::new(0),
        gate: Some(gate.clone()),
        identity_response: None,
    });
    let service = AdapterCapabilityService::new_with_http_for_tests(paths, http);
    let pending = service
        .import_oauth_client_json(
            definition.compiled.semantic_digest.as_str(),
            br#"{"installed":{"client_id":"client-marker","client_secret":"secret-marker"}}"#,
        )
        .await
        .expect("client metadata");
    let redirect_uri = "http://127.0.0.1:43123/adapter/oauth/callback";
    let started = service
        .start_oauth_setup_at(
            "human:local",
            &pending.descriptor.connection_id,
            pending.descriptor.revisions,
            Oauth2CallbackMode::Loopback,
            redirect_uri,
            100,
        )
        .await
        .expect("start");
    let state = Url::parse(&started.authorization_url)
        .expect("authorization URL")
        .query_pairs()
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.into_owned())
        .expect("state");
    let callback = format!("{redirect_uri}?code=code-marker&state={state}");
    let completion_service = service.clone();
    let completion = tokio::spawn(async move {
        completion_service
            .complete_oauth_callback_at(&callback, 101)
            .await
    });
    gate.entered.notified().await;

    let restart = tokio::time::timeout(
        Duration::from_millis(100),
        service.start_oauth_setup_at(
            "human:local",
            &pending.descriptor.connection_id,
            pending.descriptor.revisions,
            Oauth2CallbackMode::Loopback,
            redirect_uri,
            102,
        ),
    )
    .await
    .expect("external exchange does not hold the connection lock");
    assert_eq!(restart, Err(AdapterOAuthSetupError::Unavailable));

    gate.release.notify_one();
    let completed = completion
        .await
        .expect("completion task")
        .expect("completion");
    assert_eq!(completed.descriptor.status, AdapterConnectionStatus::Active);
    assert_eq!(completed.descriptor.account_label, None);
}
