//! Durable authority for reviewed tool invocations.

use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Map, Value, json};

use crate::{
    NoemaStore, StoreError, WorkRunFence, authorization_context::MAX_AUTHORIZATION_CONTEXT_BYTES,
    governed_action_approvals::mark_origin_run_waiting_tx,
    governed_action_fencing::require_origin_execution_live_tx, ids::allocate_id,
    work_row::sha256_hex,
};

const MAX_ARGUMENTS_BYTES: usize = 1_048_576;
const MAX_SCHEMA_BYTES: usize = 262_144;
const MAX_EXPLANATION_CHARS: usize = 4_000;
const MAX_REASON_CODES: usize = 16;
const MAX_REVIEW_FIELDS: usize = 128;
const MAX_REVIEW_ITEMS: usize = 16;
const MAX_REVIEW_DEPTH: usize = 8;

/// Review route that originated a durable action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionReviewRoute {
    /// A human must approve the exact proposal.
    HumanReview,
    /// A reviewer model evaluates the exact proposal first.
    LlmReview,
}

impl ExecutionReviewRoute {
    fn as_str(self) -> &'static str {
        match self {
            Self::HumanReview => "human_review",
            Self::LlmReview => "llm_review",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "human_review" => Ok(Self::HumanReview),
            "llm_review" => Ok(Self::LlmReview),
            other => crate::ids::invalid_enum("execution_review_route", other),
        }
    }
}

/// Complete tool behavior snapshot retained with a new reviewed action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredToolBehavior {
    /// The operation does not modify state.
    pub read_only: bool,
    /// Repeating identical arguments has no additional effect.
    pub idempotent: bool,
    /// The operation can perform a destructive change.
    pub destructive: bool,
    /// The operation may interact with an unbounded external world.
    pub open_world: bool,
}

/// Durable action lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernedActionState {
    /// Persisted before review.
    Proposed,
    /// Waiting for a human decision.
    AwaitingApproval,
    /// Cleared and available for one execution claim.
    Executable,
    /// Claimed by the runtime.
    Executing,
    /// Approved execution paused for MCP credentials.
    AwaitingAuthentication,
    /// Completed successfully.
    Succeeded,
    /// Completed with a known failure.
    Failed,
    /// Sent externally without a known outcome.
    OutcomeUncertain,
    /// Declined by the human.
    Declined,
    /// Invalidated by a newer authority revision.
    Superseded,
    /// Cancelled with its owning execution.
    Cancelled,
}

impl GovernedActionState {
    /// Return the stable persisted state name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Executable => "executable",
            Self::Executing => "executing",
            Self::AwaitingAuthentication => "awaiting_authentication",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::OutcomeUncertain => "outcome_uncertain",
            Self::Declined => "declined",
            Self::Superseded => "superseded",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "proposed" => Ok(Self::Proposed),
            "awaiting_approval" => Ok(Self::AwaitingApproval),
            "executable" => Ok(Self::Executable),
            "executing" => Ok(Self::Executing),
            "awaiting_authentication" => Ok(Self::AwaitingAuthentication),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "outcome_uncertain" => Ok(Self::OutcomeUncertain),
            "declined" => Ok(Self::Declined),
            "superseded" => Ok(Self::Superseded),
            "cancelled" => Ok(Self::Cancelled),
            other => crate::ids::invalid_enum("governed_action_state", other),
        }
    }
}

/// Immutable proposed action.
#[derive(Debug, Clone)]
pub struct NewGovernedAction {
    /// Human who owns the approval decision.
    pub owner_human_id: String,
    /// Foreground conversation origin.
    pub conversation_id: Option<String>,
    /// Foreground turn origin.
    pub turn_id: Option<String>,
    /// Background task origin.
    pub task_id: Option<String>,
    /// Background run origin.
    pub run_id: Option<String>,
    /// Agent that requested the action.
    pub requesting_agent_id: String,
    /// Exact canonical capability name.
    pub capability_name: String,
    /// Exact binding authority token.
    pub operation_token: String,
    /// Review route that originated this action.
    pub review_route: ExecutionReviewRoute,
    /// Complete behavior snapshot at proposal time.
    pub behavior: StoredToolBehavior,
    /// Exact proposed arguments.
    pub arguments: Value,
    /// Exact advertised input schema.
    pub input_schema: Value,
    /// Structured human authority plus bounded role-labelled context.
    pub authorization_context: Value,
    /// Payload-free summary safe for attention surfaces.
    pub safe_summary: String,
}

