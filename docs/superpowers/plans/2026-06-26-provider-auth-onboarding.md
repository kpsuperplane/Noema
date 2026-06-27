# Provider Auth Onboarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add provider-generic onboarding and auth plumbing so Noema's first screen can initiate Codex device-code login, persist provider credential state under `NOEMA_HOME`, and only start chat after provider onboarding is complete.

**Architecture:** Add a provider-account registry backed by Postgres metadata and filesystem credential homes, a short-lived provider auth attempt manager with a Codex device-code adapter, and a derived onboarding service exposed through HTTP. The frontend calls onboarding first, renders an onboarding screen while blocked, and opens the chat WebSocket only after onboarding succeeds. Docker dev installs Codex so `docker compose up dev` no longer depends on a host Codex binary.

**Tech Stack:** Rust 1.96, Tokio process management, SQLx/Postgres, serde/ts-rs frontend protocol generation, React/TypeScript/Vite/Bun, Docker Compose.

---

## File Structure

- Create `crates/noema-core/src/memory_persistence/provider_accounts.rs`
  - Owns `ProviderAccountRecord`, account status/auth method enums, default account bootstrap, account status updates, and active account lookup.
- Modify `crates/noema-core/src/memory_persistence.rs`
  - Exports provider account types and includes the new module.
- Modify `crates/noema-core/src/memory_persistence/postgres_schema.rs`
  - Adds `provider_accounts` table and indexes to `POSTGRES_SCHEMA_SQL`.
- Modify `crates/noema-core/src/memory_persistence/postgres_tests.rs`
  - Adds provider account schema/repository tests.
- Modify `crates/noema-core/src/paths.rs`
  - Adds `provider_account_home(provider, account)` for credential homes.
- Create `crates/noema-core/src/provider_auth.rs`
  - Defines generic auth methods, auth attempt state, redaction, and `ProviderAuthManager`.
- Create `crates/noema-core/src/providers/codex_auth.rs`
  - Implements Codex device-code auth attempts using `codex login --device-auth` with account-specific `CODEX_HOME`.
- Create `crates/noema-core/src/onboarding.rs`
  - Computes derived onboarding status from the active provider account.
- Modify `crates/noema-core/src/lib.rs`
  - Exports onboarding/provider-auth types needed by daemon web and tests.
- Modify `crates/noema-core/src/frontend_protocol.rs`
  - Adds TypeScript-exported onboarding/provider account/auth attempt response types and provider status fields.
- Modify `crates/noema-core/src/daemon/server.rs`
  - Builds provider account registry, provider auth manager, onboarding service, and passes them into web state.
- Modify `crates/noema-core/src/daemon/web/mod.rs`
  - Adds HTTP body parsing and endpoints for onboarding/provider auth.
- Modify `crates/noema-core/src/daemon/runtime.rs`
  - Resolves the active provider account home for Codex before starting app-server.
- Modify `crates/noema-core/web/src/api.ts`
  - Adds onboarding and provider-auth fetch helpers.
- Modify `crates/noema-core/web/src/App.tsx`
  - Gates WebSocket startup on onboarding completion.
- Create `crates/noema-core/web/src/components/Onboarding.tsx`
  - Renders provider login states and triggers auth attempts.
- Modify `crates/noema-core/web/src/components/StatusCluster.tsx`
  - Shows provider status.
- Modify `crates/noema-core/web/src/styles.css`
  - Adds restrained onboarding layout.
- Modify `Dockerfile`
  - Installs Codex CLI in the dev image.
- Modify `compose.yaml`
  - Ensures `NOEMA_HOME` provider credential homes are persisted and compatible with Codex.
- Modify `README.md` and `docs/context/current.md`
  - Documents web onboarding and terminal fallback.

## Task 1: Provider Account Schema And Repository

**Files:**
- Create: `crates/noema-core/src/memory_persistence/provider_accounts.rs`
- Modify: `crates/noema-core/src/memory_persistence.rs`
- Modify: `crates/noema-core/src/memory_persistence/postgres_schema.rs`
- Test: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Write schema test for provider account table**

Add this test near the existing bootstrap schema tests in `crates/noema-core/src/memory_persistence/postgres_tests.rs`:

```rust
#[tokio::test]
async fn postgres_bootstrap_creates_provider_accounts_table() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let columns = sqlx::query_scalar::<_, String>(
        r#"
        SELECT column_name
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'provider_accounts'
        ORDER BY ordinal_position
        "#,
    )
    .fetch_all(repo.pool())
    .await
    .expect("provider account columns");

    assert_eq!(
        columns,
        [
            "provider_account_id",
            "provider_kind",
            "account_key",
            "display_name",
            "auth_method",
            "is_active",
            "is_default",
            "status",
            "last_checked_at",
            "last_authenticated_at",
            "last_error_code",
            "last_error_message",
            "metadata",
            "created_at",
            "updated_at",
            "deleted_at",
        ]
    );

    assert_index_exists(
        repo.pool(),
        "provider_accounts",
        "idx_provider_accounts_active_default",
    )
    .await;
}
```

- [ ] **Step 2: Run the schema test to verify it fails**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests::postgres_bootstrap_creates_provider_accounts_table -- --nocapture
```

Expected: FAIL because `provider_accounts` does not exist.

- [ ] **Step 3: Add the `provider_accounts` table**

In `crates/noema-core/src/memory_persistence/postgres_schema.rs`, add this table after `tools` and before `conversations`:

```sql
CREATE TABLE IF NOT EXISTS provider_accounts (
  provider_account_id TEXT PRIMARY KEY,
  provider_kind TEXT NOT NULL,
  account_key TEXT NOT NULL,
  display_name TEXT NOT NULL,
  auth_method TEXT NOT NULL
    CHECK (auth_method IN ('oauth_device_code','secret_input','external_manual','none')),
  is_active BOOLEAN NOT NULL DEFAULT true,
  is_default BOOLEAN NOT NULL DEFAULT false,
  status TEXT NOT NULL DEFAULT 'unknown'
    CHECK (status IN ('unknown','checking','authenticated','unauthenticated','unavailable')),
  last_checked_at TIMESTAMPTZ,
  last_authenticated_at TIMESTAMPTZ,
  last_error_code TEXT,
  last_error_message TEXT,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  UNIQUE (provider_kind, account_key)
);

CREATE INDEX IF NOT EXISTS idx_provider_accounts_active_default
  ON provider_accounts(provider_kind, is_active, is_default)
  WHERE deleted_at IS NULL;
```

Also add `"provider_accounts"` to the expected table list in `bootstrap_creates_core_tables`, placed after `"object_provenance_edges"` and before `"relationships"` to match alphabetical ordering.

- [ ] **Step 4: Run schema tests**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests::postgres_bootstrap_creates_provider_accounts_table memory_persistence::postgres_tests::bootstrap_creates_core_tables -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Write repository tests for default provider account**

Add these tests to `crates/noema-core/src/memory_persistence/postgres_tests.rs` after the provider-account table test:

```rust
#[tokio::test]
async fn ensure_default_provider_account_creates_codex_default() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let account = repo
        .ensure_default_provider_account()
        .await
        .expect("default provider account");

    assert_eq!(account.provider_account_id, "provider_account:codex:default");
    assert_eq!(account.provider_kind, "codex");
    assert_eq!(account.account_key, "default");
    assert_eq!(account.display_name, "Codex");
    assert_eq!(account.auth_method, crate::ProviderAuthMethod::OauthDeviceCode);
    assert!(account.is_active);
    assert!(account.is_default);
    assert_eq!(account.status, crate::ProviderAccountStatus::Unknown);
}

#[tokio::test]
async fn active_provider_account_returns_default_account() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_provider_account()
        .await
        .expect("default provider account");

    let account = repo
        .active_provider_account()
        .await
        .expect("active provider account")
        .expect("account exists");

    assert_eq!(account.provider_kind, "codex");
    assert_eq!(account.account_key, "default");
}

