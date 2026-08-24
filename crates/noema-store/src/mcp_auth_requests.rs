//! Durable capability authentication interruptions.

use noema_capabilities::{
    CapabilityAuthenticationAuthorityKind, CapabilityAuthenticationChallenge,
    CapabilityAuthenticationChallengeKind,
};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use crate::{
    NoemaStore, StoreError, WorkRunFence,
    governed_action_approvals::mark_run_waiting_for_intervention_tx, ids::allocate_id,
};

mod rows;
use rows::{
    REQUEST_SELECT, request_by_governed_action, request_by_id, request_by_origin, request_from_row,
};

const MAX_SCHEMA_BYTES: usize = 262_144;

/// Durable state of a capability authentication interruption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    missing_docs,
    reason = "variants are the stable persisted state vocabulary"
)]
pub enum CapabilityAuthenticationRequestState {
    AwaitingUser,
    Authorizing,
    Resuming,
    Completed,
    Cancelled,
    Superseded,
}

impl CapabilityAuthenticationRequestState {
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
            other => crate::ids::invalid_enum("capability_authentication_request_state", other),
        }
    }
}

/// Non-secret durable metadata for an invocation retained in protected storage.
#[derive(Debug, Clone)]
#[allow(
    missing_docs,
    reason = "field names are the concrete authority vocabulary"
)]
pub struct NewCapabilityAuthenticationRequest {
    pub owner_human_id: String,
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub task_generation: Option<u64>,
    pub requesting_agent_id: String,
    pub challenge: CapabilityAuthenticationChallenge,
    pub capability_name: String,
    pub operation_token: String,
    pub input_schema: Value,
    pub protected_arguments_ref: String,
    pub arguments_sha256: String,
    pub provider_selection_digest: String,
    pub output_index: usize,
    pub call_id: Option<String>,
    pub provider_call_id: Option<String>,
    pub provider_name: Option<String>,
    pub governed_action: Option<(String, u64)>,
    pub result_context: Value,
}

