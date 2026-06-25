//! Text output for context graph inspection.

use noema_core::ContextGraphSummary;
use std::io::Write;

use crate::CliError;

use super::{
    effect_label, external_egress_policy_label, grant_target, memory_status_label,
    participant_role_label, participant_visibility_policy_label, preview, purpose_label,
    redacted_event_details, redacted_json, relationship_status_label,
    retrieval_policy_status_label, sensitivity_label, subject_role_label, use_record_object,
};

pub(super) fn write_context_graph_text(
    stdout: &mut impl Write,
    graph: &ContextGraphSummary,
) -> Result<(), CliError> {
    writeln!(stdout, "Context graph").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "memories={} entities={} subject_edges={} participant_edges={} provenance_edges={} object_link_edges={} purpose_rules={} access_grants={} context_packets={} packet_memory_edges={} packet_omissions={} memory_use_records={} memory_events={} relationships={}",
        graph.memories.len(),
        graph.entities.len(),
        graph.subject_edges.len(),
        graph.participant_edges.len(),
        graph.provenance_edges.len(),
        graph.object_link_edges.len(),
        graph.purpose_rules.len(),
        graph.access_grants.len(),
        graph.context_packets.len(),
        graph.context_packet_memory_edges.len(),
        graph.context_packet_omissions.len(),
        graph.memory_use_records.len(),
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
            preview(&memory.title, 80),
        )
        .map_err(CliError::WriteOutput)?;
        writeln!(
            stdout,
            "{:<38}  content: {}",
            "",
            preview(&memory.content, 120),
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
            preview(&memory.retrieval_hints, 64),
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
    writeln!(stdout, "Context packets").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<24}  {:<28}  {:<24}  Active scopes",
        "Packet", "Run", "Principal", "Purpose"
    )
    .map_err(CliError::WriteOutput)?;
    for packet in &graph.context_packets {
        writeln!(
            stdout,
            "{:<38}  {:<24}  {:<28}  {:<24}  {}",
            packet.context_packet_id,
            packet.run_id,
            packet.requesting_principal_id,
            purpose_label(packet.purpose),
            preview(&packet.active_scopes, 96),
        )
        .map_err(CliError::WriteOutput)?;
        writeln!(
            stdout,
            "{:<38}  agent_visible_omissions: {}",
            "",
            preview(&packet.agent_visible_omissions, 96),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Context packet memories").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<38}  {:<38}  {:<20}  {:<6}  Eligibility",
        "Edge", "Packet", "Memory", "Stage", "Score"
    )
    .map_err(CliError::WriteOutput)?;
    for edge in &graph.context_packet_memory_edges {
        writeln!(
            stdout,
            "{:<38}  {:<38}  {:<38}  {:<20}  {:<6}  {}",
            edge.packet_memory_id,
            edge.context_packet_id,
            edge.memory_id,
            edge.stage,
            edge.rank_score
                .map(|score| score.to_string())
                .unwrap_or_else(|| "-".to_string()),
            edge.eligibility_reason.as_deref().unwrap_or("-"),
        )
        .map_err(CliError::WriteOutput)?;
        writeln!(
            stdout,
            "{:<38}  rank_reasons: {}",
            "",
            redacted_json(edge.memory_sensitivity, &edge.rank_reasons, 96),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Context packet omissions").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<38}  {:<38}  {:<38}  {:<28}  Audit",
        "Omission", "Packet", "Memory", "Relationship", "Agent reason"
    )
    .map_err(CliError::WriteOutput)?;
    for omission in &graph.context_packet_omissions {
        writeln!(
            stdout,
            "{:<38}  {:<38}  {:<38}  {:<38}  {:<28}  {}",
            omission.omission_id,
            omission.context_packet_id,
            omission.memory_id.as_deref().unwrap_or("-"),
            omission.relationship_id.as_deref().unwrap_or("-"),
            omission.agent_visible_reason,
            omission.audit_reason,
        )
        .map_err(CliError::WriteOutput)?;
        writeln!(
            stdout,
            "{:<38}  details: {}",
            "",
            redacted_json(omission.omission_sensitivity, &omission.details, 96),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Memory use records").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<24}  {:<38}  {:<38}  {:<20}  Purpose",
        "Use", "Run", "Packet", "Memory", "Stage"
    )
    .map_err(CliError::WriteOutput)?;
    for record in &graph.memory_use_records {
        writeln!(
            stdout,
            "{:<38}  {:<24}  {:<38}  {:<38}  {:<20}  {}",
            record.memory_use_id,
            record.run_id,
            record.context_packet_id.as_deref().unwrap_or("-"),
            record.memory_id,
            record.stage,
            purpose_label(record.purpose),
        )
        .map_err(CliError::WriteOutput)?;
        writeln!(
            stdout,
            "{:<38}  agent={} scope={} object={} details: {}",
            "",
            record.agent_principal_id.as_deref().unwrap_or("-"),
            record.scope_id.as_deref().unwrap_or("-"),
            use_record_object(
                record.used_for_object_type.as_deref(),
                record.used_for_object_id.as_deref(),
            ),
            redacted_json(record.memory_sensitivity, &record.details, 96),
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
