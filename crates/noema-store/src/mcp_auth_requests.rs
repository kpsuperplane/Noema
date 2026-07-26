//! Durable MCP authentication interruptions.

use rusqlite::{OptionalExtension, Row, params};
use serde_json::Value;

use crate::{
    NoemaStore, StoreError, WorkRunFence,
    governed_action_approvals::mark_run_waiting_for_intervention_tx, ids::allocate_id,
    work_row::sha256_hex,
};

const MAX_ARGUMENTS_BYTES: usize = 1_048_576;
const MAX_SCHEMA_BYTES: usize = 262_144;

/// Durable state of an MCP sign-in interruption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    missing_docs,
    reason = "variants are the stable persisted state vocabulary"
)]
pub enum McpAuthenticationRequestState {
    AwaitingUser,
    Authorizing,
    Resuming,
    Completed,
    Cancelled,
    Superseded,
}

impl McpAuthenticationRequestState {
    /// Return the stable persisted state name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingUser => "awaiting_user",
            Self::Authorizing => "authorizing",
            Self::Resuming => "resuming",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "awaiting_user" => Ok(Self::AwaitingUser),
            "authorizing" => Ok(Self::Authorizing),
            "resuming" => Ok(Self::Resuming),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "superseded" => Ok(Self::Superseded),
            other => crate::ids::invalid_enum("mcp_authentication_request_state", other),
        }
    }
}

/// Exact private invocation retained while authentication is pending.
#[derive(Debug, Clone)]
#[allow(
    missing_docs,
    reason = "field names are the concrete authority vocabulary"
)]
pub struct NewMcpAuthenticationRequest {
    pub owner_human_id: String,
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub task_generation: Option<u64>,
    pub requesting_agent_id: String,
    pub mcp_server_id: String,
    pub capability_name: String,
    pub operation_token: String,
    pub input_schema: Value,
    pub arguments: Value,
    pub output_index: usize,
    pub call_id: Option<String>,
    pub provider_call_id: Option<String>,
    pub provider_name: Option<String>,
    pub governed_action: Option<(String, u64)>,
    pub result_context: Value,
}

