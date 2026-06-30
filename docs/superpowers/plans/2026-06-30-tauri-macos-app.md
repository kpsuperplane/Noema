# Tauri macOS App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an unsigned Tauri macOS developer app for Noema that starts the local Noema runtime invisibly, uses Tauri IPC instead of an exposed local web server, reuses the existing React UI, and opens provider auth URLs in the system browser.

**Architecture:** Extract a reusable `noema-core` runtime host so daemon web mode and desktop mode share store/runtime/provider setup without depending on daemon web types. Refactor GraphQL state to use that host-facing state, then add a Tauri app in `crates/noema-desktop` with GraphQL execute/subscribe IPC commands and a desktop Apollo transport. Keep the existing daemon HTTP/WebSocket path for CLI and browser development.

**Tech Stack:** Rust 2024, Tokio, async-graphql, SurrealDB embedded RocksDB, Tauri 2, Bun, Vite, React, Apollo Client.

---

## File Structure

Create:

- `crates/noema-core/src/runtime_host.rs` - shared runtime host for daemon and desktop.
- `crates/noema-core/src/graphql/runtime_state.rs` - GraphQL-facing state wrapper independent of daemon web state.
- `crates/noema-core/web/src/graphql/browserTransport.ts` - existing HTTP/WebSocket Apollo link.
- `crates/noema-core/web/src/graphql/desktopBridge.ts` - typed wrappers around Tauri `invoke` and `listen`.
- `crates/noema-core/web/src/graphql/desktopTransport.ts` - Apollo link for Tauri GraphQL execute/subscribe IPC.
- `crates/noema-core/web/src/graphql/externalUrls.ts` - desktop-aware provider auth URL opener.
- `crates/noema-core/web/src/graphql/transportMode.ts` - runtime Tauri/browser detection.
- `crates/noema-core/web/src/graphql/desktopTransport.test.ts` - desktop transport unit tests.
- `crates/noema-core/web/src/graphql/externalUrls.test.ts` - URL-opening behavior tests.
- `crates/noema-core/web/vite.desktop.config.ts` - desktop bundle Vite config with relative asset paths.
- `crates/noema-desktop/Cargo.toml` - desktop crate manifest.
- `crates/noema-desktop/build.rs` - Tauri build script.
- `crates/noema-desktop/src/main.rs` - Tauri app entrypoint.
- `crates/noema-desktop/src/desktop_state.rs` - managed app state and runtime startup state.
- `crates/noema-desktop/src/graphql_ipc.rs` - GraphQL execute/subscribe IPC bridge.
- `crates/noema-desktop/src/external_url.rs` - native external URL command.
- `crates/noema-desktop/tauri.conf.json` - Tauri config loaded by `tauri::generate_context!()`.
- `crates/noema-desktop/capabilities/default.json` - narrow IPC capabilities.

Modify:

- `Cargo.toml` - add `crates/noema-desktop` workspace member and shared Tauri-related dependencies only if centralizing them is useful.
- `crates/noema-core/src/lib.rs` - export the runtime host and any GraphQL state constructors required by desktop.
- `crates/noema-core/src/graphql.rs` - register `runtime_state` module.
- `crates/noema-core/src/graphql/schema.rs` - replace daemon-web-specific state with runtime state.
- `crates/noema-core/src/graphql/chat.rs` - use runtime/store/provider state accessors and core replay helpers.
- `crates/noema-core/src/graphql/onboarding.rs` - use runtime/store/provider auth/path accessors and shared auth helpers.
- `crates/noema-core/src/daemon/server.rs` - build a `NoemaRuntimeHost` and pass host-derived state into web mode.
- `crates/noema-core/src/daemon/web/mod.rs` - reduce `WebState` to web transport state or remove runtime/store duplication if no longer needed.
- `crates/noema-core/web/src/graphql/client.ts` - select browser or desktop Apollo transport.
- `crates/noema-core/web/src/components/onboarding/AuthAttempt.tsx` - call the desktop URL opener for provider auth URLs.
- `crates/noema-core/web/package.json` - add Tauri Vite dev/build scripts.
- `docs/context/current.md` - record the settled desktop direction after implementation is complete.

Keep under watch:

- `crates/noema-core/src/daemon/web/provider_auth.rs` and `crates/noema-core/src/daemon/web/replay.rs` currently contain helpers GraphQL uses. Move only the helpers required to remove daemon-web coupling; leave HTTP-only logic in daemon web modules.
- Preserve unrelated dirty changes in web shell files and generated daemon assets.

---

### Task 1: Extract A Shared Runtime Host

**Files:**
- Create: `crates/noema-core/src/runtime_host.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/daemon/server.rs`
- Test: `crates/noema-core/src/runtime_host.rs`

- [ ] **Step 1: Write a failing runtime host unit test**

Add this test module at the bottom of `crates/noema-core/src/runtime_host.rs` when creating the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_host_error_messages_are_plain_language() {
        assert_eq!(
            RuntimeHostError::DataFolder("permission denied".to_string()).user_message(),
            "Noema could not open its data folder."
        );
        assert_eq!(
            RuntimeHostError::Store("rocksdb failed".to_string()).user_message(),
            "Noema could not start its local memory store."
        );
        assert_eq!(
            RuntimeHostError::Runtime("provider failed".to_string()).user_message(),
            "Noema could not start the local assistant service."
        );
    }
}
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test -p noema-core runtime_host_error_messages_are_plain_language
```

Expected: FAIL because `runtime_host.rs`, `RuntimeHostError`, and `user_message` do not exist yet.

- [ ] **Step 3: Add the shared runtime host**

Create `crates/noema-core/src/runtime_host.rs` with this initial implementation:

```rust
//! Shared Noema runtime host used by daemon and desktop shells.

use crate::{
    CodexProviderConfig, DaemonError, NoemaPathError, NoemaPaths, NoemaStore, StoreConfig,
    daemon::CodexRuntimeHandle, provider_auth::ProviderAuthManager,
};
use thiserror::Error;

/// Shared host state for Noema client surfaces.
#[derive(Clone)]
pub struct NoemaRuntimeHost {
    runtime: CodexRuntimeHandle,
    store: NoemaStore,
    provider_auth: ProviderAuthManager,
    paths: NoemaPaths,
    subscriptions: crate::graphql::ConversationSubscriptionRegistry,
}

