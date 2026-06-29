//! CLI inspection commands for local memory and context state.

use clap::Subcommand;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    path::PathBuf,
};

use crate::{
    CliError,
    graphql_client::{self, GraphqlRequest},
};

#[derive(Debug, Subcommand)]
pub(crate) enum MemoryCommand {
    #[command(about = "List recent local memories.")]
    List {
        #[arg(long, default_value_t = 20, help = "Maximum memories to show.")]
        limit: u32,
        #[arg(long, help = "Text query matched against claim facts and labels.")]
        query: Option<String>,
        #[arg(long, help = "Only show claims with this lifecycle status.")]
        status: Option<String>,
        #[arg(
            long = "predicate-id",
            help = "Only show claims with this predicate id."
        )]
        predicate_id: Option<String>,
    },
    #[command(about = "Show one local memory.")]
    Show {
        #[arg(value_name = "MEMORY_ID")]
        memory_id: String,
    },
    #[command(about = "List predicate proposals awaiting memory review.")]
    PredicateProposals {
        #[arg(
            long,
            default_value_t = 20,
            help = "Maximum predicate proposals to show."
        )]
        limit: u32,
        #[arg(long, help = "Only show proposals with this lifecycle status.")]
        status: Option<String>,
    },
    #[command(about = "Show one predicate proposal.")]
    PredicateProposal {
        #[arg(value_name = "PROPOSAL_ID")]
        proposal_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ContextCommand {
    #[command(about = "Context graph inspection is not available yet.")]
    Graph,
}

pub(crate) async fn run_memory(
    command: &MemoryCommand,
    graphql_base_url: &str,
) -> Result<(), CliError> {
    match command {
        MemoryCommand::List {
            limit,
            query,
            status,
            predicate_id,
        } => {
            let data = graphql_client::execute::<MemoryClaimsData>(
                graphql_base_url,
                GraphqlRequest::new(
                    MEMORY_CLAIMS_QUERY,
                    json!({
                        "query": query,
                        "status": status,
                        "predicateId": predicate_id,
                        "limit": i32::try_from(*limit).unwrap_or(i32::MAX),
                    }),
                ),
            )
            .await
            .map_err(CliError::Graphql)?;
            let mut stdout = io::stdout();
            write_memory_claim_list(&mut stdout, &data.memory_claims).map_err(CliError::WriteOutput)
        }
        MemoryCommand::Show { memory_id } => {
            let data = graphql_client::execute::<MemoryClaimData>(
                graphql_base_url,
                GraphqlRequest::new(
                    MEMORY_CLAIM_QUERY,
                    json!({
                        "claimId": memory_id,
                    }),
                ),
            )
            .await
            .map_err(CliError::Graphql)?;
            let Some(claim) = data.memory_claim else {
                return Err(CliError::Unavailable(format!(
                    "memory claim not found: {memory_id}"
                )));
            };
            let mut stdout = io::stdout();
            write_memory_claim_detail(&mut stdout, &claim).map_err(CliError::WriteOutput)
        }
        MemoryCommand::PredicateProposals { limit, status } => {
            let data = graphql_client::execute::<PredicateProposalsData>(
                graphql_base_url,
                GraphqlRequest::new(
                    PREDICATE_PROPOSALS_QUERY,
                    json!({
                        "status": status,
                        "limit": i32::try_from(*limit).unwrap_or(i32::MAX),
                    }),
                ),
            )
            .await
            .map_err(CliError::Graphql)?;
            let mut stdout = io::stdout();
            write_predicate_proposal_list(&mut stdout, &data.memory_predicate_proposals)
                .map_err(CliError::WriteOutput)
        }
        MemoryCommand::PredicateProposal { proposal_id } => {
            let data = graphql_client::execute::<PredicateProposalData>(
                graphql_base_url,
                GraphqlRequest::new(
                    PREDICATE_PROPOSAL_QUERY,
                    json!({
                        "proposalId": proposal_id,
                    }),
                ),
            )
            .await
            .map_err(CliError::Graphql)?;
            let Some(proposal) = data.memory_predicate_proposal else {
                return Err(CliError::Unavailable(format!(
                    "predicate proposal not found: {proposal_id}"
                )));
            };
            let mut stdout = io::stdout();
            write_predicate_proposal_detail(&mut stdout, &proposal).map_err(CliError::WriteOutput)
        }
    }
}

pub(crate) async fn run_context(
    command: &ContextCommand,
    _config_path: Option<PathBuf>,
) -> Result<(), CliError> {
    match command {
        ContextCommand::Graph => Err(context_inspection_unavailable()),
    }
}

const MEMORY_CLAIMS_QUERY: &str = r#"
query CliMemoryClaims($query: String, $status: String, $predicateId: String, $limit: Int) {
  memoryClaims(query: $query, status: $status, predicateId: $predicateId, limit: $limit) {
    claimId
    fact
    factRedacted
    predicateId
    predicateLabel
    subjectEntityId
    subjectEntityName
    subjectEntityType
    objectEntityId
    objectEntityName
    objectEntityType
    status
    sensitivity
    confidence
    evidenceCount
    createdAt
    updatedAt
  }
}
"#;

const MEMORY_CLAIM_QUERY: &str = r#"
query CliMemoryClaim($claimId: String!) {
  memoryClaim(claimId: $claimId) {
    claimId
    fact
    predicateId
    predicateLabel
    subjectEntityId
    subjectEntityName
    subjectEntityType
    objectEntityId
    objectEntityName
    objectEntityType
    status
    sensitivity
    confidence
    evidenceCount
    createdAt
    updatedAt
    evidence {
      evidenceId
      sourceItemId
      authority
      excerpt
      observedAt
      createdAt
    }
  }
}
"#;

const PREDICATE_PROPOSALS_QUERY: &str = r#"
query CliPredicateProposals($status: String, $limit: Int) {
  memoryPredicateProposals(status: $status, limit: $limit) {
    proposalId
    label
    description
    status
    sourceItemId
    createdAt
    updatedAt
  }
}
"#;

const PREDICATE_PROPOSAL_QUERY: &str = r#"
query CliPredicateProposal($proposalId: String!) {
  memoryPredicateProposal(proposalId: $proposalId) {
    proposalId
    label
    description
    proposedPredicate
    proposedClaim
    status
    sourceItemId
    createdAt
    updatedAt
  }
}
"#;

#[derive(Debug, Deserialize)]
struct MemoryClaimsData {
    #[serde(rename = "memoryClaims")]
    memory_claims: Vec<GraphqlMemoryClaim>,
}

#[derive(Debug, Deserialize)]
struct MemoryClaimData {
    #[serde(rename = "memoryClaim")]
    memory_claim: Option<GraphqlMemoryClaimDetail>,
}

#[derive(Debug, Deserialize)]
struct PredicateProposalsData {
    #[serde(rename = "memoryPredicateProposals")]
    memory_predicate_proposals: Vec<GraphqlPredicateProposalSummary>,
}

#[derive(Debug, Deserialize)]
struct PredicateProposalData {
    #[serde(rename = "memoryPredicateProposal")]
    memory_predicate_proposal: Option<GraphqlPredicateProposal>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct GraphqlMemoryClaim {
    #[serde(rename = "claimId")]
    claim_id: String,
    fact: String,
    #[serde(rename = "factRedacted")]
    fact_redacted: bool,
    #[serde(rename = "predicateId")]
    predicate_id: String,
    #[serde(rename = "predicateLabel")]
    predicate_label: String,
    #[serde(rename = "subjectEntityId")]
    subject_entity_id: String,
    #[serde(rename = "subjectEntityName")]
    subject_entity_name: String,
    #[serde(rename = "subjectEntityType")]
    subject_entity_type: String,
    #[serde(rename = "objectEntityId")]
    object_entity_id: Option<String>,
    #[serde(rename = "objectEntityName")]
    object_entity_name: Option<String>,
    #[serde(rename = "objectEntityType")]
    object_entity_type: Option<String>,
    status: String,
    sensitivity: String,
    confidence: Option<f64>,
    #[serde(rename = "evidenceCount")]
    evidence_count: i64,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct GraphqlMemoryClaimDetail {
    #[serde(rename = "claimId")]
    claim_id: String,
    fact: String,
    #[serde(rename = "predicateId")]
    predicate_id: String,
    #[serde(rename = "predicateLabel")]
    predicate_label: String,
    #[serde(rename = "subjectEntityId")]
    subject_entity_id: String,
    #[serde(rename = "subjectEntityName")]
    subject_entity_name: String,
    #[serde(rename = "subjectEntityType")]
    subject_entity_type: String,
    #[serde(rename = "objectEntityId")]
    object_entity_id: Option<String>,
    #[serde(rename = "objectEntityName")]
    object_entity_name: Option<String>,
    #[serde(rename = "objectEntityType")]
    object_entity_type: Option<String>,
    status: String,
    sensitivity: String,
    confidence: Option<f64>,
    #[serde(rename = "evidenceCount")]
    evidence_count: i64,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    evidence: Vec<GraphqlMemoryClaimEvidence>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct GraphqlMemoryClaimEvidence {
    #[serde(rename = "evidenceId")]
    evidence_id: Option<String>,
    #[serde(rename = "sourceItemId")]
    source_item_id: Option<String>,
    authority: String,
    excerpt: Option<String>,
    #[serde(rename = "observedAt")]
    observed_at: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct GraphqlPredicateProposal {
    #[serde(rename = "proposalId")]
    proposal_id: String,
    label: String,
    description: String,
    #[serde(rename = "proposedPredicate")]
    proposed_predicate: Value,
    #[serde(rename = "proposedClaim")]
    proposed_claim: Value,
    status: String,
    #[serde(rename = "sourceItemId")]
    source_item_id: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct GraphqlPredicateProposalSummary {
    #[serde(rename = "proposalId")]
    proposal_id: String,
    label: String,
    description: String,
    status: String,
    #[serde(rename = "sourceItemId")]
    source_item_id: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

fn context_inspection_unavailable() -> CliError {
    CliError::Unavailable(
        "context graph inspection is unavailable until graph retrieval lands".to_string(),
    )
}

fn write_memory_claim_list<W: Write>(
    writer: &mut W,
    claims: &[GraphqlMemoryClaim],
) -> io::Result<()> {
    if claims.is_empty() {
        writeln!(writer, "No memory claims found.")?;
        return Ok(());
    }

    writeln!(
        writer,
        "{:<38}  {:<10}  {:<10}  {:<18}  {:<24}  Fact",
        "ID", "Status", "Privacy", "Predicate", "Created"
    )?;

    for claim in claims {
        writeln!(
            writer,
            "{:<38}  {:<10}  {:<10}  {:<18}  {:<24}  {}",
            claim.claim_id,
            claim.status,
            claim.sensitivity,
            preview(&claim.predicate_label, 18),
            claim.created_at,
            claim_list_fact(claim),
        )?;
    }

    Ok(())
}

fn write_memory_claim_detail<W: Write>(
    writer: &mut W,
    claim: &GraphqlMemoryClaimDetail,
) -> io::Result<()> {
    writeln!(writer, "ID: {}", claim.claim_id)?;
    writeln!(writer, "Status: {}", claim.status)?;
    writeln!(writer, "Sensitivity: {}", claim.sensitivity)?;
    writeln!(
        writer,
        "Predicate: {} ({})",
        claim.predicate_label, claim.predicate_id
    )?;
    writeln!(
        writer,
        "Subject: {} ({}, {})",
        claim.subject_entity_name, claim.subject_entity_type, claim.subject_entity_id
    )?;
    if let Some(object_id) = &claim.object_entity_id {
        writeln!(
            writer,
            "Object: {} ({}, {})",
            claim.object_entity_name.as_deref().unwrap_or("-"),
            claim.object_entity_type.as_deref().unwrap_or("-"),
            object_id
        )?;
    }
    if let Some(confidence) = claim.confidence {
        writeln!(writer, "Confidence: {confidence:.3}")?;
    }
    writeln!(writer, "Evidence count: {}", claim.evidence_count)?;
    writeln!(writer, "Created: {}", claim.created_at)?;
    writeln!(writer, "Updated: {}", claim.updated_at)?;
    writeln!(writer, "Fact: {}", claim.fact)?;

    if !claim.evidence.is_empty() {
        writeln!(writer)?;
        writeln!(writer, "Evidence")?;
        for evidence in &claim.evidence {
            writeln!(
                writer,
                "- {} source={} observed={} created={}",
                evidence.authority,
                evidence.source_item_id.as_deref().unwrap_or("-"),
                evidence.observed_at.as_deref().unwrap_or("-"),
                evidence.created_at
            )?;
            if let Some(evidence_id) = &evidence.evidence_id {
                writeln!(writer, "  id: {evidence_id}")?;
            }
            if let Some(excerpt) = &evidence.excerpt {
                writeln!(writer, "  excerpt: {excerpt}")?;
            }
        }
    }

    Ok(())
}

fn write_predicate_proposal_list<W: Write>(
    writer: &mut W,
    proposals: &[GraphqlPredicateProposalSummary],
) -> io::Result<()> {
    if proposals.is_empty() {
        writeln!(writer, "No predicate proposals found.")?;
        return Ok(());
    }

    writeln!(
        writer,
        "{:<38}  {:<10}  {:<22}  {:<24}  Description",
        "ID", "Status", "Label", "Created"
    )?;

    for proposal in proposals {
        writeln!(
            writer,
            "{:<38}  {:<10}  {:<22}  {:<24}  {}",
            proposal.proposal_id,
            proposal.status,
            preview(&proposal.label, 22),
            proposal.created_at,
            preview(&proposal.description, 96),
        )?;
    }

    Ok(())
}

fn write_predicate_proposal_detail<W: Write>(
    writer: &mut W,
    proposal: &GraphqlPredicateProposal,
) -> io::Result<()> {
    writeln!(writer, "ID: {}", proposal.proposal_id)?;
    writeln!(writer, "Status: {}", proposal.status)?;
    writeln!(writer, "Label: {}", proposal.label)?;
    writeln!(writer, "Description: {}", proposal.description)?;
    writeln!(
        writer,
        "Source: {}",
        proposal.source_item_id.as_deref().unwrap_or("-")
    )?;
    writeln!(writer, "Created: {}", proposal.created_at)?;
    writeln!(writer, "Updated: {}", proposal.updated_at)?;
    writeln!(writer)?;
    writeln!(writer, "Proposed predicate")?;
    writeln!(writer, "{}", pretty_json(&proposal.proposed_predicate))?;
    writeln!(writer)?;
    writeln!(writer, "Proposed claim")?;
    writeln!(writer, "{}", pretty_json(&proposal.proposed_claim))?;

    Ok(())
}

fn claim_list_fact(claim: &GraphqlMemoryClaim) -> String {
    if claim.fact_redacted {
        claim.fact.clone()
    } else {
        preview(&claim.fact, 96)
    }
}

fn preview(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let mut preview: String = trimmed.chars().take(max_chars).collect();
    if trimmed.chars().count() > max_chars {
        preview.push_str("...");
    }
    preview
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
#[path = "inspection_tests.rs"]
mod inspection_tests;
