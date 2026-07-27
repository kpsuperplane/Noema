use super::*;
use crate::{
    AdapterConnectionRevisions, AdapterConnectionStore, AdapterConnectionV1,
    AdapterCredentialMaterial, AdapterDefinitionStore, AdapterManifestV1,
    network::{AdapterBearerCredential, AdapterHttpExecutor, AdapterHttpFuture},
    request::EncodedAdapterRequest,
};
use noema_capabilities::{
    CapabilityBindingSource, CapabilityInvocation, CapabilityInvoker, OperationToken,
};
use noema_home::NoemaPaths;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct RecordedRequest {
    request: EncodedAdapterRequest,
    bearer: Option<String>,
}

struct RecordingHttp {
    outcome: AdapterHttpOutcome,
    requests: Mutex<Vec<RecordedRequest>>,
}

impl RecordingHttp {
    fn new(outcome: AdapterHttpOutcome) -> Self {
        Self {
            outcome,
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl AdapterHttpExecutor for RecordingHttp {
    fn execute(
        &self,
        _method: crate::HttpMethod,
        _retry: crate::RetryPolicy,
        request: EncodedAdapterRequest,
        credential: Option<AdapterBearerCredential>,
    ) -> AdapterHttpFuture<'_> {
        let bearer = credential.map(AdapterBearerCredential::into_secret_for_tests);
        self.requests
            .lock()
            .expect("requests")
            .push(RecordedRequest { request, bearer });
        let outcome = self.outcome.clone();
        Box::pin(async move { Ok(outcome) })
    }
}

fn fixture(
    outcome: AdapterHttpOutcome,
) -> (
    tempfile::TempDir,
    AdapterCapabilityService,
    Arc<RecordingHttp>,
    String,
) {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let manifest: AdapterManifestV1 = serde_json::from_value(json!({
        "schema_version": 1,
        "definition_id": "definition:invocation_fixture",
        "adapter_id": "invocation_fixture",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {"mode": "static_bearer", "scopes": ["items.read"]},
        "provider_data_policy": {"retention_allowed": false, "deletion_supported": true},
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
            "effect": "read_only",
            "admission": "direct",
            "result": {"classification": "private", "model_route": "local_only", "model_payload": "full", "provider_retention": "deny", "persistence": "omit"},
            "retry": "transport_safe_read",
            "pagination": {"kind": "none"}
        }]
    }))
    .expect("manifest");
    let definition = AdapterDefinitionStore::new(paths.clone())
        .install(&manifest, "fixture://company-a/items.json", None, None)
        .expect("definition");
    let connection_id = "a".repeat(32);
    let generation_id = "b".repeat(32);
    let descriptor = AdapterConnectionV1 {
        schema_version: 1,
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
        allowed_operations: vec!["get_item".to_string()],
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
    let http = Arc::new(RecordingHttp::new(outcome));
    let service = AdapterCapabilityService::new_with_http_for_tests(paths, http.clone());
    (home, service, http, connection_id)
}

async fn advertised_invocation(service: &AdapterCapabilityService) -> CapabilityInvocation {
    let catalog = CapabilityBindingSource::catalog(service)
        .await
        .expect("catalog");
    let binding = catalog.snapshot.iter().next().expect("binding");
    CapabilityInvocation {
        operation: binding.spec().name.clone(),
        operation_token: binding.target().operation_token().clone(),
        arguments: json!({"item_id": "folder/item", "view": "full"}),
        governed_admission: None,
    }
}

#[tokio::test]
async fn active_read_revalidates_and_invokes_the_exact_connection_credential() {
    let (_home, service, http, _connection_id) = fixture(AdapterHttpOutcome::Success(json!({
        "id": "one"
    })));
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
    assert_eq!(output.payload, json!({"id": "one"}));
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
async fn quarantine_after_advertisement_fences_the_send() {
    let (_home, service, http, connection_id) = fixture(AdapterHttpOutcome::Success(Value::Null));
    let invocation = advertised_invocation(&service).await;
    let mut stale = invocation.clone();
    let mut authority = AdapterOperationAuthorityV1::from_operation_token(&stale.operation_token)
        .expect("authority");
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
        .quarantine_connection(&connection_id)
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
    let (_home, service, _http, _connection_id) =
        fixture(AdapterHttpOutcome::AuthenticationRequired);
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
