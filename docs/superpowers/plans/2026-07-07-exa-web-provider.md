# Exa Web Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Exa as a user-created provider account that can supply Noema's existing `web.search` and `web.fetch` tools.

**Architecture:** Keep model-visible tools provider-neutral and route them through the existing provider capability binding layer. Add a provider account catalog and generic account-creation path, store Exa API keys as write-only provider-account secrets under `${NOEMA_HOME}/providers/exa/<account_key>/`, and add Exa search/fetch runtime adapters selected by capability bindings.

**Tech Stack:** Rust, SurrealDB schema strings, async-graphql, reqwest, serde/serde_json, existing Noema provider account homes, React, Apollo, Astryx/StyleX, Bun GraphQL typegen.

## Global Constraints

- No Exa account is listed until the user creates one through Settings.
- Do not read Exa credentials from environment variables.
- Store Exa API keys only under the provider account home, never in SurrealDB.
- Treat Exa keys as write-only: never return them through GraphQL, UI state, logs, transcript metadata, provider metadata, or snapshots.
- Keep model-visible tool names and arguments unchanged: no provider argument on `web.search` or `web.fetch`.
- Exa crawl, research, answer, Websets, monitors, MCP support, and generalized multi-field credentials are out of scope.
- Pre-V1 schema changes may rewrite schema strings directly; do not add migrations or compatibility layers.
- Preserve unrelated dirty worktree changes.
- Run unit tests only. Do not run smoke tests or fixture tests unless explicitly requested.
- Do not inspect frontend with browser tools unless explicitly requested.
- Do not modify `CARGO_BUILD_RUSTC_WRAPPER` or otherwise interfere with `sccache`.

---

## File Structure

- Modify `crates/noema-core/src/provider/capabilities.rs`
  - Declare Exa `web.search` and `web.fetch` capabilities.
- Create `crates/noema-core/src/provider/secret_input.rs`
  - Narrow write-only API-key file storage for `secret_input` provider accounts.
- Modify `crates/noema-core/src/provider.rs`
  - Export `secret_input`.
- Modify `crates/noema-core/src/store/schema.rs`
  - Add `exa` to allowed `provider_accounts.provider_kind` values.
- Modify `crates/noema-core/src/store/provider_accounts.rs`
  - Add provider account catalog, all-active account listing, generated account-key creation, and safe status updates.
- Modify `crates/noema-core/src/store.rs`
  - Export new provider catalog/account creation types.
- Modify `crates/noema-core/src/graphql/provider_accounts.rs`
  - Expose provider account catalog and mutations for create/save-secret/clear-secret.
- Modify `crates/noema-core/src/graphql/schema.rs`
  - Add GraphQL query/mutation fields and integration tests.
- Modify `crates/noema-core/src/graphql/web_tool_settings.rs`
  - Build web provider options from system accounts plus all active created accounts.
- Modify `crates/noema-core/web/src/graphql/operations.ts`
  - Query provider account catalog and add provider secret mutations.
- Modify `crates/noema-core/web/src/components/settings/ProvidersSettingsPane.tsx`
  - Wire provider catalog and mutations into Providers Settings.
- Modify `crates/noema-core/web/src/components/settings/ProvidersSettingsPaneContent.tsx`
  - Add account creation and write-only secret replacement/clear controls.
- Modify `crates/noema-core/web/src/components/settings/providerMetadata.ts`
  - Add provider/account display helpers if needed.
- Create `crates/noema-core/src/search/exa.rs`
  - Exa `/search` request/response adapter.
- Modify `crates/noema-core/src/search.rs`
  - Export the Exa search module.
- Modify `crates/noema-core/src/search/types.rs`
  - Add `SearchRuntimeProvider::Exa`.
- Create `crates/noema-core/src/web_fetch/exa.rs`
  - Exa `/contents` request/response adapter.
- Modify `crates/noema-core/src/web_fetch/mod.rs`
  - Export the Exa fetch module.
- Modify `crates/noema-core/src/web_fetch/types.rs`
  - Add `WebFetchRuntimeProvider::Exa`.
- Modify `crates/noema-core/src/daemon/runtime/web_tools.rs`
  - Keep resolving only created Exa accounts and fallback safely when unavailable.
- Modify `crates/noema-core/src/daemon/runtime/local_tools.rs`
  - Load Exa credentials, construct Exa runtime providers, and update account status on auth failures.
- Regenerate `crates/noema-core/web/src/generated/schema.graphql` and `crates/noema-core/web/src/generated/graphql.ts` with `bun run gen:types`.

---

### Task 1: Provider Catalog And Exa Account Creation

**Files:**
- Modify: `crates/noema-core/src/provider/capabilities.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/provider_accounts.rs`
- Modify: `crates/noema-core/src/store.rs`
- Test: `crates/noema-core/src/provider/capabilities.rs`
- Test: `crates/noema-core/src/store/tests.rs`

**Interfaces:**
- Produces: `ProviderAccountCatalogEntry { provider_kind, display_name, auth_method, capabilities }`.
- Produces: `NewProviderAccount { provider_kind, display_name, auth_method, status, metadata }`.
- Produces: `NoemaStore::provider_account_catalog(&self) -> Vec<ProviderAccountCatalogEntry>`.
- Produces: `NoemaStore::create_provider_account(&self, input: NewProviderAccount) -> Result<ProviderAccountRecord, StoreError>`.
- Produces: `NoemaStore::active_provider_accounts(&self) -> Result<Vec<ProviderAccountRecord>, StoreError>`.
- Consumes: existing `ProviderAccountRecord`, `ProviderAccountStatus`, `ProviderAuthMethod`, and `capabilities_for_provider_account`.

- [ ] **Step 1: Write failing capability and catalog tests**

Add this test to `crates/noema-core/src/provider/capabilities.rs`:

```rust
#[test]
fn exa_account_declares_search_and_fetch_only() {
    let capabilities =
        capabilities_for_provider_account("exa", "research", ProviderAccountStatus::Authenticated);
    let ids = capabilities
        .iter()
        .map(|capability| capability.capability_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, vec!["web.search", "web.fetch"]);
    assert!(capabilities.iter().all(|capability| {
        capability.provider_kind == "exa"
            && capability.account_key == "research"
            && capability.reliability_contract == ReliabilityContract::HostedProvider
    }));
    assert!(capabilities.iter().any(|capability| {
        capability.capability_id == CapabilityId::WebSearch
            && capability.data_flow_class == DataFlowClass::TrustedExternalSearchQuery
            && capability.features.citations
            && !capability.features.direct_url_fetch
    }));
    assert!(capabilities.iter().any(|capability| {
        capability.capability_id == CapabilityId::WebFetch
            && capability.data_flow_class == DataFlowClass::ExternalWebFetch
            && capability.features.direct_url_fetch
    }));
}
```

