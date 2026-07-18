use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::protocol::{
    challenge_metadata_url, fetch_metadata, metadata_candidates, metadata_resource,
};
use super::*;
use crate::{McpSecretMaterial, McpStreamableHttpSetupConfig};
use rmcp::model::ProtocolVersion;
use url::Url;

struct FakeBackend {
    callback_count: Arc<AtomicUsize>,
    callback_delay: Duration,
    authorization_url: String,
}

impl McpOAuthBackend for FakeBackend {
    fn start<'a>(&'a self, _: &'a str, _: &'a str) -> OAuthFuture<'a, McpOAuthStarted> {
        let runtime = FakeRuntime {
            callback_count: Arc::clone(&self.callback_count),
            callback_delay: self.callback_delay,
        };
        let authorization_url = self.authorization_url.clone();
        Box::pin(async move {
            Ok(McpOAuthStarted {
                authorization_url,
                runtime: Box::new(runtime),
            })
        })
    }
}

struct FakeRuntime {
    callback_count: Arc<AtomicUsize>,
    callback_delay: Duration,
}

impl McpOAuthRuntime for FakeRuntime {
    fn complete<'a>(&'a mut self, _: &'a str) -> OAuthFuture<'a, McpOAuthStoredCredentials> {
        self.callback_count.fetch_add(1, Ordering::SeqCst);
        let delay = self.callback_delay;
        Box::pin(async move {
            tokio::time::sleep(delay).await;
            Ok(McpOAuthStoredCredentials {
                client_id: "private-client".to_string(),
                token_response: json!({"access_token": "private-token"}),
                token_received_at: Some(42),
            })
        })
    }
}

fn test_registry(config: McpOAuthRegistryConfig) -> (McpOAuthRegistry, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let backend = FakeBackend {
        callback_count: Arc::clone(&count),
        callback_delay: Duration::ZERO,
        authorization_url: "https://auth.example/?state=authorization-secret".to_string(),
    };
    (
        McpOAuthRegistry::with_backend(config, CancellationToken::new(), Arc::new(backend)),
        count,
    )
}

fn start_request(label: &str) -> McpOAuthStartRequest {
    McpOAuthStartRequest {
        context: McpOAuthAttemptContext::PendingCreate(Box::new(CreateMcpServerCommand {
            display_name: label.to_string(),
            transport: McpSetupTransportConfig::StreamableHttp(McpStreamableHttpSetupConfig {
                url: "https://mcp.example/mcp".to_string(),
                headers: BTreeMap::new(),
            }),
            secrets: McpSecretMaterial::default(),
        })),
        redirect_uri: "http://127.0.0.1/oauth/callback".to_string(),
    }
}

#[tokio::test]
async fn registry_attempt_lifecycle_contracts() {
    // Case: callback_is_consumed_exactly_once.
    let (registry, count) = test_registry(McpOAuthRegistryConfig::default());
    let view = registry
        .start_attempt(start_request("Docs"))
        .await
        .expect("start");
    let callback = || CompleteMcpOAuthSetupCommand {
        attempt_id: view.attempt_id.clone(),
        callback_url: "http://127.0.0.1/callback?code=secret&state=secret".to_string(),
    };

    let completion = registry
        .complete_callback(callback())
        .await
        .expect("callback");
    let error = registry
        .complete_callback(callback())
        .await
        .expect_err("second callback rejected");

    assert_eq!(error.kind(), McpOAuthErrorKind::Conflict);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(completion.credentials.client_id, "private-client");
    assert!(matches!(
        completion.context,
        McpOAuthAttemptContext::PendingCreate(_)
    ));

    // Case: capacity_evicts_oldest_and_ttl_expires_entries.
    let (registry, _) = test_registry(McpOAuthRegistryConfig {
        capacity: 1,
        ..McpOAuthRegistryConfig::default()
    });
    let first = registry
        .start_attempt(start_request("First"))
        .await
        .expect("first");
    let second = registry
        .start_attempt(start_request("Second"))
        .await
        .expect("second");
    assert!(registry.attempt(&first.attempt_id).await.is_none());
    assert!(registry.attempt(&second.attempt_id).await.is_some());

    let (expiring, _) = test_registry(McpOAuthRegistryConfig {
        attempt_ttl: Duration::from_millis(1),
        ..McpOAuthRegistryConfig::default()
    });
    let view = expiring
        .start_attempt(start_request("Expires"))
        .await
        .expect("start");
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert!(expiring.attempt(&view.attempt_id).await.is_none());

    // Case: completing_attempt_is_not_expired_or_evicted_before_terminal_transition.
    let config = McpOAuthRegistryConfig {
        attempt_ttl: Duration::from_millis(1),
        capacity: 1,
        ..McpOAuthRegistryConfig::default()
    };
    let (registry, _) = test_registry(config);
    let view = registry
        .start_attempt(start_request("Protected"))
        .await
        .expect("start");
    let completion = registry
        .complete_callback(CompleteMcpOAuthSetupCommand {
            attempt_id: view.attempt_id.clone(),
            callback_url: "http://127.0.0.1/callback?code=x&state=y".to_string(),
        })
        .await
        .expect("callback");
    tokio::time::sleep(Duration::from_millis(5)).await;

    assert!(registry.attempt(&view.attempt_id).await.is_some());
    assert_eq!(
        registry
            .start_attempt(start_request("Capacity"))
            .await
            .expect_err("completing attempt is protected")
            .kind(),
        McpOAuthErrorKind::Capacity
    );

    registry
        .finish_failure(&completion, McpOAuthSetupFailure::DiscoveryFailed)
        .await
        .expect("terminal transition");
    registry
        .start_attempt(start_request("After terminal"))
        .await
        .expect("terminal attempt can be evicted");

    // Case: abandoned_completion_expires_and_releases_capacity.
    let config = McpOAuthRegistryConfig {
        attempt_ttl: Duration::from_secs(60),
        in_flight_ttl: Duration::from_millis(10),
        capacity: 1,
        ..McpOAuthRegistryConfig::default()
    };
    let (registry, _) = test_registry(config);
    let view = registry
        .start_attempt(start_request("Abandoned"))
        .await
        .expect("start");
    let _completion = registry
        .complete_callback(CompleteMcpOAuthSetupCommand {
            attempt_id: view.attempt_id.clone(),
            callback_url: "http://127.0.0.1/callback?code=x&state=y".to_string(),
        })
        .await
        .expect("callback");

    tokio::time::sleep(Duration::from_millis(25)).await;
    registry
        .start_attempt(start_request("Replacement"))
        .await
        .expect("expired in-flight attempt releases capacity");
    assert!(registry.attempt(&view.attempt_id).await.is_none());
}