/// Canonical MCP authentication request.
#[derive(Debug, Clone, PartialEq)]
#[allow(
    missing_docs,
    reason = "field names mirror the canonical persisted record"
)]
pub struct McpAuthenticationRequestRecord {
    pub request_id: String,
    pub revision: u64,
    pub owner_human_id: String,
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub task_generation: Option<u64>,
    pub requesting_agent_id: String,
    pub mcp_server_id: String,
    pub server_display_name: String,
    pub capability_name: String,
    pub operation_token: String,
    pub input_schema: Value,
    pub arguments: Value,
    pub arguments_sha256: String,
    pub output_index: usize,
    pub call_id: Option<String>,
    pub provider_call_id: Option<String>,
    pub provider_name: Option<String>,
    pub governed_action: Option<(String, u64)>,
    pub result_context: Option<Value>,
    pub oauth_attempt_id: Option<String>,
    pub state: McpAuthenticationRequestState,
    pub output: Option<Value>,
    pub failure_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl NoemaStore {
    /// Create one request per exact provider call and release a live task lease.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the request is invalid, stale, or cannot be persisted.
    pub async fn create_mcp_authentication_request(
        &self,
        input: NewMcpAuthenticationRequest,
        run_fence: Option<&WorkRunFence>,
    ) -> Result<McpAuthenticationRequestRecord, StoreError> {
        validate_new_request(&input)?;
        let arguments_json = serde_json::to_string(&input.arguments)?;
        let input_schema_json = serde_json::to_string(&input.input_schema)?;
        let result_context_json = serde_json::to_string(&input.result_context)?;
        if arguments_json.len() > MAX_ARGUMENTS_BYTES
            || input_schema_json.len() > MAX_SCHEMA_BYTES
            || result_context_json.len() > MAX_SCHEMA_BYTES
        {
            return Err(conflict("MCP authentication request payload is too large"));
        }
        let arguments_sha256 = sha256_hex(arguments_json.as_bytes());
        let request_id = allocate_id("mcp_auth");
        let output_index = i64::try_from(input.output_index)
            .map_err(|_| conflict("MCP authentication output index is invalid"))?;
        let governed_action_id = input.governed_action.as_ref().map(|value| value.0.as_str());
        let governed_action_revision = input
            .governed_action
            .as_ref()
            .map(|value| i64::try_from(value.1))
            .transpose()
            .map_err(|_| conflict("MCP authentication action revision is invalid"))?;
        let task_generation = input
            .task_generation
            .map(i64::try_from)
            .transpose()
            .map_err(|_| conflict("MCP authentication task generation is invalid"))?;
        self.with_immediate_transaction_retry(|transaction| {
            let existing = if let Some((action_id, revision)) = input.governed_action.as_ref() {
                request_by_governed_action(transaction, action_id, *revision)?
            } else {
                request_by_origin(
                    transaction,
                    input.conversation_id.as_deref(),
                    input.turn_id.as_deref(),
                    input.run_id.as_deref(),
                    output_index,
                )?
            };
            if let Some(existing) = existing {
                return Ok(existing);
            }
            if input.run_id.is_some() && run_fence.is_some() {
                mark_run_waiting_for_intervention_tx(
                    transaction,
                    input.task_id.as_deref().expect("validated task origin"),
                    input.run_id.as_deref().expect("validated task origin"),
                    run_fence,
                )?;
            } else if input.run_id.is_some() && input.governed_action.is_none() {
                return Err(conflict(
                    "task MCP authentication request has no live run fence",
                ));
            }
            transaction.execute(
                r#"
                INSERT INTO mcp_auth_requests (
                  request_id, owner_human_id, conversation_id, turn_id, task_id, run_id,
                  task_generation, requesting_agent_id, mcp_server_id, capability_name, operation_token,
                  input_schema_json, arguments_json, arguments_sha256, output_index,
                  call_id, provider_call_id, provider_name, governed_action_id,
                  governed_action_revision, result_context_json, state
                ) VALUES (
                  ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                  ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, 'awaiting_user'
                )
                "#,
                params![
                    request_id,
                    input.owner_human_id,
                    input.conversation_id,
                    input.turn_id,
                    input.task_id,
                    input.run_id,
                    task_generation,
                    input.requesting_agent_id,
                    input.mcp_server_id,
                    input.capability_name,
                    input.operation_token,
                    input_schema_json,
                    arguments_json,
                    arguments_sha256,
                    output_index,
                    input.call_id,
                    input.provider_call_id,
                    input.provider_name,
                    governed_action_id,
                    governed_action_revision,
                    result_context_json,
                ],
            )?;
            if let Some((action_id, revision)) = input.governed_action.as_ref() {
                let changed = transaction.execute(
                    "UPDATE governed_actions SET authentication_pending = 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE action_id = ?1 AND revision = ?2 AND state = 'executing' AND authentication_pending = 0",
                    params![action_id, revision],
                )?;
                if changed != 1 {
                    return Err(conflict("governed action cannot await authentication"));
                }
            }
            request_by_id(transaction, &request_id)?
                .ok_or_else(|| conflict("MCP authentication request disappeared during creation"))
        })
        .await
    }

    /// Read one exact request revision.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the request cannot be read.
    pub async fn get_mcp_authentication_request(
        &self,
        request_id: &str,
        revision: u64,
    ) -> Result<Option<McpAuthenticationRequestRecord>, StoreError> {
        self.with_connection(|connection| {
            let request = request_by_id(connection, request_id)?;
            Ok(request.filter(|request| request.revision == revision))
        })
        .await
    }

    /// List requests needing human attention.
    ///
    /// # Errors
    /// Returns [`StoreError`] when pending requests cannot be read.
    pub async fn list_pending_mcp_authentication_requests(
        &self,
        owner_human_id: &str,
        conversation_id: Option<&str>,
        task_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<McpAuthenticationRequestRecord>, StoreError> {
        let limit = i64::try_from(limit.clamp(1, 100)).unwrap_or(100);
        self.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                "{REQUEST_SELECT} WHERE requests.owner_human_id = ?1 AND requests.state IN ('awaiting_user', 'authorizing') AND (?2 IS NULL OR requests.conversation_id = ?2) AND (?3 IS NULL OR requests.task_id = ?3) ORDER BY requests.created_at DESC, requests.request_id DESC LIMIT ?4"
            ))?;
            statement
                .query_map(params![owner_human_id, conversation_id, task_id, limit], request_from_row)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
        .await
    }

