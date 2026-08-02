use super::*;
use crate::{
    AdapterManifestV4, HttpMethod, RetryPolicy,
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

fn manifest() -> AdapterManifestV4 {
    serde_json::from_value(json!({
        "schema_version": 4,
        "definition_id": "definition:service_oauth",
        "adapter_id": "service_oauth",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {
            "kind": "oauth2_authorization_code_pkce",
            "scopes": ["calendar.read"],
            "authorization_endpoint": "https://auth.example.test/authorize",
            "token_endpoint": "https://auth.example.test/token",
            "client_authentication": "client_secret_post",
            "setups": [{
                "callback_mode": "loopback",
                "setup": {
                    "credential_type": "Desktop app",
                    "setup_url": "https://developers.example.test/oauth/clients/new",
                    "instructions": ["Create a Desktop app OAuth client and download its JSON."],
                    "input": {
                        "kind": "document",
                        "media_type": "application/json",
                        "fields": [
                            {"id": "client_id", "label": "Client ID"},
                            {"id": "client_secret", "label": "Client secret"}
                        ],
                        "normalize": {
                            "language": "luau",
                            "source": "return function(input) local document = json.decode(input.document) return { client_id = document.installed.client_id, client_secret = document.installed.client_secret } end"
                        }
                    }
                }
            }],
            "extra_authorization_parameters": {},
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

fn service(paths: NoemaPaths) -> AdapterCapabilityService {
    let service = AdapterCapabilityService::new(paths);
    service.set_oauth_callback_mode(Oauth2CallbackMode::Loopback);
    service
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
    let service = service(paths);
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
    let labeled = service
        .save_connection_label(
            pending.descriptor.connection_id.clone(),
            pending.descriptor.revisions.connection,
            None,
            Some(" Personal API ".to_string()),
        )
        .await
        .expect("set connection label");
    assert_eq!(
        labeled.descriptor.connection_label.as_deref(),
        Some("Personal API")
    );
    assert_eq!(labeled.descriptor.revisions, pending.descriptor.revisions);
    assert_eq!(
        labeled.descriptor.credential_generation,
        pending.descriptor.credential_generation
    );
    let cleared = service
        .save_connection_label(
            pending.descriptor.connection_id.clone(),
            pending.descriptor.revisions.connection,
            Some("Personal API".to_string()),
            Some("  ".to_string()),
        )
        .await
        .expect("clear connection label");
    assert_eq!(cleared.descriptor.connection_label, None);
    assert_eq!(cleared.descriptor.revisions, pending.descriptor.revisions);
    assert_eq!(
        service
            .save_connection_label(
                pending.descriptor.connection_id.clone(),
                pending.descriptor.revisions.connection,
                Some("Personal API".to_string()),
                Some("Stale".to_string()),
            )
            .await
            .expect_err("stale label"),
        AdapterManagementError::Conflict
    );
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
fn legacy_state_is_recoverably_invalidated_and_idempotent() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let definitions = AdapterDefinitionStore::new(paths.clone());
    let definition = definitions
        .install(
            &manifest(),
            "https://developers.example.test/oauth",
            None,
            None,
        )
        .expect("definition");
    let digest = definition.compiled.semantic_digest.to_string();
    let definition_dir = paths.adapter_definition_dir(&digest).expect("legacy path");
    let credential = AdapterCredentialGenerationV2 {
        schema_version: 2,
        generation_id: "a".repeat(32),
        material: AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
            callback_mode: Oauth2CallbackMode::Loopback,
            client_id: "client-marker".to_string(),
            client_secret: Some("client-secret-marker".to_string()),
            access_token: "access-secret-marker".to_string(),
            refresh_token: Some("refresh-secret-marker".to_string()),
            expires_at_epoch_seconds: Some(4_000),
        },
    };
    let descriptor = AdapterConnectionV3 {
        schema_version: 3,
        connection_id: "b".repeat(32),
        connection_slug: "personal".to_string(),
        semantic_digest: digest.clone(),
        account_id: None,
        connection_label: None,
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
        .install(&descriptor, Some(&credential), &definition.compiled)
        .expect("active connection");
    let connection_dir = paths
        .adapter_connection_dir(&descriptor.connection_id)
        .expect("connection path");
    let mut legacy = serde_json::to_value(manifest()).expect("legacy value");
    legacy["schema_version"] = json!(3);
    fs::write(
        definition_dir.join("manifest.json"),
        crate::digest::canonical_json_bytes(&legacy).expect("legacy manifest bytes"),
    )
    .expect("legacy manifest");

    let service = service(paths.clone());
    service.prepare_filesystem().expect("legacy invalidation");
    assert!(!definition_dir.exists());
    assert!(!connection_dir.exists());
    assert!(
        paths
            .adapter_quarantine_dir()
            .join("definitions")
            .join(&digest)
            .is_dir()
    );
    let quarantined_connection = paths
        .quarantined_adapter_connection_dir(&descriptor.connection_id)
        .expect("quarantined connection");
    assert!(quarantined_connection.is_dir());
    assert!(
        quarantined_connection
            .join("credentials")
            .join(format!("{}.json", credential.generation_id))
            .is_file()
    );

    service
        .prepare_filesystem()
        .expect("idempotent invalidation");
    assert!(quarantined_connection.is_dir());
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
    service.set_oauth_callback_mode(Oauth2CallbackMode::Loopback);
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
    assert!(active.newly_activated);
    assert_eq!(
        active.connection.descriptor.status,
        AdapterConnectionStatus::Active
    );
    assert_eq!(active.connection.descriptor.revisions.connection, 2);
    assert_eq!(active.connection.descriptor.revisions.credential, 2);
    assert_eq!(active.connection.descriptor.revisions.grant, 2);
    assert_eq!(
        active.connection.descriptor.connection_label.as_deref(),
        Some("person@example.test")
    );
    assert_eq!(*http.exchanges.lock().expect("exchanges"), 1);
    assert!(matches!(
        service.complete_oauth_callback_at(&callback, 102).await,
        Err(AdapterOAuthSetupError::Invalid)
    ));

    let customized = service
        .save_connection_label(
            active.connection.descriptor.connection_id.clone(),
            active.connection.descriptor.revisions.connection,
            Some("person@example.test".to_string()),
            Some("My calendar".to_string()),
        )
        .await
        .expect("custom label");
    let reauthentication = service
        .start_oauth_setup_at(
            "human:local",
            &customized.descriptor.connection_id,
            customized.descriptor.revisions,
            Oauth2CallbackMode::Loopback,
            redirect_uri,
            103,
        )
        .await
        .expect("restart authentication");
    let reauthentication_state = Url::parse(&reauthentication.authorization_url)
        .expect("reauthentication URL")
        .query_pairs()
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.into_owned())
        .expect("reauthentication state");
    let reauthentication_callback =
        format!("{redirect_uri}?code=second-code&state={reauthentication_state}");
    let reauthenticated = service
        .complete_oauth_callback_at(&reauthentication_callback, 104)
        .await
        .expect("reauthenticate");
    assert_eq!(
        reauthenticated
            .connection
            .descriptor
            .connection_label
            .as_deref(),
        Some("My calendar")
    );
    assert_eq!(*http.exchanges.lock().expect("exchanges"), 2);

    let (_, credential) = AdapterConnectionStore::new(paths)
        .load_for_invocation(
            &active.connection.descriptor.connection_id,
            &definition.compiled,
        )
        .expect("active credential");
    let Some(AdapterCredentialGenerationV2 {
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
    service.set_oauth_callback_mode(Oauth2CallbackMode::Loopback);
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
    assert_eq!(
        completed.connection.descriptor.status,
        AdapterConnectionStatus::Active
    );
    assert_eq!(completed.connection.descriptor.connection_label, None);
}
