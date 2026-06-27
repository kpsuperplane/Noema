use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

use super::model::*;
use super::store_helpers::*;

/// In-memory implementation of the retrieval policy model.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    memories: HashMap<MemoryId, MemoryItem>,
    participants: Vec<MemoryParticipant>,
    subjects: Vec<MemorySubject>,
    purpose_rules: Vec<PurposeRule>,
    object_links: Vec<MemoryObjectLink>,
    access_grants: Vec<AccessGrant>,
    relationships: HashMap<RelationshipId, Relationship>,
    provenance: Vec<ProvenanceEdge>,
}

impl MemoryStore {
    /// Insert or replace a memory.
    pub fn insert_memory(&mut self, memory: MemoryItem) {
        self.memories.insert(memory.memory_id.clone(), memory);
    }

    /// Add a participant to a memory.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError::UnknownMemory`] when `memory_id` is not in
    /// the store.
    pub fn add_participant(
        &mut self,
        memory_id: impl Into<MemoryId>,
        principal_id: impl Into<PrincipalId>,
        role: ParticipantRole,
    ) -> Result<(), MemoryStoreError> {
        let memory_id = memory_id.into();
        self.require_memory(&memory_id)?;
        self.participants.push(MemoryParticipant {
            memory_id,
            principal_id: principal_id.into(),
            role,
        });
        Ok(())
    }

    /// Add a subject entity to a memory.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError::UnknownMemory`] when `memory_id` is not in
    /// the store.
    pub fn add_subject(
        &mut self,
        memory_id: impl Into<MemoryId>,
        entity_id: impl Into<EntityId>,
        role: SubjectRole,
    ) -> Result<(), MemoryStoreError> {
        let memory_id = memory_id.into();
        self.require_memory(&memory_id)?;
        self.subjects.push(MemorySubject {
            memory_id,
            entity_id: entity_id.into(),
            role,
        });
        Ok(())
    }

    /// Add or replace a purpose-specific rule for a memory.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError::UnknownMemory`] when `memory_id` is not in
    /// the store.
    pub fn add_purpose_rule(
        &mut self,
        memory_id: impl Into<MemoryId>,
        purpose: Purpose,
        effect: Effect,
    ) -> Result<(), MemoryStoreError> {
        let memory_id = memory_id.into();
        self.require_memory(&memory_id)?;
        self.purpose_rules
            .retain(|rule| rule.memory_id != memory_id || rule.purpose != purpose);
        self.purpose_rules.push(PurposeRule {
            memory_id,
            purpose,
            effect,
        });
        Ok(())
    }

    /// Link a memory to a trusted object.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError::UnknownMemory`] when `memory_id` is not in
    /// the store.
    pub fn add_object_link(
        &mut self,
        memory_id: impl Into<MemoryId>,
        mut object: ObjectLink,
        relation: impl Into<String>,
        authorized_actor_id: Option<PrincipalId>,
    ) -> Result<(), MemoryStoreError> {
        let memory_id = memory_id.into();
        self.require_memory(&memory_id)?;
        object.relation = Some(relation.into());
        object.authorized_actor_id = authorized_actor_id;
        self.object_links
            .push(MemoryObjectLink { memory_id, object });
        Ok(())
    }

    /// Add an access grant.
    pub fn add_access_grant(&mut self, grant: AccessGrant) {
        self.access_grants.push(grant);
    }

    /// Add provenance for a memory.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError::UnknownMemory`] when `memory_id` is not in
    /// the store.
    pub fn add_provenance(
        &mut self,
        memory_id: impl Into<MemoryId>,
        source_id: impl Into<String>,
    ) -> Result<(), MemoryStoreError> {
        let memory_id = memory_id.into();
        self.require_memory(&memory_id)?;
        self.provenance.push(ProvenanceEdge {
            memory_id,
            source_id: source_id.into(),
        });
        Ok(())
    }

