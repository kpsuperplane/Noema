# MCP Server Settings Config Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the guided Settings flow for adding an MCP server, storing disk-backed secrets, verifying metadata-only MCP connectivity, fetching tool schemas, and immediately prompting tool calibration.

**Architecture:** Add a backend setup service that validates GraphQL input, writes safe config to SurrealDB, stores secret material under `${NOEMA_HOME}/mcp/<mcp_server_id>/secrets.json`, runs metadata-only MCP discovery, persists discovered tools, and returns a setup status. The web Settings pane becomes a focused setup wizard layered above the existing server list, using generated GraphQL mutations and local form parsing for `KEY=value` env/header textareas.

**Tech Stack:** Rust, SurrealDB, async-graphql, serde_json, existing MCP client runtime, React, Apollo Client, Bun/Vite.

---

## File Structure

- Modify `crates/noema-core/src/paths.rs`: add `mcp_dir()` and `mcp_server_home()` helpers with sanitized server ids.
- Create `crates/noema-core/src/mcp/secrets.rs`: disk-backed MCP secret file model and private file writing.
- Modify `crates/noema-core/src/mcp.rs`: export the new `secrets` module.
- Modify `crates/noema-core/src/mcp/client.rs`: add an auth-needed error variant used by setup status mapping.
- Modify `crates/noema-core/src/store/schema.rs`: add `needs_auth` to `mcp_servers.auth_status` if not already accepted.
- Modify `crates/noema-core/src/store/mcp.rs`: add server status update helpers and enum string helpers.
- Modify `crates/noema-core/src/store/tests/mcp.rs`: add focused store/status and safe-config tests.
- Create `crates/noema-core/src/mcp/setup.rs`: setup orchestration, validation, id generation, metadata persistence, status mapping, and test fake transport.
- Modify `crates/noema-core/src/graphql/mcp.rs`: add create/continue setup input and result types plus resolver functions.
- Modify `crates/noema-core/src/graphql/schema.rs`: expose the new mutations and add GraphQL tests.
- Modify `crates/noema-core/web/src/graphql/operations.ts`: add `CreateMcpServer` and `ContinueMcpServerSetup` mutations.
- Modify generated files: `crates/noema-core/web/src/generated/schema.graphql`, `crates/noema-core/web/src/generated/graphql.ts`, and `crates/noema-core/src/daemon/web/assets/app.js`.
- Create `crates/noema-core/web/src/components/settings/mcpSetupForm.ts`: parse and validate `KEY=value` textareas.
- Create `crates/noema-core/web/src/components/settings/McpServerSetupFlow.tsx`: guided setup UI and calibration prompt.
- Modify `crates/noema-core/web/src/components/settings/McpSettingsPane.tsx`: wire mutations, refetch, and setup state.
- Modify `crates/noema-core/web/src/components/settings/McpSettingsPaneContent.tsx`: render add action and setup flow above server list.
- Modify `crates/noema-core/web/src/pages/SettingsPage.test.tsx`: add UI coverage for add/setup/calibration states.
- Create `crates/noema-core/web/src/components/settings/mcpSetupForm.test.ts`: parser tests.
- Modify `docs/context/current.md` and `docs/frontend/governance-inspection.md`: sync the now-implemented MCP setup behavior after code lands.

## Task 1: MCP Secret Paths And Disk Store

**Files:**
- Modify: `crates/noema-core/src/paths.rs`
- Create: `crates/noema-core/src/mcp/secrets.rs`
- Modify: `crates/noema-core/src/mcp.rs`
- Test: `crates/noema-core/src/paths.rs`
- Test: `crates/noema-core/src/mcp/secrets.rs`

- [ ] **Step 1: Add failing path tests**

Add these tests to `crates/noema-core/src/paths.rs`:

```rust
#[test]
fn mcp_server_home_is_under_noema_mcp_dir() {
    let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

    assert_eq!(paths.mcp_dir(), PathBuf::from("/tmp/noema/mcp"));
    assert_eq!(
        paths.mcp_server_home("mcp:GitHub/Default"),
        PathBuf::from("/tmp/noema/mcp/mcp_GitHub_Default")
    );
}
```

- [ ] **Step 2: Run the path test to verify it fails**

Run: `cargo test -p noema-core paths::tests::mcp_server_home_is_under_noema_mcp_dir --no-fail-fast`

