use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;
use surrealdb::types::SurrealValue;

use crate::memory::{
    ClaimRetrievalRequest, ClaimStatusForPolicy, PolicyClaim, Sensitivity, UseMode,
    claim_policy_allows,
};

use super::{NoemaStore, StoreError};

const RAW_CONTENT_MATCH_SCORE: i64 = 100;
const CONTENT_TERM_MATCH_BASE_SCORE: i64 = 70;
const HINT_MATCH_BASE_SCORE: i64 = 50;
const MAX_CONTENT_TERM_BONUS_BELOW_RAW: usize =
    (RAW_CONTENT_MATCH_SCORE - CONTENT_TERM_MATCH_BASE_SCORE - 1) as usize;
const MAX_HINT_TERM_BONUS_BELOW_CONTENT: usize =
    (CONTENT_TERM_MATCH_BASE_SCORE - HINT_MATCH_BASE_SCORE - 1) as usize;

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
        self.retrieve_claims_scoped(request, query_text, &[], limit)
            .await
    }

    /// Retrieve graph claims connected to the requested scope ids, then apply
    /// deterministic text matching and policy gates.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when stored enum values are invalid, referenced
    /// predicates are missing, claim rows are malformed, or the embedded store
    /// query fails.
    pub async fn retrieve_claims_scoped(
        &self,
        request: &ClaimRetrievalRequest,
        query_text: &str,
        scope_ids: &[String],
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
        let entity_names = self.retrieval_entity_names(&rows).await?;
        let query = QueryTerms::from_query(query_text);
        let scope_set = scope_ids.iter().cloned().collect::<HashSet<_>>();
        let mut result = ClaimRetrievalResult::default();
        let mut included = Vec::new();

        for row in rows {
            if !scope_matches_claim(&row, &scope_set) {
                continue;
            }

            let predicate = self.predicate_policy(&row.predicate_id).await?;
            let Some(match_score) =
                deterministic_match_score(&row, &predicate, &entity_names, &query)
            else {
                continue;
            };
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

    async fn retrieval_entity_names(
        &self,
        rows: &[ClaimRetrievalRow],
    ) -> Result<HashMap<String, String>, StoreError> {
        let mut entity_ids = rows
            .iter()
            .flat_map(|row| {
                row.object_entity_id
                    .iter()
                    .chain(std::iter::once(&row.subject_entity_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        entity_ids.sort();
        entity_ids.dedup();
        if entity_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut response = self
            .db
            .query("SELECT entity_id, canonical_name FROM entities WHERE entity_id IN $entity_ids;")
            .bind(("entity_ids", entity_ids))
            .await?;
        let rows: Vec<RetrievalEntityNameRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.entity_id, row.canonical_name))
            .collect())
    }

    async fn predicate_policy(&self, predicate_id: &str) -> Result<PredicatePolicyRow, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT predicate_id, label, allowed_use_modes
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

#[derive(Debug, Deserialize, SurrealValue)]
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

#[derive(Debug, Deserialize, SurrealValue)]
struct PredicatePolicyRow {
    #[allow(dead_code)]
    predicate_id: String,
    label: String,
    allowed_use_modes: Vec<String>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct RetrievalEntityNameRow {
    entity_id: String,
    canonical_name: String,
}

struct QueryTerms {
    raw: String,
    raw_tokens: Vec<String>,
    terms: Vec<String>,
}

impl QueryTerms {
    fn from_query(query: &str) -> Self {
        let raw = query.trim().to_lowercase();
        let raw_tokens = raw
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .map(str::trim)
            .filter(|term| !term.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let terms = raw_tokens
            .iter()
            .filter(|term| term.len() > 1)
            .cloned()
            .collect();
        Self {
            raw,
            raw_tokens,
            terms,
        }
    }

    fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
}

fn scope_matches_claim(row: &ClaimRetrievalRow, scope_ids: &HashSet<String>) -> bool {
    if scope_ids.is_empty() {
        return true;
    }
    scope_ids.contains(&row.subject_entity_id)
        || row
            .object_entity_id
            .as_ref()
            .is_some_and(|object_entity_id| scope_ids.contains(object_entity_id))
}

fn deterministic_match_score(
    row: &ClaimRetrievalRow,
    predicate: &PredicatePolicyRow,
    entity_names: &HashMap<String, String>,
    query: &QueryTerms,
) -> Option<i64> {
    if query.is_empty() {
        return Some(50);
    }

    let mut haystack = vec![
        row.fact.as_str(),
        row.predicate_id.as_str(),
        predicate.label.as_str(),
    ];
    if let Some(subject_name) = entity_names.get(&row.subject_entity_id) {
        haystack.push(subject_name.as_str());
    }
    if let Some(object_entity_id) = row.object_entity_id.as_ref()
        && let Some(object_name) = entity_names.get(object_entity_id)
    {
        haystack.push(object_name.as_str());
    }
    let joined = haystack.join(" ").to_lowercase();
    let content_tokens = tokenize_match_text(&joined);

    if text_matches_raw_query(&joined, &content_tokens, query) {
        return Some(RAW_CONTENT_MATCH_SCORE);
    }

    let content_terms = query
        .terms
        .iter()
        .filter(|term| content_tokens.contains(&term.as_str()))
        .count();
    if content_terms > 0 {
        return Some(
            CONTENT_TERM_MATCH_BASE_SCORE
                + i64::try_from(content_terms.min(MAX_CONTENT_TERM_BONUS_BELOW_RAW)).unwrap_or(0),
        );
    }

    let hints = row.retrieval_hints.to_string().to_lowercase();
    let hint_tokens = tokenize_match_text(&hints);
    if text_matches_raw_query(&hints, &hint_tokens, query) {
        return Some(HINT_MATCH_BASE_SCORE);
    }

    let hint_terms = query
        .terms
        .iter()
        .filter(|term| hint_tokens.contains(&term.as_str()))
        .count();
    (hint_terms > 0).then_some(
        HINT_MATCH_BASE_SCORE
            + i64::try_from(hint_terms.min(MAX_HINT_TERM_BONUS_BELOW_CONTENT)).unwrap_or(0),
    )
}

fn tokenize_match_text(text: &str) -> Vec<&str> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect()
}

fn text_matches_raw_query(text: &str, tokens: &[&str], query: &QueryTerms) -> bool {
    if query.raw_tokens.len() == 1 {
        return tokens.contains(&query.raw_tokens[0].as_str());
    }

    text.contains(&query.raw)
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
