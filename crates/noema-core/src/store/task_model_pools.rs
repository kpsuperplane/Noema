//! Human-controlled executor model pools.

#![allow(clippy::missing_errors_doc)]

use rusqlite::{OptionalExtension, params};

use crate::{
    ProviderAccountStatus,
    provider::ReasoningEffort,
    task::{ModelConfigSnapshot, TaskComplexity, provider_defaults::provider_default_task_models},
};

use super::{NoemaStore, StoreError};

const TASK_MODEL_POOL_SETTING_PREFIX: &str = "task_pool:setting:";
const LEGACY_PROVIDER_DEFAULT_POOL_PREFIX: &str = "task_pool:provider_default:";

/// Input for creating or updating one executor pool entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTaskModelPoolEntry {
    /// Optional stable id; one is allocated when absent.
    pub pool_entry_id: Option<String>,
    /// Complexity tier exposed to the primary agent.
    pub complexity: TaskComplexity,
    /// Optional human-facing label.
    pub label: Option<String>,
    /// Provider family for the exact model profile.
    pub provider_kind: String,
    /// Active provider account that owns the model profile.
    pub provider_account_id: String,
    /// Exact provider model/profile.
    pub model_profile: String,
    /// Optional explicit reasoning effort.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Whether the entry can be selected for new tasks.
    pub enabled: bool,
    /// Human-controlled ordering within a complexity tier.
    pub sort_order: i64,
}

/// Persisted executor model pool entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskModelPoolEntry {
    /// Stable pool-entry id.
    pub pool_entry_id: String,
    /// Complexity tier exposed to the primary agent.
    pub complexity: TaskComplexity,
    /// Optional human-facing label.
    pub label: Option<String>,
    /// Immutable model snapshot selected by this entry.
    pub model: ModelConfigSnapshot,
    /// Whether the entry can be selected for new tasks.
    pub enabled: bool,
    /// Human-controlled ordering within a complexity tier.
    pub sort_order: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl TaskModelPoolEntry {
    /// Return whether this row is one of Noema's three global settings.
    #[must_use]
    pub fn is_global_setting(&self) -> bool {
        is_global_task_model_pool_setting_id(&self.pool_entry_id)
    }
}

/// Return whether an id belongs to one of the three global executor settings.
#[must_use]
pub fn is_global_task_model_pool_setting_id(pool_entry_id: &str) -> bool {
    pool_entry_id.starts_with(TASK_MODEL_POOL_SETTING_PREFIX)
}

impl NewTaskModelPoolEntry {
    fn normalized(&self) -> Result<Self, StoreError> {
        let provider_kind = self.provider_kind.trim().to_ascii_lowercase();
        let provider_account_id = self.provider_account_id.trim().to_string();
        let model_profile = self.model_profile.trim().to_string();
        let model = ModelConfigSnapshot::explicit(
            provider_kind.clone(),
            provider_account_id.clone(),
            model_profile.clone(),
            self.reasoning_effort,
            Some("task_model_pool".to_string()),
        )
        .normalized()
        .map_err(|error| StoreError::InvariantViolation {
            message: error.to_string(),
        })?;
        if provider_account_id.is_empty() || model_profile.is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "task model pool provider account and model profile are required"
                    .to_string(),
            });
        }
        Ok(Self {
            pool_entry_id: self
                .pool_entry_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
            complexity: self.complexity,
            label: self
                .label
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
            provider_kind,
            provider_account_id,
            model_profile,
            reasoning_effort: model.reasoning_effort,
            enabled: self.enabled,
            sort_order: self.sort_order,
        })
    }
}

