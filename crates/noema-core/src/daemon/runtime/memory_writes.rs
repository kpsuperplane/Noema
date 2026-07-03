use crate::{
    ClaimWriteOutcome,
    memory::Sensitivity,
    memory::consolidation::{
        CanonicalClaimCandidate, ConsolidationDecision, ConsolidationDecisionKind,
        MemoryConsolidationError, MemoryWriteProposal, PredicateResolution,
        build_claim_canonicalization_prompt, build_consolidation_prompt,
        parse_canonicalization_response, parse_consolidation_decision,
    },
    memory::extraction::{
        ExtractorMemoryResponse, partition_memory_extraction_response_with_assistant_items,
    },
    provider::GenerateRequest,
    store::{
        ClaimStatus, ConsolidationMatch, ConsolidationMatchRequest, NewClaimCandidate,
        RelatedClaimCandidate, SupersedeClaimCandidate,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::{
    actor::CodexRuntimeActor,
    handle::RuntimeModelProvider,
    transcript_persistence::send_transient_turn_item,
    turn::{ExplicitMemoryOutcome, ProviderMemoryProposalBatch, ValidatedProviderMemoryProposal},
};
use crate::daemon::{
    memory_pipeline::{
        ConversationMemoryContext, claim_status_from_memory_status, deterministic_canonical_claim,
        explicit_memory_write_proposal, memory_activity, new_claim_from_canonical,
        predicate_proposal_candidate_from_canonical, project_scope_from_cwd,
        provider_memory_write_proposal,
    },
    protocol::{DaemonError, TurnActivityStatus, TurnStreamEvent},
};

impl CodexRuntimeActor {
    async fn canonicalize_memory_write(
        &self,
        provider: &dyn RuntimeModelProvider,
        proposal: &MemoryWriteProposal,
    ) -> Result<Vec<CanonicalClaimCandidate>, DaemonError> {
        let predicates = self.store.predicate_catalog().await?;
        let catalog_json = serde_json::to_value(&predicates).map_err(|error| {
            DaemonError::Protocol(format!("predicate catalog serialization failed: {error}"))
        })?;
        let prompt = build_claim_canonicalization_prompt(proposal, &catalog_json);
        let mut ignored_events = |_| {};
        let response = provider
            .generate_streaming(GenerateRequest::text(prompt), &mut ignored_events)
            .await
            .map_err(DaemonError::Provider)?;
        let parsed = match parse_canonicalization_response(&response.assistant_text()) {
            Ok(parsed) => parsed,
            Err(MemoryConsolidationError::InvalidCanonicalization(
                "candidates must not be empty",
            )) => return Ok(Vec::new()),
            Err(error) => {
                return Err(DaemonError::Protocol(format!(
                    "memory canonicalization failed: {error}"
                )));
            }
        };
        for candidate in &parsed.candidates {
            if let PredicateResolution::PromotedPredicate { predicate_id } = &candidate.predicate
                && !predicates
                    .iter()
                    .any(|predicate| predicate.predicate_id == *predicate_id)
            {
                return Err(DaemonError::Protocol(format!(
                    "memory canonicalization failed: unknown promoted predicate_id {predicate_id}"
                )));
            }
        }
        Ok(parsed.candidates)
    }

    pub(super) async fn persist_provider_memory_proposals(
        &mut self,
        batches: Vec<ProviderMemoryProposalBatch>,
        provider: Arc<dyn RuntimeModelProvider>,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let Some(activity_context) = batches.first().map(|batch| batch.context.clone()) else {
            return Ok(());
        };
        let turn_index = activity_context.turn_index;
        let activity_id = format!(
            "memory_extraction:{}:{turn_index}",
            activity_context.conversation_id
        );
        let proposal_count = batches
            .iter()
            .map(|batch| batch.proposals.len())
            .sum::<usize>();
        let mut validated_proposals = Vec::with_capacity(proposal_count);
        let mut rejected_extraction_proposals = Vec::new();
        let mut proposal_index_offset = 0usize;
        for batch in batches {
            let batch_proposal_count = batch.proposals.len();
            let assistant_item_texts = batch
                .context
                .assistant_items
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>();
            let partition = partition_memory_extraction_response_with_assistant_items(
                ExtractorMemoryResponse {
                    proposals: batch.proposals,
                },
                &batch.context.user_content,
                &assistant_item_texts,
            );
            for rejected in partition.rejected {
                rejected_extraction_proposals.push(json!({
                    "proposal_index": proposal_index_offset + rejected.proposal_index,
                    "error": rejected.error,
                }));
            }
            for accepted in partition.accepted {
                validated_proposals.push(ValidatedProviderMemoryProposal {
                    context: batch.context.clone(),
                    proposal: accepted.proposal,
                    proposal_index: proposal_index_offset + accepted.proposal_index,
                });
            }
            proposal_index_offset += batch_proposal_count;
        }

        let validated_proposal_count = validated_proposals.len();
        if validated_proposal_count == 0 {
            return Ok(());
        }
        let rejected_extraction_proposal_count = rejected_extraction_proposals.len();

        let proposed_summary = match validated_proposal_count {
            1 => "creating 1 memory candidate".to_string(),
            count => format!("creating {count} memory candidates"),
        };
        let proposed_activity = memory_activity(
            &activity_id,
            TurnActivityStatus::Started,
            "Memory proposed",
            Some(&proposed_summary),
            json!({
                "turn_index": turn_index,
                "proposal_count": proposal_count,
                "validated_proposal_count": validated_proposal_count,
                "rejected_extraction_proposal_count": rejected_extraction_proposal_count,
                "rejected_extraction_proposals": rejected_extraction_proposals.clone(),
                "cwd_project_hint": project_scope_from_cwd(activity_context.cwd.as_deref()),
            }),
        );
        send_transient_turn_item(&activity_context, proposed_activity, item_tx);
        tokio::task::yield_now().await;

        let mut claim_ids = Vec::with_capacity(validated_proposal_count);
        let mut claim_outcomes = Vec::with_capacity(validated_proposal_count);
        let mut created_claim_count = 0usize;
        let mut reinforced_claim_count = 0usize;
        let mut disputed_claim_count = 0usize;
        let mut related_claim_count = 0usize;
        let mut needs_review_claim_count = 0usize;
        let mut superseded_claim_count = 0usize;
        let mut active_saved_claim_count = 0usize;
        let mut predicate_proposal_count = 0usize;
        let mut failed_proposals = Vec::new();
        for proposal in validated_proposals {
            let proposal_index = proposal.proposal_index;
            let write_proposal = provider_memory_write_proposal(
                &proposal.proposal,
                &proposal.context,
                proposal_index,
                "ordinary_chat",
            );

            let canonical_candidates = match self
                .canonicalize_memory_write(provider.as_ref(), &write_proposal)
                .await
            {
                Ok(candidates) => candidates,
                Err(error) => {
                    failed_proposals.push(json!({
                        "proposal_index": proposal_index,
                        "error": error.to_string(),
                    }));
                    continue;
                }
            };
            let canonical_candidates_empty = canonical_candidates.is_empty();
            for canonical in canonical_candidates {
                match &canonical.predicate {
                    PredicateResolution::PromotedPredicate { .. } => {
                        let candidate = match new_claim_from_canonical(
                            &canonical,
                            &write_proposal,
                            crate::EvidenceAuthority::AgentInference,
                        ) {
                            Ok(candidate) => candidate,
                            Err(error) => {
                                failed_proposals.push(json!({
                                    "proposal_index": proposal_index,
                                    "error": error.to_string(),
                                }));
                                continue;
                            }
                        };
                        match self
                            .consolidate_promoted_claim(provider.as_ref(), candidate, &canonical)
                            .await
                        {
                            Ok(outcome) => {
                                match outcome.outcome {
                                    "created" => created_claim_count += 1,
                                    "reinforced" => reinforced_claim_count += 1,
                                    "disputed" => disputed_claim_count += 1,
                                    "related" => related_claim_count += 1,
                                    "superseded" => superseded_claim_count += 1,
                                    "needs_review" => needs_review_claim_count += 1,
                                    _ => {}
                                }
                                if saved_claim_counts_as_active(outcome.status) {
                                    active_saved_claim_count += 1;
                                }
                                claim_outcomes.push(json!({
                                    "claim_id": outcome.claim_id,
                                    "outcome": outcome.outcome,
                                    "fact_preview": outcome.fact_preview,
                                    "sensitivity": outcome.sensitivity,
                                }));
                                if let Some(claim_id) = outcome.claim_id {
                                    claim_ids.push(claim_id);
                                }
                            }
                            Err(error) => {
                                failed_proposals.push(json!({
                                    "proposal_index": proposal_index,
                                    "error": error.to_string(),
                                }));
                            }
                        }
                    }
                    PredicateResolution::PredicateProposal { .. } => {
                        let Some(candidate) = predicate_proposal_candidate_from_canonical(
                            &canonical,
                            &write_proposal,
                        ) else {
                            continue;
                        };
                        match self.store.create_predicate_proposal(candidate).await {
                            Ok(record) => {
                                predicate_proposal_count += 1;
                                claim_outcomes.push(json!({
                                    "outcome": "needs_review",
                                    "predicate_proposal_id": record.proposal_id,
                                    "fact_preview": fact_preview(&canonical.fact),
                                    "sensitivity": sensitivity_label(canonical.sensitivity),
                                }));
                            }
                            Err(error) => {
                                failed_proposals.push(json!({
                                    "proposal_index": proposal_index,
                                    "error": error.to_string(),
                                }));
                            }
                        }
                    }
                    PredicateResolution::FallbackNote => {
                        let candidate = deterministic_canonical_claim(
                            &write_proposal,
                            claim_status_from_memory_status(proposal.proposal.status),
                            Some(f64::from(proposal.proposal.proposal.confidence)),
                            crate::EvidenceAuthority::AgentInference,
                        );
                        match self.store.create_or_reinforce_claim(candidate).await {
                            Ok(summary) => {
                                match summary.write_outcome {
                                    ClaimWriteOutcome::Created => created_claim_count += 1,
                                    ClaimWriteOutcome::Reinforced => reinforced_claim_count += 1,
                                }
                                if saved_claim_counts_as_active(summary.status) {
                                    active_saved_claim_count += 1;
                                }
                                claim_outcomes.push(claim_outcome_json(&summary));
                                claim_ids.push(summary.claim_id);
                            }
                            Err(error) => {
                                failed_proposals.push(json!({
                                    "proposal_index": proposal_index,
                                    "error": error.to_string(),
                                }));
                            }
                        }
                    }
                }
            }

            if canonical_candidates_empty {
                let candidate = deterministic_canonical_claim(
                    &write_proposal,
                    claim_status_from_memory_status(proposal.proposal.status),
                    Some(f64::from(proposal.proposal.proposal.confidence)),
                    crate::EvidenceAuthority::AgentInference,
                );
                match self.store.create_or_reinforce_claim(candidate).await {
                    Ok(summary) => {
                        match summary.write_outcome {
                            ClaimWriteOutcome::Created => created_claim_count += 1,
                            ClaimWriteOutcome::Reinforced => reinforced_claim_count += 1,
                        }
                        if saved_claim_counts_as_active(summary.status) {
                            active_saved_claim_count += 1;
                        }
                        claim_outcomes.push(claim_outcome_json(&summary));
                        claim_ids.push(summary.claim_id);
                    }
                    Err(error) => {
                        failed_proposals.push(json!({
                            "proposal_index": proposal_index,
                            "error": error.to_string(),
                        }));
                    }
                }
            }
        }

        let failed_proposal_count = failed_proposals.len();
        let saved_claim_count = claim_ids.len();
        let review_claim_count =
            disputed_claim_count + needs_review_claim_count + superseded_claim_count;
        let (status, title, persisted_summary) =
            if predicate_proposal_count > 0 || review_claim_count > 0 {
                (
                    if failed_proposal_count == 0 {
                        TurnActivityStatus::Completed
                    } else {
                        TurnActivityStatus::Failed
                    },
                    "Memory needs review",
                    provider_memory_review_summary(
                        predicate_proposal_count,
                        review_claim_count,
                        failed_proposal_count,
                    ),
                )
            } else {
                provider_memory_claim_activity(
                    saved_claim_count,
                    failed_proposal_count,
                    failed_proposals
                        .first()
                        .and_then(|proposal| proposal.get("error"))
                        .and_then(Value::as_str),
                )
            };
        let activity = memory_activity(
            &activity_id,
            status,
            title,
            Some(&persisted_summary),
            json!({
                "turn_index": turn_index,
                "source": "provider_structured_output",
                "proposal_count": proposal_count,
                "validated_proposal_count": validated_proposal_count,
                "rejected_extraction_proposal_count": rejected_extraction_proposal_count,
                "rejected_extraction_proposals": rejected_extraction_proposals,
                "claim_ids": claim_ids,
                "claim_outcomes": claim_outcomes,
                "created_claim_count": created_claim_count,
                "reinforced_claim_count": reinforced_claim_count,
                "disputed_claim_count": disputed_claim_count,
                "related_claim_count": related_claim_count,
                "superseded_claim_count": superseded_claim_count,
                "needs_review_claim_count": needs_review_claim_count,
                "active_saved_claim_count": active_saved_claim_count,
                "predicate_proposal_count": predicate_proposal_count,
                "failed_proposal_count": failed_proposal_count,
                "failed_proposals": failed_proposals,
                "cwd_project_hint": project_scope_from_cwd(activity_context.cwd.as_deref()),
            }),
        );
        self.persist_and_send_turn_item(&activity_context, activity, item_tx)
            .await
    }

    async fn consolidate_promoted_claim(
        &self,
        provider: &dyn RuntimeModelProvider,
        candidate: NewClaimCandidate,
        canonical: &CanonicalClaimCandidate,
    ) -> Result<PersistedMemoryOutcome, DaemonError> {
        let query_terms = canonical
            .retrieval_hints
            .get("keywords")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let matches = self
            .store
            .find_consolidation_matches(ConsolidationMatchRequest {
                subject_entity_id: candidate.subject.entity_id.clone(),
                predicate_id: candidate.predicate_id.clone(),
                object_entity_id: Some(candidate.object.entity_id.clone()),
                query_terms,
                sensitivity: candidate.sensitivity,
                limit: 12,
            })
            .await?;

        if matches.is_empty() {
            let summary = self.store.create_or_reinforce_claim(candidate).await?;
            return Ok(PersistedMemoryOutcome::from_claim_summary(summary));
        }

        let existing_json = serde_json::to_value(
            matches
                .iter()
                .map(|item| {
                    json!({
                        "memory_id": item.claim_id,
                        "claim_id": item.claim_id,
                        "fact": item.fact,
                        "predicate_id": item.predicate_id,
                        "status": claim_status_label(item.status),
                        "sensitivity": sensitivity_label(item.sensitivity),
                    })
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|error| DaemonError::Protocol(format!("match serialization failed: {error}")))?;
        let prompt = build_consolidation_prompt(canonical, &existing_json);
        let mut ignored_events = |_| {};
        let response = provider
            .generate_streaming(GenerateRequest::text(prompt), &mut ignored_events)
            .await
            .map_err(DaemonError::Provider)?;
        let decision =
            parse_consolidation_decision(&response.assistant_text()).map_err(|error| {
                DaemonError::Protocol(format!("memory consolidation failed: {error}"))
            })?;
        self.persist_consolidation_decision(candidate, decision, &matches)
            .await
    }

    async fn persist_consolidation_decision(
        &self,
        mut candidate: NewClaimCandidate,
        decision: ConsolidationDecision,
        allowed_matches: &[ConsolidationMatch],
    ) -> Result<PersistedMemoryOutcome, DaemonError> {
        match decision.decision {
            ConsolidationDecisionKind::Create => {
                let summary = self.store.create_or_reinforce_claim(candidate).await?;
                Ok(PersistedMemoryOutcome::from_claim_summary(summary))
            }
            ConsolidationDecisionKind::Reinforce => {
                let existing_claim_id = decision.existing_claim_id.clone().ok_or_else(|| {
                    DaemonError::Protocol(
                        "reinforce decision missing existing claim id".to_string(),
                    )
                })?;
                let selected_match = validate_consolidation_decision_target(
                    "reinforce",
                    &existing_claim_id,
                    allowed_matches,
                )?;
                let summary = self
                    .store
                    .reinforce_matched_claim_by_id(
                        &existing_claim_id,
                        selected_match.object_entity_id.as_deref(),
                        candidate,
                    )
                    .await?;
                Ok(PersistedMemoryOutcome::from_claim_summary(summary))
            }
            ConsolidationDecisionKind::Dispute => {
                candidate.status = ClaimStatus::Disputed;
                let summary = self.store.create_or_reinforce_claim(candidate).await?;
                Ok(PersistedMemoryOutcome {
                    claim_id: Some(summary.claim_id),
                    outcome: "disputed",
                    fact_preview: fact_preview(&summary.fact),
                    sensitivity: sensitivity_label(summary.sensitivity).to_string(),
                    status: summary.status,
                })
            }
            ConsolidationDecisionKind::Relate => {
                let related_claim_id = decision.existing_claim_id.clone().ok_or_else(|| {
                    DaemonError::Protocol("relate decision missing existing claim id".to_string())
                })?;
                validate_consolidation_decision_target(
                    "relate",
                    &related_claim_id,
                    allowed_matches,
                )?;
                if self
                    .store
                    .claim_candidate_resolves_to_claim_id(&candidate, &related_claim_id)
                    .await?
                {
                    return Err(DaemonError::Protocol(format!(
                        "relate decision target resolves to incoming claim: {related_claim_id}"
                    )));
                }
                let summary = self.store.create_or_reinforce_claim(candidate).await?;
                self.store
                    .relate_claims(RelatedClaimCandidate {
                        claim_id: summary.claim_id.clone(),
                        related_claim_id,
                        relation_kind: "semantic_related".to_string(),
                        rationale: decision.rationale,
                    })
                    .await?;
                Ok(PersistedMemoryOutcome {
                    claim_id: Some(summary.claim_id),
                    outcome: "related",
                    fact_preview: fact_preview(&summary.fact),
                    sensitivity: sensitivity_label(summary.sensitivity).to_string(),
                    status: summary.status,
                })
            }
            ConsolidationDecisionKind::Supersede => {
                let existing_claim_id = decision.existing_claim_id.clone().ok_or_else(|| {
                    DaemonError::Protocol(
                        "supersede decision missing existing claim id".to_string(),
                    )
                })?;
                validate_consolidation_decision_target(
                    "supersede",
                    &existing_claim_id,
                    allowed_matches,
                )?;
                let metadata = json!({
                    "rationale": decision.rationale,
                    "confidence": decision.confidence,
                });
                let summary = self
                    .store
                    .supersede_claim(SupersedeClaimCandidate {
                        replacement: candidate,
                        superseded_claim_id: existing_claim_id,
                        metadata,
                    })
                    .await?;
                Ok(PersistedMemoryOutcome {
                    claim_id: Some(summary.claim_id),
                    outcome: "superseded",
                    fact_preview: fact_preview(&summary.fact),
                    sensitivity: sensitivity_label(summary.sensitivity).to_string(),
                    status: summary.status,
                })
            }
            ConsolidationDecisionKind::NeedsReview => {
                candidate.status = ClaimStatus::Candidate;
                let summary = self.store.create_or_reinforce_claim(candidate).await?;
                Ok(PersistedMemoryOutcome {
                    claim_id: Some(summary.claim_id),
                    outcome: "needs_review",
                    fact_preview: fact_preview(&summary.fact),
                    sensitivity: sensitivity_label(summary.sensitivity).to_string(),
                    status: summary.status,
                })
            }
        }
    }

    pub(super) async fn persist_explicit_memory_claim(
        &mut self,
        context: &ConversationMemoryContext,
        content: &str,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<ExplicitMemoryOutcome, DaemonError> {
        let write_proposal = explicit_memory_write_proposal(content, context);
        let candidate = deterministic_canonical_claim(
            &write_proposal,
            crate::ClaimStatus::Confirmed,
            Some(1.0),
            crate::EvidenceAuthority::ExplicitHumanStatement,
        );
        match self.store.create_or_reinforce_claim(candidate).await {
            Ok(summary) => {
                let claim_outcome = claim_outcome_json(&summary);
                let claim_id = summary.claim_id.clone();
                let activity = memory_activity(
                    &format!(
                        "explicit_memory_saved:{}:{}",
                        context.conversation_id, context.turn_index
                    ),
                    TurnActivityStatus::Completed,
                    "Explicit memory saved",
                    Some("saved graph claim"),
                    json!({
                        "turn_index": context.turn_index,
                        "trigger": "explicit_remember",
                        "claim_id": claim_id,
                        "claim_outcomes": [claim_outcome],
                        "created_claim_count": if summary.write_outcome == ClaimWriteOutcome::Created { 1 } else { 0 },
                        "reinforced_claim_count": if summary.write_outcome == ClaimWriteOutcome::Reinforced { 1 } else { 0 },
                        "failed_proposal_count": 0,
                        "predicate_id": summary.predicate_id,
                        "source_item_id": context.user_item_id,
                        "evidence_count": summary.evidence_count,
                        "sensitivity": sensitivity_label(summary.sensitivity),
                    }),
                );
                self.persist_and_send_turn_item(context, activity, item_tx)
                    .await?;
                Ok(ExplicitMemoryOutcome::Saved)
            }
            Err(error) => {
                let activity = memory_activity(
                    &format!(
                        "explicit_memory_failed:{}:{}",
                        context.conversation_id, context.turn_index
                    ),
                    TurnActivityStatus::Failed,
                    "Explicit memory save failed",
                    Some("graph claim write failed"),
                    json!({
                        "turn_index": context.turn_index,
                        "trigger": "explicit_remember",
                        "source_item_id": context.user_item_id,
                        "error": error.to_string(),
                    }),
                );
                self.persist_and_send_turn_item(context, activity, item_tx)
                    .await?;
                Ok(ExplicitMemoryOutcome::Failed)
            }
        }
    }
}

fn sensitivity_label(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
}

fn claim_status_label(status: ClaimStatus) -> &'static str {
    match status {
        ClaimStatus::Candidate => "candidate",
        ClaimStatus::Active => "active",
        ClaimStatus::Confirmed => "confirmed",
        ClaimStatus::Disputed => "disputed",
        ClaimStatus::Superseded => "superseded",
        ClaimStatus::Archived => "archived",
        ClaimStatus::Deleted => "deleted",
    }
}

fn provider_memory_claim_activity(
    saved_count: usize,
    failed_count: usize,
    first_error: Option<&str>,
) -> (TurnActivityStatus, &'static str, String) {
    if failed_count == 0 {
        return (
            TurnActivityStatus::Completed,
            "Memory persisted",
            provider_memory_claim_summary(saved_count, failed_count),
        );
    }

    if saved_count == 0 {
        let summary = if failed_count == 1 {
            provider_memory_single_failure_summary(first_error)
        } else {
            provider_memory_claim_summary(saved_count, failed_count)
        };
        return (
            TurnActivityStatus::Failed,
            "Memory persistence failed",
            summary,
        );
    }

    (
        TurnActivityStatus::Failed,
        "Memory persistence partially failed",
        provider_memory_claim_summary(saved_count, failed_count),
    )
}

fn provider_memory_single_failure_summary(error: Option<&str>) -> String {
    if error.is_some_and(|error| error.contains("memory canonicalization failed")) {
        "memory canonicalization failed".to_string()
    } else {
        "graph claim write failed".to_string()
    }
}

fn claim_outcome_json(summary: &crate::ClaimSummary) -> Value {
    json!({
        "claim_id": summary.claim_id,
        "outcome": claim_write_outcome_label(summary.write_outcome),
        "fact_preview": fact_preview(&summary.fact),
        "sensitivity": sensitivity_label(summary.sensitivity),
    })
}

fn validate_consolidation_decision_target<'a>(
    decision_kind: &str,
    existing_claim_id: &str,
    allowed_matches: &'a [ConsolidationMatch],
) -> Result<&'a ConsolidationMatch, DaemonError> {
    if let Some(selected_match) = allowed_matches
        .iter()
        .find(|item| item.claim_id == existing_claim_id)
    {
        return Ok(selected_match);
    }
    Err(DaemonError::Protocol(format!(
        "{decision_kind} decision target is not in consolidation match set: {existing_claim_id}"
    )))
}

#[derive(Debug, Clone)]
pub(super) struct PersistedMemoryOutcome {
    claim_id: Option<String>,
    outcome: &'static str,
    fact_preview: String,
    sensitivity: String,
    status: ClaimStatus,
}

impl PersistedMemoryOutcome {
    fn from_claim_summary(summary: crate::ClaimSummary) -> Self {
        Self {
            claim_id: Some(summary.claim_id),
            outcome: claim_write_outcome_label(summary.write_outcome),
            fact_preview: fact_preview(&summary.fact),
            sensitivity: sensitivity_label(summary.sensitivity).to_string(),
            status: summary.status,
        }
    }
}

fn claim_write_outcome_label(outcome: ClaimWriteOutcome) -> &'static str {
    match outcome {
        ClaimWriteOutcome::Created => "created",
        ClaimWriteOutcome::Reinforced => "reinforced",
    }
}

fn fact_preview(fact: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 120;
    let trimmed = fact.trim();
    if trimmed.chars().count() <= MAX_PREVIEW_CHARS {
        return trimmed.to_string();
    }
    let preview = trimmed
        .chars()
        .take(MAX_PREVIEW_CHARS - 3)
        .collect::<String>();
    format!("{preview}...")
}

const fn saved_claim_counts_as_active(status: crate::ClaimStatus) -> bool {
    matches!(
        status,
        crate::ClaimStatus::Active | crate::ClaimStatus::Confirmed
    )
}

fn provider_memory_claim_summary(saved_count: usize, failed_count: usize) -> String {
    match (saved_count, failed_count) {
        (1, 0) => "saved 1 graph claim".to_string(),
        (count, 0) => format!("saved {count} graph claims"),
        (0, 1) => "saved 0 graph claims; 1 proposal failed".to_string(),
        (0, failed) => format!("saved 0 graph claims; {failed} proposals failed"),
        (1, 1) => "saved 1 graph claim; 1 proposal failed".to_string(),
        (1, failed) => format!("saved 1 graph claim; {failed} proposals failed"),
        (saved, 1) => format!("saved {saved} graph claims; 1 proposal failed"),
        (saved, failed) => format!("saved {saved} graph claims; {failed} proposals failed"),
    }
}

fn provider_memory_review_summary(
    predicate_proposal_count: usize,
    review_claim_count: usize,
    failed_count: usize,
) -> String {
    let reviewed = predicate_proposal_count + review_claim_count;
    let reviewed_label = match (predicate_proposal_count, review_claim_count) {
        (0, 1) => "stored 1 graph claim for review".to_string(),
        (0, count) => format!("stored {count} graph claims for review"),
        (1, 0) => "stored 1 predicate proposal for review".to_string(),
        (count, 0) => format!("stored {count} predicate proposals for review"),
        (1, 1) => "stored 1 graph claim and 1 predicate proposal for review".to_string(),
        (predicates, 1) => {
            format!("stored 1 graph claim and {predicates} predicate proposals for review")
        }
        (1, claims) => {
            format!("stored {claims} graph claims and 1 predicate proposal for review")
        }
        (predicates, claims) => {
            format!("stored {claims} graph claims and {predicates} predicate proposals for review")
        }
    };

    match (reviewed, failed_count) {
        (_, 0) => reviewed_label,
        (_, 1) => format!("{reviewed_label}; 1 proposal failed"),
        (_, failed) => format!("{reviewed_label}; {failed} proposals failed"),
    }
}
