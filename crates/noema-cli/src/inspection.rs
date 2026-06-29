//! CLI inspection commands for local memory and context state.
#![allow(
    dead_code,
    reason = "context graph inspection remains disabled, but its formatter is test-covered for a future graph-inspection slice"
)]

use clap::Subcommand;
pub(crate) use context_graph_output::ContextGraphFormat;
#[cfg(test)]
use context_graph_output::write_context_graph;
use noema_core::memory::{
    Effect, ExternalEgressPolicy, MemoryStatus, ParticipantRole, ParticipantVisibilityPolicy,
    Purpose, RelationshipStatus, RetrievalPolicyStatus, Sensitivity, SubjectRole,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    io::{self, Write},
    path::PathBuf,
};

use crate::{
    CliError,
    graphql_client::{self, GraphqlRequest},
};

#[allow(
    dead_code,
    reason = "context graph inspection remains unavailable while formatter tests preserve behavior"
)]
mod context_graph_output;
#[allow(
    dead_code,
    reason = "context graph inspection remains unavailable while formatter tests preserve behavior"
)]
mod context_graph_text;

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
}

#[derive(Debug, Subcommand)]
pub(crate) enum ContextCommand {
    #[command(about = "Inspect the persisted context graph.")]
    Graph {
        #[arg(long, default_value_t = 50, help = "Maximum rows per graph section.")]
        limit: u32,
        #[arg(long, help = "Only inspect context packets for this run id.")]
        run_id: Option<String>,
        #[arg(long = "packet-id", help = "Only inspect this context packet id.")]
        context_packet_id: Option<String>,
        #[arg(
            long,
            value_enum,
            default_value_t = ContextGraphFormat::Text,
            help = "Output format for the graph."
        )]
        format: ContextGraphFormat,
    },
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
    }
}

pub(crate) async fn run_context(
    command: &ContextCommand,
    _config_path: Option<PathBuf>,
) -> Result<(), CliError> {
    match command {
        ContextCommand::Graph { .. } => Err(context_inspection_unavailable()),
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

fn claim_list_fact(claim: &GraphqlMemoryClaim) -> String {
    if claim.fact_redacted {
        claim.fact.clone()
    } else {
        preview(&claim.fact, 96)
    }
}

fn redacted_json(sensitivity: Sensitivity, value: &str, max_chars: usize) -> String {
    match sensitivity {
        Sensitivity::Public => preview(value, max_chars),
        Sensitivity::Normal
        | Sensitivity::Private
        | Sensitivity::Sensitive
        | Sensitivity::Secret => "[redacted; use memory show <id>]".to_string(),
    }
}

fn redacted_event_details(
    memory_sensitivity: Option<Sensitivity>,
    details: &str,
    max_chars: usize,
) -> String {
    match memory_sensitivity {
        Some(Sensitivity::Public) => preview(details, max_chars),
        None => "[redacted; scope-level event details hidden]".to_string(),
        Some(
            Sensitivity::Normal
            | Sensitivity::Private
            | Sensitivity::Sensitive
            | Sensitivity::Secret,
        ) => "[redacted; use memory show <id>]".to_string(),
    }
}

fn object_ref(object_type: &str, object_id: &str) -> String {
    format!("{object_type}:{object_id}")
}

fn optional_object_ref(object_type: Option<&str>, object_id: Option<&str>) -> String {
    match (object_type, object_id) {
        (Some(object_type), Some(object_id)) => object_ref(object_type, object_id),
        _ => "-".to_string(),
    }
}

fn use_record_object(object_type: Option<&str>, object_id: Option<&str>) -> String {
    match (object_type, object_id) {
        (Some(object_type), Some(object_id)) => format!("{object_type}:{object_id}"),
        (Some(object_type), None) => object_type.to_string(),
        (None, Some(object_id)) => object_id.to_string(),
        (None, None) => "-".to_string(),
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

fn retrieval_policy_status_label(status: RetrievalPolicyStatus) -> &'static str {
    match status {
        RetrievalPolicyStatus::Valid => "valid",
        RetrievalPolicyStatus::Stale => "stale",
        RetrievalPolicyStatus::Invalid => "invalid",
        RetrievalPolicyStatus::NeedsReview => "needs_review",
    }
}

fn participant_visibility_policy_label(policy: ParticipantVisibilityPolicy) -> &'static str {
    match policy {
        ParticipantVisibilityPolicy::AnyActiveHuman => "any_active_human",
        ParticipantVisibilityPolicy::AllOriginalHumans => "all_original_humans",
        ParticipantVisibilityPolicy::OwnerOnly => "owner_only",
        ParticipantVisibilityPolicy::ExplicitGrantOnly => "explicit_grant_only",
    }
}

fn external_egress_policy_label(policy: ExternalEgressPolicy) -> &'static str {
    match policy {
        ExternalEgressPolicy::Allow => "allow",
        ExternalEgressPolicy::ApprovalRequired => "approval_required",
        ExternalEgressPolicy::Deny => "deny",
    }
}

fn purpose_label(purpose: Purpose) -> &'static str {
    match purpose {
        Purpose::AnswerHumanQuestion => "answer_human_question",
        Purpose::DraftInternalContent => "draft_internal_content",
        Purpose::GeneralPersonalization => "general_personalization",
        Purpose::ManageTask => "manage_task",
        Purpose::ManageCalendar => "manage_calendar",
        Purpose::DraftExternalContent => "draft_external_content",
        Purpose::UseTool => "use_tool",
        Purpose::ProactiveSuggestion => "proactive_suggestion",
        Purpose::ExternalAction => "external_action",
        Purpose::DebugAudit => "debug_audit",
    }
}

fn effect_label(effect: Effect) -> &'static str {
    match effect {
        Effect::Allow => "allow",
        Effect::Deny => "deny",
    }
}

fn memory_status_label(status: MemoryStatus) -> &'static str {
    match status {
        MemoryStatus::Candidate => "candidate",
        MemoryStatus::Active => "active",
        MemoryStatus::Confirmed => "confirmed",
        MemoryStatus::Inferred => "inferred",
        MemoryStatus::Stale => "stale",
        MemoryStatus::Superseded => "superseded",
        MemoryStatus::Archived => "archived",
        MemoryStatus::Deleted => "deleted",
        MemoryStatus::Disputed => "disputed",
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

fn relationship_status_label(status: RelationshipStatus) -> &'static str {
    match status {
        RelationshipStatus::Candidate => "candidate",
        RelationshipStatus::Active => "active",
        RelationshipStatus::Confirmed => "confirmed",
        RelationshipStatus::Superseded => "superseded",
        RelationshipStatus::Archived => "archived",
        RelationshipStatus::Deleted => "deleted",
        RelationshipStatus::Disputed => "disputed",
    }
}

fn participant_role_label(role: ParticipantRole) -> &'static str {
    match role {
        ParticipantRole::HumanInScope => "human_in_scope",
        ParticipantRole::AgentInScope => "agent_in_scope",
        ParticipantRole::Originator => "originator",
        ParticipantRole::Observer => "observer",
    }
}

fn subject_role_label(role: SubjectRole) -> &'static str {
    match role {
        SubjectRole::About => "about",
        SubjectRole::Claimant => "claimant",
        SubjectRole::Affected => "affected",
        SubjectRole::Owner => "owner",
        SubjectRole::Assignee => "assignee",
        SubjectRole::Source => "source",
        SubjectRole::Target => "target",
    }
}

#[cfg(test)]
#[path = "inspection_tests.rs"]
mod inspection_tests;
