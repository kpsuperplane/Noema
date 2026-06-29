use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;
use surrealdb::sql::Datetime;

use crate::memory::Sensitivity;

use super::{
    NoemaStore, StoreError,
    ids::{allocate_id, record_fragment},
    ontology::EntityCandidate,
};

/// Claim lifecycle status stored in the embedded graph store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClaimStatus {
    /// Proposed claim not normally retrieved.
    Candidate,
    /// Current active claim.
    Active,
    /// Human or system-confirmed claim.
    Confirmed,
    /// Claim has unresolved contradictory evidence.
    Disputed,
    /// Claim was replaced by a newer claim.
    Superseded,
    /// Claim is retained but not active.
    Archived,
    /// Claim is deleted.
    Deleted,
}

/// Result category for a graph claim write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClaimWriteOutcome {
    /// A new claim row was inserted.
    Created,
    /// An existing claim row was reinforced with new evidence.
    Reinforced,
}

impl ClaimStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Active => "active",
            Self::Confirmed => "confirmed",
            Self::Disputed => "disputed",
            Self::Superseded => "superseded",
            Self::Archived => "archived",
            Self::Deleted => "deleted",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "candidate" => Ok(Self::Candidate),
            "active" => Ok(Self::Active),
            "confirmed" => Ok(Self::Confirmed),
            "disputed" => Ok(Self::Disputed),
            "superseded" => Ok(Self::Superseded),
            "archived" => Ok(Self::Archived),
            "deleted" => Ok(Self::Deleted),
            _ => Err(StoreError::InvalidEnum {
                kind: "claim status",
                value: value.to_string(),
            }),
        }
    }
}

/// Evidence authority vocabulary for claim support rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceAuthority {
    /// Human explicitly corrected a prior claim.
    HumanCorrection,
    /// Human explicitly stated the claim.
    ExplicitHumanStatement,
    /// Claim came from a document source.
    DocumentSource,
    /// Claim was observed repeatedly.
    RepeatedObservation,
    /// Claim was inferred by an agent.
    AgentInference,
    /// Claim was weakly inferred by an agent.
    WeakInference,
    /// Claim follows from a system rule.
    SystemRule,
}

impl EvidenceAuthority {
    const fn as_str(self) -> &'static str {
        match self {
            Self::HumanCorrection => "human_correction",
            Self::ExplicitHumanStatement => "explicit_human_statement",
            Self::DocumentSource => "document_source",
            Self::RepeatedObservation => "repeated_observation",
            Self::AgentInference => "agent_inference",
            Self::WeakInference => "weak_inference",
            Self::SystemRule => "system_rule",
        }
    }
}

/// Candidate evidence row supporting a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceCandidate {
    /// Source conversation item id.
    pub source_item_id: String,
    /// Authority class for this evidence.
    pub authority: EvidenceAuthority,
    /// Optional short source excerpt.
    pub excerpt: Option<String>,
}

/// Candidate claim plus one supporting evidence row.
#[derive(Debug, Clone, PartialEq)]
pub struct NewClaimCandidate {
    /// Subject entity to upsert.
    pub subject: EntityCandidate,
    /// Object entity to upsert.
    pub object: EntityCandidate,
    /// Existing predicate id.
    pub predicate_id: String,
    /// Human-readable fact text.
    pub fact: String,
    /// Claim sensitivity.
    pub sensitivity: Sensitivity,
    /// Claim lifecycle status.
    pub status: ClaimStatus,
    /// Optional confidence in `[0, 1]`.
    pub confidence: Option<f64>,
    /// Supporting source evidence.
    pub evidence: EvidenceCandidate,
    /// Retrieval hints stored with the claim.
    pub retrieval_hints: Value,
    /// Caller metadata stored with the claim.
    pub metadata: Value,
}

