# Third-Party MCP Control Plane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a third-party MCP control plane where MCP servers are configured in Settings, tools are calibrated before agent use, trusted identities govern mixed ownership, and every MCP call is mediated by Noema before model-visible release.

**Architecture:** Add focused store modules for MCP servers, tools, calibrations, trusted identities, approvals, and invocation audit. Expose read models and command mutations through GraphQL, build Settings tabs around those read models, then route runtime tool proposals through a Capability Gateway that validates, examines, quarantines, approves, invokes, and releases results. Keep the first implementation deterministic and fail-closed; MCP tool calls are never run during setup.

**Tech Stack:** Rust, async-graphql, embedded SurrealDB v3, serde/serde_json, current Noema daemon runtime, React, Apollo Client, Bun, shadcn/base-rhea UI wrappers, lucide-react.

---

## Scope Check

The approved spec spans several subsystems. This plan keeps them in one file for continuity, but execution should treat each task as a separate unit with its own tests and commit. If parallelized, assign ownership by module area:

- Store and domain model tasks.
- GraphQL read/mutation tasks.
- Web Settings tasks.
- MCP client/runtime tasks.
- Capability Gateway and approval tasks.

The first shippable milestone is read-only MCP governance state in Settings. Agent-visible runtime execution should remain disabled until calibration, gateway, quarantine, and approval tasks are complete.

## File Structure

Create or modify these files:

- Modify `crates/noema-core/src/store/schema.rs`: add SurrealDB tables for MCP servers, discovered tools, calibrations, trusted identity selectors, approvals, tool invocations, and quarantined results.
- Modify `crates/noema-core/src/store.rs`: export new store modules and record types.
- Create `crates/noema-core/src/store/mcp.rs`: MCP server/tool/calibration/trusted identity/approval persistence and read models.
- Create `crates/noema-core/src/mcp.rs`: domain enums, normalization helpers, owner extractor structs, and metadata-only MCP tool models.
- Create `crates/noema-core/src/capability.rs`: capability gateway decision types, examination outcomes, and fail-closed policy evaluation.
- Modify `crates/noema-core/src/lib.rs`: export MCP and capability domain types.
- Create `crates/noema-core/src/graphql/mcp.rs`: GraphQL MCP settings read models and command inputs.
- Modify `crates/noema-core/src/graphql.rs`: register the MCP GraphQL module.
- Modify `crates/noema-core/src/graphql/schema.rs`: expose MCP queries and mutations.
- Modify `crates/noema-core/web/src/routes.ts`: add Settings sections for `mcps`, `trusted-identities`, `approvals`, and `audit`.
- Modify `crates/noema-core/web/src/graphql/operations.ts`: add MCP Settings operations.
- Create `crates/noema-core/web/src/components/settings/McpSettingsPane.tsx`.
- Create `crates/noema-core/web/src/components/settings/McpSettingsPaneContent.tsx`.
- Create `crates/noema-core/web/src/components/settings/mcpMetadata.ts`.
- Create `crates/noema-core/web/src/components/settings/TrustedIdentitiesSettingsPane.tsx`.
- Create `crates/noema-core/web/src/components/settings/TrustedIdentitiesSettingsPaneContent.tsx`.
- Create `crates/noema-core/web/src/components/settings/ApprovalsSettingsPane.tsx`.
- Create `crates/noema-core/web/src/components/settings/ApprovalsSettingsPaneContent.tsx`.
- Create `crates/noema-core/web/src/components/settings/AuditSettingsPane.tsx`.
- Modify `crates/noema-core/web/src/components/settings/SettingsSidebar.tsx`: add settings navigation items.
- Modify `crates/noema-core/web/src/pages/SettingsPage.tsx`: render the new panes.
- Modify `crates/noema-core/web/src/pages/SettingsPage.test.tsx` and `crates/noema-core/web/src/routes.test.ts`: cover routes and panes.
- Later runtime tasks create `crates/noema-core/src/mcp/client.rs`, `crates/noema-core/src/capability/gateway.rs`, `crates/noema-core/src/capability/ownership.rs`, `crates/noema-core/src/capability/examination.rs`, and wire the daemon local tool path through the gateway.

## Task 1: Store Schema And Domain Types

**Files:**
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store.rs`
- Create: `crates/noema-core/src/mcp.rs`
- Modify: `crates/noema-core/src/lib.rs`

- [ ] **Step 1: Add failing schema/domain tests**

Add tests under `crates/noema-core/src/store/tests.rs` or a new `mcp` test module if that file is already split during implementation:

```rust
#[tokio::test]
async fn mcp_control_plane_tables_bootstrap() {
    let store = test_store().await;
    assert_eq!(store.schema_version().await.expect("schema version"), 1);
    let servers = store.list_mcp_servers().await.expect("servers");
    assert!(servers.is_empty());
    let identities = store
        .list_trusted_identity_selectors("human:local")
        .await
        .expect("trusted identities");
    assert!(identities.is_empty());
}

#[test]
fn trusted_identity_selectors_normalize_email_phone_and_domain() {
    use crate::{TrustedIdentitySelectorKind, normalize_trusted_identity_value};

    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Email, " Kevin@Example.COM "),
        Some("kevin@example.com".to_string())
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Domain, " Example.COM "),
        Some("example.com".to_string())
    );
    assert_eq!(
        normalize_trusted_identity_value(TrustedIdentitySelectorKind::Phone, " +1 (415) 555-0100 "),
        Some("+14155550100".to_string())
    );
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p noema-core mcp_control_plane_tables_bootstrap trusted_identity_selectors_normalize_email_phone_and_domain --no-fail-fast
```

Expected: compile failure because MCP store methods and identity helpers are not defined.

- [ ] **Step 3: Add MCP domain types**

Create `crates/noema-core/src/mcp.rs`:

```rust
//! Third-party MCP control-plane domain types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTrustClassification {
    None,
    Trusted,
    Untrusted,
    Mixed,
}

