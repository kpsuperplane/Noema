//! Guided MCP server setup orchestration.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpTransportKind, NewMcpServer,
    NewMcpTool, NoemaPaths, NoemaStore, StoreError,
    mcp::{
        client::{DiscoveredMcpTool, McpClientError, McpClientRuntime, McpTransport},
        secrets::{McpSecretMaterial, read_mcp_secrets, write_mcp_secrets},
    },
};

/// Status returned by guided MCP setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSetupStatus {
    /// Setup cannot continue until the user supplies or updates credentials.
    NeedsAuth,
    /// Metadata discovery succeeded and the UI should prompt calibration.
    ReadyForCalibration,
    /// Transport, command, network, or endpoint setup failed.
    Unavailable,
    /// MCP metadata was malformed.
    Malformed,
}

impl McpSetupStatus {
    /// Return the GraphQL/status string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeedsAuth => "needs_auth",
            Self::ReadyForCalibration => "ready_for_calibration",
            Self::Unavailable => "unavailable",
            Self::Malformed => "malformed",
        }
    }
}

/// Result of guided MCP setup.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerSetupResult {
    /// Server metadata row safe to return to clients, when persisted.
    pub server: Option<McpServerRecord>,
    /// High-level setup status.
    pub setup_status: McpSetupStatus,
    /// Metadata discovery status, when discovery was attempted.
    pub discovery_status: Option<String>,
    /// Number of tools discovered and persisted.
    pub discovered_tool_count: usize,
    /// Non-secret setup error text suitable for the UI.
    pub setup_error: Option<String>,
    /// Authentication options detected or applicable for this setup result.
    pub auth: Option<McpSetupAuthDetails>,
}

/// Authentication options safe to return during guided MCP setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSetupAuthDetails {
    /// Whether OAuth client-secret credentials can be attempted for this server.
    pub oauth_client_credentials_supported: bool,
    /// OAuth scopes Noema should suggest, when known.
    pub scopes: Vec<String>,
}

/// New MCP server setup input after GraphQL/local validation.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpServerSetup {
    /// Human-visible server name.
    pub display_name: String,
    /// Transport kind to configure.
    pub transport_kind: McpTransportKind,
    /// Non-secret transport config.
    pub safe_config: Value,
    /// Secret setup material stored on disk.
    pub secrets: McpSecretMaterial,
}

/// Continue setup by updating disk-backed secrets and retrying discovery.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinueMcpServerSetup {
    /// Durable MCP server id.
    pub mcp_server_id: String,
    /// New or replacement secret setup material.
    pub secrets: McpSecretMaterial,
}

/// Create, verify, and discover metadata for an MCP server.
///
/// # Errors
///
/// Returns [`StoreError`] for validation, secret write, or store failures.
pub async fn create_mcp_server_setup<T>(
    store: &NoemaStore,
    paths: &NoemaPaths,
    input: NewMcpServerSetup,
    make_transport: impl Fn(&McpServerRecord, &McpSecretMaterial) -> T,
) -> Result<McpServerSetupResult, StoreError>
where
    T: McpTransport,
{
    let display_name = validate_display_name(&input.display_name)?;
    let mcp_server_id = next_server_id(store, &display_name).await?;
    let safe_config =
        normalize_safe_config(input.transport_kind, input.safe_config, &input.secrets)?;
    let preview = preview_mcp_server(
        mcp_server_id.clone(),
        display_name.clone(),
        input.transport_kind,
        safe_config.clone(),
    );
    let mut runtime = McpClientRuntime::new(make_transport(&preview, &input.secrets));
    let tools = match runtime.discover_tools().await {
        Ok(tools) => tools,
        Err(error) => {
            return Ok(unpersisted_setup_result_from_error(
                input.transport_kind,
                error,
            ));
        }
    };

    let server_home = paths.mcp_server_home(&mcp_server_id);
    write_mcp_secrets(&server_home, &input.secrets)
        .map_err(|error| StoreError::Schema(format!("failed to write MCP secrets: {error}")))?;
    let server = store
        .create_mcp_server(NewMcpServer {
            mcp_server_id,
            display_name,
            transport_kind: input.transport_kind,
            safe_config,
        })
        .await?;
    let auth_status = auth_status_for_verified_secrets(&input.secrets);
    persist_discovered_tools(store, server, tools, auth_status).await
}

