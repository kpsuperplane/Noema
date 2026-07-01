# MCP Tool Calibration Autofill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add advisory LLM-powered Autofill suggestions for MCP tool calibration, applying suggestions to frontend draft state while requiring the user to click Save before persistence.

**Architecture:** Add a pure MCP autofill module for prompt construction, strict JSON parsing, and validation. Expose it through a GraphQL mutation that uses the existing runtime model provider via a new one-shot generation command. The React MCP permissions modal calls that mutation, applies validated suggestions into local drafts, and automatically runs it once after new server setup opens calibration.

**Tech Stack:** Rust, async-graphql, serde/serde_json, existing Noema provider runtime, React, Apollo Client, Bun codegen/build.

---

## File Structure

- Create `crates/noema-core/src/mcp/autofill.rs`: pure autofill request/response types, prompt builder, JSON parser, validation against known tools and MCP calibration enums.
- Modify `crates/noema-core/src/mcp.rs`: export the new `autofill` module.
- Modify `crates/noema-core/src/daemon/runtime/handle.rs`: add one-shot `generate_once` command to reuse the configured provider outside chat turns.
- Modify `crates/noema-core/src/daemon/runtime/actor.rs`: handle the one-shot generation command.
- Modify `crates/noema-core/src/graphql/runtime_state.rs`: add a test constructor that accepts a runtime handle.
- Modify `crates/noema-core/src/graphql/mcp.rs`: add GraphQL suggestion types and `autofill_tool_calibrations` resolver.
- Modify `crates/noema-core/src/graphql/schema.rs`: expose the mutation and add GraphQL tests.
- Modify `crates/noema-core/web/src/graphql/operations.ts`: add `AutofillToolCalibrationsDocument`.
- Modify `crates/noema-core/web/src/components/settings/McpSettingsPane.tsx`: track newly added server ids that should auto-run Autofill once.
- Modify `crates/noema-core/web/src/components/settings/McpSettingsPaneContent.tsx`: pass auto-autofill state into the permissions modal.
- Modify `crates/noema-core/web/src/components/settings/McpToolPermissionsModal.tsx`: add Autofill button, mutation, draft merge, status messages, and automatic once-per-new-server trigger.
- Regenerate `crates/noema-core/web/src/generated/graphql.ts`, `crates/noema-core/web/src/generated/schema.graphql`, and daemon web assets with the existing Bun scripts.
- Update `docs/context/current.md` after the implementation lands.

## Task 1: Pure Autofill Module

**Files:**
- Create: `crates/noema-core/src/mcp/autofill.rs`
- Modify: `crates/noema-core/src/mcp.rs`

- [ ] **Step 1: Write module tests first**

Add tests in `crates/noema-core/src/mcp/autofill.rs` under `#[cfg(test)]`:

```rust
#[test]
fn parses_valid_autofill_response_for_known_tools() {
    let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
    let response = r#"{
      "suggestions": [{
        "mcp_tool_id": "mcp_tool:docs:read",
        "read_classification": "mixed",
        "write_classification": "none",
        "export_classification": "none",
        "owner_extractors": [{
          "source": "arguments",
          "selector_kind": "email",
          "path": "/owner_email"
        }],
        "disabled": false
      }]
    }"#;

    let suggestions = parse_autofill_response(response, &tools).expect("suggestions");
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].mcp_tool_id, "mcp_tool:docs:read");
    assert_eq!(suggestions[0].read_classification, McpTrustClassification::Mixed);
    assert_eq!(suggestions[0].owner_extractors[0].path, "/owner_email");
    assert!(!suggestions[0].disabled);
}

#[test]
fn rejects_unknown_tool_id_without_partial_suggestions() {
    let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
    let response = r#"{
      "suggestions": [{
        "mcp_tool_id": "mcp_tool:docs:missing",
        "read_classification": "mixed",
        "write_classification": "none",
        "export_classification": "none",
        "owner_extractors": [],
        "disabled": false
      }]
    }"#;

    let error = parse_autofill_response(response, &tools).expect_err("unknown tool rejected");
    assert!(error.to_string().contains("unknown MCP tool id"));
}

#[test]
fn rejects_invalid_enum_and_blank_extractor_path() {
    let tools = vec![test_tool("mcp_tool:docs:read", "read_doc")];
    let response = r#"{
      "suggestions": [{
        "mcp_tool_id": "mcp_tool:docs:read",
        "read_classification": "Mixed",
        "write_classification": "none",
        "export_classification": "none",
        "owner_extractors": [{
          "source": "arguments",
          "selector_kind": "email",
          "path": ""
        }],
        "disabled": false
      }]
    }"#;

    let error = parse_autofill_response(response, &tools).expect_err("invalid output rejected");
    assert!(error.to_string().contains("invalid read_classification"));
}

#[test]
fn prompt_names_trust_axes_and_demands_strict_json() {
    let prompt = build_autofill_prompt("Docs", &[test_tool("mcp_tool:docs:read", "read_doc")]);
    assert!(prompt.contains("Return strict JSON only"));
    assert!(prompt.contains("export means"));
    assert!(prompt.contains("metadata only"));
    assert!(prompt.contains("mcp_tool:docs:read"));
}
```