/// Canonical persisted action revision.
#[derive(Debug, Clone, PartialEq)]
pub struct GovernedActionRecord {
    /// Stable action id.
    pub action_id: String,
    /// Immutable revision.
    pub revision: u64,
    /// Human who owns the approval decision.
    pub owner_human_id: String,
    /// Foreground conversation origin.
    pub conversation_id: Option<String>,
    /// Foreground turn origin.
    pub turn_id: Option<String>,
    /// Background task origin.
    pub task_id: Option<String>,
    /// Background run origin.
    pub run_id: Option<String>,
    /// Agent that requested the action.
    pub requesting_agent_id: String,
    /// Exact canonical capability name.
    pub capability_name: String,
    /// Exact binding authority token.
    pub operation_token: String,
    /// Review route that originated this action.
    pub review_route: ExecutionReviewRoute,
    /// Complete behavior snapshot when one was available.
    pub behavior: Option<StoredToolBehavior>,
    /// Exact proposed arguments.
    pub arguments: Value,
    /// Digest of the serialized exact arguments.
    pub arguments_sha256: String,
    /// Exact advertised input schema.
    pub input_schema: Value,
    /// Structured human authority plus bounded role-labelled context.
    pub authorization_context: Value,
    /// Payload-free summary safe for attention surfaces.
    pub safe_summary: String,
    /// Current action state.
    pub state: GovernedActionState,
    /// Exact terminal output, when known.
    pub output: Option<Value>,
    /// Stable terminal failure category.
    pub failure_code: Option<String>,
    /// Reviewer assessment that determined whether approval was required.
    pub assessment: Option<GovernedActionAssessmentRecord>,
}

impl GovernedActionRecord {
    /// Return bounded, value-free argument metadata for reviewer shape summaries.
    /// Exact arguments remain the governed action and human-review authority.
    #[must_use]
    pub fn safe_arguments(&self) -> Value {
        safe_value_projection(&self.arguments)
    }

    /// Return bounded, value-free authorization-context metadata for display
    /// surfaces that do not own the private authority text.
    #[must_use]
    pub fn safe_authorization_context(&self) -> Value {
        safe_value_projection(&self.authorization_context)
    }
}

/// Reviewer execution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernedAssessmentStatus {
    /// Reviewer returned a valid closed-schema result.
    Completed,
    /// Reviewer selection or provider was unavailable.
    ReviewerUnavailable,
    /// Reviewer output violated the closed schema.
    InvalidResponse,
}

impl GovernedAssessmentStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::ReviewerUnavailable => "reviewer_unavailable",
            Self::InvalidResponse => "invalid_response",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "completed" => Ok(Self::Completed),
            "reviewer_unavailable" => Ok(Self::ReviewerUnavailable),
            "invalid_response" => Ok(Self::InvalidResponse),
            other => crate::ids::invalid_enum("governed_assessment_status", other),
        }
    }
}

/// Closed reviewer authorization assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernedAuthorization {
    /// Exact action was directly authorized.
    Explicit,
    /// The request substantively covers the exact action.
    Substantive,
    /// Some related intent exists but does not cover the action clearly.
    Weak,
    /// No trusted authorization covers the action.
    Absent,
}

impl GovernedAuthorization {
    fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Substantive => "substantive",
            Self::Weak => "weak",
            Self::Absent => "absent",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "explicit" => Ok(Self::Explicit),
            "substantive" => Ok(Self::Substantive),
            "weak" => Ok(Self::Weak),
            "absent" => Ok(Self::Absent),
            other => crate::ids::invalid_enum("governed_authorization", other),
        }
    }
}