impl NoemaRuntimeHost {
    /// Start the shared Noema runtime host.
    pub async fn start(codex: CodexProviderConfig) -> Result<Self, RuntimeHostError> {
        let paths = NoemaPaths::from_process_env()
            .map_err(|source| RuntimeHostError::DataFolder(source.to_string()))?;
        crate::init_noema_home(
            &paths,
            crate::NoemaHomeInitOptions {
                force: false,
                write_config: !paths.config_exists(),
            },
        )
        .map_err(|source| RuntimeHostError::DataFolder(source.to_string()))?;
        let store = NoemaStore::open(&StoreConfig::from_paths(&paths))
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        store
            .ensure_default_provider_account()
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
        let runtime = CodexRuntimeHandle::spawn(codex, store.clone())
            .await
            .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;

        Ok(Self {
            runtime,
            store,
            provider_auth: ProviderAuthManager::new(),
            paths,
            subscriptions: crate::graphql::ConversationSubscriptionRegistry::default(),
        })
    }

    /// Construct a host around already-open test state.
    #[cfg(test)]
    pub(crate) async fn for_tests_with_store_and_runtime(
        store: NoemaStore,
        runtime: CodexRuntimeHandle,
    ) -> Self {
        Self {
            runtime,
            store,
            provider_auth: ProviderAuthManager::new(),
            paths: NoemaPaths::from_process_env().expect("test paths"),
            subscriptions: crate::graphql::ConversationSubscriptionRegistry::default(),
        }
    }

    /// Runtime command handle.
    pub(crate) fn runtime(&self) -> &CodexRuntimeHandle {
        &self.runtime
    }

    /// Store handle.
    pub fn store(&self) -> &NoemaStore {
        &self.store
    }

    /// Provider auth manager.
    pub fn provider_auth(&self) -> &ProviderAuthManager {
        &self.provider_auth
    }

    /// Resolved Noema paths.
    pub fn paths(&self) -> &NoemaPaths {
        &self.paths
    }

    /// Conversation subscription registry.
    pub(crate) fn subscriptions(&self) -> &crate::graphql::ConversationSubscriptionRegistry {
        &self.subscriptions
    }

    /// Shut down runtime-owned work.
    pub async fn shutdown(&self) {
        self.runtime.shutdown().await;
    }
}

/// Runtime host startup error.
#[derive(Debug, Error)]
pub enum RuntimeHostError {
    /// Data folder/path setup failed.
    #[error("data folder setup failed: {0}")]
    DataFolder(String),
    /// Store setup failed.
    #[error("store setup failed: {0}")]
    Store(String),
    /// Runtime setup failed.
    #[error("runtime setup failed: {0}")]
    Runtime(String),
}

impl RuntimeHostError {
    /// Plain-language user-facing message.
    #[must_use]
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::DataFolder(_) => "Noema could not open its data folder.",
            Self::Store(_) => "Noema could not start its local memory store.",
            Self::Runtime(_) => "Noema could not start the local assistant service.",
        }
    }

    /// Detailed diagnostic string.
    #[must_use]
    pub fn technical_details(&self) -> String {
        self.to_string()
    }
}

impl From<NoemaPathError> for RuntimeHostError {
    fn from(source: NoemaPathError) -> Self {
        Self::DataFolder(source.to_string())
    }
}

impl From<DaemonError> for RuntimeHostError {
    fn from(source: DaemonError) -> Self {
        Self::Runtime(source.to_string())
    }
}
```

Keep `runtime_host.rs` inside `noema-core`, where `crate::daemon::CodexRuntimeHandle` remains visible through the existing `pub(crate)` re-export. Do not make the raw runtime handle part of the public crate API.

- [ ] **Step 4: Export the runtime host from `lib.rs`**

Modify `crates/noema-core/src/lib.rs`:

```rust
/// Shared runtime host for daemon and desktop client surfaces.
pub mod runtime_host;
```

Add to the exports:

```rust
pub use runtime_host::{NoemaRuntimeHost, RuntimeHostError};
```

- [ ] **Step 5: Refactor daemon startup to create the host once**

In `crates/noema-core/src/daemon/server.rs`, replace the duplicated path/store/runtime/provider-auth setup:

```rust
let paths = NoemaPaths::from_process_env()?;
let store = crate::NoemaStore::open(&StoreConfig::from_paths(&paths)).await?;
store.ensure_default_provider_account().await?;
let runtime = CodexRuntimeHandle::spawn(config.codex, store.clone()).await?;
let provider_auth = crate::provider_auth::ProviderAuthManager::new();
```

with:

```rust
let host = crate::NoemaRuntimeHost::start(config.codex)
    .await
    .map_err(|source| DaemonError::Protocol(source.to_string()))?;
```

Then build web state from the host in Task 2. Leave the old local variables in place until Task 2 if the compiler needs them temporarily; do not commit a non-compiling intermediate state.

- [ ] **Step 6: Run the focused test**

Run:

```bash
cargo test -p noema-core runtime_host_error_messages_are_plain_language
```

Expected: PASS.

- [ ] **Step 7: Commit Task 1**

Run:

```bash
git add crates/noema-core/src/runtime_host.rs crates/noema-core/src/lib.rs crates/noema-core/src/daemon/server.rs
git commit -m "refactor(core): add shared runtime host"
```

---

### Task 2: Decouple GraphQL State From Daemon Web State

**Files:**
- Create: `crates/noema-core/src/graphql/runtime_state.rs`
- Modify: `crates/noema-core/src/graphql.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/src/graphql/chat.rs`
- Modify: `crates/noema-core/src/graphql/onboarding.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Modify: `crates/noema-core/src/daemon/server.rs`
- Test: `crates/noema-core/src/graphql/schema.rs`
- Test: `crates/noema-core/src/graphql/resolvers.rs`

- [ ] **Step 1: Write failing GraphQL state tests**

Add this test to `crates/noema-core/src/graphql/schema.rs`:

```rust
#[test]
fn graphql_state_for_tests_has_runtime_state_accessors() {
    let state = GraphqlState::for_tests();
    assert!(state.optional_store().is_none());
    assert_eq!(
        state.memory_storage(),
        crate::graphql::local_status::GraphqlMemoryStorageStatus::Ready
    );
}
```

Add this test to `crates/noema-core/src/graphql/resolvers.rs`:

```rust
#[tokio::test]
async fn local_status_still_works_without_daemon_web_state() {
    let schema = build_schema(GraphqlState::for_tests());
    let response = schema
        .execute(Request::new("{ localStatus { localService assistantConnection memoryStorage } }"))
        .await
        .into_result()
        .expect("query should succeed");

    assert_eq!(
        response.data,
        Value::from_json(serde_json::json!({
            "localStatus": {
                "localService": "RUNNING",
                "assistantConnection": "CODEX",
                "memoryStorage": "READY"
            }
        }))
        .expect("valid json")
    );
}
```

- [ ] **Step 2: Run focused GraphQL tests to verify the new accessor test fails**

