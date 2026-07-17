//! MCP setup validation shared by local control-plane orchestration.

use std::collections::{BTreeMap, BTreeSet};

use noema_capabilities::ToolName;
use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error;
use url::Url;

use crate::{
    CreateMcpServerCommand, McpDiscoveredTool, McpSecretMaterial, McpServerAuthStatus,
    McpServerRecord, McpSetupTransportConfig, NewMcpServer, discovered_tool_fingerprint,
    limits::{
        MAX_ANNOTATIONS_BYTES, MAX_DISCOVERED_TOOLS, MAX_SCHEMA_BYTES, MAX_TOOL_DESCRIPTION_BYTES,
        MAX_TOOL_NAME_BYTES, json_within_limits,
    },
};

/// Validated server and secret input ready for transport preparation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValidatedMcpServerSetup {
    pub(crate) server: NewMcpServer,
    pub(crate) secrets: McpSecretMaterial,
}

/// Internal setup validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum McpSetupValidationError {
    #[error("MCP server display name is invalid")]
    InvalidDisplayName,
    #[error("MCP transport configuration is invalid")]
    InvalidTransport,
    #[error("MCP safe configuration contains a secret-shaped key")]
    SecretInSafeConfiguration,
    #[error("MCP discovery returned duplicate tool names")]
    DuplicateToolName,
    #[error("MCP discovery returned an invalid tool contract")]
    InvalidToolContract,
    #[error("MCP connection identity could not be rotated")]
    IdentityGeneration,
}

pub(crate) fn validate_create_command(
    command: CreateMcpServerCommand,
) -> Result<ValidatedMcpServerSetup, McpSetupValidationError> {
    let display_name = command.display_name.trim();
    if display_name.is_empty() || display_name.chars().any(char::is_control) {
        return Err(McpSetupValidationError::InvalidDisplayName);
    }
    validate_transport(&command.transport)?;
    let mut secrets = command.secrets;
    let mut safe_config = safe_config_with_secret_refs(command.transport.safe_config(), &secrets)?;
    if secrets.has_secret_material() {
        safe_config = rotate_secret_identity_revision(safe_config, &mut secrets)?;
    }
    Ok(ValidatedMcpServerSetup {
        server: NewMcpServer {
            display_name: display_name.to_string(),
            transport_kind: command.transport.transport_kind(),
            safe_config,
        },
        secrets,
    })
}

pub(crate) fn validate_discovered_tools(
    mut tools: Vec<McpDiscoveredTool>,
) -> Result<Vec<McpDiscoveredTool>, McpSetupValidationError> {
    if tools.len() > MAX_DISCOVERED_TOOLS {
        return Err(McpSetupValidationError::InvalidToolContract);
    }
    let mut names = BTreeSet::new();
    for tool in &mut tools {
        if !names.insert(tool.name.as_str()) {
            return Err(McpSetupValidationError::DuplicateToolName);
        }
        if tool.name.len() > MAX_TOOL_NAME_BYTES
            || tool
                .description
                .as_ref()
                .is_some_and(|description| description.len() > MAX_TOOL_DESCRIPTION_BYTES)
            || ToolName::new(&tool.name).is_err()
            || tool
                .input_schema
                .get("type")
                .and_then(serde_json::Value::as_str)
                != Some("object")
            || tool.output_schema.as_ref().is_some_and(|schema| {
                schema.get("type").and_then(serde_json::Value::as_str) != Some("object")
            })
            || !json_within_limits(&tool.input_schema, MAX_SCHEMA_BYTES)
            || tool
                .output_schema
                .as_ref()
                .is_some_and(|schema| !json_within_limits(schema, MAX_SCHEMA_BYTES))
            || !json_within_limits(&tool.annotations, MAX_ANNOTATIONS_BYTES)
        {
            return Err(McpSetupValidationError::InvalidToolContract);
        }
        tool.metadata_fingerprint = discovered_tool_fingerprint(tool);
    }
    tools.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(tools)
}

pub(crate) fn merge_secret_material(
    mut current: McpSecretMaterial,
    replacement: McpSecretMaterial,
) -> McpSecretMaterial {
    current.env.extend(replacement.env);
    current.headers.extend(replacement.headers);
    if replacement.oauth_client_credentials.is_some() {
        current.oauth_client_credentials = replacement.oauth_client_credentials;
    }
    if replacement.oauth_credentials.is_some() {
        current.oauth_credentials = replacement.oauth_credentials;
    }
    current
}