/// Closed reviewer risk assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernedRisk {
    /// Narrow, reversible, and low-impact.
    Low,
    /// Bounded impact that remains proportionate.
    Medium,
    /// Broad, sensitive, or difficult to reverse.
    High,
    /// Severe or potentially catastrophic impact.
    Critical,
}

impl GovernedRisk {
    fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            other => crate::ids::invalid_enum("governed_risk", other),
        }
    }
}

/// Canonical reviewer assessment for one governed action revision.
#[derive(Debug, Clone, PartialEq)]
pub struct GovernedActionAssessmentRecord {
    /// Whether the reviewer returned a valid closed-schema result.
    pub status: GovernedAssessmentStatus,
    /// Exact reviewer route selection when review completed.
    pub reviewer_selection: Option<Value>,
    /// How directly authenticated human authority covered the action.
    pub authorization: Option<GovernedAuthorization>,
    /// Consequence if the proposed action is wrong.
    pub risk: Option<GovernedRisk>,
    /// Closed reason codes selected by the reviewer.
    pub reason_codes: Vec<String>,
    /// Bounded natural-language reviewer reasoning.
    pub explanation: String,
}

/// Composed execution recommendation. A reviewer never hard-denies an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GovernedRecommendation {
    /// Permit one automatic execution claim.
    AutoExecute,
    /// Require one human decision.
    RequireApproval,
}

impl GovernedRecommendation {
    fn as_str(self) -> &'static str {
        match self {
            Self::AutoExecute => "auto_execute",
            Self::RequireApproval => "require_approval",
        }
    }
}

/// Assessment written exactly once for an action revision.
#[derive(Debug, Clone)]
pub struct NewGovernedActionAssessment {
    /// Reviewer execution status.
    pub status: GovernedAssessmentStatus,
    /// Exact reviewer route selection when a review completed.
    pub reviewer_selection: Option<Value>,
    /// Closed authorization judgment.
    pub authorization: Option<GovernedAuthorization>,
    /// Closed risk judgment.
    pub risk: Option<GovernedRisk>,
    /// Closed, bounded reason codes.
    pub reason_codes: Vec<String>,
    /// Bounded reviewer explanation.
    pub explanation: String,
}

/// Terminal execution result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernedExecutionOutcome {
    /// Tool reported success.
    Succeeded,
    /// Tool or gateway reported a known failure.
    Failed,
    /// Remote call was sent but its outcome is unknown.
    OutcomeUncertain,
}