    /// Return the active shared OAuth attempt for one owner and server.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the active attempt cannot be read.
    pub async fn active_mcp_authentication_attempt(
        &self,
        owner_human_id: &str,
        mcp_server_id: &str,
    ) -> Result<Option<String>, StoreError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT oauth_attempt_id FROM mcp_auth_requests WHERE owner_human_id = ?1 AND mcp_server_id = ?2 AND state = 'authorizing' AND oauth_attempt_id IS NOT NULL ORDER BY created_at LIMIT 1",
                    params![owner_human_id, mcp_server_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(Into::into)
        })
        .await
    }

    /// Bind one OAuth attempt to every pending request for the same server.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the request is stale or cannot be updated.
    pub async fn begin_mcp_authentication(
        &self,
        request_id: &str,
        revision: u64,
        owner_human_id: &str,
        attempt_id: &str,
    ) -> Result<McpAuthenticationRequestRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let request = request_by_id(transaction, request_id)?
                .ok_or_else(|| conflict("MCP authentication request was not found"))?;
            if request.revision != revision || request.owner_human_id != owner_human_id {
                return Err(conflict("MCP authentication request is stale"));
            }
            if !matches!(
                request.state,
                McpAuthenticationRequestState::AwaitingUser
                    | McpAuthenticationRequestState::Authorizing
            ) {
                return Err(conflict("MCP authentication request is already resolved"));
            }
            if request.state == McpAuthenticationRequestState::Authorizing {
                if request.oauth_attempt_id.as_deref() == Some(attempt_id) {
                    return Ok(request);
                }
                return Err(conflict("MCP authentication request is already authorizing"));
            }
            transaction.execute(
                "UPDATE mcp_auth_requests SET state = 'authorizing', oauth_attempt_id = ?3, failure_code = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE owner_human_id = ?1 AND mcp_server_id = ?2 AND state = 'awaiting_user'",
                params![owner_human_id, request.mcp_server_id, attempt_id],
            )?;
            request_by_id(transaction, request_id)?.ok_or_else(|| {
                conflict("MCP authentication request disappeared during authorization")
            })
        })
        .await
    }

    /// Return authorizing requests for one completed OAuth attempt.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the requests cannot be read.
    pub async fn list_mcp_authentication_requests_for_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Vec<McpAuthenticationRequestRecord>, StoreError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                "{REQUEST_SELECT} WHERE requests.oauth_attempt_id = ?1 AND requests.state = 'authorizing' ORDER BY requests.created_at, requests.request_id"
            ))?;
            statement
                .query_map([attempt_id], request_from_row)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
        .await
    }

    /// Return redispatches interrupted by a prior process exit.
    ///
    /// # Errors
    /// Returns [`StoreError`] when interrupted requests cannot be read.
    pub async fn list_interrupted_mcp_authentication_resumptions(
        &self,
    ) -> Result<Vec<McpAuthenticationRequestRecord>, StoreError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                "{REQUEST_SELECT} WHERE requests.state = 'resuming' ORDER BY requests.created_at, requests.request_id"
            ))?;
            statement
                .query_map([], request_from_row)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
        .await
    }

    /// Claim one request for exactly one post-authentication dispatch.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the request is stale or already claimed.
    pub async fn claim_mcp_authentication_resumption(
        &self,
        request_id: &str,
        revision: u64,
    ) -> Result<McpAuthenticationRequestRecord, StoreError> {
        transition_request(
            self,
            request_id,
            revision,
            "authorizing",
            "resuming",
            None,
            None,
        )
        .await
    }

    /// Complete a claimed request with the exact tool outcome.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the terminal state or request claim is invalid.
    pub async fn finish_mcp_authentication_request(
        &self,
        request_id: &str,
        revision: u64,
        state: McpAuthenticationRequestState,
        output: Option<&Value>,
        failure_code: Option<&str>,
    ) -> Result<McpAuthenticationRequestRecord, StoreError> {
        if !matches!(
            state,
            McpAuthenticationRequestState::Completed | McpAuthenticationRequestState::Superseded
        ) {
            return Err(conflict("invalid MCP authentication terminal state"));
        }
        transition_request(
            self,
            request_id,
            revision,
            "resuming",
            state.as_str(),
            output,
            failure_code,
        )
        .await
    }

    /// Return a claimed request to human attention after authentication is still required.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the request is stale or cannot be updated.
    pub async fn retry_mcp_authentication_request(
        &self,
        request_id: &str,
        revision: u64,
    ) -> Result<McpAuthenticationRequestRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                "UPDATE mcp_auth_requests SET state = 'awaiting_user', oauth_attempt_id = NULL, failure_code = 'authentication_required', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE request_id = ?1 AND revision = ?2 AND state = 'resuming'",
                params![request_id, revision],
            )?;
            if changed != 1 {
                return Err(conflict("MCP authentication retry is stale"));
            }
            request_by_id(transaction, request_id)?.ok_or_else(|| {
                conflict("MCP authentication request disappeared during retry")
            })
        })
        .await
    }

    /// Cancel a pending request without dispatching the saved invocation.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the owner, revision, or request state is stale.
    pub async fn cancel_mcp_authentication_request(
        &self,
        request_id: &str,
        revision: u64,
        owner_human_id: &str,
    ) -> Result<McpAuthenticationRequestRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let request = request_by_id(transaction, request_id)?
                .ok_or_else(|| conflict("MCP authentication request was not found"))?;
            if request.revision != revision || request.owner_human_id != owner_human_id {
                return Err(conflict("MCP authentication request is stale"));
            }
            let changed = transaction.execute(
                "UPDATE mcp_auth_requests SET state = 'cancelled', output_json = '{\"code\":\"authentication_skipped\"}', failure_code = 'authentication_skipped', completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE request_id = ?1 AND revision = ?2 AND state IN ('awaiting_user', 'authorizing')",
                params![request_id, revision],
            )?;
            if changed != 1 {
                return Err(conflict("MCP authentication request is already resolved"));
            }
            request_by_id(transaction, request_id)?.ok_or_else(|| {
                conflict("MCP authentication request disappeared during cancellation")
            })
        })
        .await
    }

    /// Return an unsuccessful OAuth attempt to its retryable state.
    ///
    /// # Errors
    /// Returns [`StoreError`] when affected requests cannot be updated.
    pub async fn reset_mcp_authentication_attempt(
        &self,
        attempt_id: &str,
        failure_code: &str,
    ) -> Result<(), StoreError> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE mcp_auth_requests SET state = 'awaiting_user', oauth_attempt_id = NULL, failure_code = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE oauth_attempt_id = ?1 AND state = 'authorizing'",
                params![attempt_id, failure_code],
            )?;
            Ok(())
        })
        .await
    }
}

