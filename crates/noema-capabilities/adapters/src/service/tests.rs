use super::*;
use crate::{
    AdapterManifestV1, HttpMethod, RetryPolicy,
    network::{
        AdapterBearerCredential, AdapterHttpError, AdapterHttpFuture, AdapterOAuthTokenFuture,
        AdapterOAuthTokenOutcome,
    },
    request::EncodedAdapterRequest,
};
use noema_home::NoemaPaths;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::{sync::Notify, time::Duration};
use url::Url;

#[derive(Default)]
struct SyntheticOAuthHttp {
    exchanges: Mutex<usize>,
    gate: Option<Arc<ExchangeGate>>,
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
        Box::pin(async { Err(AdapterHttpError::Unavailable) })
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

fn manifest() -> AdapterManifestV1 {
    serde_json::from_value(json!({
        "schema_version": 1,
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
            }
        },
        "provider_data_policy": {"retention_allowed": false, "deletion_supported": true},
        "quota": {"cost_class": "free"},
        "operations": [{
            "operation_id": "list_events",
            "method": "GET",
            "path": "/v1/events",
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
    .expect("manifest")
}

#[tokio::test]
async fn setup_callback_exchanges_once_and_publishes_active_token_generation() {
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
    let http = Arc::new(SyntheticOAuthHttp::default());
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
    assert_eq!(
        completion
            .await
            .expect("completion task")
            .expect("completion")
            .descriptor
            .status,
        AdapterConnectionStatus::Active
    );
}
