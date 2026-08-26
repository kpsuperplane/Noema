use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

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

/// Persisted readiness of one configured ACP process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpAgentHealthStatus {
    /// The command has not been probed yet.
    Unknown,
    /// Initialization completed successfully.
    Healthy,
    /// The process could not initialize.
    Unavailable,
}

impl AcpAgentHealthStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Healthy => "healthy",
            Self::Unavailable => "unavailable",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "unknown" => Ok(Self::Unknown),
            "healthy" => Ok(Self::Healthy),
            "unavailable" => Ok(Self::Unavailable),
            other => Err(StoreError::InvalidEnum {
                kind: "acp_agent_health_status",
                value: other.to_string(),
            }),
        }
    }
}

/// Persisted authentication state reported by one ACP agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpAgentAuthStatus {
    /// Authentication has not been probed.
    Unknown,
    /// The agent advertises no authentication requirement.
    None,
    /// The agent requires authentication.
    Required,
    /// Authentication completed successfully.
    Authenticated,
    /// The most recent authentication attempt failed.
    Failed,
}

impl AcpAgentAuthStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::None => "none",
            Self::Required => "required",
            Self::Authenticated => "authenticated",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "unknown" => Ok(Self::Unknown),
            "none" => Ok(Self::None),
            "required" => Ok(Self::Required),
            "authenticated" => Ok(Self::Authenticated),
            "failed" => Ok(Self::Failed),
            other => Err(StoreError::InvalidEnum {
                kind: "acp_agent_auth_status",
                value: other.to_string(),
            }),
        }
    }
}

/// Concrete configured ACP executor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpAgentRecord {
    /// Durable agent identity.
    pub agent_id: String,
    /// Human-visible name.
    pub display_name: String,
    /// Executable launched directly.
    pub command: String,
    /// Exact executable arguments.
    pub arguments: Vec<String>,
    /// Whether new tasks may select this agent.
    pub enabled: bool,
    /// Authentication state.
    pub auth_status: AcpAgentAuthStatus,
    /// Process health state.
    pub health_status: AcpAgentHealthStatus,
    /// Agent-reported implementation name.
    pub implementation_name: Option<String>,
    /// Agent-reported implementation version.
    pub implementation_version: Option<String>,
    /// Agent-reported capabilities for inspection.
    pub capabilities: serde_json::Value,
    /// Monotonic launch-configuration revision.
    pub connection_revision: u64,
    /// Safe bounded setup error.
    pub last_error: Option<String>,
}

impl NoemaStore {
    /// Create one custom ACP agent and its launch configuration atomically.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid configuration or a failed durable write.
    pub async fn create_acp_agent(
        &self,
        display_name: &str,
        command: &str,
        arguments: &[String],
    ) -> Result<AcpAgentRecord, StoreError> {
        let display_name = required_text(display_name, "ACP agent name")?;
        let command = required_text(command, "ACP command")?;
        validate_arguments(arguments)?;
        let agent_id = crate::ids::allocate_id("agent");
        let arguments_json = serde_json::to_string(arguments)?;
        self.with_immediate_transaction_retry(|tx| {
            tx.execute(
                "INSERT INTO agents (agent_id, display_name) VALUES (?1, ?2)",
                params![agent_id, display_name],
            )?;
            tx.execute(
                "INSERT INTO acp_agents (agent_id, command, arguments_json) VALUES (?1, ?2, ?3)",
                params![agent_id, command, arguments_json],
            )?;
            load_acp_agent_tx(tx, &agent_id)?.ok_or(StoreError::InvariantViolation {
                message: "created ACP agent is unavailable".to_string(),
            })
        })
        .await
    }

