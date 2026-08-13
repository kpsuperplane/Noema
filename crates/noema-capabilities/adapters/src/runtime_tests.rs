use crate::{
    AdapterCapabilityService, AdapterConnectionAuthenticationV1, AdapterConnectionStatus,
    AdapterConnectionStore, AdapterConnectionV4, AdapterDefinitionStore, AdapterManifest,
    AuthorizationGrantStatus, AuthorizationGrantV1, OauthApplicationCredentialV1,
    OauthApplicationStatus, OauthApplicationV1, OauthAuthorityStore, OauthGrantTokenV1,
    network::{
        AdapterBearerCredential, AdapterHttpExecutor, AdapterHttpFuture, AdapterHttpResponse,
        AdapterOAuthTokenError, AdapterOAuthTokenFuture, AdapterOAuthTokenOutcome,
        AdapterOAuthTokenRequest,
    },
    request::EncodedAdapterRequest,
};
use noema_capabilities::{
    CapabilityAuthenticationAuthorityKind, CapabilityBindingSource, CapabilityError,
    CapabilityInvocation, CapabilityInvoker, ReviewedCapabilityAuthorization,
};
use noema_home::NoemaPaths;
use serde_json::json;
use std::sync::{Arc, Mutex};

struct GrantHttp {
    token_result: Mutex<Option<Result<AdapterOAuthTokenOutcome, AdapterOAuthTokenError>>>,
    exchanges: Mutex<usize>,
    bearers: Mutex<Vec<String>>,
}

impl AdapterHttpExecutor for GrantHttp {
    fn execute(
        &self,
        _method: crate::HttpMethod,
        _retry: crate::RetryPolicy,
        _request: EncodedAdapterRequest,
        credential: Option<AdapterBearerCredential>,
    ) -> AdapterHttpFuture<'_> {
        if let Some(credential) = credential {
            self.bearers
                .lock()
                .expect("bearers")
                .push(credential.into_secret_for_tests());
        }
        Box::pin(async {
            Ok(AdapterHttpResponse {
                status: 200,
                content_type: Some("application/json".to_string()),
                body: br#"{"id":"item-one"}"#.to_vec(),
            })
        })
    }

    fn exchange_oauth_token(
        &self,
        _request: AdapterOAuthTokenRequest,
    ) -> AdapterOAuthTokenFuture<'_> {
        *self.exchanges.lock().expect("exchanges") += 1;
        let result = self
            .token_result
            .lock()
            .expect("token result")
            .take()
            .unwrap_or(Err(AdapterOAuthTokenError::Unavailable));
        Box::pin(async move { result })
    }
}

struct GrantFixture {
    _home: tempfile::TempDir,
    paths: NoemaPaths,
    service: AdapterCapabilityService,
    http: Arc<GrantHttp>,
    grant_id: String,
    application_id: String,
    semantic_digest: String,
}

#[tokio::test]
async fn reviewed_enablement_restores_one_disabled_adapter_tool() {
    let fixture = grant_fixture(Err(AdapterOAuthTokenError::Unavailable));
    let snapshot = fixture.service.management_snapshot().expect("management");
    let connection = snapshot
        .connections
        .connections
        .iter()
        .find(|connection| connection.descriptor.connection_slug == "first")
        .expect("connection");
    let operation = &snapshot.definitions.definitions[0].compiled.operations[0];
    fixture
        .service
        .set_management_tool_enabled(
            crate::AdapterManagementFence {
                connection_id: connection.descriptor.connection_id.clone(),
                expected_connection_revision: connection.descriptor.connection_revision,
                expected_policy_revision: connection.descriptor.policy_revision,
            },
            operation.operation_id.clone(),
            operation.operation_digest.to_string(),
            false,
        )
        .await
        .expect("disable tool");

    let disabled = CapabilityBindingSource::catalog(&fixture.service)
        .await
        .expect("disabled catalog");
    let disabled_name = "shared_grant_fixture_first.get_item";
    let enablement = disabled
        .snapshot
        .resolve("enable.shared_grant_fixture_first.get_item")
        .expect("enablement tool");

    let arguments = json!({});
    CapabilityInvoker::invoke(
        &fixture.service,
        CapabilityInvocation {
            operation: enablement.spec().name.clone(),
            operation_token: enablement.target().operation_token().clone(),
            arguments: arguments.clone(),
            reviewed_authorization: Some(ReviewedCapabilityAuthorization::for_action(
                "action:test",
                1,
                &arguments,
            )),
        },
    )
    .await
    .expect("enable tool");

    let restored = CapabilityBindingSource::catalog(&fixture.service)
        .await
        .expect("restored catalog");
    assert!(restored.snapshot.resolve(disabled_name).is_some());
}