/// Update secrets for an existing MCP server and retry metadata discovery.
///
/// # Errors
///
/// Returns [`StoreError`] for missing server, secret write, or store failures.
pub async fn continue_mcp_server_setup<T>(
    store: &NoemaStore,
    paths: &NoemaPaths,
    input: ContinueMcpServerSetup,
    make_transport: impl Fn(&McpServerRecord, &McpSecretMaterial) -> T,
) -> Result<McpServerSetupResult, StoreError>
where
    T: McpTransport,
{
    let mut server = store
        .get_mcp_server(&input.mcp_server_id)
        .await?
        .ok_or_else(|| {
            StoreError::Schema(format!("missing MCP server: {}", input.mcp_server_id))
        })?;
    let server_home = paths.mcp_server_home(&input.mcp_server_id);
    let mut secrets = read_mcp_secrets(&server_home).unwrap_or_default();
    secrets.env.extend(input.secrets.env);
    secrets.headers.extend(input.secrets.headers);
    write_mcp_secrets(&server_home, &secrets)
        .map_err(|error| StoreError::Schema(format!("failed to write MCP secrets: {error}")))?;
    server.safe_config = safe_config_with_secret_refs(server.safe_config, &secrets)?;
    update_mcp_server_safe_config(store, &server.mcp_server_id, server.safe_config.clone()).await?;
    let server = store
        .get_mcp_server(&input.mcp_server_id)
        .await?
        .ok_or_else(|| {
            StoreError::Schema(format!("missing MCP server: {}", input.mcp_server_id))
        })?;
    discover_and_persist_tools(store, server, &secrets, make_transport).await
}

async fn discover_and_persist_tools<T>(
    store: &NoemaStore,
    server: McpServerRecord,
    secrets: &McpSecretMaterial,
    make_transport: impl Fn(&McpServerRecord, &McpSecretMaterial) -> T,
) -> Result<McpServerSetupResult, StoreError>
where
    T: McpTransport,
{
    let mut runtime = McpClientRuntime::new(make_transport(&server, secrets));
    match runtime.discover_tools().await {
        Ok(tools) => {
            let auth_status = auth_status_for_verified_secrets(secrets);
            persist_discovered_tools(store, server, tools, auth_status).await
        }
        Err(error) => {
            setup_result_from_error(store, &server.mcp_server_id, server.transport_kind, error)
                .await
        }
    }
}

async fn persist_discovered_tools(
    store: &NoemaStore,
    server: McpServerRecord,
    tools: Vec<DiscoveredMcpTool>,
    auth_status: McpServerAuthStatus,
) -> Result<McpServerSetupResult, StoreError> {
    let count = tools.len();
    for tool in tools {
        store
            .upsert_discovered_mcp_tool(new_mcp_tool(&server, tool))
            .await?;
    }
    let server = store
        .update_mcp_server_setup_status(
            &server.mcp_server_id,
            McpServerHealthStatus::Healthy,
            auth_status,
        )
        .await?;
    Ok(McpServerSetupResult {
        server: Some(server),
        setup_status: McpSetupStatus::ReadyForCalibration,
        discovery_status: Some("discovered".to_string()),
        discovered_tool_count: count,
        setup_error: None,
        auth: None,
    })
}