    /// Return all configured ACP agents in deterministic display order.
    ///
    /// # Errors
    ///
    /// Returns an error when configured agents cannot be read.
    pub async fn list_acp_agents(&self) -> Result<Vec<AcpAgentRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT a.agent_id, a.display_name, c.command, c.arguments_json, c.enabled, c.auth_status, c.health_status, c.implementation_name, c.implementation_version, c.capabilities_json, c.connection_revision, c.last_error FROM acp_agents c JOIN agents a ON a.agent_id = c.agent_id ORDER BY a.display_name, a.agent_id",
            )?;
            statement
                .query_map([], acp_agent_from_row)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Return one configured ACP agent.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured agent cannot be read.
    pub async fn get_acp_agent(
        &self,
        agent_id: &str,
    ) -> Result<Option<AcpAgentRecord>, StoreError> {
        let agent_id = agent_id.to_string();
        self.with_connection(move |conn| load_acp_agent_tx(conn, &agent_id))
            .await
    }

    /// Replace editable ACP configuration and advance its revision.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or stale configuration or a failed durable write.
    pub async fn update_acp_agent(
        &self,
        agent_id: &str,
        expected_revision: u64,
        display_name: &str,
        command: &str,
        arguments: &[String],
        enabled: bool,
    ) -> Result<AcpAgentRecord, StoreError> {
        let display_name = required_text(display_name, "ACP agent name")?;
        let command = required_text(command, "ACP command")?;
        validate_arguments(arguments)?;
        let arguments_json = serde_json::to_string(arguments)?;
        let agent_id = agent_id.to_string();
        self.with_immediate_transaction_retry(|tx| {
            let changed = tx.execute(
                "UPDATE acp_agents SET command = ?3, arguments_json = ?4, enabled = ?5, connection_revision = connection_revision + 1, auth_status = 'unknown', health_status = 'unknown', implementation_name = NULL, implementation_version = NULL, capabilities_json = '{}', last_error = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE agent_id = ?1 AND connection_revision = ?2",
                params![agent_id, expected_revision, command, arguments_json, enabled as i64],
            )?;
            if changed != 1 {
                return Err(StoreError::Work(
                    noema_tasks::WorkDomainError::IdempotencyConflict,
                ));
            }
            tx.execute(
                "UPDATE agents SET display_name = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE agent_id = ?1",
                params![agent_id, display_name],
            )?;
            load_acp_agent_tx(tx, &agent_id)?.ok_or(StoreError::InvariantViolation {
                message: "updated ACP agent is unavailable".to_string(),
            })
        })
        .await
    }

    /// Hard-delete one unreferenced ACP executor and its mutable setup state.
    ///
    /// # Errors
    ///
    /// Returns an error when the revision is stale, current work still names
    /// the executor, or the durable write fails. A missing executor returns
    /// `Ok(false)`.
    pub async fn delete_acp_agent(
        &self,
        agent_id: &str,
        expected_revision: u64,
    ) -> Result<bool, StoreError> {
        let agent_id = agent_id.to_string();
        let expected_revision =
            i64::try_from(expected_revision).map_err(|_| StoreError::AcpAgentRevisionConflict {
                agent_id: agent_id.clone(),
            })?;
        self.with_immediate_transaction_retry(|tx| {
            let current_revision = tx
                .query_row(
                    "SELECT connection_revision FROM acp_agents WHERE agent_id = ?1",
                    [&agent_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            let Some(current_revision) = current_revision else {
                return Ok(false);
            };
            if current_revision != expected_revision {
                return Err(StoreError::AcpAgentRevisionConflict {
                    agent_id: agent_id.clone(),
                });
            }
            let authentication_pending = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM acp_auth_attempts WHERE agent_id = ?1 AND state = 'pending')",
                [&agent_id],
                |row| row.get::<_, bool>(0),
            )?;
            if authentication_pending {
                return Err(StoreError::AcpAgentAuthenticationInProgress {
                    agent_id: agent_id.clone(),
                });
            }
            if acp_agent_is_referenced(tx, &agent_id)? {
                return Err(StoreError::AcpAgentInUse {
                    agent_id: agent_id.clone(),
                });
            }

            tx.execute(
                "DELETE FROM agent_runtime_preferences WHERE agent_id = ?1",
                [&agent_id],
            )?;
            tx.execute(
                "DELETE FROM acp_auth_attempts WHERE agent_id = ?1",
                [&agent_id],
            )?;
            let deleted_configuration = tx.execute(
                "DELETE FROM acp_agents WHERE agent_id = ?1 AND connection_revision = ?2",
                params![agent_id, expected_revision],
            )?;
            let deleted_identity = tx.execute(
                "DELETE FROM agents WHERE agent_id = ?1 AND system_role IS NULL",
                [&agent_id],
            )?;
            if deleted_configuration != 1 || deleted_identity != 1 {
                return Err(StoreError::InvariantViolation {
                    message: "guarded ACP executor delete changed an unexpected row count"
                        .to_string(),
                });
            }
            Ok(true)
        })
        .await
    }

    /// Persist the bounded result of an ACP initialization probe.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale revision or a failed durable write.
    #[allow(
        clippy::too_many_arguments,
        reason = "the persisted probe is one flat ACP initialization response"
    )]
    pub async fn record_acp_agent_probe(
        &self,
        agent_id: &str,
        expected_revision: u64,
        health: AcpAgentHealthStatus,
        auth: AcpAgentAuthStatus,
        implementation_name: Option<&str>,
        implementation_version: Option<&str>,
        capabilities: &serde_json::Value,
        last_error: Option<&str>,
    ) -> Result<AcpAgentRecord, StoreError> {
        let capabilities_json = serde_json::to_string(capabilities)?;
        let last_error = last_error.map(|value| value.chars().take(512).collect::<String>());
        let agent_id = agent_id.to_string();
        self.with_immediate_transaction_retry(|tx| {
            let changed = tx.execute(
                "UPDATE acp_agents SET health_status = ?3, auth_status = ?4, implementation_name = ?5, implementation_version = ?6, capabilities_json = ?7, last_error = ?8, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE agent_id = ?1 AND connection_revision = ?2",
                params![agent_id, expected_revision, health.as_str(), auth.as_str(), implementation_name, implementation_version, capabilities_json, last_error],
            )?;
            if changed != 1 {
                return Err(StoreError::Work(
                    noema_tasks::WorkDomainError::IdempotencyConflict,
                ));
            }
            load_acp_agent_tx(tx, &agent_id)?.ok_or(StoreError::InvariantViolation {
                message: "probed ACP agent is unavailable".to_string(),
            })
    }).await
    }

    /// Persist one ACP authentication attempt and return its diagnostic identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid method or a failed durable write.
    pub async fn begin_acp_auth_attempt(
        &self,
        agent_id: &str,
        connection_revision: u64,
        method_id: &str,
    ) -> Result<String, StoreError> {
        let method_id = required_text(method_id, "ACP authentication method")?;
        let attempt_id = crate::ids::allocate_id("acp_auth");
        let agent_id = agent_id.to_string();
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO acp_auth_attempts (attempt_id, agent_id, connection_revision, method_id, state) VALUES (?1, ?2, ?3, ?4, 'pending')",
                params![attempt_id, agent_id, connection_revision, method_id],
            )?;
            Ok(attempt_id)
        })
        .await
    }

    /// Complete one ACP authentication attempt without storing credentials.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale attempt or a failed durable write.
    pub async fn finish_acp_auth_attempt(
        &self,
        attempt_id: &str,
        succeeded: bool,
        safe_message: Option<&str>,
    ) -> Result<AcpAgentRecord, StoreError> {
        let attempt_id = attempt_id.to_string();
        let safe_message = safe_message.map(|value| value.chars().take(512).collect::<String>());
        self.with_immediate_transaction_retry(|tx| {
            let row = tx
                .query_row(
                    "SELECT agent_id, connection_revision FROM acp_auth_attempts WHERE attempt_id = ?1 AND state = 'pending'",
                    [&attempt_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()?;
            let Some((agent_id, connection_revision)) = row else {
                return Err(StoreError::InvariantViolation {
                    message: "ACP authentication attempt is not pending".to_string(),
                });
            };
            let state = if succeeded { "completed" } else { "failed" };
            let failure_code = (!succeeded).then_some("acp_authentication_failed");
            tx.execute(
                "UPDATE acp_auth_attempts SET state = ?2, safe_message = ?3, failure_code = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE attempt_id = ?1",
                params![attempt_id, state, safe_message, failure_code],
            )?;
            let auth_status = if succeeded { "authenticated" } else { "failed" };
            tx.execute(
                "UPDATE acp_agents SET auth_status = ?3, last_error = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE agent_id = ?1 AND connection_revision = ?2",
                params![agent_id, connection_revision, auth_status, if succeeded { None } else { safe_message.as_deref() }],
            )?;
            load_acp_agent_tx(tx, &agent_id)?.ok_or(StoreError::InvariantViolation {
                message: "authenticated ACP agent is unavailable".to_string(),
            })
        })
        .await
    }

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

fn required_text(value: &str, label: &'static str) -> Result<String, StoreError> {
    let value = value.trim();
    if value.is_empty() || value.contains('\0') {
        return Err(StoreError::Schema(format!("{label} is required")));
    }
    Ok(value.to_string())
}

fn validate_arguments(arguments: &[String]) -> Result<(), StoreError> {
    if arguments.len() > 128
        || arguments
            .iter()
            .any(|argument| argument.len() > 4096 || argument.contains('\0'))
    {
        return Err(StoreError::Schema(
            "ACP arguments exceed the configured bounds".to_string(),
        ));
    }
    Ok(())
}

fn load_acp_agent_tx(
    conn: &rusqlite::Connection,
    agent_id: &str,
) -> Result<Option<AcpAgentRecord>, StoreError> {
    conn.query_row(
        "SELECT a.agent_id, a.display_name, c.command, c.arguments_json, c.enabled, c.auth_status, c.health_status, c.implementation_name, c.implementation_version, c.capabilities_json, c.connection_revision, c.last_error FROM acp_agents c JOIN agents a ON a.agent_id = c.agent_id WHERE c.agent_id = ?1",
        [agent_id],
        acp_agent_from_row,
    )
    .optional()
    .map_err(StoreError::Sqlite)
}

fn acp_agent_is_referenced(
    transaction: &rusqlite::Transaction<'_>,
    agent_id: &str,
) -> Result<bool, StoreError> {
    transaction
        .query_row(
            r#"
            SELECT EXISTS (
              SELECT 1
              FROM tasks AS task
              WHERE task.executor_agent_id = ?1
                AND task.stage_id NOT IN ('stage:personal:done', 'stage:personal:cancelled')
              UNION ALL
              SELECT 1
              FROM task_recurrences
              WHERE executor_agent_id = ?1
                AND lifecycle <> 'ended'
            )
            "#,
            [agent_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(StoreError::Sqlite)
}

fn acp_agent_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AcpAgentRecord> {
    let arguments_json: String = row.get(3)?;
    let capabilities_json: String = row.get(9)?;
    Ok(AcpAgentRecord {
        agent_id: row.get(0)?,
        display_name: row
            .get::<_, Option<String>>(1)?
            .unwrap_or_else(|| "ACP agent".to_string()),
        command: row.get(2)?,
        arguments: serde_json::from_str(&arguments_json)
            .map_err(|error| conversion_failure(3, rusqlite::types::Type::Text, error))?,
        enabled: row.get(4)?,
        auth_status: AcpAgentAuthStatus::parse(&row.get::<_, String>(5)?)
            .map_err(|error| conversion_failure(5, rusqlite::types::Type::Text, error))?,
        health_status: AcpAgentHealthStatus::parse(&row.get::<_, String>(6)?)
            .map_err(|error| conversion_failure(6, rusqlite::types::Type::Text, error))?,
        implementation_name: row.get(7)?,
        implementation_version: row.get(8)?,
        capabilities: serde_json::from_str(&capabilities_json)
            .map_err(|error| conversion_failure(9, rusqlite::types::Type::Text, error))?,
        connection_revision: row
            .get::<_, i64>(10)?
            .try_into()
            .map_err(|error| conversion_failure(10, rusqlite::types::Type::Integer, error))?,
        last_error: row.get(11)?,
    })
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