impl McpTrustClassification {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Trusted => "trusted",
            Self::Untrusted => "untrusted",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustedIdentitySelectorKind {
    Email,
    Phone,
    Domain,
}

impl TrustedIdentitySelectorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Phone => "phone",
            Self::Domain => "domain",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportKind {
    Stdio,
    HttpSse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpCalibrationStatus {
    NeedsReview,
    BlockedUnresolvedOwnership,
    Ready,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerExtractor {
    pub source: OwnerExtractorSource,
    pub selector_kind: TrustedIdentitySelectorKind,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerExtractorSource {
    Arguments,
    StructuredContent,
    Metadata,
    ResourceUri,
    BuiltInAdapter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolSchema {
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub annotations: Value,
}

pub fn normalize_trusted_identity_value(
    kind: TrustedIdentitySelectorKind,
    value: &str,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    match kind {
        TrustedIdentitySelectorKind::Email | TrustedIdentitySelectorKind::Domain => {
            Some(trimmed.to_ascii_lowercase())
        }
        TrustedIdentitySelectorKind::Phone => {
            let mut normalized = String::new();
            for (index, ch) in trimmed.chars().enumerate() {
                if ch == '+' && index == 0 {
                    normalized.push(ch);
                } else if ch.is_ascii_digit() {
                    normalized.push(ch);
                }
            }
            if normalized.chars().any(|ch| ch.is_ascii_digit()) {
                Some(normalized)
            } else {
                None
            }
        }
    }
}
```

- [ ] **Step 4: Export the MCP module**

Modify `crates/noema-core/src/lib.rs`:

```rust
/// Third-party MCP control-plane types.
pub mod mcp;
```

Add these exports near the other `pub use` blocks:

```rust
pub use mcp::{
    McpCalibrationStatus, McpToolSchema, McpTransportKind, McpTrustClassification,
    OwnerExtractor, OwnerExtractorSource, TrustedIdentitySelectorKind,
    normalize_trusted_identity_value,
};
```

- [ ] **Step 5: Add SurrealDB tables**

In `crates/noema-core/src/store/schema.rs`, append table definitions after `provider_accounts` or before conversation tables:

```sql
DEFINE TABLE IF NOT EXISTS mcp_servers SCHEMAFULL;
DEFINE FIELD OVERWRITE mcp_server_id ON TABLE mcp_servers TYPE string;
DEFINE FIELD OVERWRITE display_name ON TABLE mcp_servers TYPE string;
DEFINE FIELD OVERWRITE transport_kind ON TABLE mcp_servers TYPE string ASSERT $value INSIDE ['stdio', 'http_sse'];
DEFINE FIELD OVERWRITE safe_config ON TABLE mcp_servers TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE auth_status ON TABLE mcp_servers TYPE string ASSERT $value INSIDE ['none', 'needs_auth', 'authenticated', 'unavailable'];
DEFINE FIELD OVERWRITE health_status ON TABLE mcp_servers TYPE string ASSERT $value INSIDE ['unknown', 'healthy', 'unavailable'];
DEFINE FIELD OVERWRITE enabled ON TABLE mcp_servers TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE metadata_fingerprint ON TABLE mcp_servers TYPE option<string>;
DEFINE FIELD OVERWRITE last_discovered_at ON TABLE mcp_servers TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE mcp_servers TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE mcp_servers TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS mcp_servers_server_id ON TABLE mcp_servers COLUMNS mcp_server_id UNIQUE;

DEFINE TABLE IF NOT EXISTS mcp_tools SCHEMAFULL;
DEFINE FIELD OVERWRITE mcp_tool_id ON TABLE mcp_tools TYPE string;
DEFINE FIELD OVERWRITE mcp_server_id ON TABLE mcp_tools TYPE string;
DEFINE FIELD OVERWRITE name ON TABLE mcp_tools TYPE string;
DEFINE FIELD OVERWRITE description ON TABLE mcp_tools TYPE option<string>;
DEFINE FIELD OVERWRITE input_schema ON TABLE mcp_tools TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE output_schema ON TABLE mcp_tools TYPE option<object>;
DEFINE FIELD OVERWRITE annotations ON TABLE mcp_tools TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE metadata_fingerprint ON TABLE mcp_tools TYPE string;
DEFINE FIELD OVERWRITE discovered_at ON TABLE mcp_tools TYPE string;
DEFINE FIELD OVERWRITE created_at ON TABLE mcp_tools TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE mcp_tools TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS mcp_tools_tool_id ON TABLE mcp_tools COLUMNS mcp_tool_id UNIQUE;
DEFINE INDEX IF NOT EXISTS mcp_tools_server_name ON TABLE mcp_tools COLUMNS mcp_server_id, name UNIQUE;

DEFINE TABLE IF NOT EXISTS tool_calibrations SCHEMAFULL;
DEFINE FIELD OVERWRITE calibration_id ON TABLE tool_calibrations TYPE string;
DEFINE FIELD OVERWRITE mcp_tool_id ON TABLE tool_calibrations TYPE string;
DEFINE FIELD OVERWRITE read_classification ON TABLE tool_calibrations TYPE string ASSERT $value INSIDE ['none', 'trusted', 'untrusted', 'mixed'];
DEFINE FIELD OVERWRITE write_classification ON TABLE tool_calibrations TYPE string ASSERT $value INSIDE ['none', 'trusted', 'untrusted', 'mixed'];
DEFINE FIELD OVERWRITE export_classification ON TABLE tool_calibrations TYPE string ASSERT $value INSIDE ['none', 'trusted', 'untrusted', 'mixed'];
DEFINE FIELD OVERWRITE owner_extractors ON TABLE tool_calibrations TYPE array<object> DEFAULT [];
DEFINE FIELD OVERWRITE enabled_agent_ids ON TABLE tool_calibrations TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE enabled_scope_ids ON TABLE tool_calibrations TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE status ON TABLE tool_calibrations TYPE string ASSERT $value INSIDE ['needs_review', 'blocked_unresolved_ownership', 'ready', 'disabled'];
DEFINE FIELD OVERWRITE reviewed_by ON TABLE tool_calibrations TYPE option<string>;
DEFINE FIELD OVERWRITE reviewed_metadata_fingerprint ON TABLE tool_calibrations TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE tool_calibrations TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE tool_calibrations TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS tool_calibrations_calibration_id ON TABLE tool_calibrations COLUMNS calibration_id UNIQUE;
DEFINE INDEX IF NOT EXISTS tool_calibrations_tool_id ON TABLE tool_calibrations COLUMNS mcp_tool_id UNIQUE;

DEFINE TABLE IF NOT EXISTS trusted_identity_selectors SCHEMAFULL;
DEFINE FIELD OVERWRITE selector_id ON TABLE trusted_identity_selectors TYPE string;
DEFINE FIELD OVERWRITE owner_scope_id ON TABLE trusted_identity_selectors TYPE string;
DEFINE FIELD OVERWRITE selector_kind ON TABLE trusted_identity_selectors TYPE string ASSERT $value INSIDE ['email', 'phone', 'domain'];
DEFINE FIELD OVERWRITE normalized_value ON TABLE trusted_identity_selectors TYPE string;
DEFINE FIELD OVERWRITE effect ON TABLE trusted_identity_selectors TYPE string ASSERT $value INSIDE ['trust', 'restrict'];
DEFINE FIELD OVERWRITE issuer_actor_id ON TABLE trusted_identity_selectors TYPE string;
DEFINE FIELD OVERWRITE revoked_at ON TABLE trusted_identity_selectors TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE trusted_identity_selectors TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE trusted_identity_selectors TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS trusted_identity_selectors_selector_id ON TABLE trusted_identity_selectors COLUMNS selector_id UNIQUE;
DEFINE INDEX IF NOT EXISTS trusted_identity_selectors_owner_value ON TABLE trusted_identity_selectors COLUMNS owner_scope_id, selector_kind, normalized_value UNIQUE;
```

- [ ] **Step 6: Run focused validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core mcp_control_plane_tables_bootstrap trusted_identity_selectors_normalize_email_phone_and_domain --no-fail-fast
```

Expected: tests compile; store method tests still fail until Task 2 adds the store module.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/store/schema.rs crates/noema-core/src/store.rs crates/noema-core/src/mcp.rs crates/noema-core/src/lib.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: add mcp control plane schema"
```

## Task 2: Store Read Models And Seed Commands

**Files:**
- Create: `crates/noema-core/src/store/mcp.rs`
- Modify: `crates/noema-core/src/store.rs`
- Test: `crates/noema-core/src/store/tests.rs`

- [ ] **Step 1: Add failing store tests**

Add tests:

```rust
#[tokio::test]
async fn creates_and_lists_mcp_server_with_discovered_tool() {
    let store = test_store().await;
    let server = store
        .create_mcp_server(NewMcpServer {
            mcp_server_id: "mcp_server:google".to_string(),
            display_name: "Google".to_string(),
            transport_kind: McpTransportKind::HttpSse,
            safe_config: serde_json::json!({"url": "https://mcp.example.test"}),
        })
        .await
        .expect("create server");
    assert_eq!(server.display_name, "Google");

    let tool = store
        .upsert_discovered_mcp_tool(NewMcpTool {
            mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
            mcp_server_id: server.mcp_server_id.clone(),
            name: "read_doc".to_string(),
            description: Some("Read a Google Doc".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
            annotations: serde_json::json!({"readOnlyHint": true}),
            metadata_fingerprint: "fingerprint_1".to_string(),
        })
        .await
        .expect("upsert tool");
    assert_eq!(tool.name, "read_doc");

    let servers = store.list_mcp_servers().await.expect("servers");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].tool_count, 1);
}

#[tokio::test]
async fn stores_trusted_identity_selector_normalized() {
    let store = test_store().await;
    let selector = store
        .create_trusted_identity_selector(NewTrustedIdentitySelector {
            selector_id: "trusted_identity:human_local:email".to_string(),
            owner_scope_id: "human:local".to_string(),
            selector_kind: TrustedIdentitySelectorKind::Email,
            raw_value: " Kevin@Example.COM ".to_string(),
            effect: "trust".to_string(),
            issuer_actor_id: "human:local".to_string(),
        })
        .await
        .expect("create selector");
    assert_eq!(selector.normalized_value, "kevin@example.com");
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p noema-core creates_and_lists_mcp_server_with_discovered_tool stores_trusted_identity_selector_normalized --no-fail-fast
```

Expected: compile failure for missing store types and methods.

- [ ] **Step 3: Implement store records and commands**

Create `crates/noema-core/src/store/mcp.rs` with these public records and methods:

```rust
use serde::Deserialize;
use serde_json::Value;
use surrealdb::types::SurrealValue;

use crate::{
    McpCalibrationStatus, McpTransportKind, McpTrustClassification,
    TrustedIdentitySelectorKind, normalize_trusted_identity_value,
};

use super::{NoemaStore, StoreError, ids::now_string};

#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpServer {
    pub mcp_server_id: String,
    pub display_name: String,
    pub transport_kind: McpTransportKind,
    pub safe_config: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpServerRecord {
    pub mcp_server_id: String,
    pub display_name: String,
    pub transport_kind: McpTransportKind,
    pub safe_config: Value,
    pub enabled: bool,
    pub health_status: String,
    pub auth_status: String,
    pub tool_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpTool {
    pub mcp_tool_id: String,
    pub mcp_server_id: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub annotations: Value,
    pub metadata_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpToolRecord {
    pub mcp_tool_id: String,
    pub mcp_server_id: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub annotations: Value,
    pub metadata_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewTrustedIdentitySelector {
    pub selector_id: String,
    pub owner_scope_id: String,
    pub selector_kind: TrustedIdentitySelectorKind,
    pub raw_value: String,
    pub effect: String,
    pub issuer_actor_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedIdentitySelectorRecord {
    pub selector_id: String,
    pub owner_scope_id: String,
    pub selector_kind: TrustedIdentitySelectorKind,
    pub normalized_value: String,
    pub effect: String,
    pub issuer_actor_id: String,
    pub revoked_at: Option<String>,
}

impl NoemaStore {
    pub async fn create_mcp_server(
        &self,
        server: NewMcpServer,
    ) -> Result<McpServerRecord, StoreError> {
        self.db
            .query(
                r#"
                CREATE type::record('mcp_servers', $record_id) SET
                  mcp_server_id = $mcp_server_id,
                  display_name = $display_name,
                  transport_kind = $transport_kind,
                  safe_config = $safe_config,
                  auth_status = 'none',
                  health_status = 'unknown',
                  enabled = false,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&server.mcp_server_id)))
            .bind(("mcp_server_id", server.mcp_server_id.clone()))
            .bind(("display_name", server.display_name))
            .bind(("transport_kind", transport_kind_str(server.transport_kind)))
            .bind(("safe_config", server.safe_config))
            .await?
            .check()?;
        self.get_mcp_server(&server.mcp_server_id)
            .await?
            .ok_or_else(|| StoreError::Schema("created MCP server was not returned".to_string()))
    }

    pub async fn get_mcp_server(
        &self,
        mcp_server_id: &str,
    ) -> Result<Option<McpServerRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT mcp_server_id, display_name, transport_kind, safe_config,
                  auth_status, health_status, enabled,
                  count((SELECT VALUE mcp_tool_id FROM mcp_tools WHERE mcp_server_id = $mcp_server_id)) AS tool_count
                FROM mcp_servers
                WHERE mcp_server_id = $mcp_server_id
                LIMIT 1;
                "#,
            )
            .bind(("mcp_server_id", mcp_server_id.to_string()))
            .await?;
        let rows: Vec<McpServerRow> = response.take(0)?;
        rows.into_iter().next().map(server_from_row).transpose()
    }

    pub async fn list_mcp_servers(&self) -> Result<Vec<McpServerRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT mcp_server_id, display_name, transport_kind, safe_config,
                  auth_status, health_status, enabled,
                  count((SELECT VALUE mcp_tool_id FROM mcp_tools WHERE mcp_server_id = $parent.mcp_server_id)) AS tool_count
                FROM mcp_servers;
                "#,
            )
            .await?;
        let rows: Vec<McpServerRow> = response.take(0)?;
        rows.into_iter().map(server_from_row).collect()
    }

    pub async fn upsert_discovered_mcp_tool(
        &self,
        tool: NewMcpTool,
    ) -> Result<McpToolRecord, StoreError> {
        let discovered_at = now_string();
        self.db
            .query(
                r#"
                UPSERT type::record('mcp_tools', $record_id) SET
                  mcp_tool_id = $mcp_tool_id,
                  mcp_server_id = $mcp_server_id,
                  name = $name,
                  description = $description,
                  input_schema = $input_schema,
                  output_schema = $output_schema,
                  annotations = $annotations,
                  metadata_fingerprint = $metadata_fingerprint,
                  discovered_at = $discovered_at,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&tool.mcp_tool_id)))
            .bind(("mcp_tool_id", tool.mcp_tool_id.clone()))
            .bind(("mcp_server_id", tool.mcp_server_id))
            .bind(("name", tool.name))
            .bind(("description", tool.description))
            .bind(("input_schema", tool.input_schema))
            .bind(("output_schema", tool.output_schema))
            .bind(("annotations", tool.annotations))
            .bind(("metadata_fingerprint", tool.metadata_fingerprint))
            .bind(("discovered_at", discovered_at))
            .await?
            .check()?;
        self.get_mcp_tool(&tool.mcp_tool_id)
            .await?
            .ok_or_else(|| StoreError::Schema("upserted MCP tool was not returned".to_string()))
    }

    pub async fn get_mcp_tool(
        &self,
        mcp_tool_id: &str,
    ) -> Result<Option<McpToolRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT mcp_tool_id, mcp_server_id, name, description, input_schema,
                  output_schema, annotations, metadata_fingerprint
                FROM mcp_tools
                WHERE mcp_tool_id = $mcp_tool_id
                LIMIT 1;
                "#,
            )
            .bind(("mcp_tool_id", mcp_tool_id.to_string()))
            .await?;
        let rows: Vec<McpToolRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(tool_from_row))
    }

    pub async fn create_trusted_identity_selector(
        &self,
        selector: NewTrustedIdentitySelector,
    ) -> Result<TrustedIdentitySelectorRecord, StoreError> {
        let normalized_value =
            normalize_trusted_identity_value(selector.selector_kind, &selector.raw_value)
                .ok_or_else(|| StoreError::Schema("empty trusted identity selector".to_string()))?;
        self.db
            .query(
                r#"
                CREATE type::record('trusted_identity_selectors', $record_id) SET
                  selector_id = $selector_id,
                  owner_scope_id = $owner_scope_id,
                  selector_kind = $selector_kind,
                  normalized_value = $normalized_value,
                  effect = $effect,
                  issuer_actor_id = $issuer_actor_id,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&selector.selector_id)))
            .bind(("selector_id", selector.selector_id.clone()))
            .bind(("owner_scope_id", selector.owner_scope_id))
            .bind(("selector_kind", selector_kind_str(selector.selector_kind)))
            .bind(("normalized_value", normalized_value))
            .bind(("effect", selector.effect))
            .bind(("issuer_actor_id", selector.issuer_actor_id))
            .await?
            .check()?;
        self.get_trusted_identity_selector(&selector.selector_id)
            .await?
            .ok_or_else(|| StoreError::Schema("created selector was not returned".to_string()))
    }
}
```

Also add row structs, parsing helpers, `get_trusted_identity_selector`, and `list_trusted_identity_selectors`. Keep parsing closed over exact enum strings.

- [ ] **Step 4: Export store records**

Modify `crates/noema-core/src/store.rs`:

```rust
mod mcp;
pub use mcp::{
    McpServerRecord, McpToolRecord, NewMcpServer, NewMcpTool,
    NewTrustedIdentitySelector, TrustedIdentitySelectorRecord,
};
```

Modify `crates/noema-core/src/lib.rs` store exports:

```rust
McpServerRecord, McpToolRecord, NewMcpServer, NewMcpTool,
NewTrustedIdentitySelector, TrustedIdentitySelectorRecord,
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core creates_and_lists_mcp_server_with_discovered_tool stores_trusted_identity_selector_normalized mcp_control_plane_tables_bootstrap --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/store.rs crates/noema-core/src/store/mcp.rs crates/noema-core/src/lib.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: add mcp settings store"
```

## Task 3: GraphQL MCP Settings Read Models

**Files:**
- Create: `crates/noema-core/src/graphql/mcp.rs`
- Modify: `crates/noema-core/src/graphql.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Test: existing GraphQL schema tests or add focused tests near `crates/noema-core/src/graphql/schema.rs`