    /// Add a graph relationship claim.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryStoreError`] when an active or confirmed relationship is
    /// missing supporting memory, references an unknown memory, or references a
    /// memory without provenance.
    pub fn add_relationship(&mut self, relationship: Relationship) -> Result<(), MemoryStoreError> {
        if relationship.status != RelationshipStatus::Candidate {
            let memory_id = relationship.memory_id.as_ref().ok_or_else(|| {
                MemoryStoreError::RelationshipRequiresMemory {
                    relationship_id: relationship.relationship_id.clone(),
                }
            })?;
            self.require_memory(memory_id).map_err(|_| {
                MemoryStoreError::RelationshipReferencesUnknownMemory {
                    relationship_id: relationship.relationship_id.clone(),
                    memory_id: memory_id.clone(),
                }
            })?;
            if !self.memory_has_provenance(memory_id) {
                return Err(MemoryStoreError::RelationshipRequiresProvenance {
                    relationship_id: relationship.relationship_id,
                    memory_id: memory_id.clone(),
                });
            }
        }

        self.relationships
            .insert(relationship.relationship_id.clone(), relationship);
        Ok(())
    }

    /// Retrieve memories using deterministic policy gates and ranking.
    #[must_use]
    pub fn retrieve(&self, request: &MemoryRetrievalRequest) -> MemoryRetrievalResult {
        self.retrieve_inner(request, PublicHintCandidates::AllMatchingHints)
    }

    /// Retrieve memories using precomputed public-hint candidate IDs.
    ///
    /// This is used by durable retrieval, where the database text index is the
    /// candidate generator for untrusted hint matches. Structured paths such as active
    /// scope, grants, participants, trusted object links, and graph expansion
    /// are still evaluated from canonical rows.
    #[must_use]
    pub fn retrieve_with_public_hint_candidates(
        &self,
        request: &MemoryRetrievalRequest,
        public_hint_candidates: &HashSet<MemoryId>,
    ) -> MemoryRetrievalResult {
        self.retrieve_inner(
            request,
            PublicHintCandidates::CandidateIds(public_hint_candidates),
        )
    }

    fn retrieve_inner(
        &self,
        request: &MemoryRetrievalRequest,
        public_hint_candidates: PublicHintCandidates<'_>,
    ) -> MemoryRetrievalResult {
        let mut result = MemoryRetrievalResult::default();
        let mut candidates = HashMap::<MemoryId, CandidateSource>::new();

        for memory in self.memories.values() {
            if let Some(source) =
                self.direct_candidate_source(memory, request, public_hint_candidates)
            {
                candidates.insert(memory.memory_id.clone(), source);
            }
        }

        let mut included_ids = HashSet::new();
        for (memory_id, source) in candidates {
            self.evaluate_and_push(memory_id, source, request, &mut included_ids, &mut result);
        }

        let start_entities = self.graph_start_entities(request, &included_ids);
        for relationship in self.relationships.values() {
            if !relationship_is_current(relationship.status) {
                continue;
            }

            let traversed = start_entities.contains(&relationship.subject_entity_id)
                || start_entities.contains(&relationship.object_entity_id);
            if !traversed {
                continue;
            }

            let Some(memory_id) = relationship.memory_id.clone() else {
                result.denied_for_audit.push(AuditDenial {
                    memory_id: None,
                    relationship_id: Some(relationship.relationship_id.clone()),
                    reason: DenialReason::RelationshipUnsupported,
                });
                continue;
            };

            if !self.memory_has_provenance(&memory_id) {
                result.denied_for_audit.push(AuditDenial {
                    memory_id: Some(memory_id),
                    relationship_id: Some(relationship.relationship_id.clone()),
                    reason: DenialReason::RelationshipUnsupported,
                });
                continue;
            }

            self.evaluate_and_push(
                memory_id,
                CandidateSource::GraphExpansion,
                request,
                &mut included_ids,
                &mut result,
            );
        }

        result
            .included
            .sort_by_key(|memory| Reverse(memory.rank_score));

        if !result.denied_for_audit.is_empty() {
            result.agent_visible_omissions.push(AgentVisibleOmission {
                reason: "policy_restricted_context",
            });
        }

        result
    }

