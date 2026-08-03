use super::*;
use crate::{
    AdapterConnectionRevisions, AdapterConnectionStore, AdapterConnectionV3,
    AdapterCredentialMaterial, AdapterDefinitionStore, AdapterManifestV5, ResponseContract,
    network::{
        AdapterBearerCredential, AdapterHttpError, AdapterHttpExecutor, AdapterHttpFuture,
        AdapterHttpResponse, AdapterOAuthTokenError, AdapterOAuthTokenFuture,
        AdapterOAuthTokenGrant, AdapterOAuthTokenOutcome, AdapterOAuthTokenRequest,
    },
    request::EncodedAdapterRequest,
};
use noema_capabilities::{
    CapabilityBindingSource, CapabilityError, CapabilityInvocation, CapabilityInvoker,
    OperationToken, ReviewedCapabilityAuthorization,
};
use noema_home::NoemaPaths;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

#[derive(Clone)]
struct RecordedRequest {
    method: crate::HttpMethod,
    request: EncodedAdapterRequest,
    bearer: Option<String>,
}

struct RecordingHttp {
    outcome: Mutex<Result<AdapterHttpResponse, AdapterHttpError>>,
    queued_outcomes: Mutex<VecDeque<Result<AdapterHttpResponse, AdapterHttpError>>>,
    requests: Mutex<Vec<RecordedRequest>>,
    token_outcomes: Mutex<VecDeque<Result<AdapterOAuthTokenOutcome, AdapterOAuthTokenError>>>,
    token_exchanges: Mutex<usize>,
    refresh_gate: Mutex<Option<Arc<RefreshGate>>>,
}

#[derive(Default)]
struct RefreshGate {
    entered: Notify,
    release: Notify,
}

type Fixture = (
    tempfile::TempDir,
    AdapterCapabilityService,
    Arc<RecordingHttp>,
    String,
);

impl AdapterHttpExecutor for RecordingHttp {
    fn execute(
        &self,
        method: crate::HttpMethod,
        _retry: crate::RetryPolicy,
        request: EncodedAdapterRequest,
        credential: Option<AdapterBearerCredential>,
    ) -> AdapterHttpFuture<'_> {
        let bearer = credential.map(AdapterBearerCredential::into_secret_for_tests);
        self.requests
            .lock()
            .expect("requests")
            .push(RecordedRequest {
                method,
                request,
                bearer,
            });
        let outcome = self
            .queued_outcomes
            .lock()
            .expect("queued outcomes")
            .pop_front()
            .unwrap_or_else(|| self.outcome.lock().expect("outcome").clone());
        Box::pin(async move { outcome })
    }

    fn exchange_oauth_token(
        &self,
        request: AdapterOAuthTokenRequest,
    ) -> AdapterOAuthTokenFuture<'_> {
        Box::pin(async move {
            assert!(matches!(
                &request.grant,
                AdapterOAuthTokenGrant::RefreshToken { .. }
            ));
            *self.token_exchanges.lock().expect("token exchanges") += 1;
            let gate = self.refresh_gate.lock().expect("refresh gate").clone();
            if let Some(gate) = gate {
                gate.entered.notify_one();
                gate.release.notified().await;
            }
            self.token_outcomes
                .lock()
                .expect("token outcomes")
                .pop_front()
                .unwrap_or(Err(AdapterOAuthTokenError::Unavailable))
        })
    }
}

fn fixture(outcome: AdapterHttpResponse) -> Fixture {
    fixture_with_http(Ok(outcome))
}

fn fixture_with_http(outcome: Result<AdapterHttpResponse, AdapterHttpError>) -> Fixture {
    fixture_with_manifest(outcome, |_| {})
}

fn fixture_with_manifest(
    outcome: Result<AdapterHttpResponse, AdapterHttpError>,
    configure: impl FnOnce(&mut AdapterManifestV5),
) -> Fixture {
    fixture_with_options(outcome, configure, None)
}