Expected: fail because `mcp_dir` and `mcp_server_home` are not defined.

- [ ] **Step 3: Implement MCP path helpers**

Add methods to `impl NoemaPaths` in `crates/noema-core/src/paths.rs`:

```rust
/// Path to the MCP server configuration root.
#[must_use]
pub fn mcp_dir(&self) -> PathBuf {
    self.root.join("mcp")
}

/// Path to one MCP server's private configuration home.
#[must_use]
pub fn mcp_server_home(&self, mcp_server_id: &str) -> PathBuf {
    self.mcp_dir().join(sanitize_path_segment(mcp_server_id))
}
```

- [ ] **Step 4: Run the path test to verify it passes**

Run: `cargo test -p noema-core paths::tests::mcp_server_home_is_under_noema_mcp_dir --no-fail-fast`

Expected: pass.

- [ ] **Step 5: Add failing secret-store tests**

Create `crates/noema-core/src/mcp/secrets.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secret_file_round_trips_env_and_headers_without_safe_config() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("mcp_server");
        let secrets = McpSecretMaterial {
            env: map_from_pairs([("GITHUB_TOKEN", "secret")]),
            headers: map_from_pairs([("Authorization", "Bearer secret")]),
        };

        write_mcp_secrets(&home, &secrets).expect("write secrets");
        let loaded = read_mcp_secrets(&home).expect("read secrets");

        assert_eq!(loaded, secrets);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string(home.join("secrets.json")).expect("secret file")
            )
            .expect("json"),
            json!({
                "env": { "GITHUB_TOKEN": "secret" },
                "headers": { "Authorization": "Bearer secret" }
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn secret_home_and_file_are_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("mcp_server");

        write_mcp_secrets(&home, &McpSecretMaterial::default()).expect("write secrets");

        assert_eq!(
            std::fs::metadata(&home).expect("home metadata").permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(home.join("secrets.json"))
                .expect("file metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    fn map_from_pairs<const N: usize>(pairs: [(&str, &str); N]) -> std::collections::BTreeMap<String, String> {
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }
}
```

- [ ] **Step 6: Run the secret tests to verify they fail**

Run: `cargo test -p noema-core mcp::secrets --no-fail-fast`

Expected: fail because the module and functions are not wired yet.

- [ ] **Step 7: Implement the secret module**

In `crates/noema-core/src/mcp.rs`, add:

```rust
pub mod secrets;
```

In `crates/noema-core/src/mcp/secrets.rs`, implement:

```rust
//! Disk-backed MCP secret storage.

use std::{collections::BTreeMap, fs, io, path::Path};

use serde::{Deserialize, Serialize};

const MCP_SECRET_FILE: &str = "secrets.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpSecretMaterial {
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

pub fn write_mcp_secrets(server_home: &Path, secrets: &McpSecretMaterial) -> io::Result<()> {
    create_private_dir_all(server_home)?;
    let path = server_home.join(MCP_SECRET_FILE);
    let bytes = serde_json::to_vec_pretty(secrets).map_err(io::Error::other)?;
    fs::write(&path, bytes)?;
    set_private_file_permissions(&path)
}

pub fn read_mcp_secrets(server_home: &Path) -> io::Result<McpSecretMaterial> {
    let path = server_home.join(MCP_SECRET_FILE);
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

#[cfg(unix)]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}
```

- [ ] **Step 8: Run Task 1 tests**

Run:

```bash
cargo test -p noema-core paths::tests::mcp_server_home_is_under_noema_mcp_dir mcp::secrets --no-fail-fast
```

Expected: all Task 1 tests pass.

- [ ] **Step 9: Commit Task 1**

```bash
git add crates/noema-core/src/paths.rs crates/noema-core/src/mcp.rs crates/noema-core/src/mcp/secrets.rs
git commit -m "feat: add mcp secret storage"
```

## Task 2: Store Status Helpers And Metadata Persistence Hooks

**Files:**
- Modify: `crates/noema-core/src/mcp/client.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/mcp.rs`
- Modify: `crates/noema-core/src/store/tests/mcp.rs`

- [ ] **Step 1: Add failing tests for auth status and server health updates**

In `crates/noema-core/src/store/tests/mcp.rs`, add:

```rust
#[tokio::test]
async fn mcp_server_status_can_be_updated_after_setup() {
    let store = test_store().await;
    store
        .create_mcp_server(NewMcpServer {
            mcp_server_id: "mcp:setup".to_string(),
            display_name: "Setup".to_string(),
            transport_kind: McpTransportKind::Stdio,
            safe_config: json!({ "command": "test-mcp" }),
        })
        .await
        .expect("server");

    let updated = store
        .update_mcp_server_setup_status(
            "mcp:setup",
            McpServerHealthStatus::Healthy,
            McpServerAuthStatus::NeedsAuth,
        )
        .await
        .expect("updated server");

    assert_eq!(updated.health_status, McpServerHealthStatus::Healthy);
    assert_eq!(updated.auth_status, McpServerAuthStatus::NeedsAuth);
}
```

- [ ] **Step 2: Run the status test to verify it fails**

Run: `cargo test -p noema-core mcp_server_status_can_be_updated_after_setup --no-fail-fast`

Expected: fail because `update_mcp_server_setup_status` is not defined or `needs_auth` is not accepted.

- [ ] **Step 3: Add `NeedsAuth` enum support**

In `crates/noema-core/src/store/schema.rs`, ensure the auth status assertion includes `needs_auth`:

```surql
DEFINE FIELD OVERWRITE auth_status ON TABLE mcp_servers TYPE string ASSERT $value INSIDE ['none', 'needs_auth', 'authenticated', 'unavailable'];
```

In `crates/noema-core/src/store/mcp.rs`, keep or add `McpServerAuthStatus::NeedsAuth` and make sure the parser accepts it:

```rust
fn parse_mcp_server_auth_status(value: &str) -> Result<McpServerAuthStatus, StoreError> {
    match value {
        "none" => Ok(McpServerAuthStatus::None),
        "needs_auth" => Ok(McpServerAuthStatus::NeedsAuth),
        "authenticated" => Ok(McpServerAuthStatus::Authenticated),
        "unavailable" => Ok(McpServerAuthStatus::Unavailable),
        _ => invalid_enum("mcp_server_auth_status", value),
    }
}
```

- [ ] **Step 4: Implement server setup status update helper**

Add to `impl NoemaStore` in `crates/noema-core/src/store/mcp.rs`:

```rust
/// Update MCP server setup health/auth status after metadata discovery.
///
/// # Errors
///
/// Returns [`StoreError`] when the server is missing or the embedded store write fails.
pub async fn update_mcp_server_setup_status(
    &self,
    mcp_server_id: &str,
    health_status: McpServerHealthStatus,
    auth_status: McpServerAuthStatus,
) -> Result<McpServerRecord, StoreError> {
    self.db
        .query(
            r#"
            UPDATE mcp_servers SET
              health_status = $health_status,
              auth_status = $auth_status,
              updated_at = time::now()
            WHERE mcp_server_id = $mcp_server_id;
            "#,
        )
        .bind(("mcp_server_id", mcp_server_id.to_string()))
        .bind(("health_status", health_status.as_str().to_string()))
        .bind(("auth_status", auth_status.as_str().to_string()))
        .await?
        .check()?;

    self.get_mcp_server(mcp_server_id).await?.ok_or_else(|| {
        StoreError::Schema(format!("missing MCP server after status update: {mcp_server_id}"))
    })
}
```

If `McpServerHealthStatus` and `McpServerAuthStatus` do not have `as_str`, add const methods matching the existing parse labels.

- [ ] **Step 5: Add MCP client auth-needed error**

In `crates/noema-core/src/mcp/client.rs`, add an error variant:

```rust
/// The MCP server requires credentials before metadata can be listed.
#[error("MCP authentication required: {0}")]
AuthRequired(String),
```

Add a unit test that a fake transport returning `AuthRequired("missing token".into())` from `initialize` makes `discover_tools` return the same variant.

- [ ] **Step 6: Run Task 2 tests**

Run:

```bash
cargo test -p noema-core mcp_server_status_can_be_updated_after_setup metadata_discovery_lists_tools_without_calling_them --no-fail-fast
```

Expected: all selected tests pass.

- [ ] **Step 7: Commit Task 2**

```bash
git add crates/noema-core/src/mcp/client.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/mcp.rs crates/noema-core/src/store/tests/mcp.rs
git commit -m "feat: track mcp setup status"
```