    fn direct_candidate_source(
        &self,
        memory: &MemoryItem,
        request: &MemoryRetrievalRequest,
        public_hint_candidates: PublicHintCandidates<'_>,
    ) -> Option<CandidateSource> {
        if Self::home_scope_active(memory, request) {
            return Some(CandidateSource::ActiveScope);
        }

        if self.has_explicit_allow(memory, request) {
            return Some(CandidateSource::ExplicitGrant);
        }

        if self.memory_object_link_matches(&memory.memory_id, request) {
            return Some(CandidateSource::TrustedObjectLink);
        }

        if self.same_human_participant(memory, request) {
            return Some(CandidateSource::ParticipantOverlap);
        }

        if memory.sensitivity == Sensitivity::Public
            && public_hint_candidates.matches(memory, &request.untrusted_hints)
        {
            return Some(CandidateSource::PublicHint);
        }

        None
    }

    fn evaluate_and_push(
        &self,
        memory_id: MemoryId,
        source: CandidateSource,
        request: &MemoryRetrievalRequest,
        included_ids: &mut HashSet<MemoryId>,
        result: &mut MemoryRetrievalResult,
    ) {
        if included_ids.contains(&memory_id) {
            return;
        }

        match self.evaluate_memory(&memory_id, source, request) {
            Ok(retrieved) => {
                included_ids.insert(memory_id.clone());
                result.use_records.push(MemoryUseRecord {
                    memory_id,
                    stage: MemoryUseStage::Retrieved,
                });
                result.included.push(retrieved);
            }
            Err(reason) => result.denied_for_audit.push(AuditDenial {
                memory_id: Some(memory_id),
                relationship_id: None,
                reason,
            }),
        }
    }

    fn evaluate_memory(
        &self,
        memory_id: &MemoryId,
        source: CandidateSource,
        request: &MemoryRetrievalRequest,
    ) -> Result<RetrievedMemory, DenialReason> {
        let memory = self
            .memories
            .get(memory_id)
            .expect("candidate memory should exist");

        if self.has_explicit_deny(memory, request) {
            return Err(DenialReason::ExplicitDenyGrant);
        }

        if memory.sensitivity > request.trusted.sensitivity_ceiling {
            return Err(DenialReason::SensitivityCeiling);
        }

        if !Self::status_allows(memory.status, request) {
            return Err(match memory.status {
                MemoryStatus::Candidate => DenialReason::CandidateExcluded,
                MemoryStatus::Stale | MemoryStatus::Disputed => DenialReason::DisputedOrStale,
                _ => DenialReason::ArchivedOrDeleted,
            });
        }

        let purpose = self.purpose_effect(memory_id, request.trusted.purpose);
        if purpose == Some(Effect::Deny) {
            return Err(DenialReason::PurposeDenied);
        }
        Self::require_egress_policy(memory, request.trusted.purpose)?;

        match memory.sensitivity {
            Sensitivity::Public => {}
            Sensitivity::Normal => {
                if !self.aperture_matches(memory, request) {
                    return Err(DenialReason::OutsideSearchAperture);
                }
            }
            Sensitivity::Private => {
                if !Self::home_scope_active(memory, request)
                    && !self.has_explicit_allow(memory, request)
                {
                    return Err(DenialReason::OutsideSearchAperture);
                }
                Self::require_valid_policy(memory)?;
                self.require_allowed_purpose(memory_id, request)?;
                if !self.participant_visibility_allows(memory, request) {
                    return Err(DenialReason::ParticipantVisibilityDenied);
                }
            }
            Sensitivity::Sensitive => {
                if !self.aperture_matches(memory, request) {
                    return Err(DenialReason::OutsideSearchAperture);
                }
                Self::require_valid_policy(memory)?;
                self.require_allowed_purpose(memory_id, request)?;
                if !self.participant_visibility_allows(memory, request) {
                    return Err(DenialReason::ParticipantVisibilityDenied);
                }
                if !request.trusted.explicit_memory_request
                    && !self.memory_object_link_matches(memory_id, request)
                {
                    return Err(DenialReason::SensitiveUnlockMissing);
                }
            }
            Sensitivity::Secret => {
                Self::require_valid_policy(memory)?;
                self.require_allowed_purpose(memory_id, request)?;
                if !request.trusted.explicit_memory_request
                    || !request.trusted.approved_secret_access
                {
                    return Err(DenialReason::SecretApprovalMissing);
                }
                if !self.participant_visibility_allows(memory, request) {
                    return Err(DenialReason::ParticipantVisibilityDenied);
                }
            }
        }

        let ranked = self.rank_memory(memory, source, request);
        Ok(RetrievedMemory {
            memory_id: memory_id.clone(),
            rank_score: ranked.score,
            eligibility_reason: source.eligibility_reason(),
            rank_reasons: ranked.reasons,
        })
    }