impl NoemaStore {
    /// Find a consumed one-time ACP permission with an exact canonical request fingerprint.
    ///
    /// # Errors
    ///
    /// Returns an error when fingerprinting or the atomic consumption write fails.
    pub async fn consume_succeeded_acp_permission(
        &self,
        task_id: &str,
        run_id: &str,
        arguments: &Value,
    ) -> Result<Option<GovernedActionRecord>, StoreError> {
        let arguments_sha256 = sha256_hex(&serde_json::to_vec(arguments)?);
        let task_id = task_id.to_string();
        let run_id = run_id.to_string();
        self.with_immediate_transaction_retry(|transaction| {
            let candidate = transaction
                .query_row(
                    "SELECT action_id, revision FROM governed_actions WHERE task_id = ?1 AND capability_name = 'acp.permission' AND arguments_sha256 = ?2 AND state = 'succeeded' AND NOT EXISTS (SELECT 1 FROM acp_permission_consumptions consumed WHERE consumed.action_id = governed_actions.action_id) ORDER BY completed_at DESC, action_id DESC LIMIT 1",
                    params![task_id, arguments_sha256],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()?;
            let Some((action_id, revision)) = candidate else {
                return Ok(None);
            };
            transaction.execute(
                "INSERT INTO acp_permission_consumptions (action_id, revision, consumed_by_run_id) VALUES (?1, ?2, ?3)",
                params![action_id, revision, run_id],
            )?;
            action_from_tx(transaction, &action_id, u64::try_from(revision).map_err(|_| StoreError::InvariantViolation { message: "ACP permission revision overflow".to_string() })?)
        }).await
    }

    /// Return whether the exact ACP request was explicitly denied once.
    ///
    /// # Errors
    ///
    /// Returns an error when fingerprinting or the durable lookup fails.
    pub async fn has_declined_acp_permission(
        &self,
        task_id: &str,
        arguments: &Value,
    ) -> Result<bool, StoreError> {
        let arguments_sha256 = sha256_hex(&serde_json::to_vec(arguments)?);
        let task_id = task_id.to_string();
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM governed_actions WHERE task_id = ?1 AND capability_name = 'acp.permission' AND arguments_sha256 = ?2 AND state = 'declined')",
                    params![task_id, arguments_sha256],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
    }
    /// Persist one exact action revision before review begins.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the action is invalid or cannot be persisted atomically.
    pub async fn create_governed_action(
        &self,
        action: NewGovernedAction,
    ) -> Result<GovernedActionRecord, StoreError> {
        validate_new_action(&action)?;
        let action_id = allocate_id("action");
        let revision = 1_u64;
        let arguments_json = bounded_json(&action.arguments, MAX_ARGUMENTS_BYTES, "arguments")?;
        let input_schema_json = bounded_json(&action.input_schema, MAX_SCHEMA_BYTES, "schema")?;
        let authorization_context_json = bounded_json(
            &action.authorization_context,
            MAX_AUTHORIZATION_CONTEXT_BYTES,
            "authorization_context",
        )?;
        let arguments_sha256 = sha256_hex(arguments_json.as_bytes());
        self.with_immediate_transaction_retry(|transaction| {
            transaction.execute(
                r#"
                INSERT INTO governed_actions (
                  action_id, revision, owner_human_id, conversation_id, turn_id,
                  task_id, run_id, requesting_agent_id, capability_name, operation_token,
                  review_route, read_only, idempotent, destructive, open_world,
                  arguments_json, arguments_sha256, input_schema_json,
                  authorization_context_json, safe_summary, state
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, 'proposed')
                "#,
                params![
                    action_id,
                    revision,
                    action.owner_human_id,
                    action.conversation_id,
                    action.turn_id,
                    action.task_id,
                    action.run_id,
                    action.requesting_agent_id,
                    action.capability_name,
                    action.operation_token,
                    action.review_route.as_str(),
                    action.behavior.read_only,
                    action.behavior.idempotent,
                    action.behavior.destructive,
                    action.behavior.open_world,
                    arguments_json,
                    arguments_sha256,
                    input_schema_json,
                    authorization_context_json,
                    action.safe_summary,
                ],
            )?;
            insert_event(
                transaction,
                &action_id,
                revision,
                "proposed",
                &action.requesting_agent_id,
                &serde_json::json!({"summary": action.safe_summary}),
            )?;
            action_from_tx(transaction, &action_id, revision)?.ok_or_else(|| {
                StoreError::InvariantViolation {
                    message: "new governed action could not be reloaded".to_string(),
                }
            })
        })
        .await
    }

