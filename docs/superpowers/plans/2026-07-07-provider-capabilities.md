# Provider Capabilities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Noema's provider-capability layer so provider accounts can supply typed capabilities such as `model.generate`, `web.search`, and `web.fetch` while stable model-visible tools stay provider-neutral.

**Architecture:** Add provider capability vocabulary and static capability declarations under `provider`, expose safe capability metadata through GraphQL provider settings, persist explicit web-tool bindings separately from model-visible tool arguments, and route `web.search` / `web.fetch` through resolved provider capabilities. OpenAI hosted web search is added as an optional `web.search` backend; `web.fetch` remains direct HTTP only in this plan.

**Tech Stack:** Rust, SurrealDB schema strings, async-graphql, serde/serde_json, reqwest, existing OpenAI Responses transport helpers, Bun/Apollo generated frontend types.

## Global Constraints

- Providers are service integrations that can supply one or more Noema capabilities, not only model vendors.
- Keep stable model-visible tool names `web.search` and `web.fetch`.
- Do not add a model-visible `provider` argument to `web.search` or `web.fetch`.
- OpenAI hosted web search is a future `web.search` provider option, not a `web.fetch` provider.
- Codex native web search remains unavailable until there is a documented callable provider API.
- Firecrawl is an illustrative example only; do not add Firecrawl provider code, schema rows, settings options, tests, or docs in this implementation.
- No `web.crawl` implementation in this plan.
- No browser-backed fetch in this plan.
- No hosted fetch provider in this plan.
- Pre-V1 schema changes may rewrite tables/docs directly. Do not add migrations or compatibility layers.
- Run unit tests only. Do not run smoke tests or fixture tests unless explicitly requested.
- Do not write UI/frontend tests unless explicitly requested.
- Do not modify `CARGO_BUILD_RUSTC_WRAPPER` or otherwise interfere with `sccache`.

---

## File Structure

- Create `crates/noema-core/src/provider/capabilities.rs`
  - Provider capability ids, feature flags, static declarations, and filtering helpers.
- Modify `crates/noema-core/src/provider.rs`
  - Re-export capability types.
- Modify `crates/noema-core/src/provider/accounts.rs`
  - Attach derived capability metadata to `ProviderAccountRecord`.
- Modify `crates/noema-core/src/store/provider_accounts.rs`
  - Hydrate provider account records with derived capabilities.
- Modify `crates/noema-core/src/store/schema.rs`
  - Add a pre-V1 binding table for active web tool provider selections.
- Create `crates/noema-core/src/store/provider_capability_bindings.rs`
  - Store helpers for `web.search` and `web.fetch` capability bindings.
- Modify `crates/noema-core/src/store.rs`
  - Export the new store module and binding types.
- Modify `crates/noema-core/src/graphql/provider_accounts.rs`
  - Expose capabilities on provider account settings rows.
- Modify `crates/noema-core/src/graphql/web_fetch_settings.rs`
  - Keep summarizer model settings intact and add web tool binding data behind a broader `webToolSettings` query in a later task.
- Create `crates/noema-core/src/graphql/web_tool_settings.rs`
  - GraphQL query/mutation for active web search/fetch provider bindings.
- Modify `crates/noema-core/src/graphql/schema.rs`
  - Add the new GraphQL query and mutation fields.
- Modify `crates/noema-core/src/search/types.rs`
  - Extend search response metadata and add an OpenAI-hosted provider variant.
- Create `crates/noema-core/src/search/openai_hosted.rs`
  - OpenAI hosted web search adapter.
- Modify `crates/noema-core/src/search/tool.rs`
  - Preserve the stable model-visible schema and normalize new provider metadata.
- Modify `crates/noema-core/src/daemon/runtime/actor.rs`
  - Replace fixed web provider fields with a capability-aware web tool runtime.
- Modify `crates/noema-core/src/daemon/runtime/local_tools.rs`
  - Resolve the effective web provider binding at tool execution time.
- Modify `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
  - Display provider contract and fallback metadata safely.
- Modify `crates/noema-core/web/src/components/settings/WebSettingsPane.tsx`
  - Query and save web provider bindings.
- Modify `crates/noema-core/web/src/components/settings/WebSettingsPaneContent.tsx`
  - Show active search/fetch bindings and behavior labels.
- Modify `crates/noema-core/web/src/graphql/settings.graphql`
  - Add web tool settings query and save mutation.
- Regenerate frontend GraphQL artifacts with `bun run gen:types`.

---

### Task 1: Provider Capability Vocabulary

**Files:**
- Create: `crates/noema-core/src/provider/capabilities.rs`
- Modify: `crates/noema-core/src/provider.rs`
- Test: `crates/noema-core/src/provider/capabilities.rs`

**Interfaces:**
- Produces: `CapabilityId`, `ProviderCapability`, `CapabilityFeatures`, `ProviderCapabilityStatus`, `ReliabilityContract`, `DataFlowClass`, `ResultPersistencePolicy`.
- Produces: `capabilities_for_provider_account(provider_kind: &str, account_key: &str, account_status: ProviderAccountStatus) -> Vec<ProviderCapability>`.
- Consumes: `ProviderAccountStatus` from `crate::provider::accounts`.

- [ ] **Step 1: Write failing tests for static capability declarations**

Add tests at the bottom of `crates/noema-core/src/provider/capabilities.rs` while creating the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderAccountStatus;

    #[test]
    fn openai_account_declares_models_and_hosted_search() {
        let capabilities =
            capabilities_for_provider_account("openai", "default", ProviderAccountStatus::Authenticated);
        let ids = capabilities
            .iter()
            .map(|capability| capability.capability_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["model.generate", "model.classify", "web.search"]);
        assert!(capabilities.iter().any(|capability| {
            capability.provider_kind == "openai"
                && capability.account_key == "default"
                && capability.capability_id == CapabilityId::WebSearch
                && capability.features.citations
                && !capability.features.direct_url_fetch
        }));
    }

    #[test]
    fn codex_account_does_not_declare_native_web_search() {
        let capabilities =
            capabilities_for_provider_account("codex", "default", ProviderAccountStatus::Authenticated);

        assert!(capabilities.iter().any(|capability| {
            capability.capability_id == CapabilityId::ModelGenerate
        }));
        assert!(!capabilities.iter().any(|capability| {
            capability.capability_id == CapabilityId::WebSearch
        }));
    }

    #[test]
    fn system_web_providers_have_no_secret_requirements() {
        let search =
            capabilities_for_provider_account("duckduckgo_public", "system", ProviderAccountStatus::Authenticated);
        let fetch =
            capabilities_for_provider_account("direct_http", "system", ProviderAccountStatus::Authenticated);

        assert_eq!(search[0].capability_id, CapabilityId::WebSearch);
        assert_eq!(search[0].reliability_contract, ReliabilityContract::BestEffortPublic);
        assert_eq!(fetch[0].capability_id, CapabilityId::WebFetch);
        assert!(fetch[0].features.direct_url_fetch);
    }

    #[test]
    fn firecrawl_is_not_declared() {
        let capabilities =
            capabilities_for_provider_account("firecrawl", "default", ProviderAccountStatus::Authenticated);

        assert!(capabilities.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p noema-core provider::capabilities::tests::openai_account_declares_models_and_hosted_search --no-fail-fast
```

Expected: FAIL because `provider::capabilities` does not exist yet.

- [ ] **Step 3: Implement capability types and static declarations**

Create `crates/noema-core/src/provider/capabilities.rs`:

```rust
use crate::ProviderAccountStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum CapabilityId {
    ModelGenerate,
    ModelClassify,
    WebSearch,
    WebFetch,
}

impl CapabilityId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelGenerate => "model.generate",
            Self::ModelClassify => "model.classify",
            Self::WebSearch => "web.search",
            Self::WebFetch => "web.fetch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ProviderCapabilityStatus {
    Available,
    AccountDependent,
    Unavailable,
}

impl ProviderCapabilityStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::AccountDependent => "account_dependent",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ReliabilityContract {
    FirstParty,
    HostedProvider,
    BestEffortPublic,
}

impl ReliabilityContract {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstParty => "first_party",
            Self::HostedProvider => "hosted_provider",
            Self::BestEffortPublic => "best_effort_public",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum DataFlowClass {
    ModelProviderPrompt,
    TrustedExternalSearchQuery,
    ExternalWebFetch,
}

impl DataFlowClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelProviderPrompt => "model_provider_prompt",
            Self::TrustedExternalSearchQuery => "trusted_external_search_query",
            Self::ExternalWebFetch => "external_web_fetch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ResultPersistencePolicy {
    CompactMetadata,
    CompactContent,
}

impl ResultPersistencePolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompactMetadata => "compact_metadata",
            Self::CompactContent => "compact_content",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct CapabilityFeatures {
    pub citations: bool,
    pub direct_url_fetch: bool,
    pub js_rendering: bool,
    pub authenticated_context: bool,
    pub result_persistence: ResultPersistencePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct ProviderCapability {
    pub provider_kind: String,
    pub account_key: String,
    pub capability_id: CapabilityId,
    pub status: ProviderCapabilityStatus,
    pub reliability_contract: ReliabilityContract,
    pub data_flow_class: DataFlowClass,
    pub features: CapabilityFeatures,
}

#[must_use]
pub fn capabilities_for_provider_account(
    provider_kind: &str,
    account_key: &str,
    account_status: ProviderAccountStatus,
) -> Vec<ProviderCapability> {
    let status = capability_status_for_account(account_status);
    match provider_kind {
        "openai" => vec![
            model_capability(provider_kind, account_key, CapabilityId::ModelGenerate, status),
            model_capability(provider_kind, account_key, CapabilityId::ModelClassify, status),
            ProviderCapability {
                provider_kind: provider_kind.to_string(),
                account_key: account_key.to_string(),
                capability_id: CapabilityId::WebSearch,
                status,
                reliability_contract: ReliabilityContract::HostedProvider,
                data_flow_class: DataFlowClass::TrustedExternalSearchQuery,
                features: CapabilityFeatures {
                    citations: true,
                    direct_url_fetch: false,
                    js_rendering: false,
                    authenticated_context: false,
                    result_persistence: ResultPersistencePolicy::CompactMetadata,
                },
            },
        ],
        "codex" | "foundation_local" => vec![
            model_capability(provider_kind, account_key, CapabilityId::ModelGenerate, status),
            model_capability(provider_kind, account_key, CapabilityId::ModelClassify, status),
        ],
        "duckduckgo_public" => vec![ProviderCapability {
            provider_kind: provider_kind.to_string(),
            account_key: account_key.to_string(),
            capability_id: CapabilityId::WebSearch,
            status: ProviderCapabilityStatus::Available,
            reliability_contract: ReliabilityContract::BestEffortPublic,
            data_flow_class: DataFlowClass::TrustedExternalSearchQuery,
            features: CapabilityFeatures {
                citations: false,
                direct_url_fetch: false,
                js_rendering: false,
                authenticated_context: false,
                result_persistence: ResultPersistencePolicy::CompactMetadata,
            },
        }],
        "direct_http" => vec![ProviderCapability {
            provider_kind: provider_kind.to_string(),
            account_key: account_key.to_string(),
            capability_id: CapabilityId::WebFetch,
            status: ProviderCapabilityStatus::Available,
            reliability_contract: ReliabilityContract::FirstParty,
            data_flow_class: DataFlowClass::ExternalWebFetch,
            features: CapabilityFeatures {
                citations: false,
                direct_url_fetch: true,
                js_rendering: false,
                authenticated_context: false,
                result_persistence: ResultPersistencePolicy::CompactContent,
            },
        }],
        _ => Vec::new(),
    }
}

fn model_capability(
    provider_kind: &str,
    account_key: &str,
    capability_id: CapabilityId,
    status: ProviderCapabilityStatus,
) -> ProviderCapability {
    ProviderCapability {
        provider_kind: provider_kind.to_string(),
        account_key: account_key.to_string(),
        capability_id,
        status,
        reliability_contract: ReliabilityContract::HostedProvider,
        data_flow_class: DataFlowClass::ModelProviderPrompt,
        features: CapabilityFeatures {
            citations: false,
            direct_url_fetch: false,
            js_rendering: false,
            authenticated_context: false,
            result_persistence: ResultPersistencePolicy::CompactMetadata,
        },
    }
}

const fn capability_status_for_account(
    account_status: ProviderAccountStatus,
) -> ProviderCapabilityStatus {
    match account_status {
        ProviderAccountStatus::Authenticated => ProviderCapabilityStatus::Available,
        ProviderAccountStatus::Unknown | ProviderAccountStatus::Checking => {
            ProviderCapabilityStatus::AccountDependent
        }
        ProviderAccountStatus::Unauthenticated | ProviderAccountStatus::Unavailable => {
            ProviderCapabilityStatus::Unavailable
        }
    }
}
```

Modify `crates/noema-core/src/provider.rs`:

```rust
/// Provider capability declarations and behavior metadata.
pub mod capabilities;

pub use capabilities::{
    capabilities_for_provider_account, CapabilityFeatures, CapabilityId, DataFlowClass,
    ProviderCapability, ProviderCapabilityStatus, ReliabilityContract, ResultPersistencePolicy,
};
```

- [ ] **Step 4: Run task tests**

Run:

```bash
cargo test -p noema-core provider::capabilities --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/provider.rs crates/noema-core/src/provider/capabilities.rs
git commit -m "feat: add provider capability vocabulary"
```

---

### Task 2: Attach Capabilities To Provider Accounts

**Files:**
- Modify: `crates/noema-core/src/provider/accounts.rs`
- Modify: `crates/noema-core/src/store/provider_accounts.rs`
- Test: `crates/noema-core/src/store/tests.rs`

**Interfaces:**
- Consumes: `capabilities_for_provider_account` from Task 1.
- Produces: `ProviderAccountRecord.capabilities: Vec<ProviderCapability>`.
- Produces: `NoemaStore::system_provider_accounts() -> Vec<ProviderAccountRecord>`.

- [ ] **Step 1: Write failing store test for derived capabilities**

Add this test to `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn provider_accounts_include_derived_capabilities() {
    let store = test_store().await;
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("default account");

    assert_eq!(account.provider_kind, "codex");
    assert!(account
        .capabilities
        .iter()
        .any(|capability| capability.capability_id.as_str() == "model.generate"));
    assert!(!account
        .capabilities
        .iter()
        .any(|capability| capability.capability_id.as_str() == "web.search"));

    let system_accounts = store.system_provider_accounts();
    assert!(system_accounts.iter().any(|account| {
        account.provider_kind == "duckduckgo_public"
            && account.capabilities.iter().any(|capability| {
                capability.capability_id.as_str() == "web.search"
            })
    }));
    assert!(system_accounts.iter().any(|account| {
        account.provider_kind == "direct_http"
            && account.capabilities.iter().any(|capability| {
                capability.capability_id.as_str() == "web.fetch"
            })
    }));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p noema-core provider_accounts_include_derived_capabilities --no-fail-fast
```

Expected: FAIL because `ProviderAccountRecord.capabilities` and `system_provider_accounts` do not exist.

- [ ] **Step 3: Add derived capabilities to account records**

Modify `crates/noema-core/src/provider/accounts.rs`:

```rust
use crate::ProviderCapability;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderAccountRecord {
    pub provider_account_id: String,
    pub provider_kind: String,
    pub account_key: String,
    pub display_name: String,
    pub auth_method: ProviderAuthMethod,
    pub is_active: bool,
    pub is_default: bool,
    pub status: ProviderAccountStatus,
    pub last_checked_at: Option<String>,
    pub last_authenticated_at: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub metadata: Value,
    #[serde(default)]
    pub capabilities: Vec<ProviderCapability>,
}
```