- [ ] **Step 1: Add failing GraphQL tests**

Add a GraphQL test that creates one server/tool/selector in a test store, builds `GraphqlState::for_tests_with_store(store)`, and executes:

```graphql
query McpSettings {
  mcpServers {
    mcpServerId
    displayName
    transportKind
    enabled
    healthStatus
    toolCount
  }
  trustedIdentitySelectors(ownerScopeId: "human:local") {
    selectorId
    ownerScopeId
    selectorKind
    normalizedValue
    effect
  }
}
```

Assert that the server and selector appear.

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test -p noema-core mcp_settings_graphql_lists_servers_and_identities --no-fail-fast
```

Expected: GraphQL query fails because fields are missing.

- [ ] **Step 3: Implement GraphQL read module**

Create `crates/noema-core/src/graphql/mcp.rs`:

```rust
use async_graphql::{Result, SimpleObject};

use crate::{McpServerRecord, TrustedIdentitySelectorRecord};

use super::{errors::graphql_error, schema::GraphqlState};

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlMcpServer {
    pub mcp_server_id: String,
    pub display_name: String,
    pub transport_kind: String,
    pub enabled: bool,
    pub health_status: String,
    pub auth_status: String,
    pub tool_count: i32,
}

impl From<McpServerRecord> for GraphqlMcpServer {
    fn from(server: McpServerRecord) -> Self {
        Self {
            mcp_server_id: server.mcp_server_id,
            display_name: server.display_name,
            transport_kind: server.transport_kind.as_str().to_string(),
            enabled: server.enabled,
            health_status: server.health_status,
            auth_status: server.auth_status,
            tool_count: i32::try_from(server.tool_count).unwrap_or(i32::MAX),
        }
    }
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlTrustedIdentitySelector {
    pub selector_id: String,
    pub owner_scope_id: String,
    pub selector_kind: String,
    pub normalized_value: String,
    pub effect: String,
    pub issuer_actor_id: String,
}

impl From<TrustedIdentitySelectorRecord> for GraphqlTrustedIdentitySelector {
    fn from(selector: TrustedIdentitySelectorRecord) -> Self {
        Self {
            selector_id: selector.selector_id,
            owner_scope_id: selector.owner_scope_id,
            selector_kind: selector.selector_kind.as_str().to_string(),
            normalized_value: selector.normalized_value,
            effect: selector.effect,
            issuer_actor_id: selector.issuer_actor_id,
        }
    }
}

pub(super) async fn mcp_servers(state: &GraphqlState) -> Result<Vec<GraphqlMcpServer>> {
    let store = state.store()?;
    let servers = store.list_mcp_servers().await.map_err(graphql_error)?;
    Ok(servers.into_iter().map(Into::into).collect())
}

pub(super) async fn trusted_identity_selectors(
    state: &GraphqlState,
    owner_scope_id: String,
) -> Result<Vec<GraphqlTrustedIdentitySelector>> {
    let store = state.store()?;
    let selectors = store
        .list_trusted_identity_selectors(&owner_scope_id)
        .await
        .map_err(graphql_error)?;
    Ok(selectors.into_iter().map(Into::into).collect())
}
```

- [ ] **Step 4: Register module and schema fields**

Modify `crates/noema-core/src/graphql.rs`:

```rust
mod mcp;
```

Modify `crates/noema-core/src/graphql/schema.rs` imports:

```rust
mcp::{self, GraphqlMcpServer, GraphqlTrustedIdentitySelector},
```

Add query fields:

```rust
/// List configured MCP servers for Settings.
async fn mcp_servers(&self, ctx: &Context<'_>) -> Result<Vec<GraphqlMcpServer>> {
    let state = ctx.data_unchecked::<GraphqlState>();
    mcp::mcp_servers(state).await
}

/// List trusted identity selectors for one owner scope.
async fn trusted_identity_selectors(
    &self,
    ctx: &Context<'_>,
    owner_scope_id: String,
) -> Result<Vec<GraphqlTrustedIdentitySelector>> {
    let state = ctx.data_unchecked::<GraphqlState>();
    mcp::trusted_identity_selectors(state, owner_scope_id).await
}
```

- [ ] **Step 5: Run tests**

```bash
cargo fmt --all --check
cargo test -p noema-core mcp_settings_graphql_lists_servers_and_identities --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/graphql.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/src/graphql/mcp.rs
git commit -m "feat: expose mcp settings graphql"
```

## Task 4: Web Settings Routes And Read-Only MCP Dashboard

**Files:**
- Modify: `crates/noema-core/web/src/routes.ts`
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Create: `crates/noema-core/web/src/components/settings/McpSettingsPane.tsx`
- Create: `crates/noema-core/web/src/components/settings/McpSettingsPaneContent.tsx`
- Create: `crates/noema-core/web/src/components/settings/mcpMetadata.ts`
- Modify: `crates/noema-core/web/src/components/settings/SettingsSidebar.tsx`
- Modify: `crates/noema-core/web/src/pages/SettingsPage.tsx`
- Modify: `crates/noema-core/web/src/pages/SettingsPage.test.tsx`
- Modify: `crates/noema-core/web/src/routes.test.ts`

- [ ] **Step 1: Add failing route and Settings tests**

Extend `routes.test.ts`:

```ts
assert.deepEqual(routeFromPathname("/settings/mcps"), {
  kind: "settings",
  section: "mcps"
});
assert.equal(pathForRoute({ kind: "settings", section: "mcps" }), "/settings/mcps");
```

Extend `SettingsPage.test.tsx` to assert the sidebar includes `MCPs` and route selection emits `{ kind: "settings", section: "mcps" }`.

- [ ] **Step 2: Run failing frontend tests**

Run from `crates/noema-core/web`:

```bash
bun run lint
```

Expected: TypeScript failure because `mcps` is not part of `SettingsSection`.

- [ ] **Step 3: Add route sections**

Modify `crates/noema-core/web/src/routes.ts`:

```ts
export type SettingsSection =
  | "providers"
  | "agents"
  | "mcps"
  | "trusted-identities"
  | "approvals"
  | "audit";
```

Add route parsing for `/settings/mcps`, `/settings/trusted-identities`, `/settings/approvals`, and `/settings/audit`. Update `pathForRoute` with a `switch` so each section maps to its exact path.

- [ ] **Step 4: Add GraphQL operations**

Modify `crates/noema-core/web/src/graphql/operations.ts`:

```ts
export const McpSettingsDocument = gql`
  query McpSettings {
    mcpServers {
      mcpServerId
      displayName
      transportKind
      enabled
      healthStatus
      authStatus
      toolCount
    }
  }
`;
```

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: generated GraphQL types include `McpSettingsQuery`.

- [ ] **Step 5: Add MCP pane content**

Create `McpSettingsPaneContent.tsx`:

```tsx
import { Badge } from "@/components/ui/badge";
import type { McpSettingsQuery } from "@/generated/graphql";

export type McpSettingsServer = McpSettingsQuery["mcpServers"][number];

export function McpSettingsPaneContent({
  servers,
  loading,
  error
}: {
  servers: readonly McpSettingsServer[];
  loading: boolean;
  error: string | null;
}) {
  if (loading) {
    return <p className="m-0 text-sm text-muted-foreground">Loading MCP servers...</p>;
  }
  if (error) {
    return <p className="m-0 text-sm text-muted-foreground">{error}</p>;
  }
  if (servers.length === 0) {
    return (
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">
          No MCP servers are configured yet.
        </p>
      </div>
    );
  }
  return (
    <div className="grid gap-3">
      {servers.map((server) => (
        <article
          key={server.mcpServerId}
          className="grid gap-2 rounded-md border border-[var(--border-subtle)] bg-white p-4"
        >
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h2 className="m-0 font-heading text-xl tracking-normal">{server.displayName}</h2>
            <Badge variant="outline">{server.enabled ? "Enabled" : "Disabled"}</Badge>
          </div>
          <dl className="m-0 grid gap-2 text-sm">
            <div className="flex justify-between gap-4">
              <dt className="text-muted-foreground">Transport</dt>
              <dd className="m-0 font-mono">{server.transportKind}</dd>
            </div>
            <div className="flex justify-between gap-4">
              <dt className="text-muted-foreground">Tools</dt>
              <dd className="m-0">{server.toolCount}</dd>
            </div>
            <div className="flex justify-between gap-4">
              <dt className="text-muted-foreground">Health</dt>
              <dd className="m-0">{server.healthStatus}</dd>
            </div>
          </dl>
        </article>
      ))}
    </div>
  );
}
```

Create `McpSettingsPane.tsx`:

```tsx
import { useQuery } from "@apollo/client";
import { McpSettingsDocument } from "@/generated/graphql";
import { McpSettingsPaneContent } from "./McpSettingsPaneContent";

export function McpSettingsPane() {
  const result = useQuery(McpSettingsDocument);
  const error = result.error ? result.error.message : null;
  return (
    <McpSettingsPaneContent
      servers={result.data ? result.data.mcpServers : []}
      loading={result.loading}
      error={error}
    />
  );
}
```

- [ ] **Step 6: Wire Settings page and sidebar**

Modify `SettingsSidebar.tsx` to add `Cable`, `Fingerprint`, `Inbox`, and `SearchCheck` icons from `lucide-react`. Add buttons for `MCPs`, `Trusted identities`, `Approvals`, and `Audit`.

Modify `SettingsPage.tsx` copy and render switch:

```tsx
function settingsPane(section: SettingsSection) {
  switch (section) {
    case "agents":
      return <AgentsSettingsPane />;
    case "mcps":
      return <McpSettingsPane />;
    case "trusted-identities":
      return <TrustedIdentitiesSettingsPane />;
    case "approvals":
      return <ApprovalsSettingsPane />;
    case "audit":
      return <AuditSettingsPane />;
    case "providers":
      return <ProvidersSettingsPane />;
  }
}
```

For this task, `TrustedIdentitiesSettingsPane`, `ApprovalsSettingsPane`, and `AuditSettingsPane` may render read-only empty states with the exact copy defined in Task 5.

- [ ] **Step 7: Run frontend validation**

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/web/src/routes.ts crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/generated/graphql.ts crates/noema-core/web/src/components/settings crates/noema-core/web/src/pages/SettingsPage.tsx crates/noema-core/web/src/pages/SettingsPage.test.tsx crates/noema-core/web/src/routes.test.ts
git commit -m "feat: add mcp settings dashboard"
```

## Task 5: Trusted Identities, Approvals, And Audit Settings Panes

**Files:**
- Create/modify settings pane files from Task 4
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Modify: `crates/noema-core/src/graphql/mcp.rs`

- [ ] **Step 1: Add GraphQL operation and UI tests**

Add `TrustedIdentitySettingsDocument`:

```ts
export const TrustedIdentitySettingsDocument = gql`
  query TrustedIdentitySettings($ownerScopeId: String!) {
    trustedIdentitySelectors(ownerScopeId: $ownerScopeId) {
      selectorId
      ownerScopeId
      selectorKind
      normalizedValue
      effect
      issuerActorId
    }
  }
`;
```

Add tests that `TrustedIdentitiesSettingsPaneContent` renders email, phone, and domain rows and that approvals/audit panes render empty states without overlapping text.

- [ ] **Step 2: Implement trusted identity pane**

Create `TrustedIdentitiesSettingsPaneContent.tsx`:

```tsx
import type { TrustedIdentitySettingsQuery } from "@/generated/graphql";

export type TrustedIdentityRow =
  TrustedIdentitySettingsQuery["trustedIdentitySelectors"][number];

export function TrustedIdentitiesSettingsPaneContent({
  selectors,
  loading,
  error
}: {
  selectors: readonly TrustedIdentityRow[];
  loading: boolean;
  error: string | null;
}) {
  if (loading) return <p className="m-0 text-sm text-muted-foreground">Loading trusted identities...</p>;
  if (error) return <p className="m-0 text-sm text-muted-foreground">{error}</p>;
  if (selectors.length === 0) {
    return <p className="m-0 text-sm text-muted-foreground">No trusted identities are configured for this scope.</p>;
  }
  return (
    <div className="overflow-hidden rounded-md border border-[var(--border-subtle)] bg-white">
      <dl className="m-0 divide-y divide-[var(--border-subtle)]">
        {selectors.map((selector) => (
          <div key={selector.selectorId} className="grid grid-cols-[140px_1fr] gap-4 px-4 py-3">
            <dt className="text-sm font-medium text-muted-foreground">{selector.selectorKind}</dt>
            <dd className="m-0 break-words font-mono text-sm">{selector.normalizedValue}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
```

Create `TrustedIdentitiesSettingsPane.tsx` using `useQuery(TrustedIdentitySettingsDocument, { variables: { ownerScopeId: "human:local" } })`.

- [ ] **Step 3: Implement approvals and audit placeholder panes**

Create `ApprovalsSettingsPaneContent.tsx`:

```tsx
export function ApprovalsSettingsPaneContent() {
  return (
    <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
      <p className="m-0 text-sm text-muted-foreground">
        No MCP approvals are pending.
      </p>
    </div>
  );
}
```

Create `AuditSettingsPaneContent.tsx`:

```tsx
export function AuditSettingsPaneContent() {
  return (
    <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
      <p className="m-0 text-sm text-muted-foreground">
        MCP audit records will appear here after mediated tool calls run.
      </p>
    </div>
  );
}
```

Wrap each in a pane file that exports the component.

- [ ] **Step 4: Run frontend validation**

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/web/src/components/settings crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/generated/graphql.ts crates/noema-core/web/src/pages/SettingsPage.test.tsx
git commit -m "feat: add mcp governance settings panes"
```

## Task 6: MCP Metadata Client Runtime

**Files:**
- Create: `crates/noema-core/src/mcp/client.rs`
- Modify: `crates/noema-core/src/mcp.rs`
- Test: `crates/noema-core/src/mcp/client.rs`

- [ ] **Step 1: Add failing metadata-only client tests**

Add tests with a fake in-process MCP transport:

```rust
#[tokio::test]
async fn metadata_discovery_lists_tools_without_calling_them() {
    let transport = FakeMcpTransport::new(vec![fake_tool("read_doc")]);
    let mut client = McpClientRuntime::new(Box::new(transport.clone()));
    let tools = client.discover_tools().await.expect("tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "read_doc");
    assert_eq!(transport.call_count(), 0);
}
```

- [ ] **Step 2: Implement transport trait and metadata discovery**

Create `client.rs`:

```rust
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredMcpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub annotations: Value,
}

#[derive(Debug, Error)]
pub enum McpClientError {
    #[error("MCP transport failed: {0}")]
    Transport(String),
    #[error("MCP response was malformed: {0}")]
    Malformed(String),
}

pub trait McpTransport: Send {
    fn initialize(&mut self) -> impl std::future::Future<Output = Result<(), McpClientError>> + Send;
    fn list_tools(&mut self) -> impl std::future::Future<Output = Result<Vec<DiscoveredMcpTool>, McpClientError>> + Send;
}

pub struct McpClientRuntime<T> {
    transport: T,
}

impl<T: McpTransport> McpClientRuntime<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub async fn discover_tools(&mut self) -> Result<Vec<DiscoveredMcpTool>, McpClientError> {
        self.transport.initialize().await?;
        self.transport.list_tools().await
    }
}
```

Use concrete generic transports first; add dynamic dispatch only if the runtime host needs it.

- [ ] **Step 3: Run tests**

```bash
cargo fmt --all --check
cargo test -p noema-core metadata_discovery_lists_tools_without_calling_them --no-fail-fast
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/noema-core/src/mcp.rs crates/noema-core/src/mcp/client.rs
git commit -m "feat: add metadata-only mcp client"
```

## Task 7: Tool Calibration Mutations

**Files:**
- Modify: `crates/noema-core/src/store/mcp.rs`
- Modify: `crates/noema-core/src/graphql/mcp.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/web/src/graphql/operations.ts`

- [ ] **Step 1: Add failing calibration tests**

Store test:

```rust
#[tokio::test]
async fn calibration_blocks_unresolved_ownership_until_reviewed() {
    let store = test_store_with_mcp_tool().await;
    let calibration = store
        .save_tool_calibration(NewToolCalibration {
            calibration_id: "tool_calibration:read_doc".to_string(),
            mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
            read_classification: McpTrustClassification::Mixed,
            write_classification: McpTrustClassification::None,
            export_classification: McpTrustClassification::None,
            owner_extractors: Vec::new(),
            enabled_agent_ids: vec!["agent:primary".to_string()],
            enabled_scope_ids: vec!["human:local".to_string()],
            status: McpCalibrationStatus::BlockedUnresolvedOwnership,
            reviewed_by: Some("human:local".to_string()),
            reviewed_metadata_fingerprint: Some("fingerprint_1".to_string()),
        })
        .await
        .expect("save calibration");
    assert_eq!(calibration.status, McpCalibrationStatus::BlockedUnresolvedOwnership);
}
```

- [ ] **Step 2: Implement calibration store and GraphQL mutation**

Add `NewToolCalibration` and `ToolCalibrationRecord` to `store/mcp.rs`. Add `save_tool_calibration` and `get_tool_calibration`.

Add GraphQL input:

```rust
#[derive(InputObject)]
pub struct GraphqlSaveToolCalibrationInput {
    pub calibration_id: String,
    pub mcp_tool_id: String,
    pub read_classification: String,
    pub write_classification: String,
    pub export_classification: String,
    pub enabled_agent_ids: Vec<String>,
    pub enabled_scope_ids: Vec<String>,
    pub status: String,
}
```

Parse enum strings with explicit matches and return `graphql_error` on invalid values.

- [ ] **Step 3: Add web mutation operation**

Add:

```ts
export const SaveToolCalibrationDocument = gql`
  mutation SaveToolCalibration($input: GraphqlSaveToolCalibrationInput!) {
    saveToolCalibration(input: $input) {
      calibrationId
      mcpToolId
      status
      readClassification
      writeClassification
      exportClassification
    }
  }
`;
```

- [ ] **Step 4: Run validation**

```bash
cargo fmt --all --check
cargo test -p noema-core calibration_blocks_unresolved_ownership_until_reviewed --no-fail-fast
cd crates/noema-core/web
bun run gen:types
bun run lint
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/store/mcp.rs crates/noema-core/src/graphql/mcp.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/generated/graphql.ts
git commit -m "feat: add mcp tool calibration"
```

## Task 8: Capability Gateway Policy Core

**Files:**
- Create: `crates/noema-core/src/capability.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Test: `crates/noema-core/src/capability.rs`

