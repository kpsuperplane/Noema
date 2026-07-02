# System Errors Log Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an uncapped, unredacted developer diagnostic `errors.log` under `NOEMA_HOME` for system-level failures that require Noema code, prompt, schema, protocol, parser, or adapter changes.

**Architecture:** Add a focused `system_errors` module with a cloneable append-only JSONL logger. Wire the logger through runtime, provider, MCP setup, and Capability Gateway boundaries so parse/invariant failures can record full raw payloads while preserving existing user-facing errors.

**Tech Stack:** Rust, `serde`, `serde_json`, `time`, typed `thiserror` errors, existing Noema provider/MCP/runtime modules, Rust unit tests.

---

## File Map

- Modify `Cargo.toml`
  - Add the `time` workspace dependency for UTC RFC3339 timestamps.
- Modify `crates/noema-core/Cargo.toml`
  - Depend on `time.workspace = true`.
- Modify `crates/noema-core/src/paths.rs`
  - Add `NoemaPaths::errors_log_path()`.
  - Add a path resolution unit test.
- Create `crates/noema-core/src/system_errors.rs`
  - Define `SystemErrorLogger`, `SystemErrorEvent`, `SystemErrorWriteError`, category constants, JSONL append behavior, and test helpers.
- Modify `crates/noema-core/src/lib.rs`
  - Export the new module and core types.
- Modify `crates/noema-core/src/runtime_host.rs`
  - Construct a logger after home initialization and pass it to the runtime.
- Modify `crates/noema-core/src/daemon/runtime/handle.rs`
  - Carry the logger into provider construction and runtime actor startup.
- Modify `crates/noema-core/src/daemon/runtime/actor.rs`
  - Store the logger on `CodexRuntimeActor`.
- Modify `crates/noema-core/src/daemon/runtime/turn.rs`
  - Log the onboarding no-assistant-text invariant.
- Modify `crates/noema-core/src/daemon/runtime/local_tools.rs`
  - Pass the logger to `CapabilityGateway`.
- Modify `crates/noema-core/src/provider/adapters/codex_responses.rs`
  - Store the logger in Codex provider config/provider and log strict envelope failures with raw text.
- Modify `crates/noema-core/src/provider/adapters/openai.rs`
  - Store the logger in OpenAI provider config/provider and log strict envelope failures with raw text.
- Modify `crates/noema-core/src/provider/adapters/responses.rs`
  - Accept a diagnostic context for HTTP body and parsed response failures.
- Modify `crates/noema-core/src/provider/adapters/sse.rs`
  - Log malformed SSE UTF-8/JSON/output-item failures with raw event/chunk data.
- Modify `crates/noema-core/src/mcp/client.rs`
  - Add logging-aware `parse_tools_list_result_with_diagnostics`.
- Modify `crates/noema-core/src/mcp/setup.rs`
  - Pass diagnostics into MCP setup discovery.
- Modify `crates/noema-core/src/mcp/http.rs`
  - Store diagnostics on HTTP transports and log malformed SSE/JSON-RPC/tool list payloads.
- Modify `crates/noema-core/src/mcp/stdio.rs`
  - Store diagnostics on stdio transports and log malformed SDK metadata/tool-call serialization.
- Modify `crates/noema-core/src/capability/gateway.rs`
  - Accept diagnostics and log malformed runtime MCP tool call failures while preserving model-visible sanitized errors.
- Modify focused tests in the touched modules.

## Task 1: Add Logger Primitive And Path

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/noema-core/Cargo.toml`
- Modify: `crates/noema-core/src/paths.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Create: `crates/noema-core/src/system_errors.rs`

- [ ] **Step 1: Add failing path test**

Add this test to `crates/noema-core/src/paths.rs` inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn errors_log_path_lives_at_noema_root() {
    let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

    assert_eq!(paths.errors_log_path(), PathBuf::from("/tmp/noema/errors.log"));
}
```

- [ ] **Step 2: Run path test to verify it fails**

Run:

```bash
cargo test -p noema-core paths::tests::errors_log_path_lives_at_noema_root --no-fail-fast
```

Expected: fail with a missing `errors_log_path` method.

- [ ] **Step 3: Add `NoemaPaths::errors_log_path`**

Add this method in `impl NoemaPaths` in `crates/noema-core/src/paths.rs` after `config_path`:

```rust
/// Path to the developer diagnostic system error log.
#[must_use]
pub fn errors_log_path(&self) -> PathBuf {
    self.root.join("errors.log")
}
```

- [ ] **Step 4: Run path test to verify it passes**

Run:

```bash
cargo test -p noema-core paths::tests::errors_log_path_lives_at_noema_root --no-fail-fast
```

Expected: pass.

- [ ] **Step 5: Add `time` dependency**

Modify the workspace dependencies in `Cargo.toml`:

```toml
time = { version = "0.3", features = ["formatting", "macros"] }
```

Modify `crates/noema-core/Cargo.toml` dependencies:

```toml
time.workspace = true
```

- [ ] **Step 6: Create failing logger tests**

Create `crates/noema-core/src/system_errors.rs` with this initial test module at the bottom after the production code added in the next step:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn appends_jsonl_events_without_overwriting() {
        let dir = tempfile::tempdir().expect("temp dir");
        let logger = SystemErrorLogger::new(dir.path().join("errors.log"));

        logger
            .append(SystemErrorEvent::new(
                SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE,
                "first failure",
            )
            .with_context(json!({"provider_kind": "codex"}))
            .with_raw(json!({"provider_text": "line one\nline two"})))
            .expect("first append");
        logger
            .append(SystemErrorEvent::new(
                SYSTEM_ERROR_MCP_MALFORMED_RESPONSE,
                "second failure",
            )
            .with_context(json!({"mcp_server_id": "mcp:test"}))
            .with_error_chain(["outer".to_string(), "inner".to_string()])
            .with_raw(json!({"result": {"tools": "bad"}})))
            .expect("second append");

        let events = read_system_error_events(dir.path().join("errors.log")).expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["category"], SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE);
        assert_eq!(events[0]["severity"], "error");
        assert_eq!(events[0]["context"]["provider_kind"], "codex");
        assert_eq!(events[0]["raw"]["provider_text"], "line one\nline two");
        assert_eq!(events[1]["category"], SYSTEM_ERROR_MCP_MALFORMED_RESPONSE);
        assert_eq!(events[1]["error_chain"], json!(["outer", "inner"]));
        assert_eq!(events[1]["raw"]["result"]["tools"], "bad");
    }

    #[test]
    fn try_append_swallows_write_failures() {
        let dir = tempfile::tempdir().expect("temp dir");
        let logger = SystemErrorLogger::new(dir.path());

        logger.try_append(SystemErrorEvent::new(
            SYSTEM_ERROR_RUNTIME_INVARIANT,
            "cannot append to a directory",
        ));
    }
}
```

