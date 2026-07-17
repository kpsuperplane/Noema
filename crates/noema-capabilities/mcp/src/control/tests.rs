use std::collections::BTreeMap;

use crate::{
    ContinueMcpServerSetupCommand, CreateMcpServerCommand, McpClientError, McpDiscoveryStatus,
    McpListToolsCommand, McpOperationError, McpOperations, McpSecretMaterial, McpSetupIssue,
    McpSetupStatus, McpSetupTransportConfig, McpStdioSetupConfig, McpStreamableHttpSetupConfig,
    service::test_support::{TestHarness, secret_material},
};

#[tokio::test]
async fn auth_required_create_discards_staged_secrets_and_projects_typed_browser_auth() {
    let harness = TestHarness::new();
    harness
        .sessions
        .set_prepare_error(McpClientError::AuthenticationRequired(
            "credentials rejected".to_string(),
        ));
    let private_header = "Bearer create-private-secret";

    let result =
        McpOperations::create_server(&harness.service, streamable_create_command(private_header))
            .await
            .expect("authentication-required setup result");

    assert_eq!(result.setup_status, McpSetupStatus::NeedsAuth);
    assert_eq!(result.discovery_status, Some(McpDiscoveryStatus::NeedsAuth));
    assert_eq!(result.issue, Some(McpSetupIssue::AuthenticationRequired));
    assert!(result.server.is_none());
    let auth = result.auth.as_ref().expect("typed auth projection");
    assert!(auth.oauth_client_credentials_supported);
    assert!(auth.oauth_authorization_supported);
    assert!(auth.scopes.is_empty());
    assert!(!format!("{result:?}").contains(private_header));
    assert!(!harness.secrets.active_file_exists("mcp:created"));
    assert_eq!(harness.secrets.staged_file_count(), 0);
    let events = harness.events.lock().expect("events").clone();
    assert!(events.contains(&"secret_stage"), "events: {events:?}");
    assert!(events.contains(&"session_prepare"), "events: {events:?}");
    assert!(events.contains(&"secret_discard"), "events: {events:?}");
    assert!(!events.contains(&"secret_commit"), "events: {events:?}");
    assert!(
        !harness
            .repository
            .events()
            .contains(&"commit_initial_discovery")
    );
    assert!(
        !format!("{:?}", harness.diagnostics.take()).contains(private_header),
        "input secret leaked to diagnostics"
    );
}

#[tokio::test]
async fn streamable_http_create_and_continue_persist_secret_headers_and_discover_tools() {
    let harness = TestHarness::new();
    let initial_secret = "Bearer initial-private-secret";
    let created =
        McpOperations::create_server(&harness.service, streamable_create_command(initial_secret))
            .await
            .expect("create streamable HTTP server");

    assert_eq!(created.setup_status, McpSetupStatus::ReadyForCalibration);
    assert_eq!(
        created.discovery_status,
        Some(McpDiscoveryStatus::Discovered)
    );
    assert_eq!(created.discovered_tool_count, 1);
    let created_server = created.server.expect("created server");
    assert_eq!(created_server.mcp_server_id, "mcp:created");
    assert_eq!(created_server.tool_count, 1);
    assert_eq!(created_server.safe_config["url"], "https://example.com/mcp");
    assert_eq!(created_server.safe_config["headers"]["X-Team"], "infra");
    assert_eq!(
        created_server.safe_config["secret_refs"]["headers"],
        serde_json::json!(["Authorization"])
    );
    assert!(
        !created_server
            .safe_config
            .to_string()
            .contains(initial_secret)
    );
    let initial_material = harness.secrets.material("mcp:created");
    assert_eq!(
        initial_material
            .headers
            .get("Authorization")
            .map(String::as_str),
        Some(initial_secret)
    );
    assert_eq!(
        created_server.safe_config["secret_identity_revision"].as_str(),
        initial_material.secret_identity_revision.as_deref()
    );

    let replacement_secret = "Bearer replacement-private-secret";
    let continued = McpOperations::continue_setup(
        &harness.service,
        ContinueMcpServerSetupCommand {
            mcp_server_id: "mcp:created".to_string(),
            secrets: McpSecretMaterial {
                headers: BTreeMap::from([(
                    "Authorization".to_string(),
                    replacement_secret.to_string(),
                )]),
                ..McpSecretMaterial::default()
            },
        },
    )
    .await
    .expect("continue streamable HTTP setup");

    assert_eq!(continued.setup_status, McpSetupStatus::ReadyForCalibration);
    assert_eq!(
        continued.discovery_status,
        Some(McpDiscoveryStatus::Discovered)
    );
    assert_eq!(continued.discovered_tool_count, 1);
    let continued_server = continued.server.expect("continued server");
    let replacement_material = harness.secrets.material("mcp:created");
    assert_eq!(
        replacement_material
            .headers
            .get("Authorization")
            .map(String::as_str),
        Some(replacement_secret)
    );
    assert_ne!(
        replacement_material.secret_identity_revision,
        initial_material.secret_identity_revision
    );
    assert_eq!(
        continued_server.safe_config["secret_identity_revision"].as_str(),
        replacement_material.secret_identity_revision.as_deref()
    );
    let safe_config = continued_server.safe_config.to_string();
    assert!(!safe_config.contains(initial_secret));
    assert!(!safe_config.contains(replacement_secret));

    let persisted = McpOperations::list_tools(
        &harness.service,
        McpListToolsCommand {
            mcp_server_id: "mcp:created".to_string(),
        },
    )
    .await
    .expect("persisted tools");
    assert_eq!(persisted.tools.len(), 1);
    assert_eq!(persisted.tools[0].tool.name, "read");
    let secret_file = String::from_utf8(harness.secrets.active_file_bytes("mcp:created"))
        .expect("UTF-8 secret file");
    assert!(secret_file.contains(replacement_secret));
    assert!(!secret_file.contains(initial_secret));
}

