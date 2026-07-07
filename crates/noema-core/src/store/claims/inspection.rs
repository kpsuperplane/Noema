use std::collections::HashMap;

use crate::memory::Sensitivity;
use crate::store::{NoemaStore, StoreError};

use super::{
    labels::{contains_case_folded, format_datetime, parse_sensitivity},
    model::{
        ClaimStatus, MemoryClaimDetail, MemoryClaimEvidence, MemoryClaimFilter, MemoryClaimRecord,
    },
    rows::{
        InspectionClaimRow, InspectionEntity, InspectionEntityRow, InspectionEvidenceCountRow,
        InspectionEvidenceRow, InspectionPredicateRow,
    },
};

impl NoemaStore {
    /// List graph-memory claims for memory-management inspection.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when stored enum data is invalid or the embedded
    /// store read fails.
    pub async fn list_claims(
        &self,
        filter: MemoryClaimFilter,
    ) -> Result<Vec<MemoryClaimRecord>, StoreError> {
        let predicates = self.inspection_predicates().await?;
        let entities = self.inspection_entities().await?;
        let evidence_counts = self.inspection_evidence_counts().await?;
        let query = filter
            .query
            .as_deref()
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(str::to_ascii_lowercase);
        let limit = clamp_claim_inspection_limit(filter.limit);
        let db_limit = query.is_none().then_some(limit);
        let mut sql = String::from(
            r#"
            SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
              status, sensitivity, confidence, created_at, updated_at
            FROM claims
            "#,
        );
        let mut clauses = Vec::new();
        if filter.status.is_some() {
            clauses.push("status = $status");
        }
        if filter.predicate_id.is_some() {
            clauses.push("predicate_id = $predicate_id");
        }
        if !clauses.is_empty() {
            sql.push_str("WHERE ");
            sql.push_str(&clauses.join(" AND "));
            sql.push('\n');
        }
        sql.push_str("ORDER BY created_at DESC, claim_id ASC\n");
        if db_limit.is_some() {
            sql.push_str("LIMIT $limit\n");
        }
        sql.push(';');

        let mut statement = self.db().query(sql);
        if let Some(status) = filter.status {
            statement = statement.bind(("status", status.as_str().to_string()));
        }
        if let Some(predicate_id) = filter.predicate_id {
            statement = statement.bind(("predicate_id", predicate_id));
        }
        if let Some(db_limit) = db_limit {
            statement = statement.bind(("limit", db_limit));
        }
        let mut response = statement.await?;
        let rows: Vec<InspectionClaimRow> = response.take(0)?;
        let mut claims = Vec::new();

        for row in rows {
            let claim = memory_claim_record(row, &predicates, &entities, &evidence_counts)?;
            if let Some(query) = query.as_deref()
                && !claim.matches_query(query)
            {
                continue;
            }
            claims.push(claim);
            if claims.len() >= limit {
                break;
            }
        }

        Ok(claims)
    }

    /// Return one graph-memory claim with support evidence for memory-management inspection.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when stored enum data is invalid or the embedded
    /// store read fails.
    pub async fn get_claim_detail(
        &self,
        claim_id: &str,
    ) -> Result<Option<MemoryClaimDetail>, StoreError> {
        let predicates = self.inspection_predicates().await?;
        let entities = self.inspection_entities().await?;
        let evidence_counts = self.inspection_evidence_counts().await?;
        let mut response = self
            .db()
            .query(
                r#"
                SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
                  status, sensitivity, confidence, created_at, updated_at
                FROM claims
                WHERE claim_id = $claim_id
                LIMIT 1;

                SELECT relation_id, source_item_id, authority, excerpt, observed_at, created_at
                FROM supported_by
                WHERE claim_id = $claim_id
                ORDER BY created_at ASC, relation_id ASC;
                "#,
            )
            .bind(("claim_id", claim_id.to_string()))
            .await?;
        let rows: Vec<InspectionClaimRow> = response.take(0)?;
        let Some(row) = rows.into_iter().next() else {
            return Ok(None);
        };
        let evidence_rows: Vec<InspectionEvidenceRow> = response.take(1)?;
        let claim = memory_claim_record(row, &predicates, &entities, &evidence_counts)?;
        let evidence = evidence_rows
            .into_iter()
            .map(|row| MemoryClaimEvidence {
                evidence_id: row.relation_id,
                source_item_id: row.source_item_id,
                authority: row.authority,
                excerpt: row.excerpt,
                observed_at: row.observed_at.map(format_datetime),
                created_at: format_datetime(row.created_at),
            })
            .collect();

        Ok(Some(MemoryClaimDetail { claim, evidence }))
    }