- [ ] **Step 1: Add failing policy tests**

```rust
#[test]
fn export_always_requires_approval_in_v1() {
    let decision = evaluate_capability_policy(CapabilityPolicyInput {
        axis: CapabilityAxis::Export,
        classification: McpTrustClassification::Trusted,
        owner_trust: OwnerTrust::Trusted,
    });
    assert_eq!(decision.outcome, CapabilityDecisionOutcome::RequireApproval);
}

#[test]
fn trusted_read_allows_without_examination() {
    let decision = evaluate_capability_policy(CapabilityPolicyInput {
        axis: CapabilityAxis::Read,
        classification: McpTrustClassification::Trusted,
        owner_trust: OwnerTrust::Trusted,
    });
    assert_eq!(decision.outcome, CapabilityDecisionOutcome::Allow);
}
```

- [ ] **Step 2: Implement policy types**

Create `capability.rs`:

```rust
use crate::McpTrustClassification;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityAxis {
    Read,
    Write,
    Export,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerTrust {
    Trusted,
    Untrusted,
    Mixed,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityDecisionOutcome {
    Allow,
    AllowSanitized,
    RequireApproval,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityPolicyInput {
    pub axis: CapabilityAxis,
    pub classification: McpTrustClassification,
    pub owner_trust: OwnerTrust,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPolicyDecision {
    pub outcome: CapabilityDecisionOutcome,
    pub reason: &'static str,
}

#[must_use]
pub fn evaluate_capability_policy(input: CapabilityPolicyInput) -> CapabilityPolicyDecision {
    if input.classification == McpTrustClassification::None {
        return CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Deny,
            reason: "axis_not_supported",
        };
    }
    if input.axis == CapabilityAxis::Export {
        return CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::RequireApproval,
            reason: "v1_export_requires_manual_approval",
        };
    }
    if input.owner_trust == OwnerTrust::Unresolved {
        return CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Deny,
            reason: "ownership_unresolved",
        };
    }
    match (input.axis, input.classification, input.owner_trust) {
        (_, McpTrustClassification::Trusted, OwnerTrust::Trusted) => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Allow,
            reason: "trusted_owner",
        },
        (CapabilityAxis::Read, _, _) => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::AllowSanitized,
            reason: "read_requires_examination",
        },
        (CapabilityAxis::Write, _, _) => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::RequireApproval,
            reason: "write_requires_examination",
        },
        _ => CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Deny,
            reason: "unsupported_decision",
        },
    }
}
```

