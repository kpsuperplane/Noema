use crate::store::{NoemaStore, StoreError};

use super::{
    labels::{
        allowed_match_sensitivities, compatible_match_predicates, exact_match_only_sensitivity,
        parse_sensitivity,
    },
    model::{ClaimStatus, ConsolidationMatch, ConsolidationMatchRequest},
    rows::ConsolidationMatchRow,
};

impl NoemaStore {
    /// Find bounded same-subject candidate matches for memory consolidation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when stored enum data is invalid or the embedded
    /// store read fails.
    pub async fn find_consolidation_matches(
        &self,
        request: ConsolidationMatchRequest,
    ) -> Result<Vec<ConsolidationMatch>, StoreError> {
        let limit = request.limit.clamp(1, 20);
        let query_terms = request
            .query_terms
            .iter()
            .map(|term| term.trim().to_ascii_lowercase())
            .filter(|term| !term.is_empty())
            .collect::<Vec<_>>();

        if exact_match_only_sensitivity(request.sensitivity) {
            let Some(object_entity_id) = request.object_entity_id.as_deref() else {
                return Ok(Vec::new());
            };
            let mut response = self
                .db
                .query(
                    r#"
                    SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
                      status, sensitivity, confidence, updated_at
                    FROM claims
                    WHERE subject_entity_id = $subject_entity_id
                      AND object_entity_id = $object_entity_id
                      AND predicate_id IN $predicate_ids
                      AND status IN ['candidate', 'active', 'confirmed']
                      AND sensitivity IN $allowed_sensitivities
                    ORDER BY updated_at DESC, claim_id ASC
                    LIMIT $limit;
                    "#,
                )
                .bind(("subject_entity_id", request.subject_entity_id))
                .bind(("object_entity_id", object_entity_id.to_string()))
                .bind((
                    "predicate_ids",
                    compatible_match_predicates(&request.predicate_id),
                ))
                .bind((
                    "allowed_sensitivities",
                    allowed_match_sensitivities(request.sensitivity),
                ))
                .bind(("limit", limit))
                .await?;
            let rows: Vec<ConsolidationMatchRow> = response.take(0)?;
            return rows.into_iter().map(consolidation_match).collect();
        }

        if let Some(object_entity_id) = request.object_entity_id.as_deref()
            && query_terms.is_empty()
        {
            let mut response = self
                .db
                .query(
                    r#"
                    SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
                      status, sensitivity, confidence, updated_at
                    FROM claims
                    WHERE subject_entity_id = $subject_entity_id
                      AND object_entity_id = $object_entity_id
                      AND predicate_id IN $predicate_ids
                      AND status IN ['candidate', 'active', 'confirmed']
                      AND sensitivity IN $allowed_sensitivities
                    ORDER BY updated_at DESC, claim_id ASC
                    LIMIT $limit;
                    "#,
                )
                .bind(("subject_entity_id", request.subject_entity_id))
                .bind(("object_entity_id", object_entity_id.to_string()))
                .bind((
                    "predicate_ids",
                    compatible_match_predicates(&request.predicate_id),
                ))
                .bind((
                    "allowed_sensitivities",
                    allowed_match_sensitivities(request.sensitivity),
                ))
                .bind(("limit", limit))
                .await?;
            let rows: Vec<ConsolidationMatchRow> = response.take(0)?;
            return rows.into_iter().map(consolidation_match).collect();
        }

        if let Some(object_entity_id) = request.object_entity_id.as_deref() {
            let object_entity_id = object_entity_id.to_string();
            let mut response = self
                .db
                .query(
                    r#"
                    SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
                      status, sensitivity, confidence, updated_at
                    FROM claims
                    WHERE subject_entity_id = $subject_entity_id
                      AND object_entity_id = $object_entity_id
                      AND predicate_id IN $predicate_ids
                      AND status IN ['candidate', 'active', 'confirmed']
                      AND sensitivity IN $allowed_sensitivities
                    ORDER BY updated_at DESC, claim_id ASC
                    LIMIT $limit;
                    "#,
                )
                .bind(("subject_entity_id", request.subject_entity_id.clone()))
                .bind(("object_entity_id", object_entity_id.clone()))
                .bind((
                    "predicate_ids",
                    compatible_match_predicates(&request.predicate_id),
                ))
                .bind((
                    "allowed_sensitivities",
                    allowed_match_sensitivities(request.sensitivity),
                ))
                .bind(("limit", limit))
                .await?;
            let exact_rows: Vec<ConsolidationMatchRow> = response.take(0)?;
            let mut matches = exact_rows
                .into_iter()
                .map(consolidation_match)
                .collect::<Result<Vec<_>, _>>()?;
            if matches.len() >= limit {
                return Ok(matches);
            }

            let mut response = self
                .db
                .query(
                    r#"
                    SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
                      status, sensitivity, confidence, updated_at
                    FROM claims
                    WHERE subject_entity_id = $subject_entity_id
                      AND predicate_id IN $predicate_ids
                      AND status IN ['candidate', 'active', 'confirmed']
                      AND sensitivity IN $allowed_sensitivities
                    ORDER BY updated_at DESC, claim_id ASC
                    LIMIT $candidate_limit;
                    "#,
                )
                .bind(("subject_entity_id", request.subject_entity_id))
                .bind((
                    "predicate_ids",
                    compatible_match_predicates(&request.predicate_id),
                ))
                .bind((
                    "allowed_sensitivities",
                    allowed_match_sensitivities(request.sensitivity),
                ))
                .bind(("candidate_limit", limit))
                .await?;
            let rows: Vec<ConsolidationMatchRow> = response.take(0)?;
            for row in rows {
                if !consolidation_match_is_relevant(
                    &row,
                    Some(object_entity_id.as_str()),
                    &query_terms,
                ) {
                    continue;
                }
                let item = consolidation_match(row)?;
                if matches
                    .iter()
                    .any(|existing| existing.claim_id == item.claim_id)
                {
                    continue;
                }
                matches.push(item);
                if matches.len() >= limit {
                    break;
                }
            }
            return Ok(matches);
        }

        if request.object_entity_id.is_none() && query_terms.is_empty() {
            let mut response = self
                .db
                .query(
                    r#"
                    SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
                      status, sensitivity, confidence, updated_at
                    FROM claims
                    WHERE subject_entity_id = $subject_entity_id
                      AND predicate_id IN $predicate_ids
                      AND status IN ['candidate', 'active', 'confirmed']
                      AND sensitivity IN $allowed_sensitivities
                    ORDER BY updated_at DESC, claim_id ASC
                    LIMIT $limit;
                    "#,
                )
                .bind(("subject_entity_id", request.subject_entity_id))
                .bind((
                    "predicate_ids",
                    compatible_match_predicates(&request.predicate_id),
                ))
                .bind((
                    "allowed_sensitivities",
                    allowed_match_sensitivities(request.sensitivity),
                ))
                .bind(("limit", limit))
                .await?;
            let rows: Vec<ConsolidationMatchRow> = response.take(0)?;
            return rows.into_iter().map(consolidation_match).collect();
        }

        let mut response = self
            .db
            .query(
                r#"
                SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
                  status, sensitivity, confidence, updated_at
                FROM claims
                WHERE subject_entity_id = $subject_entity_id
                  AND predicate_id IN $predicate_ids
                  AND status IN ['candidate', 'active', 'confirmed']
                  AND sensitivity IN $allowed_sensitivities
                ORDER BY updated_at DESC, claim_id ASC
                LIMIT $candidate_limit;
                "#,
            )
            .bind(("subject_entity_id", request.subject_entity_id))
            .bind((
                "predicate_ids",
                compatible_match_predicates(&request.predicate_id),
            ))
            .bind((
                "allowed_sensitivities",
                allowed_match_sensitivities(request.sensitivity),
            ))
            .bind(("candidate_limit", limit))
            .await?;
        let rows: Vec<ConsolidationMatchRow> = response.take(0)?;
        rows.into_iter()
            .filter(|row| {
                consolidation_match_is_relevant(
                    row,
                    request.object_entity_id.as_deref(),
                    &query_terms,
                )
            })
            .map(consolidation_match)
            .take(limit)
            .collect()
    }
}
fn consolidation_match(row: ConsolidationMatchRow) -> Result<ConsolidationMatch, StoreError> {
    Ok(ConsolidationMatch {
        claim_id: row.claim_id,
        subject_entity_id: row.subject_entity_id,
        object_entity_id: row.object_entity_id,
        predicate_id: row.predicate_id,
        fact: row.fact,
        status: ClaimStatus::parse(&row.status)?,
        sensitivity: parse_sensitivity(&row.sensitivity)?,
        confidence: row.confidence,
    })
}

fn consolidation_match_is_relevant(
    row: &ConsolidationMatchRow,
    object_entity_id: Option<&str>,
    query_terms: &[String],
) -> bool {
    if let Some(object_entity_id) = object_entity_id
        && row.object_entity_id.as_deref() == Some(object_entity_id)
    {
        return true;
    }

    !query_terms.is_empty()
        && query_terms
            .iter()
            .any(|term| row.fact.to_ascii_lowercase().contains(term))
}