- [ ] **Step 2: Run the focused tests and confirm they fail**

Run: `cargo test -p noema-core mcp::autofill --no-fail-fast`

Expected: compile failure because `mcp::autofill`, `build_autofill_prompt`, and `parse_autofill_response` do not exist yet.

- [ ] **Step 3: Implement the pure module**

Create `crates/noema-core/src/mcp/autofill.rs` with:

```rust
use std::collections::BTreeSet;

use serde::Deserialize;
use thiserror::Error;

use crate::{
    McpToolRecord, McpTrustClassification, OwnerExtractor, OwnerExtractorSource,
    TrustedIdentitySelectorKind,
};

#[derive(Clone, Debug, PartialEq)]
pub struct McpToolCalibrationSuggestion {
    pub mcp_tool_id: String,
    pub read_classification: McpTrustClassification,
    pub write_classification: McpTrustClassification,
    pub export_classification: McpTrustClassification,
    pub owner_extractors: Vec<OwnerExtractor>,
    pub disabled: bool,
}

#[derive(Debug, Error)]
pub enum McpAutofillError {
    #[error("invalid autofill JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unknown MCP tool id in autofill response: {0}")]
    UnknownToolId(String),
    #[error("invalid {field}: expected one of none, trusted, untrusted, mixed")]
    InvalidClassification { field: &'static str },
    #[error("invalid owner extractor source: {0}")]
    InvalidExtractorSource(String),
    #[error("invalid owner extractor selector kind: {0}")]
    InvalidSelectorKind(String),
    #[error("owner extractor path cannot be empty")]
    BlankExtractorPath,
}

#[derive(Debug, Deserialize)]
struct AutofillResponse {
    suggestions: Vec<RawSuggestion>,
}

#[derive(Debug, Deserialize)]
struct RawSuggestion {
    mcp_tool_id: String,
    read_classification: String,
    write_classification: String,
    export_classification: String,
    #[serde(default)]
    owner_extractors: Vec<RawOwnerExtractor>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Deserialize)]
struct RawOwnerExtractor {
    source: String,
    selector_kind: String,
    path: String,
}

pub fn build_autofill_prompt(server_name: &str, tools: &[McpToolRecord]) -> String {
    let tool_payload = tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "mcp_tool_id": tool.mcp_tool_id,
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
                "output_schema": tool.output_schema,
                "annotations": tool.annotations,
                "metadata_fingerprint": tool.metadata_fingerprint
            })
        })
        .collect::<Vec<_>>();
    format!(
        r#"You are Noema's MCP tool calibration assistant.
Return strict JSON only. Do not include Markdown, comments, code fences, or prose.
Classify from metadata only for MCP server "{server_name}".

Definitions:
- read means the tool can bring data from the MCP destination into Noema.
- write means the tool can mutate state inside the MCP destination.
- export means the tool can share information beyond the MCP destination.
- Use none only when the axis clearly does not apply.
- Use mixed when trust or ownership depends on runtime contents.
- Use trusted or untrusted only when metadata makes the trust boundary clear without runtime data.
- Suggest owner_extractors only when a deterministic field exists in the metadata shape.
- If ownership cannot be resolved, return an empty owner_extractors array.

Return exactly:
{{"suggestions":[{{"mcp_tool_id":"...","read_classification":"none|trusted|untrusted|mixed","write_classification":"none|trusted|untrusted|mixed","export_classification":"none|trusted|untrusted|mixed","owner_extractors":[{{"source":"arguments|structured_content|metadata|resource_uri|built_in_adapter","selector_kind":"email|phone|domain","path":"..."}}],"disabled":false}}]}}

Tools:
{}"#,
        serde_json::to_string_pretty(&tool_payload).expect("tool metadata is serializable")
    )
}

pub fn parse_autofill_response(
    text: &str,
    tools: &[McpToolRecord],
) -> Result<Vec<McpToolCalibrationSuggestion>, McpAutofillError> {
    let response: AutofillResponse = serde_json::from_str(text.trim())?;
    let known_tool_ids = tools
        .iter()
        .map(|tool| tool.mcp_tool_id.as_str())
        .collect::<BTreeSet<_>>();
    response
        .suggestions
        .into_iter()
        .map(|suggestion| {
            if !known_tool_ids.contains(suggestion.mcp_tool_id.as_str()) {
                return Err(McpAutofillError::UnknownToolId(suggestion.mcp_tool_id));
            }
            Ok(McpToolCalibrationSuggestion {
                mcp_tool_id: suggestion.mcp_tool_id,
                read_classification: parse_classification(
                    &suggestion.read_classification,
                    "read_classification",
                )?,
                write_classification: parse_classification(
                    &suggestion.write_classification,
                    "write_classification",
                )?,
                export_classification: parse_classification(
                    &suggestion.export_classification,
                    "export_classification",
                )?,
                owner_extractors: suggestion
                    .owner_extractors
                    .into_iter()
                    .map(parse_owner_extractor)
                    .collect::<Result<Vec<_>, _>>()?,
                disabled: suggestion.disabled,
            })
        })
        .collect()
}

fn parse_classification(
    value: &str,
    field: &'static str,
) -> Result<McpTrustClassification, McpAutofillError> {
    match value {
        "none" => Ok(McpTrustClassification::None),
        "trusted" => Ok(McpTrustClassification::Trusted),
        "untrusted" => Ok(McpTrustClassification::Untrusted),
        "mixed" => Ok(McpTrustClassification::Mixed),
        _ => Err(McpAutofillError::InvalidClassification { field }),
    }
}

fn parse_owner_extractor(raw: RawOwnerExtractor) -> Result<OwnerExtractor, McpAutofillError> {
    let path = raw.path.trim().to_string();
    if path.is_empty() {
        return Err(McpAutofillError::BlankExtractorPath);
    }
    Ok(OwnerExtractor {
        source: parse_extractor_source(&raw.source)?,
        selector_kind: parse_selector_kind(&raw.selector_kind)?,
        path,
    })
}

fn parse_extractor_source(value: &str) -> Result<OwnerExtractorSource, McpAutofillError> {
    match value {
        "arguments" => Ok(OwnerExtractorSource::Arguments),
        "structured_content" => Ok(OwnerExtractorSource::StructuredContent),
        "metadata" => Ok(OwnerExtractorSource::Metadata),
        "resource_uri" => Ok(OwnerExtractorSource::ResourceUri),
        "built_in_adapter" => Ok(OwnerExtractorSource::BuiltInAdapter),
        _ => Err(McpAutofillError::InvalidExtractorSource(value.to_string())),
    }
}

fn parse_selector_kind(value: &str) -> Result<TrustedIdentitySelectorKind, McpAutofillError> {
    match value {
        "email" => Ok(TrustedIdentitySelectorKind::Email),
        "phone" => Ok(TrustedIdentitySelectorKind::Phone),
        "domain" => Ok(TrustedIdentitySelectorKind::Domain),
        _ => Err(McpAutofillError::InvalidSelectorKind(value.to_string())),
    }
}
```