    pub(super) async fn inspection_predicates(
        &self,
    ) -> Result<HashMap<String, String>, StoreError> {
        let mut response = self
            .db()
            .query("SELECT predicate_id, label FROM predicates;")
            .await?;
        let rows: Vec<InspectionPredicateRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.predicate_id, row.label))
            .collect())
    }

    pub(super) async fn inspection_entities(
        &self,
    ) -> Result<HashMap<String, InspectionEntity>, StoreError> {
        let mut response = self
            .db()
            .query("SELECT entity_id, entity_type, canonical_name FROM entities;")
            .await?;
        let rows: Vec<InspectionEntityRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.entity_id,
                    InspectionEntity {
                        entity_type: row.entity_type,
                        canonical_name: row.canonical_name,
                    },
                )
            })
            .collect())
    }

    pub(super) async fn inspection_evidence_counts(
        &self,
    ) -> Result<HashMap<String, i64>, StoreError> {
        let mut response = self
            .db()
            .query("SELECT claim_id, count() AS count FROM supported_by GROUP BY claim_id;")
            .await?;
        let rows: Vec<InspectionEvidenceCountRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.claim_id, row.count))
            .collect())
    }

    pub(super) async fn inspection_predicates_for_claims(
        &self,
        rows: &[InspectionClaimRow],
    ) -> Result<HashMap<String, String>, StoreError> {
        let predicate_ids = claim_predicate_ids(rows);
        if predicate_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut response = self
            .db()
            .query(
                "SELECT predicate_id, label FROM predicates WHERE predicate_id IN $predicate_ids;",
            )
            .bind(("predicate_ids", predicate_ids))
            .await?;
        let rows: Vec<InspectionPredicateRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.predicate_id, row.label))
            .collect())
    }

    pub(super) async fn inspection_entities_for_claims(
        &self,
        rows: &[InspectionClaimRow],
    ) -> Result<HashMap<String, InspectionEntity>, StoreError> {
        let entity_ids = claim_entity_ids(rows);
        if entity_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut response = self
            .db()
            .query(
                "SELECT entity_id, entity_type, canonical_name FROM entities WHERE entity_id IN $entity_ids;",
            )
            .bind(("entity_ids", entity_ids))
            .await?;
        let rows: Vec<InspectionEntityRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.entity_id,
                    InspectionEntity {
                        entity_type: row.entity_type,
                        canonical_name: row.canonical_name,
                    },
                )
            })
            .collect())
    }

    pub(super) async fn inspection_evidence_counts_for_claims(
        &self,
        rows: &[InspectionClaimRow],
    ) -> Result<HashMap<String, i64>, StoreError> {
        let claim_ids = inspection_claim_ids(rows);
        if claim_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut response = self
            .db()
            .query(
                "SELECT claim_id, count() AS count FROM supported_by WHERE claim_id IN $claim_ids GROUP BY claim_id;",
            )
            .bind(("claim_ids", claim_ids))
            .await?;
        let rows: Vec<InspectionEvidenceCountRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.claim_id, row.count))
            .collect())
    }
}
pub(super) fn memory_claim_record(
    row: InspectionClaimRow,
    predicates: &HashMap<String, String>,
    entities: &HashMap<String, InspectionEntity>,
    evidence_counts: &HashMap<String, i64>,
) -> Result<MemoryClaimRecord, StoreError> {
    let subject = entities.get(&row.subject_entity_id).ok_or_else(|| {
        StoreError::Schema(format!("claim missing subject entity: {}", row.claim_id))
    })?;
    let object = row
        .object_entity_id
        .as_deref()
        .map(|entity_id| {
            entities.get(entity_id).ok_or_else(|| {
                StoreError::Schema(format!(
                    "claim missing object entity: {} references {}",
                    row.claim_id, entity_id
                ))
            })
        })
        .transpose()?;
    Ok(MemoryClaimRecord {
        claim_id: row.claim_id.clone(),
        fact: row.fact,
        predicate_label: predicates
            .get(&row.predicate_id)
            .cloned()
            .unwrap_or_else(|| row.predicate_id.clone()),
        predicate_id: row.predicate_id,
        subject_entity_id: row.subject_entity_id,
        subject_entity_name: subject.canonical_name.clone(),
        subject_entity_type: subject.entity_type.clone(),
        object_entity_id: row.object_entity_id,
        object_entity_name: object.map(|entity| entity.canonical_name.clone()),
        object_entity_type: object.map(|entity| entity.entity_type.clone()),
        status: ClaimStatus::parse(&row.status)?,
        sensitivity: parse_sensitivity(&row.sensitivity)?,
        confidence: row.confidence,
        evidence_count: evidence_counts.get(&row.claim_id).copied().unwrap_or(0),
        created_at: format_datetime(row.created_at),
        updated_at: format_datetime(row.updated_at),
    })
}