impl NoemaStore {
    /// Ensure exactly one global executor model setting exists per tier.
    ///
    /// The selected default provider supplies initial values. Existing global
    /// settings remain user-controlled, while older provider-scoped rows are
    /// consolidated and retired.
    pub async fn ensure_default_task_model_pool_settings(
        &self,
        default_provider_kind: &str,
    ) -> Result<Vec<TaskModelPoolEntry>, StoreError> {
        let account = self
            .active_provider_account(default_provider_kind)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!(
                    "default task model provider account is unavailable: {default_provider_kind}"
                ),
            })?;
        let defaults = provider_default_task_models(default_provider_kind);
        if defaults.len() != 3 {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "default task model provider {default_provider_kind} does not define all three tiers"
                ),
            });
        }
        let existing = self.list_task_model_pool_entries(None).await?;
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            for default in defaults {
                let pool_entry_id = global_task_model_pool_setting_id(default.complexity);
                if existing
                    .iter()
                    .any(|entry| entry.pool_entry_id == pool_entry_id)
                {
                    continue;
                }
                let migrated = existing
                    .iter()
                    .find(|entry| {
                        entry.complexity == default.complexity
                            && !entry
                                .pool_entry_id
                                .starts_with(LEGACY_PROVIDER_DEFAULT_POOL_PREFIX)
                    })
                    .or_else(|| {
                        existing.iter().find(|entry| {
                            entry.complexity == default.complexity
                                && entry.model.provider_account_id == account.provider_account_id
                        })
                    });
                if let Some(migrated) = migrated {
                    transaction.execute(
                        "UPDATE tasks SET pool_entry_id = ?1 WHERE pool_entry_id = ?2",
                        params![pool_entry_id, migrated.pool_entry_id],
                    )?;
                    transaction.execute(
                        "UPDATE task_model_pool_entries SET pool_entry_id = ?1, sort_order = 0, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE pool_entry_id = ?2",
                        params![pool_entry_id, migrated.pool_entry_id],
                    )?;
                    continue;
                }
                transaction.execute(
                    r#"
                    INSERT INTO task_model_pool_entries (
                      pool_entry_id, complexity, label, provider_kind,
                      provider_account_id, model_profile, reasoning_effort,
                      enabled, sort_order
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)
                    "#,
                    params![
                        pool_entry_id,
                        default.complexity.as_str(),
                        default.label,
                        account.provider_kind,
                        account.provider_account_id,
                        default.model_profile,
                        default
                            .reasoning_effort
                            .map(ReasoningEffort::as_persistence_str),
                        true,
                    ],
                )?;
            }
            for entry in &existing {
                if entry.is_global_setting() {
                    continue;
                }
                let references: i64 = transaction.query_row(
                    "SELECT COUNT(*) FROM tasks WHERE pool_entry_id = ?1",
                    [&entry.pool_entry_id],
                    |row| row.get(0),
                )?;
                if references > 0 {
                    transaction.execute(
                        "UPDATE task_model_pool_entries SET enabled = 0, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE pool_entry_id = ?1",
                        [&entry.pool_entry_id],
                    )?;
                } else {
                    transaction.execute(
                        "DELETE FROM task_model_pool_entries WHERE pool_entry_id = ?1",
                        [&entry.pool_entry_id],
                    )?;
                }
            }
            transaction.commit()?;
            Ok(())
        })
        .await?;
        self.list_task_model_pool_settings(None).await
    }

    /// Update an existing pool entry while retaining its stable id.
    pub async fn update_task_model_pool_entry(
        &self,
        pool_entry_id: &str,
        input: NewTaskModelPoolEntry,
    ) -> Result<TaskModelPoolEntry, StoreError> {
        let input = input.normalized()?;
        self.validate_pool_account(&input.provider_kind, &input.provider_account_id)
            .await?;
        let existing = self
            .get_task_model_pool_entry(pool_entry_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("task model pool entry not found: {pool_entry_id}"),
            })?;
        if !existing.is_global_setting() || existing.complexity != input.complexity {
            return Err(StoreError::InvariantViolation {
                message: "task model pool settings have stable complexity tiers".to_string(),
            });
        }
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE task_model_pool_entries
                SET complexity = ?2,
                    label = ?3,
                    provider_kind = ?4,
                    provider_account_id = ?5,
                    model_profile = ?6,
                    reasoning_effort = ?7,
                    enabled = ?8,
                    sort_order = ?9,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE pool_entry_id = ?1
                "#,
                params![
                    existing.pool_entry_id,
                    input.complexity.as_str(),
                    input.label,
                    input.provider_kind,
                    input.provider_account_id,
                    input.model_profile,
                    input
                        .reasoning_effort
                        .map(ReasoningEffort::as_persistence_str),
                    input.enabled,
                    input.sort_order,
                ],
            )?;
            Ok(())
        })
        .await?;
        self.get_task_model_pool_entry(pool_entry_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("updated task model pool entry disappeared: {pool_entry_id}"),
            })
    }

    /// Return one pool entry by stable id.
    pub async fn get_task_model_pool_entry(
        &self,
        pool_entry_id: &str,
    ) -> Result<Option<TaskModelPoolEntry>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT pool_entry_id, complexity, label, provider_kind,
                       provider_account_id, model_profile, reasoning_effort,
                       enabled, sort_order, created_at, updated_at
                FROM task_model_pool_entries
                WHERE pool_entry_id = ?1
                LIMIT 1
                "#,
                [pool_entry_id],
                pool_entry_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// List pool entries, optionally restricted to one complexity tier.
    pub async fn list_task_model_pool_entries(
        &self,
        complexity: Option<TaskComplexity>,
    ) -> Result<Vec<TaskModelPoolEntry>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT pool_entry_id, complexity, label, provider_kind,
                       provider_account_id, model_profile, reasoning_effort,
                       enabled, sort_order, created_at, updated_at
                FROM task_model_pool_entries
                WHERE (?1 IS NULL OR complexity = ?1)
                ORDER BY complexity, sort_order, label, pool_entry_id
                "#,
            )?;
            let rows = statement.query_map(
                [complexity.map(TaskComplexity::as_str)],
                pool_entry_from_row,
            )?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// List the three global task-executor settings.
    pub async fn list_task_model_pool_settings(
        &self,
        complexity: Option<TaskComplexity>,
    ) -> Result<Vec<TaskModelPoolEntry>, StoreError> {
        Ok(self
            .list_task_model_pool_entries(complexity)
            .await?
            .into_iter()
            .filter(TaskModelPoolEntry::is_global_setting)
            .collect())
    }

    /// List enabled pool entries backed by an authenticated provider account.
    pub async fn list_usable_task_model_pool_entries(
        &self,
    ) -> Result<Vec<TaskModelPoolEntry>, StoreError> {
        let entries = self.list_task_model_pool_settings(None).await?;
        let mut usable = Vec::new();
        for entry in entries.into_iter().filter(|entry| entry.enabled) {
            if self
                .validate_task_model_snapshot(&entry.model)
                .await
                .is_ok()
            {
                usable.push(entry);
            }
        }
        Ok(usable)
    }

    /// Select an enabled exact entry from the requested tier.
    pub async fn select_task_model_pool_entry(
        &self,
        complexity: TaskComplexity,
        pool_entry_id: &str,
    ) -> Result<TaskModelPoolEntry, StoreError> {
        let entry = self
            .get_task_model_pool_entry(pool_entry_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("task model pool entry not found: {pool_entry_id}"),
            })?;
        if entry.complexity != complexity {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "task model pool entry {pool_entry_id} belongs to {}, not {}",
                    entry.complexity, complexity
                ),
            });
        }
        if !entry.enabled {
            return Err(StoreError::InvariantViolation {
                message: format!("task model pool entry is disabled: {pool_entry_id}"),
            });
        }
        self.validate_task_model_snapshot(&entry.model).await?;
        Ok(entry)
    }

    /// Verify that a task model still belongs to an authenticated account and,
    /// when a catalog is available, that the exact profile is advertised.
    pub async fn validate_task_model_snapshot(
        &self,
        model: &ModelConfigSnapshot,
    ) -> Result<(), StoreError> {
        self.validate_usable_pool_account(
            &model.provider_kind,
            &model.provider_account_id,
            model.model_profile.as_deref(),
        )
        .await
    }

    async fn validate_pool_account(
        &self,
        provider_kind: &str,
        provider_account_id: &str,
    ) -> Result<(), StoreError> {
        let account = self
            .get_provider_account(provider_account_id)
            .await?
            .ok_or_else(|| StoreError::ProviderAccountNotFound {
                provider_account_id: provider_account_id.to_string(),
            })?;
        if !account.is_active || account.provider_kind != provider_kind {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "provider account {} is not an active {} account",
                    provider_account_id, provider_kind
                ),
            });
        }
        Ok(())
    }

    async fn validate_usable_pool_account(
        &self,
        provider_kind: &str,
        provider_account_id: &str,
        model_profile: Option<&str>,
    ) -> Result<(), StoreError> {
        self.validate_pool_account(provider_kind, provider_account_id)
            .await?;
        let account = self
            .get_provider_account(provider_account_id)
            .await?
            .ok_or_else(|| StoreError::ProviderAccountNotFound {
                provider_account_id: provider_account_id.to_string(),
            })?;
        if account.status != ProviderAccountStatus::Authenticated {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "provider account {provider_account_id} is not authenticated for task execution"
                ),
            });
        }
        if let Some(model_profile) = model_profile
            && let Some(profiles) = account
                .metadata
                .get("profiles")
                .and_then(serde_json::Value::as_array)
            && !profiles.is_empty()
            && !profiles.iter().any(|profile| {
                profile.get("id").and_then(serde_json::Value::as_str) == Some(model_profile)
            })
        {
            return Err(StoreError::InvariantViolation {
                message: format!(
                    "model profile {model_profile} is not available for provider account {provider_account_id}"
                ),
            });
        }
        Ok(())
    }
}