fn grant_fixture(
    token_result: Result<AdapterOAuthTokenOutcome, AdapterOAuthTokenError>,
) -> GrantFixture {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let authorities = OauthAuthorityStore::new(paths.clone());
    let profile = authorities
        .install_profile(&crate::reviewed_google_oauth_profile())
        .expect("profile");
    let manifest: AdapterManifest = serde_json::from_value(json!({
        "schema_version": 9,
        "definition_id": "definition:shared_grant_fixture",
        "adapter_id": "shared_grant_fixture",
        "display_name": "Shared grant fixture",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {
            "kind": "oauth2_authorization_code_pkce",
            "profile_digest": profile.profile_digest
        },
        "operations": [{
            "operation_id": "get_item",
            "description": "Get one item.",
            "method": "GET",
            "path": "/v1/items/one",
            "authorization": {"kind": "oauth_scopes", "accepted_scope_sets": [["scope.read"]]},
            "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
            "retry": "transport_safe_read",
            "pagination": {"kind": "none"},
            "response": {
                "accepted_content_types": ["application/json"],
                "output_schema": {"type": "object", "properties": {"id": {"type": "string", "maxBytes": 64}}, "required": ["id"], "additionalProperties": false}
            }
        }]
    }))
    .expect("manifest");
    let definition = AdapterDefinitionStore::new(paths.clone())
        .install(&manifest, "https://developers.example.test/api", None, None)
        .expect("definition");
    let application_id = "a".repeat(32);
    let app_generation = "b".repeat(32);
    authorities
        .install_application(
            &OauthApplicationV1 {
                schema_version: 1,
                application_id: application_id.clone(),
                profile_digest: profile.profile_digest,
                callback_mode: crate::Oauth2CallbackMode::Loopback,
                client_id: "client-marker".to_string(),
                project_label: None,
                credential_generation: app_generation.clone(),
                revision: 1,
                status: OauthApplicationStatus::Active,
            },
            &OauthApplicationCredentialV1 {
                schema_version: 1,
                generation_id: app_generation,
                client_secret: Some("client-secret-marker".to_string()),
            },
        )
        .expect("application");
    let grant_id = "c".repeat(32);
    let token_generation = "d".repeat(32);
    authorities
        .install_grant(
            &AuthorizationGrantV1 {
                schema_version: 1,
                grant_id: grant_id.clone(),
                application_id,
                account_id: None,
                account_label: Some("Personal Google".to_string()),
                audience: "google-apis".to_string(),
                desired_scopes: vec!["scope.extra".to_string(), "scope.read".to_string()],
                granted_scopes: vec!["scope.extra".to_string(), "scope.read".to_string()],
                authority_revision: 3,
                token_generation: Some(token_generation.clone()),
                token_revision: 4,
                status: AuthorizationGrantStatus::Active,
            },
            Some(&OauthGrantTokenV1 {
                schema_version: 1,
                generation_id: token_generation,
                access_token: "stale-access".to_string(),
                refresh_token: Some("refresh-marker".to_string()),
                expires_at_epoch_seconds: Some(1),
            }),
        )
        .expect("grant");
    for (id, slug) in [("e".repeat(32), "first"), ("f".repeat(32), "second")] {
        AdapterConnectionStore::new(paths.clone())
            .install(
                &AdapterConnectionV4 {
                    schema_version: 4,
                    connection_id: id,
                    connection_slug: slug.to_string(),
                    semantic_digest: definition.compiled.semantic_digest.to_string(),
                    connection_label: None,
                    status: AdapterConnectionStatus::Active,
                    connection_revision: 1,
                    policy_revision: 1,
                    authentication: AdapterConnectionAuthenticationV1::OauthGrant {
                        grant_id: grant_id.clone(),
                    },
                    allowed_operations: vec!["get_item".to_string()],
                    policy: Some(noema_capabilities::CapabilityConnectionPolicy {
                        data_sharing:
                            noema_capabilities::CapabilityDataSharingPolicy::AllowAutomatically,
                        unsafe_actions:
                            noema_capabilities::CapabilityUnsafeActionPolicy::ReviewerMayApprove,
                        revision: 1,
                    }),
                    tool_overrides: Vec::new(),
                },
                None,
                &definition.compiled,
            )
            .expect("connection");
    }
    let http = Arc::new(GrantHttp {
        token_result: Mutex::new(Some(token_result)),
        exchanges: Mutex::new(0),
        bearers: Mutex::new(Vec::new()),
    });
    let service = AdapterCapabilityService::new_with_http_for_tests(paths.clone(), http.clone());
    service.set_oauth_callback_mode(crate::Oauth2CallbackMode::Loopback);
    GrantFixture {
        _home: home,
        paths,
        service,
        http,
        grant_id,
        application_id: "a".repeat(32),
        semantic_digest: definition.compiled.semantic_digest.to_string(),
    }
}