async fn transition_request(
    store: &NoemaStore,
    request_id: &str,
    revision: u64,
    from: &str,
    to: &str,
    output: Option<&Value>,
    failure_code: Option<&str>,
) -> Result<McpAuthenticationRequestRecord, StoreError> {
    let output = output.map(serde_json::to_string).transpose()?;
    store
        .with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                "UPDATE mcp_auth_requests SET state = ?4, output_json = ?5, failure_code = ?6, completed_at = CASE WHEN ?4 IN ('completed', 'cancelled', 'superseded') THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE NULL END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE request_id = ?1 AND revision = ?2 AND state = ?3",
                params![request_id, revision, from, to, output, failure_code],
            )?;
            if changed != 1 {
                return Err(conflict("MCP authentication request transition is stale"));
            }
            request_by_id(transaction, request_id)?.ok_or_else(|| {
                conflict("MCP authentication request disappeared during transition")
            })
        })
        .await
}

fn validate_new_request(input: &NewMcpAuthenticationRequest) -> Result<(), StoreError> {
    let foreground = input.conversation_id.is_some()
        && input.turn_id.is_some()
        && input.task_id.is_none()
        && input.run_id.is_none()
        && input.task_generation.is_none();
    let task = input.conversation_id.is_none()
        && input.turn_id.is_none()
        && input.task_id.is_some()
        && input.run_id.is_some()
        && input
            .task_generation
            .is_some_and(|generation| generation > 0);
    if (!foreground && !task)
        || input.owner_human_id.trim().is_empty()
        || input.requesting_agent_id.trim().is_empty()
        || input.mcp_server_id.trim().is_empty()
        || input.capability_name.trim().is_empty()
        || input.operation_token.trim().is_empty()
    {
        return Err(conflict("invalid MCP authentication request"));
    }
    Ok(())
}

const REQUEST_SELECT: &str = r#"
SELECT requests.request_id, requests.revision, requests.owner_human_id,
       requests.conversation_id, requests.turn_id, requests.task_id, requests.run_id,
       requests.task_generation,
       requests.requesting_agent_id, requests.mcp_server_id, servers.display_name,
       requests.capability_name, requests.operation_token, requests.input_schema_json,
       requests.arguments_json, requests.arguments_sha256, requests.output_index,
       requests.call_id, requests.provider_call_id, requests.provider_name,
       requests.governed_action_id, requests.governed_action_revision,
       requests.oauth_attempt_id, requests.state, requests.output_json,
       requests.failure_code, requests.created_at, requests.updated_at,
       requests.result_context_json