Modify `provider_account_from_row` in `crates/noema-core/src/store/provider_accounts.rs`:

```rust
fn provider_account_from_row(row: ProviderAccountRow) -> Result<ProviderAccountRecord, StoreError> {
    let capabilities = crate::capabilities_for_provider_account(
        &row.provider_kind,
        &row.account_key,
        parse_provider_status(&row.status)?,
    );
    Ok(ProviderAccountRecord {
        provider_account_id: row.provider_account_id,
        provider_kind: row.provider_kind,
        account_key: row.account_key,
        display_name: row.display_name,
        auth_method: parse_provider_auth_method(&row.auth_method)?,
        is_active: row.is_active,
        is_default: row.is_default,
        status: parse_provider_status(&row.status)?,
        last_checked_at: row.last_checked_at,
        last_authenticated_at: row.last_authenticated_at,
        last_error_code: row.last_error_code,
        last_error_message: row.last_error_message,
        metadata: row.metadata,
        capabilities,
    })
}
```

Keep `parse_provider_status` as a helper so the status is parsed once before deriving capabilities:

```rust
fn parse_provider_status(value: &str) -> Result<ProviderAccountStatus, StoreError> {
    match value {
        "unknown" => Ok(ProviderAccountStatus::Unknown),
        "checking" => Ok(ProviderAccountStatus::Checking),
        "authenticated" => Ok(ProviderAccountStatus::Authenticated),
        "unauthenticated" => Ok(ProviderAccountStatus::Unauthenticated),
        "unavailable" => Ok(ProviderAccountStatus::Unavailable),
        _ => invalid_enum("provider_account_status", value),
    }
}
```

Add system account records in `impl NoemaStore`:

```rust
#[must_use]
pub fn system_provider_accounts(&self) -> Vec<ProviderAccountRecord> {
    vec![
        system_provider_account("duckduckgo_public", "DuckDuckGo public search"),
        system_provider_account("direct_http", "Direct HTTP web fetch"),
    ]
}

fn system_provider_account(provider_kind: &str, display_name: &str) -> ProviderAccountRecord {
    let account_key = "system".to_string();
    let status = ProviderAccountStatus::Authenticated;
    ProviderAccountRecord {
        provider_account_id: format!("provider_account:{provider_kind}:system"),
        provider_kind: provider_kind.to_string(),
        account_key: account_key.clone(),
        display_name: display_name.to_string(),
        auth_method: ProviderAuthMethod::None,
        is_active: true,
        is_default: true,
        status,
        last_checked_at: None,
        last_authenticated_at: None,
        last_error_code: None,
        last_error_message: None,
        metadata: serde_json::json!({}),
        capabilities: crate::capabilities_for_provider_account(provider_kind, &account_key, status),
    }
}
```

- [ ] **Step 4: Run task test**

Run:

```bash
cargo test -p noema-core provider_accounts_include_derived_capabilities --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Run provider account test subset**

Run:

```bash
cargo test -p noema-core provider_account --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/provider/accounts.rs crates/noema-core/src/store/provider_accounts.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: derive capabilities for provider accounts"
```

---

### Task 3: GraphQL Provider Capability Metadata

**Files:**
- Modify: `crates/noema-core/src/graphql/provider_accounts.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Test: `crates/noema-core/src/graphql/provider_accounts.rs`

**Interfaces:**
- Consumes: `ProviderAccountRecord.capabilities` from Task 2.
- Produces: `GraphqlProviderCapability`, `GraphqlCapabilityFeatures`, and `ProviderAccount.capabilities`.

- [ ] **Step 1: Write failing GraphQL mapping test**

Add this test to `crates/noema-core/src/graphql/provider_accounts.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        capabilities_for_provider_account, ProviderAccountRecord, ProviderAccountStatus,
        ProviderAuthMethod,
    };
    use serde_json::json;

    #[test]
    fn graphql_provider_account_exposes_capabilities() {
        let account = ProviderAccountRecord {
            provider_account_id: "provider_account:openai:default".to_string(),
            provider_kind: "openai".to_string(),
            account_key: "default".to_string(),
            display_name: "OpenAI".to_string(),
            auth_method: ProviderAuthMethod::SecretInput,
            is_active: true,
            is_default: true,
            status: ProviderAccountStatus::Authenticated,
            last_checked_at: None,
            last_authenticated_at: None,
            last_error_code: None,
            last_error_message: None,
            metadata: json!({}),
            capabilities: capabilities_for_provider_account(
                "openai",
                "default",
                ProviderAccountStatus::Authenticated,
            ),
        };

        let graphql = GraphqlProviderAccount::from(account);

        assert!(graphql.capabilities.iter().any(|capability| {
            capability.capability_id == "web.search"
                && capability.status == "available"
                && capability.features.citations
                && !capability.features.direct_url_fetch
        }));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p noema-core graphql::provider_accounts::tests::graphql_provider_account_exposes_capabilities --no-fail-fast
```

Expected: FAIL because `GraphqlProviderAccount.capabilities` does not exist.

- [ ] **Step 3: Add GraphQL capability objects**

Modify `crates/noema-core/src/graphql/provider_accounts.rs`:

```rust
use crate::{ProviderCapability, ResultPersistencePolicy};

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProviderCapability")]
pub struct GraphqlProviderCapability {
    pub capability_id: String,
    pub status: String,
    pub reliability_contract: String,
    pub data_flow_class: String,
    pub features: GraphqlCapabilityFeatures,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "CapabilityFeatures")]
pub struct GraphqlCapabilityFeatures {
    pub citations: bool,
    pub direct_url_fetch: bool,
    pub js_rendering: bool,
    pub authenticated_context: bool,
    pub result_persistence: String,
}

impl From<ProviderCapability> for GraphqlProviderCapability {
    fn from(capability: ProviderCapability) -> Self {
        Self {
            capability_id: capability.capability_id.as_str().to_string(),
            status: capability.status.as_str().to_string(),
            reliability_contract: capability.reliability_contract.as_str().to_string(),
            data_flow_class: capability.data_flow_class.as_str().to_string(),
            features: GraphqlCapabilityFeatures {
                citations: capability.features.citations,
                direct_url_fetch: capability.features.direct_url_fetch,
                js_rendering: capability.features.js_rendering,
                authenticated_context: capability.features.authenticated_context,
                result_persistence: result_persistence_label(
                    capability.features.result_persistence,
                )
                .to_string(),
            },
        }
    }
}

const fn result_persistence_label(policy: ResultPersistencePolicy) -> &'static str {
    policy.as_str()
}
```

Add the field:

```rust
pub struct GraphqlProviderAccount {
    pub provider_kind: String,
    pub account_key: String,
    pub display_name: String,
    pub auth_method: String,
    pub status: GraphqlProviderAccountStatus,
    pub is_active: bool,
    pub is_default: bool,
    pub last_checked_at: Option<String>,
    pub last_authenticated_at: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub capabilities: Vec<GraphqlProviderCapability>,
}
```

Map it in `impl From<ProviderAccountRecord>`:

```rust
capabilities: account.capabilities.into_iter().map(Into::into).collect(),
```

Update imports in `crates/noema-core/src/graphql/schema.rs`:

```rust
provider_accounts::{
    self, GraphqlCapabilityFeatures, GraphqlProviderAccount, GraphqlProviderCapability,
},
```

This import keeps async-graphql schema generation aware of the nested types.

- [ ] **Step 4: Run task test**

Run:

```bash
cargo test -p noema-core graphql::provider_accounts::tests::graphql_provider_account_exposes_capabilities --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Run schema generation check**

Run:

```bash
cargo test -p noema-core graphql --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/graphql/provider_accounts.rs crates/noema-core/src/graphql/schema.rs
git commit -m "feat: expose provider capabilities in GraphQL"
```

---

### Task 4: Persist Web Tool Capability Bindings

**Files:**
- Modify: `crates/noema-core/src/store/schema.rs`
- Create: `crates/noema-core/src/store/provider_capability_bindings.rs`
- Modify: `crates/noema-core/src/store.rs`
- Test: `crates/noema-core/src/store/tests.rs`

**Interfaces:**
- Produces: `ProviderCapabilityBindingRecord`.
- Produces: `NoemaStore::upsert_provider_capability_binding(tool_name, capability_id, provider_account_id)`.
- Produces: `NoemaStore::provider_capability_binding(tool_name, capability_id)`.
- Consumes: no model-visible tool argument changes.

- [ ] **Step 1: Write failing store binding test**

Add to `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn provider_capability_binding_round_trips_web_search() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");

    let saved = store
        .upsert_provider_capability_binding(
            "web.search",
            "web.search",
            "provider_account:openai:default",
        )
        .await
        .expect("save binding");

    assert_eq!(saved.tool_name, "web.search");
    assert_eq!(saved.capability_id, "web.search");
    assert_eq!(saved.provider_account_id, "provider_account:openai:default");

    let loaded = store
        .provider_capability_binding("web.search", "web.search")
        .await
        .expect("load binding")
        .expect("binding exists");

    assert_eq!(loaded.provider_account_id, "provider_account:openai:default");
}

#[test]
fn web_tool_specs_do_not_gain_provider_argument() {
    let search = crate::search::tool::web_search_tool_spec().expect("search spec");
    let fetch = crate::web_fetch::tool::web_fetch_tool_spec().expect("fetch spec");

    assert!(search.input_schema.as_value()["properties"].get("provider").is_none());
    assert!(fetch.input_schema.as_value()["properties"].get("provider").is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p noema-core provider_capability_binding_round_trips_web_search --no-fail-fast
```

Expected: FAIL because the binding store module does not exist.

- [ ] **Step 3: Add schema table**

Modify `crates/noema-core/src/store/schema.rs`:

```rust
DEFINE TABLE IF NOT EXISTS provider_capability_bindings SCHEMAFULL;
DEFINE FIELD OVERWRITE binding_id ON TABLE provider_capability_bindings TYPE string;
DEFINE FIELD OVERWRITE tool_name ON TABLE provider_capability_bindings TYPE string ASSERT $value INSIDE ['web.search', 'web.fetch'];
DEFINE FIELD OVERWRITE capability_id ON TABLE provider_capability_bindings TYPE string ASSERT $value INSIDE ['web.search', 'web.fetch'];
DEFINE FIELD OVERWRITE provider_account_id ON TABLE provider_capability_bindings TYPE string;
DEFINE FIELD OVERWRITE created_at ON TABLE provider_capability_bindings TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE provider_capability_bindings TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS provider_capability_bindings_binding_id ON TABLE provider_capability_bindings COLUMNS binding_id UNIQUE;
DEFINE INDEX IF NOT EXISTS provider_capability_bindings_unique_tool_capability ON TABLE provider_capability_bindings COLUMNS tool_name, capability_id UNIQUE;
```

- [ ] **Step 4: Implement binding store helpers**

Create `crates/noema-core/src/store/provider_capability_bindings.rs`:

```rust
use serde::Deserialize;
use surrealdb::types::SurrealValue;

use super::{NoemaStore, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilityBindingRecord {
    pub binding_id: String,
    pub tool_name: String,
    pub capability_id: String,
    pub provider_account_id: String,
}

impl NoemaStore {
    pub async fn upsert_provider_capability_binding(
        &self,
        tool_name: &str,
        capability_id: &str,
        provider_account_id: &str,
    ) -> Result<ProviderCapabilityBindingRecord, StoreError> {
        let binding_id = binding_id(tool_name, capability_id);
        self.db
            .query(
                r#"
                UPSERT type::record('provider_capability_bindings', $record_id) SET
                  binding_id = $binding_id,
                  tool_name = $tool_name,
                  capability_id = $capability_id,
                  provider_account_id = $provider_account_id,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&binding_id)))
            .bind(("binding_id", binding_id.clone()))
            .bind(("tool_name", tool_name.to_string()))
            .bind(("capability_id", capability_id.to_string()))
            .bind(("provider_account_id", provider_account_id.to_string()))
            .await?
            .check()?;
        self.provider_capability_binding(tool_name, capability_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("provider capability binding {binding_id} was not readable after save"),
            })
    }

    pub async fn provider_capability_binding(
        &self,
        tool_name: &str,
        capability_id: &str,
    ) -> Result<Option<ProviderCapabilityBindingRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT binding_id, tool_name, capability_id, provider_account_id
                FROM provider_capability_bindings
                WHERE tool_name = $tool_name
                  AND capability_id = $capability_id
                LIMIT 1;
                "#,
            )
            .bind(("tool_name", tool_name.to_string()))
            .bind(("capability_id", capability_id.to_string()))
            .await?;
        let rows: Vec<ProviderCapabilityBindingRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(Into::into))
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct ProviderCapabilityBindingRow {
    binding_id: String,
    tool_name: String,
    capability_id: String,
    provider_account_id: String,
}

impl From<ProviderCapabilityBindingRow> for ProviderCapabilityBindingRecord {
    fn from(row: ProviderCapabilityBindingRow) -> Self {
        Self {
            binding_id: row.binding_id,
            tool_name: row.tool_name,
            capability_id: row.capability_id,
            provider_account_id: row.provider_account_id,
        }
    }
}

fn binding_id(tool_name: &str, capability_id: &str) -> String {
    format!("provider_capability_binding:{tool_name}:{capability_id}")
}

fn record_fragment(id: &str) -> String {
    id.replace(':', "_").replace('.', "_")
}
```

If `StoreError::InvariantViolation` does not exist, add:

```rust
#[error("store invariant violated: {message}")]
InvariantViolation { message: String },
```

to `crates/noema-core/src/store/error.rs`.

Modify `crates/noema-core/src/store.rs`:

```rust
mod provider_capability_bindings;
pub use provider_capability_bindings::ProviderCapabilityBindingRecord;
```

- [ ] **Step 5: Run task tests**

Run:

```bash
cargo test -p noema-core provider_capability_binding_round_trips_web_search web_tool_specs_do_not_gain_provider_argument --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/store/schema.rs crates/noema-core/src/store/provider_capability_bindings.rs crates/noema-core/src/store.rs crates/noema-core/src/store/error.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: persist web provider capability bindings"
```

---

### Task 5: Resolve Web Tool Providers From Capability Bindings

**Files:**
- Create: `crates/noema-core/src/daemon/runtime/web_tools.rs`
- Modify: `crates/noema-core/src/daemon/runtime/actor.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Modify: `crates/noema-core/src/daemon/runtime/mod.rs`
- Test: `crates/noema-core/src/daemon/runtime/web_tools.rs`

**Interfaces:**
- Consumes: binding helpers from Task 4.
- Produces: `ResolvedWebSearchProvider` and `ResolvedWebFetchProvider`.
- Produces: `CodexRuntimeActor::resolved_web_search_provider()` and `CodexRuntimeActor::resolved_web_fetch_provider()`.
- Preserves: default DuckDuckGo search and direct HTTP fetch behavior when no binding exists.

- [ ] **Step 1: Write failing resolver tests**

Create `crates/noema-core/src/daemon/runtime/web_tools.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::test_store;

    #[tokio::test]
    async fn resolves_duckduckgo_default_without_binding() {
        let store = test_store().await;
        let resolved = resolve_web_search_provider(&store).await.expect("resolve");

        assert_eq!(resolved.provider_account_id, "provider_account:duckduckgo_public:system");
        assert_eq!(resolved.provider_kind, "duckduckgo_public");
        assert!(resolved.fallback_from.is_none());
    }

    #[tokio::test]
    async fn resolves_direct_http_default_without_binding() {
        let store = test_store().await;
        let resolved = resolve_web_fetch_provider(&store).await.expect("resolve");

        assert_eq!(resolved.provider_account_id, "provider_account:direct_http:system");
        assert_eq!(resolved.provider_kind, "direct_http");
        assert!(resolved.fallback_from.is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p noema-core daemon::runtime::web_tools --no-fail-fast
```