Add this `test_tool` helper in the test module:

```rust
fn test_tool(mcp_tool_id: &str, name: &str) -> McpToolRecord {
    McpToolRecord {
        mcp_tool_id: mcp_tool_id.to_string(),
        mcp_server_id: "mcp_server:docs".to_string(),
        name: name.to_string(),
        description: Some("Read a document".to_string()),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "owner_email": { "type": "string" }
            }
        }),
        output_schema: None,
        annotations: serde_json::json!({"readOnlyHint": true}),
        metadata_fingerprint: "fingerprint_1".to_string(),
        discovered_at: "2026-07-01T00:00:00Z".to_string(),
    }
}
```

Add `pub mod autofill;` to `crates/noema-core/src/mcp.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p noema-core mcp::autofill --no-fail-fast`

Expected: PASS for the new pure autofill tests.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/mcp.rs crates/noema-core/src/mcp/autofill.rs
git commit -m "feat: add mcp calibration autofill parser"
```

## Task 2: Runtime One-Shot Generation

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/handle.rs`
- Modify: `crates/noema-core/src/daemon/runtime/actor.rs`

- [ ] **Step 1: Add a failing runtime test**

In `crates/noema-core/src/daemon/tests.rs`, add a test near existing runtime-handle tests:

```rust
#[tokio::test]
async fn runtime_handle_generate_once_uses_provider() {
    let handle = test_runtime_handle(fake_codex_provider()).await;
    let response = handle
        .generate_once(crate::provider::GenerateRequest::text("hello"))
        .await
        .expect("generate once");

    assert_eq!(response.assistant_text(), "Hello from Noema.");
}
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run: `cargo test -p noema-core runtime_handle_generate_once_uses_provider --no-fail-fast`

Expected: compile failure because `CodexRuntimeHandle::generate_once` does not exist.

- [ ] **Step 3: Implement the command**

In `crates/noema-core/src/daemon/runtime/handle.rs`, add:

```rust
pub(crate) async fn generate_once(
    &self,
    request: GenerateRequest,
) -> Result<GenerateResponse, DaemonError> {
    let (reply, reply_rx) = oneshot::channel();
    self.sender
        .send(CodexRuntimeCommand::GenerateOnce { request, reply })
        .await
        .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
    reply_rx
        .await
        .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
}
```

Add the command variant:

```rust
GenerateOnce {
    request: GenerateRequest,
    reply: oneshot::Sender<Result<GenerateResponse, DaemonError>>,
},
```

In `crates/noema-core/src/daemon/runtime/actor.rs`, handle it:

```rust
CodexRuntimeCommand::GenerateOnce { request, reply } => {
    let mut ignore_event = |_| {};
    let result = self
        .provider
        .generate_streaming(request, &mut ignore_event)
        .await
        .map_err(DaemonError::Provider);
    let _ = reply.send(result);
}
```

- [ ] **Step 4: Run test**

Run: `cargo test -p noema-core runtime_handle_generate_once_uses_provider --no-fail-fast`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/daemon/runtime/handle.rs crates/noema-core/src/daemon/runtime/actor.rs crates/noema-core/src/daemon/tests.rs
git commit -m "feat: add runtime one-shot generation"
```

