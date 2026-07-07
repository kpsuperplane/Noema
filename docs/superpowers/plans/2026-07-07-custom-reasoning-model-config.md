# Custom Reasoning Model Config Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional custom reasoning effort to every explicit model configuration while preserving provider-owned defaults when no custom model is configured.

**Architecture:** Add a provider-neutral `ReasoningEffort` enum and carry it through config, store-backed preferences, GraphQL, runtime request construction, and OpenAI Responses serialization. Provider profile metadata is the authority for whether a selected profile supports configurable reasoning; Codex must remain gated until independently verified.

**Tech Stack:** Rust 2024, SurrealDB schema definitions, async-graphql, OpenAI-compatible Responses adapters, React 19, TypeScript, Astryx, StyleX, Bun codegen/lint.

## Global Constraints

- Work on `main`.
- Preserve unrelated dirty worktree changes.
- Do not modify `CARGO_BUILD_RUSTC_WRAPPER`, unset sccache, or bypass the Rust compiler wrapper.
- Do not run smoke tests or fixture tests.
- Do not run browser inspection unless explicitly requested.
- Generated default config must not write a model id.
- Do not add a global default reasoning level.
- Do not infer reasoning support or defaults from English model id matching.
- Do not expose raw or summarized reasoning in the UI.
- Keep encrypted reasoning persistence/replay separate from explicit reasoning effort.
- OpenAI and Codex request behavior must be verified independently.
- Pre-V1 schema changes may rewrite table definitions directly; do not add migrations.
- For frontend work, do not write UI tests unless explicitly requested.

---

## File Structure

- Modify `crates/noema-core/src/provider/contract.rs`
  - Owns `ReasoningEffort`, its lowercase serialization, and `GenerateOptions.reasoning_effort`.
- Modify `crates/noema-core/src/provider/adapters/responses.rs`
  - Adds shared Responses `reasoning` request payload support.
- Modify `crates/noema-core/src/provider/adapters/openai.rs`
  - Sends `reasoning: { effort }` when `GenerateOptions.reasoning_effort` is set.
- Modify `crates/noema-core/src/provider/adapters/codex_responses.rs`
  - Does not send reasoning effort until Codex support is verified; adds an explicit regression test for that behavior.
- Modify `crates/noema-core/src/config/file.rs`, `crates/noema-core/src/config/loading.rs`, `crates/noema-core/src/config/raw.rs`, `crates/noema-core/src/config/provider.rs`, `crates/noema-core/src/home.rs`, and `crates/noema-core/src/config/tests.rs`
  - Removes generated default model, parses/validates config `reasoning_effort`, and carries explicit file config effort into provider configs.
- Modify `crates/noema-core/src/store/schema.rs`, `crates/noema-core/src/store/agent_runtime_preferences.rs`, `crates/noema-core/src/store/auxiliary_model_preferences.rs`, and `crates/noema-core/src/store/tests.rs`
  - Persists optional `reasoning_effort` alongside explicit store-backed model preferences.
- Modify `crates/noema-core/src/graphql/agents.rs`, `crates/noema-core/src/graphql/web_fetch_settings.rs`, `crates/noema-core/src/graphql/usage_settings.rs`, `crates/noema-core/src/graphql/schema.rs`
  - Exposes GraphQL `ReasoningEffort`, profile-supported efforts, preference effort, and save validation.
- Modify `crates/noema-core/src/daemon/runtime/conversation_state.rs`, `crates/noema-core/src/daemon/runtime/actor.rs`, `crates/noema-core/src/daemon/runtime/turn.rs`, `crates/noema-core/src/daemon/runtime/context_compaction.rs`, `crates/noema-core/src/daemon/runtime/local_tools.rs`, `crates/noema-core/src/daemon/runtime/progress_audit.rs`, and focused tests in `crates/noema-core/src/daemon/tests.rs`
  - Threads saved reasoning effort into every explicit-model generation path.
- Modify `crates/noema-core/web/src/graphql/operations.ts`, generated files, and settings components under `crates/noema-core/web/src/components/settings/`
  - Displays and saves reasoning effort where a selected profile advertises support.

---

### Task 1: Provider Contract, OpenAI Serialization, Codex Gate, And Config Defaults

**Files:**
- Modify: `crates/noema-core/src/provider/contract.rs`
- Modify: `crates/noema-core/src/provider/adapters/responses.rs`
- Modify: `crates/noema-core/src/provider/adapters/openai.rs`
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs`
- Modify: `crates/noema-core/src/config/provider.rs`
- Modify: `crates/noema-core/src/config/raw.rs`
- Modify: `crates/noema-core/src/config/file.rs`
- Modify: `crates/noema-core/src/config/loading.rs`
- Modify: `crates/noema-core/src/config/tests.rs`
- Modify: `crates/noema-core/src/home.rs`

**Interfaces:**
- Produces: `pub enum ReasoningEffort { None, Minimal, Low, Medium, High, XHigh }`
- Produces: `GenerateOptions { reasoning_effort: Option<ReasoningEffort>, ... }`
- Produces: provider config fields `reasoning_effort: Option<ReasoningEffort>` for explicit config defaults.
- Produces: OpenAI Responses JSON `reasoning: { "effort": "<lowercase>" }` when set.
- Produces: Codex regression proving no reasoning field is sent while unsupported.

- [ ] **Step 1: Write failing provider contract serialization tests**

Add this test module near the bottom of `crates/noema-core/src/provider/contract.rs`:

```rust
#[cfg(test)]
mod reasoning_effort_tests {
    use super::*;

    #[test]
    fn reasoning_effort_serializes_lowercase_api_values() {
        assert_eq!(serde_json::to_string(&ReasoningEffort::None).unwrap(), "\"none\"");
        assert_eq!(
            serde_json::to_string(&ReasoningEffort::Minimal).unwrap(),
            "\"minimal\""
        );
        assert_eq!(serde_json::to_string(&ReasoningEffort::Low).unwrap(), "\"low\"");
        assert_eq!(
            serde_json::to_string(&ReasoningEffort::Medium).unwrap(),
            "\"medium\""
        );
        assert_eq!(serde_json::to_string(&ReasoningEffort::High).unwrap(), "\"high\"");
        assert_eq!(serde_json::to_string(&ReasoningEffort::XHigh).unwrap(), "\"xhigh\"");
    }

