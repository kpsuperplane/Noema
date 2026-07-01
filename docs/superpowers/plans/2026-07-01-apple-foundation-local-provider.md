# Apple Foundation Local Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Apple Foundation Models as an optional macOS-only local provider, with daemon-owned Swift bridge lifecycle, Noema-owned conversation resume, and web dashboard controls for per-agent provider/model selection.

**Architecture:** Keep provider account availability separate from agent runtime model choice. Portable Rust owns config, store, GraphQL, dashboard data, bridge protocol types, transcript replay shaping, and non-macOS unavailable behavior. macOS-only code owns launching a Swift bridge child process that manages live `LanguageModelSession`s while Noema remains the source of durable conversation state.

**Tech Stack:** Rust 2024, Tokio child processes/stdout pipes, async-graphql, SurrealDB v3 embedded store, TypeScript React, Apollo Client, Bun, Swift Foundation Models bridge gated to macOS targets.

---

## Scope Check

This spec spans provider metadata, agent preferences, runtime selection, bridge protocol/lifecycle, and dashboard UI. They are tightly coupled parts of one feature: a user must be able to configure an agent to use the local Apple provider, the runtime must resolve that preference, and the provider must either run on macOS or report clean unavailability elsewhere. The plan is split into milestones with commits so each unit is reviewable and testable.

The first implementation should ship the portable infrastructure, UI, and fake-bridge-tested lifecycle. Native Swift/Foundation Models execution should be guarded and opt-in so Linux/Windows and non-macOS CI stay green.

## File Structure

### Rust Provider And Config

- Modify `crates/noema-core/src/config.rs`: add `ProviderKind::FoundationLocal`, `ProviderConfig::FoundationLocal`, daemon config resolution for any selected provider, and raw `foundation_local` config.
- Modify `crates/noema-core/src/config/tests.rs`: add config parsing tests for `foundation_local`.
- Create `crates/noema-core/src/provider/adapters/foundation_local.rs`: portable provider adapter facade. On non-macOS it returns unavailable. On macOS it delegates to the bridge lifecycle manager.
- Create `crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs`: serde protocol structs for bridge messages and responses.
- Modify `crates/noema-core/src/provider/adapters.rs`: export the new modules.
- Modify `crates/noema-core/src/lib.rs`: export new config and provider types.

### Store

- Modify `crates/noema-core/src/store/schema.rs`: allow `foundation_local` in provider account and conversation provider fields, add `agent_runtime_preferences`, and add model/profile metadata fields.
- Modify `crates/noema-core/src/store/provider_accounts.rs`: add default Foundation account creation and list all active default accounts.
- Create `crates/noema-core/src/store/agent_runtime_preferences.rs`: store APIs for agent runtime preferences.
- Modify `crates/noema-core/src/store/mod.rs`: include and export the new store module.
- Modify `crates/noema-core/src/store/tests.rs`: add provider and preference storage tests.

### GraphQL

- Modify `crates/noema-core/src/graphql/provider_accounts.rs`: return all active/default provider accounts.
- Modify `crates/noema-core/src/graphql/agents.rs`: expose agent model preference and available provider/profile options.
- Modify `crates/noema-core/src/graphql/schema.rs`: add the `saveAgentModelPreference` mutation and SDL tests.
- Modify `crates/noema-core/web/src/graphql/operations.ts`: update the `Agents` query and add the save mutation.

### Runtime

- Modify `crates/noema-core/src/runtime_host.rs`: start the runtime from `ProviderConfig` rather than `CodexProviderConfig`.
- Modify `crates/noema-core/src/daemon/runtime/handle.rs`: add a provider factory that can construct Codex, OpenAI, or Foundation Local providers while keeping the existing runtime actor API.
- Modify `crates/noema-core/src/daemon/runtime/turn.rs`: resolve the agent preference for the conversation before building provider requests.
- Modify `crates/noema-core/src/graphql/runtime_state.rs`: retain access to the runtime and store after the runtime type is generalized.
- Modify daemon and GraphQL tests under `crates/noema-core/src/daemon/tests.rs` and `crates/noema-core/src/graphql/schema.rs` for provider resolution behavior.

### Swift Bridge

- Create `crates/noema-core/apple-foundation-bridge/Package.swift`: Swift package, macOS-only.
- Create `crates/noema-core/apple-foundation-bridge/Sources/NoemaFoundationBridge/main.swift`: JSON-lines stdio bridge entrypoint.
- Create `crates/noema-core/apple-foundation-bridge/README.md`: manual opt-in bridge build notes.

The Swift bridge directory must not become a Cargo workspace member and must not be required for default Rust validation.

### Web Dashboard

- Modify `crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx`: wire mutation state and refetch/update behavior.
- Modify `crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx`: render current provider/profile and compact edit controls.
- Modify `crates/noema-core/web/src/components/settings/agentMetadata.ts`: format provider/model rows and disabled reasons.
- Modify `crates/noema-core/web/src/components/settings/ProvidersSettingsPaneContent.tsx`: ensure local Apple provider appears as status-only with no model picker.
- Modify `crates/noema-core/web/src/generated/graphql.ts`: regenerate with `bun run gen:types`.

---

### Task 1: Provider Kind, Config, And Provider Account Storage

**Files:**
- Modify: `crates/noema-core/src/config.rs`
- Modify: `crates/noema-core/src/config/tests.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/provider_accounts.rs`
- Modify: `crates/noema-core/src/store/tests.rs`
- Modify: `crates/noema-core/src/lib.rs`

- [ ] **Step 1: Write config tests for `foundation_local`**

Add these tests to `crates/noema-core/src/config/tests.rs`:

```rust
#[test]
fn foundation_local_config_does_not_require_openai_or_codex_credentials() {
    let resolved = load_resolved(
        None,
        CliOverrides::new(Some("foundation_local".to_string()), None, None),
        None,
        &[],
    )
    .expect("foundation local config should resolve without credentials");

    assert_eq!(resolved.provider.kind(), ProviderKind::FoundationLocal);

    let ProviderConfig::FoundationLocal(config) = resolved.provider else {
        panic!("expected foundation local config");
    };

    assert_eq!(config.default_profile, "default");
    assert_eq!(config.bridge_path, None);
}

#[test]
fn foundation_local_config_reads_yaml_and_env_overrides() {
    let file = write_config(
        r"
provider: foundation_local
foundation_local:
  default_profile: compact
  bridge_path: /tmp/noema-foundation-bridge
",
    );

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        CliOverrides::default(),
        None,
        &[
            ("NOEMA_FOUNDATION_LOCAL__DEFAULT_PROFILE", "default"),
            (
                "NOEMA_FOUNDATION_LOCAL__BRIDGE_PATH",
                "/tmp/env-noema-foundation-bridge",
            ),
        ],
    )
    .expect("foundation local config should resolve");

    let ProviderConfig::FoundationLocal(config) = resolved.provider else {
        panic!("expected foundation local config");
    };

    assert_eq!(config.default_profile, "default");
    assert_eq!(
        config.bridge_path.as_deref(),
        Some(std::path::Path::new("/tmp/env-noema-foundation-bridge"))
    );
}
```

- [ ] **Step 2: Run the focused config tests and verify they fail**

Run:

```bash
cargo test -p noema-core config::tests::foundation_local -- --nocapture
```

Expected: FAIL because `ProviderKind::FoundationLocal`, `ProviderConfig::FoundationLocal`, and `RawFoundationLocalConfig` do not exist.

- [ ] **Step 3: Add the Foundation Local config types**

In `crates/noema-core/src/config.rs`, add the import and constants near the existing provider constants:

```rust
/// Default profile id for Apple Foundation Models.
pub const DEFAULT_FOUNDATION_LOCAL_PROFILE: &str = "default";
```

Add env keys to `CONFIG_ENV_KEYS`:

```rust
    "foundation_local.default_profile",
    "foundation_local.bridge_path",
```

Extend `ProviderKind`:

```rust
pub enum ProviderKind {
    /// Codex CLI provider.
    Codex,
    /// `OpenAI` Responses API provider.
    OpenAi,
    /// Local Apple Foundation Models provider.
    FoundationLocal,
}

impl ProviderKind {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::OpenAi => "openai",
            Self::FoundationLocal => "foundation_local",
        }
    }
}

impl FromStr for ProviderKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "codex" => Ok(Self::Codex),
            "openai" => Ok(Self::OpenAi),
            "foundation_local" => Ok(Self::FoundationLocal),
            other => Err(other.to_string()),
        }
    }
}
```

Add config structs and enum variant:

```rust
/// Apple Foundation Models provider configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundationLocalProviderConfig {
    /// Default user-facing profile id when an agent has no preference.
    pub default_profile: String,
    /// Optional path to a manually built Swift bridge executable.
    pub bridge_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderConfig {
    /// Codex provider configuration.
    Codex(CodexProviderConfig),
    /// `OpenAI` provider configuration.
    OpenAi(OpenAiProviderConfig),
    /// Apple Foundation Models local provider configuration.
    FoundationLocal(FoundationLocalProviderConfig),
}
```