## Task 3: GraphQL Autofill Mutation

**Files:**
- Modify: `crates/noema-core/src/graphql/runtime_state.rs`
- Modify: `crates/noema-core/src/graphql/mcp.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`

- [ ] **Step 1: Add failing GraphQL tests**

In `crates/noema-core/src/graphql/schema.rs`, add tests near MCP calibration tests:

```rust
#[tokio::test]
async fn autofill_tool_calibrations_returns_validated_suggestions_without_persisting() {
    let store = crate::store::tests::test_store().await;
    seed_autofill_server(&store).await;
    let runtime = test_autofill_runtime(
        r#"{"suggestions":[{"mcp_tool_id":"mcp_tool:docs:read_doc","read_classification":"mixed","write_classification":"none","export_classification":"none","owner_extractors":[{"source":"arguments","selector_kind":"email","path":"/owner_email"}],"disabled":false}]}"#
    ).await;
    let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(store.clone(), runtime));

    let response = schema
        .execute(async_graphql::Request::new(
            r#"
            mutation {
              autofillToolCalibrations(mcpServerId: "mcp_server:docs") {
                suggestions {
                  mcpToolId
                  readClassification
                  writeClassification
                  exportClassification
                  disabled
                  ownerExtractors { source selectorKind path }
                }
              }
            }
            "#,
        ))
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    let suggestion = &data["autofillToolCalibrations"]["suggestions"][0];
    assert_eq!(suggestion["mcpToolId"], "mcp_tool:docs:read_doc");
    assert_eq!(suggestion["readClassification"], "mixed");
    assert_eq!(suggestion["ownerExtractors"][0]["path"], "/owner_email");
    assert!(
        store
            .get_tool_calibration("mcp_tool:docs:read_doc")
            .await
            .expect("get calibration")
            .is_none()
    );
}

#[tokio::test]
async fn autofill_tool_calibrations_rejects_invalid_model_output() {
    let store = crate::store::tests::test_store().await;
    seed_autofill_server(&store).await;
    let runtime = test_autofill_runtime(
        r#"{"suggestions":[{"mcp_tool_id":"mcp_tool:docs:missing","read_classification":"mixed","write_classification":"none","export_classification":"none","owner_extractors":[],"disabled":false}]}"#
    ).await;
    let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(store.clone(), runtime));

    let response = schema
        .execute(async_graphql::Request::new(
            r#"mutation { autofillToolCalibrations(mcpServerId: "mcp_server:docs") { suggestions { mcpToolId } } }"#,
        ))
        .await;

    assert!(!response.errors.is_empty());
    assert!(
        response.errors[0].message.contains("unknown MCP tool id"),
        "{:?}",
        response.errors
    );
    assert!(
        store
            .get_tool_calibration("mcp_tool:docs:read_doc")
            .await
            .expect("get calibration")
            .is_none()
    );
}
```