fn fixture_with_options(
    outcome: Result<AdapterHttpResponse, AdapterHttpError>,
    configure: impl FnOnce(&mut AdapterManifestV5),
    oauth_expiry: Option<u64>,
) -> Fixture {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let mut manifest: AdapterManifestV5 = serde_json::from_value(json!({
        "schema_version": 5,
        "definition_id": "definition:invocation_fixture",
        "adapter_id": "invocation_fixture",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {
            "kind": "credential",
            "setup": {
                "credential_type": "API token",
                "setup_url": "https://developers.example.test/tokens",
                "instructions": ["Create an API token."],
                "input": {"kind": "fields", "fields": [{"id": "token", "label": "API token"}]}
            },
            "request_auth": {
                "language": "luau",
                "source": "return function(input) return { headers = { Authorization = 'Bearer ' .. input.credentials.token } } end"
            }
        },
        "quota": {"cost_class": "free"},
        "operations": [{
            "operation_id": "get_item",
            "method": "GET",
            "path": "/v1/items/{item_id}",
            "fixed_headers": {"accept": "application/json"},
            "arguments": [
                {"name": "item_id", "source": "model_input", "location": "path", "type": "string", "required": true},
                {"name": "view", "source": "model_input", "location": "query", "type": "string"}
            ],
            "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
            "retry": "transport_safe_read",
            "pagination": {"kind": "none"},
            "response": {
                "accepted_content_types": ["application/json"],
                "output_schema": {"type": "object", "properties": {
                    "id": {"type": "string", "maxBytes": 128},
                    "access_token": {"type": "string", "maxBytes": 256},
                    "nested": {"type": "object", "properties": {"password": {"type": "string", "maxBytes": 256}, "label": {"type": "string", "maxBytes": 256}}, "required": ["password", "label"], "additionalProperties": false}
                }, "required": ["id", "access_token", "nested"], "additionalProperties": false}
            }
        }, {
            "operation_id": "create_item",
            "method": "POST",
            "path": "/v1/items",
            "arguments": [
                {"name": "title", "source": "model_input", "location": "json_body", "type": "string", "required": true}
            ],
            "behavior": {"readOnly": {"value": false, "source": "model"}, "idempotent": {"value": false, "source": "model"}, "destructive": {"value": true, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
            "retry": "never",
            "pagination": {"kind": "none"},
            "response": {
                "accepted_content_types": ["application/json"],
                "output_schema": {"type": "object", "properties": {"id": {"type": "string", "maxBytes": 128}}, "required": ["id"], "additionalProperties": false}
            }
        }]
    }))
    .expect("manifest");
    configure(&mut manifest);
    let definition = AdapterDefinitionStore::new(paths.clone())
        .install(&manifest, "fixture://company-a/items.json", None, None)
        .expect("definition");
    let connection_id = "a".repeat(32);
    let generation_id = "b".repeat(32);
    let descriptor = AdapterConnectionV3 {
        schema_version: 3,
        connection_id: connection_id.clone(),
        connection_slug: "personal".to_string(),
        semantic_digest: definition.compiled.semantic_digest.to_string(),
        account_id: Some("account:one".to_string()),
        connection_label: None,
        account_kind: "personal".to_string(),
        status: AdapterConnectionStatus::Active,
        revisions: AdapterConnectionRevisions {
            connection: 2,
            credential: 3,
            grant: 5,
            policy: 7,
        },
        credential_generation: Some(generation_id.clone()),
        granted_scopes: oauth_expiry
            .map(|_| definition.compiled.authentication.scopes().to_vec())
            .unwrap_or_default(),
        allowed_operations: vec!["create_item".to_string(), "get_item".to_string()],
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
        material: oauth_expiry.map_or_else(
            || AdapterCredentialMaterial::Credential {
                fields: std::collections::BTreeMap::from([(
                    "token".to_string(),
                    "synthetic-secret-marker".to_string(),
                )]),
            },
            |expires_at_epoch_seconds| AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
                callback_mode: crate::Oauth2CallbackMode::Loopback,
                client_id: "client-marker".to_string(),
                client_secret: None,
                access_token: "stale-access-marker".to_string(),
                refresh_token: Some("refresh-marker".to_string()),
                expires_at_epoch_seconds: Some(expires_at_epoch_seconds),
            },
        ),
    };
    AdapterConnectionStore::new(paths.clone())
        .install(&descriptor, Some(&credential), &definition.compiled)
        .expect("connection");
    let http = Arc::new(RecordingHttp {
        outcome: Mutex::new(outcome),
        queued_outcomes: Mutex::new(VecDeque::new()),
        requests: Mutex::new(Vec::new()),
        token_outcomes: Mutex::new(VecDeque::new()),
        token_exchanges: Mutex::new(0),
        refresh_gate: Mutex::new(None),
    });
    let service = AdapterCapabilityService::new_with_http_for_tests(paths, http.clone());
    if matches!(
        &credential.material,
        AdapterCredentialMaterial::Oauth2AuthorizationCodePkce { .. }
    ) {
        service.set_oauth_callback_mode(crate::Oauth2CallbackMode::Loopback);
    }
    (home, service, http, connection_id)
}

fn oauth_fixture(
    outcome: Result<AdapterHttpResponse, AdapterHttpError>,
    expires_at_epoch_seconds: u64,
    token_outcomes: VecDeque<Result<AdapterOAuthTokenOutcome, AdapterOAuthTokenError>>,
    refresh_gate: Option<Arc<RefreshGate>>,
) -> Fixture {
    let fixture = fixture_with_options(
        outcome,
        |manifest| {
            manifest.authentication = serde_json::from_value(json!({
                "kind": "oauth2_authorization_code_pkce",
                "scopes": ["items.read"],
                "authorization_endpoint": "https://auth.example.test/authorize",
                "token_endpoint": "https://auth.example.test/token",
                "client_authentication": "none",
                "setups": [{
                    "callback_mode": "loopback",
                    "setup": {
                        "credential_type": "Desktop app",
                        "setup_url": "https://developers.example.test/oauth/clients/new",
                        "instructions": ["Create an OAuth client."],
                        "input": {
                            "kind": "document",
                            "media_type": "application/json",
                            "fields": [{"id": "client_id", "label": "Client ID"}],
                            "normalize": {
                                "language": "luau",
                                "source": "return function(input) return { client_id = 'client' } end"
                            }
                        }
                    }
                }],
                "extra_authorization_parameters": {}
            }))
            .expect("OAuth authentication");
        },
        Some(expires_at_epoch_seconds),
    );
    fixture
        .2
        .token_outcomes
        .lock()
        .expect("token outcomes")
        .extend(token_outcomes);
    *fixture.2.refresh_gate.lock().expect("refresh gate") = refresh_gate;
    fixture
}

fn refreshed_token(access_token: &str) -> AdapterOAuthTokenOutcome {
    AdapterOAuthTokenOutcome {
        access_token: access_token.to_string(),
        refresh_token: None,
        expires_at_epoch_seconds: Some(u64::MAX - 1),
        granted_scopes: vec!["items.read".to_string()],
    }
}

fn json_response(status: u16, payload: &Value) -> AdapterHttpResponse {
    AdapterHttpResponse {
        status,
        content_type: Some("application/json".to_string()),
        body: serde_json::to_vec(payload).expect("JSON response"),
    }
}

fn empty_response(status: u16) -> AdapterHttpResponse {
    AdapterHttpResponse {
        status,
        content_type: None,
        body: Vec::new(),
    }
}

fn item_response() -> AdapterHttpResponse {
    json_response(
        200,
        &json!({"id": "one", "access_token": "redacted", "nested": {
            "password": "redacted", "label": "kept"
        }}),
    )
}

fn response_contract(content_type: &str, source: &str, output_schema: Value) -> ResponseContract {
    serde_json::from_value(json!({
        "accepted_content_types": [content_type],
        "transform": {"language": "luau", "source": source},
        "output_schema": output_schema
    }))
    .expect("response contract")
}

fn configure_paginated_events(manifest: &mut AdapterManifestV5) {
    manifest.operations[0].pagination = serde_json::from_value(json!({
        "kind": "response_token",
        "response_pointer": "/nextPageToken",
        "request_argument": "pageToken",
        "page_size": {"request_argument": "maxResults", "value": 25}
    }))
    .expect("pagination");
    manifest.operations[0].response = response_contract(
        "application/json",
        "return function(response) local body = json.decode(response.body) local events = {} for index, event in ipairs(body.events or {}) do if index > 25 then break end events[index] = { id = event.id, summary = event.summary, start = event.start, finish = event.finish } end return { events = events } end",
        json!({
            "type": "object",
            "properties": {"events": {
                "type": "array", "maxItems": 25,
                "items": {"type": "object", "properties": {
                    "id": {"type": "string", "maxBytes": 40},
                    "summary": {"type": "string", "maxBytes": 64},
                    "start": {"type": "string", "maxBytes": 32},
                    "finish": {"type": "string", "maxBytes": 32}
                }, "required": ["id", "summary", "start", "finish"], "additionalProperties": false}
            }},
            "required": ["events"],
            "additionalProperties": false
        }),
    );
}

fn event_page(count: usize, token: Option<&str>) -> AdapterHttpResponse {
    let mut page = json!({
        "events": (0..count).map(|index| json!({
            "id": format!("event-{index}"),
            "summary": format!("Event {index}"),
            "start": "2026-08-02T09:00:00Z",
            "finish": "2026-08-02T09:30:00Z",
            "verbose": "provider-only metadata".repeat(20)
        })).collect::<Vec<_>>()
    });
    if let Some(token) = token {
        page["nextPageToken"] = json!(token);
    }
    json_response(200, &page)
}

async fn advertised_invocation(service: &AdapterCapabilityService) -> CapabilityInvocation {
    let catalog = CapabilityBindingSource::catalog(service)
        .await
        .expect("catalog");
    let binding = catalog
        .snapshot
        .iter()
        .find(|binding| binding.spec().name.as_str().ends_with(".get_item"))
        .expect("read binding");
    CapabilityInvocation {
        operation: binding.spec().name.clone(),
        operation_token: binding.target().operation_token().clone(),
        arguments: json!({"item_id": "folder/item", "view": "full"}),
        reviewed_authorization: None,
    }
}

async fn advertised_write_invocation(service: &AdapterCapabilityService) -> CapabilityInvocation {
    let catalog = CapabilityBindingSource::catalog(service)
        .await
        .expect("catalog");
    let binding = catalog
        .snapshot
        .iter()
        .find(|binding| binding.spec().name.as_str().ends_with(".create_item"))
        .expect("write binding");
    CapabilityInvocation {
        operation: binding.spec().name.clone(),
        operation_token: binding.target().operation_token().clone(),
        arguments: json!({"title": "draft"}),
        reviewed_authorization: None,
    }
}

#[tokio::test]
async fn active_read_revalidates_and_invokes_the_exact_connection_credential() {
    let (_home, service, http, _connection_id) = fixture(json_response(
        200,
        &json!({
            "id": "one",
            "access_token": "must-not-reach-model",
            "nested": {"password": "also-secret", "label": "kept"}
        }),
    ));
    let invocation = advertised_invocation(&service).await;
    assert!(
        !invocation
            .operation_token
            .as_str()
            .contains("synthetic-secret-marker")
    );

    let output = CapabilityInvoker::invoke(&service, invocation)
        .await
        .expect("output");
    assert_eq!(
        output.payload,
        json!({
            "id": "one",
            "access_token": "[REDACTED]",
            "nested": {"password": "[REDACTED]", "label": "kept"}
        })
    );
    let requests = http.requests.lock().expect("requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].request.url.as_str(),
        "https://api.example.test/v1/items/folder%2Fitem?view=full"
    );
    assert!(requests[0].bearer.is_none());
    assert_eq!(
        requests[0]
            .request
            .sensitive_headers
            .get("Authorization")
            .map(String::as_str),
        Some("Bearer synthetic-secret-marker")
    );
}

#[tokio::test]
async fn transformed_json_is_schema_checked_and_redacted_once() {
    let response = json_response(
        200,
        &json!({"profile": {"name": "Alex", "token": "provider-secret"}}),
    );
    let (_home, service, _http, _connection_id) = fixture_with_manifest(Ok(response), |manifest| {
        manifest.operations[0].response = response_contract(
            "application/json",
            "return function(response)\n  local body = json.decode(response.body)\n  return { name = body.profile.name, access_token = body.profile.token }\nend",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "maxBytes": 128},
                    "access_token": {"type": "string", "maxBytes": 256}
                },
                "required": ["name", "access_token"],
                "additionalProperties": false
            }),
        );
    });
    let output = CapabilityInvoker::invoke(&service, advertised_invocation(&service).await)
        .await
        .expect("transformed output");
    assert_eq!(
        output.payload,
        json!({"name": "Alex", "access_token": "[REDACTED]"})
    );
}