#[tokio::test]
async fn corrupt_active_secret_blocks_continuation_without_overwrite() {
    let harness = TestHarness::new();
    harness.secrets.seed("mcp:docs", &secret_material("old"));
    let corrupt = b"{private-corrupt-active-secret";
    harness.secrets.overwrite_active_file("mcp:docs", corrupt);

    let result = McpOperations::continue_setup(
        &harness.service,
        ContinueMcpServerSetupCommand {
            mcp_server_id: "mcp:docs".to_string(),
            secrets: secret_material("replacement"),
        },
    )
    .await;

    assert_eq!(result, Err(McpOperationError::Unavailable));
    assert_eq!(harness.secrets.active_file_bytes("mcp:docs"), corrupt);
    let events = harness.events.lock().expect("events").clone();
    assert_eq!(events, vec!["secret_load"], "events: {events:?}");
    assert!(!harness.repository.events().contains(&"replace_connection"));
    assert!(!format!("{:?}", harness.diagnostics.take()).contains("private-corrupt-active-secret"));
}

fn streamable_create_command(secret: &str) -> CreateMcpServerCommand {
    CreateMcpServerCommand {
        display_name: "Remote".to_string(),
        transport: McpSetupTransportConfig::StreamableHttp(McpStreamableHttpSetupConfig {
            url: "https://example.com/mcp".to_string(),
            headers: BTreeMap::from([("X-Team".to_string(), "infra".to_string())]),
        }),
        secrets: McpSecretMaterial {
            headers: BTreeMap::from([("Authorization".to_string(), secret.to_string())]),
            ..McpSecretMaterial::default()
        },
    }
}

#[tokio::test]
async fn repository_replacement_failure_discards_stage_without_touching_active_secrets() {
    let harness = TestHarness::new();
    harness.secrets.seed("mcp:docs", &secret_material("old"));
    harness
        .repository
        .fail_replacement("private repository conflict detail");

    let result = McpOperations::continue_setup(
        &harness.service,
        ContinueMcpServerSetupCommand {
            mcp_server_id: "mcp:docs".to_string(),
            secrets: secret_material("new"),
        },
    )
    .await;

    assert_eq!(result, Err(McpOperationError::Conflict));
    assert_eq!(harness.secrets.material("mcp:docs"), secret_material("old"));
    let events = harness.events.lock().expect("events").clone();
    assert!(events.contains(&"secret_stage"), "events: {events:?}");
    assert!(events.contains(&"secret_discard"), "events: {events:?}");
    assert!(!events.contains(&"secret_commit"), "events: {events:?}");
}

#[tokio::test]
async fn failed_final_secret_commit_compensates_initial_repository_commit() {
    let harness = TestHarness::new();
    harness.secrets.fail_commit_for("mcp:created");
    let result = McpOperations::create_server(
        &harness.service,
        CreateMcpServerCommand {
            display_name: "Created".to_string(),
            transport: McpSetupTransportConfig::Stdio(McpStdioSetupConfig {
                command: "created-server".to_string(),
                args: Vec::new(),
                cwd: None,
                env: BTreeMap::new(),
            }),
            secrets: McpSecretMaterial::default(),
        },
    )
    .await;

    assert_eq!(result, Err(McpOperationError::Unavailable));
    let events = harness.repository.events();
    assert!(
        events.contains(&"commit_initial_discovery"),
        "events: {events:?}"
    );
    assert!(events.contains(&"begin_delete"), "events: {events:?}");
    assert!(events.contains(&"finish_delete"), "events: {events:?}");
}

#[tokio::test]
async fn interrupted_replacement_cannot_reuse_old_credentials_with_the_same_keys() {
    let harness = TestHarness::new();
    let old = McpSecretMaterial {
        secret_identity_revision: Some("old-revision".to_string()),
        env: BTreeMap::from([("TOKEN".to_string(), "old-value".to_string())]),
        ..McpSecretMaterial::default()
    };
    harness.secrets.seed("mcp:docs", &old);
    harness.repository.set_safe_config(serde_json::json!({
        "command": "docs-server",
        "args": [],
        "cwd": null,
        "env": {},
        "secret_refs": {"env": ["TOKEN"]},
        "secret_identity_revision": "new-revision"
    }));

    let result = McpOperations::continue_setup(
        &harness.service,
        ContinueMcpServerSetupCommand {
            mcp_server_id: "mcp:docs".to_string(),
            secrets: McpSecretMaterial::default(),
        },
    )
    .await;

    assert_eq!(result, Err(McpOperationError::Unavailable));
    assert_eq!(harness.secrets.material("mcp:docs"), old);
    assert!(
        !harness
            .events
            .lock()
            .expect("events")
            .contains(&"session_prepare")
    );
}