fn global_task_model_pool_setting_id(complexity: TaskComplexity) -> String {
    format!("{TASK_MODEL_POOL_SETTING_PREFIX}{}", complexity.as_str())
}

fn pool_entry_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskModelPoolEntry> {
    let pool_entry_id: String = row.get(0)?;
    let complexity: String = row.get(1)?;
    let complexity = complexity.parse::<TaskComplexity>().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let provider_kind: String = row.get(3)?;
    let provider_account_id: String = row.get(4)?;
    let model_profile: String = row.get(5)?;
    let reasoning_effort: Option<String> = row.get(6)?;
    let reasoning_effort = parse_reasoning(reasoning_effort.as_deref()).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(TaskModelPoolEntry {
        pool_entry_id: pool_entry_id.clone(),
        complexity,
        label: row.get(2)?,
        model: ModelConfigSnapshot::explicit(
            provider_kind,
            provider_account_id,
            model_profile,
            reasoning_effort,
            Some(if is_global_task_model_pool_setting_id(&pool_entry_id) {
                "task_model_pool_setting".to_string()
            } else {
                "task_model_pool_override".to_string()
            }),
        ),
        enabled: row.get::<_, i64>(7)? != 0,
        sort_order: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn parse_reasoning(value: Option<&str>) -> Result<Option<ReasoningEffort>, StoreError> {
    value
        .map(|value| {
            ReasoningEffort::from_persistence_str(value).ok_or_else(|| StoreError::InvalidEnum {
                kind: "reasoning_effort",
                value: value.to_string(),
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::test_store;

    #[tokio::test]
    async fn provider_defaults_seed_one_global_setting_per_tier_idempotently() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");

        let first = store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("defaults");
        let second = store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("idempotent defaults");

        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
        assert!(first.iter().all(TaskModelPoolEntry::is_global_setting));
        assert!(
            first
                .iter()
                .all(|entry| entry.model.provider_kind == "codex")
        );
        assert!(first.iter().any(|entry| {
            entry.complexity == TaskComplexity::Simple
                && entry.model.model_profile.as_deref() == Some("gpt-5.6-luna")
                && entry.model.reasoning_effort == Some(ReasoningEffort::Medium)
        }));
        assert!(first.iter().any(|entry| {
            entry.complexity == TaskComplexity::Medium
                && entry.model.model_profile.as_deref() == Some("gpt-5.6-luna")
                && entry.model.reasoning_effort == Some(ReasoningEffort::XHigh)
        }));
        assert!(first.iter().any(|entry| {
            entry.complexity == TaskComplexity::Difficult
                && entry.model.model_profile.as_deref() == Some("gpt-5.6-sol")
                && entry.model.reasoning_effort == Some(ReasoningEffort::High)
        }));
    }

    #[tokio::test]
    async fn usable_defaults_require_an_authenticated_provider() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("defaults");

        assert!(
            store
                .list_usable_task_model_pool_entries()
                .await
                .expect("unavailable defaults")
                .is_empty()
        );

        store
            .update_provider_account_status(
                "provider_account:codex:default",
                ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated account");

        let usable = store
            .list_usable_task_model_pool_entries()
            .await
            .expect("usable defaults");
        assert_eq!(usable.len(), 3);
        assert!(usable.iter().all(|entry| entry.enabled));
    }

    #[tokio::test]
    async fn selection_rejects_a_profile_missing_from_the_provider_catalog() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        let settings = store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("defaults");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated account");
        store
            .update_provider_account_metadata(
                "provider_account:codex:default",
                serde_json::json!({"profiles": [{"id": "gpt-live"}]}),
            )
            .await
            .expect("catalog");
        let simple = settings
            .iter()
            .find(|entry| entry.complexity == TaskComplexity::Simple)
            .expect("simple setting");

        let error = store
            .select_task_model_pool_entry(TaskComplexity::Simple, &simple.pool_entry_id)
            .await
            .expect_err("stale model must be rejected");
        assert!(error.to_string().contains("is not available"));
    }

    #[tokio::test]
    async fn ensuring_defaults_preserves_user_edits() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        let defaults = store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("defaults");
        let simple = defaults
            .into_iter()
            .find(|entry| {
                entry.model.provider_kind == "codex" && entry.complexity == TaskComplexity::Simple
            })
            .expect("simple default");

        store
            .update_task_model_pool_entry(
                &simple.pool_entry_id,
                NewTaskModelPoolEntry {
                    pool_entry_id: Some(simple.pool_entry_id.clone()),
                    complexity: TaskComplexity::Simple,
                    label: Some("My fast model".to_string()),
                    provider_kind: "codex".to_string(),
                    provider_account_id: "provider_account:codex:default".to_string(),
                    model_profile: "gpt-5.6-terra".to_string(),
                    reasoning_effort: Some(ReasoningEffort::High),
                    enabled: true,
                    sort_order: 0,
                },
            )
            .await
            .expect("override");

        let entries = store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("defaults after override");
        let edited = entries
            .into_iter()
            .find(|entry| entry.pool_entry_id == simple.pool_entry_id)
            .expect("edited default");
        assert_eq!(edited.label.as_deref(), Some("My fast model"));
        assert_eq!(edited.model.model_profile.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(edited.model.reasoning_effort, Some(ReasoningEffort::High));
    }

    #[tokio::test]
    async fn provider_scoped_defaults_are_consolidated_into_three_global_settings() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .with_connection(|conn| {
                for (provider_kind, account_id, profile, effort) in [
                    (
                        "codex",
                        "provider_account:codex:default",
                        "gpt-5.6-luna",
                        Some("medium"),
                    ),
                    (
                        "foundation_local",
                        "provider_account:foundation_local:default",
                        "default",
                        None,
                    ),
                ] {
                    for complexity in ["simple", "medium", "difficult"] {
                        conn.execute(
                            "INSERT INTO task_model_pool_entries (pool_entry_id, complexity, provider_kind, provider_account_id, model_profile, reasoning_effort, enabled, sort_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 0)",
                            rusqlite::params![
                                format!("{LEGACY_PROVIDER_DEFAULT_POOL_PREFIX}{account_id}:{complexity}"),
                                complexity,
                                provider_kind,
                                account_id,
                                profile,
                                effort,
                            ],
                        )?;
                    }
                }
                Ok(())
            })
            .await
            .expect("legacy defaults");

        let settings = store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("global settings");
        let all_entries = store
            .list_task_model_pool_entries(None)
            .await
            .expect("all entries");

        assert_eq!(settings.len(), 3);
        assert_eq!(all_entries, settings);
        assert!(
            settings
                .iter()
                .all(|entry| entry.model.provider_kind == "codex")
        );
    }
}
