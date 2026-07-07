use serde_json::Value;

use crate::store::{
    NoemaStore, StoreError,
    ids::{allocate_id, record_fragment},
    ontology::EntityCandidate,
};

use super::{
    labels::{entity_record_id, format_datetime, parse_sensitivity},
    model::{
        ClaimStatus, ClaimSummary, ClaimWriteOutcome, EvidenceCandidate, NewClaimCandidate,
        RelatedClaimCandidate, RelatedClaimRecord, SupersedeClaimCandidate,
    },
    rows::{
        ClaimIdRow, ClaimIdentityRecord, ClaimIdentityRow, ClaimRow, ClaimStatusRow, CountRow,
        EntityIdRow, ExistingClaimMergeRecord, ExistingClaimMergeRow, PredicateExistsRow,
        RelatedClaimRow, RelationIdRow, SourceItemRow,
    },
};

impl NoemaStore {
    /// Persist a relation between two related claims.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the relation cannot be written or read back.
    pub async fn relate_claims(
        &self,
        candidate: RelatedClaimCandidate,
    ) -> Result<RelatedClaimRecord, StoreError> {
        if candidate.claim_id == candidate.related_claim_id {
            return Err(StoreError::Schema(format!(
                "related claim relation cannot reference itself: {}",
                candidate.claim_id
            )));
        }
        self.require_non_deleted_claim(&candidate.claim_id).await?;
        self.require_non_deleted_claim(&candidate.related_claim_id)
            .await?;
        if let Some(existing) = self
            .related_claim_by_pair(
                &candidate.claim_id,
                &candidate.related_claim_id,
                &candidate.relation_kind,
            )
            .await?
        {
            return Ok(existing);
        }

        let relation_id = allocate_id("related_claim");
        self.db()
            .query(
                r#"
                CREATE type::record('related_to', $record_id) SET
                  relation_id = $relation_id,
                  claim_id = $claim_id,
                  related_claim_id = $related_claim_id,
                  relation_kind = $relation_kind,
                  rationale = $rationale,
                  metadata = {};
                "#,
            )
            .bind(("record_id", record_fragment(&relation_id)))
            .bind(("relation_id", relation_id.clone()))
            .bind(("claim_id", candidate.claim_id))
            .bind(("related_claim_id", candidate.related_claim_id))
            .bind(("relation_kind", candidate.relation_kind))
            .bind(("rationale", candidate.rationale))
            .await?
            .check()?;
        self.related_claim_by_relation_id(&relation_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "missing related claim relation after write: {relation_id}"
                ))
            })
    }

    /// List related-claim relations for a claim.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when stored relation data is invalid or the
    /// embedded store read fails.
    pub async fn related_claims(
        &self,
        claim_id: &str,
    ) -> Result<Vec<RelatedClaimRecord>, StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT relation_id, claim_id, related_claim_id, relation_kind, rationale, created_at
                FROM related_to
                WHERE claim_id = $claim_id
                ORDER BY created_at ASC, relation_id ASC;
                "#,
            )
            .bind(("claim_id", claim_id.to_string()))
            .await?;
        let rows: Vec<RelatedClaimRow> = response.take(0)?;
        rows.into_iter().map(related_claim_record).collect()
    }

    async fn related_claim_by_relation_id(
        &self,
        relation_id: &str,
    ) -> Result<Option<RelatedClaimRecord>, StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT relation_id, claim_id, related_claim_id, relation_kind, rationale, created_at
                FROM related_to
                WHERE relation_id = $relation_id
                LIMIT 1;
                "#,
            )
            .bind(("relation_id", relation_id.to_string()))
            .await?;
        let rows: Vec<RelatedClaimRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(related_claim_record)
            .transpose()
    }

    async fn related_claim_by_pair(
        &self,
        claim_id: &str,
        related_claim_id: &str,
        relation_kind: &str,
    ) -> Result<Option<RelatedClaimRecord>, StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT relation_id, claim_id, related_claim_id, relation_kind, rationale, created_at
                FROM related_to
                WHERE claim_id = $claim_id
                  AND related_claim_id = $related_claim_id
                  AND relation_kind = $relation_kind
                LIMIT 1;
                "#,
            )
            .bind(("claim_id", claim_id.to_string()))
            .bind(("related_claim_id", related_claim_id.to_string()))
            .bind(("relation_kind", relation_kind.to_string()))
            .await?;
        let rows: Vec<RelatedClaimRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(related_claim_record)
            .transpose()
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
        self.create_or_reinforce_claim_unlocked(candidate).await
    }

    async fn create_or_reinforce_claim_unlocked(
        &self,
        candidate: NewClaimCandidate,
    ) -> Result<ClaimSummary, StoreError> {
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

    /// Return whether a candidate would reinforce a specific claim by exact fingerprint.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the candidate's exact fingerprint lookup fails.
    pub(crate) async fn claim_candidate_resolves_to_claim_id(
        &self,
        candidate: &NewClaimCandidate,
        claim_id: &str,
    ) -> Result<bool, StoreError> {
        let fingerprint = claim_fingerprint(
            &candidate.subject.entity_id,
            &candidate.predicate_id,
            &candidate.object.entity_id,
            &candidate.fact,
        );
        Ok(self
            .existing_claim_id(&fingerprint)
            .await?
            .is_some_and(|existing_claim_id| existing_claim_id == claim_id))
    }

    /// Reinforce a specific existing non-deleted claim by id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the target claim is missing, deleted, or
    /// incompatible with the incoming candidate, or when validation/write fails.
    pub async fn reinforce_claim_by_id(
        &self,
        claim_id: &str,
        candidate: NewClaimCandidate,
    ) -> Result<ClaimSummary, StoreError> {
        let _claim_write_guard = self.claim_write_lock.lock().await;
        self.require_predicate(&candidate.predicate_id).await?;
        self.require_source_item(&candidate.evidence.source_item_id)
            .await?;
        self.require_compatible_reinforcement_target(claim_id, &candidate)
            .await?;
        self.upsert_entity(&candidate.subject).await?;
        self.upsert_entity(&candidate.object).await?;
        self.merge_reinforced_claim(claim_id, &candidate).await?;
        self.insert_support_evidence(claim_id, &candidate.evidence)
            .await?;
        self.claim_summary(claim_id, ClaimWriteOutcome::Reinforced)
            .await
    }

    /// Reinforce a specific claim selected from a bounded consolidation match.
    ///
    /// This accepts semantic object variants only when the selected match still
    /// names the same object as the stored target claim.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the target claim is missing, deleted, or
    /// incompatible with the incoming candidate or selected match context.
    pub(crate) async fn reinforce_matched_claim_by_id(
        &self,
        claim_id: &str,
        matched_object_entity_id: Option<&str>,
        candidate: NewClaimCandidate,
    ) -> Result<ClaimSummary, StoreError> {
        let _claim_write_guard = self.claim_write_lock.lock().await;
        self.require_predicate(&candidate.predicate_id).await?;
        self.require_source_item(&candidate.evidence.source_item_id)
            .await?;
        self.require_matched_reinforcement_target(claim_id, matched_object_entity_id, &candidate)
            .await?;
        self.merge_reinforced_claim(claim_id, &candidate).await?;
        self.insert_support_evidence(claim_id, &candidate.evidence)
            .await?;
        self.claim_summary(claim_id, ClaimWriteOutcome::Reinforced)
            .await
    }

    /// Create or reinforce a replacement claim, mark the old claim superseded,
    /// and record a supersedes edge from replacement to old claim.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when either claim is missing/deleted, when the
    /// replacement resolves to the same claim, or when validation/write fails.
    pub async fn supersede_claim(
        &self,
        candidate: SupersedeClaimCandidate,
    ) -> Result<ClaimSummary, StoreError> {
        let _claim_write_guard = self.claim_write_lock.lock().await;
        self.require_non_deleted_claim(&candidate.superseded_claim_id)
            .await?;
        if self
            .claim_candidate_resolves_to_claim_id(
                &candidate.replacement,
                &candidate.superseded_claim_id,
            )
            .await?
        {
            return Err(StoreError::Schema(format!(
                "claim cannot supersede itself: {}",
                candidate.superseded_claim_id
            )));
        }
        let summary = self
            .create_or_reinforce_claim_unlocked(candidate.replacement)
            .await?;
        self.insert_supersedes_relation(
            &summary.claim_id,
            &candidate.superseded_claim_id,
            candidate.metadata,
        )
        .await?;
        Ok(summary)
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

        self.db()
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
            .bind(("sensitivity", sensitivity.as_str().to_string()))
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
            .db()
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

    async fn require_compatible_reinforcement_target(
        &self,
        claim_id: &str,
        candidate: &NewClaimCandidate,
    ) -> Result<(), StoreError> {
        let existing = self.claim_identity_for_reinforcement(claim_id).await?;
        if existing.status == ClaimStatus::Deleted {
            return Err(StoreError::Schema(format!(
                "deleted claim cannot be reinforced: {claim_id}"
            )));
        }
        if existing.subject_entity_id != candidate.subject.entity_id
            || existing.predicate_id != candidate.predicate_id
            || existing.object_entity_id.as_deref() != Some(candidate.object.entity_id.as_str())
        {
            return Err(StoreError::Schema(format!(
                "claim reinforcement target is incompatible: {claim_id}"
            )));
        }
        Ok(())
    }

    async fn require_matched_reinforcement_target(
        &self,
        claim_id: &str,
        matched_object_entity_id: Option<&str>,
        candidate: &NewClaimCandidate,
    ) -> Result<(), StoreError> {
        let existing = self.claim_identity_for_reinforcement(claim_id).await?;
        if existing.status == ClaimStatus::Deleted {
            return Err(StoreError::Schema(format!(
                "deleted claim cannot be reinforced: {claim_id}"
            )));
        }
        if existing.subject_entity_id != candidate.subject.entity_id
            || existing.predicate_id != candidate.predicate_id
            || existing.object_entity_id.as_deref() != matched_object_entity_id
        {
            return Err(StoreError::Schema(format!(
                "claim reinforcement target is incompatible: {claim_id}"
            )));
        }
        Ok(())
    }

    async fn claim_identity_for_reinforcement(
        &self,
        claim_id: &str,
    ) -> Result<ClaimIdentityRow, StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, status
                FROM claims
                WHERE claim_id = $claim_id
                LIMIT 1;
                "#,
            )
            .bind(("claim_id", claim_id.to_string()))
            .await?;
        let rows: Vec<ClaimIdentityRecord> = response.take(0)?;
        let row = rows.into_iter().next().ok_or_else(|| {
            StoreError::Schema(format!("missing claim for reinforcement: {claim_id}"))
        })?;
        Ok(ClaimIdentityRow {
            subject_entity_id: row.subject_entity_id,
            object_entity_id: row.object_entity_id,
            predicate_id: row.predicate_id,
            status: ClaimStatus::parse(&row.status)?,
        })
    }

    async fn upsert_entity(&self, entity: &EntityCandidate) -> Result<(), StoreError> {
        if self.entity_exists(&entity.entity_id).await? {
            self.db()
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
            self.db()
                .query(
                    r#"
                    CREATE type::record('entities', $record_id) SET
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
            .db()
            .query("SELECT entity_id FROM entities WHERE entity_id = $entity_id LIMIT 1;")
            .bind(("entity_id", entity_id.to_string()))
            .await?;
        let rows: Vec<EntityIdRow> = response.take(0)?;
        Ok(!rows.is_empty())
    }

    async fn require_predicate(&self, predicate_id: &str) -> Result<(), StoreError> {
        let mut response = self
            .db()
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
        let rows: Vec<PredicateExistsRow> = response.take(0)?;
        if rows.is_empty() {
            Err(StoreError::PredicateNotFound {
                predicate_id: predicate_id.to_string(),
            })
        } else {
            Ok(())
        }
    }

    pub(in crate::store) async fn require_source_item(
        &self,
        item_id: &str,
    ) -> Result<(), StoreError> {
        let mut response = self
            .db()
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

    async fn require_non_deleted_claim(&self, claim_id: &str) -> Result<(), StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT claim_id, status
                FROM claims
                WHERE claim_id = $claim_id
                LIMIT 1;
                "#,
            )
            .bind(("claim_id", claim_id.to_string()))
            .await?;
        let rows: Vec<ClaimStatusRow> = response.take(0)?;
        let Some(row) = rows.into_iter().next() else {
            return Err(StoreError::Schema(format!(
                "missing claim for relation: {claim_id}"
            )));
        };
        if ClaimStatus::parse(&row.status)? == ClaimStatus::Deleted {
            return Err(StoreError::Schema(format!(
                "deleted claim for relation: {claim_id}"
            )));
        }
        Ok(())
    }

    async fn existing_claim_id(&self, fingerprint: &str) -> Result<Option<String>, StoreError> {
        let mut response = self
            .db()
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
        self.db()
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
        self.db()
            .query(
                r#"
                CREATE type::record('claims', $record_id) SET
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
            .bind(("sensitivity", candidate.sensitivity.as_str().to_string()))
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
        self.db()
            .query(
                r#"
                CREATE type::record('supported_by', $record_id) SET
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

    async fn insert_supersedes_relation(
        &self,
        claim_id: &str,
        superseded_claim_id: &str,
        metadata: Value,
    ) -> Result<(), StoreError> {
        if claim_id == superseded_claim_id {
            return Err(StoreError::Schema(format!(
                "claim cannot supersede itself: {claim_id}"
            )));
        }
        self.require_non_deleted_claim(claim_id).await?;
        self.require_non_deleted_claim(superseded_claim_id).await?;
        if self
            .supersedes_relation_exists(claim_id, superseded_claim_id)
            .await?
        {
            self.mark_claim_superseded(superseded_claim_id).await?;
            return Ok(());
        }

        let relation_id = allocate_id("supersedes");
        self.db()
            .query(
                r#"
                CREATE type::record('supersedes', $record_id) SET
                  relation_id = $relation_id,
                  claim_id = $claim_id,
                  superseded_claim_id = $superseded_claim_id,
                  metadata = $metadata;
                "#,
            )
            .bind(("record_id", record_fragment(&relation_id)))
            .bind(("relation_id", relation_id))
            .bind(("claim_id", claim_id.to_string()))
            .bind(("superseded_claim_id", superseded_claim_id.to_string()))
            .bind(("metadata", metadata))
            .await?
            .check()?;
        self.mark_claim_superseded(superseded_claim_id).await?;
        Ok(())
    }

    async fn supersedes_relation_exists(
        &self,
        claim_id: &str,
        superseded_claim_id: &str,
    ) -> Result<bool, StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT relation_id
                FROM supersedes
                WHERE claim_id = $claim_id
                  AND superseded_claim_id = $superseded_claim_id
                LIMIT 1;
                "#,
            )
            .bind(("claim_id", claim_id.to_string()))
            .bind(("superseded_claim_id", superseded_claim_id.to_string()))
            .await?;
        let rows: Vec<RelationIdRow> = response.take(0)?;
        Ok(!rows.is_empty())
    }

    async fn mark_claim_superseded(&self, claim_id: &str) -> Result<(), StoreError> {
        self.db()
            .query(
                r#"
                UPDATE claims SET
                  status = 'superseded',
                  updated_at = time::now()
                WHERE claim_id = $claim_id
                  AND status != 'deleted';
                "#,
            )
            .bind(("claim_id", claim_id.to_string()))
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
            .db()
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
fn related_claim_record(row: RelatedClaimRow) -> Result<RelatedClaimRecord, StoreError> {
    Ok(RelatedClaimRecord {
        relation_id: row.relation_id,
        claim_id: row.claim_id,
        related_claim_id: row.related_claim_id,
        relation_kind: row.relation_kind,
        rationale: row.rationale,
        created_at: format_datetime(row.created_at),
    })
}