    /// Record the one reviewer assessment and open approval or execution.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the assessment is invalid, stale, or cannot be persisted.
    pub async fn record_governed_action_assessment(
        &self,
        action_id: &str,
        revision: u64,
        assessment: NewGovernedActionAssessment,
        run_fence: Option<&WorkRunFence>,
    ) -> Result<GovernedActionRecord, StoreError> {
        validate_assessment(&assessment)?;
        let recommendation = compose_recommendation(&assessment);
        let selection_json = assessment
            .reviewer_selection
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let reason_codes_json = serde_json::to_string(&assessment.reason_codes)?;
        self.with_immediate_transaction_retry(|transaction| {
            require_state(transaction, action_id, revision, GovernedActionState::Proposed)?;
            transaction.execute(
                r#"
                INSERT INTO governed_action_assessments (
                  action_id, action_revision, status, reviewer_selection_json,
                  authorization, risk, recommendation, reason_codes_json, explanation
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    action_id,
                    revision,
                    assessment.status.as_str(),
                    selection_json,
                    assessment.authorization.map(GovernedAuthorization::as_str),
                    assessment.risk.map(GovernedRisk::as_str),
                    recommendation.as_str(),
                    reason_codes_json,
                    assessment.explanation,
                ],
            )?;
            let next = match recommendation {
                GovernedRecommendation::AutoExecute => {
                    require_origin_execution_live_tx(
                        transaction,
                        action_id,
                        revision,
                        run_fence,
                    )?;
                    GovernedActionState::Executable
                }
                GovernedRecommendation::RequireApproval => {
                    mark_origin_run_waiting_tx(transaction, action_id, revision, run_fence)?;
                    transaction.execute(
                        "INSERT INTO governed_action_approvals (action_id, action_revision, state) VALUES (?1, ?2, 'pending')",
                        params![action_id, revision],
                    )?;
                    insert_event(
                        transaction,
                        action_id,
                        revision,
                        "approval_requested",
                        "system:action_gateway",
                        &serde_json::json!({}),
                    )?;
                    GovernedActionState::AwaitingApproval
                }
            };
            transaction.execute(
                "UPDATE governed_actions SET state = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE action_id = ?1 AND revision = ?2",
                params![action_id, revision, next.as_str()],
            )?;
            insert_event(
                transaction,
                action_id,
                revision,
                "reviewed",
                "system:action_reviewer",
                &serde_json::json!({"recommendation": recommendation.as_str()}),
            )?;
            action_from_tx(transaction, action_id, revision)?.ok_or_else(|| StoreError::InvariantViolation {
                message: "reviewed governed action could not be reloaded".to_string(),
            })
        })
        .await
    }

