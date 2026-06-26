//! CLI inspection commands for local memory and context state.

use clap::Subcommand;
pub(crate) use context_graph_output::ContextGraphFormat;
use context_graph_output::write_context_graph;
use noema_core::{
    CliOverrides, Config, ContextGraphFilter, PostgresMemoryRepository,
    memory::{
        Effect, ExternalEgressPolicy, MemoryStatus, ParticipantRole, ParticipantVisibilityPolicy,
        Purpose, RelationshipStatus, RetrievalPolicyStatus, Sensitivity, SubjectRole,
    },
    memory_persistence::MemorySummary,
};
use std::{
    io::{self, Write},
    path::PathBuf,
};

use crate::CliError;

mod context_graph_output;
mod context_graph_text;

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
    config_path: Option<PathBuf>,
) -> Result<(), CliError> {
    let config = Config::load_daemon(config_path, CliOverrides::default())?;
    let repo = PostgresMemoryRepository::connect(&config.database).await?;

    match command {
        MemoryCommand::List { limit } => {
            let memories = repo.list_recent_memories(Some(*limit)).await?;
            print_memory_list(&memories)?;
        }
        MemoryCommand::Show { memory_id } => {
            let memory = repo
                .get_memory(memory_id)
                .await?
                .ok_or_else(|| CliError::MemoryNotFound(memory_id.clone()))?;
            print_memory_detail(&memory)?;
        }
    }

    Ok(())
}

pub(crate) async fn run_context(
    command: &ContextCommand,
    config_path: Option<PathBuf>,
) -> Result<(), CliError> {
    let config = Config::load_daemon(config_path, CliOverrides::default())?;
    let repo = PostgresMemoryRepository::connect(&config.database).await?;

    match command {
        ContextCommand::Graph {
            limit,
            run_id,
            context_packet_id,
            format,
        } => {
            let filter = ContextGraphFilter {
                run_id: run_id.clone(),
                context_packet_id: context_packet_id.clone(),
            };
            let graph = repo
                .inspect_context_graph_with_filter(&filter, Some(*limit))
                .await?;
            print_context_graph(&graph, *format)?;
        }
    }

    Ok(())
}

fn print_context_graph(
    graph: &noema_core::ContextGraphSummary,
    format: ContextGraphFormat,
) -> Result<(), CliError> {
    let mut stdout = io::stdout();
    write_context_graph(&mut stdout, graph, format)
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
    writeln!(
        stdout,
        "Owner: {}",
        object_ref(&memory.owner_object_type, &memory.owner_object_id)
    )
    .map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Created: {}", memory.created_at).map_err(CliError::WriteOutput)?;
    if let Some(conversation_id) = &memory.conversation_id {
        writeln!(stdout, "Conversation: {conversation_id}").map_err(CliError::WriteOutput)?;
    }
    if let Some(source) = memory
        .source_object_type
        .as_deref()
        .zip(memory.source_object_id.as_deref())
    {
        writeln!(stdout, "Source: {}", object_ref(source.0, source.1))
            .map_err(CliError::WriteOutput)?;
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