Update `ProviderConfig::kind` and `ProviderConfig::model`:

```rust
    pub fn kind(&self) -> ProviderKind {
        match self {
            Self::Codex(_) => ProviderKind::Codex,
            Self::OpenAi(_) => ProviderKind::OpenAi,
            Self::FoundationLocal(_) => ProviderKind::FoundationLocal,
        }
    }

    pub fn model(&self) -> Option<&str> {
        match self {
            Self::Codex(config) => config.default_model.as_deref(),
            Self::OpenAi(config) => Some(config.default_model.as_str()),
            Self::FoundationLocal(config) => Some(config.default_profile.as_str()),
        }
    }
```

Add raw config:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawFoundationLocalConfig {
    default_profile: String,
    bridge_path: Option<PathBuf>,
}

impl Default for RawFoundationLocalConfig {
    fn default() -> Self {
        Self {
            default_profile: DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
            bridge_path: None,
        }
    }
}
```

Add it to `RawConfig`:

```rust
    foundation_local: RawFoundationLocalConfig,
```

Initialize it in `Default for RawConfig`:

```rust
            foundation_local: RawFoundationLocalConfig::default(),
```

Update `RawConfig::resolve`:

```rust
            ProviderKind::OpenAi => ProviderConfig::OpenAi(self.resolve_openai_config()?),
            ProviderKind::Codex => ProviderConfig::Codex(self.resolve_codex_config()?),
            ProviderKind::FoundationLocal => {
                ProviderConfig::FoundationLocal(self.resolve_foundation_local_config()?)
            }
```

Add resolver:

```rust
    fn resolve_foundation_local_config(&self) -> Result<FoundationLocalProviderConfig, ConfigError> {
        let default_profile = non_empty_option(Some(self.foundation_local.default_profile.as_str()))
            .unwrap_or(DEFAULT_FOUNDATION_LOCAL_PROFILE)
            .to_string();
        Ok(FoundationLocalProviderConfig {
            default_profile,
            bridge_path: self.foundation_local.bridge_path.clone(),
        })
    }
```

- [ ] **Step 4: Run the config tests and verify they pass**

Run:

```bash
cargo test -p noema-core config::tests::foundation_local -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Write store tests for Foundation provider accounts**

Add tests near `default_provider_account_round_trips_status` in `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn default_foundation_local_provider_account_is_available_metadata() {
    let store = test_store().await;

    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation provider account");

    assert_eq!(
        account.provider_account_id,
        "provider_account:foundation_local:default"
    );
    assert_eq!(account.provider_kind, "foundation_local");
    assert_eq!(account.account_key, "default");
    assert_eq!(account.display_name, "Apple Foundation Models");
    assert_eq!(account.auth_method, crate::ProviderAuthMethod::None);
    assert_eq!(account.status, ProviderAccountStatus::Unknown);
}

#[tokio::test]
async fn active_default_provider_accounts_lists_codex_and_foundation() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");

    let accounts = store
        .active_default_provider_accounts()
        .await
        .expect("provider accounts");

    let kinds: Vec<_> = accounts
        .iter()
        .map(|account| account.provider_kind.as_str())
        .collect();
    assert_eq!(kinds, vec!["codex", "foundation_local"]);
}
```

- [ ] **Step 6: Run the store tests and verify they fail**

Run:

```bash
cargo test -p noema-core store::tests::default_foundation store::tests::active_default_provider_accounts -- --nocapture
```

Expected: FAIL because the new store methods and schema enum values do not exist.

- [ ] **Step 7: Update store schema and provider account APIs**

In `crates/noema-core/src/store/schema.rs`, update provider assertions:

```rust
DEFINE FIELD OVERWRITE provider_kind ON TABLE provider_accounts TYPE string ASSERT $value INSIDE ['codex', 'foundation_local'];
```

Update conversation provider assertion:

```rust
DEFINE FIELD OVERWRITE provider ON TABLE conversations TYPE string ASSERT $value INSIDE ['codex', 'foundation_local'];
```

In `crates/noema-core/src/store/provider_accounts.rs`, add:

```rust
    /// Create or return the default Apple Foundation Models provider account metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn ensure_default_foundation_local_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, StoreError> {
        const ACCOUNT_ID: &str = "provider_account:foundation_local:default";
        if let Some(account) = self.get_provider_account(ACCOUNT_ID).await? {
            return Ok(account);
        }

        self.db
            .query(
                r#"
                UPSERT type::record('provider_accounts', 'foundation_local_default') SET
                  provider_account_id = 'provider_account:foundation_local:default',
                  provider_kind = 'foundation_local',
                  account_key = 'default',
                  display_name = 'Apple Foundation Models',
                  auth_method = 'none',
                  is_active = true,
                  is_default = true,
                  status = 'unknown',
                  metadata = { profiles: [{ id: 'default', label: 'Default on-device' }] },
                  updated_at = time::now();
                "#,
            )
            .await?
            .check()?;
        self.get_provider_account(ACCOUNT_ID)
            .await?
            .ok_or_else(|| StoreError::ProviderAccountNotFound {
                provider_account_id: ACCOUNT_ID.to_string(),
            })
    }

    /// Return all active default provider accounts in stable Settings order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn active_default_provider_accounts(
        &self,
    ) -> Result<Vec<ProviderAccountRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, last_checked_at,
                  last_authenticated_at, last_error_code, last_error_message, metadata
                FROM provider_accounts
                WHERE is_active = true
                  AND is_default = true;
                "#,
            )
            .await?;
        let rows: Vec<ProviderAccountRow> = response.take(0)?;
        let mut accounts = rows
            .into_iter()
            .map(provider_account_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        accounts.sort_by(|left, right| left.provider_kind.cmp(&right.provider_kind));
        Ok(accounts)
    }
```

- [ ] **Step 8: Ensure runtime bootstrap creates both default accounts**

In `crates/noema-core/src/runtime_host.rs`, after `ensure_default_provider_account()`, add:

```rust
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .map_err(|source| RuntimeHostError::Store(source.to_string()))?;
```

In daemon startup code that directly opens the store, add the same call wherever `ensure_default_provider_account()` is called.

- [ ] **Step 9: Export Foundation config type**

In `crates/noema-core/src/lib.rs`, export the type:

```rust
pub use config::{
    CliOverrides, Config, ConfigError, DaemonResolvedConfig, FoundationLocalProviderConfig,
    ProviderConfig, ProviderKind, ResolvedConfig, WebConfig,
};
```

- [ ] **Step 10: Run tests for this task**

Run:

```bash
cargo test -p noema-core config::tests::foundation_local store::tests::default_foundation store::tests::active_default_provider_accounts -- --nocapture
```

Expected: PASS.

- [ ] **Step 11: Commit Task 1**

```bash
git add crates/noema-core/src/config.rs crates/noema-core/src/config/tests.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/provider_accounts.rs crates/noema-core/src/store/tests.rs crates/noema-core/src/lib.rs crates/noema-core/src/runtime_host.rs
git commit -m "Add foundation local provider metadata"
```

---

### Task 2: Agent Runtime Preference Store

**Files:**
- Create: `crates/noema-core/src/store/agent_runtime_preferences.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/mod.rs`
- Modify: `crates/noema-core/src/store/tests.rs`
- Modify: `crates/noema-core/src/lib.rs`

- [ ] **Step 1: Write store tests for agent runtime preferences**

Add to `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn agent_runtime_preference_round_trips() {
    let store = test_store().await;
    store
        .ensure_default_actors()
        .await
        .expect("default actors");
    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");

    let saved = store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: account.provider_account_id.clone(),
            model_profile: "default".to_string(),
        })
        .await
        .expect("save preference");

    assert_eq!(saved.agent_id, "agent:primary");
    assert_eq!(saved.provider_kind, "foundation_local");
    assert_eq!(saved.provider_account_id, account.provider_account_id);
    assert_eq!(saved.model_profile, "default");

    let loaded = store
        .get_agent_runtime_preference("agent:primary")
        .await
        .expect("load preference")
        .expect("preference exists");

    assert_eq!(loaded, saved);
}

#[tokio::test]
async fn agent_runtime_preference_rejects_unknown_provider_account() {
    let store = test_store().await;
    store
        .ensure_default_actors()
        .await
        .expect("default actors");

    let error = store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: "provider_account:foundation_local:missing".to_string(),
            model_profile: "default".to_string(),
        })
        .await
        .expect_err("unknown provider account should fail");

    assert!(matches!(
        error,
        StoreError::ProviderAccountNotFound { provider_account_id }
            if provider_account_id == "provider_account:foundation_local:missing"
    ));
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p noema-core store::tests::agent_runtime_preference -- --nocapture
```