#[tokio::test]
async fn update_provider_account_status_records_auth_metadata() {
    let Some(repo) = test_repo().await else {
        return;
    };
    let account = repo
        .ensure_default_provider_account()
        .await
        .expect("default provider account");

    repo.update_provider_account_status(
        account.provider_account_id.as_str(),
        crate::ProviderAccountStatus::Authenticated,
        None,
        None,
    )
    .await
    .expect("status update");

    let stored = repo
        .active_provider_account()
        .await
        .expect("active provider account")
        .expect("account exists");
    assert_eq!(stored.status, crate::ProviderAccountStatus::Authenticated);
    assert!(stored.last_checked_at.is_some());
    assert!(stored.last_authenticated_at.is_some());
    assert_eq!(stored.last_error_code, None);
    assert_eq!(stored.last_error_message, None);
}
```

- [ ] **Step 6: Run repository tests to verify they fail**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests::ensure_default_provider_account_creates_codex_default memory_persistence::postgres_tests::active_provider_account_returns_default_account memory_persistence::postgres_tests::update_provider_account_status_records_auth_metadata -- --nocapture
```

Expected: FAIL because provider account types and repository methods do not exist.

- [ ] **Step 7: Create provider account repository module**

Create `crates/noema-core/src/memory_persistence/provider_accounts.rs`:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{error::MemoryPersistenceError, repository::PostgresMemoryRepository};

/// Supported provider account authentication methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthMethod {
    /// OAuth device-code login.
    OauthDeviceCode,
    /// Secret input such as an API key or access token.
    SecretInput,
    /// Manual terminal or externally managed login.
    ExternalManual,
    /// Provider does not require authentication.
    None,
}

impl ProviderAuthMethod {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::OauthDeviceCode => "oauth_device_code",
            Self::SecretInput => "secret_input",
            Self::ExternalManual => "external_manual",
            Self::None => "none",
        }
    }
}

/// Provider account readiness status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAccountStatus {
    /// Status has not been checked.
    Unknown,
    /// Status check is in progress.
    Checking,
    /// Provider credentials are usable.
    Authenticated,
    /// Provider credentials are absent, expired, revoked, or invalid.
    Unauthenticated,
    /// Provider cannot be used because its binary/service/config is unavailable.
    Unavailable,
}

impl ProviderAccountStatus {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Checking => "checking",
            Self::Authenticated => "authenticated",
            Self::Unauthenticated => "unauthenticated",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Durable non-secret provider account metadata.
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
}

impl PostgresMemoryRepository {
    /// Create or return the default active provider account.
    pub async fn ensure_default_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, MemoryPersistenceError> {
        sqlx::query(
            r#"
            INSERT INTO provider_accounts (
              provider_account_id, provider_kind, account_key, display_name,
              auth_method, is_active, is_default, status
            )
            VALUES (
              'provider_account:codex:default', 'codex', 'default', 'Codex',
              'oauth_device_code', true, true, 'unknown'
            )
            ON CONFLICT (provider_kind, account_key) DO UPDATE
              SET display_name = EXCLUDED.display_name,
                  auth_method = EXCLUDED.auth_method,
                  is_active = true,
                  is_default = true,
                  updated_at = now()
            "#,
        )
        .execute(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        self.get_provider_account("provider_account:codex:default")
            .await?
            .ok_or_else(|| MemoryPersistenceError::ProviderAccountNotFound {
                provider_account_id: "provider_account:codex:default".to_string(),
            })
    }

    /// Return the active default provider account.
    pub async fn active_provider_account(
        &self,
    ) -> Result<Option<ProviderAccountRecord>, MemoryPersistenceError> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                bool,
                bool,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Value,
            ),
        >(
            r#"
            SELECT
              provider_account_id, provider_kind, account_key, display_name,
              auth_method, is_active, is_default, status,
              last_checked_at::text, last_authenticated_at::text,
              last_error_code, last_error_message, metadata
            FROM provider_accounts
            WHERE is_active = true
              AND is_default = true
              AND deleted_at IS NULL
            ORDER BY created_at
            LIMIT 1
            "#,
        )
        .fetch_optional(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        row.map(provider_account_from_row).transpose()
    }

    /// Return one provider account by id.
    pub async fn get_provider_account(
        &self,
        provider_account_id: &str,
    ) -> Result<Option<ProviderAccountRecord>, MemoryPersistenceError> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                bool,
                bool,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Value,
            ),
        >(
            r#"
            SELECT
              provider_account_id, provider_kind, account_key, display_name,
              auth_method, is_active, is_default, status,
              last_checked_at::text, last_authenticated_at::text,
              last_error_code, last_error_message, metadata
            FROM provider_accounts
            WHERE provider_account_id = $1
              AND deleted_at IS NULL
            "#,
        )
        .bind(provider_account_id)
        .fetch_optional(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        row.map(provider_account_from_row).transpose()
    }

    /// Update safe provider account status metadata.
    pub async fn update_provider_account_status(
        &self,
        provider_account_id: &str,
        status: ProviderAccountStatus,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), MemoryPersistenceError> {
        sqlx::query(
            r#"
            UPDATE provider_accounts
            SET status = $2,
                last_checked_at = now(),
                last_authenticated_at = CASE WHEN $2 = 'authenticated' THEN now() ELSE last_authenticated_at END,
                last_error_code = $3,
                last_error_message = $4,
                updated_at = now()
            WHERE provider_account_id = $1
            "#,
        )
        .bind(provider_account_id)
        .bind(status.as_str())
        .bind(error_code)
        .bind(error_message)
        .execute(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;
        Ok(())
    }
}

fn provider_account_from_row(
    row: (
        String,
        String,
        String,
        String,
        String,
        bool,
        bool,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Value,
    ),
) -> Result<ProviderAccountRecord, MemoryPersistenceError> {
    let (
        provider_account_id,
        provider_kind,
        account_key,
        display_name,
        auth_method,
        is_active,
        is_default,
        status,
        last_checked_at,
        last_authenticated_at,
        last_error_code,
        last_error_message,
        metadata,
    ) = row;

    Ok(ProviderAccountRecord {
        provider_account_id,
        provider_kind,
        account_key,
        display_name,
        auth_method: parse_auth_method(&auth_method)?,
        is_active,
        is_default,
        status: parse_account_status(&status)?,
        last_checked_at,
        last_authenticated_at,
        last_error_code,
        last_error_message,
        metadata,
    })
}

fn parse_auth_method(value: &str) -> Result<ProviderAuthMethod, MemoryPersistenceError> {
    match value {
        "oauth_device_code" => Ok(ProviderAuthMethod::OauthDeviceCode),
        "secret_input" => Ok(ProviderAuthMethod::SecretInput),
        "external_manual" => Ok(ProviderAuthMethod::ExternalManual),
        "none" => Ok(ProviderAuthMethod::None),
        other => Err(MemoryPersistenceError::InvalidEnum {
            kind: "provider_auth_method",
            value: other.to_string(),
        }),
    }
}