- [ ] **Step 3: Export module and run tests**

```bash
cargo fmt --all --check
cargo test -p noema-core export_always_requires_approval_in_v1 trusted_read_allows_without_examination --no-fail-fast
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/noema-core/src/capability.rs crates/noema-core/src/lib.rs
git commit -m "feat: add capability policy core"
```

## Task 9: Ownership Resolver And Read Quarantine

**Files:**
- Create: `crates/noema-core/src/capability/ownership.rs`
- Create: `crates/noema-core/src/capability/examination.rs`
- Modify: `crates/noema-core/src/capability.rs`
- Modify: `crates/noema-core/src/store/mcp.rs`

- [ ] **Step 1: Add failing tests**

```rust
#[test]
fn json_pointer_owner_extractor_resolves_email_from_structured_content() {
    let extractor = OwnerExtractor {
        source: OwnerExtractorSource::StructuredContent,
        selector_kind: TrustedIdentitySelectorKind::Email,
        path: "/owner/email".to_string(),
    };
    let result = resolve_owner_from_json(
        &extractor,
        &serde_json::json!({"owner": {"email": "Kevin@Example.com"}}),
    )
    .expect("owner");
    assert_eq!(result.normalized_value, "kevin@example.com");
}

#[test]
fn unresolved_owner_blocks_read_release() {
    let decision = examine_read_result(ReadExaminationInput {
        owner_trust: OwnerTrust::Unresolved,
        content: "hello".to_string(),
    });
    assert_eq!(decision.outcome, CapabilityDecisionOutcome::Deny);
}
```