    /// Atomically claim a reviewer-cleared or human-approved action for execution.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the action is stale, not executable, or cannot be updated.
    pub async fn claim_governed_action_execution(
        &self,
        action_id: &str,
        revision: u64,
        run_fence: Option<&WorkRunFence>,
    ) -> Result<GovernedActionRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            require_origin_execution_live_tx(transaction, action_id, revision, run_fence)?;
            let approval_state = transaction
                .query_row(
                    "SELECT state FROM governed_action_approvals WHERE action_id = ?1 AND action_revision = ?2",
                    params![action_id, revision],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if let Some(state) = approval_state {
                if state != "approved" {
                    return Err(action_conflict("one-shot approval is not executable"));
                }
                transaction.execute(
                    "UPDATE governed_action_approvals SET state = 'consumed', consumed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE action_id = ?1 AND action_revision = ?2 AND state = 'approved'",
                    params![action_id, revision],
                )?;
            }
            let changed = transaction.execute(
                "UPDATE governed_actions SET state = 'executing', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE action_id = ?1 AND revision = ?2 AND state = 'executable'",
                params![action_id, revision],
            )?;
            if changed != 1 {
                return Err(action_conflict("action is not executable"));
            }
            insert_event(
                transaction,
                action_id,
                revision,
                "execution_started",
                "system:action_gateway",
                &serde_json::json!({}),
            )?;
            action_from_tx(transaction, action_id, revision)?.ok_or_else(|| StoreError::InvariantViolation {
                message: "claimed governed action could not be reloaded".to_string(),
            })
        })
        .await
    }

    /// Commit the terminal result of one claimed execution.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the claim is stale or the result cannot be persisted.
    pub async fn finish_governed_action_execution(
        &self,
        action_id: &str,
        revision: u64,
        outcome: GovernedExecutionOutcome,
        output: Option<&Value>,
        failure_code: Option<&str>,
    ) -> Result<GovernedActionRecord, StoreError> {
        let output_json = output.map(serde_json::to_string).transpose()?;
        let (state, event_kind) = match outcome {
            GovernedExecutionOutcome::Succeeded => (GovernedActionState::Succeeded, "succeeded"),
            GovernedExecutionOutcome::Failed => (GovernedActionState::Failed, "failed"),
            GovernedExecutionOutcome::OutcomeUncertain => {
                (GovernedActionState::OutcomeUncertain, "outcome_uncertain")
            }
        };
        self.with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                r#"
                UPDATE governed_actions
                SET state = ?3, output_json = ?4, failure_code = ?5,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE action_id = ?1 AND revision = ?2 AND state = 'executing'
                "#,
                params![
                    action_id,
                    revision,
                    state.as_str(),
                    output_json,
                    failure_code
                ],
            )?;
            if changed != 1 {
                return Err(action_conflict("action execution was not claimed"));
            }
            insert_event(
                transaction,
                action_id,
                revision,
                event_kind,
                "system:action_gateway",
                &serde_json::json!({"failure_code": failure_code}),
            )?;
            action_from_tx(transaction, action_id, revision)?.ok_or_else(|| {
                StoreError::InvariantViolation {
                    message: "finished governed action could not be reloaded".to_string(),
                }
            })
        })
        .await
    }

    /// Return an authentication-paused approved action to its execution claim.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the action is stale or cannot be persisted.
    pub async fn resume_governed_action_after_authentication(
        &self,
        action_id: &str,
        revision: u64,
    ) -> Result<GovernedActionRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                "UPDATE governed_actions SET authentication_pending = 0, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE action_id = ?1 AND revision = ?2 AND state = 'executing' AND authentication_pending = 1",
                params![action_id, revision],
            )?;
            if changed != 1 {
                return Err(action_conflict("action authentication request is stale"));
            }
            action_from_tx(transaction, action_id, revision)?.ok_or_else(|| {
                StoreError::InvariantViolation {
                    message: "resumed governed action could not be reloaded".to_string(),
                }
            })
        })
        .await
    }

    /// Read one exact action revision.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn get_governed_action(
        &self,
        action_id: &str,
        revision: u64,
    ) -> Result<Option<GovernedActionRecord>, StoreError> {
        self.with_connection(|connection| action_from_tx(connection, action_id, revision))
            .await
    }

    /// Return terminal reviewed actions whose originating conversation or run
    /// has not yet received its durable continuation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the recovery read fails or contains invalid data.
    pub async fn list_interrupted_governed_action_resumptions(
        &self,
    ) -> Result<Vec<GovernedActionRecord>, StoreError> {
        self.with_connection(|connection| {
            let ids = connection
                .prepare(
                    r#"
                    SELECT actions.action_id, actions.revision
                    FROM governed_actions actions
                    WHERE actions.state IN ('succeeded', 'failed', 'outcome_uncertain', 'declined', 'superseded', 'cancelled')
                      AND (
                        (actions.conversation_id IS NOT NULL AND NOT EXISTS (
                          SELECT 1 FROM conversation_items items
                          WHERE items.item_id = 'item:governed_action:' || actions.action_id || ':' || actions.revision
                        ))
                        OR
                        (actions.run_id IS NOT NULL AND EXISTS (
                          SELECT 1 FROM agent_runs runs
                          WHERE runs.run_id = actions.run_id AND runs.status = 'waiting_for_approval'
                        ) AND EXISTS (
                          SELECT 1 FROM governed_action_approvals approvals
                          WHERE approvals.action_id = actions.action_id
                            AND approvals.action_revision = actions.revision
                        ))
                      )
                    ORDER BY actions.created_at, actions.action_id
                    "#,
                )?
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            ids.into_iter()
                .map(|(action_id, revision)| {
                    let revision = u64::try_from(revision)
                        .map_err(|_| action_conflict("invalid revision"))?;
                    action_from_tx(connection, &action_id, revision)?.ok_or_else(|| {
                        action_conflict("reviewed action disappeared during recovery read")
                    })
                })
                .collect()
        })
        .await
    }
}

fn safe_value_projection(value: &Value) -> Value {
    project_safe_value(value, 0)
}

fn project_safe_value(value: &Value, depth: usize) -> Value {
    if depth >= MAX_REVIEW_DEPTH {
        return json!({"type": value_type(value), "truncated": true});
    }
    match value {
        Value::Null => json!({"type": "null"}),
        Value::Bool(_) => json!({"type": "boolean"}),
        Value::Number(_) => json!({"type": "number"}),
        Value::String(text) => json!({"type": "string", "length": text.len()}),
        Value::Array(items) => json!({
            "type": "array",
            "item_count": items.len(),
            "items": items
                .iter()
                .take(MAX_REVIEW_ITEMS)
                .map(|item| project_safe_value(item, depth + 1))
                .collect::<Vec<_>>(),
            "truncated": items.len() > MAX_REVIEW_ITEMS,
        }),
        Value::Object(fields) => {
            let mut projected = Map::new();
            for (name, value) in fields.iter().take(MAX_REVIEW_FIELDS) {
                projected.insert(name.clone(), project_safe_value(value, depth + 1));
            }
            json!({
                "type": "object",
                "field_count": fields.len(),
                "fields": projected,
                "truncated": fields.len() > MAX_REVIEW_FIELDS,
            })
        }
    }
}

const fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn validate_new_action(action: &NewGovernedAction) -> Result<(), StoreError> {
    for (kind, value) in [
        ("owner_human_id", action.owner_human_id.as_str()),
        ("requesting_agent_id", action.requesting_agent_id.as_str()),
        ("capability_name", action.capability_name.as_str()),
        ("operation_token", action.operation_token.as_str()),
        ("safe_summary", action.safe_summary.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(action_conflict(format!("{kind} is empty")));
        }
    }
    if action.task_id.is_some() != action.run_id.is_some() {
        return Err(action_conflict(
            "task and run origins must be supplied together",
        ));
    }
    Ok(())
}

fn validate_assessment(assessment: &NewGovernedActionAssessment) -> Result<(), StoreError> {
    if assessment.explanation.trim().is_empty()
        || assessment.explanation.chars().count() > MAX_EXPLANATION_CHARS
        || assessment.reason_codes.len() > MAX_REASON_CODES
        || assessment
            .reason_codes
            .iter()
            .any(|code| code.trim().is_empty() || code.chars().count() > 80)
    {
        return Err(action_conflict("reviewer assessment is invalid"));
    }
    let completed = assessment.status == GovernedAssessmentStatus::Completed;
    if completed
        != (assessment.reviewer_selection.is_some()
            && assessment.authorization.is_some()
            && assessment.risk.is_some())
    {
        return Err(action_conflict(
            "reviewer assessment fields are inconsistent",
        ));
    }
    Ok(())
}

fn compose_recommendation(assessment: &NewGovernedActionAssessment) -> GovernedRecommendation {
    if assessment.status != GovernedAssessmentStatus::Completed {
        return GovernedRecommendation::RequireApproval;
    }
    match (assessment.authorization, assessment.risk) {
        (
            Some(GovernedAuthorization::Explicit | GovernedAuthorization::Substantive),
            Some(GovernedRisk::Low | GovernedRisk::Medium),
        )
        | (Some(GovernedAuthorization::Weak), Some(GovernedRisk::Low)) => {
            GovernedRecommendation::AutoExecute
        }
        _ => GovernedRecommendation::RequireApproval,
    }
}

fn bounded_json(value: &Value, max_bytes: usize, kind: &str) -> Result<String, StoreError> {
    let encoded = serde_json::to_string(value)?;
    if encoded.len() > max_bytes {
        return Err(action_conflict(format!("{kind} exceeds its storage limit")));
    }
    Ok(encoded)
}

