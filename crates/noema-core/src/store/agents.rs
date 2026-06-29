use serde::Deserialize;
use surrealdb::types::SurrealValue;

use super::{NoemaStore, StoreError};

/// Input for creating a durable agent row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAgent {
    /// Durable concrete agent id.
    pub agent_id: String,
    /// Optional human-visible agent name.
    pub display_name: Option<String>,
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
    /// Create one durable agent row.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn create_agent(&self, agent: NewAgent) -> Result<AgentRecord, StoreError> {
        let display_name = normalize_agent_display_name(agent.display_name.as_deref())?;
        self.db
            .query(
                r#"
                CREATE type::record('agents', $record_id) SET
                  agent_id = $agent_id,
                  display_name = $display_name,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", agent_record_fragment(&agent.agent_id)))
            .bind(("agent_id", agent.agent_id.clone()))
            .bind(("display_name", display_name))
            .await?
            .check()?;
        self.get_agent(&agent.agent_id)
            .await?
            .ok_or(StoreError::AgentNotFound {
                agent_id: agent.agent_id,
            })
    }

    /// Return one agent by durable id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT agent_id, display_name
                FROM agents
                WHERE agent_id = $agent_id
                LIMIT 1;
                "#,
            )
            .bind(("agent_id", agent_id.to_string()))
            .await?;
        let rows: Vec<AgentRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(agent_from_row))
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
        self.db
            .query(
                r#"
                UPDATE agents SET
                  display_name = $display_name,
                  updated_at = time::now()
                WHERE agent_id = $agent_id;
                "#,
            )
            .bind(("agent_id", agent_id.to_string()))
            .bind(("display_name", display_name))
            .await?
            .check()?;
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

#[derive(Debug, Deserialize, SurrealValue)]
struct AgentRow {
    agent_id: String,
    display_name: Option<String>,
}

fn agent_from_row(row: AgentRow) -> AgentRecord {
    AgentRecord {
        agent_id: row.agent_id,
        display_name: row.display_name,
    }
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

pub(super) fn agent_record_fragment(agent_id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut fragment = String::with_capacity("agent_".len() + agent_id.len() * 2);
    fragment.push_str("agent_");
    for byte in agent_id.bytes() {
        fragment.push(HEX[(byte >> 4) as usize] as char);
        fragment.push(HEX[(byte & 0x0f) as usize] as char);
    }
    fragment
}
