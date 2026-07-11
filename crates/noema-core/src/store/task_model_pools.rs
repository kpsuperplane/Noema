//! Human-controlled executor model pools.

use rusqlite::{OptionalExtension, params};

use crate::{
    provider::ReasoningEffort,
    task::{ModelConfigSnapshot, TaskComplexity},
};

use super::{NoemaStore, StoreError, ids::allocate_id};

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
    /// Create one enabled/disabled executor model-pool entry.
    ///
    /// The account family and exact profile are validated before insertion.
    pub async fn create_task_model_pool_entry(
        &self,
        input: NewTaskModelPoolEntry,
    ) -> Result<TaskModelPoolEntry, StoreError> {
        let input = input.normalized()?;
        self.validate_pool_account(&input.provider_kind, &input.provider_account_id)
            .await?;
        let pool_entry_id = input
            .pool_entry_id
            .clone()
            .unwrap_or_else(|| allocate_id("task_pool"));
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO task_model_pool_entries (
                  pool_entry_id, complexity, label, provider_kind,
                  provider_account_id, model_profile, reasoning_effort,
                  enabled, sort_order
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    pool_entry_id,
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
        self.get_task_model_pool_entry(&pool_entry_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("created task model pool entry disappeared: {pool_entry_id}"),
            })
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
        self.validate_pool_account(&entry.model.provider_kind, &entry.model.provider_account_id)
            .await?;
        Ok(entry)
    }

    /// Disable an entry when it has historical task references; otherwise remove it.
    pub async fn delete_task_model_pool_entry(
        &self,
        pool_entry_id: &str,
    ) -> Result<(), StoreError> {
        let exists = self
            .get_task_model_pool_entry(pool_entry_id)
            .await?
            .is_some();
        if !exists {
            return Err(StoreError::InvariantViolation {
                message: format!("task model pool entry not found: {pool_entry_id}"),
            });
        }
        self.with_connection(|conn| {
            let references: i64 = conn.query_row(
                "SELECT COUNT(*) FROM tasks WHERE pool_entry_id = ?1",
                [pool_entry_id],
                |row| row.get(0),
            )?;
            if references > 0 {
                conn.execute(
                    "UPDATE task_model_pool_entries SET enabled = 0, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE pool_entry_id = ?1",
                    [pool_entry_id],
                )?;
            } else {
                conn.execute(
                    "DELETE FROM task_model_pool_entries WHERE pool_entry_id = ?1",
                    [pool_entry_id],
                )?;
            }
            Ok(())
        })
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
}

fn pool_entry_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskModelPoolEntry> {
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
        pool_entry_id: row.get(0)?,
        complexity,
        label: row.get(2)?,
        model: ModelConfigSnapshot::explicit(
            provider_kind,
            provider_account_id,
            model_profile,
            reasoning_effort,
            Some("task_model_pool".to_string()),
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