- [ ] **Step 2: Implement deterministic ownership extraction**

Implement JSON Pointer first:

```rust
pub fn resolve_owner_from_json(
    extractor: &OwnerExtractor,
    value: &serde_json::Value,
) -> Option<ResolvedOwner> {
    let raw = value.pointer(&extractor.path)?.as_str()?;
    let normalized =
        normalize_trusted_identity_value(extractor.selector_kind, raw)?;
    Some(ResolvedOwner {
        selector_kind: extractor.selector_kind,
        normalized_value: normalized,
    })
}
```

- [ ] **Step 3: Implement minimal read examination**

```rust
pub fn examine_read_result(input: ReadExaminationInput) -> CapabilityPolicyDecision {
    if input.owner_trust == OwnerTrust::Unresolved {
        return CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::Deny,
            reason: "ownership_unresolved",
        };
    }
    if contains_prompt_injection_marker(&input.content) {
        return CapabilityPolicyDecision {
            outcome: CapabilityDecisionOutcome::AllowSanitized,
            reason: "hostile_instruction_sanitized",
        };
    }
    CapabilityPolicyDecision {
        outcome: CapabilityDecisionOutcome::Allow,
        reason: "read_examined",
    }
}

fn contains_prompt_injection_marker(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    lower.contains("ignore previous instructions")
        || lower.contains("system prompt")
        || lower.contains("developer message")
}
```

