use std::collections::HashSet;

use super::model::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CandidateSource {
    ActiveScope,
    ParticipantOverlap,
    ExplicitGrant,
    TrustedObjectLink,
    PublicHint,
    GraphExpansion,
}

#[derive(Clone, Copy)]
pub(super) enum PublicHintCandidates<'a> {
    AllMatchingHints,
    CandidateIds(&'a HashSet<MemoryId>),
}

impl PublicHintCandidates<'_> {
    pub(super) fn matches(self, memory: &MemoryItem, hints: &UntrustedHints) -> bool {
        match self {
            Self::AllMatchingHints => hint_score(memory, hints).score > 0,
            Self::CandidateIds(candidate_ids) => candidate_ids.contains(&memory.memory_id),
        }
    }
}

impl CandidateSource {
    pub(super) fn eligibility_reason(self) -> EligibilityReason {
        match self {
            Self::ActiveScope => EligibilityReason::ActiveScope,
            Self::ParticipantOverlap => EligibilityReason::ParticipantOverlap,
            Self::ExplicitGrant => EligibilityReason::ExplicitGrant,
            Self::TrustedObjectLink => EligibilityReason::TrustedObjectLink,
            Self::PublicHint => EligibilityReason::PublicHint,
            Self::GraphExpansion => EligibilityReason::GraphExpansion,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct RankedMemory {
    pub(super) score: u32,
    pub(super) reasons: Vec<RankReason>,
}

pub(super) fn hint_score(memory: &MemoryItem, hints: &UntrustedHints) -> RankedMemory {
    let mut ranked = RankedMemory::default();
    let query = normalize(&hints.query_text);
    let fuzzy_topics = normalized_set(&hints.fuzzy_topics);
    let fuzzy_entities = normalized_set(&hints.fuzzy_entities);

    for topic in &memory.retrieval_hints.topics {
        let topic = normalize(topic);
        if fuzzy_topics.contains(&topic) {
            ranked.score += 40;
            ranked.reasons.push(RankReason::FuzzyTopic);
        }
    }

    for keyword in &memory.retrieval_hints.keywords {
        let keyword = normalize(keyword);
        if !keyword.is_empty() && query.contains(&keyword) {
            ranked.score += 25;
            ranked.reasons.push(RankReason::FuzzyKeyword);
        }
    }

    if let Some(summary) = &memory.retrieval_hints.summary {
        let summary = normalize(summary);
        if fuzzy_entities
            .iter()
            .any(|entity| !entity.is_empty() && summary.contains(entity))
        {
            ranked.score += 10;
            ranked.reasons.push(RankReason::FuzzyKeyword);
        }
    }

    ranked
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalized_set(values: &[String]) -> HashSet<String> {
    values.iter().map(|value| normalize(value)).collect()
}

pub(super) fn relationship_is_current(status: RelationshipStatus) -> bool {
    matches!(
        status,
        RelationshipStatus::Active | RelationshipStatus::Confirmed
    )
}

pub(super) fn object_link_matches(
    policy: &ObjectLink,
    active: &ObjectLink,
    request: &MemoryRetrievalRequest,
) -> bool {
    if policy.object_type != active.object_type || policy.object_id != active.object_id {
        return false;
    }

    if policy
        .relation
        .as_ref()
        .is_some_and(|relation| active.relation.as_deref() != Some(relation.as_str()))
    {
        return false;
    }

    match &policy.authorized_actor_id {
        Some(actor_id) => {
            active
                .authorized_actor_id
                .as_ref()
                .is_some_and(|active_actor_id| active_actor_id == actor_id)
                || &request.requesting_principal_id == actor_id
                || request
                    .trusted
                    .active_human_ids
                    .iter()
                    .any(|active_actor_id| active_actor_id == actor_id)
                || request
                    .trusted
                    .active_agent_ids
                    .iter()
                    .any(|active_actor_id| active_actor_id == actor_id)
        }
        None => true,
    }
}

pub(super) fn grant_matches_memory(grant: &AccessGrant, memory: &MemoryItem) -> bool {
    grant
        .memory_id
        .as_ref()
        .is_some_and(|memory_id| memory_id == &memory.memory_id)
        || grant
            .scope_id
            .as_ref()
            .is_some_and(|scope_id| scope_id == &memory.home_scope_id)
}
