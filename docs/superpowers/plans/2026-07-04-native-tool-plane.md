# Native Tool Plane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move executable Noema tool calls from assistant-authored JSON envelopes to a provider-neutral native tool plane, with OpenAI/Codex using provider-native tool calls first.

**Architecture:** Add canonical Noema tool specs and provider tool capabilities, then derive builtin and MCP tool exposure from those specs. OpenAI-compatible Responses adapters lower canonical specs into native `tools` request fields and parse native function-call output items back into `GenerateToolCall`, while non-native providers receive an explicit fallback mode that does not expose calibrated third-party MCP tools through the old envelope path.

**Tech Stack:** Rust, serde/serde_json, Noema provider contracts, OpenAI-compatible Responses API adapters, Capability Gateway, SurrealDB-backed runtime tests.

---

## Scope Check

This plan implements the native tool-plane portion of
`docs/superpowers/specs/2026-07-04-native-tool-plane-design.md`.

The replay cleanup section from the spec is intentionally excluded because it
is an independent GraphQL/web replay problem. Write a separate replay plan after
the native tool path is merged.

## File Structure

- Create: `crates/noema-core/src/provider/tools.rs`
  - Provider-neutral canonical tool specs, calls, results, capabilities, schema
    normalization, and tool-choice types.
- Modify: `crates/noema-core/src/provider.rs`
  - Re-export canonical tool types.
- Modify: `crates/noema-core/src/provider/contract.rs`
  - Add `GenerateRequest.tools`, `GenerateRequest.tool_choice`, and
    `GenerateRequest.parallel_tool_calls`.
  - Add `ModelProvider::tool_capabilities`.
  - Keep `GenerateResponse.tool_calls` as the provider-neutral parsed output
    slot.
- Modify: `crates/noema-core/src/daemon/memory/tool.rs`
  - Expose `search_memory_tool_spec`.
- Modify: `crates/noema-core/src/daemon/agent_name_tool.rs`
  - Expose `update_own_name_tool_spec`.
- Create: `crates/noema-core/src/daemon/runtime/model_tools.rs`
  - Build eligible canonical tools for a turn from builtin tools and calibrated
    MCP metadata.
  - Apply provider capability fallback policy.
- Modify: `crates/noema-core/src/daemon/runtime/mod.rs`
  - Add the private `model_tools` module.
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
  - Use `model_tools` when rendering prompts and building provider requests.
  - Pass canonical tool specs into initial and continuation provider requests.
  - Hide calibrated MCP tools from non-native providers.
- Modify: `crates/noema-core/src/daemon/runtime/handle.rs`
  - Mirror provider tool capabilities through the object-safe runtime provider
    trait used by daemon tests and provider maps.
- Modify: `crates/noema-core/src/daemon/prompts.rs`
  - Remove native-path instructions that tell the model to put executable tool
    calls in the Noema JSON envelope.
  - Keep legacy builtin-envelope instructions only when fallback mode says
    builtins may be exposed through the response object.
- Modify: `crates/noema-core/src/provider/adapters/responses.rs`
  - Serialize OpenAI-compatible native tools.
  - Parse native `function_call` output items.
  - Provide helper functions for combining assistant response text and native
    tool calls into `ParsedNoemaResponse`.
- Modify: `crates/noema-core/src/provider/adapters/openai.rs`
  - Declare native tool capabilities.
  - Send native tools in the Responses request.
  - Parse native tool calls into `GenerateResponse.tool_calls`.
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs`
  - Same as OpenAI, including retry body construction.
- Modify: `crates/noema-core/src/provider/adapters/foundation_local.rs`
  - Declare no native third-party tool support for now.
- Modify: `crates/noema-core/src/daemon/tests.rs`
  - Add runtime tests proving native-capable providers receive canonical tool
    specs and non-native providers do not receive calibrated MCP tools.
- Modify: `docs/context/current.md`
  - Record implementation status after the first native tool-plane slice lands.

## Task 1: Add Canonical Tool Contract

**Files:**
- Create: `crates/noema-core/src/provider/tools.rs`
- Modify: `crates/noema-core/src/provider.rs`
- Modify: `crates/noema-core/src/provider/contract.rs`

- [ ] **Step 1: Add failing provider-tool contract tests**

Add this test module to the bottom of the new
`crates/noema-core/src/provider/tools.rs` file before implementing the types:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_tool_spec_requires_object_input_schema() {
        let error = NoemaToolSpec::new(
            "search_memory",
            "Search governed Noema memory.",
            json!({"type": "string"}),
            NoemaToolExecution::LocalBuiltin,
        )
        .expect_err("non-object schema rejected");

        assert_eq!(
            error.to_string(),
            "tool search_memory input schema root must be an object"
        );
    }

    #[test]
    fn canonical_tool_spec_accepts_object_input_schema() {
        let spec = NoemaToolSpec::new(
            "search_memory",
            "Search governed Noema memory.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            NoemaToolExecution::LocalBuiltin,
        )
        .expect("object schema");

        assert_eq!(spec.name.as_str(), "search_memory");
        assert_eq!(spec.description, "Search governed Noema memory.");
        assert!(matches!(spec.execution, NoemaToolExecution::LocalBuiltin));
    }

    #[test]
    fn default_tool_capabilities_do_not_expose_native_tools() {
        let capabilities = ProviderToolCapabilities::default();

        assert!(!capabilities.native_tools);
        assert!(!capabilities.parallel_tool_calls);
        assert_eq!(capabilities.fallback_mode, ProviderToolFallbackMode::NoTools);
    }
}
```

Run:

```bash
cargo test -p noema-core provider::tools::tests -- --nocapture
```

Expected: compilation fails because `provider::tools` and its types do not
exist.

- [ ] **Step 2: Implement canonical tool types**

Create `crates/noema-core/src/provider/tools.rs` with:

```rust
//! Provider-neutral tool contracts.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolName(String);

impl ToolName {
    pub fn new(value: impl Into<String>) -> Result<Self, ToolContractError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(ToolContractError::InvalidToolName(
                "tool name cannot be empty".to_string(),
            ));
        }
        if trimmed != value {
            return Err(ToolContractError::InvalidToolName(format!(
                "tool name cannot contain leading or trailing whitespace: {value:?}"
            )));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ToolName {
    type Err = ToolContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoemaToolSpec {
    pub name: ToolName,
    pub description: String,
    pub input_schema: NoemaToolSchema,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<NoemaToolSchema>,
    pub execution: NoemaToolExecution,
    pub exposure: ToolExposurePolicy,
}

impl NoemaToolSpec {
    pub fn new(
        name: impl AsRef<str>,
        description: impl Into<String>,
        input_schema: Value,
        execution: NoemaToolExecution,
    ) -> Result<Self, ToolContractError> {
        let name = ToolName::new(name.as_ref())?;
        let input_schema = NoemaToolSchema::new(name.as_str(), input_schema)?;
        let description = description.into().trim().to_string();
        if description.is_empty() {
            return Err(ToolContractError::InvalidDescription(format!(
                "tool {name} description cannot be empty"
            )));
        }
        Ok(Self {
            name,
            description,
            input_schema,
            output_schema: None,
            execution,
            exposure: ToolExposurePolicy::default(),
        })
    }

    #[must_use]
    pub fn with_output_schema(mut self, schema: Value) -> Result<Self, ToolContractError> {
        self.output_schema = Some(NoemaToolSchema::new(self.name.as_str(), schema)?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoemaToolSchema {
    value: Value,
}

impl NoemaToolSchema {
    pub fn new(tool_name: &str, value: Value) -> Result<Self, ToolContractError> {
        if value.get("type").and_then(Value::as_str) != Some("object") {
            return Err(ToolContractError::InvalidSchema(format!(
                "tool {tool_name} input schema root must be an object"
            )));
        }
        Ok(Self { value })
    }

    #[must_use]
    pub fn as_value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoemaToolExecution {
    LocalBuiltin,
    Mcp {
        server_id: String,
        tool_name: String,
        tool_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExposurePolicy {
    pub fallback_mode: ProviderToolFallbackMode,
}

impl Default for ToolExposurePolicy {
    fn default() -> Self {
        Self {
            fallback_mode: ProviderToolFallbackMode::NativeRequired,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderToolFallbackMode {
    NoTools,
    BuiltinOnlyEnvelope,
    LegacyEnvelope,
    NativeRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderToolSchemaDialect {
    None,
    OpenAiResponses,
    Anthropic,
    FoundationLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoemaToolChoice {
    Auto,
    None,
    Required,
}

impl Default for NoemaToolChoice {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderToolCapabilities {
    pub native_tools: bool,
    pub parallel_tool_calls: bool,
    pub tool_choice: bool,
    pub schema_dialect: ProviderToolSchemaDialect,
    pub strict_schema: bool,
    pub custom_tools: bool,
    pub native_tool_results: bool,
    pub fallback_mode: ProviderToolFallbackMode,
}

impl Default for ProviderToolCapabilities {
    fn default() -> Self {
        Self {
            native_tools: false,
            parallel_tool_calls: false,
            tool_choice: false,
            schema_dialect: ProviderToolSchemaDialect::None,
            strict_schema: false,
            custom_tools: false,
            native_tool_results: false,
            fallback_mode: ProviderToolFallbackMode::NoTools,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoemaToolCall {
    pub id: Option<String>,
    pub name: ToolName,
    pub arguments: Value,
    pub provider_call_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoemaToolResult {
    pub call_id: Option<String>,
    pub name: ToolName,
    pub success: bool,
    pub output: Value,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolContractError {
    #[error("{0}")]
    InvalidToolName(String),
    #[error("{0}")]
    InvalidDescription(String),
    #[error("{0}")]
    InvalidSchema(String),
}
```

Keep the test module from Step 1 at the bottom of this file.

- [ ] **Step 3: Export the new module and request fields**

Modify `crates/noema-core/src/provider.rs`:

```rust
/// Provider-neutral native tool contracts.
pub mod tools;

pub use tools::{
    NoemaToolCall, NoemaToolChoice, NoemaToolExecution, NoemaToolResult, NoemaToolSchema,
    NoemaToolSpec, ProviderToolCapabilities, ProviderToolFallbackMode,
    ProviderToolSchemaDialect, ToolContractError, ToolExposurePolicy, ToolName,
};
```

Modify `crates/noema-core/src/provider/contract.rs` imports:

```rust
use super::tools::{NoemaToolChoice, NoemaToolSpec, ProviderToolCapabilities};
use crate::memory::extraction::ExtractorMemoryProposal;
```

Add this method to the `ModelProvider` trait:

```rust
/// Return native tool-calling capabilities for this provider/model.
fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
    ProviderToolCapabilities::default()
}
```

Add these fields to `GenerateRequest` after `options`:

```rust
/// Provider-neutral model-visible tools for this request.
pub tools: Vec<NoemaToolSpec>,
/// Tool selection policy requested by Noema.
pub tool_choice: NoemaToolChoice,
/// Whether Noema allows the provider to emit independent tool calls in parallel.
pub parallel_tool_calls: bool,
```

Update `GenerateRequest::text`:

```rust
Self {
    conversation_id: None,
    model: None,
    input: GenerateInput::Text(input.into()),
    instructions: None,
    options: GenerateOptions::default(),
    tools: Vec::new(),
    tool_choice: NoemaToolChoice::Auto,
    parallel_tool_calls: false,
}
```

Update every `GenerateRequest { ... }` literal by adding:

```rust
tools: Vec::new(),
tool_choice: crate::provider::NoemaToolChoice::Auto,
parallel_tool_calls: false,
```

- [ ] **Step 4: Run focused contract tests**

Run:

```bash
cargo test -p noema-core provider::tools::tests provider::contract::tests -- --nocapture
```