Run:

```bash
cargo test -p noema-core graphql_state_for_tests_has_runtime_state_accessors local_status_still_works_without_daemon_web_state
```

Expected: FAIL if accessors are still private or daemon-web-specific.

- [ ] **Step 3: Add `runtime_state.rs`**

Create `crates/noema-core/src/graphql/runtime_state.rs`:

```rust
use crate::{
    NoemaPaths, NoemaRuntimeHost, NoemaStore, daemon::CodexRuntimeHandle,
    provider_auth::ProviderAuthManager,
};

use super::{ConversationSubscriptionRegistry, local_status::GraphqlMemoryStorageStatus};

/// GraphQL resolver state shared by web daemon and desktop transports.
#[derive(Clone)]
pub struct GraphqlRuntimeState {
    runtime: Option<CodexRuntimeHandle>,
    store: Option<NoemaStore>,
    provider_auth: Option<ProviderAuthManager>,
    paths: Option<NoemaPaths>,
    subscriptions: ConversationSubscriptionRegistry,
    memory_storage: GraphqlMemoryStorageStatus,
}

impl GraphqlRuntimeState {
    /// Build state from a real runtime host.
    #[must_use]
    pub fn from_host(host: &NoemaRuntimeHost) -> Self {
        Self {
            runtime: Some(host.runtime().clone()),
            store: Some(host.store().clone()),
            provider_auth: Some(host.provider_auth().clone()),
            paths: Some(host.paths().clone()),
            subscriptions: host.subscriptions().clone(),
            memory_storage: GraphqlMemoryStorageStatus::Ready,
        }
    }

    /// Build state for resolver tests that do not need runtime access.
    #[must_use]
    pub fn for_tests() -> Self {
        Self {
            runtime: None,
            store: None,
            provider_auth: None,
            paths: None,
            subscriptions: ConversationSubscriptionRegistry::default(),
            memory_storage: GraphqlMemoryStorageStatus::Ready,
        }
    }

    /// Build state for resolver tests with a store.
    #[must_use]
    pub fn for_tests_with_store(store: NoemaStore) -> Self {
        Self {
            store: Some(store),
            ..Self::for_tests()
        }
    }

    pub(crate) fn runtime(&self) -> async_graphql::Result<&CodexRuntimeHandle> {
        self.runtime
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema runtime is unavailable"))
    }

    pub(crate) fn store(&self) -> async_graphql::Result<&NoemaStore> {
        self.store
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema store is unavailable"))
    }

    pub(crate) fn optional_store(&self) -> Option<&NoemaStore> {
        self.store.as_ref()
    }

    pub(crate) fn provider_auth(&self) -> async_graphql::Result<&ProviderAuthManager> {
        self.provider_auth
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema provider auth is unavailable"))
    }

    pub(crate) fn paths(&self) -> async_graphql::Result<&NoemaPaths> {
        self.paths
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema paths are unavailable"))
    }

    pub(crate) fn subscriptions(&self) -> &ConversationSubscriptionRegistry {
        &self.subscriptions
    }

    pub(crate) fn memory_storage(&self) -> GraphqlMemoryStorageStatus {
        self.memory_storage
    }
}
```

In `crates/noema-core/src/graphql.rs`, add:

```rust
mod runtime_state;
```

and export internally:

```rust
pub(crate) use runtime_state::GraphqlRuntimeState;
```

- [ ] **Step 4: Refactor `GraphqlState`**

In `crates/noema-core/src/graphql/schema.rs`, replace the current `GraphqlState` fields with:

```rust
#[derive(Clone)]
pub struct GraphqlState {
    runtime_state: GraphqlRuntimeState,
}
```

Update constructors and accessors:

```rust
impl GraphqlState {
    #[must_use]
    pub fn for_tests() -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests(),
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn for_tests_with_store(store: crate::NoemaStore) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::for_tests_with_store(store),
        }
    }

    #[must_use]
    pub fn from_runtime_host(host: &crate::NoemaRuntimeHost) -> Self {
        Self {
            runtime_state: GraphqlRuntimeState::from_host(host),
        }
    }

    pub(crate) fn runtime(&self) -> Result<&crate::daemon::CodexRuntimeHandle> {
        self.runtime_state.runtime()
    }

    pub(crate) fn store(&self) -> Result<&crate::NoemaStore> {
        self.runtime_state.store()
    }

    pub(crate) fn optional_store(&self) -> Option<&crate::NoemaStore> {
        self.runtime_state.optional_store()
    }

    pub(crate) fn provider_auth(&self) -> Result<&crate::provider_auth::ProviderAuthManager> {
        self.runtime_state.provider_auth()
    }

    pub(crate) fn paths(&self) -> Result<&crate::NoemaPaths> {
        self.runtime_state.paths()
    }

    pub(crate) fn subscriptions(&self) -> &ConversationSubscriptionRegistry {
        self.runtime_state.subscriptions()
    }

    pub(crate) fn memory_storage(&self) -> GraphqlMemoryStorageStatus {
        self.runtime_state.memory_storage()
    }
}
```

Remove `web_state`, `test_store`, and `from_web_state`.

- [ ] **Step 5: Update daemon web schema creation**

In `crates/noema-core/src/daemon/web/mod.rs`, remove runtime/store/provider/path fields from `WebState` after `GraphqlState` no longer needs them. Keep only data needed by the HTTP/WebSocket transport if any. In `handle_graphql_http` and the schema route, build schemas from the host-derived state passed from `server.rs`:

```rust
let schema = crate::graphql::build_schema(state.graphql_state().clone());
```

If `WebState` still exists, give it:

```rust
#[derive(Clone)]
pub(crate) struct WebState {
    graphql_state: crate::graphql::GraphqlState,
}

impl WebState {
    pub(crate) fn new(graphql_state: crate::graphql::GraphqlState) -> Self {
        Self { graphql_state }
    }

    pub(crate) fn graphql_state(&self) -> &crate::graphql::GraphqlState {
        &self.graphql_state
    }
}
```

In `server.rs`, after Task 1:

```rust
let graphql_state = crate::graphql::GraphqlState::from_runtime_host(&host);
let web_state = WebState::new(graphql_state);
```

Keep `state.runtime.shutdown().await` behavior by changing shutdown to:

```rust
host.shutdown().await;
```

- [ ] **Step 6: Update chat resolvers**

In `crates/noema-core/src/graphql/chat.rs`, replace:

```rust
let web = state.web_state()?;
```

with direct accessors:

```rust
let store = state.store()?;
let runtime = state.runtime()?;
```

For onboarding check:

```rust
let account = store
    .active_provider_account("codex")
    .await
    .map_err(graphql_error)?;
if !crate::daemon::web::is_user_onboarded_for_chat(account) {
    return Err(async_graphql::Error::new(
        "Noema onboarding is incomplete. Connect a provider account before starting chat.",
    ));
}
```

For replay:

```rust
let replay_records =
    crate::daemon::web::visible_conversation_replay(store, &started.conversation_id)
        .await
        .map_err(graphql_error)?;
```

For sending turns:

```rust
let runtime = state.runtime()?.clone();
let subscriptions = state.subscriptions().clone();
```

This step may still call `crate::daemon::web` replay helpers. That is acceptable within `noema-core` for this task. Move those helpers to a neutral module only if compiler visibility requires it.

- [ ] **Step 7: Update onboarding resolvers**

In `crates/noema-core/src/graphql/onboarding.rs`, replace `state.web_state()?` with:

```rust
let store = state.store()?;
let paths = state.paths()?;
let provider_auth = state.provider_auth()?;
```

For status:

```rust
let account = store
    .active_provider_account("codex")
    .await
    .map_err(graphql_error)?;
let account = crate::daemon::web::reconcile_onboarding_provider_account(store, paths, account)
    .await
    .map_err(graphql_error)?;
```

For polling:

```rust
let attempt = provider_auth
    .poll_attempt(&attempt_id)
    .await
    .map_err(graphql_error)?;
if let Some(attempt) = &attempt {
    crate::daemon::web::persist_provider_account_status_from_attempt(store, attempt)
        .await
        .map_err(graphql_error)?;
}
```

For starting auth:

```rust
let request = crate::daemon::web::ProviderAuthStartRequest {
    provider_kind: input.provider_kind,
    provider_account_id: input.provider_account_id,
    method: input.method.into(),
};
let attempt = crate::daemon::web::start_provider_auth_attempt_view_from_parts(
    provider_auth,
    store,
    paths,
    request,
)
.await
.map_err(|error| async_graphql::Error::new(error.message()))?;
```

Add `start_provider_auth_attempt_view_from_parts` in `daemon/web/provider_auth.rs` by extracting the existing `start_provider_auth_attempt_view` body so web and GraphQL can use state parts without a `WebState`.

- [ ] **Step 8: Run GraphQL and daemon tests**

Run:

```bash
cargo test -p noema-core graphql_state_for_tests_has_runtime_state_accessors local_status_still_works_without_daemon_web_state
cargo test -p noema-core graphql_endpoint_accepts_post_path graphql_ws_endpoint_accepts_get_path
```

Expected: PASS.

- [ ] **Step 9: Commit Task 2**

Run:

```bash
git add crates/noema-core/src/graphql.rs crates/noema-core/src/graphql/runtime_state.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/src/graphql/chat.rs crates/noema-core/src/graphql/onboarding.rs crates/noema-core/src/daemon/web/mod.rs crates/noema-core/src/daemon/web/provider_auth.rs crates/noema-core/src/daemon/server.rs
git commit -m "refactor(core): decouple graphql from daemon web state"
```

---

### Task 3: Add Browser/Desktop GraphQL Transport Selection

**Files:**
- Create: `crates/noema-core/web/src/graphql/browserTransport.ts`
- Create: `crates/noema-core/web/src/graphql/desktopBridge.ts`
- Create: `crates/noema-core/web/src/graphql/desktopTransport.ts`
- Create: `crates/noema-core/web/src/graphql/transportMode.ts`
- Create: `crates/noema-core/web/src/graphql/desktopTransport.test.ts`
- Modify: `crates/noema-core/web/src/graphql/client.ts`

- [ ] **Step 1: Write failing transport mode tests**

Create `crates/noema-core/web/src/graphql/desktopTransport.test.ts`:

```ts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { isTauriRuntime } from "./transportMode";

describe("isTauriRuntime", () => {
  test("detects Tauri from window internals", () => {
    assert.equal(isTauriRuntime({ __TAURI_INTERNALS__: {} }), true);
    assert.equal(isTauriRuntime({}), false);
    assert.equal(isTauriRuntime(undefined), false);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cd crates/noema-core/web
bun test src/graphql/desktopTransport.test.ts
```

Expected: FAIL because `transportMode.ts` does not exist.

- [ ] **Step 3: Create transport mode detection**

Create `crates/noema-core/web/src/graphql/transportMode.ts`:

```ts
type TauriCandidate = {
  __TAURI_INTERNALS__?: unknown;
};

export function isTauriRuntime(candidate: TauriCandidate | undefined = globalThis.window as TauriCandidate | undefined) {
  return Boolean(candidate?.__TAURI_INTERNALS__);
}
```

- [ ] **Step 4: Extract the existing browser transport**

Create `crates/noema-core/web/src/graphql/browserTransport.ts`:

```ts
import { ApolloLink, HttpLink } from "@apollo/client";
import { GraphQLWsLink } from "@apollo/client/link/subscriptions";
import { OperationTypeNode } from "graphql";
import { createClient } from "graphql-ws";

function graphqlWsUrl() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/graphql/ws`;
}

export function createBrowserGraphqlLink() {
  const httpLink = new HttpLink({
    uri: "/graphql"
  });

  const wsLink = new GraphQLWsLink(
    createClient({
      url: graphqlWsUrl(),
      lazy: true,
      retryAttempts: 5
    })
  );

  return ApolloLink.split(
    ({ operationType }) => operationType === OperationTypeNode.SUBSCRIPTION,
    wsLink,
    httpLink
  );
}
```

- [ ] **Step 5: Add a minimal desktop bridge wrapper**

Create `crates/noema-core/web/src/graphql/desktopBridge.ts`:

```ts
type Unlisten = () => void;

type TauriCore = {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
};

type TauriEvent = {
  listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<Unlisten>;
};

async function importTauriCore(): Promise<TauriCore> {
  return await import("@tauri-apps/api/core");
}

async function importTauriEvent(): Promise<TauriEvent> {
  return await import("@tauri-apps/api/event");
}

export async function invokeDesktop<T>(command: string, args?: Record<string, unknown>) {
  const { invoke } = await importTauriCore();
  return invoke<T>(command, args);
}

export async function listenDesktop<T>(eventName: string, handler: (payload: T) => void) {
  const { listen } = await importTauriEvent();
  return listen<T>(eventName, (event) => handler(event.payload));
}
```

Add `@tauri-apps/api` to `crates/noema-core/web/package.json` dependencies if it is not already present:

```json
"@tauri-apps/api": "^2.0.0"
```

- [ ] **Step 6: Add desktop Apollo link**

Create `crates/noema-core/web/src/graphql/desktopTransport.ts`:

```ts
import { ApolloLink, Observable, type FetchResult, type Operation } from "@apollo/client";
import { print } from "graphql";
import { invokeDesktop, listenDesktop } from "./desktopBridge";