#[tokio::test]
async fn paginated_calendar_results_accept_null_continuation_and_keep_provider_arguments_private() {
    let (_home, service, http, _connection_id) = fixture_with_manifest(
        Ok(event_page(250, Some("provider-token-1"))),
        configure_paginated_events,
    );
    let mut invocation = advertised_invocation(&service).await;
    invocation.arguments["continuation"] = Value::Null;
    let output = CapabilityInvoker::invoke(&service, invocation)
        .await
        .expect("first page");
    assert_eq!(
        output.payload["events"].as_array().expect("events").len(),
        25
    );
    assert!(serde_json::to_vec(&output.payload).expect("payload").len() < 8 * 1024);
    assert!(output.payload["continuation"].as_str().is_some());
    assert!(!output.payload.to_string().contains("provider-token-1"));

    let requests = http.requests.lock().expect("requests");
    let query = requests[0]
        .request
        .url
        .query_pairs()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        query.get("maxResults").map(|value| value.as_ref()),
        Some("25")
    );
    assert!(!query.contains_key("pageToken"));
}

#[tokio::test]
async fn continuation_rotates_after_success_survives_failure_and_retires_terminally() {
    let (_home, service, http, _connection_id) = fixture_with_manifest(
        Ok(event_page(1, Some("provider-token-1"))),
        configure_paginated_events,
    );
    let original = advertised_invocation(&service).await;
    let first = CapabilityInvoker::invoke(&service, original.clone())
        .await
        .expect("first page");
    let first_reference = first.payload["continuation"].as_str().expect("first");

    *http.outcome.lock().expect("outcome") = Ok(event_page(1, Some("provider-token-2")));
    let mut next = original.clone();
    next.arguments["continuation"] = json!(first_reference);
    let second = CapabilityInvoker::invoke(&service, next.clone())
        .await
        .expect("second page");
    let second_reference = second.payload["continuation"].as_str().expect("second");
    assert_ne!(first_reference, second_reference);
    let injected_token = {
        let requests = http.requests.lock().expect("requests");
        requests[1]
            .request
            .url
            .query_pairs()
            .find(|(name, _)| name == "pageToken")
            .map(|(_, value)| value.into_owned())
    };
    assert_eq!(injected_token, Some("provider-token-1".to_string()));
    assert_eq!(
        CapabilityInvoker::invoke(&service, next).await,
        Err(CapabilityError::InvalidArguments)
    );

    *http.outcome.lock().expect("outcome") = Err(AdapterHttpError::Unavailable);
    let mut terminal = original.clone();
    terminal.arguments["continuation"] = json!(second_reference);
    assert_eq!(
        CapabilityInvoker::invoke(&service, terminal.clone()).await,
        Err(CapabilityError::Unavailable)
    );
    *http.outcome.lock().expect("outcome") = Ok(event_page(1, None));
    let final_page = CapabilityInvoker::invoke(&service, terminal.clone())
        .await
        .expect("terminal page");
    assert!(final_page.payload.get("continuation").is_none());
    assert_eq!(
        CapabilityInvoker::invoke(&service, terminal).await,
        Err(CapabilityError::InvalidArguments)
    );
}