Expected: FAIL because the preference table, types, and methods do not exist.

- [ ] **Step 3: Add the schema table**

In `crates/noema-core/src/store/schema.rs`, after the `agents` table:

```rust
DEFINE TABLE IF NOT EXISTS agent_runtime_preferences SCHEMAFULL;
DEFINE FIELD OVERWRITE agent_id ON TABLE agent_runtime_preferences TYPE string;
DEFINE FIELD OVERWRITE provider_kind ON TABLE agent_runtime_preferences TYPE string ASSERT $value INSIDE ['codex', 'openai', 'foundation_local'];
DEFINE FIELD OVERWRITE provider_account_id ON TABLE agent_runtime_preferences TYPE string;
DEFINE FIELD OVERWRITE model_profile ON TABLE agent_runtime_preferences TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE created_at ON TABLE agent_runtime_preferences TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE agent_runtime_preferences TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS agent_runtime_preferences_agent_id ON TABLE agent_runtime_preferences COLUMNS agent_id UNIQUE;
```

- [ ] **Step 4: Create the store module**

Create `crates/noema-core/src/store/agent_runtime_preferences.rs`:

```rust
use serde::Deserialize;
use surrealdb::types::SurrealValue;

use super::{NoemaStore, StoreError, agents::agent_record_fragment};

/// New or updated agent runtime preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAgentRuntimePreference {
    /// Durable agent id.
    pub agent_id: String,
    /// Provider kind selected for this agent.
    pub provider_kind: String,
    /// Provider account id selected for this agent.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
}

/// Persisted agent runtime preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntimePreferenceRecord {
    /// Durable agent id.
    pub agent_id: String,
    /// Provider kind selected for this agent.
    pub provider_kind: String,
    /// Provider account id selected for this agent.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
}

impl NoemaStore {
    /// Return the runtime preference for one agent.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn get_agent_runtime_preference(
        &self,
        agent_id: &str,
    ) -> Result<Option<AgentRuntimePreferenceRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT agent_id, provider_kind, provider_account_id, model_profile
                FROM agent_runtime_preferences
                WHERE agent_id = $agent_id
                LIMIT 1;
                "#,
            )
            .bind(("agent_id", agent_id.to_string()))
            .await?;
        let rows: Vec<AgentRuntimePreferenceRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(preference_from_row))
    }

    /// Create or update one agent runtime preference.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the agent or provider account is missing, the
    /// model/profile is blank, or the embedded store write/read fails.
    pub async fn upsert_agent_runtime_preference(
        &self,
        preference: NewAgentRuntimePreference,
    ) -> Result<AgentRuntimePreferenceRecord, StoreError> {
        self.require_agent(&preference.agent_id).await?;
        let Some(account) = self
            .get_provider_account(&preference.provider_account_id)
            .await?
        else {
            return Err(StoreError::ProviderAccountNotFound {
                provider_account_id: preference.provider_account_id,
            });
        };
        if account.provider_kind != preference.provider_kind {
            return Err(StoreError::InvalidEnum {
                kind: "agent_runtime_provider_kind",
                value: preference.provider_kind,
            });
        }
        let model_profile = preference.model_profile.trim();
        if model_profile.is_empty() {
            return Err(StoreError::InvalidEnum {
                kind: "agent_runtime_model_profile",
                value: preference.model_profile,
            });
        }

        self.db
            .query(
                r#"
                UPSERT type::record('agent_runtime_preferences', $record_id) SET
                  agent_id = $agent_id,
                  provider_kind = $provider_kind,
                  provider_account_id = $provider_account_id,
                  model_profile = $model_profile,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", agent_record_fragment(&preference.agent_id)))
            .bind(("agent_id", preference.agent_id.clone()))
            .bind(("provider_kind", account.provider_kind))
            .bind(("provider_account_id", account.provider_account_id))
            .bind(("model_profile", model_profile.to_string()))
            .await?
            .check()?;
        self.get_agent_runtime_preference(&preference.agent_id)
            .await?
            .ok_or(StoreError::AgentNotFound {
                agent_id: preference.agent_id,
            })
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct AgentRuntimePreferenceRow {
    agent_id: String,
    provider_kind: String,
    provider_account_id: String,
    model_profile: String,
}

fn preference_from_row(row: AgentRuntimePreferenceRow) -> AgentRuntimePreferenceRecord {
    AgentRuntimePreferenceRecord {
        agent_id: row.agent_id,
        provider_kind: row.provider_kind,
        provider_account_id: row.provider_account_id,
        model_profile: row.model_profile,
    }
}
```

- [ ] **Step 5: Wire module exports**

In `crates/noema-core/src/store/mod.rs`, add:

```rust
pub mod agent_runtime_preferences;
```

and export the types from the module's public exports:

```rust
pub use agent_runtime_preferences::{AgentRuntimePreferenceRecord, NewAgentRuntimePreference};
```

In `crates/noema-core/src/lib.rs`, add to the `pub use store::{...}` list:

```rust
AgentRuntimePreferenceRecord, NewAgentRuntimePreference,
```

- [ ] **Step 6: Run tests for this task**

Run:

```bash
cargo test -p noema-core store::tests::agent_runtime_preference -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit Task 2**

```bash
git add crates/noema-core/src/store/schema.rs crates/noema-core/src/store/agent_runtime_preferences.rs crates/noema-core/src/store/mod.rs crates/noema-core/src/store/tests.rs crates/noema-core/src/lib.rs
git commit -m "Add agent runtime preferences"
```

---

### Task 3: GraphQL Agent Model Preference API

**Files:**
- Modify: `crates/noema-core/src/graphql/agents.rs`
- Modify: `crates/noema-core/src/graphql/provider_accounts.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/web/src/graphql/operations.ts`

- [ ] **Step 1: Write GraphQL tests for provider listing and agent options**

Add to `crates/noema-core/src/graphql/schema.rs` tests:

```rust
#[tokio::test]
async fn provider_accounts_query_returns_all_active_default_accounts() {
    use crate::store::tests::test_store;

    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");

    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let response = schema
        .execute(async_graphql::Request::new(
            r#"
            {
              providerAccounts {
                providerKind
                accountKey
                displayName
                authMethod
              }
            }
            "#,
        ))
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let accounts = response.data.into_json().expect("json")["providerAccounts"]
        .as_array()
        .expect("accounts")
        .clone();
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0]["providerKind"], "codex");
    assert_eq!(accounts[1]["providerKind"], "foundation_local");
    assert_eq!(accounts[1]["authMethod"], "none");
}

#[tokio::test]
async fn agents_query_exposes_model_preference_options() {
    use crate::store::tests::test_store;

    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    let foundation = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: foundation.provider_account_id,
            model_profile: "default".to_string(),
        })
        .await
        .expect("preference");

    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let response = schema
        .execute(async_graphql::Request::new(
            r#"
            {
              agents {
                agentId
                modelPreference {
                  providerKind
                  providerAccountId
                  modelProfile
                }
                modelOptions {
                  providerKind
                  providerAccountId
                  providerDisplayName
                  status
                  profiles {
                    id
                    label
                    disabledReason
                  }
                }
              }
            }
            "#,
        ))
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let agent = &response.data.into_json().expect("json")["agents"][0];
    assert_eq!(agent["modelPreference"]["providerKind"], "foundation_local");
    assert_eq!(agent["modelPreference"]["modelProfile"], "default");
    assert_eq!(agent["modelOptions"][1]["providerKind"], "foundation_local");
    assert_eq!(agent["modelOptions"][1]["profiles"][0]["label"], "Default on-device");
}
```

- [ ] **Step 2: Run GraphQL tests and verify they fail**

Run:

```bash
cargo test -p noema-core graphql::schema::tests::provider_accounts_query_returns_all_active_default_accounts graphql::schema::tests::agents_query_exposes_model_preference_options -- --nocapture
```

Expected: FAIL because provider listing is Codex-only and agent model fields do not exist.

- [ ] **Step 3: Update provider account resolver**

In `crates/noema-core/src/graphql/provider_accounts.rs`, replace the resolver body:

```rust
pub(super) async fn provider_accounts(state: &GraphqlState) -> Result<Vec<GraphqlProviderAccount>> {
    let store = state.store()?;
    let accounts = store
        .active_default_provider_accounts()
        .await
        .map_err(graphql_error)?;
    Ok(accounts.into_iter().map(Into::into).collect())
}
```

- [ ] **Step 4: Add GraphQL model preference types**

In `crates/noema-core/src/graphql/agents.rs`, add imports:

```rust
use async_graphql::{InputObject, Result, SimpleObject};
use serde_json::Value;