type DesktopGraphqlResponse = FetchResult<Record<string, unknown>>;

type DesktopSubscriptionPayload = {
  subscriptionId: string;
  response: DesktopGraphqlResponse;
};

function requestJson(operation: Operation) {
  return {
    query: print(operation.query),
    variables: operation.variables,
    operationName: operation.operationName
  };
}

export function createDesktopGraphqlLink() {
  return new ApolloLink((operation) => {
    if (operation.operationType === "subscription") {
      return new Observable<FetchResult>((observer) => {
        const subscriptionId = crypto.randomUUID();
        let disposed = false;
        let unlisten: (() => void) | null = null;

        void listenDesktop<DesktopSubscriptionPayload>("graphql_subscription_event", (payload) => {
          if (payload.subscriptionId !== subscriptionId) {
            return;
          }
          observer.next(payload.response);
        }).then((nextUnlisten) => {
          unlisten = nextUnlisten;
          if (disposed) {
            unlisten();
          }
        }).catch((error: unknown) => {
          observer.error(error);
        });

        void invokeDesktop("graphql_subscribe", {
          subscriptionId,
          requestJson: requestJson(operation)
        }).catch((error: unknown) => {
          observer.error(error);
        });

        return () => {
          disposed = true;
          unlisten?.();
          void invokeDesktop("graphql_unsubscribe", { subscriptionId });
        };
      });
    }

    return new Observable<FetchResult>((observer) => {
      void invokeDesktop<DesktopGraphqlResponse>("graphql_execute", {
        requestJson: requestJson(operation)
      }).then((response) => {
        observer.next(response);
        observer.complete();
      }).catch((error: unknown) => {
        observer.error(error);
      });
    });
  });
}
```

- [ ] **Step 7: Select transport in `client.ts`**

Replace `crates/noema-core/web/src/graphql/client.ts` with:

```ts
import { ApolloClient, InMemoryCache } from "@apollo/client";
import { createBrowserGraphqlLink } from "./browserTransport";
import { createDesktopGraphqlLink } from "./desktopTransport";
import { isTauriRuntime } from "./transportMode";

const link = isTauriRuntime() ? createDesktopGraphqlLink() : createBrowserGraphqlLink();

export const apolloClient = new ApolloClient({
  link,
  cache: new InMemoryCache({
    typePolicies: {
      GraphqlConversationItem: {
        keyFields: ["itemId"]
      },
      GraphqlConversationItemEvent: {
        keyFields: ["itemId"]
      },
      GraphqlAgentStatusEvent: {
        keyFields: false
      },
      GraphqlTurnCompletedEvent: {
        keyFields: false
      }
    }
  })
});
```

- [ ] **Step 8: Run focused frontend test**

Run:

```bash
cd crates/noema-core/web
bun test src/graphql/desktopTransport.test.ts
```

Expected: PASS.

- [ ] **Step 9: Commit Task 3**

Run:

```bash
git add crates/noema-core/web/package.json crates/noema-core/web/bun.lock crates/noema-core/web/src/graphql/client.ts crates/noema-core/web/src/graphql/browserTransport.ts crates/noema-core/web/src/graphql/desktopBridge.ts crates/noema-core/web/src/graphql/desktopTransport.ts crates/noema-core/web/src/graphql/transportMode.ts crates/noema-core/web/src/graphql/desktopTransport.test.ts
git commit -m "feat(web): add desktop graphql transport"
```

---

### Task 4: Add Desktop-Aware Provider Auth URL Opening

**Files:**
- Create: `crates/noema-core/web/src/graphql/externalUrls.ts`
- Create: `crates/noema-core/web/src/graphql/externalUrls.test.ts`
- Modify: `crates/noema-core/web/src/components/onboarding/AuthAttempt.tsx`

- [ ] **Step 1: Write failing URL opener tests**

Create `crates/noema-core/web/src/graphql/externalUrls.test.ts`:

```ts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { openExternalUrlForAuth } from "./externalUrls";

