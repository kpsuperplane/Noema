//! Durable authority for reviewed external write and export actions.

use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

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

/// Calibrated external effect governed by this authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernedActionEffect {
    /// External state change.
    Write,
    /// External data egress.
    Export,
    /// External state change and data egress.
    WriteAndExport,
}

impl GovernedActionEffect {
    fn as_str(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Export => "export",
            Self::WriteAndExport => "write_export",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "write" => Ok(Self::Write),
            "export" => Ok(Self::Export),
            "write_export" => Ok(Self::WriteAndExport),
            other => crate::ids::invalid_enum("governed_action_effect", other),
        }
    }
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
    /// Calibrated external effect.
    pub effect: GovernedActionEffect,
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
    /// Calibrated external effect.
    pub effect: GovernedActionEffect,
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
                  effect, arguments_json, arguments_sha256, input_schema_json,
                  authorization_context_json, safe_summary, state
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 'proposed')
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
                    action.effect.as_str(),
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
                   effect, arguments_json, arguments_sha256, input_schema_json,
                   authorization_context_json, safe_summary, state, output_json, failure_code
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
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, String>(14)?,
                    row.get::<_, String>(15)?,
                    row.get::<_, String>(16)?,
                    row.get::<_, Option<String>>(17)?,
                    row.get::<_, Option<String>>(18)?,
                ))
            },
        )
        .optional()?;
    raw.map(|raw| {
        Ok(GovernedActionRecord {
            action_id: raw.0,
            revision: u64::try_from(raw.1).map_err(|_| action_conflict("invalid revision"))?,
            owner_human_id: raw.2,
            conversation_id: raw.3,
            turn_id: raw.4,
            task_id: raw.5,
            run_id: raw.6,
            requesting_agent_id: raw.7,
            capability_name: raw.8,
            operation_token: raw.9,
            effect: GovernedActionEffect::parse(&raw.10)?,
            arguments: serde_json::from_str(&raw.11)?,
            arguments_sha256: raw.12,
            input_schema: serde_json::from_str(&raw.13)?,
            authorization_context: serde_json::from_str(&raw.14)?,
            safe_summary: raw.15,
            state: GovernedActionState::parse(&raw.16)?,
            output: raw
                .17
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
            failure_code: raw.18,
        })
    })
    .transpose()
}

fn action_conflict(message: impl Into<String>) -> StoreError {
    StoreError::InvariantViolation {
        message: message.into(),
    }
}