#[tokio::test]
async fn reviewed_non_json_response_is_transformed_without_a_json_fallback() {
    let response = AdapterHttpResponse {
        status: 200,
        content_type: Some("text/csv".to_string()),
        body: b"name,score\nAlex,10".to_vec(),
    };
    let (_home, service, _http, _connection_id) = fixture_with_manifest(Ok(response), |manifest| {
        manifest.operations[0].response = response_contract(
            "text/csv",
            "return function(response)\n  return { body = response.body, media_type = response.content_type }\nend",
            json!({
                "type": "object",
                "properties": {
                    "body": {"type": "string", "maxBytes": 1024},
                    "media_type": {"type": "string", "maxBytes": 128}
                },
                "required": ["body", "media_type"],
                "additionalProperties": false
            }),
        );
    });
    let output = CapabilityInvoker::invoke(&service, advertised_invocation(&service).await)
        .await
        .expect("CSV output");
    assert_eq!(
        output.payload,
        json!({"body": "name,score\nAlex,10", "media_type": "text/csv"})
    );
}

#[tokio::test]
async fn transformed_no_content_response_has_an_empty_media_free_abi() {
    let response = AdapterHttpResponse {
        status: 204,
        content_type: Some("text/plain".to_string()),
        body: b"ignored".to_vec(),
    };
    let (_home, service, _http, _connection_id) = fixture_with_manifest(Ok(response), |manifest| {
        manifest.operations[0].response = response_contract(
            "application/json",
            "return function(response)\n  return { empty = response.body == '', has_media_type = response.content_type ~= nil }\nend",
            json!({
                "type": "object",
                "properties": {
                    "empty": {"type": "boolean"},
                    "has_media_type": {"type": "boolean"}
                },
                "required": ["empty", "has_media_type"],
                "additionalProperties": false
            }),
        );
    });
    let output = CapabilityInvoker::invoke(&service, advertised_invocation(&service).await)
        .await
        .expect("204 output");
    assert_eq!(
        output.payload,
        json!({"empty": true, "has_media_type": false})
    );
}