use crate::{
    AgentRecord, AgentRuntimePreferenceRecord, NewAgentRuntimePreference, ProviderAccountRecord,
    ProviderAccountStatus,
};
```

Add types:

```rust
/// Agent model preference safe to expose in Settings.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlAgentModelPreference {
    /// Provider kind selected for this agent.
    pub provider_kind: String,
    /// Provider account id selected for this agent.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
}

/// One selectable model/profile.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlAgentModelProfileOption {
    /// Stable profile or model id.
    pub id: String,
    /// User-facing label.
    pub label: String,
    /// Why this option is disabled, when unavailable.
    pub disabled_reason: Option<String>,
}

/// One selectable provider account and its profiles.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlAgentModelProviderOption {
    /// Provider kind.
    pub provider_kind: String,
    /// Provider account id.
    pub provider_account_id: String,
    /// User-facing provider display name.
    pub provider_display_name: String,
    /// Provider account status.
    pub status: super::onboarding::GraphqlProviderAccountStatus,
    /// Available profiles or model ids.
    pub profiles: Vec<GraphqlAgentModelProfileOption>,
    /// Why this provider is disabled, when unavailable.
    pub disabled_reason: Option<String>,
}

/// Input for saving an agent model preference.
#[derive(Clone, Debug, InputObject)]
pub struct GraphqlSaveAgentModelPreferenceInput {
    /// Agent to update.
    pub agent_id: String,
    /// Provider account id to use.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
}
```

Extend `GraphqlAgent`:

```rust
    /// Current model preference, when configured.
    pub model_preference: Option<GraphqlAgentModelPreference>,
    /// Provider/profile options available to this agent.
    pub model_options: Vec<GraphqlAgentModelProviderOption>,
```

Replace the `impl From<AgentRecord>` with an explicit constructor:

```rust
impl GraphqlAgent {
    fn from_parts(
        agent: AgentRecord,
        preference: Option<AgentRuntimePreferenceRecord>,
        accounts: &[ProviderAccountRecord],
    ) -> Self {
        let is_primary = agent.agent_id == "agent:primary";
        Self {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
            is_primary,
            model_preference: preference.map(|preference| GraphqlAgentModelPreference {
                provider_kind: preference.provider_kind,
                provider_account_id: preference.provider_account_id,
                model_profile: preference.model_profile,
            }),
            model_options: accounts.iter().map(option_from_account).collect(),
        }
    }
}
```

Add helper functions:

```rust
fn option_from_account(account: &ProviderAccountRecord) -> GraphqlAgentModelProviderOption {
    let disabled_reason = provider_disabled_reason(account);
    GraphqlAgentModelProviderOption {
        provider_kind: account.provider_kind.clone(),
        provider_account_id: account.provider_account_id.clone(),
        provider_display_name: account.display_name.clone(),
        status: account.status.into(),
        profiles: profiles_from_account(account, disabled_reason.as_deref()),
        disabled_reason,
    }
}

fn provider_disabled_reason(account: &ProviderAccountRecord) -> Option<String> {
    match account.status {
        ProviderAccountStatus::Authenticated => None,
        ProviderAccountStatus::Unknown if account.provider_kind == "foundation_local" => None,
        ProviderAccountStatus::Unknown => Some("Provider status has not been checked.".to_string()),
        ProviderAccountStatus::Checking => Some("Provider status is still checking.".to_string()),
        ProviderAccountStatus::Unauthenticated => Some("Provider account is not authenticated.".to_string()),
        ProviderAccountStatus::Unavailable => account
            .last_error_message
            .clone()
            .or_else(|| Some("Provider is unavailable on this machine.".to_string())),
    }
}

fn profiles_from_account(
    account: &ProviderAccountRecord,
    disabled_reason: Option<&str>,
) -> Vec<GraphqlAgentModelProfileOption> {
    let metadata_profiles = metadata_profiles(&account.metadata, disabled_reason);
    if !metadata_profiles.is_empty() {
        return metadata_profiles;
    }
    match account.provider_kind.as_str() {
        "foundation_local" => vec![GraphqlAgentModelProfileOption {
            id: "default".to_string(),
            label: "Default on-device".to_string(),
            disabled_reason: disabled_reason.map(ToString::to_string),
        }],
        "codex" => vec![GraphqlAgentModelProfileOption {
            id: "gpt-5.5".to_string(),
            label: "gpt-5.5".to_string(),
            disabled_reason: disabled_reason.map(ToString::to_string),
        }],
        "openai" => vec![GraphqlAgentModelProfileOption {
            id: "gpt-5.5".to_string(),
            label: "gpt-5.5".to_string(),
            disabled_reason: disabled_reason.map(ToString::to_string),
        }],
        _ => Vec::new(),
    }
}

fn metadata_profiles(
    metadata: &Value,
    disabled_reason: Option<&str>,
) -> Vec<GraphqlAgentModelProfileOption> {
    metadata
        .get("profiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|profile| {
            let id = profile.get("id")?.as_str()?.trim();
            if id.is_empty() {
                return None;
            }
            let label = profile
                .get("label")
                .and_then(Value::as_str)
                .filter(|label| !label.trim().is_empty())
                .unwrap_or(id);
            Some(GraphqlAgentModelProfileOption {
                id: id.to_string(),
                label: label.to_string(),
                disabled_reason: disabled_reason.map(ToString::to_string),
            })
        })
        .collect()
}
```

Update `agents`:

```rust
pub(super) async fn agents(state: &GraphqlState) -> Result<Vec<GraphqlAgent>> {
    let store = state.store()?;
    let agents = store.list_agents().await.map_err(graphql_error)?;
    let accounts = store
        .active_default_provider_accounts()
        .await
        .map_err(graphql_error)?;
    let mut output = Vec::with_capacity(agents.len());
    for agent in agents {
        let preference = store
            .get_agent_runtime_preference(&agent.agent_id)
            .await
            .map_err(graphql_error)?;
        output.push(GraphqlAgent::from_parts(agent, preference, &accounts));
    }
    Ok(output)
}
```

Add mutation function:

```rust
pub(super) async fn save_agent_model_preference(
    state: &GraphqlState,
    input: GraphqlSaveAgentModelPreferenceInput,
) -> Result<GraphqlAgentModelPreference> {
    let store = state.store()?;
    let account = store
        .get_provider_account(&input.provider_account_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("provider account not found"))?;
    let profiles = profiles_from_account(&account, None);
    if !profiles.iter().any(|profile| profile.id == input.model_profile) {
        return Err(async_graphql::Error::new("model profile is not available for provider"));
    }
    let saved = store
        .upsert_agent_runtime_preference(NewAgentRuntimePreference {
            agent_id: input.agent_id,
            provider_kind: account.provider_kind,
            provider_account_id: account.provider_account_id,
            model_profile: input.model_profile,
        })
        .await
        .map_err(graphql_error)?;
    Ok(GraphqlAgentModelPreference {
        provider_kind: saved.provider_kind,
        provider_account_id: saved.provider_account_id,
        model_profile: saved.model_profile,
    })
}
```

- [ ] **Step 5: Wire schema mutation and SDL assertions**

In `crates/noema-core/src/graphql/schema.rs`, import the new types:

```rust
agents::{
    self, GraphqlAgent, GraphqlAgentModelPreference, GraphqlAgentModelProfileOption,
    GraphqlAgentModelProviderOption, GraphqlSaveAgentModelPreferenceInput,
},
```

Add mutation:

```rust
    /// Save one agent's model/provider preference.
    async fn save_agent_model_preference(
        &self,
        ctx: &Context<'_>,
        input: GraphqlSaveAgentModelPreferenceInput,
    ) -> Result<GraphqlAgentModelPreference> {
        let state = ctx.data_unchecked::<GraphqlState>();
        agents::save_agent_model_preference(state, input).await
    }
```

Add SDL assertions:

```rust
        assert!(sdl.contains("saveAgentModelPreference"));
        assert!(sdl.contains("type GraphqlAgentModelPreference"));
        assert!(sdl.contains("type GraphqlAgentModelProviderOption"));
        assert!(sdl.contains("type GraphqlAgentModelProfileOption"));
```

- [ ] **Step 6: Update frontend operations**

In `crates/noema-core/web/src/graphql/operations.ts`, replace the `AgentsDocument` selection:

```ts
export const AgentsDocument = gql`
  query Agents {
    agents {
      agentId
      displayName
      isPrimary
      modelPreference {
        providerKind
        providerAccountId
        modelProfile
      }
      modelOptions {
        providerKind
        providerAccountId
        providerDisplayName
        status
        disabledReason
        profiles {
          id
          label
          disabledReason
        }
      }
    }
  }
`;
```

Add:

```ts
export const SaveAgentModelPreferenceDocument = gql`
  mutation SaveAgentModelPreference($input: GraphqlSaveAgentModelPreferenceInput!) {
    saveAgentModelPreference(input: $input) {
      providerKind
      providerAccountId
      modelProfile
    }
  }
`;
```

- [ ] **Step 7: Run GraphQL tests**

Run:

```bash
cargo test -p noema-core graphql::schema::tests::provider_accounts_query_returns_all_active_default_accounts graphql::schema::tests::agents_query_exposes_model_preference_options -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Regenerate frontend GraphQL types**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: generated `src/generated/graphql.ts` includes `SaveAgentModelPreferenceDocument` and new agent model fields.