FROM mcp_auth_requests requests
JOIN mcp_servers servers ON servers.mcp_server_id = requests.mcp_server_id
"#;

fn request_by_id(
    connection: &rusqlite::Connection,
    request_id: &str,
) -> Result<Option<McpAuthenticationRequestRecord>, StoreError> {
    connection
        .query_row(
            &format!("{REQUEST_SELECT} WHERE requests.request_id = ?1"),
            [request_id],
            request_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn request_by_origin(
    connection: &rusqlite::Connection,
    conversation_id: Option<&str>,
    turn_id: Option<&str>,
    run_id: Option<&str>,
    output_index: i64,
) -> Result<Option<McpAuthenticationRequestRecord>, StoreError> {
    connection
        .query_row(
            &format!(
                "{REQUEST_SELECT} WHERE (requests.conversation_id = ?1 AND requests.turn_id = ?2 AND requests.output_index = ?4) OR (requests.run_id = ?3 AND requests.output_index = ?4)"
            ),
            params![conversation_id, turn_id, run_id, output_index],
            request_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn request_by_governed_action(
    connection: &rusqlite::Connection,
    action_id: &str,
    revision: u64,
) -> Result<Option<McpAuthenticationRequestRecord>, StoreError> {
    let revision = i64::try_from(revision)
        .map_err(|_| conflict("MCP authentication action revision is invalid"))?;
    connection
        .query_row(
            &format!(
                "{REQUEST_SELECT} WHERE requests.governed_action_id = ?1 AND requests.governed_action_revision = ?2"
            ),
            params![action_id, revision],
            request_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn request_from_row(row: &Row<'_>) -> rusqlite::Result<McpAuthenticationRequestRecord> {
    let revision = row.get::<_, i64>(1)?;
    let task_generation = row.get::<_, Option<i64>>(7)?;
    let output_index = row.get::<_, i64>(16)?;
    let governed_action_id = row.get::<_, Option<String>>(20)?;
    let governed_action_revision = row.get::<_, Option<i64>>(21)?;
    Ok(McpAuthenticationRequestRecord {
        request_id: row.get(0)?,
        revision: u64::try_from(revision)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(1, revision))?,
        owner_human_id: row.get(2)?,
        conversation_id: row.get(3)?,
        turn_id: row.get(4)?,
        task_id: row.get(5)?,
        run_id: row.get(6)?,
        task_generation: task_generation
            .map(|generation| {
                u64::try_from(generation)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(7, generation))
            })
            .transpose()?,
        requesting_agent_id: row.get(8)?,
        mcp_server_id: row.get(9)?,
        server_display_name: row.get(10)?,
        capability_name: row.get(11)?,
        operation_token: row.get(12)?,
        input_schema: serde_json::from_str(&row.get::<_, String>(13)?).map_err(json_error)?,
        arguments: serde_json::from_str(&row.get::<_, String>(14)?).map_err(json_error)?,
        arguments_sha256: row.get(15)?,
        output_index: usize::try_from(output_index)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(16, output_index))?,
        call_id: row.get(17)?,
        provider_call_id: row.get(18)?,
        provider_name: row.get(19)?,
        governed_action: match (governed_action_id, governed_action_revision) {
            (Some(id), Some(revision)) => Some((
                id,
                u64::try_from(revision)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(21, revision))?,
            )),
            _ => None,
        },
        oauth_attempt_id: row.get(22)?,
        state: McpAuthenticationRequestState::parse(&row.get::<_, String>(23)?).map_err(
            |error| {
                rusqlite::Error::FromSqlConversionFailure(
                    23,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            },
        )?,
        output: row
            .get::<_, Option<String>>(24)?
            .map(|value| serde_json::from_str(&value).map_err(json_error))
            .transpose()?,
        failure_code: row.get(25)?,
        created_at: row.get(26)?,
        updated_at: row.get(27)?,
        result_context: row
            .get::<_, Option<String>>(28)?
            .map(|value| serde_json::from_str(&value).map_err(json_error))
            .transpose()?,
    })
}

fn json_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn conflict(message: impl Into<String>) -> StoreError {
    StoreError::InvariantViolation {
        message: message.into(),
    }
}