## Task 3: Backend MCP Setup Service

**Files:**
- Create: `crates/noema-core/src/mcp/setup.rs`
- Modify: `crates/noema-core/src/mcp.rs`
- Modify: `crates/noema-core/src/lib.rs` if setup types need crate-level exports
- Test: `crates/noema-core/src/mcp/setup.rs`

- [ ] **Step 1: Add failing setup service tests**

Create `crates/noema-core/src/mcp/setup.rs` with tests first:

- `create_stdio_setup_writes_secret_env_persists_safe_config_and_discovers_tools`: build a `NewMcpServerSetup` with `display_name = "GitHub"`, `transport_kind = Stdio`, safe config `{ "command": "npx", "args": ["-y", "server"], "env": { "GITHUB_OWNER": "example" } }`, and secret env `{ "GITHUB_TOKEN": "secret" }`; fake discovery returns one tool named `list_repos`; assert `setup_status = ReadyForCalibration`, persisted `safe_config` has `secret_refs.env = ["GITHUB_TOKEN"]`, the secret file contains the token, and `list_mcp_tools_for_server("mcp:github")` returns `list_repos`.
- `create_http_sse_setup_writes_secret_headers_and_discovers_tools`: build a `NewMcpServerSetup` with `transport_kind = HttpSse`, safe config `{ "url": "https://example.com/mcp", "headers": { "X-Team": "infra" } }`, and secret headers `{ "Authorization": "Bearer secret" }`; assert `secret_refs.headers = ["Authorization"]`, no secret value appears in `server.safe_config`, and discovered tools are persisted.
- `auth_required_setup_returns_needs_auth_without_secret_values`: fake discovery returns `McpClientError::AuthRequired("missing authorization".to_string())`; assert `setup_status = NeedsAuth`, `setup_error = Some("missing authorization")`, `server.auth_status = NeedsAuth`, and the debug string for the setup result does not contain the submitted secret value.
- `continue_setup_updates_secrets_and_retries_discovery`: create a saved server with an empty secret file, call `continue_mcp_server_setup` with `secret_headers.Authorization = "Bearer retry"`, fake discovery succeeds, and assert the final status is `ReadyForCalibration` with the new header value present only in `secrets.json`.
- `safe_config_rejects_secret_shaped_keys`: attempt setup with safe env `{ "GITHUB_TOKEN": "not-safe" }` and safe headers `{ "Authorization": "not-safe" }`; assert the call returns a schema error naming the unsafe key.

Use a fake `McpSetupTransportFactory` trait in the test module so no real subprocess or network call runs.

- [ ] **Step 2: Run setup tests to verify they fail**

Run: `cargo test -p noema-core mcp::setup --no-fail-fast`

Expected: fail because setup types are missing.

- [ ] **Step 3: Implement setup input/result types**

In `crates/noema-core/src/mcp/setup.rs`, define:

```rust
use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::{
    mcp::{
        client::{DiscoveredMcpTool, McpClientError, McpClientRuntime, McpTransport},
        secrets::{write_mcp_secrets, McpSecretMaterial},
    },
    McpServerAuthStatus, McpServerHealthStatus, McpTransportKind, NewMcpServer, NewMcpTool,
    NoemaPaths, NoemaStore, StoreError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSetupStatus {
    NeedsAuth,
    ReadyForCalibration,
    Unavailable,
    Malformed,
}

impl McpSetupStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeedsAuth => "needs_auth",
            Self::ReadyForCalibration => "ready_for_calibration",
            Self::Unavailable => "unavailable",
            Self::Malformed => "malformed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpServerSetupResult {
    pub server: crate::McpServerRecord,
    pub setup_status: McpSetupStatus,
    pub discovery_status: Option<String>,
    pub discovered_tool_count: usize,
    pub setup_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpServerSetup {
    pub display_name: String,
    pub transport_kind: McpTransportKind,
    pub safe_config: Value,
    pub secrets: McpSecretMaterial,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContinueMcpServerSetup {
    pub mcp_server_id: String,
    pub secrets: McpSecretMaterial,
}
```

- [ ] **Step 4: Implement validation helpers**

Add helpers that:

- trim `display_name`
- reject empty display names
- reject safe map keys containing `authorization`, `token`, `api_key`, `apikey`, `cookie`, `password`, `credential`, `secret`, `private_key`, `session`, `set-cookie`
- require safe/secret maps to already be `BTreeMap<String, String>`
- require `stdio.command` non-empty
- require `http_sse.url` starts with `http://` or `https://`

Use `StoreError::Schema("...".to_string())` for validation failures so GraphQL can surface plain messages.

- [ ] **Step 5: Implement server id and safe config creation**

Add server id generation:

```rust
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
```

If a collision exists, append `-2`, `-3`, and so on by checking `store.get_mcp_server(&candidate).await?`.

- [ ] **Step 6: Implement discovery persistence**

Add a function that converts `DiscoveredMcpTool` to `NewMcpTool` and calls `store.upsert_mcp_tool`. Fingerprint can be deterministic for V1:

```rust
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
```

Add a local deterministic FNV-1a helper in `setup.rs`:

```rust
fn stable_hex_fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
```

Then return `format!("mcp-tool-metadata:v1:{}", stable_hex_fingerprint(json_string.as_bytes()))`.

- [ ] **Step 7: Implement setup orchestration**

Implement functions shaped like:

```rust
pub async fn create_mcp_server_setup<T>(
    store: &NoemaStore,
    paths: &NoemaPaths,
    input: NewMcpServerSetup,
    make_transport: impl Fn(&crate::McpServerRecord, &McpSecretMaterial) -> T,
) -> Result<McpServerSetupResult, StoreError>
where
    T: McpTransport,
{
    let server = create_validated_server_record(store, paths, input).await?;
    discover_and_persist_tools(store, &server, make_transport).await
}
```

and:

```rust
pub async fn continue_mcp_server_setup<T>(
    store: &NoemaStore,
    paths: &NoemaPaths,
    input: ContinueMcpServerSetup,
    make_transport: impl Fn(&crate::McpServerRecord, &McpSecretMaterial) -> T,
) -> Result<McpServerSetupResult, StoreError>
where
    T: McpTransport,
{
    let server = update_saved_server_secrets(store, paths, input).await?;
    discover_and_persist_tools(store, &server, make_transport).await
}
```

Map `McpClientError::AuthRequired` to `NeedsAuth` plus `auth_status = needs_auth`. Map `Transport` to `Unavailable`. Map `Malformed` to `Malformed`.

- [ ] **Step 8: Wire module export**

In `crates/noema-core/src/mcp.rs`, add:

```rust
pub mod setup;
```

- [ ] **Step 9: Run Task 3 tests**

Run: `cargo test -p noema-core mcp::setup --no-fail-fast`

Expected: all setup tests pass and no real MCP process/network is used.

- [ ] **Step 10: Commit Task 3**

```bash
git add crates/noema-core/src/mcp.rs crates/noema-core/src/mcp/setup.rs
git commit -m "feat: add mcp server setup service"
```

## Task 4: GraphQL MCP Setup Mutations

**Files:**
- Modify: `crates/noema-core/src/graphql/mcp.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Test: `crates/noema-core/src/graphql/schema.rs`

- [ ] **Step 1: Add failing GraphQL mutation tests**

In `crates/noema-core/src/graphql/schema.rs`, add tests:

- `create_mcp_server_mutation_returns_ready_for_calibration_without_secrets`: execute a `createMcpServer` mutation with stdio safe env plus secret env, assert `setupStatus = "ready_for_calibration"`, `discoveredToolCount = 1`, response JSON contains the server metadata, and response JSON does not contain the secret value.
- `create_mcp_server_mutation_rejects_secret_shaped_safe_keys`: execute `createMcpServer` with safe `headers: { Authorization: "Bearer unsafe" }`, assert GraphQL returns an error containing `Authorization`.
- `continue_mcp_server_setup_mutation_returns_needs_auth_or_ready_status`: create a saved server whose first fake discovery returns `needs_auth`, execute `continueMcpServerSetup` with a secret header, assert the retry response transitions to `ready_for_calibration` and still omits the secret value.

Use `GraphqlState::for_tests_with_store(store)` and the fake setup transport hook added for tests. If `GraphqlState` cannot carry a setup transport factory yet, add a test-only constructor in this task.

- [ ] **Step 2: Run GraphQL tests to verify they fail**

Run:

```bash
cargo test -p noema-core create_mcp_server_mutation_returns_ready_for_calibration_without_secrets create_mcp_server_mutation_rejects_secret_shaped_safe_keys continue_mcp_server_setup_mutation_returns_needs_auth_or_ready_status --no-fail-fast
```

Expected: fail because mutations and input types are missing.

- [ ] **Step 3: Add GraphQL input/result types**

In `crates/noema-core/src/graphql/mcp.rs`, add:

```rust
#[derive(Clone, Debug, InputObject)]
pub struct GraphqlCreateMcpServerInput {
    pub display_name: String,
    pub transport_kind: String,
    pub stdio: Option<GraphqlMcpStdioConfigInput>,
    pub http_sse: Option<GraphqlMcpHttpSseConfigInput>,
}