Add these tests to `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn provider_account_catalog_lists_exa_without_creating_account() {
    let store = test_store().await;

    let catalog = store.provider_account_catalog();
    let exa = catalog
        .iter()
        .find(|entry| entry.provider_kind == "exa")
        .expect("exa catalog entry");
    assert_eq!(exa.display_name, "Exa");
    assert_eq!(exa.auth_method, crate::ProviderAuthMethod::SecretInput);
    assert!(exa.capabilities.iter().any(|capability| {
        capability.capability_id == crate::provider::CapabilityId::WebSearch
    }));
    assert!(store.active_provider_account("exa").await.expect("read").is_none());
}

#[tokio::test]
async fn create_provider_account_generates_stable_key_and_capabilities() {
    let store = test_store().await;

    let account = store
        .create_provider_account(crate::store::NewProviderAccount {
            provider_kind: "exa".to_string(),
            display_name: Some("Research Search".to_string()),
            auth_method: crate::ProviderAuthMethod::SecretInput,
            status: crate::ProviderAccountStatus::Authenticated,
            metadata: serde_json::json!({ "secretConfigured": true }),
        })
        .await
        .expect("created account");

    assert_eq!(account.provider_kind, "exa");
    assert_eq!(account.display_name, "Research Search");
    assert!(account.account_key.starts_with("acct_"));
    assert_eq!(
        account.provider_account_id,
        format!("provider_account:exa:{}", account.account_key)
    );
    assert!(!account.is_default);
    assert!(account.capabilities.iter().any(|capability| {
        capability.capability_id == crate::provider::CapabilityId::WebFetch
    }));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p noema-core exa_account_declares_search_and_fetch_only provider_account_catalog_lists_exa_without_creating_account create_provider_account_generates_stable_key_and_capabilities --no-fail-fast
```

Expected: FAIL because Exa capabilities and store creation APIs do not exist.

- [ ] **Step 3: Add Exa capabilities**

In `capabilities_for_provider_account`, add this `match` arm before `_ => Vec::new()`:

```rust
"exa" => vec![
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
    ProviderCapability {
        provider_kind: provider_kind.to_string(),
        account_key: account_key.to_string(),
        capability_id: CapabilityId::WebFetch,
        status,
        reliability_contract: ReliabilityContract::HostedProvider,
        data_flow_class: DataFlowClass::ExternalWebFetch,
        features: CapabilityFeatures {
            citations: true,
            direct_url_fetch: true,
            js_rendering: false,
            authenticated_context: false,
            result_persistence: ResultPersistencePolicy::CompactContent,
        },
    },
],
```

- [ ] **Step 4: Add store types and account creation**

In `crates/noema-core/src/store/schema.rs`, update the provider kind assertion:

```sql
DEFINE FIELD OVERWRITE provider_kind ON TABLE provider_accounts TYPE string ASSERT $value INSIDE ['codex', 'openai', 'foundation_local', 'exa'];
```

In `crates/noema-core/src/store/provider_accounts.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderAccountCatalogEntry {
    pub provider_kind: String,
    pub display_name: String,
    pub auth_method: ProviderAuthMethod,
    pub capabilities: Vec<crate::provider::ProviderCapability>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewProviderAccount {
    pub provider_kind: String,
    pub display_name: Option<String>,
    pub auth_method: ProviderAuthMethod,
    pub status: ProviderAccountStatus,
    pub metadata: Value,
}
```

Add methods:

```rust
use crate::store::ids::allocate_id;

#[must_use]
pub fn provider_account_catalog(&self) -> Vec<ProviderAccountCatalogEntry> {
    vec![ProviderAccountCatalogEntry {
        provider_kind: "exa".to_string(),
        display_name: "Exa".to_string(),
        auth_method: ProviderAuthMethod::SecretInput,
        capabilities: capabilities_for_provider_account(
            "exa",
            "catalog",
            ProviderAccountStatus::Authenticated,
        ),
    }]
}

pub async fn active_provider_accounts(&self) -> Result<Vec<ProviderAccountRecord>, StoreError> {
    let mut response = self.db.query(
        r#"
        SELECT provider_account_id, provider_kind, account_key, display_name,
          auth_method, is_active, is_default, status, last_checked_at,
          last_authenticated_at, last_error_code, last_error_message, metadata
        FROM provider_accounts
        WHERE is_active = true;
        "#,
    ).await?;
    let rows: Vec<ProviderAccountRow> = response.take(0)?;
    let mut accounts = rows
        .into_iter()
        .map(provider_account_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    accounts.sort_by(|left, right| {
        left.provider_kind
            .cmp(&right.provider_kind)
            .then(left.display_name.cmp(&right.display_name))
            .then(left.account_key.cmp(&right.account_key))
    });
    Ok(accounts)
}

pub async fn create_provider_account(
    &self,
    input: NewProviderAccount,
) -> Result<ProviderAccountRecord, StoreError> {
    if input.provider_kind != "exa" {
        return Err(StoreError::InvalidEnum {
            kind: "provider_kind",
            value: input.provider_kind,
        });
    }
    if input.auth_method != ProviderAuthMethod::SecretInput {
        return Err(StoreError::InvalidEnum {
            kind: "provider_auth_method",
            value: input.auth_method.as_str().to_string(),
        });
    }

    let account_key = generated_account_key(&input.provider_kind);
    let provider_account_id = format!("provider_account:{}:{account_key}", input.provider_kind);
    let record_id = format!("{}_{}", input.provider_kind, account_key);
    let display_name = input
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Exa")
        .to_string();

    self.db.query(
        r#"
        CREATE type::record('provider_accounts', $record_id) SET
          provider_account_id = $provider_account_id,
          provider_kind = $provider_kind,
          account_key = $account_key,
          display_name = $display_name,
          auth_method = $auth_method,
          is_active = true,
          is_default = false,
          status = $status,
          metadata = $metadata,
          updated_at = time::now();
        "#,
    )
    .bind(("record_id", record_id))
    .bind(("provider_account_id", provider_account_id.clone()))
    .bind(("provider_kind", input.provider_kind))
    .bind(("account_key", account_key))
    .bind(("display_name", display_name))
    .bind(("auth_method", input.auth_method.as_str().to_string()))
    .bind(("status", input.status.as_str().to_string()))
    .bind(("metadata", input.metadata))
    .await?
    .check()?;

    self.get_provider_account(&provider_account_id)
        .await?
        .ok_or(StoreError::ProviderAccountNotFound { provider_account_id })
}

fn generated_account_key(provider_kind: &str) -> String {
    let allocated = allocate_id("provider_account");
    let suffix = allocated
        .rsplit(':')
        .next()
        .unwrap_or("account")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("acct_{provider_kind}_{suffix}")
}
```

- [ ] **Step 5: Export new store types**

In `crates/noema-core/src/store.rs`, update the export:

```rust
pub use provider_accounts::{NewProviderAccount, ProviderAccountCatalogEntry};
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test -p noema-core exa_account_declares_search_and_fetch_only provider_account_catalog_lists_exa_without_creating_account create_provider_account_generates_stable_key_and_capabilities --no-fail-fast
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/provider/capabilities.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/provider_accounts.rs crates/noema-core/src/store.rs crates/noema-core/src/store/tests.rs
git commit -m "feat: add Exa provider account catalog"
```

---

### Task 2: Secret Input Storage And GraphQL Mutations

**Files:**
- Create: `crates/noema-core/src/provider/secret_input.rs`
- Modify: `crates/noema-core/src/provider.rs`
- Modify: `crates/noema-core/src/graphql/provider_accounts.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Test: `crates/noema-core/src/provider/secret_input.rs`
- Test: `crates/noema-core/src/graphql/schema.rs`

**Interfaces:**
- Consumes: `NoemaPaths::provider_account_home(provider_kind, account_key)`.
- Consumes: `NoemaStore::create_provider_account`, `NoemaStore::update_provider_account_status`, `NoemaStore::update_provider_account_metadata`, `NoemaStore::get_provider_account`.
- Produces: `SecretInputStore::new(account_home: impl Into<PathBuf>)`.
- Produces: `SecretInputStore::save_api_key(&self, api_key: &str) -> Result<(), ProviderError>`.
- Produces: `SecretInputStore::load_api_key(&self) -> Result<String, ProviderError>`.
- Produces: `SecretInputStore::clear_api_key(&self) -> Result<(), ProviderError>`.
- Produces: GraphQL `providerAccountCatalog`, `createProviderAccount`, `saveProviderSecretInput`, and `clearProviderSecret`.

- [ ] **Step 1: Write failing secret store tests**

Create `crates/noema-core/src/provider/secret_input.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_and_clear_api_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SecretInputStore::new(dir.path().join("providers/exa/acct_one"));

        store.save_api_key("secret-key").expect("save");
        assert_eq!(store.load_api_key().expect("load"), "secret-key");

        store.clear_api_key().expect("clear");
        assert!(matches!(
            store.load_api_key(),
            Err(crate::ProviderError::MissingCredentials { .. })
        ));
    }

    #[test]
    fn rejects_blank_api_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SecretInputStore::new(dir.path().join("providers/exa/acct_one"));

        let error = store.save_api_key("  ").expect_err("blank rejected");

        assert!(matches!(
            error,
            crate::ProviderError::InvalidRequest { .. }
        ));
        assert!(!store.secret_path().exists());
    }
}
```

- [ ] **Step 2: Implement write-only secret file storage**

Implement `crates/noema-core/src/provider/secret_input.rs`:

```rust
use crate::{
    ProviderError,
    provider::auth::ensure_provider_account_home,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

const API_KEY_FILE_NAME: &str = "api_key.json";

#[derive(Debug, Clone)]
pub struct SecretInputStore {
    account_home: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiKeyFile {
    api_key: String,
}

impl SecretInputStore {
    pub fn new(account_home: impl Into<PathBuf>) -> Self {
        Self { account_home: account_home.into() }
    }

    #[must_use]
    pub fn secret_path(&self) -> PathBuf {
        self.account_home.join(API_KEY_FILE_NAME)
    }

    pub fn save_api_key(&self, api_key: &str) -> Result<(), ProviderError> {
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "api key is required".to_string(),
            });
        }
        ensure_provider_account_home(&self.account_home).map_err(|source| {
            ProviderError::Unavailable {
                message: format!("provider account home unavailable: {source}"),
            }
        })?;
        let payload = serde_json::to_vec(&ApiKeyFile { api_key: api_key.to_string() })
            .map_err(|source| ProviderError::InvalidRequest { message: source.to_string() })?;
        fs::write(self.secret_path(), payload).map_err(|source| ProviderError::Unavailable {
            message: format!("provider secret could not be written: {source}"),
        })
    }

    pub fn load_api_key(&self) -> Result<String, ProviderError> {
        let text = fs::read_to_string(self.secret_path()).map_err(|_| {
            ProviderError::MissingCredentials {
                provider: "exa".to_string(),
                credential: API_KEY_FILE_NAME.to_string(),
            }
        })?;
        let file: ApiKeyFile = serde_json::from_str(&text).map_err(|_| {
            ProviderError::MissingCredentials {
                provider: "exa".to_string(),
                credential: API_KEY_FILE_NAME.to_string(),
            }
        })?;
        let api_key = file.api_key.trim();
        if api_key.is_empty() {
            return Err(ProviderError::MissingCredentials {
                provider: "exa".to_string(),
                credential: API_KEY_FILE_NAME.to_string(),
            });
        }
        Ok(api_key.to_string())
    }

    pub fn clear_api_key(&self) -> Result<(), ProviderError> {
        match fs::remove_file(self.secret_path()) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(ProviderError::Unavailable {
                message: format!("provider secret could not be removed: {source}"),
            }),
        }
    }
}
```

Add to `crates/noema-core/src/provider.rs`:

```rust
/// Write-only secret-input provider account storage.
pub mod secret_input;
```

- [ ] **Step 3: Write failing GraphQL tests**

Add tests to `crates/noema-core/src/graphql/schema.rs`:

```rust
#[tokio::test]
async fn provider_account_catalog_lists_exa() {
    let store = crate::store::tests::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));

    let response = schema.execute(async_graphql::Request::new(
        r#"
        {
          providerAccountCatalog {
            providerKind
            displayName
            authMethod
            capabilities { capabilityId }
          }
          providerAccounts { providerKind }
        }
        "#,
    )).await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    assert_eq!(data["providerAccountCatalog"][0]["providerKind"], "exa");
    assert!(data["providerAccounts"].as_array().unwrap().iter().all(|account| {
        account["providerKind"] != "exa"
    }));
}