    fn require_valid_policy(memory: &MemoryItem) -> Result<(), DenialReason> {
        if memory.retrieval_policy_status == RetrievalPolicyStatus::Valid {
            Ok(())
        } else {
            Err(DenialReason::RetrievalPolicyInvalid)
        }
    }

    fn require_allowed_purpose(
        &self,
        memory_id: &MemoryId,
        request: &MemoryRetrievalRequest,
    ) -> Result<(), DenialReason> {
        if self.purpose_effect(memory_id, request.trusted.purpose) == Some(Effect::Allow) {
            Ok(())
        } else {
            Err(DenialReason::PurposeDenied)
        }
    }

    fn require_egress_policy(memory: &MemoryItem, purpose: Purpose) -> Result<(), DenialReason> {
        if !matches!(
            purpose,
            Purpose::DraftExternalContent | Purpose::ExternalAction | Purpose::UseTool
        ) {
            return Ok(());
        }

        match memory.external_egress_policy {
            ExternalEgressPolicy::Allow => Ok(()),
            ExternalEgressPolicy::ApprovalRequired => {
                Err(DenialReason::ExternalEgressApprovalRequired)
            }
            ExternalEgressPolicy::Deny => Err(DenialReason::ExternalEgressDenied),
        }
    }

    fn rank_memory(
        &self,
        memory: &MemoryItem,
        source: CandidateSource,
        request: &MemoryRetrievalRequest,
    ) -> RankedMemory {
        let mut ranked = hint_score(memory, &request.untrusted_hints);

        match source {
            CandidateSource::ActiveScope => ranked.score += 80,
            CandidateSource::TrustedObjectLink => ranked.score += 70,
            CandidateSource::ExplicitGrant => ranked.score += 60,
            CandidateSource::ParticipantOverlap => ranked.score += 20,
            CandidateSource::GraphExpansion => ranked.score += 10,
            CandidateSource::PublicHint => {
                ranked.score += 30;
                ranked.reasons.push(RankReason::PublicHint);
            }
        }

        if request.trusted.explicit_memory_request {
            ranked.score += 100;
            ranked.reasons.push(RankReason::ExplicitMemoryRequest);
        }
        if self.memory_object_link_matches(&memory.memory_id, request) {
            ranked.score += 80;
            ranked.reasons.push(RankReason::TrustedObjectLink);
        }
        if self.same_human_participant(memory, request) {
            ranked.score += 20;
            ranked.reasons.push(RankReason::SameHumanParticipant);
        }
        if source == CandidateSource::GraphExpansion {
            ranked.score += 15;
            ranked.reasons.push(RankReason::GraphExpansion);
        }

        ranked
    }

    fn aperture_matches(&self, memory: &MemoryItem, request: &MemoryRetrievalRequest) -> bool {
        Self::home_scope_active(memory, request)
            || self.same_human_participant(memory, request)
            || self.has_explicit_allow(memory, request)
            || self.memory_object_link_matches(&memory.memory_id, request)
    }

    fn home_scope_active(memory: &MemoryItem, request: &MemoryRetrievalRequest) -> bool {
        request
            .trusted
            .active_scopes
            .iter()
            .any(|scope| scope == &memory.home_scope_id)
    }

    fn same_human_participant(
        &self,
        memory: &MemoryItem,
        request: &MemoryRetrievalRequest,
    ) -> bool {
        self.participants.iter().any(|participant| {
            participant.memory_id == memory.memory_id
                && participant.role == ParticipantRole::HumanInScope
                && request
                    .trusted
                    .active_human_ids
                    .contains(&participant.principal_id)
        })
    }