fn install_oauth_definition(
    paths: &NoemaPaths,
    base_digest: &str,
    profile_digest: &str,
    suffix: &str,
    scope: &str,
) -> String {
    let store = AdapterDefinitionStore::new(paths.clone());
    let mut value =
        serde_json::to_value(store.load(base_digest).expect("base definition").manifest)
            .expect("manifest value");
    value["definition_id"] = json!(format!("definition:{suffix}"));
    value["adapter_id"] = json!(suffix);
    value["display_name"] = json!(format!("{suffix} fixture"));
    value["authentication"]["profile_digest"] = json!(profile_digest);
    value["operations"][0]["operation_id"] = json!("use_service");
    value["operations"][0]["authorization"]["accepted_scope_sets"] = json!([[scope]]);
    let manifest = serde_json::from_value(value).expect("manifest");
    store
        .install(&manifest, "https://developers.example.test/api", None, None)
        .expect("definition")
        .compiled
        .semantic_digest
        .to_string()
}

#[tokio::test]
async fn one_oauth_attempt_unions_scopes_for_selected_services() {
    let fixture = grant_fixture(Err(AdapterOAuthTokenError::Unavailable));
    let additional_digest = install_oauth_definition(
        &fixture.paths,
        &fixture.semantic_digest,
        &crate::reviewed_google_oauth_profile_digest(),
        "additional_service",
        "scope.write",
    );
    let started = fixture
        .service
        .start_oauth_authorization(
            "human:local",
            crate::AdapterOAuthAuthorizationRequest {
                application_id: fixture.application_id,
                expected_application_revision: 1,
                grant_id: None,
                expected_grant_revision: None,
                semantic_digest: fixture.semantic_digest.clone(),
                operation_ids: vec!["get_item".to_string()],
                additional_services: vec![crate::AdapterOAuthServiceSelection {
                    semantic_digest: additional_digest,
                    operation_ids: vec!["use_service".to_string()],
                }],
                callback_mode: crate::Oauth2CallbackMode::Loopback,
                redirect_uri: "http://localhost:43123/adapter/oauth/callback".to_string(),
            },
        )
        .await
        .expect("start authorization");
    let authorization_url = url::Url::parse(&started.authorization_url).expect("authorization URL");
    let scopes = authorization_url
        .query_pairs()
        .find(|(name, _)| name == "scope")
        .map(|(_, value)| value.split(' ').map(str::to_string).collect::<Vec<_>>())
        .expect("scopes");
    assert_eq!(scopes, ["scope.read", "scope.write"]);
}

