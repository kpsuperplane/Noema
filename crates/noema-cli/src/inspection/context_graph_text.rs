//! Text output for context graph inspection.

use noema_core::ContextGraphSummary;
use std::io::Write;

use crate::CliError;

use super::{
    effect_label, external_egress_policy_label, memory_status_label, object_ref,
    optional_object_ref, participant_role_label, participant_visibility_policy_label, preview,
    purpose_label, redacted_event_details, redacted_json, relationship_status_label,
    retrieval_policy_status_label, sensitivity_label, subject_role_label, use_record_object,
};

pub(super) fn write_context_graph_text(
    stdout: &mut impl Write,
    graph: &ContextGraphSummary,
) -> Result<(), CliError> {
    writeln!(stdout, "Context graph").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "memories={} entities={} subject_edges={} participant_edges={} provenance_edges={} object_link_edges={} purpose_rules={} access_grants={} context_packets={} packet_memory_edges={} packet_omissions={} memory_use_records={} object_events={} relationships={}",
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
        graph.object_events.len(),
        graph.relationships.len(),
    )
    .map_err(CliError::WriteOutput)?;

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Memories").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<10}  {:<10}  {:<10}  {:<12}  {:<12}  {:<28}  Title",
        "ID", "Status", "Type", "Privacy", "Stored", "Effective", "Owner"
    )
    .map_err(CliError::WriteOutput)?;
    for memory in &graph.memories {
        writeln!(
            stdout,
            "{:<38}  {:<10}  {:<10}  {:<10}  {:<12}  {:<12}  {:<28}  {}",
            memory.memory_id,
            memory_status_label(memory.status),
            memory.memory_type.as_str(),
            sensitivity_label(memory.sensitivity),
            retrieval_policy_status_label(memory.retrieval_policy_status),
            retrieval_policy_status_label(memory.retrieval_policy_effective_status),
            object_ref(&memory.owner_object_type, &memory.owner_object_id),
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
                .retrieval_policy_extractor_object_type
                .as_deref()
                .zip(memory.retrieval_policy_extractor_object_id.as_deref())
                .map(|(object_type, object_id)| object_ref(object_type, object_id))
                .unwrap_or_else(|| "-".to_string()),
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
        "{:<38}  {:<14}  {:<28}  {:<28}  Name",
        "ID", "Type", "Owner", "Linked object"
    )
    .map_err(CliError::WriteOutput)?;
    for entity in &graph.entities {
        writeln!(
            stdout,
            "{:<38}  {:<14}  {:<28}  {:<28}  {}",
            entity.entity_id,
            entity.entity_type,
            optional_object_ref(
                entity.owner_object_type.as_deref(),
                entity.owner_object_id.as_deref()
            ),
            optional_object_ref(
                entity.linked_object_type.as_deref(),
                entity.linked_object_id.as_deref()
            ),
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
    writeln!(stdout, "{:<38}  {:<34}  Role", "Memory", "Participant")
        .map_err(CliError::WriteOutput)?;
    for edge in &graph.participant_edges {
        writeln!(
            stdout,
            "{:<38}  {:<34}  {}",
            edge.memory_id,
            object_ref(&edge.participant_object_type, &edge.participant_object_id),
            participant_role_label(edge.role),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Provenance edges").map_err(CliError::WriteOutput)?;
    writeln!(stdout, "{:<38}  {:<44}  Relation", "Memory", "Source")
        .map_err(CliError::WriteOutput)?;
    for edge in &graph.provenance_edges {
        writeln!(
            stdout,
            "{:<38}  {:<44}  {}",
            edge.memory_id,
            object_ref(&edge.source_object_type, &edge.source_object_id),
            edge.relation,
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Trusted object links").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<18}  {:<44}  {:<44}  Resolver",
        "Memory", "Relation", "Object", "Authorized object"
    )
    .map_err(CliError::WriteOutput)?;
    for edge in &graph.object_link_edges {
        writeln!(
            stdout,
            "{:<38}  {:<18}  {:<44}  {:<44}  {}",
            edge.memory_id,
            edge.relation,
            object_ref(&edge.object_type, &edge.object_id),
            optional_object_ref(
                edge.authorized_object_type.as_deref(),
                edge.authorized_object_id.as_deref()
            ),
            optional_object_ref(
                edge.resolver_object_type.as_deref(),
                edge.resolver_object_id.as_deref()
            ),
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
            optional_object_ref(
                rule.created_by_object_type.as_deref(),
                rule.created_by_object_id.as_deref()
            ),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Access grants").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<44}  {:<34}  {:<24}  {:<6}  Expires",
        "Grant", "Target", "Grantee", "Permission", "Effect"
    )
    .map_err(CliError::WriteOutput)?;
    for grant in &graph.access_grants {
        writeln!(
            stdout,
            "{:<38}  {:<44}  {:<34}  {:<24}  {:<6}  {}",
            grant.grant_id,
            object_ref(&grant.target_object_type, &grant.target_object_id),
            object_ref(&grant.grantee_object_type, &grant.grantee_object_id),
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
        "{:<38}  {:<24}  {:<34}  {:<24}  Active objects",
        "Packet", "Run", "Requester", "Purpose"
    )
    .map_err(CliError::WriteOutput)?;
    for packet in &graph.context_packets {
        writeln!(
            stdout,
            "{:<38}  {:<24}  {:<34}  {:<24}  {}",
            packet.context_packet_id,
            packet.run_id,
            object_ref(&packet.requesting_object_type, &packet.requesting_object_id),
            purpose_label(packet.purpose),
            preview(&packet.active_objects, 96),
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
            "{:<38}  agent={} context={} object={} details: {}",
            "",
            optional_object_ref(
                record.agent_object_type.as_deref(),
                record.agent_object_id.as_deref()
            ),
            optional_object_ref(
                record.context_object_type.as_deref(),
                record.context_object_id.as_deref()
            ),
            use_record_object(
                record.used_for_object_type.as_deref(),
                record.used_for_object_id.as_deref(),
            ),
            redacted_json(record.memory_sensitivity, &record.details, 96),
        )
        .map_err(CliError::WriteOutput)?;
    }

    writeln!(stdout).map_err(CliError::WriteOutput)?;
    writeln!(stdout, "Object events").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "{:<38}  {:<16}  {:<34}  {:<44}  Reason",
        "Event", "Type", "Actor", "Target"
    )
    .map_err(CliError::WriteOutput)?;
    for event in &graph.object_events {
        writeln!(
            stdout,
            "{:<38}  {:<16}  {:<34}  {:<44}  {}",
            event.event_id,
            event.event_type,
            optional_object_ref(
                event.actor_object_type.as_deref(),
                event.actor_object_id.as_deref()
            ),
            optional_object_ref(
                event.target_object_type.as_deref(),
                event.target_object_id.as_deref()
            ),
            event.reason.as_deref().unwrap_or("-"),
        )
        .map_err(CliError::WriteOutput)?;
        writeln!(
            stdout,
            "{:<38}  details: {}",
            "",
            redacted_event_details(event.target_memory_sensitivity, &event.details, 96),
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