    #[test]
    fn reasoning_effort_deserializes_lowercase_api_values() {
        assert_eq!(
            serde_json::from_str::<ReasoningEffort>("\"xhigh\"").unwrap(),
            ReasoningEffort::XHigh
        );
        assert!(serde_json::from_str::<ReasoningEffort>("\"extreme\"").is_err());
    }
}
```

- [ ] **Step 2: Run the failing contract test**

Run:

```bash
cargo test -p noema-core reasoning_effort_ --no-fail-fast
```

Expected: FAIL because `ReasoningEffort` does not exist.

- [ ] **Step 3: Add `ReasoningEffort` and `GenerateOptions.reasoning_effort`**

In `crates/noema-core/src/provider/contract.rs`, add above `GenerateOptions`:

```rust
/// Provider-neutral reasoning effort for providers that expose explicit reasoning controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningEffort {
    /// Disable explicit reasoning where the provider supports it.
    #[serde(rename = "none")]
    None,
    /// Minimal reasoning effort.
    #[serde(rename = "minimal")]
    Minimal,
    /// Low reasoning effort.
    #[serde(rename = "low")]
    Low,
    /// Medium reasoning effort.
    #[serde(rename = "medium")]
    Medium,
    /// High reasoning effort.
    #[serde(rename = "high")]
    High,
    /// Extra-high reasoning effort.
    #[serde(rename = "xhigh")]
    XHigh,
}
```

Then update `GenerateOptions`:

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GenerateOptions {
    /// Optional maximum number of output tokens.
    pub max_output_tokens: Option<u32>,
    /// Optional sampling temperature.
    pub temperature: Option<f32>,
    /// Optional explicit reasoning effort for reasoning-capable providers/models.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Require a strict Noema response object with response fields and memory proposals.
    pub require_noema_response: bool,
    /// Provider prompt-cache retention request when supported.
    pub prompt_cache_retention: Option<PromptCacheRetention>,
}
```

- [ ] **Step 4: Write failing OpenAI request serialization test**

In `crates/noema-core/src/provider/adapters/openai.rs`, add this test near existing request-body tests:

```rust
#[tokio::test]
async fn sends_reasoning_effort_when_configured() {
    let (base_url, request_rx) = spawn_server(
        200,
        r#"{
          "id": "resp_reasoning",
          "model": "gpt-test",
          "output": [{
            "type": "message",
            "content": [{"type": "output_text", "text": "ok"}]
          }],
          "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2}
        }"#,
    )
    .await;

    let provider = OpenAiProvider::new(OpenAiProviderConfig {
        api_key: "secret".to_string(),
        base_url,
        organization_id: None,
        project_id: None,
        default_model: "default-model".to_string(),
        tool_classification_model: None,
        timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
        system_errors: None,
        reasoning_effort: None,
    })
    .expect("provider");

    let response = provider
        .generate(GenerateRequest {
            conversation_id: None,
            model: Some("gpt-test".to_string()),
            input: GenerateInput::Text("Hello?".to_string()),
            instructions: None,
            options: crate::provider::GenerateOptions {
                reasoning_effort: Some(crate::provider::ReasoningEffort::High),
                ..crate::provider::GenerateOptions::default()
            },
            tools: Vec::new(),
            tool_choice: Default::default(),
            parallel_tool_calls: false,
        })
        .await
        .expect("response");

    assert_eq!(response.responses.len(), 1);
    let request = request_rx.recv().await.expect("request");
    assert_eq!(request["reasoning"]["effort"], "high");
}
```

- [ ] **Step 5: Run the failing OpenAI test**

Run:

```bash
cargo test -p noema-core sends_reasoning_effort_when_configured --no-fail-fast
```

Expected: FAIL because `OpenAiProviderConfig.reasoning_effort` and `ResponsesRequest.reasoning` do not exist.

- [ ] **Step 6: Add shared Responses `reasoning` payload and OpenAI serialization**

In `crates/noema-core/src/provider/adapters/responses.rs`, import `ReasoningEffort` and add:

```rust
/// Responses API reasoning controls.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ResponsesReasoning {
    /// Reasoning effort requested from the provider.
    pub effort: ReasoningEffort,
}
```

Then add this field to `ResponsesRequest`:

```rust
/// Optional explicit reasoning controls.
#[serde(skip_serializing_if = "Option::is_none")]
pub reasoning: Option<ResponsesReasoning>,
```

In `crates/noema-core/src/provider/adapters/openai.rs`, import `ResponsesReasoning`, extend `OpenAiProviderConfig`:

```rust
/// Optional explicit reasoning effort used only when config supplies an explicit model.
pub reasoning_effort: Option<crate::provider::ReasoningEffort>,
```

When building `ResponsesRequest`, set:

```rust
reasoning: request
    .options
    .reasoning_effort
    .or(self.config.reasoning_effort)
    .map(|effort| ResponsesReasoning { effort }),
```

Update every `OpenAiProviderConfig` test initializer with `reasoning_effort: None`.

- [ ] **Step 7: Write failing Codex gate test**

In `crates/noema-core/src/provider/adapters/codex_responses.rs`, add a test near Codex request-shape tests:

```rust
#[tokio::test]
async fn codex_omits_reasoning_effort_until_verified() {
    let (base_url, request_rx) = spawn_server(
        200,
        "event: response.output_text.delta\n\
         data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\
         \n\
         event: response.completed\n\
         data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
         \n",
    )
    .await;
    let (provider, _dir) = provider_with_tokens(base_url);

    provider
        .generate(GenerateRequest {
            conversation_id: Some("conversation:test".to_string()),
            model: Some("gpt-test".to_string()),
            input: GenerateInput::Text("Hello?".to_string()),
            instructions: Some("Reply in contract.".to_string()),
            options: GenerateOptions {
                require_noema_response: true,
                reasoning_effort: Some(crate::provider::ReasoningEffort::High),
                ..GenerateOptions::default()
            },
            tools: Vec::new(),
            tool_choice: Default::default(),
            parallel_tool_calls: false,
        })
        .await
        .expect("response");

    let captured = request_rx.await.expect("captured request");
    let body: Value = serde_json::from_str(&captured.body).expect("json body");
    assert!(body.get("reasoning").is_none(), "{body:#}");
}
```

- [ ] **Step 8: Run the Codex gate test**

Run:

```bash
cargo test -p noema-core codex_omits_reasoning_effort_until_verified --no-fail-fast
```

Expected: PASS after `ReasoningEffort` exists, because Codex request type has no `reasoning` field.

- [ ] **Step 9: Write failing config tests**

In `crates/noema-core/src/config/tests.rs`, add:

```rust
#[test]
fn generated_default_config_omits_codex_model() {
    assert!(!crate::home::DEFAULT_NOEMA_CONFIG_YAML.contains("model: gpt-5.5"));
    assert!(!crate::home::DEFAULT_NOEMA_CONFIG_YAML.contains("codex:\n  model:"));
}

#[test]
fn codex_explicit_model_requires_reasoning_effort() {
    let yaml = r#"
provider: codex
codex:
  model: gpt-5.5
"#;
    let error = load_config_from_yaml(yaml, ConfigOverrides::default())
        .expect_err("explicit reasoning-capable model without effort should fail");
    assert!(error.to_string().contains("reasoning_effort"));
}

#[test]
fn reasoning_effort_without_explicit_model_is_rejected() {
    let yaml = r#"
provider: codex
codex:
  reasoning_effort: medium
"#;
    let error = load_config_from_yaml(yaml, ConfigOverrides::default())
        .expect_err("effort without model should fail");
    assert!(error.to_string().contains("reasoning_effort"));
}

#[test]
fn codex_explicit_model_with_reasoning_effort_resolves() {
    let yaml = r#"
provider: codex
codex:
  model: gpt-5.5
  reasoning_effort: medium
"#;
    let resolved = load_config_from_yaml(yaml, ConfigOverrides::default()).expect("config");
    let crate::ProviderConfig::Codex(config) = resolved.provider else {
        panic!("expected codex provider");
    };
    assert_eq!(config.default_model.as_deref(), Some("gpt-5.5"));
    assert_eq!(
        config.reasoning_effort,
        Some(crate::provider::ReasoningEffort::Medium)
    );
}
```

If `load_config_from_yaml` does not exist, add this test helper in the test module using the existing `load_raw_config_from_sources` helper pattern:

```rust
fn load_config_from_yaml(
    yaml: &str,
    overrides: ConfigOverrides,
) -> Result<ResolvedConfig, ConfigError> {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, yaml).expect("write config");
    Config::load(Some(path), overrides)
}
```

- [ ] **Step 10: Run the failing config tests**

Run:

```bash
cargo test -p noema-core generated_default_config_omits_codex_model --no-fail-fast
cargo test -p noema-core codex_explicit_model_requires_reasoning_effort --no-fail-fast
cargo test -p noema-core reasoning_effort_without_explicit_model_is_rejected --no-fail-fast
cargo test -p noema-core codex_explicit_model_with_reasoning_effort_resolves --no-fail-fast
```

Expected: tests FAIL before config support is implemented.

- [ ] **Step 11: Implement config reasoning effort parsing and default-model removal**

In `crates/noema-core/src/config/provider.rs`, import and carry effort:

```rust
use crate::provider::ReasoningEffort;

pub struct OpenAiProviderConfig {
    ...
    pub reasoning_effort: Option<ReasoningEffort>,
}

pub struct CodexProviderConfig {
    ...
    pub reasoning_effort: Option<ReasoningEffort>,
}
```

In `crates/noema-core/src/config/raw.rs`, add `reasoning_effort` fields to `RawConfig` and `RawCodexConfig`:

```rust
reasoning_effort: Option<ReasoningEffort>,
```

Use top-level `reasoning_effort` for the portable selected-provider model override and `codex.reasoning_effort` for `codex.model`.

Update `RawCodexConfig::default()`:

```rust
model: None,
reasoning_effort: None,
```

Update `resolve_codex_config()`:

```rust
let top_level_model = non_empty_option(self.model.as_deref()).map(ToString::to_string);
let codex_model = non_empty_option(self.codex.model.as_deref()).map(ToString::to_string);
let explicit_model = top_level_model.or(codex_model);
let reasoning_effort = self.reasoning_effort.or(self.codex.reasoning_effort);
validate_reasoning_config(explicit_model.as_deref(), reasoning_effort, "codex")?;

default_model: explicit_model,
reasoning_effort,
```

For OpenAI, preserve existing default model behavior internally but only treat top-level `model` as an explicit override for effort validation:

```rust
let explicit_model = non_empty_option(self.model.as_deref()).map(ToString::to_string);
validate_reasoning_config(explicit_model.as_deref(), self.reasoning_effort, "openai")?;
let model = explicit_model.clone().unwrap_or(DEFAULT_OPENAI_MODEL.to_string());
...
reasoning_effort: self.reasoning_effort,
```

Add:

```rust
fn validate_reasoning_config(
    explicit_model: Option<&str>,
    reasoning_effort: Option<ReasoningEffort>,
    provider_kind: &str,
) -> Result<(), ConfigError> {
    if explicit_model.is_none() && reasoning_effort.is_some() {
        return Err(ConfigError::InvalidConfig {
            message: format!("{provider_kind} reasoning_effort requires an explicit model"),
        });
    }
    if explicit_model.is_some()
        && matches!(provider_kind, "codex" | "openai")
        && reasoning_effort.is_none()
    {
        return Err(ConfigError::InvalidConfig {
            message: format!("{provider_kind} explicit model requires reasoning_effort"),
        });
    }
    if reasoning_effort.is_some() && !matches!(provider_kind, "codex" | "openai") {
        return Err(ConfigError::InvalidConfig {
            message: format!("{provider_kind} does not support reasoning_effort"),
        });
    }
    Ok(())
}
```

If `ConfigError::InvalidConfig` does not exist, add a focused variant in `crates/noema-core/src/config/error.rs`:

```rust
#[error("invalid config: {message}")]
InvalidConfig { message: String },
```

In `crates/noema-core/src/config/file.rs` and `loading.rs`, add `reasoning_effort` to accepted config/env keys:

```rust
reasoning_effort: Option<ReasoningEffort>,
```

```rust
"reasoning_effort",
"codex.reasoning_effort",
```

In `crates/noema-core/src/home.rs`, remove the generated `codex.model` line and update comments:

```yaml
codex:
  base_url: https://chatgpt.com/backend-api/codex
  # Explicit model overrides must also set reasoning_effort when supported.
  # model: gpt-5.5
  # reasoning_effort: medium
  # tool_classification_model defaults to gpt-5.4-mini when unset.
  # tool_classification_model: gpt-5.4-mini
  timeout_seconds: 300
```

- [ ] **Step 12: Run Task 1 validation**

Run:

```bash
cargo test -p noema-core reasoning_effort_ --no-fail-fast
cargo test -p noema-core sends_reasoning_effort_when_configured --no-fail-fast
cargo test -p noema-core codex_omits_reasoning_effort_until_verified --no-fail-fast
cargo test -p noema-core generated_default_config_omits_codex_model --no-fail-fast
cargo test -p noema-core codex_explicit_model_requires_reasoning_effort --no-fail-fast
cargo test -p noema-core reasoning_effort_without_explicit_model_is_rejected --no-fail-fast
cargo test -p noema-core codex_explicit_model_with_reasoning_effort_resolves --no-fail-fast
```

Expected: all PASS.

- [ ] **Step 13: Commit Task 1**

Run:

```bash
git add crates/noema-core/src/provider/contract.rs \
  crates/noema-core/src/provider/adapters/responses.rs \
  crates/noema-core/src/provider/adapters/openai.rs \
  crates/noema-core/src/provider/adapters/codex_responses.rs \
  crates/noema-core/src/config/provider.rs \
  crates/noema-core/src/config/raw.rs \
  crates/noema-core/src/config/file.rs \
  crates/noema-core/src/config/loading.rs \
  crates/noema-core/src/config/error.rs \
  crates/noema-core/src/config/tests.rs \
  crates/noema-core/src/home.rs
git commit -m "feat: add reasoning effort request config"
```

Expected: commit includes only provider contract, adapter request serialization/gating, and file config/default changes.

---

### Task 2: Store And GraphQL Model Preference Validation

**Files:**
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/agent_runtime_preferences.rs`
- Modify: `crates/noema-core/src/store/auxiliary_model_preferences.rs`
- Modify: `crates/noema-core/src/store/tests.rs`
- Modify: `crates/noema-core/src/graphql/agents.rs`
- Modify: `crates/noema-core/src/graphql/web_fetch_settings.rs`
- Modify: `crates/noema-core/src/graphql/usage_settings.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`

**Interfaces:**
- Consumes: `crate::provider::ReasoningEffort`
- Produces: store preference records with `reasoning_effort: Option<ReasoningEffort>`
- Produces: GraphQL enum `ReasoningEffort`
- Produces: `AgentModelProfileOption.reasoningEfforts` and `defaultReasoningEffort`
- Produces: save validation for agent, web fetch summarizer, and progress audit preferences.

- [ ] **Step 1: Write failing store round-trip tests**

In `crates/noema-core/src/store/tests.rs`, add:

```rust
#[tokio::test]
async fn agent_runtime_preference_round_trips_reasoning_effort() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store.ensure_default_provider_account().await.expect("codex account");

    let saved = store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::High),
        })
        .await
        .expect("preference");

    assert_eq!(saved.reasoning_effort, Some(crate::provider::ReasoningEffort::High));
    let loaded = store
        .get_agent_runtime_preference("agent:primary")
        .await
        .expect("load")
        .expect("preference");
    assert_eq!(loaded.reasoning_effort, Some(crate::provider::ReasoningEffort::High));
}

#[tokio::test]
async fn auxiliary_model_preference_round_trips_reasoning_effort() {
    let store = test_store().await;
    let account = store.ensure_default_provider_account().await.expect("codex account");

    let saved = store
        .upsert_auxiliary_model_preference(crate::NewAuxiliaryModelPreference {
            task_id: crate::WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::Low),
        })
        .await
        .expect("preference");

    assert_eq!(saved.reasoning_effort, Some(crate::provider::ReasoningEffort::Low));
}
```

- [ ] **Step 2: Run failing store tests**

Run:

```bash
cargo test -p noema-core agent_runtime_preference_round_trips_reasoning_effort --no-fail-fast
cargo test -p noema-core auxiliary_model_preference_round_trips_reasoning_effort --no-fail-fast
```

Expected: FAIL because preference structs do not have `reasoning_effort`.

- [ ] **Step 3: Implement store persistence**

In `crates/noema-core/src/store/schema.rs`, add fields:

```sql
DEFINE FIELD OVERWRITE reasoning_effort ON TABLE agent_runtime_preferences TYPE option<string> ASSERT $value = NONE OR $value INSIDE ['none', 'minimal', 'low', 'medium', 'high', 'xhigh'];
DEFINE FIELD OVERWRITE reasoning_effort ON TABLE auxiliary_model_preferences TYPE option<string> ASSERT $value = NONE OR $value INSIDE ['none', 'minimal', 'low', 'medium', 'high', 'xhigh'];
```

Place each field immediately after `model_profile`.

In both store modules:

```rust
use crate::provider::ReasoningEffort;
```

Add `reasoning_effort: Option<ReasoningEffort>` to `New...Preference`, `...PreferenceRecord`, and row structs.

Update SELECTs:

```sql
SELECT agent_id, provider_kind, provider_account_id, model_profile, reasoning_effort
```

```sql
SELECT task_id, provider_kind, provider_account_id, model_profile, reasoning_effort
```

Update UPSERTs:

```sql
reasoning_effort = $reasoning_effort,
```

Bind:

```rust
.bind(("reasoning_effort", preference.reasoning_effort))
```

Update record conversion:

```rust
reasoning_effort: row.reasoning_effort,
```

Fix every existing `NewAgentRuntimePreference` and `NewAuxiliaryModelPreference` initializer by adding:

```rust
reasoning_effort: None,
```

- [ ] **Step 4: Run store tests**

Run:

```bash
cargo test -p noema-core agent_runtime_preference_round_trips_reasoning_effort --no-fail-fast
cargo test -p noema-core auxiliary_model_preference_round_trips_reasoning_effort --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Write failing GraphQL validation tests**

In `crates/noema-core/src/graphql/schema.rs`, add three tests:

```rust
#[tokio::test]
async fn agents_query_exposes_profile_reasoning_efforts() {
    use crate::store::tests::test_store;
    let store = test_store().await;
    let account = store.ensure_default_provider_account().await.expect("account");
    store
        .update_provider_account_metadata(
            &account.provider_account_id,
            serde_json::json!({
                "profiles": [{
                    "id": "gpt-5.5",
                    "label": "GPT-5.5",
                    "reasoning_efforts": ["low", "medium", "high"],
                    "default_reasoning_effort": "medium"
                }]
            }),
        )
        .await
        .expect("metadata");

    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let response = schema
        .execute(r#"
          query {
            agents {
              modelOptions {
                profiles {
                  id
                  reasoningEfforts
                  defaultReasoningEffort
                }
              }
            }
          }
        "#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    let profile = &data["agents"][0]["modelOptions"][0]["profiles"][0];
    assert_eq!(profile["reasoningEfforts"], serde_json::json!(["LOW", "MEDIUM", "HIGH"]));
    assert_eq!(profile["defaultReasoningEffort"], "MEDIUM");
}

#[tokio::test]
async fn save_agent_model_preference_requires_reasoning_for_reasoning_profile() {
    let (schema, account_id) = schema_with_reasoning_codex_profile().await;
    let response = schema
        .execute(format!(
            r#"
            mutation {{
              saveAgentModelPreference(input: {{
                agentId: "agent:primary",
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5"
              }}) {{
                modelProfile
              }}
            }}
            "#
        ))
        .await;
    assert!(!response.errors.is_empty());
    assert!(response.errors[0].message.contains("reasoning"));
}

#[tokio::test]
async fn save_agent_model_preference_persists_reasoning_effort() {
    let (schema, account_id) = schema_with_reasoning_codex_profile().await;
    let response = schema
        .execute(format!(
            r#"
            mutation {{
              saveAgentModelPreference(input: {{
                agentId: "agent:primary",
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5",
                reasoningEffort: HIGH
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    assert_eq!(data["saveAgentModelPreference"]["reasoningEffort"], "HIGH");
}
```