Expected: all provider tool and provider contract tests pass.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/provider.rs crates/noema-core/src/provider/contract.rs crates/noema-core/src/provider/tools.rs
git commit -m "Add provider-neutral tool contract"
```

## Task 2: Add Builtin Tool Specs

**Files:**
- Modify: `crates/noema-core/src/daemon/memory/tool.rs`
- Modify: `crates/noema-core/src/daemon/agent_name_tool.rs`

- [ ] **Step 1: Add failing builtin spec tests**

Add to `crates/noema-core/src/daemon/memory/tool.rs` tests:

```rust
#[test]
fn search_memory_tool_spec_matches_runtime_arguments() {
    let spec = search_memory_tool_spec().expect("tool spec");

    assert_eq!(spec.name.as_str(), SEARCH_MEMORY_TOOL);
    assert!(spec.description.contains("Search governed Noema memory"));
    assert_eq!(
        spec.input_schema.as_value()["required"],
        json!(["query"])
    );
    assert_eq!(
        spec.input_schema.as_value()["properties"]["scope_ids"]["items"]["type"],
        "string"
    );
    assert_eq!(
        spec.input_schema.as_value()["properties"]["purpose"]["enum"],
        json!([
            "answer_human_question",
            "draft_internal_content",
            "general_personalization",
            "manage_task",
            "manage_calendar",
            "draft_external_content",
            "use_tool",
            "proactive_suggestion",
            "external_action",
            "debug_audit"
        ])
    );
}
```

Add to `crates/noema-core/src/daemon/agent_name_tool.rs` tests:

```rust
#[test]
fn update_own_name_tool_spec_matches_runtime_arguments() {
    let spec = update_own_name_tool_spec().expect("tool spec");

    assert_eq!(spec.name.as_str(), UPDATE_OWN_NAME_TOOL);
    assert!(spec.description.contains("name or rename"));
    assert_eq!(spec.input_schema.as_value()["required"], json!(["name"]));
    assert_eq!(
        spec.input_schema.as_value()["properties"]["name"]["maxLength"],
        MAX_AGENT_NAME_CHARS
    );
}
```

Run:

```bash
cargo test -p noema-core daemon::memory::tool::tests::search_memory_tool_spec_matches_runtime_arguments daemon::agent_name_tool::tests::update_own_name_tool_spec_matches_runtime_arguments -- --nocapture
```

Expected: compilation fails because the spec functions do not exist.

- [ ] **Step 2: Implement `search_memory_tool_spec`**

Add these imports to `crates/noema-core/src/daemon/memory/tool.rs`:

```rust
use crate::provider::{NoemaToolExecution, NoemaToolSpec, ToolContractError};
```

Add this function near `is_search_memory_tool`:

```rust
pub(in crate::daemon) fn search_memory_tool_spec() -> Result<NoemaToolSpec, ToolContractError> {
    NoemaToolSpec::new(
        SEARCH_MEMORY_TOOL,
        "Search governed Noema memory for the current user, conversation, or active project.",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search text. May be empty only when scope_ids is non-empty."
                },
                "scope_ids": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Concrete active memory scopes such as human:local or conversation:<id>."
                },
                "purpose": {
                    "type": "string",
                    "enum": [
                        "answer_human_question",
                        "draft_internal_content",
                        "general_personalization",
                        "manage_task",
                        "manage_calendar",
                        "draft_external_content",
                        "use_tool",
                        "proactive_suggestion",
                        "external_action",
                        "debug_audit"
                    ],
                    "description": "Policy purpose for retrieval."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 16,
                    "description": "Maximum number of memory facts to return."
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        NoemaToolExecution::LocalBuiltin,
    )
}
```

- [ ] **Step 3: Implement `update_own_name_tool_spec`**

Add this import to `crates/noema-core/src/daemon/agent_name_tool.rs`:

```rust
use crate::provider::{NoemaToolExecution, NoemaToolSpec, ToolContractError};
```

Add this function near `is_update_own_name_tool`:

```rust
pub(super) fn update_own_name_tool_spec() -> Result<NoemaToolSpec, ToolContractError> {
    NoemaToolSpec::new(
        UPDATE_OWN_NAME_TOOL,
        "Persist the primary agent display name when the current user explicitly asks to name or rename the agent.",
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": MAX_AGENT_NAME_CHARS,
                    "description": "The agent display name requested by the current user."
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
        NoemaToolExecution::LocalBuiltin,
    )
}
```

- [ ] **Step 4: Run builtin spec tests**

Run:

```bash
cargo test -p noema-core daemon::memory::tool::tests::search_memory_tool_spec_matches_runtime_arguments daemon::agent_name_tool::tests::update_own_name_tool_spec_matches_runtime_arguments -- --nocapture
```

Expected: both tests pass.

- [ ] **Step 5: Commit Task 2**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/daemon/memory/tool.rs crates/noema-core/src/daemon/agent_name_tool.rs
git commit -m "Add builtin native tool specs"
```

## Task 3: Build Runtime Tool Exposure

**Files:**
- Create: `crates/noema-core/src/daemon/runtime/model_tools.rs`
- Modify: `crates/noema-core/src/daemon/runtime/mod.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`

- [ ] **Step 1: Add failing model-tool tests**