- [ ] **Step 9: Commit Task 3**

```bash
git add crates/noema-core/src/graphql/agents.rs crates/noema-core/src/graphql/provider_accounts.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/generated/graphql.ts
git commit -m "Expose agent model preferences"
```

---

### Task 4: Web Dashboard Model Selection UI

**Files:**
- Modify: `crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/components/settings/agentMetadata.ts`
- Modify: `crates/noema-core/web/src/components/settings/ProvidersSettingsPaneContent.tsx`

- [ ] **Step 1: Update agent metadata helpers**

In `crates/noema-core/web/src/components/settings/agentMetadata.ts`, replace the file with:

```ts
type AgentLike = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean | null;
  modelPreference?: {
    providerKind: string;
    providerAccountId: string;
    modelProfile: string;
  } | null;
  modelOptions?: readonly {
    providerKind: string;
    providerAccountId: string;
    providerDisplayName: string;
    status: string;
    disabledReason?: string | null;
    profiles: readonly {
      id: string;
      label: string;
      disabledReason?: string | null;
    }[];
  }[];
};

export type AgentMetadataRow = {
  label: string;
  value: string;
};

export function agentDisplayName(agent: Pick<AgentLike, "displayName">) {
  return agent.displayName?.trim() || "Unnamed agent";
}

export function agentBadgeLabel(agent: Pick<AgentLike, "isPrimary">) {
  return agent.isPrimary ? "Primary" : null;
}

export function selectedModelLabel(agent: AgentLike) {
  const preference = agent.modelPreference;
  if (!preference) {
    return "System default";
  }
  const provider = agent.modelOptions?.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  const profile = provider?.profiles.find((candidate) => candidate.id === preference.modelProfile);
  return profile?.label ?? preference.modelProfile;
}

export function selectedProviderLabel(agent: AgentLike) {
  const preference = agent.modelPreference;
  if (!preference) {
    return "System default";
  }
  const provider = agent.modelOptions?.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  return provider?.providerDisplayName ?? preference.providerKind;
}

export function selectedModelWarning(agent: AgentLike) {
  const preference = agent.modelPreference;
  if (!preference) {
    return null;
  }
  const provider = agent.modelOptions?.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  const profile = provider?.profiles.find((candidate) => candidate.id === preference.modelProfile);
  return profile?.disabledReason ?? provider?.disabledReason ?? null;
}

export function agentMetadataRows(agent: AgentLike): AgentMetadataRow[] {
  return [
    { label: "Agent id", value: agent.agentId },
    { label: "Provider", value: selectedProviderLabel(agent) },
    { label: "Model", value: selectedModelLabel(agent) }
  ];
}
```

- [ ] **Step 2: Wire mutation in the container component**

In `crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx`, replace the file with:

```tsx
import { useMutation, useQuery } from "@apollo/client/react";
import {
  AgentsDocument,
  SaveAgentModelPreferenceDocument,
  type AgentsQuery,
  type SaveAgentModelPreferenceMutation,
  type SaveAgentModelPreferenceMutationVariables
} from "@/generated/graphql";
import { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export function AgentsSettingsPane() {
  const result = useQuery<AgentsQuery>(AgentsDocument, { fetchPolicy: "cache-and-network" });
  const [savePreference, saveResult] = useMutation<
    SaveAgentModelPreferenceMutation,
    SaveAgentModelPreferenceMutationVariables
  >(SaveAgentModelPreferenceDocument, {
    refetchQueries: [{ query: AgentsDocument }]
  });

  return (
    <AgentsSettingsPaneContent
      agents={result.data?.agents ?? []}
      loading={result.loading}
      error={result.error?.message ?? null}
      saving={saveResult.loading}
      saveError={saveResult.error?.message ?? null}
      onSaveModelPreference={(input) => savePreference({ variables: { input } })}
    />
  );
}
```

- [ ] **Step 3: Replace the Agents pane content with editable controls**

In `crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx`, replace the file with:

```tsx
import { useMemo, useState } from "react";
import { Settings2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  agentBadgeLabel,
  agentDisplayName,
  agentMetadataRows,
  selectedModelWarning
} from "./agentMetadata";

type AgentModelProfileOption = {
  id: string;
  label: string;
  disabledReason?: string | null;
};

type AgentModelProviderOption = {
  providerKind: string;
  providerAccountId: string;
  providerDisplayName: string;
  status: string;
  disabledReason?: string | null;
  profiles: readonly AgentModelProfileOption[];
};

export type AgentSettingsAgent = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean;
  modelPreference?: {
    providerKind: string;
    providerAccountId: string;
    modelProfile: string;
  } | null;
  modelOptions?: readonly AgentModelProviderOption[];
};

type SaveAgentModelPreferenceInput = {
  agentId: string;
  providerAccountId: string;
  modelProfile: string;
};

export function AgentsSettingsPaneContent({
  agents,
  loading,
  error,
  saving,
  saveError,
  onSaveModelPreference
}: {
  agents: readonly AgentSettingsAgent[];
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveModelPreference: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
}) {
  const [editingAgentId, setEditingAgentId] = useState<string | null>(null);

  if (loading) {
    return <p className="m-0 text-sm text-muted-foreground">Loading agents...</p>;
  }

  if (error) {
    return (
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">Agent metadata could not be loaded.</p>
      </div>
    );
  }

  if (agents.length === 0) {
    return (
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">No agents were found.</p>
      </div>
    );
  }

  return (
    <div className="grid gap-3">
      {saveError ? (
        <p className="m-0 rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive">
          Noema could not save the model choice.
        </p>
      ) : null}
      {agents.map((agent) => {
        const badgeLabel = agentBadgeLabel(agent);
        const rows = agentMetadataRows(agent);
        const warning = selectedModelWarning(agent);
        const editing = editingAgentId === agent.agentId;
        return (
          <article
            key={agent.agentId}
            className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4"
          >
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex flex-wrap items-center gap-3">
                <h2 className="m-0 font-heading text-xl leading-tight tracking-normal text-foreground">
                  {agentDisplayName(agent)}
                </h2>
                {badgeLabel ? <Badge variant="outline">{badgeLabel}</Badge> : null}
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => setEditingAgentId(editing ? null : agent.agentId)}
              >
                <Settings2 className="size-4" aria-hidden="true" />
                Model
              </Button>
            </div>
            <dl className="m-0 grid gap-2">
              {rows.map((row) => (
                <div
                  key={row.label}
                  className="grid grid-cols-[minmax(120px,180px)_1fr] gap-4 max-[760px]:grid-cols-1 max-[760px]:gap-1"
                >
                  <dt className="text-sm font-medium text-muted-foreground">{row.label}</dt>
                  <dd className="m-0 min-w-0 break-words font-mono text-sm text-foreground">
                    {row.value}
                  </dd>
                </div>
              ))}
            </dl>
            {warning ? <p className="m-0 text-sm text-amber-700">{warning}</p> : null}
            {editing ? (
              <AgentModelPreferenceEditor
                agent={agent}
                saving={saving}
                onCancel={() => setEditingAgentId(null)}
                onSave={async (input) => {
                  await onSaveModelPreference(input);
                  setEditingAgentId(null);
                }}
              />
            ) : null}
          </article>
        );
      })}
    </div>
  );
}

function AgentModelPreferenceEditor({
  agent,
  saving,
  onCancel,
  onSave
}: {
  agent: AgentSettingsAgent;
  saving: boolean;
  onCancel: () => void;
  onSave: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
}) {
  const options = agent.modelOptions ?? [];
  const initialProvider = agent.modelPreference?.providerAccountId ?? options[0]?.providerAccountId ?? "";
  const [providerAccountId, setProviderAccountId] = useState(initialProvider);
  const selectedProvider = useMemo(
    () => options.find((option) => option.providerAccountId === providerAccountId) ?? options[0],
    [options, providerAccountId]
  );
  const initialProfile =
    agent.modelPreference?.modelProfile ?? selectedProvider?.profiles[0]?.id ?? "";
  const [modelProfile, setModelProfile] = useState(initialProfile);

  const profiles = selectedProvider?.profiles ?? [];
  const selectedProfileAvailable = profiles.some((profile) => profile.id === modelProfile);
  const effectiveProfile = selectedProfileAvailable ? modelProfile : profiles[0]?.id ?? "";
  const canSave = Boolean(providerAccountId && effectiveProfile && !saving);

  return (
    <form
      className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-[var(--surface-muted)] p-3"
      onSubmit={(event) => {
        event.preventDefault();
        if (canSave) {
          void onSave({
            agentId: agent.agentId,
            providerAccountId,
            modelProfile: effectiveProfile
          });
        }
      }}
    >
      <label className="grid gap-1 text-sm">
        <span className="font-medium text-foreground">Provider</span>
        <select
          className="h-9 rounded-md border border-[var(--border-subtle)] bg-white px-2 text-sm"
          value={providerAccountId}
          onChange={(event) => {
            const nextProvider = options.find(
              (option) => option.providerAccountId === event.target.value
            );
            setProviderAccountId(event.target.value);
            setModelProfile(nextProvider?.profiles[0]?.id ?? "");
          }}
        >
          {options.map((option) => (
            <option
              key={option.providerAccountId}
              value={option.providerAccountId}
              disabled={Boolean(option.disabledReason)}
            >
              {option.providerDisplayName}
            </option>
          ))}
        </select>
      </label>
      <label className="grid gap-1 text-sm">
        <span className="font-medium text-foreground">Model</span>
        <select
          className="h-9 rounded-md border border-[var(--border-subtle)] bg-white px-2 text-sm"
          value={effectiveProfile}
          onChange={(event) => setModelProfile(event.target.value)}
        >
          {profiles.map((profile) => (
            <option key={profile.id} value={profile.id} disabled={Boolean(profile.disabledReason)}>
              {profile.label}
            </option>
          ))}
        </select>
      </label>
      {selectedProvider?.disabledReason ? (
        <p className="m-0 text-sm text-amber-700">{selectedProvider.disabledReason}</p>
      ) : null}
      <div className="flex flex-wrap justify-end gap-2">
        <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit" size="sm" disabled={!canSave}>
          Save
        </Button>
      </div>
    </form>
  );
}
```