- [ ] **Step 7: Implement logger module**

Put this production code at the top of `crates/noema-core/src/system_errors.rs`:

```rust
//! Developer diagnostic system error logging.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// Provider output or transport body was malformed.
pub const SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE: &str = "provider_malformed_response";
/// MCP metadata or tool-call payload was malformed.
pub const SYSTEM_ERROR_MCP_MALFORMED_RESPONSE: &str = "mcp_malformed_response";
/// Runtime state reached an invariant violation.
pub const SYSTEM_ERROR_RUNTIME_INVARIANT: &str = "runtime_invariant_violation";
/// Store-backed state violated a closed Noema schema assumption.
pub const SYSTEM_ERROR_STORE_INVARIANT: &str = "store_invariant_violation";

/// Append-only developer diagnostic system error logger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemErrorLogger {
    path: PathBuf,
}

impl SystemErrorLogger {
    /// Create a logger for an explicit `errors.log` path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Create a logger from resolved Noema paths.
    #[must_use]
    pub fn from_paths(paths: &crate::NoemaPaths) -> Self {
        Self::new(paths.errors_log_path())
    }

    /// Return the target log path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one diagnostic event and report filesystem/serialization errors.
    ///
    /// # Errors
    ///
    /// Returns [`SystemErrorWriteError`] when the parent directory cannot be
    /// created, the event cannot be serialized, or the log line cannot be
    /// appended.
    pub fn append(&self, event: SystemErrorEvent) -> Result<(), SystemErrorWriteError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(SystemErrorWriteError::CreateDirectory)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(SystemErrorWriteError::Open)?;
        let line = serde_json::to_string(&event).map_err(SystemErrorWriteError::Serialize)?;
        writeln!(file, "{line}").map_err(SystemErrorWriteError::Write)
    }

    /// Best-effort append that never panics or masks the caller's original error.
    pub fn try_append(&self, event: SystemErrorEvent) {
        let _ = self.append(event);
    }
}

/// One JSONL system error event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemErrorEvent {
    /// UTC RFC3339 timestamp at write construction time.
    pub timestamp: String,
    /// Severity label. The first slice writes only `error`.
    pub severity: &'static str,
    /// Stable machine-readable category.
    pub category: &'static str,
    /// Concise human-readable message.
    pub message: String,
    /// Subsystem-specific context.
    pub context: Value,
    /// Ordered error strings where source errors are available.
    pub error_chain: Vec<String>,
    /// Uncapped, unredacted raw payloads.
    pub raw: Value,
}

impl SystemErrorEvent {
    /// Construct a system error event with empty context, error chain, and raw payload.
    #[must_use]
    pub fn new(category: &'static str, message: impl Into<String>) -> Self {
        Self {
            timestamp: OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string()),
            severity: "error",
            category,
            message: message.into(),
            context: json!({}),
            error_chain: Vec::new(),
            raw: json!({}),
        }
    }

    /// Attach structured context.
    #[must_use]
    pub fn with_context(mut self, context: Value) -> Self {
        self.context = context;
        self
    }

    /// Attach error-chain strings.
    #[must_use]
    pub fn with_error_chain<I>(mut self, error_chain: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        self.error_chain = error_chain.into_iter().collect();
        self
    }

    /// Attach uncapped raw diagnostic payloads.
    #[must_use]
    pub fn with_raw(mut self, raw: Value) -> Self {
        self.raw = raw;
        self
    }
}

/// Errors produced when writing developer diagnostics.
#[derive(Debug, Error)]
pub enum SystemErrorWriteError {
    /// Parent directory could not be created.
    #[error("failed to create system error log directory: {0}")]
    CreateDirectory(std::io::Error),
    /// Log file could not be opened.
    #[error("failed to open system error log: {0}")]
    Open(std::io::Error),
    /// Event could not be serialized.
    #[error("failed to serialize system error event: {0}")]
    Serialize(serde_json::Error),
    /// Log line could not be written.
    #[error("failed to write system error event: {0}")]
    Write(std::io::Error),
}

#[cfg(test)]
pub(crate) fn read_system_error_events(
    path: impl AsRef<Path>,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    text.lines()
        .map(|line| serde_json::from_str::<Value>(line).map_err(Into::into))
        .collect()
}
```

- [ ] **Step 8: Export logger module**

Modify `crates/noema-core/src/lib.rs`:

```rust
/// Developer diagnostic system error logging.
pub mod system_errors;
```

Add the exports near the other `pub use` blocks:

```rust
pub use system_errors::{
    SYSTEM_ERROR_MCP_MALFORMED_RESPONSE, SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE,
    SYSTEM_ERROR_RUNTIME_INVARIANT, SYSTEM_ERROR_STORE_INVARIANT, SystemErrorEvent,
    SystemErrorLogger, SystemErrorWriteError,
};
```

- [ ] **Step 9: Run logger tests**

Run:

```bash
cargo test -p noema-core system_errors::tests paths::tests::errors_log_path_lives_at_noema_root --no-fail-fast
```

Expected: pass.

- [ ] **Step 10: Commit logger primitive**

Run:

```bash
git add Cargo.toml Cargo.lock crates/noema-core/Cargo.toml crates/noema-core/src/paths.rs crates/noema-core/src/lib.rs crates/noema-core/src/system_errors.rs
git commit -m "feat: add system error logger"
```

## Task 2: Wire Logger Through Runtime And Providers