Add helpers in the test module:

```rust
async fn seed_autofill_server(store: &crate::NoemaStore) {
    store.create_mcp_server(crate::NewMcpServer {
        mcp_server_id: "mcp_server:docs".to_string(),
        display_name: "Docs".to_string(),
        transport_kind: crate::McpTransportKind::Stdio,
        safe_config: serde_json::json!({}),
    }).await.expect("create server");
    store.upsert_discovered_mcp_tool(crate::NewMcpTool {
        mcp_tool_id: "mcp_tool:docs:read_doc".to_string(),
        mcp_server_id: "mcp_server:docs".to_string(),
        name: "read_doc".to_string(),
        description: Some("Read a document by id".to_string()),
        input_schema: serde_json::json!({"type":"object","properties":{"owner_email":{"type":"string"}}}),
        output_schema: None,
        annotations: serde_json::json!({"readOnlyHint": true}),
        metadata_fingerprint: "fingerprint_1".to_string(),
    }).await.expect("upsert tool");
}
```

- [ ] **Step 2: Run tests and confirm they fail**

Run: `cargo test -p noema-core autofill_tool_calibrations --no-fail-fast`

Expected: compile failure because the GraphQL mutation, test runtime constructor, and test provider helper do not exist yet.

- [ ] **Step 3: Add GraphQL types and resolver**

In `crates/noema-core/src/graphql/mcp.rs`, add `SimpleObject` types:

```rust
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlAutofillToolCalibrationsResult {
    pub suggestions: Vec<GraphqlToolCalibrationSuggestion>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlToolCalibrationSuggestion {
    pub mcp_tool_id: String,
    pub read_classification: String,
    pub write_classification: String,
    pub export_classification: String,
    pub owner_extractors: Vec<GraphqlOwnerExtractor>,
    pub disabled: bool,
}
```

Add this conversion:

```rust
impl From<crate::mcp::autofill::McpToolCalibrationSuggestion>
    for GraphqlToolCalibrationSuggestion
{
    fn from(suggestion: crate::mcp::autofill::McpToolCalibrationSuggestion) -> Self {
        Self {
            mcp_tool_id: suggestion.mcp_tool_id,
            read_classification: suggestion.read_classification.as_str().to_string(),
            write_classification: suggestion.write_classification.as_str().to_string(),
            export_classification: suggestion.export_classification.as_str().to_string(),
            owner_extractors: suggestion
                .owner_extractors
                .into_iter()
                .map(Into::into)
                .collect(),
            disabled: suggestion.disabled,
        }
    }
}
```