- [ ] **Step 4: Ensure Providers pane remains status-only**

Inspect `crates/noema-core/web/src/components/settings/ProvidersSettingsPaneContent.tsx`. Do not add model selects. Add a short local-provider status line for `foundation_local` inside the current provider summary:

```tsx
{account.providerKind === "foundation_local" ? (
  <p className="m-0 text-sm text-muted-foreground">
    Local Apple model support is managed by this machine. Choose the model for each agent in Agents.
  </p>
) : null}
```

- [ ] **Step 5: Run web checks**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 6: Commit Task 4**

```bash
git add crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx crates/noema-core/web/src/components/settings/agentMetadata.ts crates/noema-core/web/src/components/settings/ProvidersSettingsPaneContent.tsx
git commit -m "Add agent model selection UI"
```

---

### Task 5: Bridge Protocol And Portable Foundation Adapter Stub

**Files:**
- Create: `crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs`
- Create: `crates/noema-core/src/provider/adapters/foundation_local.rs`
- Modify: `crates/noema-core/src/provider/adapters.rs`
- Modify: `crates/noema-core/src/lib.rs`

- [ ] **Step 1: Write protocol tests**

Create `crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn handshake_request_serializes_as_json_line_payload() {
        let message = BridgeRequest {
            id: "request-1".to_string(),
            payload: BridgeRequestPayload::Handshake {
                protocol_version: BRIDGE_PROTOCOL_VERSION,
            },
        };

        let value = serde_json::to_value(&message).expect("json");

        assert_eq!(value["id"], "request-1");
        assert_eq!(value["payload"]["type"], "handshake");
        assert_eq!(value["payload"]["protocol_version"], BRIDGE_PROTOCOL_VERSION);
    }

    #[test]
    fn generate_delta_response_parses() {
        let value = json!({
            "id": "request-2",
            "payload": {
                "type": "assistant_text_delta",
                "delta": "hello"
            }
        });

        let response: BridgeResponse = serde_json::from_value(value).expect("response");

        assert_eq!(response.id, "request-2");
        assert!(matches!(
            response.payload,
            BridgeResponsePayload::AssistantTextDelta { ref delta } if delta == "hello"
        ));
    }
}
```

- [ ] **Step 2: Run protocol tests and verify they fail**

Run:

```bash
cargo test -p noema-core provider::adapters::foundation_bridge_protocol::tests -- --nocapture
```

Expected: FAIL because protocol types are not defined yet.

- [ ] **Step 3: Add protocol types**

Put this above the tests in `foundation_bridge_protocol.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Bridge protocol version supported by this Noema build.
pub const BRIDGE_PROTOCOL_VERSION: u32 = 1;

/// Request sent from Rust to the Swift bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeRequest {
    /// Stable request id.
    pub id: String,
    /// Request payload.
    pub payload: BridgeRequestPayload,
}

/// Request payload sent from Rust to the Swift bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeRequestPayload {
    /// Version/capability handshake.
    Handshake {
        /// Protocol version requested by Rust.
        protocol_version: u32,
    },
    /// Health/capability check.
    Health,
    /// Create a session for a Noema conversation.
    CreateSession {
        /// Noema conversation id.
        conversation_id: String,
        /// Model/profile id.
        model_profile: String,
        /// Optional instructions.
        instructions: Option<String>,
    },
    /// Replay prior turns into a session.
    ReplayTurns {
        /// Bridge session id.
        session_id: String,
        /// Prior turns.
        turns: Vec<BridgeReplayTurn>,
    },
    /// Generate the next assistant response.
    Generate {
        /// Bridge session id.
        session_id: String,
        /// User input text.
        input: String,
    },
    /// Cancel an in-flight request.
    Cancel {
        /// Request id to cancel.
        request_id: String,
    },
    /// Close a bridge session.
    CloseSession {
        /// Bridge session id.
        session_id: String,
    },
    /// Ask the bridge to shut down.
    Shutdown,
}

/// Replayable turn sent to the bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeReplayTurn {
    /// Role visible to the model.
    pub role: BridgeRole,
    /// Text content.
    pub text: String,
}

/// Bridge-visible turn role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeRole {
    /// User turn.
    User,
    /// Assistant turn.
    Assistant,
}

/// Response sent from the Swift bridge to Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeResponse {
    /// Matching request id.
    pub id: String,
    /// Response payload.
    pub payload: BridgeResponsePayload,
}

/// Response payload sent from the Swift bridge to Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeResponsePayload {
    /// Handshake succeeded.
    HandshakeOk {
        /// Bridge protocol version.
        protocol_version: u32,
    },
    /// Health/capability state.
    Health {
        /// Whether generation is available.
        available: bool,
        /// Profile options.
        profiles: Vec<BridgeProfile>,
        /// Safe unavailable reason.
        unavailable_reason: Option<String>,
    },
    /// Session creation succeeded.
    SessionCreated {
        /// Bridge session id.
        session_id: String,
    },
    /// Replay completed.
    ReplayComplete,
    /// Assistant text delta.
    AssistantTextDelta {
        /// Delta text.
        delta: String,
    },
    /// Final assistant text.
    GenerateComplete {
        /// Complete text.
        text: String,
    },
    /// Request failed.
    Error {
        /// Stable safe error code.
        code: String,
        /// Safe error message.
        message: String,
    },
}

/// Bridge-reported profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeProfile {
    /// Stable profile id.
    pub id: String,
    /// User-facing label.
    pub label: String,
}
```

- [ ] **Step 4: Create portable adapter stub tests**

Create `crates/noema-core/src/provider/adapters/foundation_local.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GenerateRequest, ModelProvider, ProviderError};

    #[tokio::test]
    async fn unavailable_stub_fails_cleanly() {
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: None,
        })
        .expect("provider");

        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .expect_err("stub should be unavailable");

        assert!(matches!(error, ProviderError::ProviderUnavailable { .. }));
    }
}
```

- [ ] **Step 5: Implement portable adapter stub**

Add above the tests:

```rust
use crate::{
    FoundationLocalProviderConfig,
    provider::{
        GenerateRequest, GenerateResponse, GenerateStreamEvent, ModelProvider, ProviderError,
    },
};

/// Provider identifier for Apple Foundation Models.
pub const FOUNDATION_LOCAL_PROVIDER: &str = "foundation_local";

/// Apple Foundation Models provider facade.
#[derive(Debug, Clone)]
pub struct FoundationLocalProvider {
    config: FoundationLocalProviderConfig,
}

impl FoundationLocalProvider {
    /// Build a Foundation Local provider.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration is invalid.
    pub fn new(config: FoundationLocalProviderConfig) -> Result<Self, ProviderError> {
        if config.default_profile.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "foundation local default profile cannot be empty".to_string(),
            });
        }
        Ok(Self { config })
    }
}

impl ModelProvider for FoundationLocalProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let mut ignore_event = |_| {};
        self.generate_streaming(request, &mut ignore_event).await
    }

    async fn generate_streaming<'a>(
        &'a self,
        _request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        let _ = &self.config;
        Err(ProviderError::ProviderUnavailable {
            message: "Apple Foundation Models are unavailable on this platform.".to_string(),
        })
    }
}
```

