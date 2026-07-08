# Supermemory Model Proxy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Start managed Supermemory with a private OpenAI-compatible chat-completions endpoint backed by the model configured in Settings > Memory.

**Architecture:** Noema starts a loopback-only proxy before the managed Supermemory child process and injects `OPENAI_BASE_URL`, `OPENAI_API_KEY`, and model env vars into that child. The proxy resolves the saved `memory_service_settings` provider/model preference and calls the matching `RuntimeModelProvider` directly, avoiding chat-runtime routing.

**Tech Stack:** Rust, Tokio TCP server, reqwest tests, provider-neutral `GenerateRequest`, existing SQLite-backed memory settings.

## Global Constraints

- Preserve unrelated dirty worktree changes.
- Work on `main` unless explicitly instructed otherwise.
- Do not add a migration path or backwards compatibility layer.
- Do not modify `CARGO_BUILD_RUSTC_WRAPPER` or bypass `sccache`.
- Use TDD for behavioral changes.

---

### Task 1: OpenAI-Compatible Proxy Boundary

**Files:**
- Create: `crates/noema-core/src/supermemory/model_proxy.rs`
- Modify: `crates/noema-core/src/supermemory.rs`

**Interfaces:**
- Produces: `SupermemoryModelProxy::start(config) -> Result<Self, SupermemoryModelProxyError>`
- Produces: `SupermemoryModelProxy::{base_url, api_key, model_profile}`
- Consumes: `Arc<dyn RuntimeModelProvider>`, `GenerateRequest`, `GenerateResponse`

- [x] **Step 1: Write failing tests**

Add tests for:
- Authorized non-streaming `POST /v1/chat/completions` maps OpenAI messages/tools into `GenerateRequest`.
- Provider tool calls map back into OpenAI `tool_calls`.
- `stream: true` returns a non-success JSON error.

- [x] **Step 2: Verify tests fail**

Run: `cargo test -p noema-core supermemory::model_proxy -- --nocapture`
Expected: compile/test failure because the module does not exist.

- [x] **Step 3: Implement the proxy**

Implement a minimal loopback Tokio TCP HTTP server supporting `POST /v1/chat/completions` and `GET /v1/models`, bearer auth, JSON responses, bounded request bodies, and graceful shutdown on drop.

- [x] **Step 4: Verify tests pass**

Run: `cargo test -p noema-core supermemory::model_proxy -- --nocapture`
Expected: pass.

### Task 2: Memory Settings Provider Routing

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/handle.rs`
- Modify: `crates/noema-core/src/runtime_host.rs`

**Interfaces:**
- Produces: `CodexRuntimeHandle::provider_map_from_config(...)`
- Produces: `CodexRuntimeHandle::spawn_with_provider_map_and_supermemory(...)`
- Consumes: `MemoryServiceSettingsRecord::{provider_kind, model_profile, reasoning_effort}`

- [x] **Step 1: Write failing tests**

Add tests proving the proxy provider is selected from saved Memory settings when present, and the default provider/model is used only when Memory settings are unset.

- [x] **Step 2: Implement provider map sharing**

Extract provider-map construction from `spawn_from_config`, share the resulting `Arc<dyn RuntimeModelProvider>` map with both the proxy and chat runtime, and preserve the existing Codex/Foundation fallback providers.

- [x] **Step 3: Verify targeted tests pass**

Run: `cargo test -p noema-core runtime_host supermemory::model_proxy -- --nocapture`
Expected: pass.

### Task 3: Managed Supermemory Env Injection

**Files:**
- Modify: `crates/noema-core/src/supermemory/lifecycle.rs`
- Modify: `crates/noema-core/src/runtime_host.rs`

**Interfaces:**
- Consumes: `Option<SupermemoryModelProxy>`
- Injects: `OPENAI_BASE_URL`, `OPENAI_API_KEY`, `OPENAI_MODEL`, `OPENAI_FAST_MODEL`, `OPENAI_TEXT_MODEL`

- [x] **Step 1: Write failing lifecycle test**

Use a fake `supermemory-server` script that records its environment and keeps running long enough for startup to pass. Assert managed lifecycle injects the proxy env vars.

- [x] **Step 2: Implement lifecycle ownership**

Store the proxy in `SupermemoryLifecycle` so it lives for the child lifetime, inject env vars before spawning, and shut it down with the lifecycle.

- [x] **Step 3: Verify targeted tests pass**

Run: `cargo test -p noema-core supermemory::lifecycle -- --nocapture`
Expected: pass.

### Task 4: Validate And Commit

**Files:**
- Modify: `docs/context/current.md`

- [x] **Step 1: Update durable context**

Record that managed Supermemory now receives model access through a private Noema-hosted OpenAI-compatible proxy backed by Settings > Memory.

- [x] **Step 2: Run validation**

Run:
- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --no-fail-fast`

- [ ] **Step 3: Commit**

Run:
- `git status --short --branch`
- `git diff --check`
- `git add ...`
- `git diff --cached --stat`
- `git diff --cached --name-status`
- `git commit -m "Add private Supermemory model proxy"`