pub(crate) fn safe_config_with_secret_refs(
    mut safe_config: serde_json::Value,
    secrets: &McpSecretMaterial,
) -> Result<serde_json::Value, McpSetupValidationError> {
    let object = safe_config
        .as_object_mut()
        .ok_or(McpSetupValidationError::InvalidTransport)?;
    let mut secret_refs = serde_json::Map::new();
    if !secrets.env.is_empty() {
        secret_refs.insert(
            "env".to_string(),
            serde_json::Value::Array(
                secrets
                    .env
                    .keys()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if !secrets.headers.is_empty() {
        secret_refs.insert(
            "headers".to_string(),
            serde_json::Value::Array(
                secrets
                    .headers
                    .keys()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if secrets.oauth_client_credentials.is_some() {
        secret_refs.insert(
            "oauth_client_credentials".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    if secrets.oauth_credentials.is_some() {
        secret_refs.insert(
            "oauth_credentials".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    if secret_refs.is_empty() {
        object.remove("secret_refs");
    } else {
        object.insert(
            "secret_refs".to_string(),
            serde_json::Value::Object(secret_refs),
        );
    }
    Ok(safe_config)
}

pub(crate) fn auth_status_for_secrets(secrets: &McpSecretMaterial) -> McpServerAuthStatus {
    if secrets.has_secret_material() {
        McpServerAuthStatus::Authenticated
    } else {
        McpServerAuthStatus::None
    }
}

pub(crate) fn secret_material_matches_server(
    server: &McpServerRecord,
    secrets: &McpSecretMaterial,
) -> bool {
    let expected_authenticated = server.auth_status == McpServerAuthStatus::Authenticated;
    if expected_authenticated != secrets.has_secret_material() {
        return false;
    }
    if !secret_identity_revision_matches(&server.safe_config, secrets) {
        return false;
    }

    let Some(secret_refs) = server.safe_config.get("secret_refs") else {
        return !expected_authenticated;
    };
    let Some(secret_refs) = secret_refs.as_object() else {
        return false;
    };
    if secret_refs.is_empty()
        || secret_refs.keys().any(|key| {
            !matches!(
                key.as_str(),
                "env" | "headers" | "oauth_client_credentials" | "oauth_credentials"
            )
        })
    {
        return false;
    }

    string_refs_match(secret_refs.get("env"), &secrets.env)
        && string_refs_match(secret_refs.get("headers"), &secrets.headers)
        && option_ref_matches(
            secret_refs.get("oauth_client_credentials"),
            secrets.oauth_client_credentials.is_some(),
        )
        && option_ref_matches(
            secret_refs.get("oauth_credentials"),
            secrets.oauth_credentials.is_some(),
        )
}

pub(crate) fn secret_identity_revision_matches(
    safe_config: &serde_json::Value,
    secrets: &McpSecretMaterial,
) -> bool {
    let safe_revision = safe_config
        .get("secret_identity_revision")
        .and_then(serde_json::Value::as_str);
    match (
        safe_revision,
        secrets.secret_identity_revision.as_deref(),
        secrets.has_secret_material(),
    ) {
        (Some(safe), Some(stored), true) => safe == stored,
        (None, None, false) => true,
        _ => false,
    }
}

fn string_refs_match(
    refs: Option<&serde_json::Value>,
    material: &BTreeMap<String, String>,
) -> bool {
    let Some(refs) = refs else {
        return material.is_empty();
    };
    let Some(refs) = refs.as_array() else {
        return false;
    };
    refs.len() == material.len()
        && refs.iter().all(|value| {
            value
                .as_str()
                .is_some_and(|key| material.get(key).is_some_and(|value| !value.is_empty()))
        })
}

fn option_ref_matches(reference: Option<&serde_json::Value>, material_present: bool) -> bool {
    match reference {
        Some(reference) => reference.as_bool() == Some(true) && material_present,
        None => !material_present,
    }
}

pub(crate) fn rotate_secret_identity_revision(
    mut safe_config: serde_json::Value,
    secrets: &mut McpSecretMaterial,
) -> Result<serde_json::Value, McpSetupValidationError> {
    let object = safe_config
        .as_object_mut()
        .ok_or(McpSetupValidationError::InvalidTransport)?;
    let mut bytes = [0_u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| McpSetupValidationError::IdentityGeneration)?;
    let mut revision = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(revision, "{byte:02x}");
    }
    object.insert(
        "secret_identity_revision".to_string(),
        serde_json::Value::String(revision.clone()),
    );
    secrets.secret_identity_revision = Some(revision);
    Ok(safe_config)
}

pub(crate) fn preview_server(
    server: &NewMcpServer,
    mcp_server_id: impl Into<String>,
) -> McpServerRecord {
    McpServerRecord {
        mcp_server_id: mcp_server_id.into(),
        display_name: server.display_name.clone(),
        transport_kind: server.transport_kind,
        safe_config: server.safe_config.clone(),
        enabled: false,
        health_status: crate::McpServerHealthStatus::Unknown,
        auth_status: McpServerAuthStatus::None,
        tool_count: 0,
        authority_generation: "pending-setup".to_string(),
    }
}

fn validate_transport(transport: &McpSetupTransportConfig) -> Result<(), McpSetupValidationError> {
    match transport {
        McpSetupTransportConfig::Stdio(config) => {
            if config.command.trim().is_empty()
                || config.command.chars().any(char::is_control)
                || config
                    .cwd
                    .as_deref()
                    .is_some_and(|cwd| cwd.trim().is_empty() || cwd.chars().any(char::is_control))
            {
                return Err(McpSetupValidationError::InvalidTransport);
            }
            reject_secret_shaped_keys(&config.env)
        }
        McpSetupTransportConfig::StreamableHttp(config) => {
            let url =
                Url::parse(&config.url).map_err(|_| McpSetupValidationError::InvalidTransport)?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
            {
                return Err(McpSetupValidationError::InvalidTransport);
            }
            reject_secret_shaped_keys(&config.headers)
        }
    }
}

fn reject_secret_shaped_keys(
    values: &BTreeMap<String, String>,
) -> Result<(), McpSetupValidationError> {
    if values.keys().any(|key| {
        let normalized = key.to_ascii_lowercase();
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
        .any(|marker| normalized.contains(marker))
    }) {
        return Err(McpSetupValidationError::SecretInSafeConfiguration);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{McpStdioSetupConfig, McpStreamableHttpSetupConfig, McpTransportKind};

    fn http_command() -> CreateMcpServerCommand {
        CreateMcpServerCommand {
            display_name: " Docs ".to_string(),
            transport: McpSetupTransportConfig::StreamableHttp(McpStreamableHttpSetupConfig {
                url: "https://example.com/mcp".to_string(),
                headers: BTreeMap::from([("X-Team".to_string(), "infra".to_string())]),
            }),
            secrets: McpSecretMaterial {
                headers: BTreeMap::from([(
                    "Authorization".to_string(),
                    "Bearer private".to_string(),
                )]),
                ..McpSecretMaterial::default()
            },
        }
    }

    #[test]
    fn setup_normalizes_safe_config_without_copying_secrets() {
        let setup = validate_create_command(http_command()).expect("setup");
        assert_eq!(setup.server.display_name, "Docs");
        assert_eq!(
            setup.server.transport_kind,
            McpTransportKind::StreamableHttp
        );
        assert_eq!(
            setup.server.safe_config["url"],
            json!("https://example.com/mcp")
        );
        assert_eq!(
            setup.server.safe_config["headers"],
            json!({"X-Team": "infra"})
        );
        assert_eq!(
            setup.server.safe_config["secret_refs"],
            json!({"headers": ["Authorization"]})
        );
        assert_eq!(
            setup.server.safe_config["secret_identity_revision"].as_str(),
            setup.secrets.secret_identity_revision.as_deref()
        );
        assert!(!setup.server.safe_config.to_string().contains("private"));
    }

    #[test]
    fn safe_configuration_rejects_secret_shaped_keys() {
        let mut command = http_command();
        command.transport = McpSetupTransportConfig::StreamableHttp(McpStreamableHttpSetupConfig {
            url: "https://example.com/mcp".to_string(),
            headers: BTreeMap::from([("Authorization".to_string(), "unsafe".to_string())]),
        });
        assert_eq!(
            validate_create_command(command).expect_err("secret-shaped"),
            McpSetupValidationError::SecretInSafeConfiguration
        );
    }

    #[test]
    fn transport_validation_rejects_unsafe_url_and_empty_command() {
        let mut command = http_command();
        command.transport = McpSetupTransportConfig::StreamableHttp(McpStreamableHttpSetupConfig {
            url: "https://user:password@example.com/mcp".to_string(),
            headers: BTreeMap::new(),
        });
        assert_eq!(
            validate_create_command(command).expect_err("userinfo"),
            McpSetupValidationError::InvalidTransport
        );

        let command = CreateMcpServerCommand {
            display_name: "Local".to_string(),
            transport: McpSetupTransportConfig::Stdio(McpStdioSetupConfig {
                command: " ".to_string(),
                args: Vec::new(),
                cwd: None,
                env: BTreeMap::new(),
            }),
            secrets: McpSecretMaterial::default(),
        };
        assert_eq!(
            validate_create_command(command).expect_err("command"),
            McpSetupValidationError::InvalidTransport
        );
    }

    #[test]
    fn discovery_rejects_duplicates_and_recomputes_fingerprints() {
        let tool = McpDiscoveredTool {
            name: "read".to_string(),
            description: Some("Read".to_string()),
            input_schema: json!({"type":"object"}),
            output_schema: Some(json!({"type":"object"})),
            annotations: json!({"readOnlyHint":true}),
            metadata_fingerprint: "untrusted".to_string(),
        };
        let validated = validate_discovered_tools(vec![tool.clone()]).expect("valid");
        assert!(
            validated[0]
                .metadata_fingerprint
                .starts_with("mcp-tool-metadata:v2:")
        );
        assert_eq!(
            validate_discovered_tools(vec![tool.clone(), tool]).expect_err("duplicate"),
            McpSetupValidationError::DuplicateToolName
        );
    }

    #[test]
    fn discovery_rejects_oversized_remote_metadata_before_persistence() {
        let tool = McpDiscoveredTool {
            name: "read".to_string(),
            description: Some("x".repeat(MAX_TOOL_DESCRIPTION_BYTES + 1)),
            input_schema: json!({"type":"object"}),
            output_schema: None,
            annotations: json!({}),
            metadata_fingerprint: String::new(),
        };
        assert_eq!(
            validate_discovered_tools(vec![tool]).expect_err("oversized description"),
            McpSetupValidationError::InvalidToolContract
        );

        let tool = McpDiscoveredTool {
            name: "read".to_string(),
            description: None,
            input_schema: json!({
                "type":"object",
                "description": "x".repeat(MAX_SCHEMA_BYTES)
            }),
            output_schema: None,
            annotations: json!({}),
            metadata_fingerprint: String::new(),
        };
        assert_eq!(
            validate_discovered_tools(vec![tool]).expect_err("oversized schema"),
            McpSetupValidationError::InvalidToolContract
        );
    }

    #[test]
    fn secret_identity_revision_is_random_and_non_secret() {
        let mut first_secrets = McpSecretMaterial {
            env: BTreeMap::from([("TOKEN".to_string(), "first".to_string())]),
            ..McpSecretMaterial::default()
        };
        let mut second_secrets = McpSecretMaterial {
            env: BTreeMap::from([("TOKEN".to_string(), "second".to_string())]),
            ..McpSecretMaterial::default()
        };
        let first = rotate_secret_identity_revision(
            json!({"url":"https://example.com"}),
            &mut first_secrets,
        )
        .expect("first");
        let second = rotate_secret_identity_revision(
            json!({"url":"https://example.com"}),
            &mut second_secrets,
        )
        .expect("second");
        let first_revision = first["secret_identity_revision"]
            .as_str()
            .expect("revision");
        assert_eq!(first_revision.len(), 32);
        assert_eq!(
            first_secrets.secret_identity_revision.as_deref(),
            Some(first_revision)
        );
        assert_ne!(
            first_revision,
            second["secret_identity_revision"]
                .as_str()
                .expect("revision")
        );
    }

    #[test]
    fn secret_identity_revision_rejects_old_material_with_the_same_key_set() {
        let mut old = McpSecretMaterial {
            env: BTreeMap::from([("TOKEN".to_string(), "old".to_string())]),
            ..McpSecretMaterial::default()
        };
        let old_config =
            rotate_secret_identity_revision(json!({}), &mut old).expect("old identity");
        let mut replacement = McpSecretMaterial {
            env: BTreeMap::from([("TOKEN".to_string(), "new".to_string())]),
            ..McpSecretMaterial::default()
        };
        let replacement_config =
            rotate_secret_identity_revision(old_config, &mut replacement).expect("replacement");

        assert!(secret_identity_revision_matches(
            &replacement_config,
            &replacement
        ));
        assert!(!secret_identity_revision_matches(&replacement_config, &old));
    }
}