If `ProviderError::ProviderUnavailable` does not exist, add it to `crates/noema-core/src/provider/contract.rs` with this shape:

```rust
    /// Provider is unavailable on this machine or runtime.
    #[error("provider unavailable: {message}")]
    ProviderUnavailable {
        /// Safe user-facing message.
        message: String,
    },
```

- [ ] **Step 6: Wire module exports**

In `crates/noema-core/src/provider/adapters.rs`, add:

```rust
/// Apple Foundation Models bridge protocol.
pub mod foundation_bridge_protocol;
/// Apple Foundation Models local provider adapter.
pub mod foundation_local;
```

In `crates/noema-core/src/lib.rs`, export:

```rust
foundation_local::FoundationLocalProvider,
```

- [ ] **Step 7: Run tests for this task**

Run:

```bash
cargo test -p noema-core provider::adapters::foundation_bridge_protocol::tests provider::adapters::foundation_local::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit Task 5**

```bash
git add crates/noema-core/src/provider/adapters.rs crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs crates/noema-core/src/provider/adapters/foundation_local.rs crates/noema-core/src/provider/contract.rs crates/noema-core/src/lib.rs
git commit -m "Add foundation bridge protocol"
```

---

### Task 6: Runtime Provider Selection And Agent Preference Resolution

**Files:**
- Modify: `crates/noema-core/src/config.rs`
- Modify: `crates/noema-core/src/runtime_host.rs`
- Modify: `crates/noema-core/src/daemon/runtime/handle.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Write daemon test for preference-selected model**

In `crates/noema-core/src/daemon/tests.rs`, add a focused test near existing runtime model tests:

```rust
#[tokio::test]
async fn primary_agent_runtime_preference_supplies_turn_model() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let account = store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: account.provider_account_id,
            model_profile: "default".to_string(),
        })
        .await
        .expect("preference");

    let provider = std::sync::Arc::new(CapturingProvider::default());
    let runtime = crate::daemon::CodexRuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");

    let started = runtime
        .start_primary_conversation(None, None)
        .await
        .expect("conversation");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "hello".to_string(), tx)
        .await
        .expect("turn");

    while rx.recv().await.is_some() {}

    let requests = provider.requests.lock().expect("requests");
    assert_eq!(requests.last().and_then(|request| request.model.as_deref()), Some("default"));
}
```

Use the repository's existing fake provider style. If there is already a request-capturing provider in the test file, reuse it rather than duplicating.

- [ ] **Step 2: Run the daemon test and verify it fails**

Run:

```bash
cargo test -p noema-core daemon::tests::primary_agent_runtime_preference_supplies_turn_model -- --nocapture
```

Expected: FAIL because turns use only the conversation model override.

- [ ] **Step 3: Generalize daemon config to selected provider**

In `crates/noema-core/src/config.rs`, change:

```rust
pub struct DaemonResolvedConfig {
    pub codex: CodexProviderConfig,
    pub web: WebConfig,
}
```

to:

```rust
pub struct DaemonResolvedConfig {
    /// Provider configuration used for daemon conversations.
    pub provider: ProviderConfig,
    /// Local web UI configuration.
    pub web: WebConfig,
}
```

Update `resolve_daemon_config`:

```rust
    fn resolve_daemon_config(self) -> Result<DaemonResolvedConfig, ConfigError> {
        let resolved = self.resolve()?;
        Ok(DaemonResolvedConfig {
            provider: resolved.provider,
            web: resolved.web,
        })
    }
```

Update callers from `.codex` to `.provider`.

- [ ] **Step 4: Add provider factory to runtime handle**

In `crates/noema-core/src/daemon/runtime/handle.rs`, add imports:

```rust
use crate::{FoundationLocalProvider, OpenAiProvider, ProviderConfig};
```

Add a spawn function:

```rust
    pub(crate) async fn spawn_from_config(
        provider_config: ProviderConfig,
        store: NoemaStore,
    ) -> Result<Self, DaemonError> {
        match provider_config {
            ProviderConfig::Codex(mut codex_config) => Self::spawn(codex_config, store).await,
            ProviderConfig::OpenAi(openai_config) => {
                let provider = Arc::new(OpenAiProvider::new(openai_config)?);
                Self::spawn_with_provider(provider, store).await
            }
            ProviderConfig::FoundationLocal(config) => {
                let provider = Arc::new(FoundationLocalProvider::new(config)?);
                Self::spawn_with_provider(provider, store).await
            }
        }
    }
```

Keep the existing `spawn(CodexProviderConfig, ...)` for tests and incremental compatibility.

- [ ] **Step 5: Update runtime host**

In `crates/noema-core/src/runtime_host.rs`, change `start` signature:

```rust
pub async fn start(provider: ProviderConfig) -> Result<Self, RuntimeHostError>
```

and runtime spawn:

```rust
let runtime = CodexRuntimeHandle::spawn_from_config(provider, store.clone())
    .await
    .map_err(|source| RuntimeHostError::Runtime(source.to_string()))?;
```

- [ ] **Step 6: Resolve agent preference in turns**

In `crates/noema-core/src/daemon/runtime/turn.rs`, before constructing `GenerateRequest`, resolve:

```rust
async fn model_for_conversation(
    store: &NoemaStore,
    conversation_model: Option<String>,
) -> Option<String> {
    if conversation_model.as_ref().is_some_and(|model| !model.trim().is_empty()) {
        return conversation_model;
    }
    store
        .get_agent_runtime_preference("agent:primary")
        .await
        .ok()
        .flatten()
        .map(|preference| preference.model_profile)
}
```

Use it when creating or using `ActiveConversation` so `conversation.model` is the resolved model/profile when no explicit conversation override exists.

- [ ] **Step 7: Run daemon tests**

Run:

```bash
cargo test -p noema-core daemon::tests::primary_agent_runtime_preference_supplies_turn_model -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Run config and runtime compile checks**

Run:

```bash
cargo check -p noema-core
```

Expected: PASS.

- [ ] **Step 9: Commit Task 6**

```bash
git add crates/noema-core/src/config.rs crates/noema-core/src/runtime_host.rs crates/noema-core/src/daemon/runtime/handle.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Resolve agent provider model preferences"
```

---

### Task 7: Bridge Lifecycle Manager With Fake Process Tests

**Files:**
- Modify: `crates/noema-core/src/provider/adapters/foundation_local.rs`
- Create: `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs`
- Modify: `crates/noema-core/src/provider/adapters.rs`

- [ ] **Step 1: Write lifecycle tests against a fake bridge command**

Create `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn missing_bridge_reports_safe_error() {
        let config = FoundationBridgeConfig {
            bridge_path: PathBuf::from("/path/that/does/not/exist"),
        };

        let error = FoundationBridgeProcess::start(config)
            .await
            .expect_err("missing bridge should fail");

        assert_eq!(error.code(), "bridge_missing");
    }
}
```

- [ ] **Step 2: Run lifecycle test and verify it fails**

Run:

```bash
cargo test -p noema-core provider::adapters::foundation_bridge_process::tests -- --nocapture
```

Expected: FAIL because lifecycle types do not exist.

- [ ] **Step 3: Implement lifecycle error and missing-binary check**

Add to `foundation_bridge_process.rs`:

```rust
use std::path::PathBuf;
use thiserror::Error;

/// Bridge process configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundationBridgeConfig {
    /// Bridge executable path.
    pub bridge_path: PathBuf,
}

/// Running bridge process handle.
#[derive(Debug)]
pub struct FoundationBridgeProcess;

/// Bridge lifecycle error.
#[derive(Debug, Error)]
pub enum FoundationBridgeError {
    /// Bridge binary is missing.
    #[error("bridge binary is missing")]
    BridgeMissing,
    /// Bridge launch failed.
    #[error("bridge launch failed: {0}")]
    BridgeLaunchFailed(String),
}

impl FoundationBridgeError {
    /// Stable safe error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::BridgeMissing => "bridge_missing",
            Self::BridgeLaunchFailed(_) => "bridge_launch_failed",
        }
    }
}

impl FoundationBridgeProcess {
    /// Start the bridge process.
    ///
    /// # Errors
    ///
    /// Returns [`FoundationBridgeError`] when the bridge cannot be launched.
    pub async fn start(config: FoundationBridgeConfig) -> Result<Self, FoundationBridgeError> {
        if !config.bridge_path.exists() {
            return Err(FoundationBridgeError::BridgeMissing);
        }
        Err(FoundationBridgeError::BridgeLaunchFailed(
            "bridge process transport is not enabled in this build".to_string(),
        ))
    }
}
```

- [ ] **Step 4: Wire module**

In `crates/noema-core/src/provider/adapters.rs`, add:

```rust
/// Apple Foundation Models bridge process lifecycle.
pub mod foundation_bridge_process;
```

- [ ] **Step 5: Run lifecycle tests**

Run:

```bash
cargo test -p noema-core provider::adapters::foundation_bridge_process::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Extend Foundation provider to use the lifecycle check when `bridge_path` is configured**