Create `crates/noema-core/src/daemon/runtime/model_tools.rs` with only the
test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        McpCalibrationStatus, McpServerAuthStatus, McpServerHealthStatus, McpTransportKind,
        McpTrustClassification, NewMcpServer, NewMcpTool, NewToolCalibration,
        provider::{ProviderToolCapabilities, ProviderToolFallbackMode, ProviderToolSchemaDialect},
    };
    use serde_json::json;

    #[tokio::test]
    async fn native_provider_gets_builtin_and_calibrated_mcp_tools() {
        let (_home, store) = crate::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        seed_ready_mcp_tool(&store).await;

        let tools = build_model_tools(
            &store,
            true,
            ProviderToolCapabilities {
                native_tools: true,
                parallel_tool_calls: true,
                tool_choice: true,
                schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                strict_schema: false,
                custom_tools: false,
                native_tool_results: true,
                fallback_mode: ProviderToolFallbackMode::NativeRequired,
            },
        )
        .await
        .expect("tools");

        let names = tools.native.iter().map(|tool| tool.name.as_str()).collect::<Vec<_>>();
        assert_eq!(names, vec!["search_memory", "update_own_name", "mcp.docs.read"]);
        assert!(tools.legacy_builtin_envelope_tools.is_empty());
    }

    #[tokio::test]
    async fn non_native_provider_gets_only_builtin_envelope_fallback() {
        let (_home, store) = crate::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        seed_ready_mcp_tool(&store).await;

        let tools = build_model_tools(
            &store,
            true,
            ProviderToolCapabilities {
                fallback_mode: ProviderToolFallbackMode::BuiltinOnlyEnvelope,
                ..ProviderToolCapabilities::default()
            },
        )
        .await
        .expect("tools");

        assert!(tools.native.is_empty());
        assert_eq!(
            tools.legacy_builtin_envelope_tools,
            vec!["search_memory".to_string(), "update_own_name".to_string()]
        );
        assert!(tools.unavailable_rows.iter().all(|row| !row.contains("mcp.docs.read")));
    }

    async fn seed_ready_mcp_tool(store: &crate::NoemaStore) {
        let server = store
            .create_mcp_server(NewMcpServer {
                mcp_server_id: "docs".to_string(),
                display_name: "Docs".to_string(),
                transport_kind: McpTransportKind::Stdio,
                safe_config: json!({}),
            })
            .await
            .expect("server");
        store
            .update_mcp_server_runtime_state(
                &server.mcp_server_id,
                true,
                McpServerHealthStatus::Healthy,
                McpServerAuthStatus::Authenticated,
            )
            .await
            .expect("server state");
        store
            .upsert_discovered_mcp_tool(NewMcpTool {
                mcp_tool_id: "docs:read".to_string(),
                mcp_server_id: "docs".to_string(),
                name: "read".to_string(),
                description: Some("Read a document.".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {"document_id": {"type": "string"}},
                    "required": ["document_id"],
                    "additionalProperties": false
                }),
                output_schema: None,
                annotations: json!({}),
                metadata_fingerprint: "fp1".to_string(),
            })
            .await
            .expect("tool");
        store
            .save_tool_calibration(NewToolCalibration {
                calibration_id: "cal_docs_read".to_string(),
                mcp_tool_id: "docs:read".to_string(),
                read_classification: McpTrustClassification::Trusted,
                write_classification: McpTrustClassification::None,
                export_classification: McpTrustClassification::None,
                owner_extractors: Vec::new(),
                status: McpCalibrationStatus::Ready,
                reviewed_by: Some("human:local".to_string()),
                reviewed_metadata_fingerprint: Some("fp1".to_string()),
            })
            .await
            .expect("calibration");
    }
}
```

Run:

```bash
cargo test -p noema-core daemon::runtime::model_tools::tests -- --nocapture
```

Expected: compilation fails because `build_model_tools` and `ModelTools` do not
exist.

- [ ] **Step 2: Implement `model_tools`**

Replace the top of `crates/noema-core/src/daemon/runtime/model_tools.rs` with:

```rust
use crate::{
    McpServerAuthStatus, McpServerHealthStatus, NoemaStore,
    daemon::{
        agent_name_tool::update_own_name_tool_spec,
        memory::tool::search_memory_tool_spec,
        runtime::turn::{mcp_auth_status_label, mcp_health_status_label},
    },
    mcp::mcp_tool_ineligibility,
    provider::{
        NoemaToolExecution, NoemaToolSpec, ProviderToolCapabilities, ProviderToolFallbackMode,
        ToolContractError,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ModelTools {
    pub(super) native: Vec<NoemaToolSpec>,
    pub(super) legacy_builtin_envelope_tools: Vec<String>,
    pub(super) prompt_rows: Vec<String>,
    pub(super) unavailable_rows: Vec<String>,
}

pub(super) async fn build_model_tools(
    store: &NoemaStore,
    include_agent_name_tool: bool,
    capabilities: ProviderToolCapabilities,
) -> Result<ModelTools, ToolContractError> {
    let builtin_specs = builtin_tool_specs(include_agent_name_tool)?;
    if capabilities.native_tools {
        let mut native = builtin_specs;
        native.extend(calibrated_mcp_tool_specs(store).await?);
        let prompt_rows = native
            .iter()
            .map(|tool| format!("- native\t{}\t{}", tool.name.as_str(), tool.description))
            .collect::<Vec<_>>();
        return Ok(ModelTools {
            native,
            legacy_builtin_envelope_tools: Vec::new(),
            prompt_rows,
            unavailable_rows: unavailable_mcp_rows(store).await,
        });
    }

    let legacy_builtin_envelope_tools =
        if capabilities.fallback_mode == ProviderToolFallbackMode::BuiltinOnlyEnvelope {
            builtin_specs
                .iter()
                .map(|tool| tool.name.as_str().to_string())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
    let prompt_rows = legacy_builtin_envelope_tools
        .iter()
        .map(|name| format!("- builtin\t{name}\tNoema built-in tool"))
        .collect::<Vec<_>>();

    Ok(ModelTools {
        native: Vec::new(),
        legacy_builtin_envelope_tools,
        prompt_rows,
        unavailable_rows: unavailable_mcp_rows(store).await,
    })
}

fn builtin_tool_specs(include_agent_name_tool: bool) -> Result<Vec<NoemaToolSpec>, ToolContractError> {
    let mut specs = vec![search_memory_tool_spec()?];
    if include_agent_name_tool {
        specs.push(update_own_name_tool_spec()?);
    }
    Ok(specs)
}

async fn calibrated_mcp_tool_specs(
    store: &NoemaStore,
) -> Result<Vec<NoemaToolSpec>, ToolContractError> {
    let mut specs = Vec::new();
    for server in store.list_mcp_servers().await.map_err(store_schema_error)? {
        let tools = store
            .list_mcp_tools_for_server(&server.mcp_server_id)
            .await
            .map_err(store_schema_error)?;
        for tool in tools {
            let calibration = store
                .get_tool_calibration(&tool.mcp_tool_id)
                .await
                .map_err(store_schema_error)?;
            if mcp_tool_ineligibility(&server, &tool, calibration.as_ref()).is_some() {
                continue;
            }
            let name = format!("mcp.{}.{}", server.mcp_server_id, tool.name);
            let description = tool
                .description
                .clone()
                .filter(|description| !description.trim().is_empty())
                .unwrap_or_else(|| format!("Call MCP tool {} on {}", tool.name, server.display_name));
            specs.push(NoemaToolSpec::new(
                &name,
                description,
                tool.input_schema.clone(),
                NoemaToolExecution::Mcp {
                    server_id: server.mcp_server_id.clone(),
                    tool_name: tool.name.clone(),
                    tool_id: tool.mcp_tool_id.clone(),
                },
            )?);
        }
    }
    Ok(specs)
}

async fn unavailable_mcp_rows(store: &NoemaStore) -> Vec<String> {
    let Ok(servers) = store.list_mcp_servers().await else {
        return Vec::new();
    };
    servers
        .into_iter()
        .filter(|server| {
            server.enabled
                && (server.health_status != McpServerHealthStatus::Healthy
                    || !matches!(
                        server.auth_status,
                        McpServerAuthStatus::None | McpServerAuthStatus::Authenticated
                    ))
        })
        .map(|server| {
            format!(
                "- unavailable_mcp\t{}\t{}\thealth={}\tauth={}",
                server.mcp_server_id,
                server.display_name,
                mcp_health_status_label(server.health_status),
                mcp_auth_status_label(server.auth_status),
            )
        })
        .collect()
}

fn store_schema_error(error: crate::StoreError) -> ToolContractError {
    ToolContractError::InvalidSchema(format!("failed to read tool metadata: {error}"))
}
```

Keep the test module below this implementation.

- [ ] **Step 3: Make helper labels visible inside runtime**

In `crates/noema-core/src/daemon/runtime/turn.rs`, change:

```rust
fn mcp_health_status_label(status: crate::McpServerHealthStatus) -> &'static str {
```

to:

```rust
pub(super) fn mcp_health_status_label(status: crate::McpServerHealthStatus) -> &'static str {
```

Change:

```rust
fn mcp_auth_status_label(status: crate::McpServerAuthStatus) -> &'static str {
```

to:

```rust
pub(super) fn mcp_auth_status_label(status: crate::McpServerAuthStatus) -> &'static str {
```

In `crates/noema-core/src/daemon/runtime/mod.rs`, add:

```rust
mod model_tools;
```

- [ ] **Step 4: Run model-tool tests**

Run:

```bash
cargo test -p noema-core daemon::runtime::model_tools::tests -- --nocapture
```

Expected: tests pass.

- [ ] **Step 5: Commit Task 3**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/daemon/runtime/model_tools.rs crates/noema-core/src/daemon/runtime/mod.rs crates/noema-core/src/daemon/runtime/turn.rs
git commit -m "Build model-visible native tools"
```

## Task 4: Serialize And Parse OpenAI-Compatible Native Tools

**Files:**
- Modify: `crates/noema-core/src/provider/adapters/responses.rs`

- [ ] **Step 1: Add failing Responses adapter tests**

Add these tests to the `#[cfg(test)]` module in
`crates/noema-core/src/provider/adapters/responses.rs`:

```rust
#[test]
fn responses_request_serializes_native_tools() {
    let body = ResponsesRequest {
        model: "gpt-test".to_string(),
        input: ResponsesInput::Text("hi".to_string()),
        instructions: None,
        max_output_tokens: None,
        temperature: None,
        text: None,
        tools: vec![ResponsesTool::function(
            "search_memory",
            "Search governed Noema memory.",
            serde_json::json!({
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"],
                "additionalProperties": false
            }),
        )],
        tool_choice: Some("auto"),
        parallel_tool_calls: Some(false),
        store: false,
        prompt_cache_retention: None,
    };

    let value = serde_json::to_value(body).expect("serialize");

    assert_eq!(value["tools"][0]["type"], "function");
    assert_eq!(value["tools"][0]["name"], "search_memory");
    assert_eq!(value["tools"][0]["parameters"]["required"], serde_json::json!(["query"]));
    assert_eq!(value["tool_choice"], "auto");
    assert_eq!(value["parallel_tool_calls"], false);
}

#[test]
fn responses_response_parses_function_call_output_items() {
    let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
        "id": "resp_1",
        "model": "gpt-test",
        "output": [
            {
                "type": "function_call",
                "id": "item_1",
                "call_id": "call_1",
                "name": "search_memory",
                "arguments": "{\"query\":\"trains\",\"scope_ids\":[\"human:local\"]}"
            }
        ]
    }))
    .expect("response");

    let calls = response.native_tool_calls().expect("tool calls");

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id.as_deref(), Some("call_1"));
    assert_eq!(calls[0].name, "search_memory");
    assert_eq!(calls[0].payload["query"], "trains");
    assert_eq!(calls[0].payload["scope_ids"], serde_json::json!(["human:local"]));
}
```

Run:

```bash
cargo test -p noema-core provider::adapters::responses::tests::responses_request_serializes_native_tools provider::adapters::responses::tests::responses_response_parses_function_call_output_items -- --nocapture
```

Expected: compilation fails because `tools`, `tool_choice`,
`parallel_tool_calls`, `ResponsesTool`, and `native_tool_calls` do not exist.

- [ ] **Step 2: Add request serialization fields and tool type**

In `ResponsesRequest`, add before `store`:

```rust
/// Native Responses API tool definitions.
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub tools: Vec<ResponsesTool>,
/// Responses API tool-choice policy.
#[serde(skip_serializing_if = "Option::is_none")]
pub tool_choice: Option<&'static str>,
/// Whether parallel independent tool calls are allowed.
#[serde(skip_serializing_if = "Option::is_none")]
pub parallel_tool_calls: Option<bool>,
```

Add below `ResponsesRequest`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesTool {
    #[serde(rename = "type")]
    kind: &'static str,
    name: String,
    description: String,
    parameters: Value,
}

impl ResponsesTool {
    #[must_use]
    pub fn function(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            kind: "function",
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}
```

- [ ] **Step 3: Parse native function-call items**

Change `ResponsesOutputItem` to:

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message { content: Vec<ResponsesContent> },
    #[serde(rename = "function_call")]
    FunctionCall {
        id: Option<String>,
        call_id: Option<String>,
        name: String,
        arguments: String,
    },
    #[serde(other)]
    Other,
}
```

Add this method to `impl ResponsesResponse`:

```rust
pub fn native_tool_calls(&self) -> Result<Vec<crate::provider::GenerateToolCall>, ProviderError> {
    let mut calls = Vec::new();
    for item in &self.output {
        let ResponsesOutputItem::FunctionCall {
            id,
            call_id,
            name,
            arguments,
        } = item
        else {
            continue;
        };
        let payload = serde_json::from_str(arguments).map_err(|source| {
            ProviderError::MalformedResponse {
                message: format!("failed to parse native tool call arguments for {name}: {source}"),
            }
        })?;
        calls.push(crate::provider::GenerateToolCall {
            id: call_id.clone().or_else(|| id.clone()),
            name: name.clone(),
            payload,
        });
    }
    Ok(calls)
}
```

- [ ] **Step 4: Run Responses adapter tests**

Run:

```bash
cargo test -p noema-core provider::adapters::responses::tests::responses_request_serializes_native_tools provider::adapters::responses::tests::responses_response_parses_function_call_output_items -- --nocapture
```

Expected: tests pass.

- [ ] **Step 5: Commit Task 4**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/provider/adapters/responses.rs
git commit -m "Support OpenAI-compatible native tool payloads"
```

## Task 5: Wire OpenAI And Codex Native Tools

**Files:**
- Modify: `crates/noema-core/src/provider/adapters/openai.rs`
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs`
- Modify: `crates/noema-core/src/provider/adapters/foundation_local.rs`
- Modify: `crates/noema-core/src/daemon/runtime/handle.rs`

- [ ] **Step 1: Add failing adapter tests**

In `crates/noema-core/src/provider/adapters/openai.rs`, add a test that sends a
native tool and asserts the request contains it:

```rust
#[tokio::test]
async fn openai_request_sends_native_tool_specs() {
    let (base_url, request_rx) = spawn_server(
        200,
        r#"{
          "id": "resp_test",
          "model": "gpt-test",
          "output": [
            {
              "type": "message",
              "content": [
                {"type": "output_text", "text": "{\"response_status\":\"final\",\"responses\":[{\"kind\":\"text\",\"phase\":\"final_answer\",\"text\":\"Done\"}],\"memory_proposals\":[]}"}
              ]
            }
          ]
        }"#,
    )
    .await;
    let provider = test_provider(base_url);

    let response = provider
        .generate(GenerateRequest {
            conversation_id: None,
            model: Some("gpt-test".to_string()),
            input: GenerateInput::Text("Hi".to_string()),
            instructions: None,
            options: crate::provider::GenerateOptions {
                require_noema_response: true,
                ..crate::provider::GenerateOptions::default()
            },
            tools: vec![crate::daemon::memory::tool::search_memory_tool_spec().expect("tool")],
            tool_choice: crate::provider::NoemaToolChoice::Auto,
            parallel_tool_calls: false,
        })
        .await
        .expect("response");

    let captured = request_rx.await.expect("captured request");
    let body: Value = serde_json::from_str(&captured.body).expect("json body");
    assert_eq!(body["tools"][0]["name"], "search_memory");
    assert!(response.tool_calls.is_empty());
}
```

In `crates/noema-core/src/provider/adapters/codex_responses.rs`, add a matching
test using the existing `spawn_server`, `sse_delta`, `sse_completed`, and
`provider_with_tokens` helpers:

```rust
#[tokio::test]
async fn codex_request_sends_native_tool_specs() {
    let response_body = format!(
        "{}{}{}",
        sse_delta(r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Done"}],"memory_proposals":[]}"#),
        sse_completed(),
        ""
    );
    let (base_url, request_rx) = spawn_server(
        200,
        &response_body,
    )
    .await;
    let (provider, _home) = provider_with_tokens(base_url);

    provider
        .generate(GenerateRequest {
            conversation_id: None,
            model: Some("gpt-test".to_string()),
            input: GenerateInput::Text("Hi".to_string()),
            instructions: None,
            options: crate::provider::GenerateOptions {
                require_noema_response: true,
                ..crate::provider::GenerateOptions::default()
            },
            tools: vec![crate::daemon::memory::tool::search_memory_tool_spec().expect("tool")],
            tool_choice: crate::provider::NoemaToolChoice::Auto,
            parallel_tool_calls: false,
        })
        .await
        .expect("response");

    let captured = request_rx.await.expect("captured request");
    let body: Value = serde_json::from_str(&captured.body).expect("json body");
    assert_eq!(body["tools"][0]["name"], "search_memory");
}
```

Run the two focused tests. Expected: compilation fails because the adapters do
not lower `GenerateRequest.tools`.

- [ ] **Step 2: Declare provider capabilities**

In `impl ModelProvider for OpenAiProvider`, add:

```rust
fn tool_capabilities(&self, _model: Option<&str>) -> crate::provider::ProviderToolCapabilities {
    crate::provider::ProviderToolCapabilities {
        native_tools: true,
        parallel_tool_calls: true,
        tool_choice: true,
        schema_dialect: crate::provider::ProviderToolSchemaDialect::OpenAiResponses,
        strict_schema: false,
        custom_tools: false,
        native_tool_results: true,
        fallback_mode: crate::provider::ProviderToolFallbackMode::NativeRequired,
    }
}
```

Add the same method to `impl ModelProvider for CodexResponsesProvider`.

In `impl ModelProvider for FoundationLocalProvider`, add:

```rust
fn tool_capabilities(&self, _model: Option<&str>) -> crate::provider::ProviderToolCapabilities {
    crate::provider::ProviderToolCapabilities {
        fallback_mode: crate::provider::ProviderToolFallbackMode::BuiltinOnlyEnvelope,
        ..crate::provider::ProviderToolCapabilities::default()
    }
}
```

In `crates/noema-core/src/daemon/runtime/handle.rs`, import
`ProviderToolCapabilities`:

```rust
use crate::provider::{
    GenerateRequest, GenerateResponse, GenerateStreamEvent, ModelProvider,
    ProviderContextMetadata, ProviderError, ProviderToolCapabilities,
};
```

Add this default method to `RuntimeModelProvider`:

```rust
fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
    ProviderToolCapabilities::default()
}
```

Add this method to the blanket `impl<T> RuntimeModelProvider for T`:

```rust
fn tool_capabilities(&self, model: Option<&str>) -> ProviderToolCapabilities {
    ModelProvider::tool_capabilities(self, model)
}
```

- [ ] **Step 3: Lower canonical tool specs**

In `openai.rs`, import `ResponsesTool` and add this helper:

```rust
fn responses_tools(tools: &[crate::provider::NoemaToolSpec]) -> Vec<ResponsesTool> {
    tools
        .iter()
        .map(|tool| {
            ResponsesTool::function(
                tool.name.as_str(),
                tool.description.clone(),
                tool.input_schema.as_value().clone(),
            )
        })
        .collect()
}
```

When constructing `ResponsesRequest`, set:

```rust
tools: responses_tools(&request.tools),
tool_choice: (!request.tools.is_empty()).then_some(match request.tool_choice {
    crate::provider::NoemaToolChoice::Auto => "auto",
    crate::provider::NoemaToolChoice::None => "none",
    crate::provider::NoemaToolChoice::Required => "required",
}),
parallel_tool_calls: (!request.tools.is_empty()).then_some(request.parallel_tool_calls),
```

Add the same helper and fields to `codex_responses.rs` for
`CodexResponsesRequest`.

- [ ] **Step 4: Merge native tool calls into parsed responses**

In both OpenAI and Codex adapters, after receiving `response`, compute:

```rust
let native_tool_calls = response.native_tool_calls()?;
```

For `require_noema_response`, parse output text when present. If
`response.output_text()` returns `MalformedResponse` and `native_tool_calls` is
not empty, create:

```rust
ParsedNoemaResponse {
    responses: Vec::new(),
    tool_calls: native_tool_calls,
    memory_proposals: Vec::new(),
    response_status: GenerateResponseStatus::NeedsTools,
}
```

If output text is present and native tool calls are also present, parse the
Noema response object and then set:

```rust
parsed.tool_calls = native_tool_calls;
parsed.response_status = if parsed.tool_calls.is_empty() {
    GenerateResponseStatus::Final
} else {
    GenerateResponseStatus::NeedsTools
};
```

For non-required responses, keep final text behavior and append native tool
calls only when present.

- [ ] **Step 5: Run adapter tests**

Run:

```bash
cargo test -p noema-core provider::adapters::openai provider::adapters::codex_responses -- --nocapture
```

Expected: all OpenAI and Codex adapter tests pass.

- [ ] **Step 6: Commit Task 5**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/provider/adapters/openai.rs crates/noema-core/src/provider/adapters/codex_responses.rs crates/noema-core/src/provider/adapters/foundation_local.rs
git commit -m "Wire native tools into Responses providers"
```

## Task 6: Route Runtime Requests Through Native Tool Exposure

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/runtime/handle.rs`
- Modify: `crates/noema-core/src/daemon/prompts.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add failing runtime tests**

In `crates/noema-core/src/daemon/tests.rs`, first extend
`RecordingFakeProvider`:

```rust
struct RecordingFakeProvider {
    provider_kind: String,
    inner: FakeCodexProvider,
    requests: Mutex<Vec<GenerateRequest>>,
    tool_capabilities: crate::provider::ProviderToolCapabilities,
}

impl RecordingFakeProvider {
    fn new(provider_kind: &str, scenario: FakeCodexScenario) -> Self {
        Self {
            provider_kind: provider_kind.to_string(),
            inner: FakeCodexProvider::new(scenario),
            requests: Mutex::new(Vec::new()),
            tool_capabilities: crate::provider::ProviderToolCapabilities::default(),
        }
    }

    fn with_tool_capabilities(
        mut self,
        tool_capabilities: crate::provider::ProviderToolCapabilities,
    ) -> Self {
        self.tool_capabilities = tool_capabilities;
        self
    }

    fn requests(&self) -> Vec<GenerateRequest> {
        self.requests.lock().expect("requests").clone()
    }
}
```

Add this method to `impl super::runtime::RuntimeModelProvider for
RecordingFakeProvider`:

```rust
fn tool_capabilities(&self, _model: Option<&str>) -> crate::provider::ProviderToolCapabilities {
    self.tool_capabilities
}
```

Then add these tests:

```rust
#[tokio::test]
async fn native_capable_provider_receives_model_visible_tools() {
    let provider = Arc::new(RecordingFakeProvider::new("codex", FakeCodexScenario::Simple)
        .with_tool_capabilities(crate::provider::ProviderToolCapabilities {
        native_tools: true,
        parallel_tool_calls: true,
        tool_choice: true,
        schema_dialect: crate::provider::ProviderToolSchemaDialect::OpenAiResponses,
        strict_schema: false,
        custom_tools: false,
        native_tool_results: true,
        fallback_mode: crate::provider::ProviderToolFallbackMode::NativeRequired,
    }));
    let handle = test_runtime_handle(provider.clone()).await;

    let conversation = handle
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "What do you remember about me?".to_string(),
    )
    .await
    .expect("turn");

    let requests = provider.requests();
    let tool_names = requests
        .first()
        .expect("initial request")
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"search_memory"));
    handle.shutdown().await;
}

#[tokio::test]
async fn non_native_provider_does_not_receive_mcp_tools() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    seed_ready_docs_mcp(&store).await;
    let provider = Arc::new(RecordingFakeProvider::new("foundation_local", FakeCodexScenario::Simple)
        .with_tool_capabilities(crate::provider::ProviderToolCapabilities {
        fallback_mode: crate::provider::ProviderToolFallbackMode::BuiltinOnlyEnvelope,
        ..crate::provider::ProviderToolCapabilities::default()
    }));
    let handle = CodexRuntimeHandle::spawn_with_provider_map(
        "foundation_local",
        vec![(
            "foundation_local".to_string(),
            provider.clone() as Arc<dyn super::runtime::RuntimeModelProvider>,
        )],
        store,
    )
    .await
    .expect("runtime");

    let conversation = handle
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    collect_turn(
        &handle,
        conversation.conversation_id.clone(),
        "Read the docs page.".to_string(),
    )
    .await
    .expect("turn");

    let requests = provider.requests();
    let request = requests.first().expect("initial request");
    assert!(request.tools.is_empty());
    assert!(!request
        .instructions
        .as_deref()
        .unwrap_or_default()
        .contains("mcp.docs.read"));
    handle.shutdown().await;
}
```

Add this helper near the other MCP test seed helpers:

```rust
async fn seed_ready_docs_mcp(store: &crate::NoemaStore) {
    let server = store
        .create_mcp_server(crate::NewMcpServer {
            mcp_server_id: "docs".to_string(),
            display_name: "Docs".to_string(),
            transport_kind: crate::McpTransportKind::Stdio,
            safe_config: json!({}),
        })
        .await
        .expect("server");
    store
        .update_mcp_server_runtime_state(
            &server.mcp_server_id,
            true,
            crate::McpServerHealthStatus::Healthy,
            crate::McpServerAuthStatus::Authenticated,
        )
        .await
        .expect("server state");
    store
        .upsert_discovered_mcp_tool(crate::NewMcpTool {
            mcp_tool_id: "docs:read".to_string(),
            mcp_server_id: "docs".to_string(),
            name: "read".to_string(),
            description: Some("Read a document.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {"document_id": {"type": "string"}},
                "required": ["document_id"],
                "additionalProperties": false
            }),
            output_schema: None,
            annotations: json!({}),
            metadata_fingerprint: "fp1".to_string(),
        })
        .await
        .expect("tool");
    store
        .save_tool_calibration(crate::NewToolCalibration {
            calibration_id: "cal_docs_read".to_string(),
            mcp_tool_id: "docs:read".to_string(),
            read_classification: crate::McpTrustClassification::Trusted,
            write_classification: crate::McpTrustClassification::None,
            export_classification: crate::McpTrustClassification::None,
            owner_extractors: Vec::new(),
            status: crate::McpCalibrationStatus::Ready,
            reviewed_by: Some("human:local".to_string()),
            reviewed_metadata_fingerprint: Some("fp1".to_string()),
        })
        .await
        .expect("calibration");
}
```

Run:

```bash
cargo test -p noema-core daemon::tests::native_capable_provider_receives_model_visible_tools daemon::tests::non_native_provider_does_not_receive_mcp_tools -- --nocapture
```

Expected: tests fail because runtime requests do not build `GenerateRequest.tools`.

- [ ] **Step 2: Add provider capability lookup and model tools to initial turns**

In `turn.rs`, import:

```rust
use super::model_tools::build_model_tools;
```

Before the initial `generate_streaming` call for normal user turns, add:

```rust
let tool_capabilities = provider.tool_capabilities(conversation.model.as_deref());
let model_tools = build_model_tools(&self.store, agent_identity.display_name.is_none(), tool_capabilities)
    .await
    .map_err(|error| DaemonError::Protocol(error.to_string()))?;
