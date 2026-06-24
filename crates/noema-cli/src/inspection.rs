//! CLI inspection commands for local memory and context state.

use clap::Subcommand;
use noema_core::{
    ContextGraphSummary, NoemaPaths, SqliteMemoryRepository,
    memory::{MemoryStatus, ParticipantRole, RelationshipStatus, Sensitivity, SubjectRole},
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
        "memories={} entities={} subject_edges={} participant_edges={} provenance_edges={} object_link_edges={} relationships={}",
        graph.memories.len(),
        graph.entities.len(),
        graph.subject_edges.len(),
        graph.participant_edges.len(),
        graph.provenance_edges.len(),
        graph.object_link_edges.len(),
        graph.relationships.len(),
    )
    .map_err(CliError::WriteOutput)?;

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Memories").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<10}  {:<10}  {:<10}  {:<24}  Title",
        "ID", "Status", "Type", "Privacy", "Scope"
    )
    .map_err(CliError::WriteOutput)?;
    for memory in &graph.memories {
        writeln!(
            stdout,
            "{:<38}  {:<10}  {:<10}  {:<10}  {:<24}  {}",
            memory.memory_id,
            memory_status_label(memory.status),
            memory.memory_type.as_str(),
            sensitivity_label(memory.sensitivity),
            memory.home_scope_id,
            redacted_title(memory.sensitivity, &memory.title, 80),
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

fn preview(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let mut preview: String = trimmed.chars().take(max_chars).collect();
    if trimmed.chars().count() > max_chars {
        preview.push_str("...");
    }
    preview
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
        GraphEntityNode, GraphMemoryNode, GraphObjectLinkEdge, GraphParticipantEdge,
        GraphProvenanceEdge, GraphSubjectEdge, memory_persistence::MemoryType,
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
            relationships: Vec::new(),
        };
        let mut output = Vec::new();

        write_context_graph(&mut output, &graph).expect("write graph");
        let output = String::from_utf8(output).expect("utf8 output");

        assert!(output.contains("object_link_edges=1"));
        assert!(output.contains("Trusted object links"));
        assert!(output.contains("task:schedule_checkup"));
        assert!(output.contains("open_loop_for"));
        assert!(output.contains("conversation:health"));
        assert!(output.contains("[redacted; use memory show <id>]"));
        assert!(!output.contains("Doctor follow-up detail"));
    }
}