/// Typed summary returned after creating or reinforcing a claim.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimSummary {
    /// Stable claim id.
    pub claim_id: String,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Object entity id.
    pub object_entity_id: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Fact text.
    pub fact: String,
    /// Claim status.
    pub status: ClaimStatus,
    /// Claim sensitivity.
    pub sensitivity: Sensitivity,
    /// Count of support evidence rows.
    pub evidence_count: i64,
    /// Whether this write inserted a claim or reinforced an existing one.
    pub write_outcome: ClaimWriteOutcome,
}

/// Read-only filters for owner/admin graph-claim inspection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryClaimFilter {
    /// Optional text query matched against facts, predicate labels, and entity names.
    pub query: Option<String>,
    /// Optional claim lifecycle status.
    pub status: Option<ClaimStatus>,
    /// Optional predicate id.
    pub predicate_id: Option<String>,
    /// Optional bounded result limit.
    pub limit: Option<usize>,
}

/// Read-only graph-claim projection for owner/admin inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryClaimRecord {
    /// Stable claim id.
    pub claim_id: String,
    /// Human-readable fact text.
    pub fact: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Subject entity display name.
    pub subject_entity_name: String,
    /// Subject entity type.
    pub subject_entity_type: String,
    /// Object entity id when present.
    pub object_entity_id: Option<String>,
    /// Object entity display name when present.
    pub object_entity_name: Option<String>,
    /// Object entity type when present.
    pub object_entity_type: Option<String>,
    /// Claim status.
    pub status: ClaimStatus,
    /// Claim sensitivity.
    pub sensitivity: Sensitivity,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of supporting evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Read-only graph-claim detail projection.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryClaimDetail {
    /// Claim projection.
    pub claim: MemoryClaimRecord,
    /// Supporting evidence rows.
    pub evidence: Vec<MemoryClaimEvidence>,
}

/// Read-only evidence projection for owner/admin graph-claim inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryClaimEvidence {
    /// Stable evidence relation id if available.
    pub evidence_id: Option<String>,
    /// Source conversation item id if this evidence came from a transcript item.
    pub source_item_id: Option<String>,
    /// Evidence authority string.
    pub authority: String,
    /// Optional excerpt.
    pub excerpt: Option<String>,
    /// Observation timestamp if available.
    pub observed_at: Option<String>,
    /// Evidence creation timestamp.
    pub created_at: String,
}

impl NoemaStore {
    /// List graph-memory claims for owner/admin inspection.
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

        let mut statement = self.db.query(sql);
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

    /// Return one graph-memory claim with support evidence for owner/admin inspection.
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
            .db
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