```

Pass `model_tools.native.clone()` into the `GenerateRequest`:

```rust
tools: model_tools.native.clone(),
tool_choice: crate::provider::NoemaToolChoice::Auto,
parallel_tool_calls: tool_capabilities.parallel_tool_calls,
```

For onboarding turns, pass no tools:

```rust
tools: Vec::new(),
tool_choice: crate::provider::NoemaToolChoice::None,
parallel_tool_calls: false,
```

- [ ] **Step 3: Use model-tool rows in prompts**

Replace the existing `render_available_tools` call path with a method that
returns both prompt rows and native specs. The prompt should receive:

```rust
let available_tools_prompt = build_model_available_tools_prompt(
    &model_tools
        .prompt_rows
        .iter()
        .chain(model_tools.unavailable_rows.iter())
        .cloned()
        .collect::<Vec<_>>(),
);
```

When `model_tools.native` is non-empty, the prompt must say that executable
tools are available through the provider tool channel and must not include the
old JSON `tool_calls` item shape. When `legacy_builtin_envelope_tools` is
non-empty, keep only the existing builtin envelope instructions for those
builtin names.

- [ ] **Step 4: Pass tools to continuation requests**

For provider continuations after local tool results, compute:

```rust
let continuation_model_tools =
    build_model_tools(&self.store, false, tool_capabilities)
        .await
        .map_err(|error| DaemonError::Protocol(error.to_string()))?;
