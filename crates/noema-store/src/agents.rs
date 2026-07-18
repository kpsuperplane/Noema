use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError, sqlite::conversion_failure};

pub(super) const BUILTIN_AGENTS: [(&str, Option<&str>, &str); 3] = [
    ("agent:primary", None, "primary"),
    (
        "agent:task-executor",
        Some("Task Executor"),
        "task_executor",
    ),
    (
        "agent:task-reviewer",
        Some("Task Reviewer"),
        "task_reviewer",
    ),
];

/// Explicit built-in role for an agent row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSystemRole {
    /// The foreground user-facing agent.
    Primary,
    /// The stable identity used by background task executors.
    TaskExecutor,
    /// The stable identity used by background task reviewers.
    TaskReviewer,
}

impl AgentSystemRole {
    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "primary" => Ok(Self::Primary),
            "task_executor" => Ok(Self::TaskExecutor),
            "task_reviewer" => Ok(Self::TaskReviewer),
            other => Err(StoreError::InvalidEnum {
                kind: "agent_system_role",
                value: other.to_string(),
            }),
        }
    }
}

/// Persisted agent identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRecord {
    /// Durable concrete agent id.
    pub agent_id: String,
    /// Optional human-visible agent name.
    pub display_name: Option<String>,
    /// Explicit built-in role, when this is a system agent.
    pub system_role: Option<AgentSystemRole>,
}

impl NoemaStore {
    /// Create or refresh the built-in local human and primary agent.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write fails.
    pub async fn ensure_default_actors(&self) -> Result<(), StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO humans (human_id, display_name)
                VALUES ('human:local', 'You')
                ON CONFLICT(human_id) DO NOTHING
                "#,
                [],
            )?;
            for (agent_id, display_name, system_role) in BUILTIN_AGENTS {
                conn.execute(
                    r#"
                    INSERT INTO agents (agent_id, display_name, system_role)
                    VALUES (?1, ?2, ?3)
                    ON CONFLICT(agent_id) DO UPDATE SET
                      system_role = excluded.system_role,
                      updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                    "#,
                    params![agent_id, display_name, system_role],
                )?;
            }
            Ok(())
        })
        .await
    }

    /// Return one agent by durable id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT agent_id, display_name, system_role
                FROM agents
                WHERE agent_id = ?1
                LIMIT 1
                "#,
                [agent_id],
                agent_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// List durable agents in deterministic Settings display order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn list_agents(&self) -> Result<Vec<AgentRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT agent_id, display_name, system_role
                FROM agents
                ORDER BY
                  CASE system_role WHEN 'primary' THEN 0 WHEN 'task_executor' THEN 1 WHEN 'task_reviewer' THEN 2 ELSE 3 END,
                  CASE WHEN display_name IS NULL THEN 1 ELSE 0 END,
                  display_name,
                  agent_id
                "#,
            )?;
            let rows = statement.query_map([], agent_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Update one agent's canonical display name.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the agent is missing or the embedded store
    /// write/read fails.
    pub async fn update_agent_display_name(
        &self,
        agent_id: &str,
        display_name: &str,
    ) -> Result<AgentRecord, StoreError> {
        self.require_agent(agent_id).await?;
        let display_name = normalize_agent_display_name(Some(display_name))?
            .expect("non-empty display name is required when updating an agent");
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE agents
                SET display_name = ?2,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE agent_id = ?1
                "#,
                params![agent_id, display_name],
            )?;
            Ok(())
        })
        .await?;
        self.get_agent(agent_id)
            .await?
            .ok_or_else(|| StoreError::AgentNotFound {
                agent_id: agent_id.to_string(),
            })
    }

    pub(crate) async fn require_agent(&self, agent_id: &str) -> Result<(), StoreError> {
        if self.get_agent(agent_id).await?.is_some() {
            Ok(())
        } else {
            Err(StoreError::AgentNotFound {
                agent_id: agent_id.to_string(),
            })
        }
    }
}

fn agent_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRecord> {
    let system_role: Option<String> = row.get(2)?;
    Ok(AgentRecord {
        agent_id: row.get(0)?,
        display_name: row.get(1)?,
        system_role: system_role
            .as_deref()
            .map(AgentSystemRole::parse)
            .transpose()
            .map_err(|error| conversion_failure(2, rusqlite::types::Type::Text, error))?,
    })
}

fn normalize_agent_display_name(display_name: Option<&str>) -> Result<Option<String>, StoreError> {
    display_name
        .map(|name| {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                Err(StoreError::AgentDisplayNameEmpty)
            } else {
                Ok(trimmed.to_string())
            }
        })
        .transpose()
}
