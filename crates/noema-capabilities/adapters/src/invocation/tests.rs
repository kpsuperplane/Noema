use super::*;
use crate::{
    AdapterConnectionRevisions, AdapterConnectionStore, AdapterConnectionV2,
    AdapterCredentialMaterial, AdapterDefinitionStore, AdapterManifestV3,
    network::{
        AdapterBearerCredential, AdapterHttpError, AdapterHttpExecutor, AdapterHttpFuture,
        AdapterHttpResponse,
    },
    request::EncodedAdapterRequest,
};
use noema_capabilities::{
    CapabilityBindingSource, CapabilityError, CapabilityInvocation, CapabilityInvoker,
    OperationToken, ReviewedCapabilityAuthorization,
};
use noema_home::NoemaPaths;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct RecordedRequest {
    method: crate::HttpMethod,
    request: EncodedAdapterRequest,
    bearer: Option<String>,
}

struct RecordingHttp {
    outcome: Result<AdapterHttpResponse, AdapterHttpError>,
    requests: Mutex<Vec<RecordedRequest>>,
}

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
        let outcome = self.outcome.clone();
        Box::pin(async move { outcome })
    }
}

fn fixture(
    outcome: AdapterHttpResponse,
) -> (
    tempfile::TempDir,
    AdapterCapabilityService,
    Arc<RecordingHttp>,
    String,
) {
    fixture_with_http(Ok(outcome))
}

fn fixture_with_http(
    outcome: Result<AdapterHttpResponse, AdapterHttpError>,
) -> (
    tempfile::TempDir,
    AdapterCapabilityService,
    Arc<RecordingHttp>,
    String,
) {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let manifest: AdapterManifestV3 = serde_json::from_value(json!({
        "schema_version": 3,
        "definition_id": "definition:invocation_fixture",
        "adapter_id": "invocation_fixture",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {"mode": "static_bearer", "scopes": ["items.read"]},
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
            "pagination": {"kind": "none"}
        }, {
            "operation_id": "create_item",
            "method": "POST",
            "path": "/v1/items",
            "arguments": [
                {"name": "title", "source": "model_input", "location": "json_body", "type": "string", "required": true}
            ],
            "behavior": {"readOnly": {"value": false, "source": "model"}, "idempotent": {"value": false, "source": "model"}, "destructive": {"value": true, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
            "retry": "never",
            "pagination": {"kind": "none"}
        }]
    }))
    .expect("manifest");
    let definition = AdapterDefinitionStore::new(paths.clone())
        .install(&manifest, "fixture://company-a/items.json", None, None)
        .expect("definition");
    let connection_id = "a".repeat(32);
    let generation_id = "b".repeat(32);
    let descriptor = AdapterConnectionV2 {
        schema_version: 2,
        connection_id: connection_id.clone(),
        connection_slug: "personal".to_string(),
        semantic_digest: definition.compiled.semantic_digest.to_string(),
        account_id: Some("account:one".to_string()),
        account_kind: "personal".to_string(),
        status: AdapterConnectionStatus::Active,
        revisions: AdapterConnectionRevisions {
            connection: 2,
            credential: 3,
            grant: 5,
            policy: 7,
        },
        credential_generation: Some(generation_id.clone()),
        granted_scopes: vec!["items.read".to_string()],
        allowed_operations: vec!["create_item".to_string(), "get_item".to_string()],
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
        material: AdapterCredentialMaterial::StaticBearer {
            token: "synthetic-secret-marker".to_string(),
        },
    };
    AdapterConnectionStore::new(paths.clone())
        .install(&descriptor, Some(&credential), &definition.compiled)
        .expect("connection");
    let http = Arc::new(RecordingHttp {
        outcome,
        requests: Mutex::new(Vec::new()),
    });
    let service = AdapterCapabilityService::new_with_http_for_tests(paths, http.clone());
    (home, service, http, connection_id)
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
    assert_eq!(
        requests[0].bearer.as_deref(),
        Some("synthetic-secret-marker")
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
async fn ambiguous_write_is_uncertain_and_never_retried() {
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