Expected: FAIL because `web_tools` is not wired into `daemon::runtime`.

- [ ] **Step 3: Implement default resolver**

Create `crates/noema-core/src/daemon/runtime/web_tools.rs`:

```rust
use crate::{NoemaStore, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon) struct ResolvedWebSearchProvider {
    pub provider_account_id: String,
    pub provider_kind: String,
    pub account_key: String,
    pub fallback_from: Option<String>,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon) struct ResolvedWebFetchProvider {
    pub provider_account_id: String,
    pub provider_kind: String,
    pub account_key: String,
    pub fallback_from: Option<String>,
    pub fallback_reason: Option<String>,
}

pub(in crate::daemon) async fn resolve_web_search_provider(
    store: &NoemaStore,
) -> Result<ResolvedWebSearchProvider, StoreError> {
    if let Some(binding) = store
        .provider_capability_binding("web.search", "web.search")
        .await?
    {
        return Ok(ResolvedWebSearchProvider {
            provider_account_id: binding.provider_account_id,
            provider_kind: "openai".to_string(),
            account_key: "default".to_string(),
            fallback_from: None,
            fallback_reason: None,
        });
    }
    Ok(ResolvedWebSearchProvider {
        provider_account_id: "provider_account:duckduckgo_public:system".to_string(),
        provider_kind: "duckduckgo_public".to_string(),
        account_key: "system".to_string(),
        fallback_from: None,
        fallback_reason: None,
    })
}

pub(in crate::daemon) async fn resolve_web_fetch_provider(
    store: &NoemaStore,
) -> Result<ResolvedWebFetchProvider, StoreError> {
    if let Some(binding) = store
        .provider_capability_binding("web.fetch", "web.fetch")
        .await?
    {
        return Ok(ResolvedWebFetchProvider {
            provider_account_id: binding.provider_account_id,
            provider_kind: "direct_http".to_string(),
            account_key: "system".to_string(),
            fallback_from: None,
            fallback_reason: None,
        });
    }
    Ok(ResolvedWebFetchProvider {
        provider_account_id: "provider_account:direct_http:system".to_string(),
        provider_kind: "direct_http".to_string(),
        account_key: "system".to_string(),
        fallback_from: None,
        fallback_reason: None,
    })
}
```

In this task, only defaults are executable. Task 8 replaces the temporary OpenAI branch with real account resolution.

Modify `crates/noema-core/src/daemon/runtime/mod.rs`:

```rust
mod web_tools;
```

Add methods in `crates/noema-core/src/daemon/runtime/actor.rs`:

```rust
pub(in crate::daemon) async fn resolved_web_search_provider(
    &self,
) -> Result<super::web_tools::ResolvedWebSearchProvider, crate::StoreError> {
    super::web_tools::resolve_web_search_provider(&self.store).await
}

pub(in crate::daemon) async fn resolved_web_fetch_provider(
    &self,
) -> Result<super::web_tools::ResolvedWebFetchProvider, crate::StoreError> {
    super::web_tools::resolve_web_fetch_provider(&self.store).await
}
```

Do not remove `search_provider` or `web_fetch_provider` fields in this task. They still own executable default providers.

- [ ] **Step 4: Run resolver tests**

Run:

```bash
cargo test -p noema-core daemon::runtime::web_tools --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/daemon/runtime/web_tools.rs crates/noema-core/src/daemon/runtime/actor.rs crates/noema-core/src/daemon/runtime/mod.rs
git commit -m "feat: resolve default web provider bindings"
```

---

### Task 6: Web Tool Settings GraphQL