#[tokio::test]
async fn oauth_protocol_safety_timeout_and_redaction_contracts() {
    // Case: browser_oauth_rejects_unsafe_authorization_redirect_and_endpoint_urls.
    for authorization_url in [
        "javascript:alert(1)",
        "file:///tmp/authorize",
        "http://auth.example/authorize",
        "https://user:password@auth.example/authorize",
        "https://auth.example/authorize#access_token=secret",
    ] {
        let registry = McpOAuthRegistry::with_backend(
            McpOAuthRegistryConfig::default(),
            CancellationToken::new(),
            Arc::new(FakeBackend {
                callback_count: Arc::new(AtomicUsize::new(0)),
                callback_delay: Duration::ZERO,
                authorization_url: authorization_url.to_string(),
            }),
        );

        let error = registry
            .start_attempt(start_request("Unsafe"))
            .await
            .expect_err("unsafe authorization URL");
        assert_eq!(error.kind(), McpOAuthErrorKind::Unavailable);
        assert!(registry.state.lock().await.attempts.is_empty());
    }

    let (registry, _) = test_registry(McpOAuthRegistryConfig::default());
    let mut remote_redirect = start_request("Remote callback");
    remote_redirect.redirect_uri = "https://callback.example/oauth".to_string();
    assert_eq!(
        registry
            .start_attempt(remote_redirect)
            .await
            .expect_err("redirect listener must be loopback")
            .kind(),
        McpOAuthErrorKind::InvalidInput
    );

    let mut insecure_endpoint = start_request("Insecure endpoint");
    let McpOAuthAttemptContext::PendingCreate(command) = &mut insecure_endpoint.context else {
        panic!("create context")
    };
    let McpSetupTransportConfig::StreamableHttp(config) = &mut command.transport else {
        panic!("HTTP transport")
    };
    config.url = "http://mcp.example/mcp".to_string();
    assert_eq!(
        registry
            .start_attempt(insecure_endpoint)
            .await
            .expect_err("remote OAuth endpoint must use HTTPS")
            .kind(),
        McpOAuthErrorKind::InvalidInput
    );

    // Case: deadline_and_cancellation_fail_closed.
    let count = Arc::new(AtomicUsize::new(0));
    let registry = McpOAuthRegistry::with_backend(
        McpOAuthRegistryConfig {
            callback_timeout: Duration::from_millis(1),
            ..McpOAuthRegistryConfig::default()
        },
        CancellationToken::new(),
        Arc::new(FakeBackend {
            callback_count: count,
            callback_delay: Duration::from_millis(20),
            authorization_url: "https://auth.example/?state=authorization-secret".to_string(),
        }),
    );
    let view = registry
        .start_attempt(start_request("Deadline"))
        .await
        .expect("start");
    let error = registry
        .complete_callback(CompleteMcpOAuthSetupCommand {
            attempt_id: view.attempt_id.clone(),
            callback_url: "http://127.0.0.1/callback?code=x&state=y".to_string(),
        })
        .await
        .expect_err("timeout");
    assert_eq!(error.kind(), McpOAuthErrorKind::Timeout);
    assert_eq!(
        registry
            .attempt(&view.attempt_id)
            .await
            .expect("failed view")
            .status,
        McpOAuthSetupAttemptStatus::Failed
    );

    let shutdown = CancellationToken::new();
    shutdown.cancel();
    let cancelled = McpOAuthRegistry::with_backend(
        McpOAuthRegistryConfig::default(),
        shutdown,
        Arc::new(FakeBackend {
            callback_count: Arc::new(AtomicUsize::new(0)),
            callback_delay: Duration::ZERO,
            authorization_url: "https://auth.example/?state=authorization-secret".to_string(),
        }),
    );
    assert_eq!(
        cancelled
            .start_attempt(start_request("Cancelled"))
            .await
            .expect_err("cancelled")
            .kind(),
        McpOAuthErrorKind::Cancelled
    );

    // Case: debug_redacts_urls_ids_credentials_and_diagnostics.
    let request = start_request("private-display");
    let completion = McpOAuthCompletion {
        attempt_id: "private-attempt".to_string(),
        sequence: 1,
        context: request.context.clone(),
        credentials: McpOAuthStoredCredentials {
            client_id: "private-client".to_string(),
            token_response: json!({"access_token": "private-token"}),
            token_received_at: None,
        },
    };
    let error = McpOAuthError::new(
        McpOAuthErrorKind::Authentication,
        "callback=https://local/?code=private-code",
    );
    let debug = format!("{request:?} {completion:?} {error:?}");
    for secret in [
        "private-display",
        "private-attempt",
        "private-client",
        "private-token",
        "private-code",
    ] {
        assert!(!debug.contains(secret), "debug leaked {secret}");
    }
    assert!(debug.contains(REDACTED));

    // Case: metadata_probe_sends_current_protocol_version.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("request");
        let mut request = Vec::new();
        loop {
            let mut buffer = [0_u8; 1024];
            let read = stream.read(&mut buffer).await.expect("read");
            request.extend_from_slice(&buffer[..read]);
            if read == 0 || request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8(request).expect("UTF-8");
        let expected = format!("mcp-protocol-version: {}", ProtocolVersion::LATEST.as_str());
        assert!(request.to_ascii_lowercase().contains(&expected));
        let body = r#"{"resource":"http://127.0.0.1/"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await.expect("write");
    });
    let url = Url::parse(&format!("http://{address}/metadata")).expect("url");

    assert!(fetch_metadata(&url).await.is_some());
    server.await.expect("server task");

    // Case: protected_resource_metadata_discovery_is_origin_scoped.
    let endpoint = Url::parse("https://mcp.example/mcp").expect("url");
    let accepted = json!({
        "resource": "https://mcp.example/",
        "authorization_servers": ["https://auth.example/"]
    });
    let rejected = json!({
        "resource": "https://other.example/",
        "authorization_servers": ["https://auth.example/"]
    });

    assert_eq!(
        metadata_resource(&endpoint, &accepted).as_deref(),
        Some("https://mcp.example/")
    );
    assert_eq!(metadata_resource(&endpoint, &rejected), None);

    assert_eq!(
        metadata_candidates(&endpoint)
            .iter()
            .map(Url::as_str)
            .collect::<Vec<_>>(),
        vec![
            "https://mcp.example/.well-known/oauth-protected-resource/mcp",
            "https://mcp.example/.well-known/oauth-protected-resource",
        ]
    );
    let endpoint = Url::parse("https://mcp.example:8443/mcp").expect("url");

    assert_eq!(
        challenge_metadata_url(
            r#"Bearer resource_metadata="/.well-known/oauth-protected-resource""#,
            &endpoint,
        )
        .expect("same-origin URL")
        .as_str(),
        "https://mcp.example:8443/.well-known/oauth-protected-resource"
    );
    for challenge in [
        r#"Bearer resource_metadata="https://internal.example/metadata""#,
        r#"Bearer resource_metadata="http://mcp.example:8443/metadata""#,
        r#"Bearer resource_metadata="https://mcp.example/metadata""#,
        r#"Bearer resource_metadata="https://user:password@mcp.example:8443/metadata""#,
        r##"Bearer resource_metadata="https://mcp.example:8443/metadata#fragment""##,
    ] {
        assert!(
            challenge_metadata_url(challenge, &endpoint).is_none(),
            "{challenge}"
        );
    }
}