Add resolver:

```rust
pub(super) async fn autofill_tool_calibrations(
    state: &GraphqlState,
    mcp_server_id: String,
) -> Result<GraphqlAutofillToolCalibrationsResult> {
    let store = state.store()?;
    let server = store
        .get_mcp_server(&mcp_server_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| graphql_error("MCP server was not found"))?;
    let tools = store
        .list_mcp_tools_for_server(&mcp_server_id)
        .await
        .map_err(graphql_error)?;
    let prompt = crate::mcp::autofill::build_autofill_prompt(&server.display_name, &tools);
    let mut request = crate::provider::GenerateRequest::text(prompt);
    request.instructions = Some("Return strict JSON only for MCP calibration suggestions.".to_string());
    request.options.max_output_tokens = Some(4000);
    request.options.temperature = Some(0.0);
    let response = state.runtime()?.generate_once(request).await.map_err(graphql_error)?;
    let suggestions = crate::mcp::autofill::parse_autofill_response(
        &response.assistant_text(),
        &tools,
    )
    .map_err(graphql_error)?;
    Ok(GraphqlAutofillToolCalibrationsResult {
        suggestions: suggestions.into_iter().map(Into::into).collect(),
    })
}
```

- [ ] **Step 4: Expose mutation and test constructor**

In `crates/noema-core/src/graphql/schema.rs`, import the result type and add mutation:

```rust
async fn autofill_tool_calibrations(
    &self,
    ctx: &Context<'_>,
    mcp_server_id: String,
) -> Result<GraphqlAutofillToolCalibrationsResult> {
    let state = ctx.data_unchecked::<GraphqlState>();
    mcp::autofill_tool_calibrations(state, mcp_server_id).await
}
```

In `crates/noema-core/src/graphql/runtime_state.rs`, add:

```rust
#[cfg(test)]
#[must_use]
pub fn for_tests_with_store_and_runtime(
    store: NoemaStore,
    runtime: CodexRuntimeHandle,
) -> Self {
    Self {
        runtime: Some(runtime),
        store: Some(store),
        ..Self::for_tests()
    }
}
```

In `GraphqlState`, add this test constructor:

```rust
#[cfg(test)]
#[must_use]
pub fn for_tests_with_store_and_runtime(
    store: crate::NoemaStore,
    runtime: crate::daemon::CodexRuntimeHandle,
) -> Self {
    Self {
        runtime_state: GraphqlRuntimeState::for_tests_with_store_and_runtime(store, runtime),
        mcp_setup_outcomes: None,
    }
}
```

- [ ] **Step 5: Add test fake provider**

In `crates/noema-core/src/graphql/schema.rs` test module, add a small provider:

```rust
#[derive(Debug)]
struct AutofillTestProvider {
    text: String,
}

impl crate::provider::ModelProvider for AutofillTestProvider {
    fn generate(
        &self,
        _request: crate::provider::GenerateRequest,
    ) -> impl std::future::Future<
        Output = Result<crate::provider::GenerateResponse, crate::provider::ProviderError>,
    > + Send {
        let text = self.text.clone();
        async move {
            Ok(crate::provider::GenerateResponse {
                output: vec![crate::provider::GenerateOutputItem::AssistantText { text }],
                provider: "test".to_string(),
                model: "test-autofill".to_string(),
                response_id: None,
                usage: None,
            })
        }
    }
}

async fn test_autofill_runtime(text: &str) -> crate::daemon::CodexRuntimeHandle {
    crate::daemon::CodexRuntimeHandle::spawn_with_provider(
        std::sync::Arc::new(AutofillTestProvider { text: text.to_string() }),
        crate::store::tests::test_store().await,
    )
    .await
    .expect("runtime")
}
```

- [ ] **Step 6: Run focused GraphQL tests**