async fn setup_result_from_error(
    store: &NoemaStore,
    mcp_server_id: &str,
    transport_kind: McpTransportKind,
    error: McpClientError,
) -> Result<McpServerSetupResult, StoreError> {
    let (setup_status, discovery_status, health_status, auth_status, setup_error, auth) =
        match error {
            McpClientError::AuthRequired(_) => (
                McpSetupStatus::NeedsAuth,
                "needs_auth",
                McpServerHealthStatus::Unavailable,
                McpServerAuthStatus::NeedsAuth,
                Some(safe_setup_error_text(McpSetupStatus::NeedsAuth)),
                auth_details_for_transport(transport_kind),
            ),
            McpClientError::Transport(_) => (
                McpSetupStatus::Unavailable,
                "unavailable",
                McpServerHealthStatus::Unavailable,
                McpServerAuthStatus::Unavailable,
                Some(safe_setup_error_text(McpSetupStatus::Unavailable)),
                None,
            ),
            McpClientError::Malformed(_) => (
                McpSetupStatus::Malformed,
                "malformed",
                McpServerHealthStatus::Unavailable,
                McpServerAuthStatus::Unavailable,
                Some(safe_setup_error_text(McpSetupStatus::Malformed)),
                None,
            ),
        };
    let server = store
        .update_mcp_server_setup_status(mcp_server_id, health_status, auth_status)
        .await?;
    Ok(McpServerSetupResult {
        server: Some(server),
        setup_status,
        discovery_status: Some(discovery_status.to_string()),
        discovered_tool_count: 0,
        setup_error,
        auth,
    })
}

fn unpersisted_setup_result_from_error(
    transport_kind: McpTransportKind,
    error: McpClientError,
) -> McpServerSetupResult {
    let (setup_status, discovery_status, setup_error, auth) = match error {
        McpClientError::AuthRequired(_) => (
            McpSetupStatus::NeedsAuth,
            "needs_auth",
            Some(safe_setup_error_text(McpSetupStatus::NeedsAuth)),
            auth_details_for_transport(transport_kind),
        ),
        McpClientError::Transport(_) => (
            McpSetupStatus::Unavailable,
            "unavailable",
            Some(safe_setup_error_text(McpSetupStatus::Unavailable)),
            None,
        ),
        McpClientError::Malformed(_) => (
            McpSetupStatus::Malformed,
            "malformed",
            Some(safe_setup_error_text(McpSetupStatus::Malformed)),
            None,
        ),
    };
    McpServerSetupResult {
        server: None,
        setup_status,
        discovery_status: Some(discovery_status.to_string()),
        discovered_tool_count: 0,
        setup_error,
        auth,
    }
}

fn auth_details_for_transport(transport_kind: McpTransportKind) -> Option<McpSetupAuthDetails> {
    match transport_kind {
        McpTransportKind::Sse | McpTransportKind::StreamableHttp => Some(McpSetupAuthDetails {
            oauth_client_credentials_supported: true,
            scopes: Vec::new(),
        }),
        McpTransportKind::Stdio => None,
    }
}

fn auth_status_for_verified_secrets(secrets: &McpSecretMaterial) -> McpServerAuthStatus {
    if secrets.has_secret_material() {
        McpServerAuthStatus::Authenticated
    } else {
        McpServerAuthStatus::None
    }
}

fn safe_setup_error_text(status: McpSetupStatus) -> String {
    match status {
        McpSetupStatus::NeedsAuth => {
            "This MCP server requires authentication before Noema can list tools.".to_string()
        }
        McpSetupStatus::Unavailable => {
            "Noema could not connect to this MCP server. Check the transport details and try again."
                .to_string()
        }
        McpSetupStatus::Malformed => {
            "Noema connected, but the server returned unsupported tool metadata.".to_string()
        }
        McpSetupStatus::ReadyForCalibration => "MCP setup completed.".to_string(),
    }
}