In `foundation_local.rs`, add a method:

```rust
impl FoundationLocalProvider {
    async fn ensure_available(&self) -> Result<(), ProviderError> {
        if let Some(path) = &self.config.bridge_path {
            crate::provider::adapters::foundation_bridge_process::FoundationBridgeProcess::start(
                crate::provider::adapters::foundation_bridge_process::FoundationBridgeConfig {
                    bridge_path: path.clone(),
                },
            )
            .await
            .map(|_| ())
            .map_err(|error| ProviderError::ProviderUnavailable {
                message: format!("Apple Foundation Models bridge unavailable: {}", error.code()),
            })
        } else {
            Err(ProviderError::ProviderUnavailable {
                message: "Apple Foundation Models bridge path is not configured.".to_string(),
            })
        }
    }
}
```

Call `self.ensure_available().await?;` at the top of `generate_streaming`.

- [ ] **Step 7: Run adapter tests**

Run:

```bash
cargo test -p noema-core provider::adapters::foundation_local::tests provider::adapters::foundation_bridge_process::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit Task 7**

```bash
git add crates/noema-core/src/provider/adapters.rs crates/noema-core/src/provider/adapters/foundation_local.rs crates/noema-core/src/provider/adapters/foundation_bridge_process.rs
git commit -m "Add foundation bridge lifecycle checks"
```

---

### Task 8: macOS Swift Bridge Skeleton

**Files:**
- Create: `crates/noema-core/apple-foundation-bridge/Package.swift`
- Create: `crates/noema-core/apple-foundation-bridge/Sources/NoemaFoundationBridge/main.swift`
- Create: `crates/noema-core/apple-foundation-bridge/README.md`

- [ ] **Step 1: Create Swift package manifest**

Create `crates/noema-core/apple-foundation-bridge/Package.swift`:

```swift
// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "NoemaFoundationBridge",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .executable(name: "noema-foundation-bridge", targets: ["NoemaFoundationBridge"])
    ],
    targets: [
        .executableTarget(name: "NoemaFoundationBridge")
    ]
)
```

- [ ] **Step 2: Create JSON-lines bridge skeleton**

Create `crates/noema-core/apple-foundation-bridge/Sources/NoemaFoundationBridge/main.swift`:

```swift
import Foundation

struct BridgeRequest: Decodable {
    let id: String
    let payload: Payload

    enum Payload: Decodable {
        case handshake(protocolVersion: Int)
        case health
        case shutdown
        case unsupported

        enum CodingKeys: String, CodingKey {
            case type
            case protocolVersion = "protocol_version"
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let type = try container.decode(String.self, forKey: .type)
            switch type {
            case "handshake":
                let version = try container.decode(Int.self, forKey: .protocolVersion)
                self = .handshake(protocolVersion: version)
            case "health":
                self = .health
            case "shutdown":
                self = .shutdown
            default:
                self = .unsupported
            }
        }
    }
}

func emit(_ id: String, _ payload: [String: Any]) {
    let response: [String: Any] = ["id": id, "payload": payload]
    let data = try! JSONSerialization.data(withJSONObject: response)
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([0x0A]))
}

while let line = readLine() {
    guard let data = line.data(using: .utf8) else {
        continue
    }
    guard let request = try? JSONDecoder().decode(BridgeRequest.self, from: data) else {
        emit("unknown", ["type": "error", "code": "malformed_request", "message": "Malformed bridge request."])
        continue
    }
    switch request.payload {
    case .handshake(let protocolVersion):
        emit(request.id, ["type": "handshake_ok", "protocol_version": protocolVersion])
    case .health:
        emit(request.id, [
            "type": "health",
            "available": false,
            "profiles": [["id": "default", "label": "Default on-device"]],
            "unavailable_reason": "Foundation Models runtime is not wired in this bridge build."
        ])
    case .shutdown:
        emit(request.id, ["type": "error", "code": "shutdown", "message": "Bridge shutting down."])
        exit(0)
    case .unsupported:
        emit(request.id, ["type": "error", "code": "unsupported_request", "message": "Unsupported bridge request."])
    }
}
```

- [ ] **Step 3: Add bridge README**

Create `crates/noema-core/apple-foundation-bridge/README.md`:

```markdown
# Noema Foundation Bridge

This Swift package is the macOS-only helper process for the `foundation_local`
provider. It is not a Cargo workspace member and is not required for Linux or
Windows builds.

Manual development build on a supported macOS toolchain:

```bash
cd crates/noema-core/apple-foundation-bridge
swift build
```

The Rust daemon owns this process lifecycle. Users should not start this bridge
manually in normal Noema use.
```

- [ ] **Step 4: Validate Rust default build is unaffected**

Run:

```bash
cargo check -p noema-core
```

Expected: PASS without invoking Swift.

- [ ] **Step 5: Optionally validate Swift on macOS**

Run only on a machine with Swift installed:

```bash
cd crates/noema-core/apple-foundation-bridge
swift build
```

Expected: PASS. If Swift is unavailable, record that the macOS-only bridge build was not run.

- [ ] **Step 6: Commit Task 8**

```bash
git add crates/noema-core/apple-foundation-bridge/Package.swift crates/noema-core/apple-foundation-bridge/Sources/NoemaFoundationBridge/main.swift crates/noema-core/apple-foundation-bridge/README.md
git commit -m "Add foundation Swift bridge skeleton"
```

---

### Task 9: Documentation And Current Context Update

**Files:**
- Modify: `docs/context/current.md`
- Modify: `docs/frontend/navigation-workflows.md`
- Modify: `docs/frontend/current-contract.md`

- [ ] **Step 1: Update durable context**

In `docs/context/current.md`, extend the existing Foundation Models bullet with:

```markdown
The first implementation adds portable provider/account and per-agent model
preference infrastructure, exposes model selection in Settings -> Agents, and
keeps the Swift bridge optional and macOS-only.
```

- [ ] **Step 2: Update frontend IA docs**

In `docs/frontend/navigation-workflows.md`, add to the Settings section:

```markdown
Settings -> Providers shows provider account setup and availability only. It
does not choose an agent's model.

Settings -> Agents shows each agent's selected provider and model/profile. The
agent row or detail editor owns provider/account and model/profile selection.
Unavailable providers stay visible with a reason so users can distinguish setup
problems from agent preference choices.
```

In `docs/frontend/current-contract.md`, add:

```markdown
Agent settings now include a model preference read/write contract:

- current provider kind
- provider account id
- model/profile id
- available provider account options
- available profile options
- disabled reasons for unavailable providers or profiles

Provider settings remain secret-free and status-only.
```

- [ ] **Step 3: Run doc checks**

Run:

```bash
git diff --check
rg -n 'T[B]D|T[O]DO|F[I]XME|\?\?\?|place[h]older|uncl[e]ar' docs/context/current.md docs/frontend/navigation-workflows.md docs/frontend/current-contract.md
```

Expected: `git diff --check` exits 0. `rg` exits 1 with no matches.

- [ ] **Step 4: Commit Task 9**

```bash
git add docs/context/current.md docs/frontend/navigation-workflows.md docs/frontend/current-contract.md
git commit -m "Document foundation model settings flow"
```

---

### Task 10: Final Validation

**Files:**
- Inspect all changed files.

- [ ] **Step 1: Check branch status**

Run:

```bash
git status --short --branch
```

Expected: branch is on `main`, with only intentional changes if any task has not yet been committed.

- [ ] **Step 2: Check whitespace**

Run:

```bash
git diff --check
```

Expected: no output.

- [ ] **Step 3: Run Rust formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 4: Run Rust workspace check**

Run:

```bash
cargo check --workspace
```

Expected: PASS on the current platform without requiring Swift or Foundation Models.

- [ ] **Step 5: Run Clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Run unit tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: PASS. If a Noema daemon/OpenAI provider test fails only because sandboxed local Unix/TCP socket binding returns `PermissionDenied`, rerun the same command with required socket permissions and report the distinction.

- [ ] **Step 7: Run frontend generated type, lint, and build checks**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 8: Inspect final staged changes before any final commit**

Run:

```bash
git status --short --branch
git diff --cached --stat
git diff --cached --name-status
```

Expected: any staged files match the final unit being committed. If all tasks were already committed, cached diff is empty.

- [ ] **Step 9: Commit remaining validation/doc fix changes**

If validation caused formatting, generated type, or doc updates:

```bash
git add <changed-files>
git commit -m "Validate foundation local provider integration"
```

Expected: commit succeeds. If there are no remaining changes, skip this step and report that all task commits are already complete.