#[tokio::test]
async fn create_exa_provider_account_stores_secret_without_returning_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = crate::store::tests::test_store().await;
    let state = GraphqlState::for_tests_with_store_and_paths(
        store,
        crate::NoemaPaths::from_noema_home(dir.path()).expect("paths"),
    );
    let schema = build_schema(state);

    let response = schema.execute(async_graphql::Request::new(
        r#"
        mutation {
          createProviderAccount(input: {
            providerKind: "exa"
            displayName: "Research"
            secret: "secret-key"
          }) {
            providerKind
            accountKey
            displayName
            authMethod
            status
            lastErrorMessage
          }
        }
        "#,
    )).await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let value = response.data.into_json().expect("json");
    let account = &value["createProviderAccount"];
    assert_eq!(account["providerKind"], "exa");
    assert_eq!(account["displayName"], "Research");
    assert_eq!(account["status"], "AUTHENTICATED");
    assert!(!value.to_string().contains("secret-key"));
    let account_key = account["accountKey"].as_str().expect("account key");
    assert!(dir
        .path()
        .join(format!("providers/exa/{account_key}/api_key.json"))
        .is_file());
}
```

- [ ] **Step 4: Add GraphQL types and mutations**

In `crates/noema-core/src/graphql/provider_accounts.rs`, add input/output types:

```rust
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProviderAccount")]
pub struct GraphqlProviderAccount {
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Provider family, such as `codex`.
    pub provider_kind: String,
    /// Provider-local account key.
    pub account_key: String,
    /// Human-readable account name.
    pub display_name: String,
    /// Authentication method used for this account.
    pub auth_method: String,
    /// Last known account readiness status.
    pub status: GraphqlProviderAccountStatus,
    /// Whether the account may be selected.
    pub is_active: bool,
    /// Whether the account is the default account for its provider.
    pub is_default: bool,
    /// Last time Noema checked the account status.
    pub last_checked_at: Option<String>,
    /// Last time Noema observed successful authentication.
    pub last_authenticated_at: Option<String>,
    /// Last non-secret provider error code.
    pub last_error_code: Option<String>,
    /// Last non-secret provider error message.
    pub last_error_message: Option<String>,
    /// Provider capabilities available through this account.
    pub capabilities: Vec<GraphqlProviderCapability>,
}