fn preview_mcp_server(
    mcp_server_id: String,
    display_name: String,
    transport_kind: McpTransportKind,
    safe_config: Value,
) -> McpServerRecord {
    McpServerRecord {
        mcp_server_id,
        display_name,
        transport_kind,
        safe_config,
        enabled: false,
        health_status: McpServerHealthStatus::Unknown,
        auth_status: McpServerAuthStatus::None,
        tool_count: 0,
    }
}

fn new_mcp_tool(server: &McpServerRecord, tool: DiscoveredMcpTool) -> NewMcpTool {
    let mcp_tool_id = format!(
        "mcp_tool:{}:{}",
        safe_id_fragment(&server.mcp_server_id),
        safe_id_fragment(&tool.name)
    );
    let metadata_fingerprint = discovered_tool_fingerprint(&tool);
    NewMcpTool {
        mcp_tool_id,
        mcp_server_id: server.mcp_server_id.clone(),
        name: tool.name,
        description: tool.description,
        input_schema: tool.input_schema,
        output_schema: tool.output_schema,
        annotations: tool.annotations,
        metadata_fingerprint,
    }
}

fn discovered_tool_fingerprint(tool: &DiscoveredMcpTool) -> String {
    let payload = json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.input_schema,
        "output_schema": tool.output_schema,
        "annotations": tool.annotations
    });
    let json_string = serde_json::to_string(&payload).expect("metadata fingerprint JSON");
    format!(
        "mcp-tool-metadata:v1:{}",
        stable_hex_fingerprint(json_string.as_bytes())
    )
}

fn stable_hex_fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn validate_display_name(display_name: &str) -> Result<String, StoreError> {
    let trimmed = display_name.trim();
    if trimmed.is_empty() {
        return Err(StoreError::Schema(
            "MCP server display name cannot be empty".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

async fn next_server_id(store: &NoemaStore, display_name: &str) -> Result<String, StoreError> {
    let base = server_id_from_display_name(display_name);
    let mut candidate = base.clone();
    let mut suffix = 2;
    while store.get_mcp_server(&candidate).await?.is_some() {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    Ok(candidate)
}

fn server_id_from_display_name(display_name: &str) -> String {
    let slug = display_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else if matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    format!("mcp:{}", if slug.is_empty() { "server" } else { &slug })
}

fn normalize_safe_config(
    transport_kind: McpTransportKind,
    safe_config: Value,
    secrets: &McpSecretMaterial,
) -> Result<Value, StoreError> {
    let object = safe_config
        .as_object()
        .ok_or_else(|| StoreError::Schema("MCP safe config must be a JSON object".to_string()))?;
    match transport_kind {
        McpTransportKind::Stdio => {
            let command = string_field(object, "command")?;
            if command.trim().is_empty() {
                return Err(StoreError::Schema(
                    "MCP stdio command cannot be empty".to_string(),
                ));
            }
            let args = string_array_field(object, "args")?;
            let cwd = optional_string_field(object, "cwd")?;
            let env = string_map_field(object, "env")?;
            reject_secret_shaped_keys("env", &env)?;
            Ok(safe_config_with_secret_refs(
                json!({
                    "command": command,
                    "args": args,
                    "cwd": cwd,
                    "env": env
                }),
                secrets,
            )?)
        }
        McpTransportKind::Sse | McpTransportKind::StreamableHttp => {
            let url = string_field(object, "url")?;
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err(StoreError::Schema(
                    "MCP HTTP url must start with http:// or https://".to_string(),
                ));
            }
            let headers = string_map_field(object, "headers")?;
            reject_secret_shaped_keys("headers", &headers)?;
            Ok(safe_config_with_secret_refs(
                json!({
                    "url": url,
                    "headers": headers
                }),
                secrets,
            )?)
        }
    }
}

fn safe_config_with_secret_refs(
    mut safe_config: Value,
    secrets: &McpSecretMaterial,
) -> Result<Value, StoreError> {
    let object = safe_config
        .as_object_mut()
        .ok_or_else(|| StoreError::Schema("MCP safe config must be a JSON object".to_string()))?;
    let mut secret_refs = serde_json::Map::new();
    if !secrets.env.is_empty() {
        secret_refs.insert(
            "env".to_string(),
            Value::Array(secrets.env.keys().cloned().map(Value::String).collect()),
        );
    }
    if !secrets.headers.is_empty() {
        secret_refs.insert(
            "headers".to_string(),
            Value::Array(secrets.headers.keys().cloned().map(Value::String).collect()),
        );
    }
    if secrets.oauth_client_credentials.is_some() {
        secret_refs.insert("oauth_client_credentials".to_string(), Value::Bool(true));
    }
    if secret_refs.is_empty() {
        object.remove("secret_refs");
    } else {
        object.insert("secret_refs".to_string(), Value::Object(secret_refs));
    }
    Ok(safe_config)
}

fn string_field(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<String, StoreError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| StoreError::Schema(format!("MCP config field {field} must be a string")))
}

fn optional_string_field(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Option<String>, StoreError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(StoreError::Schema(format!(
            "MCP config field {field} must be a string"
        ))),
    }
}

fn string_array_field(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Vec<String>, StoreError> {
    match object.get(field) {
        None => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value.as_str().map(ToString::to_string).ok_or_else(|| {
                    StoreError::Schema(format!("MCP config field {field} must contain strings"))
                })
            })
            .collect(),
        Some(_) => Err(StoreError::Schema(format!(
            "MCP config field {field} must be an array"
        ))),
    }
}

fn string_map_field(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<BTreeMap<String, String>, StoreError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(BTreeMap::new()),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(key, value)| {
                value
                    .as_str()
                    .map(|string| (key.clone(), string.to_string()))
                    .ok_or_else(|| {
                        StoreError::Schema(format!(
                            "MCP config field {field} must contain string values"
                        ))
                    })
            })
            .collect(),
        Some(_) => Err(StoreError::Schema(format!(
            "MCP config field {field} must be an object"
        ))),
    }
}