    async fn inspection_predicates(&self) -> Result<HashMap<String, String>, StoreError> {
        let mut response = self
            .db
            .query("SELECT predicate_id, label FROM predicates;")
            .await?;
        let rows: Vec<InspectionPredicateRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.predicate_id, row.label))
            .collect())
    }

    async fn inspection_entities(&self) -> Result<HashMap<String, InspectionEntity>, StoreError> {
        let mut response = self
            .db
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

    async fn inspection_evidence_counts(&self) -> Result<HashMap<String, i64>, StoreError> {
        let mut response = self
            .db
            .query("SELECT claim_id, count() AS count FROM supported_by GROUP BY claim_id;")
            .await?;
        let rows: Vec<InspectionEvidenceCountRow> = response.take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.claim_id, row.count))
            .collect())
    }

    /// Create a graph-memory claim or reinforce an existing non-deleted claim.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the predicate or source item is missing,
    /// stored enum data is invalid, or the embedded store read/write fails.
    pub async fn create_or_reinforce_claim(
        &self,
        candidate: NewClaimCandidate,
    ) -> Result<ClaimSummary, StoreError> {
        let _claim_write_guard = self.claim_write_lock.lock().await;
        self.require_predicate(&candidate.predicate_id).await?;
        self.require_source_item(&candidate.evidence.source_item_id)
            .await?;
        self.upsert_entity(&candidate.subject).await?;
        self.upsert_entity(&candidate.object).await?;

        let fingerprint = claim_fingerprint(
            &candidate.subject.entity_id,
            &candidate.predicate_id,
            &candidate.object.entity_id,
            &candidate.fact,
        );
        let (claim_id, write_outcome) = match self.existing_claim_id(&fingerprint).await? {
            Some(claim_id) => {
                self.merge_reinforced_claim(&claim_id, &candidate).await?;
                (claim_id, ClaimWriteOutcome::Reinforced)
            }
            None => {
                let claim_id = allocate_id("claim");
                self.release_deleted_claim_fingerprint(&fingerprint).await?;
                self.insert_claim(&claim_id, &fingerprint, &candidate)
                    .await?;
                (claim_id, ClaimWriteOutcome::Created)
            }
        };
        self.insert_support_evidence(&claim_id, &candidate.evidence)
            .await?;
        self.claim_summary(&claim_id, write_outcome).await
    }

    async fn merge_reinforced_claim(
        &self,
        claim_id: &str,
        candidate: &NewClaimCandidate,
    ) -> Result<(), StoreError> {
        let existing = self.existing_claim_for_merge(claim_id).await?;
        let status = strongest_claim_status(existing.status, candidate.status);
        let sensitivity = existing.sensitivity.max(candidate.sensitivity);
        let confidence = strongest_confidence(existing.confidence, candidate.confidence);

        if status == existing.status
            && sensitivity == existing.sensitivity
            && confidence == existing.confidence
        {
            return Ok(());
        }

        self.db
            .query(
                r#"
                UPDATE claims SET
                  status = $status,
                  sensitivity = $sensitivity,
                  confidence = $confidence,
                  updated_at = time::now()
                WHERE claim_id = $claim_id;
                "#,
            )
            .bind(("claim_id", claim_id.to_string()))
            .bind(("status", status.as_str().to_string()))
            .bind(("sensitivity", sensitivity_to_store(sensitivity).to_string()))
            .bind(("confidence", confidence))
            .await?
            .check()?;
        Ok(())
    }

    async fn existing_claim_for_merge(
        &self,
        claim_id: &str,
    ) -> Result<ExistingClaimMergeRow, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT claim_id, status, sensitivity, confidence
                FROM claims
                WHERE claim_id = $claim_id
                LIMIT 1;
                "#,
            )
            .bind(("claim_id", claim_id.to_string()))
            .await?;
        let rows: Vec<ExistingClaimMergeRecord> = response.take(0)?;
        let row = rows.into_iter().next().ok_or_else(|| {
            StoreError::Schema(format!("missing existing claim before merge: {claim_id}"))
        })?;
        Ok(ExistingClaimMergeRow {
            status: ClaimStatus::parse(&row.status)?,
            sensitivity: parse_sensitivity(&row.sensitivity)?,
            confidence: row.confidence,
        })
    }

    async fn upsert_entity(&self, entity: &EntityCandidate) -> Result<(), StoreError> {
        if self.entity_exists(&entity.entity_id).await? {
            self.db
                .query(
                    r#"
                    UPDATE entities SET
                      entity_type = $entity_type,
                      canonical_name = $canonical_name,
                      updated_at = time::now()
                    WHERE entity_id = $entity_id;
                    "#,
                )
                .bind(("entity_id", entity.entity_id.clone()))
                .bind(("entity_type", entity.entity_type.as_str().to_string()))
                .bind(("canonical_name", entity.canonical_name.clone()))
                .await?
                .check()?;
        } else {
            self.db
                .query(
                    r#"
                    CREATE type::thing('entities', $record_id) SET
                      entity_id = $entity_id,
                      entity_type = $entity_type,
                      canonical_name = $canonical_name,
                      aliases = [],
                      metadata = {},
                      updated_at = time::now();
                    "#,
                )
                .bind(("record_id", entity_record_id(&entity.entity_id)))
                .bind(("entity_id", entity.entity_id.clone()))
                .bind(("entity_type", entity.entity_type.as_str().to_string()))
                .bind(("canonical_name", entity.canonical_name.clone()))
                .await?
                .check()?;
        }
        Ok(())
    }

    async fn entity_exists(&self, entity_id: &str) -> Result<bool, StoreError> {
        let mut response = self
            .db
            .query("SELECT entity_id FROM entities WHERE entity_id = $entity_id LIMIT 1;")
            .bind(("entity_id", entity_id.to_string()))
            .await?;
        let rows: Vec<EntityIdRow> = response.take(0)?;
        Ok(!rows.is_empty())
    }

    async fn require_predicate(&self, predicate_id: &str) -> Result<(), StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT predicate_id, label, default_sensitivity, allowed_use_modes
                FROM predicates
                WHERE predicate_id = $predicate_id
                LIMIT 1;
                "#,
            )
            .bind(("predicate_id", predicate_id.to_string()))
            .await?;
        let rows: Vec<super::PredicateRecord> = response.take(0)?;
        if rows.is_empty() {
            Err(StoreError::PredicateNotFound {
                predicate_id: predicate_id.to_string(),
            })
        } else {
            Ok(())
        }
    }

    async fn require_source_item(&self, item_id: &str) -> Result<(), StoreError> {
        let mut response = self
            .db
            .query(
                "SELECT item_id FROM conversation_items WHERE item_id = $item_id AND deleted_at = NONE LIMIT 1;",
            )
            .bind(("item_id", item_id.to_string()))
            .await?;
        let rows: Vec<SourceItemRow> = response.take(0)?;
        if rows.is_empty() {
            Err(StoreError::ConversationItemNotFound {
                item_id: item_id.to_string(),
            })
        } else {
            Ok(())
        }
    }

    async fn existing_claim_id(&self, fingerprint: &str) -> Result<Option<String>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT claim_id
                FROM claims
                WHERE dedupe_fingerprint = $dedupe_fingerprint
                  AND status != 'deleted'
                LIMIT 1;
                "#,
            )
            .bind(("dedupe_fingerprint", fingerprint.to_string()))
            .await?;
        let rows: Vec<ClaimIdRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(|row| row.claim_id))
    }

    async fn release_deleted_claim_fingerprint(&self, fingerprint: &str) -> Result<(), StoreError> {
        self.db
            .query(
                r#"
                UPDATE claims SET
                  dedupe_fingerprint = NONE,
                  updated_at = time::now()
                WHERE dedupe_fingerprint = $dedupe_fingerprint
                  AND status = 'deleted';
                "#,
            )
            .bind(("dedupe_fingerprint", fingerprint.to_string()))
            .await?
            .check()?;
        Ok(())
    }

    async fn insert_claim(
        &self,
        claim_id: &str,
        fingerprint: &str,
        candidate: &NewClaimCandidate,
    ) -> Result<(), StoreError> {
        self.db
            .query(
                r#"
                CREATE type::thing('claims', $record_id) SET
                  claim_id = $claim_id,
                  subject_entity_id = $subject_entity_id,
                  object_entity_id = $object_entity_id,
                  predicate_id = $predicate_id,
                  fact = $fact,
                  status = $status,
                  sensitivity = $sensitivity,
                  valid_from = NONE,
                  valid_to = NONE,
                  observed_at = time::now(),
                  confidence = $confidence,
                  dedupe_fingerprint = $dedupe_fingerprint,
                  retrieval_hints = $retrieval_hints,
                  policy_overrides = {},
                  metadata = $metadata,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(claim_id)))
            .bind(("claim_id", claim_id.to_string()))
            .bind(("subject_entity_id", candidate.subject.entity_id.clone()))
            .bind(("object_entity_id", Some(candidate.object.entity_id.clone())))
            .bind(("predicate_id", candidate.predicate_id.clone()))
            .bind(("fact", candidate.fact.clone()))
            .bind(("status", candidate.status.as_str().to_string()))
            .bind((
                "sensitivity",
                sensitivity_to_store(candidate.sensitivity).to_string(),
            ))
            .bind(("confidence", candidate.confidence))
            .bind(("dedupe_fingerprint", fingerprint.to_string()))
            .bind(("retrieval_hints", candidate.retrieval_hints.clone()))
            .bind(("metadata", candidate.metadata.clone()))
            .await?
            .check()?;
        Ok(())
    }

    async fn insert_support_evidence(
        &self,
        claim_id: &str,
        evidence: &EvidenceCandidate,
    ) -> Result<(), StoreError> {
        let relation_id = allocate_id("evidence");
        self.db
            .query(
                r#"
                CREATE type::thing('supported_by', $record_id) SET
                  relation_id = $relation_id,
                  claim_id = $claim_id,
                  source_kind = 'item',
                  source_item_id = $source_item_id,
                  source_object_type = NONE,
                  source_object_id = NONE,
                  authority = $authority,
                  excerpt = $excerpt,
                  observed_at = time::now(),
                  created_by = 'agent:primary',
                  metadata = {};
                "#,
            )
            .bind(("record_id", record_fragment(&relation_id)))
            .bind(("relation_id", relation_id))
            .bind(("claim_id", claim_id.to_string()))
            .bind(("source_item_id", evidence.source_item_id.clone()))
            .bind(("authority", evidence.authority.as_str().to_string()))
            .bind(("excerpt", evidence.excerpt.clone()))
            .await?
            .check()?;
        Ok(())
    }

    async fn claim_summary(
        &self,
        claim_id: &str,
        write_outcome: ClaimWriteOutcome,
    ) -> Result<ClaimSummary, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact, status, sensitivity
                FROM claims
                WHERE claim_id = $claim_id
                LIMIT 1;

                SELECT count() AS count
                FROM supported_by
                WHERE claim_id = $claim_id
                GROUP ALL;
                "#,
            )
            .bind(("claim_id", claim_id.to_string()))
            .await?;
        let rows: Vec<ClaimRow> = response.take(0)?;
        let counts: Vec<CountRow> = response.take(1)?;
        let row = rows
            .into_iter()
            .next()
            .ok_or_else(|| StoreError::Schema(format!("missing claim after write: {claim_id}")))?;
        let object_entity_id = row.object_entity_id.ok_or_else(|| {
            StoreError::Schema(format!(
                "claim missing object_entity_id after write: {claim_id}"
            ))
        })?;
        Ok(ClaimSummary {
            claim_id: row.claim_id,
            subject_entity_id: row.subject_entity_id,
            object_entity_id,
            predicate_id: row.predicate_id,
            fact: row.fact,
            status: ClaimStatus::parse(&row.status)?,
            sensitivity: parse_sensitivity(&row.sensitivity)?,
            evidence_count: counts.first().map_or(0, |row| row.count),
            write_outcome,
        })
    }
}