impl From<ProviderAccountRecord> for GraphqlProviderAccount {
    fn from(account: ProviderAccountRecord) -> Self {
        Self {
            provider_account_id: account.provider_account_id,
            provider_kind: account.provider_kind,
            account_key: account.account_key,
            display_name: account.display_name,
            auth_method: auth_method_label(account.auth_method).to_string(),
            status: account.status.into(),
            is_active: account.is_active,
            is_default: account.is_default,
            last_checked_at: account.last_checked_at,
            last_authenticated_at: account.last_authenticated_at,
            last_error_code: account.last_error_code,
            last_error_message: account.last_error_message,
            capabilities: account.capabilities.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProviderAccountCatalogEntry")]
pub struct GraphqlProviderAccountCatalogEntry {
    pub provider_kind: String,
    pub display_name: String,
    pub auth_method: String,
    pub capabilities: Vec<GraphqlProviderCapability>,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CreateProviderAccountInput")]
pub struct GraphqlCreateProviderAccountInput {
    pub provider_kind: String,
    pub display_name: Option<String>,
    pub secret: String,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ProviderSecretInput")]
pub struct GraphqlProviderSecretInput {
    pub provider_account_id: String,
    pub secret: String,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ClearProviderSecretInput")]
pub struct GraphqlClearProviderSecretInput {
    pub provider_account_id: String,
}
```

Add functions:

```rust
pub(super) async fn provider_account_catalog(
    state: &GraphqlState,
) -> Result<Vec<GraphqlProviderAccountCatalogEntry>> {
    let store = state.store()?;
    Ok(store
        .provider_account_catalog()
        .into_iter()
        .map(|entry| GraphqlProviderAccountCatalogEntry {
            provider_kind: entry.provider_kind,
            display_name: entry.display_name,
            auth_method: entry.auth_method.as_str().to_string(),
            capabilities: entry.capabilities.into_iter().map(Into::into).collect(),
        })
        .collect())
}

pub(super) async fn create_provider_account(
    state: &GraphqlState,
    input: GraphqlCreateProviderAccountInput,
) -> Result<GraphqlProviderAccount> {
    if input.provider_kind != "exa" {
        return Err(async_graphql::Error::new("unsupported provider kind"));
    }
    let store = state.store()?;
    let paths = state.paths()?;
    let account = store
        .create_provider_account(crate::store::NewProviderAccount {
            provider_kind: input.provider_kind,
            display_name: input.display_name,
            auth_method: ProviderAuthMethod::SecretInput,
            status: ProviderAccountStatus::Authenticated,
            metadata: serde_json::json!({"secretConfigured": true}),
        })
        .await
        .map_err(graphql_error)?;
    save_secret_for_account(paths, store, &account, &input.secret).await
}
```

Implement `save_secret_for_account`, `save_provider_secret_input`, and `clear_provider_secret` so they:

```rust
let secret_store = crate::provider::secret_input::SecretInputStore::new(
    paths.provider_account_home(&account.provider_kind, &account.account_key),
);
secret_store.save_api_key(secret).map_err(|error| async_graphql::Error::new(error.to_string()))?;
store
    .update_provider_account_status(&account.provider_account_id, ProviderAccountStatus::Authenticated, None, None)
    .await
    .map_err(graphql_error)?;
store
    .update_provider_account_metadata(&account.provider_account_id, serde_json::json!({"secretConfigured": true}))
    .await
    .map_err(graphql_error)?;
```

For clear, call `clear_api_key`, then set status `Unauthenticated` and metadata `{"secretConfigured": false}`.

In `crates/noema-core/src/graphql/schema.rs`, add:

```rust
async fn provider_account_catalog(
    &self,
    ctx: &Context<'_>,
) -> Result<Vec<GraphqlProviderAccountCatalogEntry>> {
    let state = ctx.data_unchecked::<GraphqlState>();
    provider_accounts::provider_account_catalog(state).await
}

async fn create_provider_account(
    &self,
    ctx: &Context<'_>,
    input: GraphqlCreateProviderAccountInput,
) -> Result<GraphqlProviderAccount> {
    let state = ctx.data_unchecked::<GraphqlState>();
    provider_accounts::create_provider_account(state, input).await
}
```

Add matching mutation fields for `saveProviderSecretInput` and `clearProviderSecret`.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p noema-core provider::secret_input graphql::schema::provider_account_catalog_lists_exa graphql::schema::create_exa_provider_account_stores_secret_without_returning_it --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/provider/secret_input.rs crates/noema-core/src/provider.rs crates/noema-core/src/graphql/provider_accounts.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/src/graphql/runtime_state.rs
git commit -m "feat: add provider secret input flow"
```

---

### Task 3: Providers Settings Add Account UI

**Files:**
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Modify: `crates/noema-core/web/src/components/settings/ProvidersSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/ProvidersSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/components/settings/providerMetadata.ts`
- Generated: `crates/noema-core/web/src/generated/schema.graphql`
- Generated: `crates/noema-core/web/src/generated/graphql.ts`

**Interfaces:**
- Consumes: GraphQL `providerAccountCatalog`, `createProviderAccount`, `saveProviderSecretInput`, `clearProviderSecret`.
- Produces: Providers Settings UI with Add Provider Account, optional display name, required API key, and write-only secret replacement/clear controls.

- [ ] **Step 1: Add GraphQL operations**

In `crates/noema-core/web/src/graphql/operations.ts`, update `ProviderAccountsDocument`:

```ts
export const ProviderAccountsDocument = gql`
  query ProviderAccounts {
    providerAccountCatalog {
      providerKind
      displayName
      authMethod
      capabilities {
        capabilityId
        status
        reliabilityContract
        dataFlowClass
      }
    }
    providerAccounts {
      providerAccountId
      providerKind
      accountKey
      displayName
      authMethod
      status
      isActive
      isDefault
      lastCheckedAt
      lastAuthenticatedAt
      lastErrorCode
      lastErrorMessage
    }
  }
`;
```

Add mutations:

```ts
export const CreateProviderAccountDocument = gql`
  mutation CreateProviderAccount($input: CreateProviderAccountInput!) {
    createProviderAccount(input: $input) {
      providerAccountId
      providerKind
      accountKey
      displayName
      authMethod
      status
      isActive
      isDefault
      lastCheckedAt
      lastAuthenticatedAt
      lastErrorCode
      lastErrorMessage
    }
  }
`;

export const SaveProviderSecretInputDocument = gql`
  mutation SaveProviderSecretInput($input: ProviderSecretInput!) {
    saveProviderSecretInput(input: $input) {
      providerAccountId
      providerKind
      accountKey
      displayName
      authMethod
      status
      isActive
      isDefault
      lastCheckedAt
      lastAuthenticatedAt
      lastErrorCode
      lastErrorMessage
    }
  }
`;

export const ClearProviderSecretDocument = gql`
  mutation ClearProviderSecret($input: ClearProviderSecretInput!) {
    clearProviderSecret(input: $input) {
      providerAccountId
      providerKind
      accountKey
      displayName
      authMethod
      status
      isActive
      isDefault
      lastCheckedAt
      lastAuthenticatedAt
      lastErrorCode
      lastErrorMessage
    }
  }
`;
```

- [ ] **Step 2: Regenerate frontend GraphQL types**

Run from `crates/noema-core/web`:

```bash
bun run gen:types
```

Expected: generated GraphQL artifacts update without errors.

- [ ] **Step 3: Wire mutations in the pane container**

In `ProvidersSettingsPane.tsx`, use Apollo mutations:

```tsx
const [createProviderAccount, createResult] = useMutation(CreateProviderAccountDocument, {
  refetchQueries: [{ query: ProviderAccountsDocument }, { query: WebToolSettingsDocument }],
  awaitRefetchQueries: true
});
const [saveProviderSecretInput, saveSecretResult] = useMutation(SaveProviderSecretInputDocument, {
  refetchQueries: [{ query: ProviderAccountsDocument }, { query: WebToolSettingsDocument }],
  awaitRefetchQueries: true
});
const [clearProviderSecret, clearSecretResult] = useMutation(ClearProviderSecretDocument, {
  refetchQueries: [{ query: ProviderAccountsDocument }, { query: WebToolSettingsDocument }],
  awaitRefetchQueries: true
});
```

Pass:

```tsx
catalog={result.data?.providerAccountCatalog ?? []}
onCreateProviderAccount={(input) => createProviderAccount({ variables: { input } })}
onSaveProviderSecret={(input) => saveProviderSecretInput({ variables: { input } })}
onClearProviderSecret={(input) => clearProviderSecret({ variables: { input } })}
mutationError={
  createResult.error?.message ??
  saveSecretResult.error?.message ??
  clearSecretResult.error?.message ??
  null
}
mutationSaving={createResult.loading || saveSecretResult.loading || clearSecretResult.loading}
```

- [ ] **Step 4: Add UI state and controls**

In `ProvidersSettingsPaneContent.tsx`, add props:

```ts
catalog: readonly ProviderAccountCatalogEntry[];
mutationSaving: boolean;
mutationError: string | null;
onCreateProviderAccount: (input: {
  providerKind: string;
  displayName?: string | null;
  secret: string;
}) => Promise<unknown>;
onSaveProviderSecret: (input: {
  providerAccountId: string;
  secret: string;
}) => Promise<unknown>;
onClearProviderSecret: (input: { providerAccountId: string }) => Promise<unknown>;
```

Add an Add Provider Account row above the list:

```tsx
<form
  {...stylex.props(styles.card)}
  onSubmit={(event) => {
    event.preventDefault();
    if (!selectedProviderKind || secret.trim().length === 0) {
      return;
    }
    void onCreateProviderAccount({
      providerKind: selectedProviderKind,
      displayName: displayName.trim() || null,
      secret
    }).then(() => {
      setDisplayName("");
      setSecret("");
    });
  }}
>
  <div {...stylex.props(styles.titleRow)}>
    <h2 {...stylex.props(styles.cardTitle)}>Add provider account</h2>
  </div>
  <select value={selectedProviderKind} onChange={(event) => setSelectedProviderKind(event.currentTarget.value)}>
    {catalog.map((entry) => (
      <option key={entry.providerKind} value={entry.providerKind}>
        {entry.displayName}
      </option>
    ))}
  </select>
  <input
    value={displayName}
    onChange={(event) => setDisplayName(event.currentTarget.value)}
    placeholder="Account name"
  />
  <input
    type="password"
    value={secret}
    onChange={(event) => setSecret(event.currentTarget.value)}
    placeholder="API key"
    autoComplete="off"
  />
  <Button type="submit" label="Add account" disabled={mutationSaving || secret.trim().length === 0} />
</form>
```

For each account with `authMethod === "secret_input"`, render a replacement form:

```tsx
<form
  onSubmit={(event) => {
    event.preventDefault();
    if (replacementSecret.trim().length === 0) {
      return;
    }
    void onSaveProviderSecret({
      providerAccountId: account.providerAccountId,
      secret: replacementSecret
    }).then(() => setReplacementSecret(""));
  }}
>
  <input
    type="password"
    value={replacementSecret}
    onChange={(event) => setReplacementSecret(event.currentTarget.value)}
    placeholder="Replace API key"
    autoComplete="off"
  />
  <Button type="submit" label="Save key" disabled={mutationSaving || replacementSecret.trim().length === 0} />
  <Button
    type="button"
    variant="secondary"
    label="Clear key"
    disabled={mutationSaving}
    onClick={() =>
      void onClearProviderSecret({
        providerAccountId: account.providerAccountId
      })
    }
  />
</form>
```

- [ ] **Step 5: Run frontend validation**

Run from `crates/noema-core/web`:

```bash
bun run gen:types
bun run lint
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/components/settings/ProvidersSettingsPane.tsx crates/noema-core/web/src/components/settings/ProvidersSettingsPaneContent.tsx crates/noema-core/web/src/components/settings/providerMetadata.ts crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
git commit -m "feat: add provider account setup UI"
```

---

### Task 4: Exa Search Adapter

**Files:**
- Create: `crates/noema-core/src/search/exa.rs`
- Modify: `crates/noema-core/src/search.rs`
- Modify: `crates/noema-core/src/search/types.rs`
- Test: `crates/noema-core/src/search/exa.rs`

**Interfaces:**
- Produces: `EXA_SEARCH_PROVIDER_ID: &str = "exa"`.
- Produces: `EXA_SEARCH_CONTRACT: &str = "hosted_provider"`.
- Produces: `ExaSearchClient { base_url, api_key, http }`.
- Produces: `search_exa(client: &ExaSearchClient, request: &SearchRequest) -> Result<SearchResponse, SearchError>`.
- Consumes: existing `SearchRequest`, `SearchResponse`, `SearchResult`, and `SearchRuntimeProvider`.

- [ ] **Step 1: Write failing Exa search tests**

Create tests in `crates/noema-core/src/search/exa.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::SearchRequest;
    use serde_json::json;

    #[test]
    fn normalizes_exa_search_results() {
        let value = json!({
            "results": [
                {
                    "title": "Rust",
                    "url": "https://www.rust-lang.org/",
                    "highlights": ["A language empowering everyone"],
                    "text": "Full text",
                    "summary": "Summary"
                },
                {
                    "title": "Cargo",
                    "url": "https://doc.rust-lang.org/cargo/",
                    "text": "Cargo is Rust's package manager."
                }
            ]
        });

        let response = normalize_exa_search_response("rust", 1, &value).expect("response");

        assert_eq!(response.provider, "exa");
        assert_eq!(response.provider_contract, "hosted_provider");
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].rank, 1);
        assert_eq!(response.results[0].title, "Rust");
        assert_eq!(response.results[0].snippet, "A language empowering everyone");
    }

    #[tokio::test]
    async fn sends_search_request_with_api_key() {
        let (base_url, mut request_rx) = spawn_server(200, r#"{"results":[]}"#).await;
        let client = ExaSearchClient {
            base_url,
            api_key: "secret".to_string(),
            http: reqwest::Client::new(),
        };

        let response = search_exa(
            &client,
            &SearchRequest { query: "rust".to_string(), reason: None, max_results: 3 },
        ).await.expect("search");

        let request = request_rx.recv().await.expect("request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/search");
        assert_eq!(request.x_api_key.as_deref(), Some("secret"));
        assert!(request.body.contains("\"query\":\"rust\""));
        assert!(request.body.contains("\"numResults\":3"));
        assert!(response.results.is_empty());
    }
}
```

Reuse the small TCP test server style from `search/openai_hosted.rs`.

- [ ] **Step 2: Implement request and normalization**

Create `crates/noema-core/src/search/exa.rs`:

```rust
use crate::search::types::{SearchError, SearchRequest, SearchResponse, SearchResult};
use serde::Serialize;
use serde_json::Value;

pub(crate) const EXA_SEARCH_PROVIDER_ID: &str = "exa";
pub(crate) const EXA_SEARCH_CONTRACT: &str = "hosted_provider";

#[derive(Debug, Clone)]
pub struct ExaSearchClient {
    pub base_url: String,
    pub api_key: String,
    pub http: reqwest::Client,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExaSearchRequest<'a> {
    query: &'a str,
    num_results: usize,
}

pub(crate) async fn search_exa(
    client: &ExaSearchClient,
    request: &SearchRequest,
) -> Result<SearchResponse, SearchError> {
    let response = client
        .http
        .post(format!("{}/search", client.base_url.trim_end_matches('/')))
        .header("x-api-key", &client.api_key)
        .json(&ExaSearchRequest {
            query: &request.query,
            num_results: request.max_results,
        })
        .send()
        .await
        .map_err(map_reqwest_error)?;
    match response.status() {
        reqwest::StatusCode::TOO_MANY_REQUESTS => return Err(SearchError::RateLimited),
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => return Err(SearchError::Http),
        status if !status.is_success() => return Err(SearchError::Http),
        _ => {}
    }
    let value: Value = response.json().await.map_err(|_| SearchError::Parse)?;
    normalize_exa_search_response(&request.query, request.max_results, &value)
}

pub(crate) fn normalize_exa_search_response(
    query: &str,
    max_results: usize,
    value: &Value,
) -> Result<SearchResponse, SearchError> {
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or(SearchError::Parse)?
        .iter()
        .filter_map(exa_result)
        .take(max_results)
        .enumerate()
        .map(|(index, (title, url, snippet))| SearchResult {
            rank: index + 1,
            title,
            url,
            snippet,
        })
        .collect::<Vec<_>>();
    let summary = match results.len() {
        0 => "No web results found".to_string(),
        1 => "Found 1 Exa web result".to_string(),
        count => format!("Found {count} Exa web results"),
    };
    Ok(SearchResponse {
        provider: EXA_SEARCH_PROVIDER_ID.to_string(),
        provider_contract: EXA_SEARCH_CONTRACT.to_string(),
        query: query.to_string(),
        results,
        summary,
    })
}

fn exa_result(value: &Value) -> Option<(String, String, String)> {
    let url = value.get("url")?.as_str()?.trim().to_string();
    if url.is_empty() {
        return None;
    }
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&url)
        .to_string();
    let snippet = value
        .get("highlights")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find_map(Value::as_str))
        .or_else(|| value.get("summary").and_then(Value::as_str))
        .or_else(|| value.get("text").and_then(Value::as_str))
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    Some((title, url, snippet))
}

fn map_reqwest_error(error: reqwest::Error) -> SearchError {
    if error.is_timeout() {
        SearchError::Timeout
    } else {
        SearchError::Http
    }
}
```

- [ ] **Step 3: Export runtime provider**

In `crates/noema-core/src/search.rs`:

```rust
pub(crate) mod exa;
```

In `SearchRuntimeProvider`, add:

```rust
Exa {
    client: crate::search::exa::ExaSearchClient,
},
```

In `SearchRuntimeProvider::search`, add:

```rust
Self::Exa { client } => crate::search::exa::search_exa(client, request).await,
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p noema-core search::exa --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/search.rs crates/noema-core/src/search/types.rs crates/noema-core/src/search/exa.rs
git commit -m "feat: add Exa web search adapter"
```

---

### Task 5: Exa Fetch Adapter

**Files:**
- Create: `crates/noema-core/src/web_fetch/exa.rs`
- Modify: `crates/noema-core/src/web_fetch/mod.rs`
- Modify: `crates/noema-core/src/web_fetch/types.rs`
- Test: `crates/noema-core/src/web_fetch/exa.rs`

**Interfaces:**
- Produces: `EXA_FETCH_PROVIDER_ID: &str = "exa"`.
- Produces: `EXA_EXTRACTION: &str = "exa_contents"`.
- Produces: `ExaFetchClient { base_url, api_key, http }`.
- Produces: `fetch_exa(client: &ExaFetchClient, request: &FetchRequest) -> Result<FetchResponse, FetchError>`.
- Consumes: existing `FetchRequest`, `FetchResponse`, `FetchError`, and URL policy validation.

- [ ] **Step 1: Write failing Exa fetch tests**

Create tests in `crates/noema-core/src/web_fetch/exa.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_fetch::types::FetchRequest;
    use serde_json::json;

    #[test]
    fn normalizes_exa_contents_response() {
        let value = json!({
            "results": [
                {
                    "title": "Rust",
                    "url": "https://www.rust-lang.org/",
                    "text": "# Rust\nFast and reliable."
                }
            ]
        });

        let response = normalize_exa_contents_response(
            "https://www.rust-lang.org/",
            20,
            &value,
        ).expect("fetch");

        assert_eq!(response.provider, "exa");
        assert_eq!(response.url, "https://www.rust-lang.org/");
        assert_eq!(response.final_url, "https://www.rust-lang.org/");
        assert_eq!(response.title.as_deref(), Some("Rust"));
        assert_eq!(response.content, "# Rust\nFast and reliable.");
        assert_eq!(response.extraction, "exa_contents");
        assert!(!response.truncated);
    }

    #[tokio::test]
    async fn sends_contents_request_with_api_key() {
        let (base_url, mut request_rx) = spawn_server(
            200,
            r#"{"results":[{"title":"Rust","url":"https://www.rust-lang.org/","text":"Rust"}]}"#,
        ).await;
        let client = ExaFetchClient {
            base_url,
            api_key: "secret".to_string(),
            http: reqwest::Client::new(),
        };

        let response = fetch_exa(
            &client,
            &FetchRequest {
                url: "https://www.rust-lang.org/".to_string(),
                reason: None,
                max_chars: 8_000,
            },
        ).await.expect("fetch");

        let request = request_rx.recv().await.expect("request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/contents");
        assert_eq!(request.x_api_key.as_deref(), Some("secret"));
        assert!(request.body.contains("\"urls\":[\"https://www.rust-lang.org/\"]"));
        assert!(request.body.contains("\"text\":true"));
        assert_eq!(response.content, "Rust");
    }
}
```

Reuse the small TCP test server style from `search/openai_hosted.rs`.

- [ ] **Step 2: Implement Exa contents adapter**

Create `crates/noema-core/src/web_fetch/exa.rs`:

```rust
use crate::web_fetch::{
    tool::sanitized_web_fetch_display_url,
    types::{
        FetchContentKind, FetchError, FetchRequest, FetchResponse, FetchSummaryStrategy,
    },
    url_policy::validate_public_web_fetch_url,
};
use serde::Serialize;
use serde_json::Value;

pub const EXA_FETCH_PROVIDER_ID: &str = "exa";
pub const EXA_EXTRACTION: &str = "exa_contents";

#[derive(Debug, Clone)]
pub struct ExaFetchClient {
    pub base_url: String,
    pub api_key: String,
    pub http: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct ExaContentsRequest<'a> {
    urls: [&'a str; 1],
    text: bool,
}

pub async fn fetch_exa(
    client: &ExaFetchClient,
    request: &FetchRequest,
) -> Result<FetchResponse, FetchError> {
    validate_public_web_fetch_url(&request.url).await?;
    let response = client
        .http
        .post(format!("{}/contents", client.base_url.trim_end_matches('/')))
        .header("x-api-key", &client.api_key)
        .json(&ExaContentsRequest {
            urls: [&request.url],
            text: true,
        })
        .send()
        .await
        .map_err(map_reqwest_error)?;
    if !response.status().is_success() {
        return Err(match response.status() {
            reqwest::StatusCode::REQUEST_TIMEOUT | reqwest::StatusCode::GATEWAY_TIMEOUT => FetchError::Timeout,
            _ => FetchError::Http,
        });
    }
    let value: Value = response.json().await.map_err(|_| FetchError::Extraction)?;
    normalize_exa_contents_response(&request.url, request.max_chars, &value)
}

pub fn normalize_exa_contents_response(
    requested_url: &str,
    max_chars: usize,
    value: &Value,
) -> Result<FetchResponse, FetchError> {
    let result = value
        .get("results")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or(FetchError::Extraction)?;
    let final_url = result
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or(requested_url)
        .to_string();
    let title = result
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let raw_content = result
        .get("text")
        .and_then(Value::as_str)
        .ok_or(FetchError::Extraction)?;
    let raw_chars = raw_content.chars().count();
    let content = raw_content.chars().take(max_chars).collect::<String>();
    let returned_chars = content.chars().count();

    Ok(FetchResponse {
        provider: EXA_FETCH_PROVIDER_ID.to_string(),
        url: sanitized_web_fetch_display_url(requested_url),
        final_url,
        title,
        format: "markdown".to_string(),
        extraction: EXA_EXTRACTION.to_string(),
        content_kind: FetchContentKind::RawMarkdown,
        content,
        raw_excerpt: None,
        raw_chars,
        returned_chars,
        summary_model: None,
        summary_strategy: FetchSummaryStrategy::NotSummarized,
        truncated: raw_chars > returned_chars,
    })
}

fn map_reqwest_error(error: reqwest::Error) -> FetchError {
    if error.is_timeout() {
        FetchError::Timeout
    } else {
        FetchError::Http
    }
}
```

- [ ] **Step 3: Export runtime provider**

In `crates/noema-core/src/web_fetch/mod.rs`:

```rust
#[doc(hidden)]
pub mod exa;
```

In `WebFetchRuntimeProvider`, add:

```rust
Exa {
    client: crate::web_fetch::exa::ExaFetchClient,
},
```

In `WebFetchRuntimeProvider::fetch`, add:

```rust
Self::Exa { client } => crate::web_fetch::exa::fetch_exa(client, request).await,
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p noema-core web_fetch::exa --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/web_fetch/mod.rs crates/noema-core/src/web_fetch/types.rs crates/noema-core/src/web_fetch/exa.rs
git commit -m "feat: add Exa web fetch adapter"
```

---

### Task 6: Runtime Binding Integration And Final Validation

**Files:**
- Modify: `crates/noema-core/src/graphql/web_tool_settings.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Modify: `crates/noema-core/src/daemon/runtime/web_tools.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Generated: `crates/noema-core/web/src/generated/schema.graphql`
- Generated: `crates/noema-core/web/src/generated/graphql.ts`
- Test: `crates/noema-core/src/graphql/web_tool_settings.rs`
- Test: `crates/noema-core/src/daemon/runtime/local_tools.rs`

**Interfaces:**
- Consumes: `SecretInputStore::load_api_key`.
- Consumes: `SearchRuntimeProvider::Exa` and `WebFetchRuntimeProvider::Exa`.
- Produces: Exa accounts appear in `webToolSettings` only when created and available.
- Produces: `web.search` and `web.fetch` execute through Exa when a binding selects an authenticated Exa account.
- Produces: unauthenticated or missing-secret Exa bindings fall back to DuckDuckGo/direct HTTP with safe fallback metadata.

- [ ] **Step 1: Write failing web settings test for non-default Exa accounts**

Add to `crates/noema-core/src/graphql/web_tool_settings.rs` tests:

```rust
#[tokio::test]
async fn web_tool_settings_include_active_non_default_exa_accounts() {
    let store = test_store().await;
    insert_provider_account(
        &store,
        "provider_account:exa:acct_research",
        "exa",
        "acct_research",
        false,
        ProviderAccountStatus::Authenticated,
    ).await;
    let state = GraphqlState::for_tests_with_store(store);

    let settings = web_tool_settings(&state).await.expect("settings");

    assert!(settings.search.provider_options.iter().any(|option| {
        option.provider_account_id == "provider_account:exa:acct_research"
    }));
    assert!(settings.fetch.provider_options.iter().any(|option| {
        option.provider_account_id == "provider_account:exa:acct_research"
    }));
}
```

- [ ] **Step 2: Update selectable accounts**

In `selectable_accounts`, replace `active_default_provider_accounts()` with `active_provider_accounts()`:

```rust
async fn selectable_accounts(store: &NoemaStore) -> Result<Vec<ProviderAccountRecord>> {
    let mut accounts = store.system_provider_accounts();
    accounts.extend(store.active_provider_accounts().await.map_err(graphql_error)?);
    Ok(accounts)
}
```

- [ ] **Step 3: Write failing runtime tests for missing secret fallback**

Add to `crates/noema-core/src/daemon/runtime/local_tools.rs` tests:

```rust
#[tokio::test]
async fn bound_exa_web_search_without_secret_falls_back_to_duckduckgo() {
    let actor = test_actor_with_store().await;
    insert_provider_account(
        actor.store(),
        "provider_account:exa:acct_research",
        "exa",
        "acct_research",
        ProviderAccountStatus::Authenticated,
    ).await;
    insert_provider_capability_binding(
        actor.store(),
        "web.search",
        "provider_account:exa:acct_research",
    ).await;

    let (provider, fallback_from, fallback_reason) = actor
        .web_search_runtime_provider_resolution()
        .await
        .expect("provider");

    assert!(matches!(provider, SearchRuntimeProvider::DuckDuckGoPublic { .. }));
    assert_eq!(fallback_from.as_deref(), Some("provider_account:exa:acct_research"));
    assert_eq!(fallback_reason.as_deref(), Some("provider account unauthenticated"));
}

#[tokio::test]
async fn bound_exa_web_fetch_without_secret_falls_back_to_direct_http() {
    let actor = test_actor_with_store().await;
    insert_provider_account(
        actor.store(),
        "provider_account:exa:acct_research",
        "exa",
        "acct_research",
        ProviderAccountStatus::Authenticated,
    ).await;
    insert_provider_capability_binding(
        actor.store(),
        "web.fetch",
        "provider_account:exa:acct_research",
    ).await;

    let (context, fallback_from, fallback_reason) = actor
        .web_fetch_runtime_execution_context()
        .await
        .expect("context");

    assert_eq!(fallback_from.as_deref(), Some("provider_account:exa:acct_research"));
    assert_eq!(fallback_reason.as_deref(), Some("provider account unauthenticated"));
    assert_eq!(context.summarizer_model, DEFAULT_TOOL_CLASSIFICATION_MODEL);
}
```

- [ ] **Step 4: Load Exa credentials and construct runtime providers**

In `local_tools.rs`, update search provider resolution:

```rust
crate::search::exa::EXA_SEARCH_PROVIDER_ID => {
    let api_key = self
        .load_provider_secret_api_key(&resolved.provider_kind, &resolved.account_key)
        .await
        .map_err(|_| "provider account unauthenticated".to_string())?;
    Ok((
        SearchRuntimeProvider::Exa {
            client: crate::search::exa::ExaSearchClient {
                base_url: "https://api.exa.ai".to_string(),
                api_key,
                http: reqwest::Client::new(),
            },
        },
        resolved.fallback_from,
        resolved.fallback_reason,
    ))
}
```

For fetch, change `web_fetch_runtime_execution_context` to return the selected `WebFetchRuntimeProvider` as well as the summarizer context:

```rust
async fn web_fetch_runtime_execution_context(
    &self,
) -> Result<(WebFetchRuntimeProvider, FetchRuntimeContext, Option<String>, Option<String>), String>
```

Return `WebFetchRuntimeProvider::DirectHttp` for `direct_http`, and:

```rust
WebFetchRuntimeProvider::Exa {
    client: crate::web_fetch::exa::ExaFetchClient {
        base_url: "https://api.exa.ai".to_string(),
        api_key,
        http: reqwest::Client::new(),
    },
}
```

Add helper:

```rust
async fn load_provider_secret_api_key(
    &self,
    provider_kind: &str,
    account_key: &str,
) -> Result<String, String> {
    let paths = self.paths().map_err(|_| "provider account unauthenticated".to_string())?;
    crate::provider::secret_input::SecretInputStore::new(
        paths.provider_account_home(provider_kind, account_key),
    )
    .load_api_key()
    .map_err(|_| "provider account unauthenticated".to_string())
}
```

Keep the error string safe and non-secret.

- [ ] **Step 5: Preserve fallback semantics**

When a selected Exa account has missing secret, return the default provider plus:

```rust
fallback_from = Some(resolved.provider_account_id.clone());
fallback_reason = Some("provider account unauthenticated".to_string());
```

Do not persist raw Exa HTTP response bodies. If a runtime Exa HTTP call returns 401 or 403, update the provider account status to `Unauthenticated` with:

```rust
error_code = Some("auth_failed")
error_message = Some("provider account unauthenticated")
```

- [ ] **Step 6: Regenerate frontend GraphQL artifacts**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: PASS.

- [ ] **Step 7: Run focused validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core provider::capabilities::tests::exa_account_declares_search_and_fetch_only --no-fail-fast
cargo test -p noema-core provider::secret_input search::exa web_fetch::exa graphql::web_tool_settings --no-fail-fast
cargo check --workspace
cd crates/noema-core/web && bun run gen:types && bun run lint
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/graphql/web_tool_settings.rs crates/noema-core/src/daemon/runtime/local_tools.rs crates/noema-core/src/daemon/runtime/web_tools.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
git commit -m "feat: route web tools through Exa provider"
```

---

## Plan Self-Review

- Spec coverage: Tasks 1 and 2 cover Exa provider kind, catalog, created accounts, secret input, and write-only storage. Task 3 covers Settings add-account and secret controls. Tasks 4 and 5 cover Exa search/fetch API adapters. Task 6 covers web tool binding options, runtime execution, fallback metadata, and validation.
- Placeholder scan: No `TBD`, `TODO`, or open-ended "add validation" steps remain. Steps name exact files, functions, and commands.
- Type consistency: The plan consistently uses `provider_kind`, `account_key`, `provider_account_id`, `ProviderAuthMethod::SecretInput`, `ProviderAccountStatus::{Authenticated,Unauthenticated}`, `SearchRuntimeProvider::Exa`, and `WebFetchRuntimeProvider::Exa`.