fn parse_account_status(value: &str) -> Result<ProviderAccountStatus, MemoryPersistenceError> {
    match value {
        "unknown" => Ok(ProviderAccountStatus::Unknown),
        "checking" => Ok(ProviderAccountStatus::Checking),
        "authenticated" => Ok(ProviderAccountStatus::Authenticated),
        "unauthenticated" => Ok(ProviderAccountStatus::Unauthenticated),
        "unavailable" => Ok(ProviderAccountStatus::Unavailable),
        other => Err(MemoryPersistenceError::InvalidEnum {
            kind: "provider_account_status",
            value: other.to_string(),
        }),
    }
}
```

Add this variant in `crates/noema-core/src/memory_persistence/error.rs` after `MemoryNotFound`:

```rust
/// A provider account expected to exist was not found.
#[error("provider account not found: {provider_account_id}")]
ProviderAccountNotFound {
    /// Missing provider account id.
    provider_account_id: String,
},
```

- [ ] **Step 8: Export provider account types**

Modify `crates/noema-core/src/memory_persistence.rs`:

```rust
mod provider_accounts;
```

Add exports:

```rust
pub use provider_accounts::{ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod};
```

Modify `crates/noema-core/src/lib.rs` provider exports from `memory_persistence`:

```rust
ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod,
```

- [ ] **Step 9: Run repository tests**

Run:

```bash
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests::ensure_default_provider_account_creates_codex_default memory_persistence::postgres_tests::active_provider_account_returns_default_account memory_persistence::postgres_tests::update_provider_account_status_records_auth_metadata -- --nocapture
```

Expected: PASS.

- [ ] **Step 10: Commit provider account schema**

Run:

```bash
git add crates/noema-core/src/memory_persistence.rs crates/noema-core/src/memory_persistence/error.rs crates/noema-core/src/memory_persistence/provider_accounts.rs crates/noema-core/src/memory_persistence/postgres_schema.rs crates/noema-core/src/memory_persistence/postgres_tests.rs crates/noema-core/src/lib.rs
git commit -m "feat: add provider account metadata"
```

## Task 2: Credential Home Paths And Codex Account Config

**Files:**
- Modify: `crates/noema-core/src/paths.rs`
- Create: `crates/noema-core/src/provider_auth.rs`
- Modify: `crates/noema-core/src/lib.rs`

- [ ] **Step 1: Write path tests**

Add tests to `crates/noema-core/src/paths.rs`:

```rust
#[test]
fn provider_account_home_is_under_noema_providers() {
    let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

    assert_eq!(
        paths.provider_account_home("codex", "default"),
        PathBuf::from("/tmp/noema/providers/codex/default")
    );
}

#[test]
fn provider_account_home_sanitizes_segments() {
    let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

    assert_eq!(
        paths.provider_account_home("co/dex", "../default"),
        PathBuf::from("/tmp/noema/providers/co_dex/___default")
    );
}
```

- [ ] **Step 2: Run path tests to verify they fail**

Run:

```bash
cargo test -p noema-core paths::tests::provider_account_home -- --nocapture
```

Expected: FAIL because `provider_account_home` does not exist.

- [ ] **Step 3: Add credential home path helpers**

Modify `crates/noema-core/src/paths.rs`:

```rust
/// Path to the provider credential root.
#[must_use]
pub fn providers_dir(&self) -> PathBuf {
    self.root.join("providers")
}

/// Path to one provider account's credential home.
#[must_use]
pub fn provider_account_home(&self, provider_kind: &str, account_key: &str) -> PathBuf {
    self.providers_dir()
        .join(sanitize_path_segment(provider_kind))
        .join(sanitize_path_segment(account_key))
}
```

Add this helper near the bottom of the file outside the `impl`:

```rust
fn sanitize_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}
```

- [ ] **Step 4: Run path tests**

Run:

```bash
cargo test -p noema-core paths::tests::provider_account_home -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Write Codex credential-home config tests**

Create `crates/noema-core/src/provider_auth.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn ensure_codex_account_home_writes_file_credential_config() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");

        ensure_codex_account_home(&account_home).expect("account home");

        let config = fs::read_to_string(account_home.join("config.toml")).expect("config");
        assert!(config.contains("cli_auth_credentials_store = \"file\""));
    }

}
```

- [ ] **Step 6: Run provider auth tests to verify they fail**

Run:

```bash
cargo test -p noema-core provider_auth::tests -- --nocapture
```

Expected: FAIL because functions do not exist.

- [ ] **Step 7: Implement credential-home config**

Add module content above the tests in `crates/noema-core/src/provider_auth.rs`:

```rust
//! Provider authentication support.

use std::{fs, io, path::Path};

/// Prepare a Codex account home for portable file-backed credentials.
///
/// # Errors
///
/// Returns an error when the account directory or config file cannot be written.
pub fn ensure_codex_account_home(account_home: &Path) -> io::Result<()> {
    fs::create_dir_all(account_home)?;
    let config_path = account_home.join("config.toml");
    if !config_path.exists() {
        fs::write(config_path, "cli_auth_credentials_store = \"file\"\n")?;
    }
    Ok(())
}

```

Modify `crates/noema-core/src/lib.rs`:

```rust
pub mod provider_auth;
```

- [ ] **Step 8: Run provider auth and path tests**

Run:

```bash
cargo test -p noema-core paths::tests::provider_account_home provider_auth::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 9: Commit credential home helpers**

Run:

```bash
git add crates/noema-core/src/paths.rs crates/noema-core/src/provider_auth.rs crates/noema-core/src/lib.rs
git commit -m "feat: add provider credential homes"
```

## Task 3: Codex Device-Code Auth Attempt Manager

**Files:**
- Modify: `crates/noema-core/src/provider_auth.rs`
- Create: `crates/noema-core/src/providers/codex_auth.rs`
- Modify: `crates/noema-core/src/providers/mod.rs`
- Test: `crates/noema-core/src/provider_auth.rs`

- [ ] **Step 1: Add auth attempt domain tests**

Append these tests in `crates/noema-core/src/provider_auth.rs`:

```rust
#[tokio::test]
async fn auth_manager_reports_codex_device_code_progress() {
    let dir = TempDir::new().expect("temp dir");
    let fake = dir.path().join("fake-codex");
    fs::write(
        &fake,
        "#!/bin/sh\nprintf 'Open https://example.com/device and enter ABCD-EFGH\\n'\nsleep 1\nexit 0\n",
    )
    .expect("fake codex");
    make_executable(&fake);

    let manager = ProviderAuthManager::new();
    let attempt = manager
        .start_codex_device_code(CodexDeviceAuthRequest {
            provider_account_id: "provider_account:codex:default".to_string(),
            account_home: dir.path().join("providers/codex/default"),
            codex_command: fake.to_string_lossy().to_string(),
        })
        .await
        .expect("start auth");

    let status = manager
        .poll_attempt(&attempt.attempt_id)
        .await
        .expect("poll attempt")
        .expect("attempt exists");

    assert_eq!(status.status, ProviderAuthAttemptStatus::WaitingForUser);
    assert_eq!(status.verification_url.as_deref(), Some("https://example.com/device"));
    assert_eq!(status.user_code.as_deref(), Some("ABCD-EFGH"));
}

#[tokio::test]
async fn auth_manager_marks_successful_attempt_completed() {
    let dir = TempDir::new().expect("temp dir");
    let fake = dir.path().join("fake-codex");
    fs::write(
        &fake,
        "#!/bin/sh\nprintf 'Open https://example.com/device and enter ABCD-EFGH\\n'\nexit 0\n",
    )
    .expect("fake codex");
    make_executable(&fake);

    let manager = ProviderAuthManager::new();
    let attempt = manager
        .start_codex_device_code(CodexDeviceAuthRequest {
            provider_account_id: "provider_account:codex:default".to_string(),
            account_home: dir.path().join("providers/codex/default"),
            codex_command: fake.to_string_lossy().to_string(),
        })
        .await
        .expect("start auth");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let status = manager
        .poll_attempt(&attempt.attempt_id)
        .await
        .expect("poll attempt")
        .expect("attempt exists");
    assert_eq!(status.status, ProviderAuthAttemptStatus::Completed);
}