This string-based scanner is only a conservative examination helper, not semantic user-intent detection.

- [ ] **Step 4: Run tests**

```bash
cargo fmt --all --check
cargo test -p noema-core json_pointer_owner_extractor_resolves_email_from_structured_content unresolved_owner_blocks_read_release --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/capability.rs crates/noema-core/src/capability/ownership.rs crates/noema-core/src/capability/examination.rs crates/noema-core/src/store/mcp.rs
git commit -m "feat: add ownership resolution and read quarantine"
```

## Task 10: Runtime Gateway Integration For Tool Proposals

**Files:**
- Create: `crates/noema-core/src/capability/gateway.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add failing runtime tests**

Add daemon tests:

```rust
#[tokio::test]
async fn uncalibrated_mcp_tool_call_returns_failed_tool_result() {
    let handle = test_runtime_handle(fake_codex_provider_with_mcp_tool_call()).await;
    handle
        .turn("conversation_1".to_string(), "read my doc".to_string(), test_tx())
        .await
        .expect("turn");
    let items = handle.store().list_conversation_items("conversation_1", ReplayMode::All).await.unwrap();
    assert!(items.iter().any(|item| {
        item.kind == ConversationItemKind::ToolResult
            && item.status == ConversationItemStatus::Failed
            && item.payload_json.to_string().contains("mcp_tool_not_calibrated")
    }));
}
```

- [ ] **Step 2: Implement gateway request/result types**

Create `gateway.rs` with:

```rust
pub struct CapabilityGateway<'a> {
    pub store: &'a NoemaStore,
}