Run: `cargo test -p noema-core autofill_tool_calibrations --no-fail-fast`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/graphql/runtime_state.rs crates/noema-core/src/graphql/mcp.rs crates/noema-core/src/graphql/schema.rs
git commit -m "feat: expose mcp calibration autofill"
```

## Task 4: Frontend Manual Autofill

**Files:**
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Modify: `crates/noema-core/web/src/components/settings/McpToolPermissionsModal.tsx`
- Generated: `crates/noema-core/web/src/generated/graphql.ts`
- Generated: `crates/noema-core/web/src/generated/schema.graphql`
- Generated assets after build.

- [ ] **Step 1: Add GraphQL operation**

In `crates/noema-core/web/src/graphql/operations.ts`, add:

```ts
export const AutofillToolCalibrationsDocument = gql`
  mutation AutofillToolCalibrations($mcpServerId: String!) {
    autofillToolCalibrations(mcpServerId: $mcpServerId) {
      suggestions {
        mcpToolId
        readClassification
        writeClassification
        exportClassification
        disabled
        ownerExtractors {
          source
          selectorKind
          path
        }
      }
    }
  }
`;
```

- [ ] **Step 2: Generate frontend types**

Run: `cd crates/noema-core/web && bun run gen:types`

Expected: generated GraphQL types include `AutofillToolCalibrationsDocument`, `AutofillToolCalibrationsMutation`, and `AutofillToolCalibrationsMutationVariables`.

- [ ] **Step 3: Wire mutation into the modal**

In `McpToolPermissionsModal.tsx`, import `Sparkles` and the generated autofill document/types. Add mutation state:

```tsx
const [autofillToolCalibrations, autofillState] =
  useMutation<AutofillToolCalibrationsMutation>(AutofillToolCalibrationsDocument);
const [autofillMessage, setAutofillMessage] = React.useState<string | null>(null);
const [autofillError, setAutofillError] = React.useState<string | null>(null);
```

Add `handleAutofill`:

```tsx
async function handleAutofill() {
  if (!serverId) return;
  setAutofillMessage(null);
  setAutofillError(null);
  try {
    const response = await autofillToolCalibrations({ variables: { mcpServerId: serverId } });
    const suggestions = response.data?.autofillToolCalibrations.suggestions ?? [];
    setDraftOverrides((current) => {
      const next = { ...current };
      for (const suggestion of suggestions) {
        next[suggestion.mcpToolId] = draftFromSuggestion(suggestion);
      }
      return next;
    });
    setAutofillMessage("Autofill suggestions applied. Review before saving.");
  } catch {
    setAutofillError("Autofill could not generate suggestions. Configure tools manually or try again.");
  }
}
```

Add helper:

```tsx
function draftFromSuggestion(
  suggestion: AutofillToolCalibrationsMutation["autofillToolCalibrations"]["suggestions"][number]
): ToolPermissionDraft {
  return {
    readClassification: suggestion.readClassification,
    writeClassification: suggestion.writeClassification,
    exportClassification: suggestion.exportClassification,
    ownerExtractors: suggestion.ownerExtractors.map((extractor, index) => ({
      id: `${suggestion.mcpToolId}:autofill:${index}`,
      source: extractor.source,
      selectorKind: extractor.selectorKind,
      path: extractor.path
    })),
    disabled: suggestion.disabled
  };
}
```

- [ ] **Step 4: Add button next to Save**

Update `ToolPermissionsFooter` props to include `autofilling` and `onAutofill`. Render Autofill immediately before Save:

```tsx
<Button
  type="button"
  variant="outline"
  className="w-fit"
  disabled={saving || loading || autofilling || !canSave}
  onClick={onAutofill}
>
  {autofilling ? (
    <Loader2 className="size-4 animate-spin" aria-hidden="true" />
  ) : (
    <Sparkles className="size-4" aria-hidden="true" />
  )}
  Autofill
</Button>
```

Disable Save while `autofillState.loading` is true, and show `autofillMessage` / `autofillError` in the dialog body under existing save errors.

- [ ] **Step 5: Run frontend validation**

Run: `cd crates/noema-core/web && bun run lint`

Expected: PASS.

- [ ] **Step 6: Build assets**

Run: `cd crates/noema-core/web && bun run build`

Expected: PASS and regenerated daemon web assets.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/components/settings/McpToolPermissionsModal.tsx crates/noema-core/web/src/generated/graphql.ts crates/noema-core/web/src/generated/schema.graphql crates/noema-core/src/daemon/web/assets/app.js crates/noema-core/src/daemon/web/assets/styles.css
git commit -m "feat: add manual mcp calibration autofill"
```

