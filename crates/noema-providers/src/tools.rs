//! Provider tool transport and selection policy.

use std::{collections::HashMap, ops::Deref};

use noema_capabilities::{ToolName, ToolSpec};
use serde::{Deserialize, Serialize};

/// Tool-schema request mode for one provider interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSchemaRequest {
    /// Do not send a tool schema on this interface.
    DoNotSend,
    /// Send a schema without a strict request.
    Send,
    /// Request strict handling when full schema conversion succeeds.
    RequestStrictWhenPossible,
}

/// Tool-schema request support for provider-native tool arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderSchemaRequestCapabilities {
    /// Request mode for provider-native tool arguments.
    pub native_tool_arguments: ProviderSchemaRequest,
}

impl ProviderSchemaRequestCapabilities {
    /// Do not send provider-native tool schemas.
    #[must_use]
    pub const fn do_not_send() -> Self {
        Self {
            native_tool_arguments: ProviderSchemaRequest::DoNotSend,
        }
    }

    /// Request strict handling when full conversion succeeds.
    #[must_use]
    pub const fn request_strict_when_possible() -> Self {
        Self {
            native_tool_arguments: ProviderSchemaRequest::RequestStrictWhenPossible,
        }
    }
}

impl Default for ProviderSchemaRequestCapabilities {
    fn default() -> Self {
        Self::do_not_send()
    }
}

/// Provider transport used to expose the canonical tool catalog.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderToolTransport {
    /// This provider does not expose model-visible tools.
    #[default]
    None,
    /// Tools are exposed through the provider's native tool channel.
    Native,
}

/// Provider-native tool schema dialect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderToolSchemaDialect {
    /// No native schema dialect.
    #[default]
    None,
    /// OpenAI Responses API function tool schema dialect.
    OpenAiResponses,
    /// Apple Foundation Models local schema dialect.
    FoundationLocal,
}

/// Tool selection policy requested by Noema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoemaToolChoice {
    /// Let the provider choose whether to call a tool.
    #[default]
    Auto,
    /// Prevent tool calls for this request.
    None,
    /// Require at least one tool call.
    Required,
    /// Restrict the callable set without changing the provider tool catalog.
    Allowed(NoemaAllowedTools),
}

/// Whether a provider may decline to call one of the allowed tools.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoemaAllowedToolsMode {
    /// Let the provider choose whether to call an allowed tool.
    #[default]
    Auto,
    /// Require at least one call to an allowed tool.
    Required,
}

/// Provider-neutral restriction to a subset of the declared tool catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoemaAllowedTools {
    /// Whether a tool call is optional or required.
    pub mode: NoemaAllowedToolsMode,
    /// Canonical Noema tool names that remain callable.
    pub tools: Vec<ToolName>,
}

/// Provider/model tool-calling capabilities.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderToolCapabilities {
    /// Transport used to expose the request's canonical tool catalog.
    pub tool_transport: ProviderToolTransport,
    /// Whether independent parallel tool calls are supported.
    pub parallel_tool_calls: bool,
    /// Whether the provider can restrict calls to a subset of a stable catalog.
    pub allowed_tools: bool,
    /// Provider schema dialect for native tools.
    pub schema_dialect: ProviderToolSchemaDialect,
    /// Whether Noema can request strict schema handling after full conversion.
    pub request_strict_schema_when_possible: bool,
    /// Whether native tool-result messages are supported.
    pub native_tool_results: bool,
    /// Whether provider prompt-cache retention requests are supported.
    pub prompt_cache_retention: bool,
    /// Whether provider requests support a stable prompt cache key.
    pub prompt_cache_key: bool,
    /// Whether request-wide prompt-cache options are supported.
    pub prompt_cache_options: bool,
    /// Whether explicit prompt-cache breakpoints are supported on input content.
    pub prompt_cache_breakpoints: bool,
    /// Human-visible provider name when this provider/model supplies hosted
    /// web search and fetch instead of Noema's configured web tools.
    pub hosted_web_provider_name: Option<&'static str>,
}

/// One request-scoped tool identity shared by model context and provider wire
/// encoding. The exposed name is model-visible; the canonical specification
/// remains the dispatch authority.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderTool {
    exposed_name: String,
    canonical: ToolSpec,
}