pub struct GatewayToolProposal<'a> {
    pub name: &'a str,
    pub payload: &'a serde_json::Value,
    pub agent_id: &'a str,
    pub scope_ids: &'a [String],
}

pub struct GatewayToolResult {
    pub success: bool,
    pub payload: serde_json::Value,
    pub requires_provider_continuation: bool,
}
```

Implement `execute_tool_proposal` so unknown MCP tools fail closed with:

```json
{"error":"mcp_tool_not_calibrated"}
```

Keep existing `search_memory` and `update_own_name` local tools on their current path until the gateway can wrap them without behavior changes.

- [ ] **Step 3: Route MCP-shaped calls through gateway**

In `local_tools.rs`, detect MCP tool names with a prefix such as `mcp.<server>.<tool>` for the first integration slice. Unknown non-local tools should produce failed results instead of being ignored.

- [ ] **Step 4: Run tests**

```bash
cargo fmt --all --check
cargo test -p noema-core uncalibrated_mcp_tool_call_returns_failed_tool_result --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/capability/gateway.rs crates/noema-core/src/capability.rs crates/noema-core/src/daemon/runtime/local_tools.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/tests.rs
git commit -m "feat: mediate mcp tool proposals"
```

## Task 11: Durable Approval Requests For Exports

**Files:**
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/mcp.rs`
- Modify: `crates/noema-core/src/graphql/mcp.rs`
- Modify: `crates/noema-core/web/src/components/settings/ApprovalsSettingsPaneContent.tsx`

- [ ] **Step 1: Add failing approval tests**

```rust
#[tokio::test]
async fn export_decision_creates_manual_approval_request() {
    let store = test_store().await;
    let approval = store
        .create_mcp_approval_request(NewMcpApprovalRequest {
            approval_id: "approval:mcp:1".to_string(),
            action_summary: "Share Google Doc".to_string(),
            mcp_server_id: "mcp_server:google".to_string(),
            mcp_tool_id: "mcp_tool:google:share_doc".to_string(),
            requester_actor_id: "agent:primary".to_string(),
            owner_scope_id: "human:local".to_string(),
            payload_preview: serde_json::json!({"recipient": "person@example.com"}),
            status: "pending".to_string(),
        })
        .await
        .expect("approval");
    assert_eq!(approval.status, "pending");
}
```

- [ ] **Step 2: Add approval schema table**

Append:

```sql
DEFINE TABLE IF NOT EXISTS approval_requests SCHEMAFULL;
DEFINE FIELD OVERWRITE approval_id ON TABLE approval_requests TYPE string;
DEFINE FIELD OVERWRITE action_summary ON TABLE approval_requests TYPE string;
DEFINE FIELD OVERWRITE mcp_server_id ON TABLE approval_requests TYPE option<string>;
DEFINE FIELD OVERWRITE mcp_tool_id ON TABLE approval_requests TYPE option<string>;
DEFINE FIELD OVERWRITE requester_actor_id ON TABLE approval_requests TYPE string;
DEFINE FIELD OVERWRITE owner_scope_id ON TABLE approval_requests TYPE string;
DEFINE FIELD OVERWRITE payload_preview ON TABLE approval_requests TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE status ON TABLE approval_requests TYPE string ASSERT $value INSIDE ['pending', 'approved', 'denied', 'cancelled'];
DEFINE FIELD OVERWRITE decision_actor_id ON TABLE approval_requests TYPE option<string>;
DEFINE FIELD OVERWRITE decision_comment ON TABLE approval_requests TYPE option<string>;
DEFINE FIELD OVERWRITE decided_at ON TABLE approval_requests TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE approval_requests TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE approval_requests TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS approval_requests_approval_id ON TABLE approval_requests COLUMNS approval_id UNIQUE;
```

- [ ] **Step 3: Implement approval store/read model and GraphQL query**

Add `create_mcp_approval_request`, `list_mcp_approval_requests`, and GraphQL query `mcpApprovalRequests(status: String)`.

- [ ] **Step 4: Render approvals in Settings**

Update `ApprovalsSettingsPaneContent.tsx` to show rows with action, tool, status, and requester.

- [ ] **Step 5: Run validation**

```bash
cargo fmt --all --check
cargo test -p noema-core export_decision_creates_manual_approval_request --no-fail-fast
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/store/schema.rs crates/noema-core/src/store/mcp.rs crates/noema-core/src/graphql/mcp.rs crates/noema-core/web/src/components/settings/ApprovalsSettingsPaneContent.tsx crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/generated/graphql.ts
git commit -m "feat: add mcp approval requests"
```

## Task 12: End-To-End Validation And Documentation Sync

**Files:**
- Modify: `docs/context/current.md`
- Modify: `docs/frontend/governance-inspection.md`
- Modify: `docs/superpowers/specs/2026-06-30-third-party-mcp-control-plane-design.md` only if implementation changes the approved design.

- [ ] **Step 1: Run backend validation**

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: PASS. If tests fail with sandbox socket permission errors, rerun the same test command with escalated socket permissions and report the distinction.

- [ ] **Step 2: Run frontend validation**

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 3: Run ship checklist**

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: no unexpected unstaged files except intentionally preserved user work. `git diff --check` may fail on pre-existing unrelated `docs/memory.md` until that file is cleaned separately.

- [ ] **Step 4: Update durable docs**

Update `docs/context/current.md` with the implemented milestone and any remaining open loops. Update `docs/frontend/governance-inspection.md` with the Settings MCP surfaces if the UI differs from the spec.

- [ ] **Step 5: Commit validation/docs**

```bash
git add docs/context/current.md docs/frontend/governance-inspection.md
git commit -m "docs: sync mcp control plane status"
```

## Plan Self-Review

- Spec coverage: The plan covers mandatory metadata-only setup, read/write/export classification, trusted identity selectors, unresolved ownership blocking, read quarantine, V1 manual export approvals, Settings tabs, audit placeholders, GraphQL read models, and fail-closed runtime mediation.
- Scope: The feature remains large. Execution should stop after each task and commit. Runtime MCP execution should not be exposed to agents until Tasks 8-11 are complete.
- Placeholder scan: The plan avoids open-ended placeholder language. Where later implementation may choose exact internals, the plan supplies concrete initial choices such as JSON Pointer extractors, MCP tool name prefixing, and Settings tab routes.
- Type consistency: The plan uses `McpTrustClassification`, `TrustedIdentitySelectorKind`, `McpTransportKind`, `McpCalibrationStatus`, `CapabilityAxis`, `OwnerTrust`, and `CapabilityDecisionOutcome` consistently across tasks.