describe("openExternalUrlForAuth", () => {
  test("uses desktop command in Tauri mode", async () => {
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const handled = await openExternalUrlForAuth("https://example.com/login", {
      isDesktop: true,
      invoke: async (command, args) => {
        calls.push({ command, args });
      }
    });

    assert.equal(handled, true);
    assert.deepEqual(calls, [
      {
        command: "open_external_url",
        args: { url: "https://example.com/login" }
      }
    ]);
  });

  test("does not invoke desktop command outside Tauri mode", async () => {
    const calls: string[] = [];
    const handled = await openExternalUrlForAuth("https://example.com/login", {
      isDesktop: false,
      invoke: async (command) => {
        calls.push(command);
      }
    });

    assert.equal(handled, false);
    assert.deepEqual(calls, []);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cd crates/noema-core/web
bun test src/graphql/externalUrls.test.ts
```

Expected: FAIL because `externalUrls.ts` does not exist.

- [ ] **Step 3: Implement URL opener helper**

Create `crates/noema-core/web/src/graphql/externalUrls.ts`:

```ts
import { invokeDesktop } from "./desktopBridge";
import { isTauriRuntime } from "./transportMode";

type OpenExternalUrlOptions = {
  isDesktop?: boolean;
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
};

export async function openExternalUrlForAuth(url: string, options: OpenExternalUrlOptions = {}) {
  const isDesktop = options.isDesktop ?? isTauriRuntime();
  if (!isDesktop) {
    return false;
  }

  const invoke = options.invoke ?? invokeDesktop;
  await invoke("open_external_url", { url });
  return true;
}
```

- [ ] **Step 4: Wire helper into `AuthAttempt`**

Modify `crates/noema-core/web/src/components/onboarding/AuthAttempt.tsx`:

```tsx
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { openExternalUrlForAuth } from "@/graphql/externalUrls";
import { isTauriRuntime } from "@/graphql/transportMode";
import type { ProviderAuthAttemptView } from "./types";

export function AuthAttempt({ attempt }: { attempt: ProviderAuthAttemptView }) {
  async function openLoginPage(event: React.MouseEvent<HTMLAnchorElement>) {
    if (!attempt.verificationUrl) {
      return;
    }
    if (isTauriRuntime()) {
      event.preventDefault();
    }
    const handled = await openExternalUrlForAuth(attempt.verificationUrl);
    if (!handled && event.defaultPrevented) {
      window.open(attempt.verificationUrl, "_blank", "noopener,noreferrer");
    }
  }

  return (
    <Card className="grid min-w-0 max-w-[560px] gap-2.5" aria-live="polite">
      <CardContent className="grid gap-3.5">
        <strong>{attempt.status === "STARTING" ? "Starting login" : "Waiting for login"}</strong>
        {attempt.verificationUrl ? (
          <Button
            render={
              <a
                href={attempt.verificationUrl}
                target="_blank"
                rel="noreferrer"
                onClick={openLoginPage}
              />
            }
          >
            Open login page
          </Button>
        ) : null}
        {attempt.userCode ? (
          <code className="w-fit max-w-full rounded-md bg-[var(--surface-sunken)] px-2 py-1.5 font-mono text-lg leading-[1.35] whitespace-normal [overflow-wrap:anywhere]">
            {attempt.userCode}
          </code>
        ) : null}
        {attempt.instructions ? <p>{attempt.instructions}</p> : null}
        <p className="m-0 text-sm text-muted-foreground">Noema will continue automatically.</p>
      </CardContent>
    </Card>
  );
}
```

If TypeScript reports that `React.MouseEvent` requires an import, change the first line to:

```tsx
import type React from "react";
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
cd crates/noema-core/web
bun test src/graphql/externalUrls.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit Task 4**

Run:

```bash
git add crates/noema-core/web/src/graphql/externalUrls.ts crates/noema-core/web/src/graphql/externalUrls.test.ts crates/noema-core/web/src/components/onboarding/AuthAttempt.tsx
git commit -m "feat(web): open auth urls through desktop shell"
```

---

### Task 5: Add Desktop GraphQL IPC Bridge Tests And Commands

**Files:**
- Create: `crates/noema-desktop/Cargo.toml`
- Create: `crates/noema-desktop/tauri.conf.json`
- Create: `crates/noema-desktop/capabilities/default.json`
- Create: `crates/noema-desktop/src/main.rs`
- Create: `crates/noema-desktop/src/desktop_state.rs`
- Create: `crates/noema-desktop/src/graphql_ipc.rs`
- Create: `crates/noema-desktop/src/external_url.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add desktop crate to workspace**

Modify root `Cargo.toml`:

```toml
[workspace]
members = ["crates/noema-cli", "crates/noema-core", "crates/noema-desktop"]
resolver = "3"
```

Create `crates/noema-desktop/Cargo.toml`:

```toml
[package]
name = "noema-desktop"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[lints]
workspace = true

[dependencies]
async-graphql.workspace = true
futures-util.workspace = true
noema-core = { path = "../noema-core" }
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
tauri = { version = "2", features = [] }
tokio.workspace = true
url.workspace = true

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

- [ ] **Step 2: Create build script, Tauri config, and module shell**

Create `crates/noema-desktop/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

Create `crates/noema-desktop/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Noema",
  "version": "0.1.0",
  "identifier": "dev.noema.app",
  "build": {
    "beforeDevCommand": "cd ../noema-core/web && bun run dev:tauri",
    "beforeBuildCommand": "cd ../noema-core/web && bun run build:tauri",
    "devUrl": "http://127.0.0.1:5173",
    "frontendDist": "../noema-core/web/dist-tauri"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "Noema",
        "width": 1180,
        "height": 820,
        "minWidth": 860,
        "minHeight": 640
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": ["app", "dmg"],
    "macOS": {
      "signingIdentity": null
    }
  }
}
```

Create `crates/noema-desktop/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Noema desktop IPC permissions",
  "windows": ["main"],
  "permissions": [
    "core:default"
  ]
}
```

Create `crates/noema-desktop/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop_state;
mod external_url;
mod graphql_ipc;

fn main() {
    noema_desktop_main();
}

fn noema_desktop_main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            graphql_ipc::graphql_execute,
            graphql_ipc::graphql_subscribe,
            graphql_ipc::graphql_unsubscribe,
            external_url::open_external_url,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Noema desktop app");
}
```

- [ ] **Step 3: Write failing external URL validation test**

Create `crates/noema-desktop/src/external_url.rs`:

```rust
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    validate_external_url(&url).map_err(|error| error.to_string())?;
    Ok(())
}