/// Canonical capability authentication request.
#[derive(Debug, Clone, PartialEq)]
#[allow(
    missing_docs,
    reason = "field names mirror the canonical persisted record"
)]
pub struct CapabilityAuthenticationRequestRecord {
    pub request_id: String,
    pub revision: u64,
    pub owner_human_id: String,
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub task_generation: Option<u64>,
    pub requesting_agent_id: String,
    pub challenge: CapabilityAuthenticationChallenge,
    pub authority_display_name: String,
    pub capability_name: String,
    pub operation_token: String,
    pub input_schema: Value,
    pub protected_arguments_ref: String,
    pub arguments_sha256: String,
    pub provider_selection_digest: String,
    pub output_index: usize,
    pub call_id: Option<String>,
    pub provider_call_id: Option<String>,
    pub provider_name: Option<String>,
    pub governed_action: Option<(String, u64)>,
    pub result_context: Value,
    pub authentication_attempt_id: Option<String>,
    pub state: CapabilityAuthenticationRequestState,
    pub output: Option<Value>,
    pub failure_code: Option<String>,
    pub supersession_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl CapabilityAuthenticationRequestRecord {
    /// Return the MCP server authority when this request belongs to MCP.
    #[must_use]
    pub fn mcp_server_id(&self) -> Option<&str> {
        (self.challenge.authority_kind() == CapabilityAuthenticationAuthorityKind::McpServer)
            .then(|| self.challenge.authority_id())
    }

    /// Return the adapter connection authority when this request belongs to an API adapter.
    #[must_use]
    pub fn adapter_connection_id(&self) -> Option<&str> {
        (self.challenge.authority_kind()
            == CapabilityAuthenticationAuthorityKind::AdapterConnection)
            .then(|| self.challenge.authority_id())
    }

    /// Return the reusable API grant authority for an OAuth request.
    #[must_use]
    pub fn adapter_grant_id(&self) -> Option<&str> {
        (self.challenge.authority_kind() == CapabilityAuthenticationAuthorityKind::AdapterGrant)
            .then(|| self.challenge.authority_id())
    }
}

impl NoemaStore {
    /// Create one request per exact provider call and release a live task lease.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the request is invalid, stale, or cannot be persisted.
    pub async fn create_capability_authentication_request(
        &self,
        input: NewCapabilityAuthenticationRequest,
        run_fence: Option<&WorkRunFence>,
    ) -> Result<CapabilityAuthenticationRequestRecord, StoreError> {
        validate_new_request(&input)?;
        let input_schema_json = serde_json::to_string(&input.input_schema)?;
        let result_context_json = serde_json::to_string(&input.result_context)?;
        if input_schema_json.len() > MAX_SCHEMA_BYTES
            || result_context_json.len() > MAX_SCHEMA_BYTES
        {
            return Err(conflict(
                "capability authentication request payload is too large",
            ));
        }
        let request_id = allocate_id("cap_auth");
        let output_index = i64::try_from(input.output_index)
            .map_err(|_| conflict("capability authentication output index is invalid"))?;
        let governed_action_id = input.governed_action.as_ref().map(|value| value.0.as_str());
        let governed_action_revision = input
            .governed_action
            .as_ref()
            .map(|value| i64::try_from(value.1))
            .transpose()
            .map_err(|_| conflict("capability authentication action revision is invalid"))?;
        let task_generation = input
            .task_generation
            .map(i64::try_from)
            .transpose()
            .map_err(|_| conflict("capability authentication task generation is invalid"))?;
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
                    "task capability authentication request has no live run fence",
                ));
            }
            transaction.execute(
                r#"
                INSERT INTO capability_auth_requests (
                  request_id, owner_human_id, conversation_id, turn_id, task_id, run_id,
                  task_generation, requesting_agent_id, authority_kind, authority_id,
                  destination_id, challenge_kind, destination_revision, capability_name, operation_token,
                  input_schema_json, protected_arguments_ref, arguments_sha256,
                  provider_selection_digest, output_index,
                  call_id, provider_call_id, provider_name, governed_action_id,
                  governed_action_revision, result_context_json, state
                ) VALUES (
                  ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                  ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                  ?21, ?22, ?23, ?24, ?25, ?26, 'awaiting_user'
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
                    input.challenge.authority_kind().as_str(),
                    input.challenge.authority_id(),
                    input.challenge.destination_id(),
                    input.challenge.challenge_kind().as_str(),
                    input.challenge.authority_revision(),
                    input.capability_name,
                    input.operation_token,
                    input_schema_json,
                    input.protected_arguments_ref,
                    input.arguments_sha256,
                    input.provider_selection_digest,
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
                .ok_or_else(|| conflict("capability authentication request disappeared during creation"))
        })
        .await
    }

    /// Read one exact request revision.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the request cannot be read.
    pub async fn get_capability_authentication_request(
        &self,
        request_id: &str,
        revision: u64,
    ) -> Result<Option<CapabilityAuthenticationRequestRecord>, StoreError> {
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
    pub async fn list_pending_capability_authentication_requests(
        &self,
        owner_human_id: &str,
        conversation_id: Option<&str>,
        task_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<CapabilityAuthenticationRequestRecord>, StoreError> {
        let limit = i64::try_from(limit.clamp(1, 100)).unwrap_or(100);
        self.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                r#"
                WITH grouped_requests AS (
                  SELECT request_id,
                    ROW_NUMBER() OVER (
                      PARTITION BY authority_kind, authority_id, challenge_kind
                      ORDER BY CASE state WHEN 'authorizing' THEN 0 ELSE 1 END,
                               created_at, request_id
                    ) AS authority_rank
                  FROM capability_auth_requests
                  WHERE owner_human_id = ?1
                    AND state IN ('awaiting_user', 'authorizing')
                    AND (?2 IS NULL OR conversation_id = ?2)
                    AND (?3 IS NULL OR task_id = ?3)
                )
                {REQUEST_SELECT}
                JOIN grouped_requests grouped ON grouped.request_id = requests.request_id
                WHERE grouped.authority_rank = 1
                ORDER BY CASE requests.state WHEN 'authorizing' THEN 0 ELSE 1 END,
                         requests.created_at, requests.request_id
                LIMIT ?4
                "#
            ))?;
            statement
                .query_map(
                    params![owner_human_id, conversation_id, task_id, limit],
                    request_from_row,
                )?
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
                    "SELECT authentication_attempt_id FROM capability_auth_requests WHERE owner_human_id = ?1 AND authority_kind = 'mcp_server' AND authority_id = ?2 AND state = 'authorizing' AND authentication_attempt_id IS NOT NULL ORDER BY created_at LIMIT 1",
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
    pub async fn begin_capability_authentication(
        &self,
        request_id: &str,
        revision: u64,
        owner_human_id: &str,
        attempt_id: &str,
    ) -> Result<CapabilityAuthenticationRequestRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let request = request_by_id(transaction, request_id)?
                .ok_or_else(|| conflict("capability authentication request was not found"))?;
            if request.revision != revision || request.owner_human_id != owner_human_id {
                return Err(conflict("capability authentication request is stale"));
            }
            if !matches!(
                request.state,
                CapabilityAuthenticationRequestState::AwaitingUser
                    | CapabilityAuthenticationRequestState::Authorizing
            ) {
                return Err(conflict("capability authentication request is already resolved"));
            }
            if request.state == CapabilityAuthenticationRequestState::Authorizing
                && request.authentication_attempt_id.as_deref() != Some(attempt_id)
                && request.challenge.authority_kind()
                    == CapabilityAuthenticationAuthorityKind::McpServer
            {
                return Err(conflict(
                    "capability authentication request is already authorizing",
                ));
            }
            transaction.execute(
                "UPDATE capability_auth_requests SET state = 'authorizing', authentication_attempt_id = ?5, failure_code = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE owner_human_id = ?1 AND authority_kind = ?2 AND authority_id = ?3 AND challenge_kind = ?4 AND state IN ('awaiting_user', 'authorizing')",
                params![
                    owner_human_id,
                    request.challenge.authority_kind().as_str(),
                    request.challenge.authority_id(),
                    request.challenge.challenge_kind().as_str(),
                    attempt_id,
                ],
            )?;
            request_by_id(transaction, request_id)?.ok_or_else(|| {
                conflict("capability authentication request disappeared during authorization")
            })
        })
        .await
    }

    /// Bind late matching requests and return every request for one OAuth attempt.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the requests cannot be read.
    pub async fn list_capability_authentication_requests_for_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Vec<CapabilityAuthenticationRequestRecord>, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let authority = transaction
                .query_row(
                    "SELECT owner_human_id, authority_kind, authority_id, challenge_kind FROM capability_auth_requests WHERE authentication_attempt_id = ?1 ORDER BY CASE state WHEN 'authorizing' THEN 0 ELSE 1 END, created_at, request_id LIMIT 1",
                    [attempt_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)),
                )
                .optional()?;
            if let Some((owner_human_id, authority_kind, authority_id, challenge_kind)) = authority {
                transaction.execute(
                    "UPDATE capability_auth_requests SET state = 'authorizing', authentication_attempt_id = ?5, failure_code = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE owner_human_id = ?1 AND authority_kind = ?2 AND authority_id = ?3 AND challenge_kind = ?4 AND state = 'awaiting_user'",
                    params![owner_human_id, authority_kind, authority_id, challenge_kind, attempt_id],
                )?;
            }
            let mut statement = transaction.prepare(&format!(
                "{REQUEST_SELECT} WHERE requests.authentication_attempt_id = ?1 AND requests.state = 'authorizing' ORDER BY requests.created_at, requests.request_id"
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
    pub async fn list_interrupted_capability_authentication_resumptions(
        &self,
    ) -> Result<Vec<CapabilityAuthenticationRequestRecord>, StoreError> {
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

    /// Return terminal requests whose durable origin has not been resumed.
    ///
    /// # Errors
    /// Returns [`StoreError`] when interrupted requests cannot be read.
    pub async fn list_unpublished_capability_authentication_origins(
        &self,
    ) -> Result<Vec<CapabilityAuthenticationRequestRecord>, StoreError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                "{REQUEST_SELECT} WHERE requests.origin_resumed_at IS NULL AND requests.state IN ('completed', 'cancelled', 'superseded') ORDER BY requests.created_at, requests.request_id"
            ))?;
            statement
                .query_map([], request_from_row)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
        .await
    }

    /// List protected argument references still needed by active or unpublished requests.
    ///
    /// # Errors
    /// Returns [`StoreError`] when references cannot be read.
    pub async fn list_active_capability_authentication_argument_references(
        &self,
    ) -> Result<Vec<String>, StoreError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT protected_arguments_ref FROM capability_auth_requests WHERE origin_resumed_at IS NULL ORDER BY protected_arguments_ref",
            )?;
            statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
        .await
    }

    /// Claim one request for exactly one post-authentication dispatch.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the request is stale or already claimed.
    pub async fn claim_capability_authentication_resumption(
        &self,
        request_id: &str,
        revision: u64,
    ) -> Result<CapabilityAuthenticationRequestRecord, StoreError> {
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
    pub async fn finish_capability_authentication_request(
        &self,
        request_id: &str,
        revision: u64,
        state: CapabilityAuthenticationRequestState,
        output: Option<&Value>,
        failure_code: Option<&str>,
    ) -> Result<CapabilityAuthenticationRequestRecord, StoreError> {
        if !matches!(
            state,
            CapabilityAuthenticationRequestState::Completed
                | CapabilityAuthenticationRequestState::Superseded
        ) {
            return Err(conflict("invalid capability authentication terminal state"));
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
    pub async fn retry_capability_authentication_request(
        &self,
        request_id: &str,
        revision: u64,
    ) -> Result<CapabilityAuthenticationRequestRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                "UPDATE capability_auth_requests SET state = 'awaiting_user', authentication_attempt_id = NULL, failure_code = 'authentication_required', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE request_id = ?1 AND revision = ?2 AND state = 'resuming'",
                params![request_id, revision],
            )?;
            if changed != 1 {
                return Err(conflict("capability authentication retry is stale"));
            }
            request_by_id(transaction, request_id)?.ok_or_else(|| {
                conflict("capability authentication request disappeared during retry")
            })
        })
        .await
    }

    /// Cancel a pending request without dispatching the saved invocation.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the owner, revision, or request state is stale.
    pub async fn cancel_capability_authentication_request(
        &self,
        request_id: &str,
        revision: u64,
        owner_human_id: &str,
    ) -> Result<CapabilityAuthenticationRequestRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let request = request_by_id(transaction, request_id)?
                .ok_or_else(|| conflict("capability authentication request was not found"))?;
            if request.revision != revision || request.owner_human_id != owner_human_id {
                return Err(conflict("capability authentication request is stale"));
            }
            let changed = transaction.execute(
                "UPDATE capability_auth_requests SET state = 'cancelled', output_json = '{\"code\":\"authentication_skipped\"}', failure_code = 'authentication_skipped', completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE request_id = ?1 AND revision = ?2 AND state IN ('awaiting_user', 'authorizing')",
                params![request_id, revision],
            )?;
            if changed != 1 {
                return Err(conflict("capability authentication request is already resolved"));
            }
            request_by_id(transaction, request_id)?.ok_or_else(|| {
                conflict("capability authentication request disappeared during cancellation")
            })
        })
        .await
    }

    /// Mark one terminal request's durable origin publication complete.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the request is stale, non-terminal, or already published.
    pub async fn mark_capability_authentication_origin_resumed(
        &self,
        request_id: &str,
        revision: u64,
    ) -> Result<(), StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                "UPDATE capability_auth_requests SET origin_resumed_at = COALESCE(origin_resumed_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE request_id = ?1 AND revision = ?2 AND state IN ('completed', 'cancelled', 'superseded')",
                params![request_id, revision],
            )?;
            if changed != 1 {
                return Err(conflict("capability authentication origin is not terminal"));
            }
            Ok(())
        })
        .await
    }

    /// Return an unsuccessful OAuth attempt to its retryable state.
    ///
    /// # Errors
    /// Returns [`StoreError`] when affected requests cannot be updated.
    pub async fn reset_capability_authentication_attempt(
        &self,
        attempt_id: &str,
        failure_code: &str,
    ) -> Result<(), StoreError> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE capability_auth_requests SET state = 'awaiting_user', authentication_attempt_id = NULL, failure_code = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE authentication_attempt_id = ?1 AND state = 'authorizing'",
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
) -> Result<CapabilityAuthenticationRequestRecord, StoreError> {
    let output = output.map(serde_json::to_string).transpose()?;
    store
        .with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                "UPDATE capability_auth_requests SET state = ?4, output_json = ?5, failure_code = ?6, supersession_reason = CASE WHEN ?4 = 'superseded' THEN ?6 ELSE NULL END, completed_at = CASE WHEN ?4 IN ('completed', 'cancelled', 'superseded') THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE NULL END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE request_id = ?1 AND revision = ?2 AND state = ?3",
                params![request_id, revision, from, to, output, failure_code],
            )?;
            if changed != 1 {
                return Err(conflict("capability authentication request transition is stale"));
            }
            request_by_id(transaction, request_id)?.ok_or_else(|| {
                conflict("capability authentication request disappeared during transition")
            })
        })
        .await
}

fn validate_new_request(input: &NewCapabilityAuthenticationRequest) -> Result<(), StoreError> {
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
        || input.capability_name.trim().is_empty()
        || input.operation_token.trim().is_empty()
        || !is_lower_hex(&input.protected_arguments_ref, 32)
        || !is_lower_hex(&input.arguments_sha256, 64)
        || !is_lower_hex(&input.provider_selection_digest, 64)
    {
        return Err(conflict("invalid capability authentication request"));
    }
    Ok(())
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn conflict(message: impl Into<String>) -> StoreError {
    StoreError::InvariantViolation {
        message: message.into(),
    }
}