#[derive(Clone, Debug, InputObject)]
pub struct GraphqlMcpStdioConfigInput {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Option<Json<Value>>,
    pub secret_env: Option<Json<Value>>,
}

#[derive(Clone, Debug, InputObject)]
pub struct GraphqlMcpHttpSseConfigInput {
    pub url: String,
    pub headers: Option<Json<Value>>,
    pub secret_headers: Option<Json<Value>>,
}

#[derive(Clone, Debug, InputObject)]
pub struct GraphqlContinueMcpServerSetupInput {
    pub mcp_server_id: String,
    pub secret_env: Option<Json<Value>>,
    pub secret_headers: Option<Json<Value>>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMcpServerSetupResult {
    pub server: GraphqlMcpServer,
    pub setup_status: String,
    pub discovery_status: Option<String>,
    pub discovered_tool_count: usize,
    pub setup_error: Option<String>,
}
```

- [ ] **Step 4: Add GraphQL conversion helpers**

Parse `transport_kind` using the existing `parse_mcp_transport_kind` pattern. Convert `Json<Value>` maps into `BTreeMap<String, String>` and reject arrays/non-string values with `graphql_error("invalid MCP env/header map: expected object with string values")`.

For V1 GraphQL resolver tests, allow a fake metadata transport in `GraphqlState` under `#[cfg(test)]`. Production may initially return a clear unavailable setup result until concrete stdio/http_sse transports are wired, but the public mutation and UI behavior must be in place.

- [ ] **Step 5: Expose mutations**

In `crates/noema-core/src/graphql/schema.rs`, import the new types and add:

```rust
/// Add and verify an MCP server.
async fn create_mcp_server(
    &self,
    ctx: &Context<'_>,
    input: GraphqlCreateMcpServerInput,
) -> Result<GraphqlMcpServerSetupResult> {
    let state = ctx.data_unchecked::<GraphqlState>();
    mcp::create_mcp_server(state, input).await
}

/// Continue MCP server setup after adding authentication material.
async fn continue_mcp_server_setup(
    &self,
    ctx: &Context<'_>,
    input: GraphqlContinueMcpServerSetupInput,
) -> Result<GraphqlMcpServerSetupResult> {
    let state = ctx.data_unchecked::<GraphqlState>();
    mcp::continue_mcp_server_setup(state, input).await
}
```

- [ ] **Step 6: Run GraphQL tests**

Run:

```bash
cargo test -p noema-core create_mcp_server_mutation_returns_ready_for_calibration_without_secrets create_mcp_server_mutation_rejects_secret_shaped_safe_keys continue_mcp_server_setup_mutation_returns_needs_auth_or_ready_status --no-fail-fast
```

Expected: pass.

- [ ] **Step 7: Commit Task 4**

```bash
git add crates/noema-core/src/graphql/mcp.rs crates/noema-core/src/graphql/schema.rs
git commit -m "feat: expose mcp server setup mutations"
```

## Task 5: Frontend Operations And Form Parser

**Files:**
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Create: `crates/noema-core/web/src/components/settings/mcpSetupForm.ts`
- Create: `crates/noema-core/web/src/components/settings/mcpSetupForm.test.ts`
- Generated: `crates/noema-core/web/src/generated/schema.graphql`
- Generated: `crates/noema-core/web/src/generated/graphql.ts`

- [ ] **Step 1: Add failing parser tests**

Create `crates/noema-core/web/src/components/settings/mcpSetupForm.test.ts`:

```ts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { parseKeyValueLines } from "./mcpSetupForm";

describe("parseKeyValueLines", () => {
  test("parses key value lines", () => {
    assert.deepEqual(parseKeyValueLines("A=1\nB = two").value, { A: "1", B: "two" });
  });

  test("rejects duplicate keys", () => {
    const result = parseKeyValueLines("A=1\nA=2");
    assert.equal(result.error, "Duplicate key: A");
  });

  test("rejects malformed lines", () => {
    const result = parseKeyValueLines("NO_EQUALS");
    assert.equal(result.error, "Line 1 must use KEY=value.");
  });
});
```

- [ ] **Step 2: Run parser tests to verify they fail**

Run: `bun test src/components/settings/mcpSetupForm.test.ts`

Expected: fail because the parser file does not exist.

- [ ] **Step 3: Implement parser**

Create `crates/noema-core/web/src/components/settings/mcpSetupForm.ts`:

```ts
export type KeyValueParseResult =
  | { value: Record<string, string>; error: null }
  | { value: null; error: string };

export function parseKeyValueLines(input: string): KeyValueParseResult {
  const value: Record<string, string> = {};
  const lines = input.split(/\r?\n/);
  for (const [index, rawLine] of lines.entries()) {
    const line = rawLine.trim();
    if (!line) continue;
    const equalsIndex = line.indexOf("=");
    if (equalsIndex <= 0) {
      return { value: null, error: `Line ${index + 1} must use KEY=value.` };
    }
    const key = line.slice(0, equalsIndex).trim();
    const parsedValue = line.slice(equalsIndex + 1).trim();
    if (!key) {
      return { value: null, error: `Line ${index + 1} must include a key.` };
    }
    if (Object.prototype.hasOwnProperty.call(value, key)) {
      return { value: null, error: `Duplicate key: ${key}` };
    }
    value[key] = parsedValue;
  }
  return { value, error: null };
}
```

- [ ] **Step 4: Add GraphQL operations**

Add to `crates/noema-core/web/src/graphql/operations.ts`:

```ts
export const CreateMcpServerDocument = gql`
  mutation CreateMcpServer($input: GraphqlCreateMcpServerInput!) {
    createMcpServer(input: $input) {
      setupStatus
      discoveryStatus
      discoveredToolCount
      setupError
      server {
        mcpServerId
        displayName
        transportKind
        enabled
        healthStatus
        authStatus
        toolCount
      }
    }
  }
`;

export const ContinueMcpServerSetupDocument = gql`
  mutation ContinueMcpServerSetup($input: GraphqlContinueMcpServerSetupInput!) {
    continueMcpServerSetup(input: $input) {
      setupStatus
      discoveryStatus
      discoveredToolCount
      setupError
      server {
        mcpServerId
        displayName
        transportKind
        enabled
        healthStatus
        authStatus
        toolCount
      }
    }
  }
`;
```

- [ ] **Step 5: Generate GraphQL types**

Run: `bun run gen:types`

Expected: generated schema/types include the two new mutations and setup result type.

- [ ] **Step 6: Run parser tests**

Run: `bun test src/components/settings/mcpSetupForm.test.ts`

Expected: pass.

- [ ] **Step 7: Commit Task 5**

```bash
git add crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/components/settings/mcpSetupForm.ts crates/noema-core/web/src/components/settings/mcpSetupForm.test.ts crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
git commit -m "feat: add mcp setup graphql operations"
```

## Task 6: Guided MCP Settings UI

**Files:**
- Create: `crates/noema-core/web/src/components/settings/McpServerSetupFlow.tsx`
- Modify: `crates/noema-core/web/src/components/settings/McpSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/McpSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/pages/SettingsPage.test.tsx`
- Generated: `crates/noema-core/src/daemon/web/assets/app.js`

- [ ] **Step 1: Add failing UI tests**

In `crates/noema-core/web/src/pages/SettingsPage.test.tsx`, add tests that render `McpSettingsPaneContent` with new props and assert:

```ts
assert.match(markup, /Add MCP server/);
assert.match(markup, /Noema verifies the server/);
assert.match(markup, /Configure tools/);
assert.match(markup, /Authentication required/);
```

Use static props rather than a live Apollo mutation for component-level rendering.

- [ ] **Step 2: Run UI tests to verify they fail**

Run: `bun test src/pages/SettingsPage.test.tsx`

Expected: fail because the setup UI props/components do not exist.

- [ ] **Step 3: Implement `McpServerSetupFlow`**

Create `crates/noema-core/web/src/components/settings/McpServerSetupFlow.tsx` with:

- a compact form for display name and transport
- conditional stdio/http_sse fields
- safe and secret `KEY=value` textareas
- progress/status panel for `Verifying server`, `Authentication required`, `Fetching tools`, `Configure tools`
- a calibration checklist that lists discovered count and says tools must be reviewed before they are enabled

Use existing `Button`, `Textarea`, and restrained Settings styles. Do not render raw safe config or secret values after submission.

- [ ] **Step 4: Wire content component props**

Modify `McpSettingsPaneContent` props to include:

```ts
setupResult: McpServerSetupResult | null;
setupSubmitting: boolean;
setupError: string | null;
onCreateServer: (input: McpSetupFormSubmission) => void;
onContinueSetup: (input: McpSetupContinueSubmission) => void;
```

Render the setup flow above the server list. Empty state should include the Add action instead of only "No MCP servers are configured."

- [ ] **Step 5: Wire Apollo mutations**

In `McpSettingsPane.tsx`, use `useMutation` for `CreateMcpServerDocument` and `ContinueMcpServerSetupDocument`. On mutation success, set local setup result and call `result.refetch()`. On GraphQL error, pass a safe error string to the setup flow.

- [ ] **Step 6: Build web assets**

Run: `bun run build`

Expected: generated daemon asset `crates/noema-core/src/daemon/web/assets/app.js` updates.

- [ ] **Step 7: Run UI tests and lint**

Run:

```bash
bun test src/components/settings/mcpSetupForm.test.ts src/pages/SettingsPage.test.tsx
bun run lint
```

Expected: pass.

- [ ] **Step 8: Commit Task 6**

```bash
git add crates/noema-core/web/src/components/settings/McpServerSetupFlow.tsx crates/noema-core/web/src/components/settings/McpSettingsPane.tsx crates/noema-core/web/src/components/settings/McpSettingsPaneContent.tsx crates/noema-core/web/src/pages/SettingsPage.test.tsx crates/noema-core/src/daemon/web/assets/app.js
git commit -m "feat: add guided mcp setup ui"
```

## Task 7: Docs And Final Validation

**Files:**
- Modify: `docs/context/current.md`
- Modify: `docs/frontend/governance-inspection.md`

- [ ] **Step 1: Update durable context**

Update `docs/context/current.md` so the MCP settled decision says Settings can now add MCP servers through a guided setup flow that verifies metadata, handles auth-required status, fetches tool schemas, and prompts calibration.

- [ ] **Step 2: Update governance IA**

Update `docs/frontend/governance-inspection.md` under the MCP Settings slice to mention the five-step flow:

```text
Input details -> verify server -> authenticate if needed -> fetch tools/schema -> configure tools.
```

- [ ] **Step 3: Run final validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
bun run gen:types
bun run lint
bun run build
git status --short --branch
git diff --check
```

Expected:

- Rust and web validation pass.
- `git diff --check` may still report the pre-existing unrelated `docs/memory.md` blank-line issue; if so, run a scoped check for changed files and report the unrelated failure.

- [ ] **Step 4: Stage and inspect docs**

Run:

```bash
git add docs/context/current.md docs/frontend/governance-inspection.md
git diff --cached --stat
git diff --cached --name-status
```

Expected: only the two docs files are staged.

- [ ] **Step 5: Commit Task 7**

```bash
git commit -m "docs: sync mcp setup flow"
```

## Self-Review

- Spec coverage: The plan covers real stdio/http_sse config, disk-backed MCP secrets, non-secret safe config, metadata-only initialize plus `tools/list`, auth-required retry, immediate calibration prompt, no tool invocation/probe calls, GraphQL mutations, frontend Settings flow, tests, docs, and validation.
- Red-flag scan: No deferred implementation notes or unbounded "add tests" steps remain. Where implementation details can vary, the plan provides exact function/type names and expected behavior.
- Type consistency: Backend setup status uses `McpSetupStatus` and GraphQL exposes `setupStatus`; old `discoveryStatus` remains optional metadata only. UI operations and tests refer to `createMcpServer` and `continueMcpServerSetup` consistently.