```

Pass:

```rust
tools: continuation_model_tools.native.clone(),
tool_choice: crate::provider::NoemaToolChoice::Auto,
parallel_tool_calls: tool_capabilities.parallel_tool_calls,
```

Keep the existing disallowance for `update_own_name` on continuations. The
native continuation tool set should not include `update_own_name`.

- [ ] **Step 5: Update prompt tests**

In `crates/noema-core/src/daemon/prompts.rs`, update tests that currently assert
the prompt contains:

```rust
"emit the relevant tool_calls item in this response"
```

Native-mode tests should assert:

```rust
assert!(prompt.contains("Executable tools are provided through the native tool channel"));
assert!(!prompt.contains("Use this tool_calls item shape"));
```

Legacy builtin fallback tests should assert:

```rust
assert!(prompt.contains("Builtin fallback tools may be emitted in tool_calls"));
assert!(prompt.contains("search_memory"));
assert!(!prompt.contains("mcp."));
```

- [ ] **Step 6: Run runtime and prompt tests**

Run:

```bash
cargo test -p noema-core daemon::prompts::tests daemon::runtime::tool_lifecycle::tests daemon::tests::native_capable_provider_receives_model_visible_tools daemon::tests::non_native_provider_does_not_receive_mcp_tools -- --nocapture
```

Expected: tests pass.

- [ ] **Step 7: Commit Task 6**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/runtime/handle.rs crates/noema-core/src/daemon/prompts.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Route runtime turns through native tool exposure"
```

