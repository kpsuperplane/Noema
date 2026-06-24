//! CLI inspection commands for local memory and context state.

use clap::Subcommand;
use noema_core::{
    ContextGraphSummary, NoemaPaths, SqliteMemoryRepository,
    memory::{
        Effect, ExternalEgressPolicy, MemoryStatus, ParticipantRole, ParticipantVisibilityPolicy,
        Purpose, RelationshipStatus, RetrievalPolicyStatus, Sensitivity, SubjectRole,
    },
    memory_persistence::{MemoryPersistenceError, MemorySummary},
};
use std::io::{self, Write};

use crate::CliError;

#[derive(Debug, Subcommand)]
pub(crate) enum MemoryCommand {
    #[command(about = "List recent local memories.")]
    List {
        #[arg(long, default_value_t = 20, help = "Maximum memories to show.")]
        limit: u32,
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
    },
}

pub(crate) fn run_memory(command: &MemoryCommand) -> Result<(), CliError> {
    let paths = NoemaPaths::from_process_env()?;
    let repo = match SqliteMemoryRepository::open_existing_readonly(&paths) {
        Ok(repo) => repo,
        Err(MemoryPersistenceError::MissingDatabase { path }) => {
            println!("No memory database found at {}", path.display());
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    match command {
        MemoryCommand::List { limit } => {
            let memories = repo.list_recent_memories(Some(*limit))?;
            print_memory_list(&memories)?;
        }
        MemoryCommand::Show { memory_id } => {
            let memory = repo
                .get_memory(memory_id)?
                .ok_or_else(|| CliError::MemoryNotFound(memory_id.clone()))?;
            print_memory_detail(&memory)?;
        }
    }

    Ok(())
}

pub(crate) fn run_context(command: &ContextCommand) -> Result<(), CliError> {
    let paths = NoemaPaths::from_process_env()?;
    let repo = match SqliteMemoryRepository::open_existing_readonly(&paths) {
        Ok(repo) => repo,
        Err(MemoryPersistenceError::MissingDatabase { path }) => {
            println!("No memory database found at {}", path.display());
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    match command {
        ContextCommand::Graph { limit } => {
            let graph = repo.inspect_context_graph(Some(*limit))?;
            print_context_graph(&graph)?;
        }
    }

    Ok(())
}

fn print_context_graph(graph: &ContextGraphSummary) -> Result<(), CliError> {
    let mut stdout = io::stdout();
    write_context_graph(&mut stdout, graph)
}

fn write_context_graph(
    stdout: &mut impl Write,
    graph: &ContextGraphSummary,
) -> Result<(), CliError> {
    writeln!(stdout, "Context graph").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "memories={} entities={} subject_edges={} participant_edges={} provenance_edges={} object_link_edges={} purpose_rules={} access_grants={} memory_events={} relationships={}",
        graph.memories.len(),
        graph.entities.len(),
        graph.subject_edges.len(),
        graph.participant_edges.len(),
        graph.provenance_edges.len(),
        graph.object_link_edges.len(),
        graph.purpose_rules.len(),
        graph.access_grants.len(),
        graph.memory_events.len(),
        graph.relationships.len(),
    )
    .map_err(CliError::WriteOutput)?;

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Memories").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<10}  {:<10}  {:<10}  {:<12}  {:<12}  {:<24}  Title",
        "ID", "Status", "Type", "Privacy", "Stored", "Effective", "Scope"
    )
    .map_err(CliError::WriteOutput)?;
    for memory in &graph.memories {
        writeln!(
            stdout,
            "{:<38}  {:<10}  {:<10}  {:<10}  {:<12}  {:<12}  {:<24}  {}",
            memory.memory_id,
            memory_status_label(memory.status),
            memory.memory_type.as_str(),
            sensitivity_label(memory.sensitivity),
            retrieval_policy_status_label(memory.retrieval_policy_status),
            retrieval_policy_status_label(memory.retrieval_policy_effective_status),
            memory.home_scope_id,
            redacted_title(memory.sensitivity, &memory.title, 80),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Retrieval policy").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<4}  {:<20}  {:<17}  {:<24}  {:<12}  {:<14}  {:<24}  Hints",
        "Memory",
        "Ver",
        "Participant visibility",
        "Egress",
        "Extractor",
        "Extractor v",
        "Fingerprint",
        "Validated"
    )
    .map_err(CliError::WriteOutput)?;
    for memory in &graph.memories {
        let fingerprint = memory
            .retrieval_policy_fingerprint
            .as_deref()
            .map(|value| preview(value, 14))
            .unwrap_or_else(|| "-".to_string());
        writeln!(
            stdout,
            "{:<38}  {:<4}  {:<20}  {:<17}  {:<24}  {:<12}  {:<14}  {:<24}  {}",
            memory.memory_id,
            memory.retrieval_policy_version,
            participant_visibility_policy_label(memory.participant_visibility_policy),
            external_egress_policy_label(memory.external_egress_policy),
            memory
                .retrieval_policy_extractor_principal_id
                .as_deref()
                .unwrap_or("-"),
            memory
                .retrieval_policy_extractor_version
                .as_deref()
                .unwrap_or("-"),
            fingerprint,
            memory
                .retrieval_policy_validated_at
                .as_deref()
                .unwrap_or("-"),
            redacted_json(memory.sensitivity, &memory.retrieval_hints, 64),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Entities").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<14}  {:<24}  {:<24}  Name",
        "ID", "Type", "Scope", "Principal"
    )
    .map_err(CliError::WriteOutput)?;
    for entity in &graph.entities {
        writeln!(
            stdout,
            "{:<38}  {:<14}  {:<24}  {:<24}  {}",
            entity.entity_id,
            entity.entity_type,
            entity.home_scope_id.as_deref().unwrap_or("-"),
            entity.linked_principal_id.as_deref().unwrap_or("-"),
            entity.canonical_name,
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Subject edges").map_err(CliError::WriteOutput)?;
    writeln!(stdout, "{:<38}  {:<38}  Role", "Memory", "Entity").map_err(CliError::WriteOutput)?;
    for edge in &graph.subject_edges {
        writeln!(
            stdout,
            "{:<38}  {:<38}  {}",
            edge.memory_id,
            edge.entity_id,
            subject_role_label(edge.role),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Participant edges").map_err(CliError::WriteOutput)?;
    writeln!(stdout, "{:<38}  {:<28}  Role", "Memory", "Principal")
        .map_err(CliError::WriteOutput)?;
    for edge in &graph.participant_edges {
        writeln!(
            stdout,
            "{:<38}  {:<28}  {}",
            edge.memory_id,
            edge.principal_id,
            participant_role_label(edge.role),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Provenance edges").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<10}  {:<38}  Relation",
        "Memory", "Source", "Source ID"
    )
    .map_err(CliError::WriteOutput)?;
    for edge in &graph.provenance_edges {
        writeln!(
            stdout,
            "{:<38}  {:<10}  {:<38}  {}",
            edge.memory_id, edge.source_type, edge.source_id, edge.relation,
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Trusted object links").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<18}  {:<16}  {:<38}  {:<24}  Resolver",
        "Memory", "Relation", "Object type", "Object ID", "Authorized scope"
    )
    .map_err(CliError::WriteOutput)?;
    for edge in &graph.object_link_edges {
        writeln!(
            stdout,
            "{:<38}  {:<18}  {:<16}  {:<38}  {:<24}  {}",
            edge.memory_id,
            edge.relation,
            edge.object_type,
            edge.object_id,
            edge.authorized_scope_id.as_deref().unwrap_or("-"),
            edge.resolver_principal_id.as_deref().unwrap_or("-"),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Purpose rules").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<24}  {:<6}  Created by",
        "Memory", "Purpose", "Effect"
    )
    .map_err(CliError::WriteOutput)?;
    for rule in &graph.purpose_rules {
        writeln!(
            stdout,
            "{:<38}  {:<24}  {:<6}  {}",
            rule.memory_id,
            purpose_label(rule.purpose),
            effect_label(rule.effect),
            rule.created_by_principal_id.as_deref().unwrap_or("-"),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Access grants").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<38}  {:<28}  {:<24}  {:<6}  Expires",
        "Grant", "Target", "Principal", "Permission", "Effect"
    )
    .map_err(CliError::WriteOutput)?;
    for grant in &graph.access_grants {
        writeln!(
            stdout,
            "{:<38}  {:<38}  {:<28}  {:<24}  {:<6}  {}",
            grant.grant_id,
            grant_target(grant.memory_id.as_deref(), grant.scope_id.as_deref()),
            grant.principal_id,
            grant.permission,
            effect_label(grant.effect),
            grant.expires_at.as_deref().unwrap_or("-"),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Memory events").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<16}  {:<28}  {:<38}  {:<24}  Reason",
        "Event", "Type", "Actor", "Memory", "Scope"
    )
    .map_err(CliError::WriteOutput)?;
    for event in &graph.memory_events {
        writeln!(
            stdout,
            "{:<38}  {:<16}  {:<28}  {:<38}  {:<24}  {}",
            event.event_id,
            event.event_type,
            event.actor_principal_id.as_deref().unwrap_or("-"),
            event.memory_id.as_deref().unwrap_or("-"),
            event.scope_id.as_deref().unwrap_or("-"),
            event.reason.as_deref().unwrap_or("-"),
        )
        .map_err(CliError::WriteOutput)?;
        writeln!(
            stdout,
            "{:<38}  details: {}",
            "",
            redacted_event_details(event.memory_sensitivity, &event.details, 96),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Relationship claims").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<10}  {:<24}  {:<18}  {:<24}  Memory",
        "ID", "Status", "Subject", "Predicate", "Object"
    )
    .map_err(CliError::WriteOutput)?;
    for relationship in &graph.relationships {
        writeln!(
            stdout,
            "{:<38}  {:<10}  {:<24}  {:<18}  {:<24}  {}",
            relationship.relationship_id,
            relationship_status_label(relationship.status),
            relationship
                .subject_name
                .as_deref()
                .unwrap_or(&relationship.subject_entity_id),
            relationship.predicate,
            relationship
                .object_name
                .as_deref()
                .unwrap_or(&relationship.object_entity_id),
            relationship.memory_id.as_deref().unwrap_or("-"),
        )
        .map_err(CliError::WriteOutput)?;
    }

    Ok(())
}

fn print_memory_list(memories: &[MemorySummary]) -> Result<(), CliError> {
    if memories.is_empty() {
        println!("No memories found.");
        return Ok(());
    }

    let mut stdout = io::stdout();
    writeln!(
        stdout,
        "{:<38}  {:<10}  {:<10}  {:<10}  {:<24}  Title",
        "ID", "Status", "Type", "Privacy", "Created"
    )
    .map_err(CliError::WriteOutput)?;

    for memory in memories {
        writeln!(
            stdout,
            "{:<38}  {:<10}  {:<10}  {:<10}  {:<24}  {}",
            memory.id,
            memory_status_label(memory.status),
            memory.memory_type.as_str(),
            sensitivity_label(memory.sensitivity),
            memory.created_at,
            redacted_list_title(memory),
        )
        .map_err(CliError::WriteOutput)?;
    }

    Ok(())
}

fn print_memory_detail(memory: &MemorySummary) -> Result<(), CliError> {
    let mut stdout = io::stdout();
    writeln!(stdout, "ID: {}", memory.id).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Status: {}", memory_status_label(memory.status))
        .map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Type: {}", memory.memory_type.as_str()).map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "Sensitivity: {}",
        sensitivity_label(memory.sensitivity)
    )
    .map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Home scope: {}", memory.home_scope_id).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Created: {}", memory.created_at).map_err(CliError::WriteOutput)?;
    if let Some(conversation_id) = &memory.conversation_id {
        writeln!(stdout, "Conversation: {conversation_id}").map_err(CliError::WriteOutput)?;
    }
    if let Some(source_type) = &memory.source_type {
        writeln!(stdout, "Source type: {source_type}").map_err(CliError::WriteOutput)?;
    }
    if let Some(source_id) = &memory.source_id {
        writeln!(stdout, "Source id: {source_id}").map_err(CliError::WriteOutput)?;
    }
    writeln!(stdout, "Title: {}", memory.title).map_err(CliError::WriteOutput)?;
    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "{}", memory.content).map_err(CliError::WriteOutput)?;
    Ok(())
}

fn redacted_list_title(memory: &MemorySummary) -> String {
    redacted_title(memory.sensitivity, &memory.title, 96)
}

fn redacted_title(sensitivity: Sensitivity, title: &str, max_chars: usize) -> String {
    match sensitivity {
        Sensitivity::Public => preview(title, max_chars),
        Sensitivity::Normal
        | Sensitivity::Private
        | Sensitivity::Sensitive
        | Sensitivity::Secret => "[redacted; use memory show <id>]".to_string(),
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
        Some(Sensitivity::Public) | None => preview(details, max_chars),
        Some(
            Sensitivity::Normal
            | Sensitivity::Private
            | Sensitivity::Sensitive
            | Sensitivity::Secret,
        ) => "[redacted; use memory show <id>]".to_string(),
    }
}

fn grant_target(memory_id: Option<&str>, scope_id: Option<&str>) -> String {
    memory_id
        .map(|id| format!("memory:{id}"))
        .or_else(|| scope_id.map(|id| format!("scope:{id}")))
        .unwrap_or_else(|| "-".to_string())
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
mod tests {
    use super::*;
    use noema_core::{
        GraphAccessGrant, GraphEntityNode, GraphMemoryEvent, GraphMemoryNode, GraphObjectLinkEdge,
        GraphParticipantEdge, GraphProvenanceEdge, GraphPurposeRule, GraphSubjectEdge,
        memory_persistence::MemoryType,
    };

    #[test]
    fn redacts_non_public_memory_list_titles() {
        let mut memory = MemorySummary {
            id: "mem_123".to_string(),
            status: MemoryStatus::Confirmed,
            memory_type: MemoryType::Note,
            home_scope_id: "conversation:conversation_1".to_string(),
            sensitivity: Sensitivity::Normal,
            title: "my API key is sk-test1234567890".to_string(),
            content: "my API key is sk-test1234567890".to_string(),
            created_at: "2026-06-24 12:00:00".to_string(),
            source_type: Some("episode".to_string()),
            source_id: Some("conversation:conversation_1".to_string()),
            conversation_id: Some("conversation:conversation_1".to_string()),
        };

        assert_eq!(
            redacted_list_title(&memory),
            "[redacted; use memory show <id>]"
        );

        memory.sensitivity = Sensitivity::Public;
        memory.title = "Public project note".to_string();
        assert_eq!(redacted_list_title(&memory), "Public project note");
    }

    #[test]
    fn context_graph_output_includes_object_links_and_redacts_memory_titles() {
        let graph = ContextGraphSummary {
            memories: vec![GraphMemoryNode {
                memory_id: "mem_sensitive".to_string(),
                status: MemoryStatus::Confirmed,
                memory_type: MemoryType::OpenLoop,
                home_scope_id: "conversation:health".to_string(),
                sensitivity: Sensitivity::Sensitive,
                title: "Doctor follow-up detail".to_string(),
                retrieval_hints: r#"{"topics":["doctor"]}"#.to_string(),
                retrieval_policy_status: RetrievalPolicyStatus::Valid,
                retrieval_policy_effective_status: RetrievalPolicyStatus::Stale,
                retrieval_policy_version: 2,
                retrieval_policy_fingerprint: Some("sha256:abcdef1234567890".to_string()),
                retrieval_policy_extractor_principal_id: Some("agent:primary".to_string()),
                retrieval_policy_extractor_version: Some("extractor-v1".to_string()),
                retrieval_policy_validated_at: Some("2026-06-24 12:00:00".to_string()),
                participant_visibility_policy: ParticipantVisibilityPolicy::OwnerOnly,
                external_egress_policy: ExternalEgressPolicy::ApprovalRequired,
                created_at: "2026-06-24 12:00:00".to_string(),
            }],
            entities: Vec::<GraphEntityNode>::new(),
            subject_edges: Vec::<GraphSubjectEdge>::new(),
            participant_edges: Vec::<GraphParticipantEdge>::new(),
            provenance_edges: Vec::<GraphProvenanceEdge>::new(),
            object_link_edges: vec![GraphObjectLinkEdge {
                memory_id: "mem_sensitive".to_string(),
                object_type: "task".to_string(),
                object_id: "task:schedule_checkup".to_string(),
                relation: "open_loop_for".to_string(),
                authorized_scope_id: Some("conversation:health".to_string()),
                resolver_principal_id: Some("agent:primary".to_string()),
                resolver_version: Some("resolver-v1".to_string()),
                source_run_id: Some("run:health".to_string()),
                created_at: "2026-06-24 12:00:00".to_string(),
            }],
            purpose_rules: vec![
                GraphPurposeRule {
                    memory_id: "mem_sensitive".to_string(),
                    purpose: Purpose::AnswerHumanQuestion,
                    effect: Effect::Allow,
                    created_by_principal_id: Some("agent:primary".to_string()),
                    created_at: "2026-06-24 12:00:00".to_string(),
                },
                GraphPurposeRule {
                    memory_id: "mem_sensitive".to_string(),
                    purpose: Purpose::ExternalAction,
                    effect: Effect::Deny,
                    created_by_principal_id: Some("agent:primary".to_string()),
                    created_at: "2026-06-24 12:00:00".to_string(),
                },
            ],
            access_grants: vec![GraphAccessGrant {
                grant_id: "grant_1".to_string(),
                memory_id: Some("mem_sensitive".to_string()),
                scope_id: None,
                principal_id: "agent:primary".to_string(),
                permission: "use_for_retrieval".to_string(),
                effect: Effect::Allow,
                expires_at: Some("2000-01-01 00:00:00".to_string()),
                created_by_principal_id: Some("human:kevin".to_string()),
                created_at: "2026-06-24 12:00:00".to_string(),
            }],
            memory_events: vec![GraphMemoryEvent {
                event_id: "event_memory_shown".to_string(),
                event_type: "shown_to_model".to_string(),
                actor_principal_id: Some("agent:primary".to_string()),
                memory_id: Some("mem_sensitive".to_string()),
                memory_sensitivity: Some(Sensitivity::Sensitive),
                scope_id: Some("conversation:health".to_string()),
                reason: Some("context_packet".to_string()),
                created_at: "2026-06-24 12:00:00".to_string(),
                details: r#"{"run_id":"run:health","quote":"doctor"}"#.to_string(),
            }],
            relationships: Vec::new(),
        };
        let mut output = Vec::new();

        write_context_graph(&mut output, &graph).expect("write graph");
        let output = String::from_utf8(output).expect("utf8 output");

        assert!(output.contains("object_link_edges=1"));
        assert!(output.contains("purpose_rules=2"));
        assert!(output.contains("access_grants=1"));
        assert!(output.contains("memory_events=1"));
        assert!(output.contains("Retrieval policy"));
        assert!(output.contains("Trusted object links"));
        assert!(output.contains("Purpose rules"));
        assert!(output.contains("Access grants"));
        assert!(output.contains("Memory events"));
        assert!(output.contains("owner_only"));
        assert!(output.contains("stale"));
        assert!(output.contains("agent:primary"));
        assert!(output.contains("extractor-v1"));
        assert!(output.contains("approval_required"));
        assert!(output.contains("answer_human_question"));
        assert!(output.contains("external_action"));
        assert!(output.contains("deny"));
        assert!(output.contains("use_for_retrieval"));
        assert!(output.contains("2000-01-01 00:00:00"));
        assert!(output.contains("shown_to_model"));
        assert!(output.contains("context_packet"));
        assert!(output.contains("task:schedule_checkup"));
        assert!(output.contains("open_loop_for"));
        assert!(output.contains("conversation:health"));
        assert!(output.contains("[redacted; use memory show <id>]"));
        assert!(!output.contains("Doctor follow-up detail"));
        assert!(!output.contains(r#"{"topics":["doctor"]}"#));
        assert!(!output.contains(r#"{"run_id":"run:health","quote":"doctor"}"#));
    }
}