#[derive(Debug, Deserialize)]
struct EntityIdRow {
    #[allow(dead_code)]
    entity_id: String,
}

#[derive(Debug, Deserialize)]
struct SourceItemRow {
    #[allow(dead_code)]
    item_id: String,
}

#[derive(Debug, Deserialize)]
struct ClaimIdRow {
    claim_id: String,
}

#[derive(Debug, Deserialize)]
struct ClaimRow {
    claim_id: String,
    subject_entity_id: String,
    object_entity_id: Option<String>,
    predicate_id: String,
    fact: String,
    status: String,
    sensitivity: String,
}

#[derive(Debug, Deserialize)]
struct CountRow {
    count: i64,
}

#[derive(Debug, Deserialize)]
struct InspectionClaimRow {
    claim_id: String,
    subject_entity_id: String,
    object_entity_id: Option<String>,
    predicate_id: String,
    fact: String,
    status: String,
    sensitivity: String,
    confidence: Option<f64>,
    created_at: Datetime,
    updated_at: Datetime,
}

#[derive(Debug, Deserialize)]
struct InspectionPredicateRow {
    predicate_id: String,
    label: String,
}

#[derive(Debug, Deserialize)]
struct InspectionEntityRow {
    entity_id: String,
    entity_type: String,
    canonical_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectionEntity {
    entity_type: String,
    canonical_name: String,
}

#[derive(Debug, Deserialize)]
struct InspectionEvidenceCountRow {
    claim_id: String,
    count: i64,
}

#[derive(Debug, Deserialize)]
struct InspectionEvidenceRow {
    relation_id: Option<String>,
    source_item_id: Option<String>,
    authority: String,
    excerpt: Option<String>,
    observed_at: Option<Datetime>,
    created_at: Datetime,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ExistingClaimMergeRow {
    status: ClaimStatus,
    sensitivity: Sensitivity,
    confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ExistingClaimMergeRecord {
    #[allow(dead_code)]
    claim_id: String,
    status: String,
    sensitivity: String,
    confidence: Option<f64>,
}

fn strongest_claim_status(existing: ClaimStatus, incoming: ClaimStatus) -> ClaimStatus {
    match (existing, incoming) {
        (ClaimStatus::Candidate, ClaimStatus::Active | ClaimStatus::Confirmed) => incoming,
        (ClaimStatus::Active, ClaimStatus::Confirmed) => ClaimStatus::Confirmed,
        _ => existing,
    }
}

fn strongest_confidence(existing: Option<f64>, incoming: Option<f64>) -> Option<f64> {
    match (existing, incoming) {
        (Some(existing), Some(incoming)) => Some(existing.max(incoming)),
        (Some(existing), None) => Some(existing),
        (None, Some(incoming)) => Some(incoming),
        (None, None) => None,
    }
}

fn claim_fingerprint(
    subject_entity_id: &str,
    predicate_id: &str,
    object_entity_id: &str,
    fact: &str,
) -> String {
    let normalized_fact = normalize_fact_for_fingerprint(predicate_id, fact);
    format!(
        "claim-fingerprint:v1:{}:{}:{}:{}:{}:{}:{}:{}",
        subject_entity_id.len(),
        subject_entity_id,
        predicate_id.len(),
        predicate_id,
        object_entity_id.len(),
        object_entity_id,
        normalized_fact.len(),
        normalized_fact
    )
}

fn normalize_fact_for_fingerprint(predicate_id: &str, fact: &str) -> String {
    let normalized = fact
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    if predicate_id != "has_note" {
        return normalized;
    }

    let terminal_punctuation_count = normalized
        .chars()
        .rev()
        .take_while(|ch| matches!(ch, '.' | '!' | '?'))
        .count();
    if terminal_punctuation_count == 1 {
        normalized
            .trim_end_matches(['.', '!', '?'])
            .trim_end()
            .to_string()
    } else {
        normalized
    }
}

fn sensitivity_to_store(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
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

fn memory_claim_record(
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
        .and_then(|entity_id| entities.get(entity_id));
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

impl MemoryClaimRecord {
    fn matches_query(&self, query: &str) -> bool {
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
}

fn contains_case_folded(value: &str, query: &str) -> bool {
    value.to_ascii_lowercase().contains(query)
}

fn clamp_claim_inspection_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(50).clamp(1, 100)
}

fn format_datetime(value: Datetime) -> String {
    value.to_string()
}

fn entity_record_id(entity_id: &str) -> String {
    let mut encoded = String::with_capacity("entity_".len() + entity_id.len().saturating_mul(2));
    encoded.push_str("entity_");
    for byte in entity_id.as_bytes() {
        encoded.push(HEX_CHARS[usize::from(byte >> 4)]);
        encoded.push(HEX_CHARS[usize::from(byte & 0x0f)]);
    }
    encoded
}

const HEX_CHARS: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];
