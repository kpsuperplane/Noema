use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError};

/// Input for creating a durable agent row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAgent {
    /// Durable concrete agent id.
    pub agent_id: String,
    /// Optional human-visible agent name.
    pub display_name: Option<String>,
}

/// Persisted human identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanRecord {
    /// Durable concrete human id.
    pub human_id: String,
    /// Human-visible name.
    pub display_name: String,
    /// Default conversation id, when one has been created.
    pub primary_conversation_id: Option<String>,
}

/// Persisted agent identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRecord {
    /// Durable concrete agent id.
    pub agent_id: String,
    /// Optional human-visible agent name.
    pub display_name: Option<String>,
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
            conn.execute(
                r#"
                INSERT INTO agents (agent_id, display_name)
                VALUES ('agent:primary', NULL)
                ON CONFLICT(agent_id) DO NOTHING
                "#,
                [],
            )?;
            Ok(())
        })
        .await
    }

    /// Create one durable agent row.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn create_agent(&self, agent: NewAgent) -> Result<AgentRecord, StoreError> {
        let display_name = normalize_agent_display_name(agent.display_name.as_deref())?;
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO agents (agent_id, display_name)
                VALUES (?1, ?2)
                ON CONFLICT(agent_id) DO UPDATE SET
                  display_name = excluded.display_name,
                  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                "#,
                params![agent.agent_id, display_name],
            )?;
            Ok(())
        })
        .await?;
        self.get_agent(&agent.agent_id)
            .await?
            .ok_or(StoreError::AgentNotFound {
                agent_id: agent.agent_id,
            })
    }

    /// Return one human by durable id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn get_human(&self, human_id: &str) -> Result<Option<HumanRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT human_id, display_name, primary_conversation_id
                FROM humans
                WHERE human_id = ?1
                LIMIT 1
                "#,
                [human_id],
                |row| {
                    Ok(HumanRecord {
                        human_id: row.get(0)?,
                        display_name: row.get(1)?,
                        primary_conversation_id: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::Sqlite)
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
                SELECT agent_id, display_name
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
                SELECT agent_id, display_name
                FROM agents
                ORDER BY
                  CASE WHEN agent_id = 'agent:primary' THEN 0 ELSE 1 END,
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
    Ok(AgentRecord {
        agent_id: row.get(0)?,
        display_name: row.get(1)?,
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