fn claim_predicate_ids(rows: &[InspectionClaimRow]) -> Vec<String> {
    let mut predicate_ids = Vec::new();
    for row in rows {
        if !predicate_ids.contains(&row.predicate_id) {
            predicate_ids.push(row.predicate_id.clone());
        }
    }
    predicate_ids
}

fn claim_entity_ids(rows: &[InspectionClaimRow]) -> Vec<String> {
    let mut entity_ids = Vec::new();
    for row in rows {
        if !entity_ids.contains(&row.subject_entity_id) {
            entity_ids.push(row.subject_entity_id.clone());
        }
        if let Some(object_entity_id) = &row.object_entity_id
            && !entity_ids.contains(object_entity_id)
        {
            entity_ids.push(object_entity_id.clone());
        }
    }
    entity_ids
}

fn inspection_claim_ids(rows: &[InspectionClaimRow]) -> Vec<String> {
    let mut claim_ids = Vec::new();
    for row in rows {
        if !claim_ids.contains(&row.claim_id) {
            claim_ids.push(row.claim_id.clone());
        }
    }
    claim_ids
}

impl MemoryClaimRecord {
    pub(super) fn matches_query(&self, query: &str) -> bool {
        contains_case_folded(&self.fact, query)
            || contains_case_folded(&self.predicate_id, query)
            || contains_case_folded(&self.predicate_label, query)
            || contains_case_folded(&self.subject_entity_id, query)
            || contains_case_folded(&self.subject_entity_name, query)
            || self
                .object_entity_id
                .as_deref()
                .is_some_and(|value| contains_case_folded(value, query))
            || self
                .object_entity_name
                .as_deref()
                .is_some_and(|value| contains_case_folded(value, query))
    }

    pub(super) fn matches_graph_query(&self, query: &str) -> bool {
        if contains_case_folded(&self.predicate_id, query)
            || contains_case_folded(&self.predicate_label, query)
        {
            return true;
        }

        if self.sensitivity != Sensitivity::Public {
            return false;
        }

        contains_case_folded(&self.fact, query)
            || contains_case_folded(&self.subject_entity_name, query)
            || self
                .object_entity_name
                .as_deref()
                .is_some_and(|value| contains_case_folded(value, query))
    }
}
fn clamp_claim_inspection_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(50).clamp(1, 100)
}