Add this helper in the same test module:

```rust
async fn schema_with_reasoning_codex_profile() -> (Schema<Query, Mutation, Subscription>, String) {
    use crate::store::tests::test_store;
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store.ensure_default_provider_account().await.expect("account");
    store
        .update_provider_account_status(
            &account.provider_account_id,
            crate::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("status");
    store
        .update_provider_account_metadata(
            &account.provider_account_id,
            serde_json::json!({
                "profiles": [{
                    "id": "gpt-5.5",
                    "label": "GPT-5.5",
                    "reasoning_efforts": ["low", "medium", "high"],
                    "default_reasoning_effort": "medium"
                }]
            }),
        )
        .await
        .expect("metadata");
    let account_id = account.provider_account_id;
    (build_schema(GraphqlState::for_tests_with_store(store)), account_id)
}
```

If the concrete schema type aliases differ, use the existing test helpers in `schema.rs` and return the same schema type those tests use.

- [ ] **Step 6: Run failing GraphQL tests**

Run:

```bash
cargo test -p noema-core agents_query_exposes_profile_reasoning_efforts --no-fail-fast
cargo test -p noema-core save_agent_model_preference_requires_reasoning_for_reasoning_profile --no-fail-fast
cargo test -p noema-core save_agent_model_preference_persists_reasoning_effort --no-fail-fast
```

Expected: FAIL because GraphQL has no reasoning fields.

- [ ] **Step 7: Implement GraphQL enum, profile parsing, and validation**

In `crates/noema-core/src/graphql/agents.rs`, add:

```rust
use async_graphql::Enum;
use crate::provider::ReasoningEffort;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ReasoningEffort")]
pub enum GraphqlReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}
```

Implement conversions:

```rust
impl From<ReasoningEffort> for GraphqlReasoningEffort {
    fn from(value: ReasoningEffort) -> Self {
        match value {
            ReasoningEffort::None => Self::None,
            ReasoningEffort::Minimal => Self::Minimal,
            ReasoningEffort::Low => Self::Low,
            ReasoningEffort::Medium => Self::Medium,
            ReasoningEffort::High => Self::High,
            ReasoningEffort::XHigh => Self::Xhigh,
        }
    }
}

impl From<GraphqlReasoningEffort> for ReasoningEffort {
    fn from(value: GraphqlReasoningEffort) -> Self {
        match value {
            GraphqlReasoningEffort::None => Self::None,
            GraphqlReasoningEffort::Minimal => Self::Minimal,
            GraphqlReasoningEffort::Low => Self::Low,
            GraphqlReasoningEffort::Medium => Self::Medium,
            GraphqlReasoningEffort::High => Self::High,
            GraphqlReasoningEffort::Xhigh => Self::XHigh,
        }
    }
}
```

Extend GraphQL structs:

```rust
pub struct GraphqlAgentModelPreference {
    ...
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
}

pub struct GraphqlAgentModelProfileOption {
    ...
    pub reasoning_efforts: Vec<GraphqlReasoningEffort>,
    pub default_reasoning_effort: Option<GraphqlReasoningEffort>,
}

pub struct GraphqlSaveAgentModelPreferenceInput {
    ...
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
}
```

Update `profile_options` default values:

```rust
reasoning_efforts: Vec::new(),
default_reasoning_effort: None,
```

Update `metadata_profiles()` to parse:

```rust
let reasoning_efforts = profile
    .get("reasoning_efforts")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(|value| value.as_str())
    .filter_map(reasoning_effort_from_metadata)
    .map(GraphqlReasoningEffort::from)
    .collect::<Vec<_>>();
let default_reasoning_effort = profile
    .get("default_reasoning_effort")
    .and_then(Value::as_str)
    .and_then(reasoning_effort_from_metadata)
    .map(GraphqlReasoningEffort::from)
    .filter(|effort| reasoning_efforts.contains(effort));
```

Add helper:

```rust
fn reasoning_effort_from_metadata(value: &str) -> Option<ReasoningEffort> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Some(ReasoningEffort::None),
        "minimal" => Some(ReasoningEffort::Minimal),
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "xhigh" => Some(ReasoningEffort::XHigh),
        _ => None,
    }
}
```

Add validation helper:

```rust
fn validate_reasoning_effort_for_profile(
    profile: &GraphqlAgentModelProfileOption,
    reasoning_effort: Option<GraphqlReasoningEffort>,
) -> Result<Option<ReasoningEffort>> {
    if profile.reasoning_efforts.is_empty() {
        if reasoning_effort.is_some() {
            return Err(async_graphql::Error::new(
                "reasoning effort is not available for selected model profile",
            ));
        }
        return Ok(None);
    }
    let Some(reasoning_effort) = reasoning_effort else {
        return Err(async_graphql::Error::new(
            "reasoning effort is required for selected model profile",
        ));
    };
    if !profile.reasoning_efforts.contains(&reasoning_effort) {
        return Err(async_graphql::Error::new(
            "reasoning effort is not available for selected model profile",
        ));
    }
    Ok(Some(reasoning_effort.into()))
}
```

Use it in `save_agent_model_preference`:

```rust
let profile = profiles
    .iter()
    .find(|profile| profile.id == input.model_profile)
    .ok_or_else(|| async_graphql::Error::new("model profile is not available for provider"))?;
let reasoning_effort = validate_reasoning_effort_for_profile(profile, input.reasoning_effort)?;
```

Pass `reasoning_effort` into `NewAgentRuntimePreference` and GraphQL output.

Repeat the same input/output field and validation in `web_fetch_settings.rs` and `usage_settings.rs`, reusing the helper by making it `pub(super)`.

- [ ] **Step 8: Run GraphQL tests**

Run:

```bash
cargo test -p noema-core agents_query_exposes_profile_reasoning_efforts --no-fail-fast
cargo test -p noema-core save_agent_model_preference_requires_reasoning_for_reasoning_profile --no-fail-fast
cargo test -p noema-core save_agent_model_preference_persists_reasoning_effort --no-fail-fast
```

Expected: PASS.

- [ ] **Step 9: Add auxiliary GraphQL coverage**

Add tests to `schema.rs`:

```rust
#[tokio::test]
async fn save_web_fetch_summarizer_preference_persists_reasoning_effort() {
    let (schema, account_id) = schema_with_reasoning_codex_profile().await;
    let response = schema
        .execute(format!(
            r#"
            mutation {{
              saveWebFetchSummarizerPreference(input: {{
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5",
                reasoningEffort: LOW
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    assert_eq!(data["saveWebFetchSummarizerPreference"]["reasoningEffort"], "LOW");
}

#[tokio::test]
async fn save_tool_progress_audit_preference_persists_reasoning_effort() {
    let (schema, account_id) = schema_with_reasoning_codex_profile().await;
    let response = schema
        .execute(format!(
            r#"
            mutation {{
              saveToolProgressAuditPreference(input: {{
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5",
                reasoningEffort: MEDIUM
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    assert_eq!(data["saveToolProgressAuditPreference"]["reasoningEffort"], "MEDIUM");
}
```

- [ ] **Step 10: Run Task 2 validation**

Run:

```bash
cargo test -p noema-core agent_runtime_preference_round_trips_reasoning_effort --no-fail-fast
cargo test -p noema-core auxiliary_model_preference_round_trips_reasoning_effort --no-fail-fast
cargo test -p noema-core agents_query_exposes_profile_reasoning_efforts --no-fail-fast
cargo test -p noema-core save_agent_model_preference_requires_reasoning_for_reasoning_profile --no-fail-fast
cargo test -p noema-core save_agent_model_preference_persists_reasoning_effort --no-fail-fast
cargo test -p noema-core save_web_fetch_summarizer_preference_persists_reasoning_effort --no-fail-fast
cargo test -p noema-core save_tool_progress_audit_preference_persists_reasoning_effort --no-fail-fast
```

Expected: all PASS.

- [ ] **Step 11: Commit Task 2**

Run:

```bash
git add crates/noema-core/src/store/schema.rs \
  crates/noema-core/src/store/agent_runtime_preferences.rs \
  crates/noema-core/src/store/auxiliary_model_preferences.rs \
  crates/noema-core/src/store/tests.rs \
  crates/noema-core/src/graphql/agents.rs \
  crates/noema-core/src/graphql/web_fetch_settings.rs \
  crates/noema-core/src/graphql/usage_settings.rs \
  crates/noema-core/src/graphql/schema.rs
git commit -m "feat: persist reasoning effort model preferences"
```

Expected: commit includes only store and GraphQL preference support.

---

### Task 3: Runtime Reasoning Effort Threading

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/actor.rs`
- Modify: `crates/noema-core/src/daemon/runtime/conversation_state.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/runtime/context_compaction.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Modify: `crates/noema-core/src/daemon/runtime/progress_audit.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

**Interfaces:**
- Consumes: store preference `reasoning_effort`
- Produces: all explicit model requests pass `GenerateOptions.reasoning_effort`
- Produces: no request passes reasoning effort when no explicit preference exists.

- [ ] **Step 1: Write failing runtime request capture tests**

In `crates/noema-core/src/daemon/tests.rs`, add a test using the existing runtime mock provider pattern:

```rust
#[tokio::test]
async fn primary_agent_runtime_preference_supplies_reasoning_effort() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::High),
        })
        .await
        .expect("preference");

    let codex_provider = Arc::new(CapturingProvider::default());
    let runtime =
        CodexRuntimeHandle::spawn_with_provider_kind(codex_provider.clone(), store, "codex")
            .await
            .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    let requests = codex_provider.requests.lock().expect("codex requests");
    assert_eq!(
        requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        Some(crate::provider::ReasoningEffort::High)
    );
}
```

Add a second test for defaults:

```rust
#[tokio::test]
async fn primary_agent_default_provider_sends_no_reasoning_effort() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let codex_provider = Arc::new(CapturingProvider::default());
    let runtime =
        CodexRuntimeHandle::spawn_with_provider_kind(codex_provider.clone(), store, "codex")
            .await
            .expect("runtime");

    let started = runtime
        .start_primary_conversation(None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    runtime.shutdown().await;

    let requests = codex_provider.requests.lock().expect("codex requests");
    assert_eq!(
        requests
            .last()
            .and_then(|request| request.options.reasoning_effort),
        None
    );
}
```

- [ ] **Step 2: Run failing runtime tests**

Run:

```bash
cargo test -p noema-core primary_agent_runtime_preference_supplies_reasoning_effort --no-fail-fast
cargo test -p noema-core primary_agent_default_provider_sends_no_reasoning_effort --no-fail-fast
```

Expected: first FAIL because runtime drops preference effort; second may PASS after Task 1 defaults.

- [ ] **Step 3: Extend active conversation selection**

In `crates/noema-core/src/daemon/runtime/conversation_state.rs`, add to `ConversationProviderSelection`:

```rust
pub(super) reasoning_effort: Option<crate::provider::ReasoningEffort>,
```

Update no-preference branch:

```rust
reasoning_effort: None,
```

Update preference branch:

```rust
reasoning_effort: preference.reasoning_effort,
```

In `crates/noema-core/src/daemon/runtime/actor.rs`, add to `ActiveConversation`:

```rust
pub(in crate::daemon) reasoning_effort: Option<crate::provider::ReasoningEffort>,
```

Set it during hydration:

```rust
reasoning_effort: selection.reasoning_effort,
```

- [ ] **Step 4: Thread active conversation effort into turn requests**

In `crates/noema-core/src/daemon/runtime/turn.rs`, every `GenerateOptions` for the active conversation should include:

```rust
reasoning_effort: conversation.reasoning_effort,
```

and continuation/finalization requests that use `turn.model` should carry a field added to the turn state:

```rust
reasoning_effort: conversation.reasoning_effort,
```

For any turn struct that stores model/provider information, add:

```rust
reasoning_effort: Option<crate::provider::ReasoningEffort>,
```

Then set continuation request options:

```rust
reasoning_effort: turn.reasoning_effort,
```

Do not set reasoning effort for local-only audit/tool activity rows.

- [ ] **Step 5: Thread compaction effort**

In `crates/noema-core/src/daemon/runtime/context_compaction.rs`, add to `ContextCompactionRequest`:

```rust
pub(super) reasoning_effort: Option<crate::provider::ReasoningEffort>,
```

When building compaction `GenerateRequest`, set:

```rust
options: GenerateOptions {
    max_output_tokens: Some(...),
    reasoning_effort: request.reasoning_effort,
    ..GenerateOptions::default()
},
```

Update every call site in `turn.rs` to pass `conversation.reasoning_effort`.

- [ ] **Step 6: Thread web fetch summarizer effort**

In `crates/noema-core/src/daemon/runtime/local_tools.rs`, add to `FetchRuntimeContext`:

```rust
summarizer_reasoning_effort: Option<crate::provider::ReasoningEffort>,
```

For saved preference:

```rust
summarizer_reasoning_effort: preference.reasoning_effort,
```

For default:

```rust
summarizer_reasoning_effort: None,
```

When constructing the summarizer `GenerateRequest`, set:

```rust
options: GenerateOptions {
    reasoning_effort: context.summarizer_reasoning_effort,
    ..GenerateOptions::default()
},
```

Add a focused unit test in `local_tools.rs`:

```rust
#[tokio::test]
async fn web_fetch_runtime_context_uses_saved_summarizer_reasoning_effort() {
    let store = crate::store::tests::test_store().await;
    let account = store.ensure_default_provider_account().await.expect("account");
    store
        .upsert_auxiliary_model_preference(NewAuxiliaryModelPreference {
            task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::Low),
        })
        .await
        .expect("preference");
    let actor = test_actor_with_codex_provider(store.clone()).await;
    let context = actor.web_fetch_runtime_context().await.expect("context");
    assert_eq!(
        context.summarizer_reasoning_effort,
        Some(crate::provider::ReasoningEffort::Low)
    );
}
```

Use the existing local test actor setup helper if it has a different name.

- [ ] **Step 7: Thread progress audit effort**

In `crates/noema-core/src/daemon/runtime/progress_audit.rs`, add to `ProgressAuditModel`:

```rust
reasoning_effort: Option<crate::provider::ReasoningEffort>,
```

For saved preference:

```rust
reasoning_effort: preference.reasoning_effort,
```

For default:

```rust
reasoning_effort: None,
```

In the audit request:

```rust
options: GenerateOptions {
    require_noema_response: false,
    reasoning_effort: audit_model.reasoning_effort,
    ..GenerateOptions::default()
},
```

Add a focused test using the existing progress audit provider capture helper:

```rust
#[tokio::test]
async fn progress_audit_uses_saved_reasoning_effort() {
    let store = crate::store::tests::test_store().await;
    let account = store.ensure_default_provider_account().await.expect("account");
    store
        .upsert_auxiliary_model_preference(crate::NewAuxiliaryModelPreference {
            task_id: crate::store::TOOL_PROGRESS_AUDIT_TASK_ID.to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "gpt-5.5".to_string(),
            reasoning_effort: Some(crate::provider::ReasoningEffort::Medium),
        })
        .await
        .expect("preference");

    let provider = Arc::new(ProgressAuditTestProvider::new_valid_continue());
    let actor = CodexRuntimeActor::new(
        "codex".to_string(),
        HashMap::from([("codex".to_string(), provider.clone() as Arc<dyn RuntimeModelProvider>)]),
        store,
        None,
    )
    .await
    .expect("actor");

    actor.run_progress_audit(&ContinuationProgressDigest::default()).await.expect("audit");
    assert_eq!(
        provider.last_request().options.reasoning_effort,
        Some(crate::provider::ReasoningEffort::Medium)
    );
}
```

Adapt provider helper names to the existing test scaffolding in `progress_audit.rs`.

- [ ] **Step 8: Run Task 3 validation**

Run:

```bash
cargo test -p noema-core primary_agent_runtime_preference_supplies_reasoning_effort --no-fail-fast
cargo test -p noema-core primary_agent_default_provider_sends_no_reasoning_effort --no-fail-fast
cargo test -p noema-core web_fetch_runtime_context_uses_saved_summarizer_reasoning_effort --no-fail-fast
cargo test -p noema-core progress_audit_uses_saved_reasoning_effort --no-fail-fast
```

Expected: all PASS.

- [ ] **Step 9: Commit Task 3**

Run:

```bash
git add crates/noema-core/src/daemon/runtime/actor.rs \
  crates/noema-core/src/daemon/runtime/conversation_state.rs \
  crates/noema-core/src/daemon/runtime/turn.rs \
  crates/noema-core/src/daemon/runtime/context_compaction.rs \
  crates/noema-core/src/daemon/runtime/local_tools.rs \
  crates/noema-core/src/daemon/runtime/progress_audit.rs \
  crates/noema-core/src/daemon/tests.rs