#[tokio::test]
async fn auth_manager_never_exposes_raw_codex_output() {
    let dir = TempDir::new().expect("temp dir");
    let fake = dir.path().join("fake-codex");
    fs::write(
        &fake,
        "#!/bin/sh\nprintf 'Open https://example.com/device and enter ABCD-EFGH with OPENAI_API_KEY=sk-secret at /tmp/noema/providers/codex/default/auth.json\\n'\nsleep 1\nexit 0\n",
    )
    .expect("fake codex");
    make_executable(&fake);

    let manager = ProviderAuthManager::new();
    let attempt = manager
        .start_codex_device_code(CodexDeviceAuthRequest {
            provider_account_id: "provider_account:codex:default".to_string(),
            account_home: dir.path().join("providers/codex/default"),
            codex_command: fake.to_string_lossy().to_string(),
        })
        .await
        .expect("start auth");

    let status = manager
        .poll_attempt(&attempt.attempt_id)
        .await
        .expect("poll attempt")
        .expect("attempt exists");

    assert_eq!(status.verification_url.as_deref(), Some("https://example.com/device"));
    assert_eq!(status.user_code.as_deref(), Some("ABCD-EFGH"));
    assert_eq!(status.instructions.as_deref(), Some("Complete the login in your browser."));
    let view = serde_json::to_string(&status).expect("view json");
    assert!(!view.contains("sk-secret"));
    assert!(!view.contains("auth.json"));
    assert!(!view.contains("/providers/"));
    assert!(!view.contains("OPENAI_API_KEY"));
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("permissions");
}
```

- [ ] **Step 2: Run auth attempt tests to verify they fail**

Run:

```bash
cargo test -p noema-core provider_auth::tests::auth_manager -- --nocapture
```

Expected: FAIL because `ProviderAuthManager` and request/status types do not exist.

- [ ] **Step 3: Add auth attempt types**

Add these types to `crates/noema-core/src/provider_auth.rs`:

```rust
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

/// Short-lived provider auth attempt status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ProviderAuthAttemptStatus {
    /// Attempt process is starting.
    Starting,
    /// Waiting for the user to complete an external auth step.
    WaitingForUser,
    /// Auth attempt completed successfully.
    Completed,
    /// Auth attempt failed.
    Failed,
    /// Auth attempt expired.
    Expired,
    /// Auth attempt was cancelled.
    Cancelled,
}

/// Request to start Codex device-code login.
#[derive(Debug, Clone)]
pub struct CodexDeviceAuthRequest {
    pub provider_account_id: String,
    pub account_home: PathBuf,
    pub codex_command: String,
}

/// Safe provider auth attempt state returned to the UI.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ProviderAuthAttemptView {
    pub attempt_id: String,
    pub provider_kind: String,
    pub provider_account_id: String,
    pub method: crate::ProviderAuthMethod,
    pub status: ProviderAuthAttemptStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub verification_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub user_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error_message: Option<String>,
}
```

- [ ] **Step 4: Implement the manager skeleton**

Add to `crates/noema-core/src/provider_auth.rs`:

```rust
/// In-memory short-lived provider auth attempt manager.
#[derive(Debug, Clone, Default)]
pub struct ProviderAuthManager {
    attempts: Arc<Mutex<HashMap<String, ProviderAuthAttemptView>>>,
}

impl ProviderAuthManager {
    /// Create an empty provider auth manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start Codex device-code auth.
    ///
    /// # Errors
    ///
    /// Returns an error when the Codex account home cannot be prepared or the
    /// Codex login process cannot be spawned.
    pub async fn start_codex_device_code(
        &self,
        request: CodexDeviceAuthRequest,
    ) -> Result<ProviderAuthAttemptView, crate::ProviderError> {
        crate::providers::codex_auth::start_codex_device_auth(self.clone(), request).await
    }

    /// Poll a provider auth attempt.
    ///
    /// # Errors
    ///
    /// Currently never returns an error; the result type leaves room for
    /// storage-backed managers.
    pub async fn poll_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Option<ProviderAuthAttemptView>, crate::ProviderError> {
        Ok(self.attempts.lock().await.get(attempt_id).cloned())
    }

    pub(crate) async fn upsert_attempt(&self, view: ProviderAuthAttemptView) {
        self.attempts
            .lock()
            .await
            .insert(view.attempt_id.clone(), view);
    }

    pub(crate) async fn update_attempt(
        &self,
        attempt_id: &str,
        update: impl FnOnce(&mut ProviderAuthAttemptView),
    ) {
        if let Some(view) = self.attempts.lock().await.get_mut(attempt_id) {
            update(view);
        }
    }
}
```

- [ ] **Step 5: Implement Codex auth adapter**

Create `crates/noema-core/src/providers/codex_auth.rs`:

```rust
//! Codex provider authentication adapter.

use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

use crate::{
    ProviderAuthMethod, ProviderError,
    provider_auth::{
        CodexDeviceAuthRequest, ProviderAuthAttemptStatus, ProviderAuthAttemptView,
        ProviderAuthManager, ensure_codex_account_home,
    },
};

/// Start a Codex device-code login attempt.
pub async fn start_codex_device_auth(
    manager: ProviderAuthManager,
    request: CodexDeviceAuthRequest,
) -> Result<ProviderAuthAttemptView, ProviderError> {
    ensure_codex_account_home(&request.account_home).map_err(|source| {
        ProviderError::ProviderUnavailable {
            provider: "codex".to_string(),
            message: format!("failed to prepare Codex account home: {source}"),
        }
    })?;

    let attempt_id = format!("provider_auth_attempt_{}", uuid_like_id());
    let initial = ProviderAuthAttemptView {
        attempt_id: attempt_id.clone(),
        provider_kind: "codex".to_string(),
        provider_account_id: request.provider_account_id.clone(),
        method: ProviderAuthMethod::OauthDeviceCode,
        status: ProviderAuthAttemptStatus::Starting,
        verification_url: None,
        user_code: None,
        instructions: None,
        error_code: None,
        error_message: None,
    };
    manager.upsert_attempt(initial.clone()).await;

    let mut command = Command::new(request.codex_command);
    command
        .arg("login")
        .arg("--device-auth")
        .env("CODEX_HOME", request.account_home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command.spawn().map_err(|source| ProviderError::ProviderUnavailable {
        provider: "codex".to_string(),
        message: if source.kind() == std::io::ErrorKind::NotFound {
            "codex command not found".to_string()
        } else {
            "codex login failed".to_string()
        },
    })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let manager_for_task = manager.clone();
    tokio::spawn(async move {
        if let Some(stdout) = stdout {
            read_auth_lines(&manager_for_task, &attempt_id, stdout).await;
        }
        if let Some(stderr) = stderr {
            read_auth_lines(&manager_for_task, &attempt_id, stderr).await;
        }
        match child.wait().await {
            Ok(status) if status.success() => {
                manager_for_task
                    .update_attempt(&attempt_id, |view| {
                        view.status = ProviderAuthAttemptStatus::Completed;
                    })
                    .await;
            }
            Ok(status) => {
                manager_for_task
                    .update_attempt(&attempt_id, |view| {
                        view.status = ProviderAuthAttemptStatus::Failed;
                        view.error_code = Some("codex_login_failed".to_string());
                        view.error_message = Some("codex login failed".to_string());
                    })
                    .await;
            }
            Err(source) => {
                manager_for_task
                    .update_attempt(&attempt_id, |view| {
                        view.status = ProviderAuthAttemptStatus::Failed;
                        view.error_code = Some("codex_login_wait_failed".to_string());
                        view.error_message = Some("codex login failed".to_string());
                    })
                    .await;
            }
        }
    });

    Ok(initial)
}

async fn read_auth_lines<R>(manager: &ProviderAuthManager, attempt_id: &str, reader: R)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let parsed = parse_device_auth_line(&line);
        manager
            .update_attempt(attempt_id, |view| {
                view.status = ProviderAuthAttemptStatus::WaitingForUser;
                view.instructions = Some("Complete the login in your browser.".to_string());
                if let Some(url) = parsed.0 {
                    view.verification_url = Some(url);
                }
                if let Some(code) = parsed.1 {
                    view.user_code = Some(code);
                }
            })
            .await;
    }
}

fn parse_device_auth_line(line: &str) -> (Option<String>, Option<String>) {
    let url = line
        .split_whitespace()
        .find(|part| part.starts_with("https://") || part.starts_with("http://"))
        .map(|part| part.trim_matches(|c: char| c == ',' || c == '.').to_string());
    let code = line
        .split_whitespace()
        .find(|part| part.contains('-') && part.chars().any(|c| c.is_ascii_digit()))
        .map(|part| part.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-').to_string());
    (url, code)
}

fn uuid_like_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}")
}
```

Modify `crates/noema-core/src/providers/mod.rs`:

```rust
pub mod codex_auth;
```

- [ ] **Step 6: Run auth manager tests**

Run:

```bash
cargo test -p noema-core provider_auth::tests::auth_manager -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Add cancellation test and implementation**