**Files:**
- Modify: `crates/noema-core/src/runtime_host.rs`
- Modify: `crates/noema-core/src/daemon/runtime/handle.rs`
- Modify: `crates/noema-core/src/daemon/runtime/actor.rs`
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs`
- Modify: `crates/noema-core/src/provider/adapters/openai.rs`

- [ ] **Step 1: Add logger fields to provider configs**

In `crates/noema-core/src/provider/adapters/codex_responses.rs`, add `SystemErrorLogger` to imports:

```rust
use crate::{
    SystemErrorLogger,
    provider::{
        DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateInput, GenerateRequest, GenerateResponse,
        GenerateStreamEvent, ModelProvider, ProviderError, output_items_from_text,
        required_output_items_from_text,
    },
};
```

Add the field to `CodexProviderConfig`:

```rust
/// Developer diagnostic system error logger.
pub system_errors: Option<SystemErrorLogger>,
```

Add it to `Default`:

```rust
system_errors: None,
```

Add this field to `CodexResponsesProvider`:

```rust
system_errors: Option<SystemErrorLogger>,
```

Set it in `with_client`:

```rust
system_errors: config.system_errors.clone(),
```

Repeat the same pattern in `crates/noema-core/src/provider/adapters/openai.rs`:

```rust
use crate::{
    SystemErrorLogger,
    provider::{
        DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateInput, GenerateRequest, GenerateResponse,
        ModelProvider, ProviderError, output_items_from_text, required_output_items_from_text,
    },
};
```

Add to `OpenAiProviderConfig`:

```rust
/// Developer diagnostic system error logger.
pub system_errors: Option<SystemErrorLogger>,
```

Add to `OpenAiProvider`:

```rust
system_errors: Option<SystemErrorLogger>,
```

Set in `with_client`:

```rust
let system_errors = config.system_errors.clone();
Ok(Self {
    transport,
    system_errors,
    config,
})
```

Update every OpenAI test config literal to include:

```rust
system_errors: None,
```

- [ ] **Step 2: Run compile check to surface missing config fields**

Run:

```bash
cargo check -p noema-core
```

Expected: fail only at provider config construction sites that still need `system_errors`.

- [ ] **Step 3: Pass logger from runtime host into runtime handle**

Modify imports in `crates/noema-core/src/runtime_host.rs`:

```rust
use crate::{
    DaemonError, NoemaHomeInitOptions, NoemaPathError, NoemaPaths, NoemaStore, ProviderConfig,
    StoreConfig, SystemErrorLogger, daemon::CodexRuntimeHandle, mcp::McpOAuthSetupManager,
    provider::auth::ProviderAuthManager,
};
```

Add a field to `NoemaRuntimeHost`:

```rust
system_errors: SystemErrorLogger,
```

In `NoemaRuntimeHost::start`, after `init_noema_home`, construct:

```rust
let system_errors = SystemErrorLogger::from_paths(&paths);
```

Change runtime spawn:

```rust
let runtime = CodexRuntimeHandle::spawn_from_config(
    provider,
    store.clone(),
    system_errors.clone(),
)
.await
.map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
```

Initialize the struct:

```rust
system_errors,
```

Add an accessor:

```rust
/// Developer diagnostic system error logger.
#[must_use]
pub fn system_errors(&self) -> &SystemErrorLogger {
    &self.system_errors
}
```

Update `for_tests_with_store_and_runtime`:

```rust
system_errors: SystemErrorLogger::from_paths(
    &NoemaPaths::from_process_env().expect("test paths"),
),
```

- [ ] **Step 4: Pass logger through `CodexRuntimeHandle`**

Modify `crates/noema-core/src/daemon/runtime/handle.rs` imports:

```rust
use crate::{
    FoundationLocalProvider, FoundationLocalProviderConfig, NoemaStore, OpenAiProvider,
    ProviderConfig, ProviderKind, SystemErrorLogger,
    config::DEFAULT_FOUNDATION_LOCAL_PROFILE,
    provider::adapters::codex_responses::{CodexProviderConfig, CodexResponsesProvider},
    provider::{
        GenerateRequest, GenerateResponse, GenerateStreamEvent, ModelProvider,
        ProviderContextMetadata, ProviderError,
    },
};
```

Change `spawn_from_config` signature:

```rust
pub(crate) async fn spawn_from_config(
    provider_config: ProviderConfig,
    store: NoemaStore,
    system_errors: SystemErrorLogger,
) -> Result<Self, DaemonError> {
```

Change provider construction:

```rust
let (default_provider_kind, default_provider) =
    provider_from_config(provider_config, system_errors.clone())?;
```

Change default Codex fallback:

```rust
Arc::new(CodexResponsesProvider::new(
    default_codex_provider_config(system_errors.clone())?,
)?)
```

Change spawn inner:

```rust
Self::spawn_with_provider_map_inner(default_provider_kind, providers, store, system_errors).await
```

Change test helper calls to use a disabled temp logger from the store root:

```rust
let system_errors = store.system_error_logger();
Self::spawn_with_provider_map_inner(
    default_provider_kind.into(),
    providers.into_iter().collect(),
    store,
    system_errors,
)
.await
```

Add `NoemaStore::system_error_logger()` in `crates/noema-core/src/store/runtime.rs`:

```rust
/// Return a developer diagnostic logger rooted in this store's Noema home.
#[must_use]
pub(crate) fn system_error_logger(&self) -> crate::SystemErrorLogger {
    crate::SystemErrorLogger::new(self.noema_home.join("errors.log"))
}
```

Change `spawn_with_provider_map_inner` signature:

```rust
async fn spawn_with_provider_map_inner(
    default_provider_kind: String,
    providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
    store: NoemaStore,
    system_errors: SystemErrorLogger,
) -> Result<Self, DaemonError> {
```

Pass it into the actor:

```rust
let actor = CodexRuntimeActor::new(
    default_provider_kind.clone(),
    providers,
    store,
    system_errors,
)
.await?;
```

Change provider helpers:

```rust
fn provider_from_config(
    provider_config: ProviderConfig,
    system_errors: SystemErrorLogger,
) -> Result<(String, Arc<dyn RuntimeModelProvider>), DaemonError> {
    match provider_config {
        ProviderConfig::Codex(codex_config) => Ok((
            ProviderKind::Codex.as_str().to_string(),
            Arc::new(CodexResponsesProvider::new(codex_provider_config(
                codex_config,
                system_errors,
            )?)?),
        )),
        ProviderConfig::OpenAi(mut openai_config) => {
            openai_config.system_errors = Some(system_errors);
            Ok((
                ProviderKind::OpenAi.as_str().to_string(),
                Arc::new(OpenAiProvider::new(openai_config)?),
            ))
        }
        ProviderConfig::FoundationLocal(config) => Ok((
            ProviderKind::FoundationLocal.as_str().to_string(),
            Arc::new(FoundationLocalProvider::new(config)?),
        )),
    }
}

fn default_codex_provider_config(
    system_errors: SystemErrorLogger,
) -> Result<CodexProviderConfig, DaemonError> {
    codex_provider_config(CodexProviderConfig::default(), system_errors)
}

fn codex_provider_config(
    mut codex_config: CodexProviderConfig,
    system_errors: SystemErrorLogger,
) -> Result<CodexProviderConfig, DaemonError> {
    let paths = crate::NoemaPaths::from_process_env()?;
    let account_home = paths.provider_account_home("codex", "default");
    crate::provider::auth::ensure_provider_account_home(&account_home)?;
    apply_provider_account_home(&mut codex_config, &account_home);
    codex_config.system_errors = Some(system_errors);
    Ok(codex_config)
}
```

- [ ] **Step 5: Store logger on actor**

Modify `crates/noema-core/src/daemon/runtime/actor.rs` imports:

```rust
use crate::{NoemaStore, SystemErrorLogger};
```

Add field:

```rust
pub(super) system_errors: SystemErrorLogger,
```

Change constructor:

```rust
pub(super) async fn new(
    default_provider_kind: String,
    providers: HashMap<String, Arc<dyn RuntimeModelProvider>>,
    store: NoemaStore,
    system_errors: SystemErrorLogger,
) -> Result<Self, DaemonError> {
    Ok(Self {
        default_provider_kind,
        providers,
        store,
        system_errors,
        conversations: HashMap::new(),
    })
}
```

- [ ] **Step 6: Run runtime/provider wiring check**

Run:

```bash
cargo check -p noema-core
```

Expected: pass.

- [ ] **Step 7: Commit runtime/provider wiring**

Run:

```bash
git add crates/noema-core/src/runtime_host.rs crates/noema-core/src/daemon/runtime/handle.rs crates/noema-core/src/daemon/runtime/actor.rs crates/noema-core/src/store/runtime.rs crates/noema-core/src/provider/adapters/codex_responses.rs crates/noema-core/src/provider/adapters/openai.rs
git commit -m "feat: wire system error logger"
```

## Task 3: Log Provider Malformed Responses

**Files:**
- Modify: `crates/noema-core/src/provider/adapters/responses.rs`
- Modify: `crates/noema-core/src/provider/adapters/sse.rs`
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs`
- Modify: `crates/noema-core/src/provider/adapters/openai.rs`

- [ ] **Step 1: Add Responses diagnostic context**

In `crates/noema-core/src/provider/adapters/responses.rs`, import logger types:

```rust
use crate::{
    SystemErrorEvent, SystemErrorLogger, SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE,
    provider::{GenerateStreamEvent, ProviderError, TokenUsage},
};
```

Add this struct near `ResponsesTransport`:

```rust
/// Diagnostic context for Responses-compatible provider calls.
#[derive(Debug, Clone)]
pub struct ResponsesDiagnosticContext {
    /// Developer diagnostic logger.
    pub logger: Option<SystemErrorLogger>,
    /// Provider kind.
    pub provider_kind: String,
    /// Model requested by Noema.
    pub model: String,
    /// Noema conversation id, when available.
    pub conversation_id: Option<String>,
}

impl ResponsesDiagnosticContext {
    /// Build a provider diagnostic context.
    #[must_use]
    pub fn new(
        logger: Option<SystemErrorLogger>,
        provider_kind: impl Into<String>,
        model: impl Into<String>,
        conversation_id: Option<String>,
    ) -> Self {
        Self {
            logger,
            provider_kind: provider_kind.into(),
            model: model.into(),
            conversation_id,
        }
    }

    fn context_json(&self, request_id: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "provider_kind": self.provider_kind,
            "model": self.model,
            "conversation_id": self.conversation_id,
            "request_id": request_id,
        })
    }

    fn log_malformed(&self, message: impl Into<String>, raw: serde_json::Value) {
        if let Some(logger) = &self.logger {
            let message = message.into();
            logger.try_append(
                SystemErrorEvent::new(SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, message.clone())
                    .with_context(self.context_json(None))
                    .with_error_chain([message])
                    .with_raw(raw),
            );
        }
    }
}
```

- [ ] **Step 2: Log non-stream JSON parse failures**

Change `ResponsesTransport::send` signature:

```rust
pub async fn send<T>(
    &self,
    bearer_token: &str,
    body: T,
    extra_headers: HeaderMap,
    diagnostics: ResponsesDiagnosticContext,
) -> Result<ResponsesResponse, ProviderError>
```

Replace the final parse with:

```rust
serde_json::from_str(&body_text).map_err(|source| {
    let message = format!("failed to parse JSON: {source}");
    diagnostics.log_malformed(
        message.clone(),
        serde_json::json!({
            "http_status": status.as_u16(),
            "body_text": body_text,
        }),
    );
    ProviderError::MalformedResponse { message }
})
```

- [ ] **Step 3: Pass diagnostics into streaming accumulator**

Change `send_stream` and `send_streaming` signatures to accept `diagnostics: ResponsesDiagnosticContext`.

In `send_stream`, call:

```rust
self.send_streaming(bearer_token, body, extra_headers, diagnostics, &mut |_| {})
    .await
```

In `send_streaming`, construct:

```rust
let mut accumulator = SseAccumulator::new(diagnostics);
```

- [ ] **Step 4: Add SSE diagnostics**

In `crates/noema-core/src/provider/adapters/sse.rs`, change imports:

```rust
use super::responses::{ResponsesDiagnosticContext, ResponsesResponse, ResponsesUsage};
use crate::{
    SystemErrorEvent, SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE,
    provider::{GenerateStreamEvent, ProviderError},
};
```

Add a field:

```rust
diagnostics: ResponsesDiagnosticContext,
```

Replace `#[derive(Default)]` with an explicit constructor:

```rust
impl SseAccumulator {
    pub(crate) fn new(diagnostics: ResponsesDiagnosticContext) -> Self {
        Self {
            diagnostics,
            pending: Vec::new(),
            output_values: Vec::new(),
            output_text: String::new(),
            response_id: None,
            model: None,
            usage: None,
            terminal_error: None,
        }
    }
```

Add this helper inside `impl SseAccumulator`:

```rust
fn log_malformed(&self, message: impl Into<String>, raw: serde_json::Value) {
    let Some(logger) = &self.diagnostics.logger else {
        return;
    };
    let message = message.into();
    logger.try_append(
        SystemErrorEvent::new(SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, message.clone())
            .with_context(self.diagnostics.context_json(self.response_id.as_deref()))
            .with_error_chain([message])
            .with_raw(raw),
    );
}
```

In `handle_event`, wrap JSON parse:

```rust
let value: Value = serde_json::from_str(&data).map_err(|source| {
    let message = format!("failed to parse SSE JSON: {source}");
    self.log_malformed(
        message.clone(),
        serde_json::json!({
            "event": event.event,
            "data": data,
        }),
    );
    ProviderError::MalformedResponse { message }
})?;
```

In `finish`, when `ResponsesResponse::from_stream_parts` returns an error, log the assembled values:

```rust
let output_values = self.output_values.clone();
let response = ResponsesResponse::from_stream_parts(
    self.response_id.clone(),
    self.model.clone(),
    self.output_values,
    self.usage,
);
if let Err(error) = &response {
    self.log_malformed(
        error.to_string(),
        serde_json::json!({
            "response_id": self.response_id,
            "model": self.model,
            "output_values": output_values,
        }),
    );
}
response
```

In `parse_sse_event_bytes`, leave the pure parser unchanged. Log UTF-8 failures in `push_bytes` by parsing raw event bytes there:

```rust
let event = parse_sse_event_bytes(&raw).map_err(|error| {
    self.log_malformed(
        error.to_string(),
        serde_json::json!({
            "raw_event_bytes_utf8_lossy": String::from_utf8_lossy(&raw).to_string(),
        }),
    );
    error
})?;
```

- [ ] **Step 5: Update SSE tests for constructor**

Replace test construction:

```rust
let mut accumulator = SseAccumulator::new(ResponsesDiagnosticContext::new(
    None,
    "test",
    "test-model",
    None,
));
```

Replace helper `response_from_sse` internals the same way.

- [ ] **Step 6: Log strict envelope failures in Codex**

In `CodexResponsesProvider::generate_with_events`, before parsing output, keep a clone:

```rust
let raw_text = text.clone();
let output = if require_noema_response {
    match required_output_items_from_text(text) {
        Ok(output) => output,
        Err(error) => {
            self.log_malformed_response(
                &error,
                &model,
                request.conversation_id.as_deref(),
                response.id.as_deref(),
                raw_text,
            );
            return Err(error);
        }
    }
} else {
    output_items_from_text(text)?
};
```

Add helper:

```rust
fn log_malformed_response(
    &self,
    error: &ProviderError,
    model: &str,
    conversation_id: Option<&str>,
    request_id: Option<&str>,
    provider_text: String,
) {
    let Some(logger) = &self.system_errors else {
        return;
    };
    logger.try_append(
        SystemErrorEvent::new(
            SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE,
            error.to_string(),
        )
        .with_context(serde_json::json!({
            "provider_kind": "codex",
            "model": model,
            "conversation_id": conversation_id,
            "request_id": request_id,
        }))
        .with_error_chain([error.to_string()])
        .with_raw(serde_json::json!({
            "provider_text": provider_text,
        })),
    );
}
```

Pass transport diagnostics to `send_streaming` calls:

```rust
ResponsesDiagnosticContext::new(
    self.system_errors.clone(),
    "codex",
    model.clone(),
    request.conversation_id.clone(),
)
```

- [ ] **Step 7: Log strict envelope failures in OpenAI**

In `OpenAiProvider::generate`, replace the transport call with:

```rust
let diagnostics = ResponsesDiagnosticContext::new(
    self.system_errors.clone(),
    "openai",
    model.clone(),
    request.conversation_id.clone(),
);
let response = self
    .transport
    .send(&self.config.api_key, body, self.extra_headers()?, diagnostics)
    .await?;
let text = response.output_text()?;
let raw_text = text.clone();

let output = if request.options.require_noema_response {
    match required_output_items_from_text(text) {
        Ok(output) => output,
        Err(error) => {
            self.log_malformed_response(
                &error,
                &model,
                request.conversation_id.as_deref(),
                response.id.as_deref(),
                raw_text,
            );
            return Err(error);
        }
    }
} else {
    output_items_from_text(text)?
};
```

Add this helper in `impl OpenAiProvider`:

```rust
fn log_malformed_response(
    &self,
    error: &ProviderError,
    model: &str,
    conversation_id: Option<&str>,
    request_id: Option<&str>,
    provider_text: String,
) {
    let Some(logger) = &self.system_errors else {
        return;
    };
    logger.try_append(
        SystemErrorEvent::new(
            SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE,
            error.to_string(),
        )
        .with_context(serde_json::json!({
            "provider_kind": "openai",
            "model": model,
            "conversation_id": conversation_id,
            "request_id": request_id,
        }))
        .with_error_chain([error.to_string()])
        .with_raw(serde_json::json!({
            "provider_text": provider_text,
        })),
    );
}
```

- [ ] **Step 8: Add provider instrumentation tests**

Add a Codex test in `crates/noema-core/src/provider/adapters/codex_responses.rs`:

```rust
#[tokio::test]
async fn logs_required_noema_response_parse_failure() {
    let response_body = r#"event: response.output_text.delta
data: {"type":"response.output_text.delta","delta":"plain text"}

event: response.completed
data: {"type":"response.completed","response":{"id":"resp_bad","model":"gpt-test","status":"completed"}}

"#;
    let (base_url, _request_rx) = spawn_server(200, response_body).await;
    let dir = TempDir::new().expect("temp dir");
    let logger = SystemErrorLogger::new(dir.path().join("errors.log"));
    let (mut provider, _tokens_dir) = provider_with_tokens(base_url);
    provider.system_errors = Some(logger.clone());

    let error = provider
        .generate(GenerateRequest {
            conversation_id: Some("conversation:test".to_string()),
            model: Some("gpt-test".to_string()),
            input: GenerateInput::Text("hello".to_string()),
            instructions: None,
            options: GenerateOptions {
                require_noema_response: true,
                ..GenerateOptions::default()
            },
        })
        .await
        .expect_err("malformed response");

    assert!(matches!(error, ProviderError::MalformedResponse { .. }));
    let events = crate::system_errors::read_system_error_events(logger.path()).expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["category"], SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE);
    assert_eq!(events[0]["context"]["conversation_id"], "conversation:test");
    assert_eq!(events[0]["raw"]["provider_text"], "plain text");
}
```

- [ ] **Step 9: Run focused provider tests**

Run:

```bash
cargo test -p noema-core provider::adapters::sse::tests provider::adapters::codex_responses::tests provider::adapters::openai::tests --no-fail-fast
```

Expected: pass.

- [ ] **Step 10: Commit provider diagnostics**

Run:

```bash
git add crates/noema-core/src/provider/adapters/responses.rs crates/noema-core/src/provider/adapters/sse.rs crates/noema-core/src/provider/adapters/codex_responses.rs crates/noema-core/src/provider/adapters/openai.rs
git commit -m "feat: log malformed provider responses"
```

## Task 4: Log MCP Metadata And Tool Call Malformation

**Files:**
- Modify: `crates/noema-core/src/mcp/client.rs`
- Modify: `crates/noema-core/src/mcp/setup.rs`
- Modify: `crates/noema-core/src/mcp/http.rs`
- Modify: `crates/noema-core/src/mcp/stdio.rs`
- Modify: `crates/noema-core/src/capability/gateway.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`

- [ ] **Step 1: Add logging parser wrapper in MCP client**

In `crates/noema-core/src/mcp/client.rs`, import:

```rust
use crate::{
    SystemErrorEvent, SystemErrorLogger, SYSTEM_ERROR_MCP_MALFORMED_RESPONSE,
};
```

Add:

```rust
/// Context for logging malformed MCP responses.
#[derive(Debug, Clone)]
pub struct McpDiagnosticContext {
    /// Developer diagnostic logger.
    pub logger: Option<SystemErrorLogger>,
    /// MCP server id when known.
    pub mcp_server_id: Option<String>,
    /// MCP transport kind when known.
    pub transport_kind: Option<String>,
    /// MCP method being parsed.
    pub method: &'static str,
}

impl McpDiagnosticContext {
    /// Construct an MCP diagnostic context.
    #[must_use]
    pub fn new(
        logger: Option<SystemErrorLogger>,
        mcp_server_id: Option<String>,
        transport_kind: Option<String>,
        method: &'static str,
    ) -> Self {
        Self {
            logger,
            mcp_server_id,
            transport_kind,
            method,
        }
    }

    /// Best-effort log of a malformed MCP payload.
    pub fn log_malformed(&self, error: &McpClientError, raw: Value) {
        let Some(logger) = &self.logger else {
            return;
        };
        logger.try_append(
            SystemErrorEvent::new(SYSTEM_ERROR_MCP_MALFORMED_RESPONSE, error.to_string())
                .with_context(json!({
                    "mcp_server_id": self.mcp_server_id,
                    "transport_kind": self.transport_kind,
                    "method": self.method,
                }))
                .with_error_chain([error.to_string()])
                .with_raw(json!({ "payload": raw })),
        );
    }
}

pub(crate) fn parse_tools_list_result_with_diagnostics(
    result: Value,
    diagnostics: &McpDiagnosticContext,
) -> Result<DiscoveredMcpToolsPage, McpClientError> {
    match parse_tools_list_result(result.clone()) {
        Ok(page) => Ok(page),
        Err(error @ McpClientError::Malformed(_)) => {
            diagnostics.log_malformed(&error, result);
            Err(error)
        }
        Err(error) => Err(error),
    }
}
```

- [ ] **Step 2: Add tests for MCP parser logging**

Add in `mcp/client.rs` tests:

```rust
#[test]
fn tools_list_parser_logs_malformed_payload() {
    let dir = tempfile::tempdir().expect("temp dir");
    let logger = SystemErrorLogger::new(dir.path().join("errors.log"));
    let diagnostics = McpDiagnosticContext::new(
        Some(logger.clone()),
        Some("mcp:test".to_string()),
        Some("sse".to_string()),
        "tools/list",
    );

    let error = parse_tools_list_result_with_diagnostics(
        json!({"tools": "bad"}),
        &diagnostics,
    )
    .expect_err("malformed");

    assert!(matches!(error, McpClientError::Malformed(_)));
    let events = crate::system_errors::read_system_error_events(logger.path()).expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["category"], SYSTEM_ERROR_MCP_MALFORMED_RESPONSE);
    assert_eq!(events[0]["context"]["mcp_server_id"], "mcp:test");
    assert_eq!(events[0]["raw"]["payload"]["tools"], "bad");
}
```

- [ ] **Step 3: Pass diagnostics through setup**

In `crates/noema-core/src/mcp/setup.rs`, create the logger from paths at the start of `create_mcp_server_setup` and `continue_mcp_server_setup`:

```rust
let system_errors = SystemErrorLogger::from_paths(paths);
```

Change `create_mcp_server_setup` and `continue_mcp_server_setup` closure bounds from:

```rust
make_transport: impl Fn(&McpServerRecord, &McpSecretMaterial) -> T,
```

to:

```rust
make_transport: impl Fn(&McpServerRecord, &McpSecretMaterial, Option<SystemErrorLogger>) -> T,
```

Update preview/runtime construction in `create_mcp_server_setup`:

```rust
let mut runtime = McpClientRuntime::new(make_transport(
    &preview,
    &input.secrets,
    Some(system_errors),
));
```

Update `discover_and_persist_tools` to accept `system_errors: SystemErrorLogger` and call:

```rust
let mut runtime = McpClientRuntime::new(make_transport(
    &server,
    secrets,
    Some(system_errors),
));
```

Update `continue_mcp_server_setup` to pass `system_errors` into `discover_and_persist_tools`.

Update all tests inside `crates/noema-core/src/mcp/setup.rs` so fake transport closures accept the third argument and ignore it:

```rust
|_, _, _system_errors| FakeMcpTransport::ok(vec![fake_tool("list_repos")])
```

Update the three setup closures in `crates/noema-core/src/graphql/mcp.rs` that call `state.mcp_setup_transport`:

```rust
let result = create_setup_service(store, paths, setup_input, |server, secrets, system_errors| {
    state.mcp_setup_transport(server, Some(secrets), system_errors)
})
```

```rust
let setup_result = create_setup_service(store, paths, runtime.setup, |server, secrets, system_errors| {
    state.mcp_setup_transport(server, Some(secrets), system_errors)
})
```

```rust
let result = continue_setup_service(
    store,
    paths,
    ContinueMcpServerSetup {
        mcp_server_id: input.mcp_server_id,
        secrets: McpSecretMaterial {
            env: json_string_map(input.secret_env, "secretEnv")?,
            headers: json_string_map(input.secret_headers, "secretHeaders")?,
            oauth_client_credentials: parse_oauth_client_credentials(
                input.oauth_client_credentials,
            )?,
            oauth_credentials: None,
        },
    },
    |server, secrets, system_errors| {
        state.mcp_setup_transport(server, Some(secrets), system_errors)
    },
)
```

Change `GraphqlState::mcp_setup_transport` in `crates/noema-core/src/graphql/schema.rs` to accept diagnostics:

```rust
pub(crate) fn mcp_setup_transport(
    &self,
    server: &crate::McpServerRecord,
    secrets_override: Option<&crate::mcp::secrets::McpSecretMaterial>,
    system_errors: Option<crate::SystemErrorLogger>,
) -> GraphqlMcpSetupTransport {
```

Attach diagnostics in each concrete transport branch:

```rust
match server.transport_kind {
    McpTransportKind::Stdio => {
        StdioMcpTransport::from_server_config(server, secrets)
            .map(|transport| transport.with_diagnostics(system_errors))
    }
    McpTransportKind::Sse => {
        SseMcpTransport::from_server_config(server, secrets)
            .map(|transport| transport.with_diagnostics(system_errors))
    }
    McpTransportKind::StreamableHttp => {
        StreamableHttpMcpTransport::from_server_config(server, secrets)
            .map(|transport| transport.with_diagnostics(system_errors))
    }
}
```

- [ ] **Step 4: Store diagnostics in HTTP transports**

Add `diagnostics: Option<SystemErrorLogger>` fields to `StreamableHttpMcpTransport` and `SseMcpTransport`.

Add builder methods:

```rust
#[must_use]
pub fn with_diagnostics(mut self, diagnostics: Option<SystemErrorLogger>) -> Self {
    self.diagnostics = diagnostics;
    self
}
```

In `SseMcpTransport::read_response`, replace:

```rust
let response: Value = serde_json::from_str(&data).map_err(|error| {
    McpClientError::Malformed(format!("invalid MCP SSE JSON-RPC event: {error}"))
})?;
```

with:

```rust
let response: Value = serde_json::from_str(&data).map_err(|error| {
    let mcp_error = McpClientError::Malformed(format!(
        "invalid MCP SSE JSON-RPC event: {error}"
    ));
    McpDiagnosticContext::new(
        self.diagnostics.clone(),
        None,
        Some("sse".to_string()),
        "json-rpc",
    )
    .log_malformed(&mcp_error, json!({
        "event": event.event,
        "data": data,
    }));
    mcp_error
})?;
```

In `SseMcpTransport::list_tools`, replace:

```rust
let page = parse_tools_list_result(result)?;
```

with:

```rust
let page = parse_tools_list_result_with_diagnostics(
    result,
    &McpDiagnosticContext::new(
        self.diagnostics.clone(),
        None,
        Some("sse".to_string()),
        "tools/list",
    ),
)?;
```

- [ ] **Step 5: Store diagnostics in stdio transport**

Add field:

```rust
diagnostics: Option<SystemErrorLogger>,
```

Initialize it to `None` in `StdioMcpTransport::new`.

Add:

```rust
#[must_use]
pub fn with_diagnostics(mut self, diagnostics: Option<SystemErrorLogger>) -> Self {
    self.diagnostics = diagnostics;
    self
}
```

In `initialize`, wrap `discovered_tool_from_rmcp` conversion:

```rust
.map(|tool| {
    let raw_tool = serde_json::to_value(&tool).unwrap_or_else(|error| {
        json!({"serialization_error": error.to_string()})
    });
    discovered_tool_from_rmcp(tool).map_err(|error| {
        McpDiagnosticContext::new(
            self.diagnostics.clone(),
            None,
            Some("stdio".to_string()),
            "tools/list",
        )
        .log_malformed(&error, raw_tool);
        error
    })
})
```

Replace stdio `call_tool_result_value(result)` calls with this explicit helper:

```rust
fn call_tool_result_value_with_diagnostics(
    result: rmcp::model::CallToolResult,
    diagnostics: Option<SystemErrorLogger>,
    transport_kind: &'static str,
) -> Result<Value, McpClientError> {
    let raw_result = format!("{result:?}");
    serde_json::to_value(result).map_err(|error| {
        let mcp_error = McpClientError::Malformed(format!(
            "invalid MCP tools/call result: {error}"
        ));
        McpDiagnosticContext::new(
            diagnostics,
            None,
            Some(transport_kind.to_string()),
            "tools/call",
        )
        .log_malformed(&mcp_error, json!({ "debug_result": raw_result }));
        mcp_error
    })
}
```

- [ ] **Step 6: Log gateway malformed tool call failures**

Modify `CapabilityGateway` in `crates/noema-core/src/capability/gateway.rs`:

```rust
pub struct CapabilityGateway<'a> {
    /// Canonical Noema store used by calibrated capability implementations.
    pub store: &'a NoemaStore,
    /// Developer diagnostic logger.
    pub system_errors: &'a SystemErrorLogger,
}
```

Update construction in `local_tools.rs`:

```rust
let gateway = CapabilityGateway {
    store: &self.store,
    system_errors: &self.system_errors,
};
```

In `call_mcp_transport_tool`, return `Result<Value, McpClientError>` instead of `Result<Value, &'static str>`. In `try_execute_mcp_tool`, when the error is `McpClientError::Malformed(_)`, log:

```rust
self.system_errors.try_append(
    SystemErrorEvent::new(SYSTEM_ERROR_MCP_MALFORMED_RESPONSE, error.to_string())
        .with_context(json!({
            "mcp_server_id": name.server_id,
            "tool_name": name.tool_name,
            "method": "tools/call",
        }))
        .with_error_chain([error.to_string()])
        .with_raw(json!({
            "arguments": arguments,
            "tool_payload": payload,
        })),
);
```

Then preserve the existing model-visible error string:

```rust
.map_err(|error| match error {
    McpClientError::Malformed(_) => "mcp_tool_call_failed",
    McpClientError::AuthRequired(_) | McpClientError::Transport(_) => "mcp_tool_call_failed",
})
```

- [ ] **Step 7: Run focused MCP tests**

Run:

```bash
cargo test -p noema-core mcp::client::tests mcp::http::tests mcp::stdio::tests capability::gateway::tests --no-fail-fast
```

Expected: pass.

- [ ] **Step 8: Commit MCP diagnostics**

Run:

```bash
git add crates/noema-core/src/mcp/client.rs crates/noema-core/src/mcp/setup.rs crates/noema-core/src/mcp/http.rs crates/noema-core/src/mcp/stdio.rs crates/noema-core/src/capability/gateway.rs crates/noema-core/src/daemon/runtime/local_tools.rs
git commit -m "feat: log malformed MCP responses"
```

## Task 5: Log Runtime And Store Invariants

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/store/error.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add runtime invariant helper**

In `crates/noema-core/src/daemon/runtime/turn.rs`, import:

```rust
use crate::{
    SystemErrorEvent, SYSTEM_ERROR_RUNTIME_INVARIANT,
    daemon::{
        agent_onboarding::AgentPromptIdentity,
        memory_pipeline::{AssistantEvidenceItem, ConversationMemoryContext, explicit_memory_content},
        prompts::{
            build_initial_name_onboarding_system_prompt,
            build_local_tool_result_continuation_system_prompt, build_model_available_tools_prompt,
        },
        protocol::{
            AgentStatus, DaemonError, StartedConversation, TurnStreamEvent, TurnTranscriptItem,
        },
    },
};
```

Add method in `impl CodexRuntimeActor`:

```rust
fn log_runtime_invariant(
    &self,
    message: impl Into<String>,
    context: serde_json::Value,
    raw: serde_json::Value,
) {
    let message = message.into();
    self.system_errors.try_append(
        SystemErrorEvent::new(SYSTEM_ERROR_RUNTIME_INVARIANT, message.clone())
            .with_context(context)
            .with_error_chain([message])
            .with_raw(raw),
    );
}
```

- [ ] **Step 2: Log initial onboarding empty assistant invariant**

In `ensure_initial_name_onboarding_message`, before returning the protocol error for `persisted_count == 0`, add:

```rust
self.log_runtime_invariant(
    "initial onboarding response did not include assistant text",
    json!({
        "conversation_id": conversation_id,
        "turn_id": turn.turn_id,
        "turn_index": turn_index,
        "provider_kind": conversation.provider_kind,
        "model": conversation.model,
    }),
    json!({
        "persisted_count": persisted_count,
    }),
);
```

Keep the existing `fail_conversation_turn`, conversation removal, and returned `DaemonError::Protocol`.

- [ ] **Step 3: Add runtime invariant test**

Add `InitialNameOnboardingNoAssistant` to `FakeCodexScenario` in `crates/noema-core/src/daemon/tests.rs`:

```rust
InitialNameOnboardingNoAssistant,
```

Add this branch in `FakeCodexProvider::generate_response`:

```rust
FakeCodexScenario::InitialNameOnboardingNoAssistant => {
    vec![GenerateOutputItem::MemoryProposals { proposals: vec![] }]
}
```

Add this helper near the existing fake provider helper functions:

```rust
fn fake_codex_provider_with_initial_name_onboarding_no_assistant() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::InitialNameOnboardingNoAssistant)
}
```

Add this test near `start_primary_conversation_generates_initial_name_onboarding_message`:

```rust
#[tokio::test]
async fn failed_initial_name_onboarding_logs_runtime_invariant() {
    let (handle, store) = test_runtime_handle_with_store(
        fake_codex_provider_with_initial_name_onboarding_no_assistant(),
    )
    .await;

    let error = handle
        .start_primary_conversation(None, None)
        .await
        .expect_err("onboarding should fail");
    assert!(
        error
            .to_string()
            .contains("initial onboarding response did not include assistant text")
    );

    let events = crate::system_errors::read_system_error_events(
        store.system_error_logger().path(),
    )
    .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["category"], SYSTEM_ERROR_RUNTIME_INVARIANT);
    assert_eq!(
        events[0]["message"],
        "initial onboarding response did not include assistant text"
    );
    handle.shutdown().await;
}
```

- [ ] **Step 4: Add store invariant event builder**

Do not instrument every store parse call in this slice. Add a reusable helper in `crates/noema-core/src/store/error.rs` so future store parser instrumentation is consistent:

```rust
impl StoreError {
    /// Return true when this store error represents a Noema schema invariant failure.
    #[must_use]
    pub fn is_system_invariant(&self) -> bool {
        matches!(self, Self::Schema(_) | Self::InvalidEnum { .. })
    }

    /// Convert this store invariant into a system error event.
    #[must_use]
    pub fn system_error_event(
        &self,
        context: serde_json::Value,
        raw: serde_json::Value,
    ) -> Option<crate::SystemErrorEvent> {
        self.is_system_invariant().then(|| {
            crate::SystemErrorEvent::new(
                crate::SYSTEM_ERROR_STORE_INVARIANT,
                self.to_string(),
            )
            .with_context(context)
            .with_error_chain([self.to_string()])
            .with_raw(raw)
        })
    }
}
```

- [ ] **Step 5: Add store helper tests**

In `store/error.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn schema_errors_build_system_error_events() {
        let error = StoreError::Schema("bad row".to_string());

        let event = error
            .system_error_event(
                json!({"table": "mcp_tools"}),
                json!({"row": {"status": "bad"}}),
            )
            .expect("event");

        assert_eq!(event.category, crate::SYSTEM_ERROR_STORE_INVARIANT);
        assert_eq!(event.context["table"], "mcp_tools");
        assert_eq!(event.raw["row"]["status"], "bad");
    }

    #[test]
    fn missing_records_are_not_store_invariants() {
        let error = StoreError::ConversationNotFound {
            conversation_id: "conversation:missing".to_string(),
        };

        assert!(error.system_error_event(json!({}), json!({})).is_none());
    }
}
```

- [ ] **Step 6: Run focused runtime/store tests**

Run:

```bash
cargo test -p noema-core daemon::tests store::error::tests --no-fail-fast
```

Expected: pass.

- [ ] **Step 7: Commit runtime/store diagnostics**

Run:

```bash
git add crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/tests.rs crates/noema-core/src/store/error.rs
git commit -m "feat: log runtime system invariants"
```

## Task 6: Final Validation And Documentation Check

**Files:**
- Inspect all files changed by Tasks 1-5.
- No docs change is required unless implementation intentionally differs from the approved spec.

- [ ] **Step 1: Run format**

Run:

```bash
cargo fmt --all --check
```

Expected: pass.

- [ ] **Step 2: Run workspace check**

Run:

```bash
cargo check --workspace
```

Expected: pass.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: pass.

- [ ] **Step 4: Run workspace tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: pass.

- [ ] **Step 5: Inspect final diff**

Run:

```bash
git status --short --branch
git diff --check
git diff --stat
git diff --name-status
```

Expected:

- No whitespace errors.
- Only files named in this plan are modified.
- Unrelated dirty files from before this work remain unstaged and are reported separately.

- [ ] **Step 6: Commit final fixes if validation required changes**

If validation forced additional implementation fixes, commit only the files changed for this feature:

```bash
git add Cargo.toml Cargo.lock crates/noema-core/Cargo.toml crates/noema-core/src/paths.rs crates/noema-core/src/lib.rs crates/noema-core/src/system_errors.rs crates/noema-core/src/runtime_host.rs crates/noema-core/src/daemon/runtime/handle.rs crates/noema-core/src/daemon/runtime/actor.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/runtime/local_tools.rs crates/noema-core/src/provider/adapters/codex_responses.rs crates/noema-core/src/provider/adapters/openai.rs crates/noema-core/src/provider/adapters/responses.rs crates/noema-core/src/provider/adapters/sse.rs crates/noema-core/src/mcp/client.rs crates/noema-core/src/mcp/setup.rs crates/noema-core/src/mcp/http.rs crates/noema-core/src/mcp/stdio.rs crates/noema-core/src/capability/gateway.rs crates/noema-core/src/store/error.rs crates/noema-core/src/store/runtime.rs crates/noema-core/src/graphql/mcp.rs crates/noema-core/src/graphql/schema.rs
git commit -m "fix: stabilize system error diagnostics"
```

If validation required no changes, do not create an empty commit.