**Files:**
- Create: `crates/noema-core/src/graphql/web_tool_settings.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Test: `crates/noema-core/src/graphql/web_tool_settings.rs`

**Interfaces:**
- Consumes: Provider account capabilities from Tasks 2-3.
- Consumes: binding helpers from Task 4.
- Produces: `webToolSettings` query.
- Produces: `saveWebToolProviderBinding(input: SaveWebToolProviderBindingInput!)`.

- [ ] **Step 1: Write failing GraphQL resolver tests**

Create `crates/noema-core/src/graphql/web_tool_settings.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::schema::GraphqlState;

    #[tokio::test]
    async fn web_tool_settings_lists_only_matching_capabilities() {
        let store = crate::store::tests::test_store().await;
        store.ensure_default_provider_account().await.expect("codex");
        let state = GraphqlState::for_tests_with_store(store);

        let settings = web_tool_settings(&state).await.expect("settings");

        assert!(settings.search.provider_options.iter().any(|option| {
            option.provider_account_id == "provider_account:duckduckgo_public:system"
        }));
        assert!(settings.fetch.provider_options.iter().any(|option| {
            option.provider_account_id == "provider_account:direct_http:system"
        }));
        assert!(!settings.search.provider_options.iter().any(|option| {
            option.provider_kind == "codex"
        }));
        assert!(!settings.fetch.provider_options.iter().any(|option| {
            option.provider_kind == "openai"
        }));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p noema-core graphql::web_tool_settings --no-fail-fast
```

Expected: FAIL because the module is not implemented.

- [ ] **Step 3: Implement GraphQL web tool settings**

Create `crates/noema-core/src/graphql/web_tool_settings.rs`:

```rust
use async_graphql::{InputObject, Result, SimpleObject};

use crate::CapabilityId;

use super::{errors::graphql_error, schema::GraphqlState};

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebToolSettings")]
pub struct GraphqlWebToolSettings {
    pub search: GraphqlWebToolBindingSettings,
    pub fetch: GraphqlWebToolBindingSettings,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebToolBindingSettings")]
pub struct GraphqlWebToolBindingSettings {
    pub tool_name: String,
    pub capability_id: String,
    pub active_provider_account_id: String,
    pub provider_options: Vec<GraphqlWebToolProviderOption>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebToolProviderOption")]
pub struct GraphqlWebToolProviderOption {
    pub provider_account_id: String,
    pub provider_kind: String,
    pub account_key: String,
    pub display_name: String,
    pub capability_id: String,
    pub reliability_contract: String,
    pub data_flow_class: String,
    pub citations: bool,
    pub direct_url_fetch: bool,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveWebToolProviderBindingInput")]
pub struct GraphqlSaveWebToolProviderBindingInput {
    pub tool_name: String,
    pub capability_id: String,
    pub provider_account_id: String,
}

pub(super) async fn web_tool_settings(state: &GraphqlState) -> Result<GraphqlWebToolSettings> {
    let store = state.store()?;
    let mut accounts = store.system_provider_accounts();
    accounts.extend(
        store
            .active_default_provider_accounts()
            .await
            .map_err(graphql_error)?,
    );

    Ok(GraphqlWebToolSettings {
        search: binding_settings(store, &accounts, "web.search", CapabilityId::WebSearch).await?,
        fetch: binding_settings(store, &accounts, "web.fetch", CapabilityId::WebFetch).await?,
    })
}

pub(super) async fn save_web_tool_provider_binding(
    state: &GraphqlState,
    input: GraphqlSaveWebToolProviderBindingInput,
) -> Result<GraphqlWebToolBindingSettings> {
    let store = state.store()?;
    let capability_id = parse_web_capability(&input.capability_id)?;
    let expected_tool = match capability_id {
        CapabilityId::WebSearch => "web.search",
        CapabilityId::WebFetch => "web.fetch",
        CapabilityId::ModelGenerate | CapabilityId::ModelClassify => {
            return Err(async_graphql::Error::new("capability is not a web tool capability"));
        }
    };
    if input.tool_name != expected_tool {
        return Err(async_graphql::Error::new("tool and capability do not match"));
    }

    let mut accounts = store.system_provider_accounts();
    accounts.extend(
        store
            .active_default_provider_accounts()
            .await
            .map_err(graphql_error)?,
    );
    let selectable = web_provider_options(&accounts, capability_id);
    if !selectable
        .iter()
        .any(|option| option.provider_account_id == input.provider_account_id)
    {
        return Err(async_graphql::Error::new(
            "provider account does not supply the requested capability",
        ));
    }

    store
        .upsert_provider_capability_binding(
            &input.tool_name,
            &input.capability_id,
            &input.provider_account_id,
        )
        .await
        .map_err(graphql_error)?;

    binding_settings(store, &accounts, expected_tool, capability_id).await
}

async fn binding_settings(
    store: &crate::NoemaStore,
    accounts: &[crate::ProviderAccountRecord],
    tool_name: &str,
    capability_id: CapabilityId,
) -> Result<GraphqlWebToolBindingSettings> {
    let fallback_account_id = match capability_id {
        CapabilityId::WebSearch => "provider_account:duckduckgo_public:system",
        CapabilityId::WebFetch => "provider_account:direct_http:system",
        CapabilityId::ModelGenerate | CapabilityId::ModelClassify => {
            return Err(async_graphql::Error::new("unsupported web capability"));
        }
    };
    let active_provider_account_id = store
        .provider_capability_binding(tool_name, capability_id.as_str())
        .await
        .map_err(graphql_error)?
        .map(|binding| binding.provider_account_id)
        .unwrap_or_else(|| fallback_account_id.to_string());

    Ok(GraphqlWebToolBindingSettings {
        tool_name: tool_name.to_string(),
        capability_id: capability_id.as_str().to_string(),
        active_provider_account_id,
        provider_options: web_provider_options(accounts, capability_id),
    })
}

fn web_provider_options(
    accounts: &[crate::ProviderAccountRecord],
    capability_id: CapabilityId,
) -> Vec<GraphqlWebToolProviderOption> {
    accounts
        .iter()
        .flat_map(|account| {
            account.capabilities.iter().filter_map(move |capability| {
                (capability.capability_id == capability_id).then(|| GraphqlWebToolProviderOption {
                    provider_account_id: account.provider_account_id.clone(),
                    provider_kind: account.provider_kind.clone(),
                    account_key: account.account_key.clone(),
                    display_name: account.display_name.clone(),
                    capability_id: capability.capability_id.as_str().to_string(),
                    reliability_contract: capability.reliability_contract.as_str().to_string(),
                    data_flow_class: capability.data_flow_class.as_str().to_string(),
                    citations: capability.features.citations,
                    direct_url_fetch: capability.features.direct_url_fetch,
                })
            })
        })
        .collect()
}

fn parse_web_capability(value: &str) -> Result<CapabilityId> {
    match value {
        "web.search" => Ok(CapabilityId::WebSearch),
        "web.fetch" => Ok(CapabilityId::WebFetch),
        _ => Err(async_graphql::Error::new("unsupported web capability")),
    }
}
```

Wire `crates/noema-core/src/graphql/schema.rs`:

```rust
web_tool_settings::{
    self, GraphqlSaveWebToolProviderBindingInput, GraphqlWebToolBindingSettings,
    GraphqlWebToolProviderOption, GraphqlWebToolSettings,
},
```

Add query:

```rust
async fn web_tool_settings(&self, ctx: &Context<'_>) -> Result<GraphqlWebToolSettings> {
    web_tool_settings::web_tool_settings(ctx.data_unchecked::<GraphqlState>()).await
}
```

Add mutation:

```rust
async fn save_web_tool_provider_binding(
    &self,
    ctx: &Context<'_>,
    input: GraphqlSaveWebToolProviderBindingInput,
) -> Result<GraphqlWebToolBindingSettings> {
    web_tool_settings::save_web_tool_provider_binding(
        ctx.data_unchecked::<GraphqlState>(),
        input,
    )
    .await
}
```

- [ ] **Step 4: Run task tests**

Run:

```bash
cargo test -p noema-core graphql::web_tool_settings --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/graphql/web_tool_settings.rs crates/noema-core/src/graphql/schema.rs
git commit -m "feat: expose web provider bindings in GraphQL"
```

---

### Task 7: OpenAI Hosted Web Search Adapter

**Files:**
- Create: `crates/noema-core/src/search/openai_hosted.rs`
- Modify: `crates/noema-core/src/search.rs`
- Modify: `crates/noema-core/src/search/types.rs`
- Modify: `crates/noema-core/src/provider/adapters/responses.rs`
- Test: `crates/noema-core/src/search/openai_hosted.rs`

**Interfaces:**
- Consumes: OpenAI API credentials already owned by `OpenAiProviderConfig`.
- Produces: `OpenAiHostedSearchProvider`.
- Produces: normalized `SearchResponse` with `provider = "openai"` and `provider_contract = "hosted_search"`.
- Does not produce: `web.fetch` support.

- [ ] **Step 1: Write parser/normalization tests**

Create `crates/noema-core/src/search/openai_hosted.rs` with parser tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_openai_hosted_search_response_with_citations() {
        let value = json!({
            "id": "resp_1",
            "output": [
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Rust current docs mention ownership.",
                            "annotations": [
                                {
                                    "type": "url_citation",
                                    "url": "https://www.rust-lang.org/learn",
                                    "title": "Learn Rust"
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let response = normalize_openai_hosted_search_response("rust learn", &value)
            .expect("normalized");

        assert_eq!(response.provider, "openai");
        assert_eq!(response.provider_contract, "hosted_search");
        assert_eq!(response.query, "rust learn");
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].url, "https://www.rust-lang.org/learn");
        assert_eq!(response.results[0].title, "Learn Rust");
        assert_eq!(response.summary, "OpenAI hosted web search returned 1 citation");
    }
}
```

- [ ] **Step 2: Run parser test to verify it fails**

Run:

```bash
cargo test -p noema-core search::openai_hosted --no-fail-fast
```

Expected: FAIL because the module is not implemented.

- [ ] **Step 3: Implement OpenAI hosted response normalization**

Create `crates/noema-core/src/search/openai_hosted.rs`:

```rust
use crate::search::types::{SearchError, SearchResponse, SearchResult};
use serde_json::Value;

pub const OPENAI_HOSTED_SEARCH_PROVIDER_ID: &str = "openai";
pub const OPENAI_HOSTED_SEARCH_CONTRACT: &str = "hosted_search";

pub fn normalize_openai_hosted_search_response(
    query: &str,
    value: &Value,
) -> Result<SearchResponse, SearchError> {
    let mut results = Vec::new();
    for annotation in value
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(output_content)
        .flat_map(output_text_annotations)
    {
        if annotation.get("type").and_then(Value::as_str) != Some("url_citation") {
            continue;
        }
        let Some(url) = annotation.get("url").and_then(Value::as_str) else {
            continue;
        };
        let title = annotation
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or(url)
            .to_string();
        if results.iter().any(|existing: &SearchResult| existing.url == url) {
            continue;
        }
        results.push(SearchResult {
            rank: results.len() + 1,
            title,
            url: url.to_string(),
            snippet: String::new(),
        });
    }

    let summary = match results.len() {
        0 => "OpenAI hosted web search returned no citations".to_string(),
        1 => "OpenAI hosted web search returned 1 citation".to_string(),
        count => format!("OpenAI hosted web search returned {count} citations"),
    };

    Ok(SearchResponse {
        provider: OPENAI_HOSTED_SEARCH_PROVIDER_ID.to_string(),
        provider_contract: OPENAI_HOSTED_SEARCH_CONTRACT.to_string(),
        query: query.to_string(),
        results,
        summary,
    })
}

fn output_content(output: &Value) -> Vec<&Value> {
    output
        .get("content")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn output_text_annotations(content: &Value) -> Vec<&Value> {
    content
        .get("annotations")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}
```

Modify `crates/noema-core/src/search.rs`:

```rust
pub mod openai_hosted;
```

- [ ] **Step 4: Add request support**

Extend `crates/noema-core/src/provider/adapters/responses.rs` with a hosted web-search request helper instead of changing the main `GenerateRequest` path:

```rust
#[derive(Debug, Serialize)]
pub(crate) struct HostedWebSearchRequest {
    pub model: String,
    pub input: String,
    pub tools: Vec<HostedWebSearchTool>,
    pub store: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct HostedWebSearchTool {
    #[serde(rename = "type")]
    pub kind: &'static str,
}

impl HostedWebSearchRequest {
    #[must_use]
    pub(crate) fn new(model: String, query: &str) -> Self {
        Self {
            model,
            input: format!("Search the web for this query and return cited sources: {query}"),
            tools: vec![HostedWebSearchTool { kind: "web_search" }],
            store: false,
        }
    }
}
```

Add a serialization test:

```rust
#[test]
fn hosted_web_search_request_serializes_openai_tool() {
    let value = serde_json::to_value(HostedWebSearchRequest::new(
        "gpt-test".to_string(),
        "rust learn",
    ))
    .expect("json");

    assert_eq!(value["tools"][0]["type"], "web_search");
    assert_eq!(value["store"], false);
}
```

- [ ] **Step 5: Run task tests**

Run:

```bash
cargo test -p noema-core search::openai_hosted provider::adapters::responses::tests::hosted_web_search_request_serializes_openai_tool --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/search.rs crates/noema-core/src/search/openai_hosted.rs crates/noema-core/src/provider/adapters/responses.rs
git commit -m "feat: normalize OpenAI hosted web search"
```

---

### Task 8: Execute Bound OpenAI Search And Preserve Default Fetch

**Files:**
- Modify: `crates/noema-core/src/search/types.rs`
- Modify: `crates/noema-core/src/search/openai_hosted.rs`
- Modify: `crates/noema-core/src/daemon/runtime/web_tools.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Test: `crates/noema-core/src/daemon/runtime/web_tools.rs`

**Interfaces:**
- Consumes: `HostedWebSearchRequest` and normalization from Task 7.
- Produces: executable OpenAI `web.search` backend when binding selects an OpenAI account.
- Preserves: `web.fetch` can only resolve to `direct_http` in this plan.

- [ ] **Step 1: Write failing resolver test for rejecting non-fetch providers**

Add to `crates/noema-core/src/daemon/runtime/web_tools.rs` tests:

```rust
#[tokio::test]
async fn fetch_binding_rejects_openai_account_without_fetch_capability() {
    let store = test_store().await;
    store
        .upsert_provider_capability_binding(
            "web.fetch",
            "web.fetch",
            "provider_account:openai:default",
        )
        .await
        .expect("save invalid binding");

    let resolved = resolve_web_fetch_provider(&store).await.expect("fallback");

    assert_eq!(resolved.provider_kind, "direct_http");
    assert_eq!(resolved.fallback_from.as_deref(), Some("provider_account:openai:default"));
    assert_eq!(
        resolved.fallback_reason.as_deref(),
        Some("provider account does not supply web.fetch")
    );
}
```

- [ ] **Step 2: Run resolver test to verify it fails**

Run:

```bash
cargo test -p noema-core fetch_binding_rejects_openai_account_without_fetch_capability --no-fail-fast
```

Expected: FAIL because `resolve_web_fetch_provider` currently trusts bindings.

- [ ] **Step 3: Validate provider capability before resolving binding**

Update `resolve_web_fetch_provider`:

```rust
if let Some(binding) = store
    .provider_capability_binding("web.fetch", "web.fetch")
    .await?
{
    let supplied = store
        .system_provider_accounts()
        .into_iter()
        .chain(store.active_default_provider_accounts().await?.into_iter())
        .find(|account| account.provider_account_id == binding.provider_account_id)
        .is_some_and(|account| {
            account.capabilities.iter().any(|capability| {
                capability.capability_id.as_str() == "web.fetch"
                    && capability.status.as_str() == "available"
            })
        });
    if supplied {
        return Ok(ResolvedWebFetchProvider {
            provider_account_id: binding.provider_account_id,
            provider_kind: "direct_http".to_string(),
            account_key: "system".to_string(),
            fallback_from: None,
            fallback_reason: None,
        });
    }
    return Ok(ResolvedWebFetchProvider {
        provider_account_id: "provider_account:direct_http:system".to_string(),
        provider_kind: "direct_http".to_string(),
        account_key: "system".to_string(),
        fallback_from: Some(binding.provider_account_id),
        fallback_reason: Some("provider account does not supply web.fetch".to_string()),
    });
}
```

Apply the same capability validation pattern in `resolve_web_search_provider`, falling back to DuckDuckGo with reason `"provider account does not supply web.search"` when a stale or unsupported binding exists.

- [ ] **Step 4: Add executable OpenAI search provider branch**

Extend `SearchRuntimeProvider` in `crates/noema-core/src/search/types.rs`:

```rust
OpenAiHosted {
    client: crate::search::openai_hosted::OpenAiHostedSearchClient,
},
```

Add client type in `crates/noema-core/src/search/openai_hosted.rs`:

```rust
#[derive(Debug, Clone)]
pub struct OpenAiHostedSearchClient {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub http: reqwest::Client,
}
```

Add execution:

```rust
pub async fn search_openai_hosted(
    client: &OpenAiHostedSearchClient,
    request: &crate::search::types::SearchRequest,
) -> Result<crate::search::types::SearchResponse, crate::search::types::SearchError> {
    let body = crate::provider::adapters::responses::HostedWebSearchRequest::new(
        client.model.clone(),
        &request.query,
    );
    let response = client
        .http
        .post(format!("{}/responses", client.base_url.trim_end_matches('/')))
        .bearer_auth(&client.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                crate::search::types::SearchError::Timeout
            } else {
                crate::search::types::SearchError::Http
            }
        })?;
    if !response.status().is_success() {
        return Err(crate::search::types::SearchError::Http);
    }
    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|_| crate::search::types::SearchError::Parse)?;
    normalize_openai_hosted_search_response(&request.query, &value)
}
```

Wire the enum branch:

```rust
Self::OpenAiHosted { client } => {
    crate::search::openai_hosted::search_openai_hosted(client, request).await
}
```

- [ ] **Step 5: Route local tool execution through resolved search provider**

In `crates/noema-core/src/daemon/runtime/local_tools.rs`, before executing search:

```rust
let resolved = self.resolved_web_search_provider().await;
```

Use `self.search_provider` for `duckduckgo_public`. For `openai`, construct `SearchRuntimeProvider::OpenAiHosted` from the selected OpenAI account only when credential loading is already available in the provider auth code. If credential loading is not reusable without duplicating secret handling, return a structured failed `WebSearchToolResult` with:

```rust
json!({
    "error": "OpenAI hosted web search provider is not available in this daemon"
})
```

This keeps the task shippable without unsafe credential shortcuts. A follow-up may factor common credential loading if needed.

- [ ] **Step 6: Run task tests**

Run:

```bash
cargo test -p noema-core daemon::runtime::web_tools search::openai_hosted --no-fail-fast
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/search/types.rs crates/noema-core/src/search/openai_hosted.rs crates/noema-core/src/daemon/runtime/web_tools.rs crates/noema-core/src/daemon/runtime/local_tools.rs
git commit -m "feat: resolve capability-backed web search"
```

---

### Task 9: Web Settings UI For Provider Bindings

**Files:**
- Modify: `crates/noema-core/web/src/graphql/settings.graphql`
- Modify: `crates/noema-core/web/src/components/settings/WebSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/WebSettingsPaneContent.tsx`
- Generated: `crates/noema-core/web/src/generated/graphql.ts`
- Generated: `crates/noema-core/web/src/generated/schema.graphql`

**Interfaces:**
- Consumes: `webToolSettings` query and `saveWebToolProviderBinding` mutation from Task 6.
- Produces: visible active provider labels for Search and Fetch.
- Produces: provider picker controls listing only valid providers for each capability.

- [ ] **Step 1: Add GraphQL operations**

Modify `crates/noema-core/web/src/graphql/settings.graphql`:

```graphql
query WebToolSettings {
  webToolSettings {
    search {
      toolName
      capabilityId
      activeProviderAccountId
      providerOptions {
        providerAccountId
        providerKind
        accountKey
        displayName
        capabilityId
        reliabilityContract
        dataFlowClass
        citations
        directUrlFetch
      }
    }
    fetch {
      toolName
      capabilityId
      activeProviderAccountId
      providerOptions {
        providerAccountId
        providerKind
        accountKey
        displayName
        capabilityId
        reliabilityContract
        dataFlowClass
        citations
        directUrlFetch
      }
    }
  }
}

mutation SaveWebToolProviderBinding($input: SaveWebToolProviderBindingInput!) {
  saveWebToolProviderBinding(input: $input) {
    toolName
    capabilityId
    activeProviderAccountId
    providerOptions {
      providerAccountId
      providerKind
      accountKey
      displayName
      capabilityId
      reliabilityContract
      dataFlowClass
      citations
      directUrlFetch
    }
  }
}
```

- [ ] **Step 2: Generate frontend types**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: generated GraphQL files update successfully.

- [ ] **Step 3: Wire WebSettingsPane data**

Modify `crates/noema-core/web/src/components/settings/WebSettingsPane.tsx` to query both existing fetch settings and new web tool settings:

```tsx
const webToolResult = useQuery<WebToolSettingsQuery>(WebToolSettingsDocument, {
  fetchPolicy: "cache-and-network"
});

const [saveWebToolProviderBinding, saveWebToolResult] = useMutation<
  SaveWebToolProviderBindingMutation,
  SaveWebToolProviderBindingMutationVariables
>(SaveWebToolProviderBindingDocument, {
  refetchQueries: [{ query: WebToolSettingsDocument }],
  awaitRefetchQueries: true
});
```

Pass:

```tsx
webToolSettings={webToolResult.data?.webToolSettings ?? null}
webToolSettingsError={webToolResult.error?.message ?? null}
onSaveWebToolProviderBinding={(input) =>
  saveWebToolProviderBinding({ variables: { input } })
}
```

- [ ] **Step 4: Add provider binding controls**

Modify `crates/noema-core/web/src/components/settings/WebSettingsPaneContent.tsx`:

```tsx
type WebToolProviderOption = {
  providerAccountId: string;
  providerKind: string;
  displayName: string;
  reliabilityContract: string;
  citations: boolean;
  directUrlFetch: boolean;
};

type WebToolBindingSettings = {
  toolName: string;
  capabilityId: string;
  activeProviderAccountId: string;
  providerOptions: WebToolProviderOption[];
};
```

Render active provider metadata in the existing Search and Fetch sections:

```tsx
<MetadataRow
  label="Provider"
  value={activeProviderLabel(webToolSettings?.search ?? null)}
/>
<MetadataRow
  label="Contract"
  value={activeProviderContract(webToolSettings?.search ?? null)}
/>
```

Use a native `<select>` if the current component set has no established select control in this file:

```tsx
function WebProviderSelect({
  settings,
  onSave
}: {
  settings: WebToolBindingSettings | null;
  onSave: (input: {
    toolName: string;
    capabilityId: string;
    providerAccountId: string;
  }) => Promise<unknown>;
}) {
  if (!settings) {
    return null;
  }
  return (
    <select
      value={settings.activeProviderAccountId}
      onChange={(event) =>
        void onSave({
          toolName: settings.toolName,
          capabilityId: settings.capabilityId,
          providerAccountId: event.currentTarget.value
        })
      }
    >
      {settings.providerOptions.map((option) => (
        <option key={option.providerAccountId} value={option.providerAccountId}>
          {option.displayName}
        </option>
      ))}
    </select>
  );
}
```

Keep Firecrawl absent because GraphQL should not return it.

- [ ] **Step 5: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run typecheck
bun run lint
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/web/src/graphql/settings.graphql crates/noema-core/web/src/components/settings/WebSettingsPane.tsx crates/noema-core/web/src/components/settings/WebSettingsPaneContent.tsx crates/noema-core/web/src/generated/graphql.ts crates/noema-core/web/src/generated/schema.graphql
git commit -m "feat: show web provider bindings in settings"
```

---

### Task 10: Transcript Metadata And Final Validation

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Modify: `docs/context/current.md`
- Test: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`

**Interfaces:**
- Consumes: provider contract and fallback metadata from web tool results.
- Produces: safe transcript displays that show provider and fallback without raw provider payloads.

- [ ] **Step 1: Write failing transcript display test**

Add to `crates/noema-core/src/daemon/runtime/transcript_persistence.rs` tests:

```rust
#[test]
fn web_search_display_shows_provider_fallback_without_raw_payload() {
    let display = tool_result_display(
        Some("web.search"),
        &serde_json::json!({
            "provider": "duckduckgo_public",
            "provider_contract": "best_effort_public",
            "fallback_from": "openai",
            "fallback_reason": "provider account unauthenticated",
            "query": "rust learn",
            "results": [],
            "summary": "No web results found",
            "raw_provider_payload": "secret"
        }),
        true,
    );

    assert_eq!(display["provider"], "DuckDuckGo public search");
    assert_eq!(display["fallbackFrom"], "openai");
    assert_eq!(display["fallbackReason"], "provider account unauthenticated");
    assert!(!display.to_string().contains("secret"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p noema-core web_search_display_shows_provider_fallback_without_raw_payload --no-fail-fast
```

Expected: FAIL because fallback display fields are not mapped.

- [ ] **Step 3: Add safe display metadata**

In `tool_result_display`, when the tool name is `web.search` or `web.fetch`, map optional fallback fields:

```rust
if let Some(fallback_from) = payload.get("fallback_from").and_then(Value::as_str) {
    display.insert("fallbackFrom".to_string(), Value::String(fallback_from.to_string()));
}
if let Some(fallback_reason) = payload.get("fallback_reason").and_then(Value::as_str) {
    display.insert(
        "fallbackReason".to_string(),
        Value::String(fallback_reason.to_string()),
    );
}
```

Do not include raw provider payloads, headers, credentials, or full page bodies in display metadata.

- [ ] **Step 4: Update durable context**

Add a short note to `docs/context/current.md` after the provider capabilities bullet:

```markdown
- The implementation plan for provider capabilities is
  `docs/superpowers/plans/2026-07-07-provider-capabilities.md`. It keeps
  Firecrawl out of scope, preserves provider-neutral web tool schemas, and
  treats OpenAI hosted web search as `web.search` only.
```

- [ ] **Step 5: Run full relevant validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cd crates/noema-core/web
bun run typecheck
bun run lint
```

Expected: PASS.

- [ ] **Step 6: Inspect and commit final task**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Stage and commit:

```bash
git add crates/noema-core/src/daemon/runtime/transcript_persistence.rs docs/context/current.md
git commit -m "feat: surface web provider fallback metadata"
```

---

## Self-Review Checklist

- Spec coverage: Tasks 1-4 cover vocabulary, accounts, capabilities, and bindings. Tasks 5-8 cover runtime selection and OpenAI hosted search as `web.search`. Task 9 covers Settings ownership. Task 10 covers audit/transcript metadata and durable context.
- Firecrawl scope: No task creates Firecrawl provider code, schema rows, settings options, tests, or docs beyond naming it as excluded in the spec.
- Stable tool contracts: Task 4 includes an explicit regression test that `web.search` and `web.fetch` do not gain a `provider` argument.
- Codex boundary: No task wraps Codex CLI/app web search.
- Fetch boundary: No task adds hosted `web.fetch`; `web.fetch` falls back to direct HTTP when unsupported bindings appear.
- Frontend tests: No UI tests are required. Frontend validation is typecheck and lint only.