Extend the manager with `cancel_attempt` that marks status `Cancelled`. Add this test:

```rust
#[tokio::test]
async fn auth_manager_cancel_marks_attempt_cancelled() {
    let manager = ProviderAuthManager::new();
    let view = ProviderAuthAttemptView {
        attempt_id: "attempt_cancel".to_string(),
        provider_kind: "codex".to_string(),
        provider_account_id: "provider_account:codex:default".to_string(),
        method: crate::ProviderAuthMethod::OauthDeviceCode,
        status: ProviderAuthAttemptStatus::WaitingForUser,
        verification_url: None,
        user_code: None,
        instructions: None,
        error_code: None,
        error_message: None,
    };
    manager.upsert_attempt(view).await;

    manager.cancel_attempt("attempt_cancel").await.expect("cancel");

    let stored = manager
        .poll_attempt("attempt_cancel")
        .await
        .expect("poll")
        .expect("attempt exists");
    assert_eq!(stored.status, ProviderAuthAttemptStatus::Cancelled);
}
```

Implement:

```rust
pub async fn cancel_attempt(&self, attempt_id: &str) -> Result<(), crate::ProviderError> {
    self.update_attempt(attempt_id, |view| {
        view.status = ProviderAuthAttemptStatus::Cancelled;
    })
    .await;
    Ok(())
}
```

- [ ] **Step 8: Commit auth attempt manager**

Run:

```bash
git add crates/noema-core/src/provider_auth.rs crates/noema-core/src/providers/codex_auth.rs crates/noema-core/src/providers/mod.rs
git commit -m "feat: add provider auth attempts"
```

## Task 4: Onboarding Service And Frontend Protocol Types

**Files:**
- Create: `crates/noema-core/src/onboarding.rs`
- Modify: `crates/noema-core/src/frontend_protocol.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Test: `crates/noema-core/src/onboarding.rs`

- [ ] **Step 1: Write onboarding service tests**

Create `crates/noema-core/src/onboarding.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::{ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod};
    use serde_json::json;

    use super::*;

    fn account(status: ProviderAccountStatus) -> ProviderAccountRecord {
        ProviderAccountRecord {
            provider_account_id: "provider_account:codex:default".to_string(),
            provider_kind: "codex".to_string(),
            account_key: "default".to_string(),
            display_name: "Codex".to_string(),
            auth_method: ProviderAuthMethod::OauthDeviceCode,
            is_active: true,
            is_default: true,
            status,
            last_checked_at: None,
            last_authenticated_at: None,
            last_error_code: None,
            last_error_message: None,
            metadata: json!({}),
        }
    }

    #[test]
    fn onboarding_complete_when_provider_authenticated() {
        let status = onboarding_status_from_account(Some(account(ProviderAccountStatus::Authenticated)));

        assert!(status.is_user_onboarded);
        assert_eq!(status.steps[0].status, OnboardingStepStatus::Complete);
    }

    #[test]
    fn onboarding_blocked_when_provider_unauthenticated() {
        let status = onboarding_status_from_account(Some(account(ProviderAccountStatus::Unauthenticated)));

        assert!(!status.is_user_onboarded);
        assert_eq!(status.steps[0].id, "connect_provider_account");
        assert_eq!(status.steps[0].status, OnboardingStepStatus::Blocked);
        assert_eq!(status.steps[0].provider_kind.as_deref(), Some("codex"));
    }
}
```

- [ ] **Step 2: Run onboarding tests to verify they fail**

Run:

```bash
cargo test -p noema-core onboarding::tests -- --nocapture
```

Expected: FAIL because onboarding types do not exist.

- [ ] **Step 3: Implement onboarding types**

Add above tests in `crates/noema-core/src/onboarding.rs`:

```rust
//! Derived first-run onboarding status.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ProviderAccountRecord, ProviderAccountStatus};

/// Status for one onboarding step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum OnboardingStepStatus {
    /// Step is complete.
    Complete,
    /// Step blocks entry into chat.
    Blocked,
}

/// One onboarding step summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct OnboardingStep {
    pub id: String,
    pub status: OnboardingStepStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub provider_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub provider_account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub account_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub auth_method: Option<crate::ProviderAuthMethod>,
}

/// Derived onboarding response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct OnboardingStatus {
    pub is_user_onboarded: bool,
    pub steps: Vec<OnboardingStep>,
}

/// Build onboarding status from the active provider account.
#[must_use]
pub fn onboarding_status_from_account(account: Option<ProviderAccountRecord>) -> OnboardingStatus {
    match account {
        Some(account) if account.status == ProviderAccountStatus::Authenticated => OnboardingStatus {
            is_user_onboarded: true,
            steps: vec![provider_step(account, OnboardingStepStatus::Complete)],
        },
        Some(account) => OnboardingStatus {
            is_user_onboarded: false,
            steps: vec![provider_step(account, OnboardingStepStatus::Blocked)],
        },
        None => OnboardingStatus {
            is_user_onboarded: false,
            steps: vec![OnboardingStep {
                id: "connect_provider_account".to_string(),
                status: OnboardingStepStatus::Blocked,
                provider_kind: Some("codex".to_string()),
                provider_account_id: Some("provider_account:codex:default".to_string()),
                account_key: Some("default".to_string()),
                display_name: Some("Codex".to_string()),
                auth_method: Some(crate::ProviderAuthMethod::OauthDeviceCode),
            }],
        },
    }
}

fn provider_step(account: ProviderAccountRecord, status: OnboardingStepStatus) -> OnboardingStep {
    OnboardingStep {
        id: "connect_provider_account".to_string(),
        status,
        provider_kind: Some(account.provider_kind),
        provider_account_id: Some(account.provider_account_id),
        account_key: Some(account.account_key),
        display_name: Some(account.display_name),
        auth_method: Some(account.auth_method),
    }
}
```

Modify `crates/noema-core/src/lib.rs`:

```rust
pub mod onboarding;
pub use onboarding::{OnboardingStatus, OnboardingStep, OnboardingStepStatus, onboarding_status_from_account};
```

- [ ] **Step 4: Export TypeScript declarations**

Modify `crates/noema-core/src/frontend_protocol.rs` `generated_frontend_typescript()` declarations array to include:

```rust
exported_decl::<crate::ProviderAuthMethod>(&config),
exported_decl::<crate::ProviderAccountStatus>(&config),
exported_decl::<crate::provider_auth::ProviderAuthAttemptStatus>(&config),
exported_decl::<crate::provider_auth::ProviderAuthAttemptView>(&config),
exported_decl::<crate::OnboardingStepStatus>(&config),
exported_decl::<crate::OnboardingStep>(&config),
exported_decl::<crate::OnboardingStatus>(&config),
```

Ensure `ProviderAuthMethod` and `ProviderAccountStatus` derive `TS` in `provider_accounts.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
```

- [ ] **Step 5: Run onboarding and type generation tests**

Run:

```bash
cargo test -p noema-core onboarding::tests frontend_protocol::tests::generated_frontend_types_are_current -- --nocapture
```

Expected: frontend protocol test FAILS until generated TypeScript is refreshed.

- [ ] **Step 6: Refresh generated TypeScript**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: `src/generated/noema.ts` is updated.

- [ ] **Step 7: Run tests again**

Run:

```bash
cargo test -p noema-core onboarding::tests frontend_protocol::tests::generated_frontend_types_are_current -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit onboarding protocol types**

Run:

```bash
git add crates/noema-core/src/onboarding.rs crates/noema-core/src/frontend_protocol.rs crates/noema-core/src/lib.rs crates/noema-core/web/src/generated/noema.ts
git commit -m "feat: add onboarding status protocol"
```

## Task 5: Web HTTP Endpoints