#[tokio::test]
async fn one_oauth_attempt_rejects_a_duplicate_service_selection() {
    let fixture = grant_fixture(Err(AdapterOAuthTokenError::Unavailable));
    let result = fixture
        .service
        .start_oauth_authorization(
            "human:local",
            crate::AdapterOAuthAuthorizationRequest {
                application_id: fixture.application_id,
                expected_application_revision: 1,
                grant_id: None,
                expected_grant_revision: None,
                semantic_digest: fixture.semantic_digest.clone(),
                operation_ids: vec!["get_item".to_string()],
                additional_services: vec![crate::AdapterOAuthServiceSelection {
                    semantic_digest: fixture.semantic_digest.clone(),
                    operation_ids: vec!["get_item".to_string()],
                }],
                callback_mode: crate::Oauth2CallbackMode::Loopback,
                redirect_uri: "http://localhost:43123/adapter/oauth/callback".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(crate::AdapterOAuthSetupError::Invalid)
    ));
}

#[tokio::test]
async fn two_connections_share_one_refresh_result() {
    let fixture = grant_fixture(Ok(AdapterOAuthTokenOutcome {
        access_token: "fresh-access".to_string(),
        refresh_token: None,
        expires_at_epoch_seconds: Some(u64::MAX - 1),
        granted_scopes: vec!["scope.read".to_string()],
    }));
    let catalog = CapabilityBindingSource::catalog(&fixture.service)
        .await
        .expect("catalog");
    let invocations = catalog
        .snapshot
        .iter()
        .filter(|binding| binding.destination().is_some())
        .map(|binding| CapabilityInvocation {
            operation: binding.spec().name.clone(),
            operation_token: binding.target().operation_token().clone(),
            arguments: json!({}),
            reviewed_authorization: None,
        })
        .collect::<Vec<_>>();
    assert_eq!(invocations.len(), 2);
    let (first, second) = tokio::join!(
        CapabilityInvoker::invoke(&fixture.service, invocations[0].clone()),
        CapabilityInvoker::invoke(&fixture.service, invocations[1].clone()),
    );
    assert!(first.is_ok());
    assert!(second.is_ok());
    assert_eq!(*fixture.http.exchanges.lock().expect("exchanges"), 1);
    assert_eq!(
        fixture.http.bearers.lock().expect("bearers").as_slice(),
        ["fresh-access", "fresh-access"]
    );
    let (grant, _) = OauthAuthorityStore::new(fixture.paths)
        .load_grant_authority(&fixture.grant_id)
        .expect("refreshed grant");
    assert_eq!(grant.granted_scopes, ["scope.extra", "scope.read"]);
    assert_eq!(grant.token_revision, 5);
}

#[tokio::test]
async fn refresh_without_the_called_scope_requires_authentication() {
    let fixture = grant_fixture(Ok(AdapterOAuthTokenOutcome {
        access_token: "fresh-access".to_string(),
        refresh_token: None,
        expires_at_epoch_seconds: Some(u64::MAX - 1),
        granted_scopes: vec!["scope.extra".to_string()],
    }));
    let catalog = CapabilityBindingSource::catalog(&fixture.service)
        .await
        .expect("catalog");
    let binding = catalog
        .snapshot
        .iter()
        .find(|binding| binding.destination().is_some())
        .expect("binding");
    let error = CapabilityInvoker::invoke(
        &fixture.service,
        CapabilityInvocation {
            operation: binding.spec().name.clone(),
            operation_token: binding.target().operation_token().clone(),
            arguments: json!({}),
            reviewed_authorization: None,
        },
    )
    .await
    .expect_err("authentication required");
    assert!(matches!(
        error,
        CapabilityError::AuthenticationRequired { .. }
    ));
    let (grant, token) = OauthAuthorityStore::new(fixture.paths)
        .load_grant_authority(&fixture.grant_id)
        .expect("inactive grant");
    assert_eq!(
        grant.status,
        AuthorizationGrantStatus::AuthenticationRequired
    );
    assert!(token.is_none());
}