fn require_state(
    transaction: &Transaction<'_>,
    action_id: &str,
    revision: u64,
    expected: GovernedActionState,
) -> Result<(), StoreError> {
    let state = transaction
        .query_row(
            "SELECT state FROM governed_actions WHERE action_id = ?1 AND revision = ?2",
            params![action_id, revision],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match state.as_deref() {
        Some(actual) if actual == expected.as_str() => Ok(()),
        Some(_) => Err(action_conflict("action state changed")),
        None => Err(action_conflict("action revision was not found")),
    }
}

pub(crate) fn insert_event(
    transaction: &Transaction<'_>,
    action_id: &str,
    revision: u64,
    event_kind: &str,
    actor_id: &str,
    safe_payload: &Value,
) -> Result<(), StoreError> {
    transaction.execute(
        r#"
        INSERT INTO governed_action_events (
          event_id, action_id, action_revision, event_kind, actor_id, safe_payload_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        params![
            allocate_id("action_event"),
            action_id,
            revision,
            event_kind,
            actor_id,
            serde_json::to_string(safe_payload)?,
        ],
    )?;
    Ok(())
}

pub(crate) fn action_from_tx(
    connection: &rusqlite::Connection,
    action_id: &str,
    revision: u64,
) -> Result<Option<GovernedActionRecord>, StoreError> {
    let raw = connection
        .query_row(
            r#"
            SELECT action_id, revision, owner_human_id, conversation_id, turn_id,
                   task_id, run_id, requesting_agent_id, capability_name, operation_token,
                   review_route, read_only, idempotent, destructive, open_world,
                   arguments_json, arguments_sha256, input_schema_json,
                   authorization_context_json, safe_summary, state, output_json, failure_code,
                   authentication_pending
            FROM governed_actions
            WHERE action_id = ?1 AND revision = ?2
            "#,
            params![action_id, revision],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, Option<bool>>(11)?,
                    row.get::<_, Option<bool>>(12)?,
                    row.get::<_, Option<bool>>(13)?,
                    row.get::<_, Option<bool>>(14)?,
                    row.get::<_, String>(15)?,
                    row.get::<_, String>(16)?,
                    row.get::<_, String>(17)?,
                    row.get::<_, String>(18)?,
                    row.get::<_, String>(19)?,
                    row.get::<_, String>(20)?,
                    row.get::<_, Option<String>>(21)?,
                    row.get::<_, Option<String>>(22)?,
                    row.get::<_, bool>(23)?,
                ))
            },
        )
        .optional()?;
    raw.map(|raw| {
        let revision = u64::try_from(raw.1).map_err(|_| action_conflict("invalid revision"))?;
        let assessment = assessment_from_tx(connection, &raw.0, revision)?;
        Ok(GovernedActionRecord {
            action_id: raw.0,
            revision,
            owner_human_id: raw.2,
            conversation_id: raw.3,
            turn_id: raw.4,
            task_id: raw.5,
            run_id: raw.6,
            requesting_agent_id: raw.7,
            capability_name: raw.8,
            operation_token: raw.9,
            review_route: ExecutionReviewRoute::parse(&raw.10)?,
            behavior: match (raw.11, raw.12, raw.13, raw.14) {
                (Some(read_only), Some(idempotent), Some(destructive), Some(open_world)) => {
                    Some(StoredToolBehavior {
                        read_only,
                        idempotent,
                        destructive,
                        open_world,
                    })
                }
                (None, None, None, None) => None,
                _ => return Err(action_conflict("incomplete tool behavior snapshot")),
            },
            arguments: serde_json::from_str(&raw.15)?,
            arguments_sha256: raw.16,
            input_schema: serde_json::from_str(&raw.17)?,
            authorization_context: serde_json::from_str(&raw.18)?,
            safe_summary: raw.19,
            state: if raw.20 == "executing" && raw.23 {
                GovernedActionState::AwaitingAuthentication
            } else {
                GovernedActionState::parse(&raw.20)?
            },
            output: raw
                .21
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
            failure_code: raw.22,
            assessment,
        })
    })
    .transpose()
}

fn assessment_from_tx(
    connection: &rusqlite::Connection,
    action_id: &str,
    revision: u64,
) -> Result<Option<GovernedActionAssessmentRecord>, StoreError> {
    let revision = i64::try_from(revision).map_err(|_| action_conflict("invalid revision"))?;
    let raw = connection
        .query_row(
            r#"
            SELECT status, reviewer_selection_json, authorization, risk,
                   reason_codes_json, explanation
            FROM governed_action_assessments
            WHERE action_id = ?1 AND action_revision = ?2
            "#,
            params![action_id, revision],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?;
    raw.map(|raw| {
        Ok(GovernedActionAssessmentRecord {
            status: GovernedAssessmentStatus::parse(&raw.0)?,
            reviewer_selection: raw
                .1
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
            authorization: raw
                .2
                .as_deref()
                .map(GovernedAuthorization::parse)
                .transpose()?,
            risk: raw.3.as_deref().map(GovernedRisk::parse).transpose()?,
            reason_codes: serde_json::from_str(&raw.4)?,
            explanation: raw.5,
        })
    })
    .transpose()
}

fn action_conflict(message: impl Into<String>) -> StoreError {
    StoreError::InvariantViolation {
        message: message.into(),
    }
}