    fn participant_visibility_allows(
        &self,
        memory: &MemoryItem,
        request: &MemoryRetrievalRequest,
    ) -> bool {
        match memory.participant_visibility_policy {
            ParticipantVisibilityPolicy::AnyActiveHuman => {
                self.same_human_participant(memory, request)
            }
            ParticipantVisibilityPolicy::AllOriginalHumans => {
                let humans: Vec<_> = self
                    .participants
                    .iter()
                    .filter(|participant| {
                        participant.memory_id == memory.memory_id
                            && participant.role == ParticipantRole::HumanInScope
                    })
                    .collect();
                !humans.is_empty()
                    && humans.iter().all(|participant| {
                        request
                            .trusted
                            .active_human_ids
                            .contains(&participant.principal_id)
                    })
            }
            ParticipantVisibilityPolicy::OwnerOnly => memory
                .owner_principal_id
                .as_ref()
                .is_some_and(|owner| request.trusted.active_human_ids.contains(owner)),
            ParticipantVisibilityPolicy::ExplicitGrantOnly => {
                self.has_explicit_allow(memory, request)
            }
        }
    }

    fn has_explicit_allow(&self, memory: &MemoryItem, request: &MemoryRetrievalRequest) -> bool {
        self.access_grants.iter().any(|grant| {
            grant.effect == Effect::Allow
                && grant.principal_id == request.requesting_principal_id
                && matches!(
                    grant.permission,
                    Permission::Read | Permission::UseForRetrieval
                )
                && grant_matches_memory(grant, memory)
        })
    }

    fn has_explicit_deny(&self, memory: &MemoryItem, request: &MemoryRetrievalRequest) -> bool {
        self.access_grants.iter().any(|grant| {
            grant.effect == Effect::Deny
                && grant.principal_id == request.requesting_principal_id
                && matches!(
                    grant.permission,
                    Permission::Read | Permission::UseForRetrieval
                )
                && grant_matches_memory(grant, memory)
        })
    }

    fn memory_object_link_matches(
        &self,
        memory_id: &MemoryId,
        request: &MemoryRetrievalRequest,
    ) -> bool {
        self.object_links.iter().any(|link| {
            &link.memory_id == memory_id
                && request
                    .trusted
                    .active_object_links
                    .iter()
                    .any(|active| object_link_matches(&link.object, active, request))
        })
    }

    fn purpose_effect(&self, memory_id: &MemoryId, purpose: Purpose) -> Option<Effect> {
        self.purpose_rules
            .iter()
            .find(|rule| &rule.memory_id == memory_id && rule.purpose == purpose)
            .map(|rule| rule.effect)
    }

    fn status_allows(status: MemoryStatus, request: &MemoryRetrievalRequest) -> bool {
        match status {
            MemoryStatus::Active | MemoryStatus::Confirmed | MemoryStatus::Inferred => true,
            MemoryStatus::Candidate => request.trusted.include_candidate_memories,
            MemoryStatus::Stale
            | MemoryStatus::Superseded
            | MemoryStatus::Archived
            | MemoryStatus::Deleted
            | MemoryStatus::Disputed => false,
        }
    }

    fn graph_start_entities(
        &self,
        request: &MemoryRetrievalRequest,
        included_ids: &HashSet<MemoryId>,
    ) -> HashSet<EntityId> {
        let mut entities = HashSet::new();
        for link in &request.trusted.active_object_links {
            entities.insert(link.object_id.clone());
        }
        for subject in &self.subjects {
            if included_ids.contains(&subject.memory_id) {
                entities.insert(subject.entity_id.clone());
            }
        }
        entities
    }

    fn require_memory(&self, memory_id: &MemoryId) -> Result<(), MemoryStoreError> {
        if self.memories.contains_key(memory_id) {
            Ok(())
        } else {
            Err(MemoryStoreError::UnknownMemory(memory_id.clone()))
        }
    }

    fn memory_has_provenance(&self, memory_id: &MemoryId) -> bool {
        self.provenance
            .iter()
            .any(|provenance| &provenance.memory_id == memory_id)
    }
}