git commit -m "feat: apply reasoning effort to model requests"
```

Expected: commit includes only runtime request threading and tests.

---

### Task 4: Frontend Model Config Editor

**Files:**
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Regenerate: `crates/noema-core/web/src/generated/graphql.ts`
- Regenerate: `crates/noema-core/web/src/generated/schema.graphql`
- Modify: `crates/noema-core/web/src/components/settings/modelPreferenceTypes.ts`
- Modify: `crates/noema-core/web/src/components/settings/modelPreferenceMetadata.ts`
- Modify: `crates/noema-core/web/src/components/settings/ModelPreferenceSelect.tsx`
- Modify: `crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/components/settings/WebSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/components/settings/UsageSettingsPaneContent.tsx`

**Interfaces:**
- Consumes: GraphQL `ReasoningEffort`, `reasoningEfforts`, `defaultReasoningEffort`, and preference `reasoningEffort`
- Produces: save inputs include `reasoningEffort` when required and `null` when unsupported.

- [ ] **Step 1: Update GraphQL operations**

In `crates/noema-core/web/src/graphql/operations.ts`, add `reasoningEffort` to every `modelPreference` selection and mutation response:

```graphql
modelPreference {
  providerKind
  providerAccountId
  modelProfile
  reasoningEffort
}
```

Add fields to every `profiles` selection:

```graphql
profiles {
  id
  label
  disabledReason
  reasoningEfforts
  defaultReasoningEffort
}
```

Add `reasoningEffort` to save mutation outputs:

```graphql
saveAgentModelPreference(input: $input) {
  providerKind
  providerAccountId
  modelProfile
  reasoningEffort
}
```

Repeat for web fetch summarizer and progress audit save mutations.

- [ ] **Step 2: Regenerate GraphQL types**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: `src/generated/graphql.ts` includes `ReasoningEffort`, `reasoningEffort`, `reasoningEfforts`, and `defaultReasoningEffort`.

- [ ] **Step 3: Update frontend model preference types**

In `crates/noema-core/web/src/components/settings/modelPreferenceTypes.ts`, import generated enum:

```ts
import type { ReasoningEffort } from "@/generated/graphql";
```

Extend types:

```ts
export type ModelProfileOption = {
  id: string;
  label: string;
  disabledReason?: string | null;
  reasoningEfforts: readonly ReasoningEffort[];
  defaultReasoningEffort?: ReasoningEffort | null;
};

export type ModelPreference = {
  providerKind: string;
  providerAccountId: string;
  modelProfile: string;
  reasoningEffort?: ReasoningEffort | null;
};