**Files:**
- Modify: `crates/noema-core/src/daemon/server.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Test: `crates/noema-core/src/daemon/web/mod.rs`

- [ ] **Step 1: Write HTTP body parser tests**

Add tests to `crates/noema-core/src/daemon/web/mod.rs` test module:

```rust
#[tokio::test]
async fn http_request_reads_json_body() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let address = listener.local_addr().expect("address");

    let client = tokio::spawn(async move {
        let mut stream = TcpStream::connect(address).await.expect("connect");
        stream
            .write_all(
                b"POST /api/provider-auth/attempts HTTP/1.1\r\nContent-Length: 15\r\n\r\n{\"hello\":true}",
            )
            .await
            .expect("write");
    });

    let (mut stream, _) = listener.accept().await.expect("accept");
    let request = HttpRequest::read_from(&mut stream).await.expect("request");
    client.await.expect("client");

    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/api/provider-auth/attempts");
    assert_eq!(request.body, br#"{"hello":true}"#);
}
```

- [ ] **Step 2: Run parser test to verify it fails**

Run:

```bash
cargo test -p noema-core daemon::web::tests::http_request_reads_json_body -- --nocapture
```

Expected: FAIL because `HttpRequest` has no `body`.

- [ ] **Step 3: Extend `HttpRequest` with body support**

Modify `HttpRequest` in `crates/noema-core/src/daemon/web/mod.rs`:

```rust
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}
```

In `read_from`, after splitting headers:

```rust
let (header_bytes, body_start) = bytes
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .map(|index| (&bytes[..index], index + 4))
    .ok_or_else(|| DaemonError::Protocol("missing web request header terminator".to_string()))?;
let header_text = std::str::from_utf8(header_bytes)
    .map_err(|source| DaemonError::Protocol(source.to_string()))?;
```

After headers are parsed:

```rust
let content_length = headers
    .get("content-length")
    .and_then(|value| value.parse::<usize>().ok())
    .unwrap_or(0);
let mut body = bytes[body_start..].to_vec();
while body.len() < content_length {
    let read = stream.read(&mut buffer).await?;
    if read == 0 {
        break;
    }
    body.extend_from_slice(&buffer[..read]);
}
body.truncate(content_length);
```

Return `body` in the parsed request:

```rust
Ok(Self {
    method,
    path: normalized_path(path),
    headers,
    body,
})
```

- [ ] **Step 4: Add JSON helper**

Add near `write_response`:

```rust
async fn write_json<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<(), DaemonError> {
    let body = serde_json::to_vec(value).map_err(|source| DaemonError::Protocol(source.to_string()))?;
    write_response(stream, "200 OK", "application/json; charset=utf-8", &body).await
}

fn parse_json_body<T: DeserializeOwned>(request: &HttpRequest) -> Result<T, DaemonError> {
    serde_json::from_slice(&request.body).map_err(|source| DaemonError::Protocol(source.to_string()))
}
```

- [ ] **Step 5: Wire web state dependencies**

Modify `WebState` in `crates/noema-core/src/daemon/web/mod.rs`:

```rust
provider_auth: crate::provider_auth::ProviderAuthManager,
paths: crate::NoemaPaths,
codex_command: String,
```

Update `WebState::new` signature and call sites. In `crates/noema-core/src/daemon/server.rs`, build:

```rust
let paths = crate::NoemaPaths::from_process_env()?;
web_repository.ensure_default_provider_account().await?;
let provider_auth = crate::provider_auth::ProviderAuthManager::new();
let web_state = WebState::new(
    runtime,
    web_repository,
    provider_auth,
    paths,
    config.codex.command.clone(),
);
```

- [ ] **Step 6: Add endpoint request type**

In `crates/noema-core/src/frontend_protocol.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct StartProviderAuthAttemptRequest {
    pub provider_kind: String,
    pub provider_account_id: String,
    pub method: crate::ProviderAuthMethod,
}
```

Export it from `generated_frontend_typescript()`.

- [ ] **Step 7: Implement onboarding and auth endpoints**

In `handle_connection`, add before asset handling:

```rust
if request.method == "GET" && request.path == "/api/onboarding/status" {
    let account = state
        .memory_repository
        .active_provider_account()
        .await
        .map_err(|source| DaemonError::Protocol(source.to_string()))?;
    let status = crate::onboarding_status_from_account(account);
    write_json(&mut stream, &status).await?;
    return Ok(());
}

if request.method == "GET" && request.path == "/api/provider-accounts" {
    let account = state
        .memory_repository
        .active_provider_account()
        .await
        .map_err(|source| DaemonError::Protocol(source.to_string()))?;
    write_json(&mut stream, &account.into_iter().collect::<Vec<_>>()).await?;
    return Ok(());
}

if request.method == "POST" && request.path == "/api/provider-auth/attempts" {
    let body: crate::frontend_protocol::StartProviderAuthAttemptRequest = parse_json_body(&request)?;
    if body.provider_kind != "codex" || body.method != crate::ProviderAuthMethod::OauthDeviceCode {
        write_response(
            &mut stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"unsupported provider auth method",
        )
        .await?;
        return Ok(());
    }

    let account = state
        .memory_repository
        .get_provider_account(&body.provider_account_id)
        .await
        .map_err(|source| DaemonError::Protocol(source.to_string()))?
        .ok_or_else(|| DaemonError::Protocol("provider account not found".to_string()))?;
    let attempt = state
        .provider_auth
        .start_codex_device_code(crate::provider_auth::CodexDeviceAuthRequest {
            provider_account_id: account.provider_account_id,
            account_home: state
                .paths
                .provider_account_home(&account.provider_kind, &account.account_key),
            codex_command: state.codex_command.clone(),
        })
        .await?;
    write_json(&mut stream, &attempt).await?;
    return Ok(());
}

if request.method == "GET"
    && request.path.starts_with("/api/provider-auth/attempts/")
{
    let attempt_id = request
        .path
        .trim_start_matches("/api/provider-auth/attempts/")
        .trim_end_matches("/cancel");
    let attempt = state
        .provider_auth
        .poll_attempt(attempt_id)
        .await?
        .ok_or_else(|| DaemonError::Protocol("provider auth attempt not found".to_string()))?;
    write_json(&mut stream, &attempt).await?;
    return Ok(());
}

if request.method == "POST"
    && request.path.starts_with("/api/provider-auth/attempts/")
    && request.path.ends_with("/cancel")
{
    let attempt_id = request
        .path
        .trim_start_matches("/api/provider-auth/attempts/")
        .trim_end_matches("/cancel")
        .trim_end_matches('/');
    state.provider_auth.cancel_attempt(attempt_id).await?;
    write_json(&mut stream, &serde_json::json!({ "ok": true })).await?;
    return Ok(());
}
```

- [ ] **Step 8: Run endpoint-related tests**

Run:

```bash
cargo test -p noema-core daemon::web::tests::http_request_reads_json_body frontend_protocol::tests::generated_frontend_types_are_current -- --nocapture
```

Expected: generated types test FAILS until running `bun run gen:types`.

- [ ] **Step 9: Refresh generated TypeScript and retest**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
cargo test -p noema-core daemon::web::tests::http_request_reads_json_body frontend_protocol::tests::generated_frontend_types_are_current -- --nocapture
```

Expected: PASS.

- [ ] **Step 10: Commit web endpoints**

Run:

```bash
git add crates/noema-core/src/daemon/server.rs crates/noema-core/src/daemon/web/mod.rs crates/noema-core/src/frontend_protocol.rs crates/noema-core/web/src/generated/noema.ts
git commit -m "feat: expose provider onboarding endpoints"
```

## Task 6: Frontend Onboarding Gate

**Files:**
- Modify: `crates/noema-core/web/src/api.ts`
- Modify: `crates/noema-core/web/src/App.tsx`
- Create: `crates/noema-core/web/src/components/Onboarding.tsx`
- Modify: `crates/noema-core/web/src/components/StatusCluster.tsx`
- Modify: `crates/noema-core/web/src/styles.css`

- [ ] **Step 1: Add frontend API helpers**

Modify `crates/noema-core/web/src/api.ts`:

```ts
import type {
  OnboardingStatus,
  ProviderAuthAttemptView,
  StartProviderAuthAttemptRequest,
  WebStatus
} from "./generated/noema";

export async function fetchOnboardingStatus(): Promise<OnboardingStatus> {
  const response = await fetch("/api/onboarding/status");
  if (!response.ok) {
    throw new Error("Failed to load onboarding status");
  }
  return (await response.json()) as OnboardingStatus;
}

export async function startProviderAuthAttempt(
  request: StartProviderAuthAttemptRequest
): Promise<ProviderAuthAttemptView> {
  const response = await fetch("/api/provider-auth/attempts", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request)
  });
  if (!response.ok) {
    throw new Error(await response.text());
  }
  return (await response.json()) as ProviderAuthAttemptView;
}

export async function fetchProviderAuthAttempt(attemptId: string): Promise<ProviderAuthAttemptView> {
  const response = await fetch(`/api/provider-auth/attempts/${encodeURIComponent(attemptId)}`);
  if (!response.ok) {
    throw new Error(await response.text());
  }
  return (await response.json()) as ProviderAuthAttemptView;
}
```

Keep existing `refreshStatus` and `webSocketUrl`.

- [ ] **Step 2: Create onboarding component**

Create `crates/noema-core/web/src/components/Onboarding.tsx`:

```tsx
import type { OnboardingStatus, ProviderAuthAttemptView } from "../generated/noema";

export function Onboarding({
  onboarding,
  attempt,
  error,
  onConnect,
  onRetry
}: {
  onboarding: OnboardingStatus;
  attempt: ProviderAuthAttemptView | null;
  error: string | null;
  onConnect: () => void;
  onRetry: () => void;
}) {
  const step = onboarding.steps.find((candidate) => candidate.id === "connect_provider_account");
  const providerName = step?.display_name ?? "Provider";
  const waiting = attempt?.status === "waiting_for_user" || attempt?.status === "starting";
  const complete = attempt?.status === "completed";
  const failed = attempt?.status === "failed" || attempt?.status === "expired" || attempt?.status === "cancelled";

  return (
    <section className="onboarding-shell" aria-label="Noema onboarding">
      <div className="onboarding-panel">
        <p className="eyebrow">First run</p>
        <h1>Connect {providerName}</h1>
        <p>
          Noema needs an authenticated provider account before it can start the
          primary chat.
        </p>

        {!attempt ? (
          <button type="button" onClick={onConnect}>
            Connect {providerName}
          </button>
        ) : null}

        {waiting ? (
          <div className="auth-attempt">
            <strong>{attempt.status === "starting" ? "Starting login" : "Waiting for login"}</strong>
            {attempt.verification_url ? (
              <a href={attempt.verification_url} target="_blank" rel="noreferrer">
                Open login page
              </a>
            ) : null}
            {attempt.user_code ? <code>{attempt.user_code}</code> : null}
            {attempt.instructions ? <p>{attempt.instructions}</p> : null}
          </div>
        ) : null}

        {complete ? <p className="onboarding-success">Provider connected. Starting chat.</p> : null}

        {failed ? (
          <button type="button" onClick={onRetry}>
            Try again
          </button>
        ) : null}

        {error ? <p className="onboarding-error">{error}</p> : null}
      </div>
    </section>
  );
}
```

- [ ] **Step 3: Gate WebSocket startup in `App.tsx`**

Modify imports in `crates/noema-core/web/src/App.tsx`:

```tsx
import { fetchOnboardingStatus, fetchProviderAuthAttempt, refreshStatus, startProviderAuthAttempt, webSocketUrl } from "./api";
import { Onboarding } from "./components/Onboarding";
import type { OnboardingStatus, ProviderAuthAttemptView, WebClientMessage, WebServerMessage as ServerMessage, WebStatus } from "./generated/noema";
```

Add state:

```tsx
const [onboarding, setOnboarding] = React.useState<OnboardingStatus | null>(null);
const [authAttempt, setAuthAttempt] = React.useState<ProviderAuthAttemptView | null>(null);
const [onboardingError, setOnboardingError] = React.useState<string | null>(null);
```

Replace the existing WebSocket-opening `useEffect` with:

```tsx
React.useEffect(() => {
  void refreshStatus(setStatus);
  void fetchOnboardingStatus()
    .then(setOnboarding)
    .catch((error: unknown) => setOnboardingError(error instanceof Error ? error.message : "Failed to load onboarding"));
}, []);

React.useEffect(() => {
  if (!onboarding?.is_user_onboarded || socketRef.current) {
    return;
  }

  const socket = new WebSocket(webSocketUrl());
  socketRef.current = socket;
  setSocketState("connecting");

  socket.addEventListener("open", () => {
    const message: WebClientMessage = { type: "primary_conversation_start" };
    setSocketState("ready");
    socket.send(JSON.stringify(message));
  });

  socket.addEventListener("message", (event: MessageEvent<string>) => {
    const message = JSON.parse(event.data) as ServerMessage;
    handleServerMessage(message, {
      setConversationId,
      setTranscript,
      setPending,
      setAgentStatus
    });
  });

  socket.addEventListener("close", () => {
    setSocketState("closed");
    setAgentStatus("closed");
    setPending(false);
  });

  socket.addEventListener("error", () => {
    setSocketState("closed");
    setAgentStatus("closed");
    setPending(false);
    pushTranscript(setTranscript, {
      id: crypto.randomUUID(),
      type: "error",
      message: "Noema's local web connection closed. Refresh the page or restart Noema.",
      recoverable: true
    });
  });

  return () => {
    socket.close();
    socketRef.current = null;
  };
}, [onboarding?.is_user_onboarded]);
```

Add handler:

```tsx
async function connectProvider() {
  const step = onboarding?.steps.find((candidate) => candidate.id === "connect_provider_account");
  if (!step?.provider_kind || !step.provider_account_id || !step.auth_method) {
    setOnboardingError("No provider account is available to connect.");
    return;
  }
  setOnboardingError(null);
  const attempt = await startProviderAuthAttempt({
    provider_kind: step.provider_kind,
    provider_account_id: step.provider_account_id,
    method: step.auth_method
  });
  setAuthAttempt(attempt);
}
```

Add polling effect:

```tsx
React.useEffect(() => {
  if (!authAttempt || authAttempt.status === "completed" || authAttempt.status === "failed") {
    return;
  }
  const timer = window.setInterval(() => {
    void fetchProviderAuthAttempt(authAttempt.attempt_id)
      .then(async (next) => {
        setAuthAttempt(next);
        if (next.status === "completed") {
          setOnboarding(await fetchOnboardingStatus());
        }
      })
      .catch((error: unknown) => setOnboardingError(error instanceof Error ? error.message : "Failed to poll auth"));
  }, 1000);
  return () => window.clearInterval(timer);
}, [authAttempt]);
```

Extract the existing top bar JSX into a component named `Header` so the chat and
onboarding branches share identical chrome:

```tsx
function Header({
  status,
  socketState,
  agentStatus,
}: {
  status: WebStatus | null;
  socketState: SocketState;
  agentStatus: ConversationAgentStatus;
}) {
  return (
    <header className="topbar">
      <div>
        <p className="eyebrow">Noema</p>
        <h1>Personal agent</h1>
      </div>
      <StatusCluster status={status} socketState={socketState} agentStatus={agentStatus} />
    </header>
  );
}
```

Replace the existing chat branch header with:

```tsx
<Header status={status} socketState={socketState} agentStatus={agentStatus} />
```

Before the chat shell render, add:

```tsx
if (onboarding && !onboarding.is_user_onboarded) {
  return (
    <main className="noema-app">
      <Header status={status} socketState={socketState} agentStatus={agentStatus} />
      <Onboarding
        onboarding={onboarding}
        attempt={authAttempt}
        error={onboardingError}
        onConnect={() => void connectProvider()}
        onRetry={() => {
          setAuthAttempt(null);
          setOnboardingError(null);
        }}
      />
    </main>
  );
}
```

- [ ] **Step 4: Add onboarding CSS**

Append to `crates/noema-core/web/src/styles.css`:

```css
.onboarding-shell {
  display: grid;
  align-content: center;
  width: min(760px, 100%);
  min-height: calc(100vh - 68px);
  margin: 0 auto;
  padding: 34px 24px;
}

.onboarding-panel {
  display: grid;
  gap: 14px;
  padding: 18px 0;
}

.onboarding-panel h1 {
  margin: 0;
  font-family: var(--font-display);
  font-size: 34px;
  line-height: 1.1;
  letter-spacing: 0;
}

.onboarding-panel p {
  max-width: 560px;
  margin: 0;
  color: var(--text-secondary);
}

.onboarding-panel button,
.auth-attempt a {
  width: fit-content;
  min-height: 40px;
  padding: 0 14px;
  border: 1px solid var(--pine-600);
  border-radius: var(--radius-sm);
  background: var(--pine-500);
  color: var(--paper-50);
  font-weight: 700;
  text-decoration: none;
}

.auth-attempt {
  display: grid;
  gap: 10px;
  max-width: 560px;
  padding: 14px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-card);
}

.auth-attempt code {
  width: fit-content;
  padding: 6px 8px;
  border-radius: var(--radius-xs);
  background: var(--surface-sunken);
  font-family: var(--font-mono);
  font-size: 18px;
}

.onboarding-error {
  color: var(--red-700);
}

.onboarding-success {
  color: var(--pine-700);
}
```

- [ ] **Step 5: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 6: Commit frontend onboarding**

Run:

```bash
git add crates/noema-core/web/src/api.ts crates/noema-core/web/src/App.tsx crates/noema-core/web/src/components/Onboarding.tsx crates/noema-core/web/src/components/StatusCluster.tsx crates/noema-core/web/src/styles.css crates/noema-core/src/daemon/web/assets/app.js crates/noema-core/src/daemon/web/assets/styles.css crates/noema-core/web/src/generated/noema.ts
git commit -m "feat: gate chat behind onboarding"
```

## Task 7: Runtime Provider Account Home And Docker Codex CLI

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/server.rs`
- Modify: `Dockerfile`
- Modify: `compose.yaml`
- Modify: `README.md`
- Modify: `docs/context/current.md`

- [ ] **Step 1: Write runtime test for Codex home override**

Add a focused unit test in `crates/noema-core/src/daemon/runtime.rs` near runtime config helpers:

```rust
#[test]
fn codex_config_for_provider_account_uses_account_home() {
    let mut config = crate::CodexProviderConfig::default();
    let account_home = std::path::PathBuf::from("/noema/providers/codex/default");

    apply_provider_account_home(&mut config, &account_home);

    assert_eq!(
        config.codex_home.as_deref(),
        Some("/noema/providers/codex/default")
    );
}
```

- [ ] **Step 2: Run runtime test to verify it fails**

Run:

```bash
cargo test -p noema-core daemon::runtime::tests::codex_config_for_provider_account_uses_account_home -- --nocapture
```

Expected: FAIL because helper does not exist.

- [ ] **Step 3: Add runtime config helper**

In `crates/noema-core/src/daemon/runtime.rs`, add:

```rust
fn apply_provider_account_home(config: &mut crate::CodexProviderConfig, account_home: &std::path::Path) {
    config.codex_home = Some(account_home.to_string_lossy().to_string());
}
```

Change `CodexRuntimeHandle::spawn` so it receives `mut codex_config: CodexProviderConfig`,
resolves the default provider account home, prepares it, and applies it before
constructing `CodexRuntimeActor`. Because `CodexRuntimeActor::new` clones the
same config into `MemoryExtractionWorkerHandle::spawn`, this single mutation
covers the main chat runtime and memory extraction runtime.

Exact code in the spawn function should look like:

```rust
let paths = crate::NoemaPaths::from_process_env()?;
let account_home = paths.provider_account_home("codex", "default");
crate::provider_auth::ensure_codex_account_home(&account_home)
    .map_err(|source| DaemonError::Protocol(format!("failed to prepare Codex account home: {source}")))?;
apply_provider_account_home(&mut codex_config, &account_home);
```

Then call:

```rust
let actor = CodexRuntimeActor::new(codex_config, database_url).await?;
```

- [ ] **Step 4: Run runtime test**

Run:

```bash
cargo test -p noema-core daemon::runtime::tests::codex_config_for_provider_account_uses_account_home -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Install Codex CLI in Dockerfile**

Modify `Dockerfile`:

```dockerfile
ARG CODEX_CLI_VERSION=0.142.0

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates unzip pkg-config libssl-dev nodejs npm \
    && rm -rf /var/lib/apt/lists/*

RUN npm install -g @openai/codex@${CODEX_CLI_VERSION} \
    && codex --version
```

Keep the existing Bun and cargo-watch install steps.

- [ ] **Step 6: Update Compose env for provider account home**

In `compose.yaml`, keep `NOEMA_HOME: /noema`. Do not set `NOEMA_CODEX__HOME`; Noema should derive the active provider account home. Do not add an npm cache volume in this slice.

- [ ] **Step 7: Update README development docs**

In `README.md` Development section, add:

```markdown
On first launch, the web UI checks onboarding status before opening chat. If
the active provider account is not authenticated, Noema shows a provider
connection step. The default development provider account stores Codex
credentials under `${NOEMA_HOME:-$HOME/.noema}/providers/codex/default`.

The preferred flow is to open <http://localhost:3737/> and click **Connect
Codex**. If you need the terminal fallback, run the login inside the dev
container so it writes to the same provider account home:

```bash
docker compose run --rm -e CODEX_HOME=/noema/providers/codex/default dev codex login --device-auth
```
```

- [ ] **Step 8: Update durable context**

In `docs/context/current.md`, add a settled decision:

```markdown
- First-run web onboarding is derived from backend readiness checks. V1 blocks
  chat until the active provider account is authenticated.
- Provider credential/session material lives under
  `${NOEMA_HOME:-$HOME/.noema}/providers/<provider>/<account>/`; Postgres stores
  only non-secret provider account metadata.
```

- [ ] **Step 9: Build Docker image**

Run:

```bash
docker compose build dev
```

Expected: image builds and `codex --version` succeeds during the build.

- [ ] **Step 10: Commit runtime and Docker integration**

Run:

```bash
git add crates/noema-core/src/daemon/runtime.rs Dockerfile compose.yaml README.md docs/context/current.md
git commit -m "feat: run codex from provider account home"
```

## Task 8: Final Validation And Smoke

**Files:**
- Verify all touched files.

- [ ] **Step 1: Run Rust formatting**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 2: Run Rust check**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 3: Run Clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Run Rust tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: PASS. If Postgres-backed tests are skipped because `NOEMA_TEST_DATABASE_URL` is unset, run the targeted database tests with Postgres after starting Compose.

- [ ] **Step 5: Run Postgres-backed targeted tests**

Run:

```bash
docker compose up -d postgres
docker compose exec postgres createdb -U noema noema_test
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests -- --nocapture
```

Expected: PASS. If `createdb` reports the database already exists, continue to the test command.

- [ ] **Step 6: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 7: Run Docker dev smoke**

Run:

```bash
docker compose up dev
```

Expected:

- Postgres becomes healthy.
- Dev service starts.
- Output does not contain `codex command not found`.
- Web UI is available at `http://localhost:3737/`.
- Unauthenticated state shows onboarding instead of starting chat.

Stop the foreground stack with Ctrl-C after confirming the smoke.

- [ ] **Step 8: Inspect diff and status**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: `git diff --check` has no whitespace errors. Remaining dirty files should be only intentional work plus any unrelated pre-existing files.

## Execution Notes

- Use `superpowers:subagent-driven-development` or `superpowers:executing-plans` before executing this plan.
- Preserve unrelated dirty worktree changes. At plan creation time, the worktree already contained unrelated edits across daemon, memory, frontend, docs, and Docker files.
- Prefer small commits at the task boundaries above.
- Do not add web-pasted API key/access-token flows in this implementation.
- Do not build provider account switching UI in this implementation.