#[tokio::test]
async fn rejected_refresh_invalidates_the_shared_grant() {
    let fixture = grant_fixture(Err(AdapterOAuthTokenError::Rejected));
    let catalog = CapabilityBindingSource::catalog(&fixture.service)
        .await
        .expect("catalog");
    let binding = catalog
        .snapshot
        .iter()
        .find(|binding| binding.destination().is_some())
        .expect("binding");
    let error = CapabilityInvoker::invoke(
        &fixture.service,
        CapabilityInvocation {
            operation: binding.spec().name.clone(),
            operation_token: binding.target().operation_token().clone(),
            arguments: json!({}),
            reviewed_authorization: None,
        },
    )
    .await
    .expect_err("authentication required");
    let CapabilityError::AuthenticationRequired { challenge } = error else {
        panic!("unexpected invocation error");
    };
    assert_eq!(
        challenge.authority_kind(),
        CapabilityAuthenticationAuthorityKind::AdapterGrant
    );
    assert_eq!(challenge.authority_id(), fixture.grant_id);
    let (grant, token) = OauthAuthorityStore::new(fixture.paths.clone())
        .load_grant_authority(&fixture.grant_id)
        .expect("grant");
    assert_eq!(
        grant.status,
        AuthorizationGrantStatus::AuthenticationRequired
    );
    assert!(token.is_none());
    assert!(fixture.paths.adapter_oauth_grant_quarantine_dir().exists());
    let unavailable = CapabilityBindingSource::catalog(&fixture.service)
        .await
        .expect("derived unavailable catalog");
    assert_eq!(
        unavailable
            .snapshot
            .iter()
            .filter(|binding| binding.destination().is_some())
            .count(),
        0
    );
    assert_eq!(unavailable.availability_notices.len(), 2);
}

#[tokio::test]
async fn new_authorization_without_identity_creates_another_account_grant() {
    let fixture = grant_fixture(Ok(AdapterOAuthTokenOutcome {
        access_token: "second-account-access".to_string(),
        refresh_token: Some("second-account-refresh".to_string()),
        expires_at_epoch_seconds: Some(u64::MAX - 1),
        granted_scopes: vec!["scope.read".to_string()],
    }));
    let redirect_uri = "http://localhost:43123/adapter/oauth/callback";
    let started = fixture
        .service
        .start_oauth_authorization(
            "human:local",
            crate::AdapterOAuthAuthorizationRequest {
                application_id: fixture.application_id.clone(),
                expected_application_revision: 1,
                grant_id: None,
                expected_grant_revision: None,
                semantic_digest: fixture.semantic_digest,
                operation_ids: vec!["get_item".to_string()],
                additional_services: Vec::new(),
                callback_mode: crate::Oauth2CallbackMode::Loopback,
                redirect_uri: redirect_uri.to_string(),
            },
        )
        .await
        .expect("start authorization");
    let authorization_url = url::Url::parse(&started.authorization_url).expect("authorization URL");
    let state = authorization_url
        .query_pairs()
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.into_owned())
        .expect("state");
    let completed = fixture
        .service
        .complete_oauth_callback(&format!("{redirect_uri}?state={state}&code=code-marker"))
        .await
        .expect("complete authorization");
    assert!(completed.newly_authorized);
    assert_ne!(completed.grant.grant_id, fixture.grant_id);
    assert!(completed.grant.account_id.is_none());
    let snapshot = OauthAuthorityStore::new(fixture.paths)
        .snapshot()
        .expect("OAuth snapshot");
    assert_eq!(snapshot.grants.len(), 2);
}

#[tokio::test]
async fn stale_oauth_management_revisions_are_rejected() {
    let fixture = grant_fixture(Err(AdapterOAuthTokenError::Unavailable));
    let attach = fixture
        .service
        .attach_oauth_connection(&fixture.semantic_digest, &fixture.grant_id, 2, None)
        .await;
    assert!(matches!(
        attach,
        Err(crate::AdapterConnectionSetupError::Conflict)
    ));

    let start = fixture
        .service
        .start_oauth_authorization(
            "human:local",
            crate::AdapterOAuthAuthorizationRequest {
                application_id: fixture.application_id,
                expected_application_revision: 2,
                grant_id: Some(fixture.grant_id),
                expected_grant_revision: Some(3),
                semantic_digest: fixture.semantic_digest,
                operation_ids: vec!["get_item".to_string()],
                additional_services: Vec::new(),
                callback_mode: crate::Oauth2CallbackMode::Loopback,
                redirect_uri: "http://localhost:43123/adapter/oauth/callback".to_string(),
            },
        )
        .await;
    assert!(matches!(
        start,
        Err(crate::AdapterOAuthSetupError::Superseded)
    ));
}