export type ModelPreferenceSaveInput = {
  providerAccountId: string;
  modelProfile: string;
  reasoningEffort?: ReasoningEffort | null;
};
```

- [ ] **Step 4: Replace `ModelPreferenceSelect` with a model config editor**

In `crates/noema-core/web/src/components/settings/ModelPreferenceSelect.tsx`, keep the exported function name to minimize call-site churn, but add reasoning state:

```tsx
const selectedProvider = providerOptions.find(
  (provider) => provider.providerAccountId === selection.providerAccountId
);
const selectedProfile = selectedProvider?.profiles.find(
  (profile) => profile.id === selection.modelProfile
);
const reasoningEfforts = selectedProfile?.reasoningEfforts ?? [];
const requiresReasoning = reasoningEfforts.length > 0;
const selectedReasoningEffort =
  selection.reasoningEffort ??
  (requiresReasoning ? selectedProfile?.defaultReasoningEffort ?? null : null);
```

Change the model dropdown `onChange` to update local selection instead of immediately saving when reasoning is required:

```tsx
onChange={(event) => {
  const nextSelection = parseModelOptionValue(event.target.value);
  if (!nextSelection) return;
  const provider = providerOptions.find(
    (candidate) => candidate.providerAccountId === nextSelection.providerAccountId
  );
  const profile = provider?.profiles.find(
    (candidate) => candidate.id === nextSelection.modelProfile
  );
  const efforts = profile?.reasoningEfforts ?? [];
  if (efforts.length === 0) {
    void onSave({ ...nextSelection, reasoningEffort: null });
    return;
  }
  const effort = profile?.defaultReasoningEffort ?? efforts[0] ?? null;
  if (effort) {
    void onSave({ ...nextSelection, reasoningEffort: effort });
  }
}}
```

Render a second select only when needed:

```tsx
{requiresReasoning ? (
  <label {...stylex.props(styles.selector)}>
    <span {...stylex.props(styles.fieldLabel)}>Reasoning</span>
    <select
      {...stylex.props(styles.select)}
      aria-label={`${ariaLabel} reasoning`}
      value={selectedReasoningEffort ?? ""}
      disabled={isDisabled || saving}
      onChange={(event) => {
        const reasoningEffort = event.target.value as ReasoningEffort;
        void onSave({
          providerAccountId: selection.providerAccountId,
          modelProfile: selection.modelProfile,
          reasoningEffort
        });
      }}
    >
      {reasoningEfforts.map((effort) => (
        <option key={effort} value={effort}>
          {reasoningEffortLabel(effort)}
        </option>
      ))}
    </select>
    <span {...stylex.props(styles.helpText)}>Required for this custom model.</span>
  </label>
) : null}
```

Add helper:

```ts
function reasoningEffortLabel(value: ReasoningEffort): string {
  switch (value) {
    case "NONE":
      return "None";
    case "MINIMAL":
      return "Minimal";
    case "LOW":
      return "Low";
    case "MEDIUM":
      return "Medium";
    case "HIGH":
      return "High";
    case "XHIGH":
      return "XHigh";
  }
}
```

If generated enum values are not string unions, adjust the cases to the generated `ReasoningEffort` enum members.

- [ ] **Step 5: Ensure call sites pass through reasoning effort**

In `AgentsSettingsPaneContent.tsx`, `WebSettingsPaneContent.tsx`, and `UsageSettingsPaneContent.tsx`, pass the complete `ModelPreferenceSaveInput` through unchanged so `reasoningEffort` reaches each save mutation.

If a call site constructs input manually, update it:

```tsx
onSave={(input) =>
  onSaveModelPreference({
    agentId: agent.agentId,
    providerAccountId: input.providerAccountId,
    modelProfile: input.modelProfile,
    reasoningEffort: input.reasoningEffort ?? null
  })
}
```

- [ ] **Step 6: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS.

- [ ] **Step 7: Commit Task 4**

Run:

```bash
git add crates/noema-core/web/src/graphql/operations.ts \
  crates/noema-core/web/src/generated/graphql.ts \
  crates/noema-core/web/src/generated/schema.graphql \
  crates/noema-core/web/src/components/settings/modelPreferenceTypes.ts \
  crates/noema-core/web/src/components/settings/modelPreferenceMetadata.ts \
  crates/noema-core/web/src/components/settings/ModelPreferenceSelect.tsx \
  crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx \
  crates/noema-core/web/src/components/settings/WebSettingsPaneContent.tsx \
  crates/noema-core/web/src/components/settings/UsageSettingsPaneContent.tsx
git commit -m "feat: configure reasoning effort in settings"
```

Expected: commit includes only frontend GraphQL/codegen and settings model config UI.

---

### Task 5: Final Validation And Handoff

**Files:**
- Inspect: git state and all staged/unstaged changes.
- Modify: none unless validation finds a defect in files changed by this plan.

**Interfaces:**
- Consumes: Tasks 1-4 commits.
- Produces: validated branch state and clear handoff.

- [ ] **Step 1: Run cheap repository checks**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: `git diff --check` exits 0. `git status` may show unrelated concurrent work; do not stage it.

- [ ] **Step 2: Run Rust validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all PASS. If `cargo fmt --all --check` fails only in unrelated concurrent files, do not run `cargo fmt --all`; report the exact unrelated files.

- [ ] **Step 3: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: PASS.

- [ ] **Step 4: Inspect final git state**

Run:

```bash
git status --short --branch
git log --oneline --decorate -8
```

Expected: plan commits are visible at the top or near top of `main`. Remaining dirty files, if any, are reported explicitly.

- [ ] **Step 5: Completion handoff**

Report:

```text
Implemented custom reasoning model configs.

Commits:
- <sha> feat: add reasoning effort request config
- <sha> feat: persist reasoning effort model preferences
- <sha> feat: apply reasoning effort to model requests
- <sha> feat: configure reasoning effort in settings

Validation:
- cargo fmt --all --check: PASS
- cargo check --workspace: PASS
- cargo clippy --workspace --all-targets -- -D warnings: PASS
- cargo test --workspace --no-fail-fast: PASS
- cd crates/noema-core/web && bun run lint: PASS

Remaining dirty files:
- <list or "none">
```

If any validation failed due to unrelated concurrent work, replace the relevant PASS line with the exact failure and file list.

---

## Self-Review

- Spec coverage: the plan covers provider-owned default config, explicit model plus required reasoning effort, store-backed agent/web-fetch/progress-audit preferences, provider metadata as the reasoning support authority, OpenAI serialization, Codex gated behavior, runtime threading, frontend settings, prompt-cache implications, and validation.
- Placeholder scan: no `TBD`, `TODO`, or "implement later" placeholders are intentionally present.
- Type consistency: `ReasoningEffort`, `reasoning_effort`, `reasoningEffort`, `reasoningEfforts`, and `defaultReasoningEffort` are used consistently across Rust, store, GraphQL, and TypeScript layers.