#[tokio::test]
async fn reviewed_write_requires_exact_authorization_and_sends_exact_json_request() {
    let (_home, service, http, _connection_id) =
        fixture(json_response(200, &json!({"id": "created"})));
    let mut invocation = advertised_write_invocation(&service).await;
    assert_eq!(
        CapabilityInvoker::invoke(&service, invocation.clone()).await,
        Err(CapabilityError::Denied)
    );
    assert!(http.requests.lock().expect("requests").is_empty());

    invocation.reviewed_authorization = Some(ReviewedCapabilityAuthorization {
        action_id: "action:synthetic".to_string(),
        revision: 1,
        arguments_sha256: "0".repeat(64),
    });
    assert_eq!(
        CapabilityInvoker::invoke(&service, invocation.clone()).await,
        Err(CapabilityError::Denied)
    );
    assert!(http.requests.lock().expect("requests").is_empty());

    invocation.reviewed_authorization = Some(ReviewedCapabilityAuthorization::for_action(
        "action:synthetic",
        1,
        &invocation.arguments,
    ));
    let output = CapabilityInvoker::invoke(&service, invocation)
        .await
        .expect("reviewed write");
    assert_eq!(output.payload, json!({"id": "created"}));
    let requests = http.requests.lock().expect("requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, crate::HttpMethod::Post);
    assert_eq!(
        requests[0].request.url.as_str(),
        "https://api.example.test/v1/items"
    );
    assert_eq!(requests[0].request.body, Some(json!({"title": "draft"})));
}

#[tokio::test]
async fn write_failures_respect_dispatch_certainty_and_never_retry_ambiguity() {
    let (_home, service, http, _connection_id) =
        fixture_with_http(Err(AdapterHttpError::Unavailable));
    let mut invocation = advertised_write_invocation(&service).await;
    invocation.reviewed_authorization = Some(ReviewedCapabilityAuthorization::for_action(
        "action:synthetic",
        1,
        &invocation.arguments,
    ));
    assert_eq!(
        CapabilityInvoker::invoke(&service, invocation).await,
        Err(CapabilityError::Unavailable)
    );
    assert_eq!(http.requests.lock().expect("requests").len(), 1);

    let (_home, service, http, _connection_id) =
        fixture_with_http(Err(AdapterHttpError::OutcomeUncertain));
    let mut invocation = advertised_write_invocation(&service).await;
    invocation.reviewed_authorization = Some(ReviewedCapabilityAuthorization::for_action(
        "action:synthetic",
        1,
        &invocation.arguments,
    ));
    assert_eq!(
        CapabilityInvoker::invoke(&service, invocation).await,
        Err(CapabilityError::OutcomeUncertain)
    );
    assert_eq!(http.requests.lock().expect("requests").len(), 1);
}

#[tokio::test]
async fn malformed_success_fails_reads_and_leaves_writes_uncertain() {
    let invalid = AdapterHttpResponse {
        status: 200,
        content_type: Some("application/json".to_string()),
        body: b"{".to_vec(),
    };
    let (_home, service, _http, _connection_id) = fixture(invalid);
    assert_eq!(
        CapabilityInvoker::invoke(&service, advertised_invocation(&service).await).await,
        Err(CapabilityError::Failed)
    );

    let (_home, service, _http, _connection_id) = fixture(json_response(
        200,
        &json!({
            "id": "x".repeat(129),
            "access_token": "token",
            "nested": {"password": "secret", "label": "label"}
        }),
    ));
    assert_eq!(
        CapabilityInvoker::invoke(&service, advertised_invocation(&service).await).await,
        Err(CapabilityError::Failed)
    );

    let mut write = advertised_write_invocation(&service).await;
    write.reviewed_authorization = Some(ReviewedCapabilityAuthorization::for_action(
        "action:synthetic",
        1,
        &write.arguments,
    ));
    assert_eq!(
        CapabilityInvoker::invoke(&service, write).await,
        Err(CapabilityError::OutcomeUncertain)
    );
}

#[tokio::test]
async fn transforms_skip_rejections_and_fail_writes_without_raw_fallback() {
    let rejection = json_response(403, &json!({"error": "denied"}));
    let (_home, service, _http, _connection_id) =
        fixture_with_manifest(Ok(rejection), |manifest| {
            manifest.operations[0].response = response_contract(
                "application/json",
                "return function(response)\n  while true do end\nend",
                json!({"type": "null"}),
            );
        });
    assert_eq!(
        CapabilityInvoker::invoke(&service, advertised_invocation(&service).await)
            .await
            .expect("remote rejection"),
        CapabilityOutput::failed(json!({
            "error": "remote_request_failed",
            "status": 403,
            "response": {"error": "denied"}
        }))
    );

    let (_home, service, _http, _connection_id) =
        fixture_with_manifest(Ok(json_response(200, &json!({"id": "raw"}))), |manifest| {
            manifest.operations[1].response = response_contract(
                "application/json",
                "return function(response)\n  return {}\nend",
                json!({
                    "type": "object",
                    "properties": {"id": {"type": "string", "maxBytes": 128}},
                    "required": ["id"],
                    "additionalProperties": false
                }),
            );
        });
    let mut write = advertised_write_invocation(&service).await;
    write.reviewed_authorization = Some(ReviewedCapabilityAuthorization::for_action(
        "action:synthetic",
        1,
        &write.arguments,
    ));
    assert_eq!(
        CapabilityInvoker::invoke(&service, write).await,
        Err(CapabilityError::OutcomeUncertain)
    );
}

#[tokio::test]
async fn server_error_write_is_uncertain_without_retry() {
    let (_home, service, http, _connection_id) = fixture(empty_response(503));
    let mut invocation = advertised_write_invocation(&service).await;
    invocation.reviewed_authorization = Some(ReviewedCapabilityAuthorization::for_action(
        "action:synthetic",
        1,
        &invocation.arguments,
    ));
    assert_eq!(
        CapabilityInvoker::invoke(&service, invocation).await,
        Err(CapabilityError::OutcomeUncertain)
    );
    assert_eq!(http.requests.lock().expect("requests").len(), 1);
}

#[tokio::test]
async fn remote_rejection_preserves_bounded_provider_details() {
    let provider_error = json!({
        "error": {
            "code": 403,
            "message": "Gmail API is disabled. Enable it, then retry.",
            "status": "PERMISSION_DENIED",
            "details": [{
                "reason": "SERVICE_DISABLED",
                "domain": "googleapis.com",
                "metadata": {
                    "service": "gmail.googleapis.com",
                    "activationUrl": "https://console.developers.google.com/apis/api/gmail.googleapis.com/overview?project=398628717117"
                }
            }]
        }
    });
    let (_home, service, _http, _connection_id) = fixture(json_response(403, &provider_error));
    let output = CapabilityInvoker::invoke(&service, advertised_invocation(&service).await)
        .await
        .expect("tool-declared rejection");
    assert_eq!(
        output,
        CapabilityOutput::failed(json!({
            "error": "remote_request_failed",
            "status": 403,
            "response": provider_error
        }))
    );

    let oversized = json!({"message": "x".repeat(5_000)});
    let (_home, service, _http, _connection_id) = fixture(json_response(400, &oversized));
    let output = CapabilityInvoker::invoke(&service, advertised_invocation(&service).await)
        .await
        .expect("bounded rejection");
    assert_eq!(
        output,
        CapabilityOutput::failed(json!({
            "error": "remote_request_failed",
            "status": 400,
            "response_omitted": "too_large"
        }))
    );
}

#[tokio::test]
async fn quarantine_after_advertisement_fences_the_send() {
    let (_home, service, http, connection_id) = fixture(empty_response(204));
    let invocation = advertised_invocation(&service).await;
    let mut stale = invocation.clone();
    let mut authority = AdapterOperationAuthorityV1::from_operation_token(&stale.operation_token)
        .expect("authority");
    let connection_revision = authority.connection_revision;
    authority.policy_revision += 1;
    let token = crate::digest::canonical_json_bytes(
        &serde_json::to_value(authority).expect("authority value"),
    )
    .expect("canonical authority");
    stale.operation_token = OperationToken::new(String::from_utf8(token).expect("utf8"));
    assert_eq!(
        CapabilityInvoker::invoke(&service, stale).await,
        Err(CapabilityError::UnknownOperation)
    );
    assert!(http.requests.lock().expect("requests").is_empty());

    service
        .quarantine_connection(&connection_id, connection_revision)
        .await
        .expect("quarantine");

    assert_eq!(
        CapabilityInvoker::invoke(&service, invocation).await,
        Err(CapabilityError::UnknownOperation)
    );
    assert!(http.requests.lock().expect("requests").is_empty());
}

#[tokio::test]
async fn remote_auth_failure_returns_an_exact_non_secret_connection_challenge() {
    let (_home, service, _http, _connection_id) = fixture(empty_response(401));
    let invocation = advertised_invocation(&service).await;
    let authority = AdapterOperationAuthorityV1::from_operation_token(&invocation.operation_token)
        .expect("authority");
    let error = CapabilityInvoker::invoke(&service, invocation)
        .await
        .expect_err("authentication challenge");
    let CapabilityError::AuthenticationRequired { challenge } = error else {
        panic!("unexpected error")
    };
    assert_eq!(
        challenge.challenge_kind(),
        CapabilityAuthenticationChallengeKind::ReplaceCredential
    );
    assert_eq!(
        challenge.authority_kind(),
        CapabilityAuthenticationAuthorityKind::AdapterConnection
    );
    assert_eq!(challenge.authority_id(), "a".repeat(32));
    assert_eq!(
        challenge.authority_revision(),
        format!(
            "definition:{}/connection:2/credential:3/grant:5/policy:7",
            authority.semantic_digest
        )
    );
}

#[tokio::test]
async fn concurrent_expiry_performs_one_refresh() {
    let gate = Arc::new(RefreshGate::default());
    let (_home, service, http, _connection_id) = oauth_fixture(
        Ok(item_response()),
        1,
        VecDeque::from([Ok(refreshed_token("fresh-access-marker"))]),
        Some(gate.clone()),
    );
    let invocation = advertised_invocation(&service).await;
    let original = AdapterOperationAuthorityV1::from_operation_token(&invocation.operation_token)
        .expect("original authority");
    let first_service = service.clone();
    let first_invocation = invocation.clone();
    let first =
        tokio::spawn(
            async move { CapabilityInvoker::invoke(&first_service, first_invocation).await },
        );
    gate.entered.notified().await;
    let second_service = service.clone();
    let second =
        tokio::spawn(async move { CapabilityInvoker::invoke(&second_service, invocation).await });
    gate.release.notify_one();

    first.await.expect("first task").expect("first invocation");
    second
        .await
        .expect("second task")
        .expect("second invocation");
    assert_eq!(*http.token_exchanges.lock().expect("exchanges"), 1);
    let refreshed = advertised_invocation(&service).await;
    let requests = http.requests.lock().expect("requests");
    assert_eq!(requests.len(), 2);
    let refreshed = AdapterOperationAuthorityV1::from_operation_token(&refreshed.operation_token)
        .expect("refreshed authority");
    assert_eq!(
        refreshed.connection_revision,
        original.connection_revision + 1
    );
    assert_eq!(
        refreshed.credential_revision,
        original.credential_revision + 1
    );
    assert_eq!(refreshed.grant_revision, original.grant_revision);
    assert_eq!(refreshed.policy_revision, original.policy_revision);
}

#[tokio::test]
async fn unauthorized_safe_operation_refreshes_and_retries_once() {
    let success = item_response();
    let (_home, service, http, _connection_id) = oauth_fixture(
        Ok(success.clone()),
        u64::MAX,
        VecDeque::from([Ok(refreshed_token("fresh-access-marker"))]),
        None,
    );
    http.queued_outcomes
        .lock()
        .expect("queued outcomes")
        .extend([Ok(empty_response(401)), Ok(success)]);
    CapabilityInvoker::invoke(&service, advertised_invocation(&service).await)
        .await
        .expect("safe retry");
    assert_eq!(
        http.requests.lock().expect("requests")[1].bearer.as_deref(),
        Some("fresh-access-marker")
    );

    let (_home, service, http, _connection_id) = oauth_fixture(
        Ok(empty_response(401)),
        u64::MAX,
        VecDeque::from([Ok(refreshed_token("fresh-access-marker"))]),
        None,
    );
    let mut write = advertised_write_invocation(&service).await;
    write.reviewed_authorization = Some(ReviewedCapabilityAuthorization::for_action(
        "action:synthetic",
        1,
        &write.arguments,
    ));
    assert_eq!(
        CapabilityInvoker::invoke(&service, write).await,
        Err(CapabilityError::Failed)
    );
    assert_eq!(http.requests.lock().expect("requests").len(), 1);
}