## Task 7: Full Validation And Context Update

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Run Rust validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all commands pass.

- [ ] **Step 2: Update durable context**

Append a concise note under `## Settled Decisions` in `docs/context/current.md`:

```markdown
- The first native tool-plane slice has landed: Noema has provider-neutral
  canonical tool specs and provider capability declarations, OpenAI/Codex
  Responses requests lower eligible builtin and calibrated MCP tools into
  native tool specs, native provider tool-call items parse into Noema runtime
  tool calls, and non-native providers do not receive calibrated third-party
  MCP tools through the legacy response envelope.
```

- [ ] **Step 3: Run ship checklist for this unit**

Run:

```bash
git status --short --branch
git diff --check
git add docs/context/current.md
git diff --cached --stat
git diff --cached --name-status
git commit -m "Record native tool plane implementation"
```

Expected staged files: only `docs/context/current.md`.

## Self-Review

Spec coverage:

- Provider-neutral canonical tool model: Task 1.
- Builtin tool schemas from executable code: Task 2.
- Calibrated MCP tool exposure from Capability Gateway metadata: Task 3.
- Provider capability declarations and explicit fallback: Tasks 1, 3, 5, 6.
- OpenAI/Codex native Responses lowering and parsing: Tasks 4 and 5.
- Runtime native request flow and continuation handling: Task 6.
- Tests for provider filtering, adapter parsing, and runtime exposure: Tasks 1
  through 7.
- Replay cleanup: intentionally excluded and called out in Scope Check as a
  separate plan.

Placeholder scan:

- This plan avoids placeholder markers, vague error-handling instructions, and
  generic test directives without concrete test code.
- The only intentionally deferred work is replay cleanup, which is an explicit
  separate subsystem rather than an unfinished step in this plan.

Type consistency:

- `NoemaToolSpec`, `NoemaToolSchema`, `NoemaToolExecution`,
  `ProviderToolCapabilities`, `ProviderToolFallbackMode`,
  `ProviderToolSchemaDialect`, and `NoemaToolChoice` are defined in Task 1 and
  reused consistently in later tasks.
- `GenerateResponse.tool_calls` remains the runtime-neutral parsed output slot.
- Native provider function calls are parsed into the existing
  `GenerateToolCall` type so `tool_lifecycle.rs` and `local_tools.rs` can
  continue to own execution ordering.
