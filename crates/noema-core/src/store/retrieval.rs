use serde::Deserialize;
use serde_json::Value;

use crate::memory::{
    ClaimRetrievalRequest, ClaimStatusForPolicy, PolicyClaim, Sensitivity, UseMode,
    claim_policy_allows,
};

use super::{NoemaStore, StoreError};

/// Claim selected for graph-memory retrieval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievedClaim {
    /// Stable claim id.
    pub claim_id: String,
    /// Human-readable fact text.
    pub fact: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Deterministic rank score.
    pub rank_score: i64,
}

/// Result of deterministic graph-claim retrieval.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClaimRetrievalResult {
    /// Claims included for the requester.
    pub included: Vec<RetrievedClaim>,
    /// Matching candidates omitted due to policy, redacted to a count.
    pub redacted_omission_count: usize,
}

impl NoemaStore {
    /// Retrieve graph claims using deterministic text matching and policy gates.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when stored enum values are invalid, referenced
    /// predicates are missing, claim rows are malformed, or the embedded store
    /// query fails.
    pub async fn retrieve_claims(
        &self,
        request: &ClaimRetrievalRequest,
        query_text: &str,
        limit: usize,
    ) -> Result<ClaimRetrievalResult, StoreError> {
        if limit == 0 {
            return Ok(ClaimRetrievalResult::default());
        }

        let mut response = self
            .db
            .query(
                r#"
                SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact, status, sensitivity, retrieval_hints
                FROM claims
                WHERE status IN ['active', 'confirmed']
                ORDER BY claim_id ASC;
                "#,
            )
            .await?;
        let rows: Vec<ClaimRetrievalRow> = response.take(0)?;
        let query = query_text.trim().to_lowercase();
        let mut result = ClaimRetrievalResult::default();
        let mut included = Vec::new();

        for row in rows {
            let Some(match_score) = deterministic_match_score(&row, &query) else {
                continue;
            };
            let predicate = self.predicate_policy(&row.predicate_id).await?;
            let object_entity_id = row.object_entity_id.clone().ok_or_else(|| {
                StoreError::Schema(format!("claim missing object_entity_id: {}", row.claim_id))
            })?;
            let policy_claim = PolicyClaim {
                claim_id: row.claim_id.clone(),
                subject_entity_id: row.subject_entity_id.clone(),
                object_entity_id,
                predicate_allowed_use_modes: predicate
                    .allowed_use_modes
                    .iter()
                    .map(|mode| parse_use_mode(mode))
                    .collect::<Result<Vec<_>, _>>()?,
                status: parse_claim_status(&row.status)?,
                sensitivity: parse_sensitivity(&row.sensitivity)?,
            };

            if claim_policy_allows(&policy_claim, request).is_ok() {
                included.push(RetrievedClaim {
                    claim_id: row.claim_id,
                    fact: row.fact,
                    predicate_id: row.predicate_id,
                    rank_score: match_score,
                });
            } else {
                result.redacted_omission_count += 1;
            }
        }

        included.sort_by(|left, right| {
            right
                .rank_score
                .cmp(&left.rank_score)
                .then_with(|| left.claim_id.cmp(&right.claim_id))
        });
        included.truncate(limit);
        result.included = included;

        Ok(result)
    }

    async fn predicate_policy(&self, predicate_id: &str) -> Result<PredicatePolicyRow, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT predicate_id, allowed_use_modes
                FROM predicates
                WHERE predicate_id = $predicate_id
                LIMIT 1;
                "#,
            )
            .bind(("predicate_id", predicate_id.to_string()))
            .await?;
        let rows: Vec<PredicatePolicyRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .ok_or_else(|| StoreError::PredicateNotFound {
                predicate_id: predicate_id.to_string(),
            })
    }
}

#[derive(Debug, Deserialize)]
struct ClaimRetrievalRow {
    claim_id: String,
    subject_entity_id: String,
    object_entity_id: Option<String>,
    predicate_id: String,
    fact: String,
    status: String,
    sensitivity: String,
    retrieval_hints: Value,
}

#[derive(Debug, Deserialize)]
struct PredicatePolicyRow {
    #[allow(dead_code)]
    predicate_id: String,
    allowed_use_modes: Vec<String>,
}

fn deterministic_match_score(row: &ClaimRetrievalRow, query: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(50);
    }

    if row.fact.to_lowercase().contains(query) {
        return Some(100);
    }

    row.retrieval_hints
        .to_string()
        .to_lowercase()
        .contains(query)
        .then_some(50)
}

fn parse_use_mode(value: &str) -> Result<UseMode, StoreError> {
    match value {
        "answer" => Ok(UseMode::Answer),
        "personalize" => Ok(UseMode::Personalize),
        "plan" => Ok(UseMode::Plan),
        "act" => Ok(UseMode::Act),
        "notify" => Ok(UseMode::Notify),
        "inspect" => Ok(UseMode::Inspect),
        "export" => Ok(UseMode::Export),
        _ => Err(StoreError::InvalidEnum {
            kind: "use mode",
            value: value.to_string(),
        }),
    }
}

fn parse_claim_status(value: &str) -> Result<ClaimStatusForPolicy, StoreError> {
    match value {
        "candidate" => Ok(ClaimStatusForPolicy::Candidate),
        "active" => Ok(ClaimStatusForPolicy::Active),
        "confirmed" => Ok(ClaimStatusForPolicy::Confirmed),
        "disputed" => Ok(ClaimStatusForPolicy::Disputed),
        "superseded" => Ok(ClaimStatusForPolicy::Superseded),
        "archived" => Ok(ClaimStatusForPolicy::Archived),
        "deleted" => Ok(ClaimStatusForPolicy::Deleted),
        _ => Err(StoreError::InvalidEnum {
            kind: "claim status",
            value: value.to_string(),
        }),
    }
}

fn parse_sensitivity(value: &str) -> Result<Sensitivity, StoreError> {
    match value {
        "public" => Ok(Sensitivity::Public),
        "normal" => Ok(Sensitivity::Normal),
        "private" => Ok(Sensitivity::Private),
        "sensitive" => Ok(Sensitivity::Sensitive),
        "secret" => Ok(Sensitivity::Secret),
        _ => Err(StoreError::InvalidEnum {
            kind: "sensitivity",
            value: value.to_string(),
        }),
    }
}