fn reject_secret_shaped_keys(
    map_name: &'static str,
    map: &BTreeMap<String, String>,
) -> Result<(), StoreError> {
    for key in map.keys() {
        if safe_config_key_is_secret_shaped(key) {
            return Err(StoreError::Schema(format!(
                "MCP safe {map_name} contains secret-shaped key: {key}"
            )));
        }
    }
    Ok(())
}

fn safe_config_key_is_secret_shaped(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "authorization",
        "token",
        "api_key",
        "apikey",
        "cookie",
        "password",
        "credential",
        "secret",
        "private_key",
        "session",
        "set-cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn safe_id_fragment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else if matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if sanitized.is_empty() {
        "item".to_string()
    } else {
        sanitized
    }
}

async fn update_mcp_server_safe_config(
    store: &NoemaStore,
    mcp_server_id: &str,
    safe_config: Value,
) -> Result<(), StoreError> {
    store
        .db()
        .query(
            r#"
            UPDATE mcp_servers SET
              safe_config = $safe_config,
              updated_at = time::now()
            WHERE mcp_server_id = $mcp_server_id;
            "#,
        )
        .bind(("mcp_server_id", mcp_server_id.to_string()))
        .bind(("safe_config", safe_config))
        .await?
        .check()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        McpServerAuthStatus, McpServerHealthStatus, McpTransportKind, NoemaPaths, NoemaStore,
        StoreConfig,
        mcp::secrets::{McpOAuthClientCredentials, McpSecretMaterial, read_mcp_secrets},
        mcp::{DiscoveredMcpTool, McpClientError, McpTransport},
    };
    use serde_json::json;
    use std::{
        collections::{BTreeMap, VecDeque},
        sync::{Arc, Mutex},
    };
    use tempfile::TempDir;

    #[tokio::test]
    async fn create_stdio_setup_writes_secret_env_persists_safe_config_and_discovers_tools() {
        let fixture = TestFixture::new().await;
        let result = create_mcp_server_setup(
            &fixture.store,
            &fixture.paths,
            NewMcpServerSetup {
                display_name: "GitHub".to_string(),
                transport_kind: McpTransportKind::Stdio,
                safe_config: json!({
                    "command": "npx",
                    "args": ["-y", "server"],
                    "env": { "GITHUB_OWNER": "example" }
                }),
                secrets: McpSecretMaterial {
                    env: map_from_pairs([("GITHUB_TOKEN", "secret")]),
                    headers: BTreeMap::new(),
                    oauth_client_credentials: None,
                },
            },
            |_, _| FakeMcpTransport::ok(vec![fake_tool("list_repos")]),
        )
        .await
        .expect("setup");
        let server = result.server.as_ref().expect("persisted server");

        assert_eq!(result.setup_status, McpSetupStatus::ReadyForCalibration);
        assert_eq!(result.discovered_tool_count, 1);
        assert_eq!(server.mcp_server_id, "mcp:github");
        assert_eq!(server.health_status, McpServerHealthStatus::Healthy);
        assert_eq!(server.auth_status, McpServerAuthStatus::Authenticated);
        assert_eq!(
            server.safe_config,
            json!({
                "command": "npx",
                "args": ["-y", "server"],
                "cwd": null,
                "env": { "GITHUB_OWNER": "example" },
                "secret_refs": { "env": ["GITHUB_TOKEN"] }
            })
        );
        assert_eq!(
            read_mcp_secrets(&fixture.paths.mcp_server_home("mcp:github"))
                .expect("secrets")
                .env
                .get("GITHUB_TOKEN")
                .map(String::as_str),
            Some("secret")
        );
        let tools = fixture
            .store
            .list_mcp_tools_for_server("mcp:github")
            .await
            .expect("tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "list_repos");
    }

    #[tokio::test]
    async fn create_streamable_http_setup_writes_secret_headers_and_discovers_tools() {
        let fixture = TestFixture::new().await;
        let result = create_mcp_server_setup(
            &fixture.store,
            &fixture.paths,
            NewMcpServerSetup {
                display_name: "Remote".to_string(),
                transport_kind: McpTransportKind::StreamableHttp,
                safe_config: json!({
                    "url": "https://example.com/mcp",
                    "headers": { "X-Team": "infra" }
                }),
                secrets: McpSecretMaterial {
                    env: BTreeMap::new(),
                    headers: map_from_pairs([("Authorization", "Bearer secret")]),
                    oauth_client_credentials: Some(McpOAuthClientCredentials {
                        client_id: "client".to_string(),
                        client_secret: "oauth-secret".to_string(),
                        scopes: vec!["tools.read".to_string()],
                    }),
                },
            },
            |_, _| FakeMcpTransport::ok(vec![fake_tool("search")]),
        )
        .await
        .expect("setup");
        let server = result.server.as_ref().expect("persisted server");

        assert_eq!(result.setup_status, McpSetupStatus::ReadyForCalibration);
        assert_eq!(
            server.safe_config,
            json!({
                "url": "https://example.com/mcp",
                "headers": { "X-Team": "infra" },
                "secret_refs": {
                    "headers": ["Authorization"],
                    "oauth_client_credentials": true
                }
            })
        );
        assert!(!server.safe_config.to_string().contains("Bearer secret"));
        assert!(!server.safe_config.to_string().contains("oauth-secret"));
        assert_eq!(server.auth_status, McpServerAuthStatus::Authenticated);
        let tools = fixture
            .store
            .list_mcp_tools_for_server("mcp:remote")
            .await
            .expect("tools");
        assert_eq!(tools[0].name, "search");
    }

    #[tokio::test]
    async fn auth_required_setup_returns_needs_auth_without_secret_values() {
        let fixture = TestFixture::new().await;
        let result = create_mcp_server_setup(
            &fixture.store,
            &fixture.paths,
            NewMcpServerSetup {
                display_name: "GitHub".to_string(),
                transport_kind: McpTransportKind::Stdio,
                safe_config: json!({ "command": "npx", "args": [] }),
                secrets: McpSecretMaterial {
                    env: map_from_pairs([("GITHUB_TOKEN", "secret")]),
                    headers: BTreeMap::new(),
                    oauth_client_credentials: None,
                },
            },
            |_, _| FakeMcpTransport::auth_required("missing authorization"),
        )
        .await
        .expect("setup");

        assert_eq!(result.setup_status, McpSetupStatus::NeedsAuth);
        assert_eq!(
            result.setup_error.as_deref(),
            Some("This MCP server requires authentication before Noema can list tools.")
        );
        assert!(result.server.is_none());
        assert!(
            fixture
                .store
                .get_mcp_server("mcp:github")
                .await
                .expect("get server")
                .is_none()
        );
        assert!(
            !fixture
                .paths
                .mcp_server_home("mcp:github")
                .join("secrets.json")
                .exists()
        );
        assert!(!format!("{result:?}").contains("\"secret\""));
    }

    #[tokio::test]
    async fn continue_setup_updates_secrets_and_retries_discovery() {
        let fixture = TestFixture::new().await;
        let outcomes = Arc::new(Mutex::new(VecDeque::from([
            FakeMcpOutcome::AuthRequired("missing authorization".to_string()),
            FakeMcpOutcome::Ok(vec![fake_tool("retry_tool")]),
            FakeMcpOutcome::Ok(vec![fake_tool("retry_tool")]),
        ])));
        let create_outcomes = outcomes.clone();
        let initial = create_mcp_server_setup(
            &fixture.store,
            &fixture.paths,
            NewMcpServerSetup {
                display_name: "Remote".to_string(),
                transport_kind: McpTransportKind::StreamableHttp,
                safe_config: json!({ "url": "https://example.com/mcp" }),
                secrets: McpSecretMaterial::default(),
            },
            move |_, _| FakeMcpTransport::from_queue(create_outcomes.clone()),
        )
        .await
        .expect("initial");
        assert_eq!(initial.setup_status, McpSetupStatus::NeedsAuth);
        assert!(initial.server.is_none());

        let retry_create_outcomes = outcomes.clone();
        let result = create_mcp_server_setup(
            &fixture.store,
            &fixture.paths,
            NewMcpServerSetup {
                display_name: "Remote".to_string(),
                transport_kind: McpTransportKind::StreamableHttp,
                safe_config: json!({ "url": "https://example.com/mcp" }),
                secrets: McpSecretMaterial {
                    env: BTreeMap::new(),
                    headers: map_from_pairs([("Authorization", "Bearer retry")]),
                    oauth_client_credentials: None,
                },
            },
            move |_, _| FakeMcpTransport::from_queue(retry_create_outcomes.clone()),
        )
        .await
        .expect("retry create");

        assert_eq!(result.setup_status, McpSetupStatus::ReadyForCalibration);
        let server = result.server.as_ref().expect("persisted server");
        assert_eq!(server.mcp_server_id, "mcp:remote");
        assert_eq!(server.auth_status, McpServerAuthStatus::Authenticated);

        let retry_outcomes = outcomes.clone();
        let result = continue_mcp_server_setup(
            &fixture.store,
            &fixture.paths,
            ContinueMcpServerSetup {
                mcp_server_id: "mcp:remote".to_string(),
                secrets: McpSecretMaterial {
                    env: BTreeMap::new(),
                    headers: map_from_pairs([("Authorization", "Bearer retry")]),
                    oauth_client_credentials: None,
                },
            },
            move |_, _| FakeMcpTransport::from_queue(retry_outcomes.clone()),
        )
        .await
        .expect("retry");

        assert_eq!(result.setup_status, McpSetupStatus::ReadyForCalibration);
        let server = result.server.as_ref().expect("persisted server");
        assert_eq!(
            read_mcp_secrets(&fixture.paths.mcp_server_home("mcp:remote"))
                .expect("secrets")
                .headers
                .get("Authorization")
                .map(String::as_str),
            Some("Bearer retry")
        );
        assert!(!server.safe_config.to_string().contains("Bearer retry"));
    }

    #[tokio::test]
    async fn safe_config_rejects_secret_shaped_keys() {
        let fixture = TestFixture::new().await;
        let error = create_mcp_server_setup(
            &fixture.store,
            &fixture.paths,
            NewMcpServerSetup {
                display_name: "Unsafe".to_string(),
                transport_kind: McpTransportKind::StreamableHttp,
                safe_config: json!({
                    "url": "https://example.com/mcp",
                    "headers": { "Authorization": "not-safe" }
                }),
                secrets: McpSecretMaterial {
                    env: map_from_pairs([("GITHUB_TOKEN", "secret")]),
                    headers: BTreeMap::new(),
                    oauth_client_credentials: None,
                },
            },
            |_, _| FakeMcpTransport::ok(Vec::new()),
        )
        .await
        .expect_err("unsafe safe config should fail");

        assert!(error.to_string().contains("Authorization"));
    }

    struct TestFixture {
        paths: NoemaPaths,
        store: NoemaStore,
        _home: TempDir,
    }

    impl TestFixture {
        async fn new() -> Self {
            let home = TempDir::new().expect("temp noema home");
            let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
            let config = StoreConfig::from_paths(&paths);
            let store = NoemaStore::open(&config).await.expect("open store");
            Self {
                paths,
                store,
                _home: home,
            }
        }
    }

    #[derive(Clone)]
    struct FakeMcpTransport {
        outcome: FakeMcpOutcome,
    }

    #[derive(Clone)]
    enum FakeMcpOutcome {
        Ok(Vec<DiscoveredMcpTool>),
        AuthRequired(String),
    }

    impl FakeMcpTransport {
        fn ok(tools: Vec<DiscoveredMcpTool>) -> Self {
            Self {
                outcome: FakeMcpOutcome::Ok(tools),
            }
        }

        fn auth_required(message: &str) -> Self {
            Self {
                outcome: FakeMcpOutcome::AuthRequired(message.to_string()),
            }
        }

        fn from_queue(outcomes: Arc<Mutex<VecDeque<FakeMcpOutcome>>>) -> Self {
            let outcome = outcomes
                .lock()
                .expect("outcomes")
                .pop_front()
                .expect("fake outcome");
            Self { outcome }
        }
    }

    impl McpTransport for FakeMcpTransport {
        async fn initialize(&mut self) -> Result<(), McpClientError> {
            match &self.outcome {
                FakeMcpOutcome::Ok(_) => Ok(()),
                FakeMcpOutcome::AuthRequired(message) => {
                    Err(McpClientError::AuthRequired(message.clone()))
                }
            }
        }

        async fn list_tools(&mut self) -> Result<Vec<DiscoveredMcpTool>, McpClientError> {
            match &self.outcome {
                FakeMcpOutcome::Ok(tools) => Ok(tools.clone()),
                FakeMcpOutcome::AuthRequired(message) => {
                    Err(McpClientError::AuthRequired(message.clone()))
                }
            }
        }
    }

    fn fake_tool(name: &str) -> DiscoveredMcpTool {
        DiscoveredMcpTool {
            name: name.to_string(),
            description: Some(format!("{name} description")),
            input_schema: json!({"type": "object"}),
            output_schema: Some(json!({"type": "object"})),
            annotations: json!({"readOnlyHint": true}),
        }
    }

    fn map_from_pairs<const N: usize>(pairs: [(&str, &str); N]) -> BTreeMap<String, String> {
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }
}