fn validate_external_url(url: &str) -> Result<(), ExternalUrlError> {
    let parsed = url::Url::parse(url).map_err(|_| ExternalUrlError::InvalidUrl)?;
    match parsed.scheme() {
        "https" | "http" => Ok(()),
        _ => Err(ExternalUrlError::UnsupportedScheme),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ExternalUrlError {
    InvalidUrl,
    UnsupportedScheme,
}

impl std::fmt::Display for ExternalUrlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl => formatter.write_str("Noema could not open your browser."),
            Self::UnsupportedScheme => formatter.write_str("Noema could not open your browser."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_urls() {
        assert_eq!(validate_external_url("https://example.com"), Ok(()));
        assert_eq!(validate_external_url("http://example.com"), Ok(()));
    }

    #[test]
    fn rejects_non_web_urls() {
        assert_eq!(
            validate_external_url("file:///etc/passwd"),
            Err(ExternalUrlError::UnsupportedScheme)
        );
        assert_eq!(
            validate_external_url("not a url"),
            Err(ExternalUrlError::InvalidUrl)
        );
    }
}
```

- [ ] **Step 4: Run the external URL tests**

Run:

```bash
cargo test -p noema-desktop accepts_http_and_https_urls rejects_non_web_urls
```

Expected: PASS after the crate compiles. If `tauri::generate_context!()` fails before config exists, move this test run after Task 6 Step 2.

- [ ] **Step 5: Add desktop state and GraphQL execute command**

Create `crates/noema-desktop/src/desktop_state.rs`:

```rust
use noema_core::{
    NoemaRuntimeHost, RuntimeHostError,
    graphql::{self, GraphqlSchema},
};
use tokio::sync::Mutex;

pub struct DesktopState {
    inner: Mutex<Option<DesktopRuntime>>,
}

impl DesktopState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub async fn initialize(
        &self,
        codex: noema_core::CodexProviderConfig,
    ) -> Result<(), RuntimeHostError> {
        let host = NoemaRuntimeHost::start(codex).await?;
        let schema = graphql::build_schema(graphql::GraphqlState::from_runtime_host(&host));
        let mut inner = self.inner.lock().await;
        *inner = Some(DesktopRuntime { host, schema });
        Ok(())
    }

    pub async fn schema(&self) -> Result<GraphqlSchema, String> {
        let inner = self.inner.lock().await;
        inner
            .as_ref()
            .map(|runtime| runtime.schema.clone())
            .ok_or_else(|| "Noema lost connection to its local app service.".to_string())
    }

    pub async fn shutdown(&self) {
        let runtime = {
            let mut inner = self.inner.lock().await;
            inner.take()
        };
        if let Some(runtime) = runtime {
            runtime.host.shutdown().await;
        }
    }
}

pub struct DesktopRuntime {
    host: NoemaRuntimeHost,
    schema: GraphqlSchema,
}
```

Create `crates/noema-desktop/src/graphql_ipc.rs`:

```rust
use async_graphql::{Request, Response};
use serde_json::Value;
use tauri::State;

use crate::desktop_state::DesktopState;

#[tauri::command]
pub async fn graphql_execute(
    state: State<'_, DesktopState>,
    request_json: Value,
) -> Result<Value, String> {
    let request: Request = serde_json::from_value(request_json)
        .map_err(|_| "Noema lost connection to its local app service.".to_string())?;
    let schema = state.schema().await?;
    let response: Response = schema.execute(request).await;
    serde_json::to_value(response)
        .map_err(|_| "Noema lost connection to its local app service.".to_string())
}

#[tauri::command]
pub async fn graphql_subscribe(
    _state: State<'_, DesktopState>,
    _subscription_id: String,
    _request_json: Value,
) -> Result<(), String> {
    Err("Noema lost connection to its local app service.".to_string())
}

#[tauri::command]
pub async fn graphql_unsubscribe(
    _state: State<'_, DesktopState>,
    _subscription_id: String,
) -> Result<(), String> {
    Ok(())
}
```

The subscription command returns a controlled error in this task so command registration compiles. Task 7 replaces it with streaming behavior before manual desktop verification.

- [ ] **Step 6: Wire managed state in `main.rs`**

Update `main.rs`:

```rust
fn noema_desktop_main() {
    tauri::Builder::default()
        .manage(desktop_state::DesktopState::new())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<desktop_state::DesktopState>();
                let codex = noema_core::Config::load_daemon(None, noema_core::CliOverrides::default())
                    .map(|config| config.codex)
                    .unwrap_or_default();
                if let Err(error) = state.initialize(codex).await {
                    eprintln!("{} {}", error.user_message(), error.technical_details());
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<desktop_state::DesktopState>();
                tauri::async_runtime::block_on(async move {
                    state.shutdown().await;
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            graphql_ipc::graphql_execute,
            graphql_ipc::graphql_subscribe,
            graphql_ipc::graphql_unsubscribe,
            external_url::open_external_url,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Noema desktop app");
}
```

- [ ] **Step 7: Commit Task 5**

Run:

```bash
git add Cargo.toml crates/noema-desktop/Cargo.toml crates/noema-desktop/build.rs crates/noema-desktop/tauri.conf.json crates/noema-desktop/capabilities/default.json crates/noema-desktop/src/main.rs crates/noema-desktop/src/desktop_state.rs crates/noema-desktop/src/graphql_ipc.rs crates/noema-desktop/src/external_url.rs
git commit -m "feat(desktop): add tauri ipc shell"
```

---

### Task 6: Add Desktop Vite Build Scripts

**Files:**
- Create: `crates/noema-core/web/vite.desktop.config.ts`
- Modify: `crates/noema-core/web/package.json`

- [ ] **Step 1: Add desktop Vite config**

Create `crates/noema-core/web/vite.desktop.config.ts`:

```ts
import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "./",
  publicDir: "public",
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src")
    }
  },
  build: {
    outDir: "dist-tauri",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: "assets/app.js",
        chunkFileNames: "assets/[name].js",
        assetFileNames: (assetInfo) => {
          if (assetInfo.names.some((name) => name.endsWith(".css"))) {
            return "assets/styles.css";
          }
          return "assets/[name][extname]";
        }
      }
    }
  }
});
```

This desktop build deliberately uses `base: "./"` and a separate `dist-tauri`
folder so the Tauri bundle does not reuse daemon web assets whose `index.html`
points at `/assets/...`.

- [ ] **Step 2: Add Tauri frontend scripts**

Add Tauri-specific frontend scripts to `crates/noema-core/web/package.json`:

```json
"dev:tauri": "bun run gen:types && vite --host 127.0.0.1",
"build:tauri": "bun run gen:types && vite build --config vite.desktop.config.ts"
```

- [ ] **Step 3: Run frontend desktop build**

Run:

```bash
cd crates/noema-core/web
bun run build:tauri
```

Expected: PASS and `crates/noema-core/web/dist-tauri/index.html` references relative `./assets/...` files.

- [ ] **Step 4: Run manifest checks**

Run:

```bash
cargo check -p noema-desktop
```

Expected: PASS, or fail only on missing downloaded dependencies in an offline environment. If dependency download is blocked, rerun with network approval during execution.

- [ ] **Step 5: Commit Task 6**

Run:

```bash
git add crates/noema-core/web/package.json crates/noema-core/web/vite.desktop.config.ts
git commit -m "build(web): add tauri frontend build"
```

---

### Task 7: Implement Desktop Subscription Streaming

**Files:**
- Modify: `crates/noema-desktop/src/desktop_state.rs`
- Modify: `crates/noema-desktop/src/graphql_ipc.rs`
- Test: `crates/noema-desktop/src/graphql_ipc.rs`

- [ ] **Step 1: Write a subscription id matching test**

Add to `crates/noema-desktop/src/graphql_ipc.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_event_payload_keeps_subscription_id() {
        let payload = SubscriptionEventPayload {
            subscription_id: "sub_1".to_string(),
            response: serde_json::json!({"data": {"ok": true}}),
        };

        assert_eq!(payload.subscription_id, "sub_1");
        assert_eq!(payload.response["data"]["ok"], true);
    }
}
```

- [ ] **Step 2: Add subscription task storage**

In `desktop_state.rs`, change `DesktopRuntime`:

```rust
use std::collections::HashMap;
use tokio::task::JoinHandle;

pub struct DesktopRuntime {
    host: NoemaRuntimeHost,
    schema: GraphqlSchema,
    subscriptions: HashMap<String, JoinHandle<()>>,
}
```

Initialize it:

```rust
*inner = Some(DesktopRuntime {
    host,
    schema,
    subscriptions: HashMap::new(),
});
```

Add methods:

```rust
pub async fn insert_subscription(&self, id: String, handle: JoinHandle<()>) -> Result<(), String> {
    let mut inner = self.inner.lock().await;
    let runtime = inner
        .as_mut()
        .ok_or_else(|| "Noema lost connection to its local app service.".to_string())?;
    if let Some(previous) = runtime.subscriptions.insert(id, handle) {
        previous.abort();
    }
    Ok(())
}