impl ProviderTool {
    /// Expose a canonical tool under the same name.
    #[must_use]
    pub fn canonical(spec: ToolSpec) -> Self {
        Self {
            exposed_name: spec.name.as_str().to_string(),
            canonical: spec,
        }
    }

    /// Exact name supplied to the model and provider native channel.
    #[must_use]
    pub fn exposed_name(&self) -> &str {
        &self.exposed_name
    }

    /// Canonical specification used for validation and dispatch.
    #[must_use]
    pub fn canonical_spec(&self) -> &ToolSpec {
        &self.canonical
    }
}

impl From<ToolSpec> for ProviderTool {
    fn from(spec: ToolSpec) -> Self {
        Self::canonical(spec)
    }
}

impl Deref for ProviderTool {
    type Target = ToolSpec;

    fn deref(&self) -> &Self::Target {
        &self.canonical
    }
}

/// Build the immutable tool identities for one provider request surface.
#[must_use]
pub fn expose_provider_tools(
    specs: Vec<ToolSpec>,
    transport: ProviderToolTransport,
    dialect: ProviderToolSchemaDialect,
) -> Vec<ProviderTool> {
    if transport != ProviderToolTransport::Native
        || dialect != ProviderToolSchemaDialect::OpenAiResponses
    {
        return specs.into_iter().map(ProviderTool::canonical).collect();
    }

    let mut exposed_names = specs
        .iter()
        .map(|spec| provider_safe_tool_name(spec.name.as_str()))
        .collect::<Vec<_>>();
    let mut alias_groups = HashMap::<String, Vec<usize>>::new();
    for (index, name) in exposed_names.iter().enumerate() {
        alias_groups.entry(name.clone()).or_default().push(index);
    }
    for indexes in alias_groups.values().filter(|indexes| indexes.len() > 1) {
        let hashes = indexes
            .iter()
            .map(|index| format!("{:016x}", fnv1a64(specs[*index].name.as_str().as_bytes())))
            .collect::<Vec<_>>();
        let prefix_len = (1..=16)
            .find(|length| {
                hashes
                    .iter()
                    .map(|hash| &hash[..*length])
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == indexes.len()
            })
            .unwrap_or(16);
        for (index, hash) in indexes.iter().zip(hashes) {
            let mut alias = exposed_names[*index].clone();
            alias.truncate(OPENAI_FUNCTION_NAME_MAX - prefix_len - 1);
            exposed_names[*index] = format!("{alias}_{}", &hash[..prefix_len]);
        }
    }

    specs
        .into_iter()
        .zip(exposed_names)
        .map(|(canonical, exposed_name)| ProviderTool {
            exposed_name,
            canonical,
        })
        .collect()
}

pub(crate) fn provider_safe_tool_name(canonical: &str) -> String {
    let meaningful_name = canonical
        .rsplit_once('.')
        .map(|(_, name)| name)
        .filter(|name| !name.is_empty())
        .unwrap_or(canonical);
    let encoded = encode_provider_safe_name(meaningful_name);

    if encoded.len() <= OPENAI_FUNCTION_NAME_MAX {
        return encoded;
    }

    const HASH_SUFFIX_LEN: usize = 18;
    let mut readable_prefix = encoded;
    readable_prefix.truncate(OPENAI_FUNCTION_NAME_MAX - HASH_SUFFIX_LEN);
    format!("{readable_prefix}_h{:016x}", fnv1a64(canonical.as_bytes()))
}

const OPENAI_FUNCTION_NAME_MAX: usize = 64;

fn encode_provider_safe_name(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        let character = byte as char;
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            encoded.push(character);
        } else {
            encoded.push_str(&format!("_x{byte:02x}_"));
        }
    }
    encoded
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl ProviderToolCapabilities {
    /// Derive the native tool-schema request mode.
    #[must_use]
    pub fn schema_request_capabilities(self) -> ProviderSchemaRequestCapabilities {
        let native = if self.tool_transport == ProviderToolTransport::Native {
            if self.request_strict_schema_when_possible {
                ProviderSchemaRequest::RequestStrictWhenPossible
            } else {
                ProviderSchemaRequest::Send
            }
        } else {
            ProviderSchemaRequest::DoNotSend
        };
        ProviderSchemaRequestCapabilities {
            native_tool_arguments: native,
        }
    }
}