## Task 5: Automatic Autofill After New Server Setup

**Files:**
- Modify: `crates/noema-core/web/src/components/settings/McpSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/McpSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/components/settings/McpToolPermissionsModal.tsx`
- Generated assets after build.

- [ ] **Step 1: Add auto-autofill props**

Extend `McpToolPermissionsModal` props:

```tsx
autoAutofill: boolean;
onAutoAutofillComplete: () => void;
```

Add an effect:

```tsx
const autoAutofillKey = React.useRef<string | null>(null);

React.useEffect(() => {
  if (!open || !serverId || !autoAutofill || result.loading || result.error || tools.length === 0) {
    return;
  }
  if (autoAutofillKey.current === serverId) return;
  autoAutofillKey.current = serverId;
  void handleAutofill().finally(onAutoAutofillComplete);
}, [autoAutofill, onAutoAutofillComplete, open, result.error, result.loading, serverId, tools.length]);
```

- [ ] **Step 2: Track newly added server in settings pane**

In `McpSettingsPane.tsx`, add:

```tsx
const [autoAutofillServerId, setAutoAutofillServerId] = React.useState<string | null>(null);
```

When setup returns `ready_for_calibration`, set both:

```tsx
setPermissionsServerId(setup.server.mcpServerId);
setAutoAutofillServerId(setup.server.mcpServerId);
```

Do the same in the OAuth completion branch. Pass both props through `McpSettingsPaneContent`.

- [ ] **Step 3: Clear auto state after the modal starts the run**

In `McpSettingsPaneContent.tsx`, pass:

```tsx
autoAutofill={permissionsServerId !== null && permissionsServerId === autoAutofillServerId}
onAutoAutofillComplete={() => setAutoAutofillServerId(null)}
```

In `McpSettingsPaneContent.tsx`, add props:

```tsx
autoAutofillServerId?: string | null;
onAutoAutofillComplete?: () => void;
```

Default them in the destructuring:

```tsx
autoAutofillServerId = null,
onAutoAutofillComplete = () => {},
```

Then pass them to `McpToolPermissionsModal`:

```tsx
autoAutofill={permissionsServerId === autoAutofillServerId}
onAutoAutofillComplete={onAutoAutofillComplete}
```

- [ ] **Step 4: Run frontend validation and build**

Run: `cd crates/noema-core/web && bun run lint`

Expected: PASS.

Run: `cd crates/noema-core/web && bun run build`

Expected: PASS and regenerated daemon web assets.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/web/src/components/settings/McpSettingsPane.tsx crates/noema-core/web/src/components/settings/McpSettingsPaneContent.tsx crates/noema-core/web/src/components/settings/McpToolPermissionsModal.tsx crates/noema-core/src/daemon/web/assets/app.js crates/noema-core/src/daemon/web/assets/styles.css
git commit -m "feat: autofill new mcp calibrations once"
```

## Task 6: Final Validation And Context

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Update durable context**

Add one sentence to the MCP settled-decision paragraph in `docs/context/current.md`:

```markdown
MCP tool calibration can now request LLM-generated Autofill suggestions from persisted tool metadata; suggestions populate frontend draft state only and require explicit Save before backend calibration records are written.
```

- [ ] **Step 2: Run backend validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all pass. If socket-related tests fail with sandbox `PermissionDenied`, rerun the same command with socket permissions and report that distinction.

- [ ] **Step 3: Run frontend validation**

Run:

```bash
cd crates/noema-core/web && bun run lint
cd crates/noema-core/web && bun run build
```

Expected: both pass.

- [ ] **Step 4: Ship checklist**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: only intended files are staged before the final commit.

- [ ] **Step 5: Commit context/update residue**

```bash
git add docs/context/current.md crates/noema-core/src/daemon/web/assets/app.js crates/noema-core/src/daemon/web/assets/styles.css
git commit -m "docs: update mcp autofill context"
```