pub async fn remove_subscription(&self, id: &str) {
    let mut inner = self.inner.lock().await;
    if let Some(runtime) = inner.as_mut()
        && let Some(handle) = runtime.subscriptions.remove(id)
    {
        handle.abort();
    }
}
```

Before shutting down host:

```rust
for (_, handle) in runtime.subscriptions {
    handle.abort();
}
runtime.host.shutdown().await;
```

- [ ] **Step 3: Implement subscription commands**

Replace `graphql_subscribe` and `graphql_unsubscribe` in `graphql_ipc.rs`:

```rust
use async_graphql::{Request, Response};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use tauri::{Emitter, State, Window};

#[derive(Clone, Debug, Serialize)]
struct SubscriptionEventPayload {
    #[serde(rename = "subscriptionId")]
    subscription_id: String,
    response: Value,
}

#[tauri::command]
pub async fn graphql_subscribe(
    window: Window,
    state: State<'_, DesktopState>,
    subscription_id: String,
    request_json: Value,
) -> Result<(), String> {
    let request: Request = serde_json::from_value(request_json)
        .map_err(|_| "Noema lost connection to its local app service.".to_string())?;
    let schema = state.schema().await?;
    let stream_id = subscription_id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let mut stream = schema.execute_stream(request);
        while let Some(response) = stream.next().await {
            let response_json = match serde_json::to_value(response) {
                Ok(value) => value,
                Err(_) => break,
            };
            let payload = SubscriptionEventPayload {
                subscription_id: stream_id.clone(),
                response: response_json,
            };
            if window.emit("graphql_subscription_event", payload).is_err() {
                break;
            }
        }
    });
    state.insert_subscription(subscription_id, handle).await
}

#[tauri::command]
pub async fn graphql_unsubscribe(
    state: State<'_, DesktopState>,
    subscription_id: String,
) -> Result<(), String> {
    state.remove_subscription(&subscription_id).await;
    Ok(())
}
```

- [ ] **Step 4: Run subscription bridge test**

Run:

```bash
cargo test -p noema-desktop subscription_event_payload_keeps_subscription_id
```

Expected: PASS.

- [ ] **Step 5: Commit Task 7**

Run:

```bash
git add crates/noema-desktop/src/desktop_state.rs crates/noema-desktop/src/graphql_ipc.rs
git commit -m "feat(desktop): stream graphql subscriptions over tauri"
```

---

### Task 8: Finish Native URL Opening

**Files:**
- Modify: `crates/noema-desktop/Cargo.toml`
- Modify: `crates/noema-desktop/src/external_url.rs`

- [ ] **Step 1: Add URL opener dependency**

Add to `crates/noema-desktop/Cargo.toml`:

```toml
open = "5"
```

- [ ] **Step 2: Call the system browser from the command**

Update `open_external_url` in `external_url.rs`:

```rust
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    validate_external_url(&url).map_err(|error| error.to_string())?;
    open::that_detached(&url).map_err(|_| "Noema could not open your browser.".to_string())
}
```

- [ ] **Step 3: Run external URL tests**

Run:

```bash
cargo test -p noema-desktop accepts_http_and_https_urls rejects_non_web_urls
```

Expected: PASS.

- [ ] **Step 4: Commit Task 8**

Run:

```bash
git add crates/noema-desktop/Cargo.toml crates/noema-desktop/src/external_url.rs Cargo.lock
git commit -m "feat(desktop): open auth urls in system browser"
```

---

### Task 9: Validate End-To-End Desktop And Existing Web Modes

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Run frontend type generation, lint, and build**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: all commands PASS. If `bun.lock` changes because `@tauri-apps/api` was added, include it in the final commit for the frontend task that introduced it.

- [ ] **Step 2: Run Rust validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all commands PASS. If tests that bind sockets fail under sandboxing with `PermissionDenied`, rerun the same command with socket permissions and record the distinction in the final summary.

- [ ] **Step 3: Launch existing daemon web mode**

Run:

```bash
cargo run -p noema-cli -- start
```

Expected: logs include a local web URL. Open that URL in the browser and verify:

- The app renders.
- `LocalStatus` and onboarding load.
- Existing browser GraphQL HTTP/WebSocket mode still works.

Stop the daemon with `Ctrl-C`.

- [ ] **Step 4: Launch desktop dev mode**

Run the Tauri dev command selected in Task 6:

```bash
cargo tauri dev --config crates/noema-desktop/tauri.conf.json
```

Expected:

- A Noema desktop window opens.
- The React app renders in the window.
- Opening the app does not require `http://127.0.0.1:3737`.
- `LocalStatus` and onboarding load through IPC.
- Provider auth opens the default browser when a verification URL exists.
- Starting the primary conversation works after onboarding.
- Sending a message produces subscription-driven transcript updates.

- [ ] **Step 5: Build unsigned developer bundle**

Run:

```bash
cargo tauri build --config crates/noema-desktop/tauri.conf.json
```

Expected: an unsigned macOS developer app bundle is produced under the Tauri target bundle output.

- [ ] **Step 6: Update durable context**

Add a short settled decision to `docs/context/current.md` under `Settled Decisions`:

```markdown
- The first macOS desktop app direction is a Tauri app in `crates/noema-desktop`
  that starts a Noema runtime host inside the app process, loads the existing
  React UI from bundled assets, and uses Tauri IPC/events for GraphQL instead
  of exposing a local HTTP/WebSocket server.
```

Add this open loop:

```markdown
- Add signing, notarization, update, and production distribution for the Tauri
  macOS app after the unsigned developer build is stable.
```

- [ ] **Step 7: Commit validation/context update**

Run:

```bash
git add docs/context/current.md
git commit -m "docs: record tauri desktop direction"
```

---

### Task 10: Final Ship Checklist For The Implementation Branch

**Files:**
- Inspect all files changed by Tasks 1-9.

- [ ] **Step 1: Check worktree**

Run:

```bash
git status --short --branch
```

Expected: only intentional untracked/unstaged files remain. Generated bundles under `target/` should not be staged.

- [ ] **Step 2: Check whitespace**

Run:

```bash
git diff --check
```

Expected: no output and exit code 0.

- [ ] **Step 3: Inspect staged changes if preparing a final commit**

Run:

```bash
git diff --cached --stat
git diff --cached --name-status
```

Expected: staged scope matches the final intended commit. If there is no final commit because each task already committed, note that in the summary.

- [ ] **Step 4: Summarize remaining risks**

Report:

- Whether desktop mode binds no Noema HTTP/WebSocket listener by code structure.
- Whether manual desktop launch passed.
- Whether unsigned bundle build passed.
- Whether existing daemon web mode still passed.
- Any dependency/network/install issue encountered with Tauri.
- Any unrelated dirty worktree files preserved.
