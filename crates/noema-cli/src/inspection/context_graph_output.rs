//! Context graph output renderers.

use clap::ValueEnum;
use noema_core::ContextGraphSummary;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Write as _},
    io::Write,
};

use crate::CliError;

use super::{
    context_graph_text::write_context_graph_text, effect_label, memory_status_label,
    participant_role_label, preview, purpose_label, redacted_event_details, redacted_json,
    relationship_status_label, retrieval_policy_status_label, sensitivity_label,
    subject_role_label, use_record_object,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ContextGraphFormat {
    Text,
    Mermaid,
}

impl fmt::Display for ContextGraphFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => formatter.write_str("text"),
            Self::Mermaid => formatter.write_str("mermaid"),
        }
    }
}

pub(crate) fn write_context_graph(
    stdout: &mut impl Write,
    graph: &ContextGraphSummary,
    format: ContextGraphFormat,
) -> Result<(), CliError> {
    match format {
        ContextGraphFormat::Text => write_context_graph_text(stdout, graph),
        ContextGraphFormat::Mermaid => write_context_graph_mermaid(stdout, graph),
    }
}

fn write_context_graph_mermaid(
    stdout: &mut impl Write,
    graph: &ContextGraphSummary,
) -> Result<(), CliError> {
    let mut mermaid = MermaidGraph::default();

    for memory in &graph.memories {
        let memory_node = memory_node_id(&memory.memory_id);
        mermaid.set_node(memory_node.clone(), memory_label(memory));

        let scope_node = add_scope_node(&mut mermaid, &memory.home_scope_id);
        mermaid.edge(memory_node, scope_node, "home_scope");
    }

    for entity in &graph.entities {
        let entity_node = entity_node_id(&entity.entity_id);
        mermaid.set_node(
            entity_node.clone(),
            format!(
                "entity: {}\n{}\n{}",
                entity.entity_id, entity.entity_type, entity.canonical_name
            ),
        );

        if let Some(scope_id) = entity.home_scope_id.as_deref() {
            let scope_node = add_scope_node(&mut mermaid, scope_id);
            mermaid.edge(entity_node.clone(), scope_node, "home_scope");
        }
        if let Some(principal_id) = entity.linked_principal_id.as_deref() {
            let principal_node = add_principal_node(&mut mermaid, principal_id);
            mermaid.edge(entity_node, principal_node, "linked_principal");
        }
    }

    for packet in &graph.context_packets {
        let packet_node = context_packet_node_id(&packet.context_packet_id);
        mermaid.set_node(
            packet_node.clone(),
            format!(
                "packet: {}\nrun: {}\npurpose: {}\nactive: {}",
                packet.context_packet_id,
                packet.run_id,
                purpose_label(packet.purpose),
                preview(&packet.active_scopes, 64)
            ),
        );
        let run_node = add_run_node(&mut mermaid, &packet.run_id);
        let principal_node = add_principal_node(&mut mermaid, &packet.requesting_principal_id);
        mermaid.edge(packet_node.clone(), run_node, "run");
        mermaid.edge(packet_node, principal_node, "requester");
    }

    for relationship in &graph.relationships {
        let relationship_node = relationship_node_id(&relationship.relationship_id);
        mermaid.set_node(
            relationship_node.clone(),
            format!(
                "relationship: {}\n{} / {}",
                relationship.relationship_id,
                relationship_status_label(relationship.status),
                relationship.predicate
            ),
        );

        let scope_node = add_scope_node(&mut mermaid, &relationship.home_scope_id);
        let subject_node = add_entity_reference(
            &mut mermaid,
            &relationship.subject_entity_id,
            relationship.subject_name.as_deref(),
        );
        let object_node = add_entity_reference(
            &mut mermaid,
            &relationship.object_entity_id,
            relationship.object_name.as_deref(),
        );
        mermaid.edge(relationship_node.clone(), scope_node, "home_scope");
        mermaid.edge(subject_node, relationship_node.clone(), "subject");
        mermaid.edge(relationship_node.clone(), object_node, "object");
        if let Some(memory_id) = relationship.memory_id.as_deref() {
            let memory_node = add_memory_reference(&mut mermaid, memory_id);
            mermaid.edge(relationship_node, memory_node, "backed_by");
        }
    }

    for edge in &graph.subject_edges {
        let memory_node = add_memory_reference(&mut mermaid, &edge.memory_id);
        let entity_node = add_entity_reference(&mut mermaid, &edge.entity_id, None);
        mermaid.edge(memory_node, entity_node, subject_role_label(edge.role));
    }

    for edge in &graph.participant_edges {
        let memory_node = add_memory_reference(&mut mermaid, &edge.memory_id);
        let principal_node = add_principal_node(&mut mermaid, &edge.principal_id);
        mermaid.edge(
            memory_node,
            principal_node,
            participant_role_label(edge.role),
        );
    }

    for edge in &graph.provenance_edges {
        let memory_node = add_memory_reference(&mut mermaid, &edge.memory_id);
        let source_node = add_source_node(&mut mermaid, &edge.source_type, &edge.source_id);
        mermaid.edge(
            memory_node,
            source_node,
            format!("{} / {}", edge.relation, edge.source_type),
        );
    }

    for edge in &graph.object_link_edges {
        let memory_node = add_memory_reference(&mut mermaid, &edge.memory_id);
        let object_node = add_object_node(&mut mermaid, &edge.object_type, &edge.object_id);
        mermaid.edge(memory_node, object_node.clone(), edge.relation.clone());
        if let Some(scope_id) = edge.authorized_scope_id.as_deref() {
            let scope_node = add_scope_node(&mut mermaid, scope_id);
            mermaid.edge(object_node.clone(), scope_node, "authorized_scope");
        }
        if let Some(principal_id) = edge.resolver_principal_id.as_deref() {
            let principal_node = add_principal_node(&mut mermaid, principal_id);
            mermaid.edge(object_node, principal_node, "resolver");
        }
    }

    for rule in &graph.purpose_rules {
        let memory_node = add_memory_reference(&mut mermaid, &rule.memory_id);
        let rule_node = purpose_rule_node_id(
            &rule.memory_id,
            purpose_label(rule.purpose),
            effect_label(rule.effect),
        );
        mermaid.set_node(
            rule_node.clone(),
            format!(
                "purpose: {}\n{}",
                purpose_label(rule.purpose),
                effect_label(rule.effect)
            ),
        );
        mermaid.edge(memory_node, rule_node.clone(), "purpose_rule");
        if let Some(principal_id) = rule.created_by_principal_id.as_deref() {
            let principal_node = add_principal_node(&mut mermaid, principal_id);
            mermaid.edge(rule_node, principal_node, "created_by");
        }
    }

    for grant in &graph.access_grants {
        let grant_node = access_grant_node_id(&grant.grant_id);
        mermaid.set_node(
            grant_node.clone(),
            format!(
                "grant: {}\n{} {}",
                grant.grant_id,
                effect_label(grant.effect),
                grant.permission
            ),
        );
        if let Some(memory_id) = grant.memory_id.as_deref() {
            let memory_node = add_memory_reference(&mut mermaid, memory_id);
            mermaid.edge(grant_node.clone(), memory_node, "target");
        }
        if let Some(scope_id) = grant.scope_id.as_deref() {
            let scope_node = add_scope_node(&mut mermaid, scope_id);
            mermaid.edge(grant_node.clone(), scope_node, "target");
        }
        let principal_node = add_principal_node(&mut mermaid, &grant.principal_id);
        mermaid.edge(grant_node.clone(), principal_node, "grantee");
        if let Some(principal_id) = grant.created_by_principal_id.as_deref() {
            let created_by_node = add_principal_node(&mut mermaid, principal_id);
            mermaid.edge(grant_node, created_by_node, "created_by");
        }
    }

    for edge in &graph.context_packet_memory_edges {
        let packet_node = add_context_packet_reference(&mut mermaid, &edge.context_packet_id);
        let memory_node = add_memory_reference(&mut mermaid, &edge.memory_id);
        mermaid.edge(packet_node, memory_node, packet_memory_edge_label(edge));
    }

    for omission in &graph.context_packet_omissions {
        let omission_node = context_packet_omission_node_id(&omission.omission_id);
        mermaid.set_node(
            omission_node.clone(),
            format!(
                "omission: {}\n{}\naudit: {}\ndetails: {}",
                omission.omission_id,
                omission.agent_visible_reason,
                omission.audit_reason,
                redacted_json(omission.omission_sensitivity, &omission.details, 64)
            ),
        );
        let packet_node = add_context_packet_reference(&mut mermaid, &omission.context_packet_id);
        mermaid.edge(packet_node, omission_node.clone(), "omits");
        if let Some(memory_id) = omission.memory_id.as_deref() {
            let memory_node = add_memory_reference(&mut mermaid, memory_id);
            mermaid.edge(omission_node.clone(), memory_node, "memory");
        }
        if let Some(relationship_id) = omission.relationship_id.as_deref() {
            let relationship_node = add_relationship_reference(&mut mermaid, relationship_id);
            mermaid.edge(omission_node, relationship_node, "relationship");
        }
    }

    for record in &graph.memory_use_records {
        let use_node = memory_use_node_id(&record.memory_use_id);
        mermaid.set_node(
            use_node.clone(),
            format!(
                "memory use: {}\n{} / {}\ndetails: {}",
                record.memory_use_id,
                record.stage,
                purpose_label(record.purpose),
                redacted_json(record.memory_sensitivity, &record.details, 64)
            ),
        );
        let run_node = add_run_node(&mut mermaid, &record.run_id);
        let memory_node = add_memory_reference(&mut mermaid, &record.memory_id);
        mermaid.edge(run_node, use_node.clone(), "memory_use");
        mermaid.edge(use_node.clone(), memory_node, record.stage.clone());
        if let Some(packet_id) = record.context_packet_id.as_deref() {
            let packet_node = add_context_packet_reference(&mut mermaid, packet_id);
            mermaid.edge(use_node.clone(), packet_node, "packet");
        }
        if let Some(principal_id) = record.agent_principal_id.as_deref() {
            let principal_node = add_principal_node(&mut mermaid, principal_id);
            mermaid.edge(use_node.clone(), principal_node, "agent");
        }
        if let Some(scope_id) = record.scope_id.as_deref() {
            let scope_node = add_scope_node(&mut mermaid, scope_id);
            mermaid.edge(use_node.clone(), scope_node, "scope");
        }
        let used_object = use_record_object(
            record.used_for_object_type.as_deref(),
            record.used_for_object_id.as_deref(),
        );
        if used_object != "-" {
            let object_node = add_object_node(
                &mut mermaid,
                record.used_for_object_type.as_deref().unwrap_or("object"),
                &used_object,
            );
            mermaid.edge(use_node, object_node, "used_for");
        }
    }

    for event in &graph.memory_events {
        let event_node = memory_event_node_id(&event.event_id);
        mermaid.set_node(
            event_node.clone(),
            format!(
                "event: {}\n{}\nreason: {}\ndetails: {}",
                event.event_id,
                event.event_type,
                event.reason.as_deref().unwrap_or("-"),
                redacted_event_details(event.memory_sensitivity, &event.details, 64)
            ),
        );
        if let Some(memory_id) = event.memory_id.as_deref() {
            let memory_node = add_memory_reference(&mut mermaid, memory_id);
            mermaid.edge(event_node.clone(), memory_node, "memory");
        }
        if let Some(principal_id) = event.actor_principal_id.as_deref() {
            let principal_node = add_principal_node(&mut mermaid, principal_id);
            mermaid.edge(event_node.clone(), principal_node, "actor");
        }
        if let Some(scope_id) = event.scope_id.as_deref() {
            let scope_node = add_scope_node(&mut mermaid, scope_id);
            mermaid.edge(event_node, scope_node, "scope");
        }
    }

    if mermaid.nodes.is_empty() {
        mermaid.set_node("empty_context_graph".to_string(), "empty context graph");
    }

    writeln!(stdout, "flowchart TD").map_err(CliError::WriteOutput)?;
    writeln!(
        stdout,
        "  %% memories={} entities={} subject_edges={} participant_edges={} provenance_edges={} object_link_edges={} purpose_rules={} access_grants={} context_packets={} packet_memory_edges={} packet_omissions={} memory_use_records={} memory_events={} relationships={}",
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

    for (node_id, label) in mermaid.nodes {
        writeln!(stdout, "  {node_id}[\"{}\"]", mermaid_label(&label))
            .map_err(CliError::WriteOutput)?;
    }
    for edge in mermaid.edges {
        writeln!(
            stdout,
            "  {} -->|{}| {}",
            edge.from,
            mermaid_edge_label(&edge.label),
            edge.to
        )
        .map_err(CliError::WriteOutput)?;
    }

    Ok(())
}

#[derive(Default)]
struct MermaidGraph {
    nodes: BTreeMap<String, String>,
    edges: BTreeSet<MermaidEdge>,
}

impl MermaidGraph {
    fn set_node(&mut self, id: String, label: impl Into<String>) -> String {
        self.nodes.insert(id.clone(), label.into());
        id
    }

    fn node(&mut self, id: String, label: impl Into<String>) -> String {
        self.nodes.entry(id.clone()).or_insert_with(|| label.into());
        id
    }

    fn edge(&mut self, from: String, to: String, label: impl Into<String>) {
        self.edges.insert(MermaidEdge {
            from,
            to,
            label: label.into(),
        });
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct MermaidEdge {
    from: String,
    to: String,
    label: String,
}

fn memory_label(memory: &noema_core::GraphMemoryNode) -> String {
    format!(
        "memory: {}\n{} / {} / {}\npolicy: {} -> {}\ntitle: {}\ncontent: {}",
        memory.memory_id,
        memory_status_label(memory.status),
        memory.memory_type.as_str(),
        sensitivity_label(memory.sensitivity),
        retrieval_policy_status_label(memory.retrieval_policy_status),
        retrieval_policy_status_label(memory.retrieval_policy_effective_status),
        preview(&memory.title, 80),
        preview(&memory.content, 120),
    )
}

fn packet_memory_edge_label(edge: &noema_core::GraphContextPacketMemoryEdge) -> String {
    let mut label = edge.stage.clone();
    if let Some(reason) = edge.eligibility_reason.as_deref() {
        label.push_str(" / ");
        label.push_str(reason);
    }
    if let Some(score) = edge.rank_score {
        label.push_str(" / score ");
        label.push_str(&score.to_string());
    }
    preview(&label, 80)
}

fn add_memory_reference(mermaid: &mut MermaidGraph, memory_id: &str) -> String {
    mermaid.node(memory_node_id(memory_id), format!("memory: {memory_id}"))
}

fn add_entity_reference(
    mermaid: &mut MermaidGraph,
    entity_id: &str,
    canonical_name: Option<&str>,
) -> String {
    let label = canonical_name
        .map(|name| format!("entity: {entity_id}\n{name}"))
        .unwrap_or_else(|| format!("entity: {entity_id}"));
    mermaid.node(entity_node_id(entity_id), label)
}

fn add_principal_node(mermaid: &mut MermaidGraph, principal_id: &str) -> String {
    mermaid.node(
        principal_node_id(principal_id),
        format!("principal: {principal_id}"),
    )
}

fn add_scope_node(mermaid: &mut MermaidGraph, scope_id: &str) -> String {
    mermaid.node(scope_node_id(scope_id), format!("scope: {scope_id}"))
}

fn add_source_node(mermaid: &mut MermaidGraph, source_type: &str, source_id: &str) -> String {
    mermaid.node(
        source_node_id(source_type, source_id),
        format!("source: {source_type}\n{source_id}"),
    )
}

fn add_object_node(mermaid: &mut MermaidGraph, object_type: &str, object_id: &str) -> String {
    mermaid.node(
        object_node_id(object_type, object_id),
        format!("object: {object_type}\n{object_id}"),
    )
}

fn add_run_node(mermaid: &mut MermaidGraph, run_id: &str) -> String {
    mermaid.node(run_node_id(run_id), format!("run: {run_id}"))
}

fn add_context_packet_reference(mermaid: &mut MermaidGraph, packet_id: &str) -> String {
    mermaid.node(
        context_packet_node_id(packet_id),
        format!("packet: {packet_id}"),
    )
}

fn add_relationship_reference(mermaid: &mut MermaidGraph, relationship_id: &str) -> String {
    mermaid.node(
        relationship_node_id(relationship_id),
        format!("relationship: {relationship_id}"),
    )
}

fn memory_node_id(memory_id: &str) -> String {
    mermaid_node_id("mem_", memory_id)
}

fn entity_node_id(entity_id: &str) -> String {
    mermaid_node_id("entity_", entity_id)
}

fn principal_node_id(principal_id: &str) -> String {
    mermaid_node_id("principal_", principal_id)
}

fn scope_node_id(scope_id: &str) -> String {
    mermaid_node_id("scope_", scope_id)
}

fn source_node_id(source_type: &str, source_id: &str) -> String {
    mermaid_node_id("source_", &format!("{source_type}:{source_id}"))
}

fn object_node_id(object_type: &str, object_id: &str) -> String {
    mermaid_node_id("object_", &format!("{object_type}:{object_id}"))
}

fn run_node_id(run_id: &str) -> String {
    mermaid_node_id("run_", run_id)
}

fn context_packet_node_id(packet_id: &str) -> String {
    mermaid_node_id("packet_", packet_id)
}

fn context_packet_omission_node_id(omission_id: &str) -> String {
    mermaid_node_id("omission_", omission_id)
}

fn relationship_node_id(relationship_id: &str) -> String {
    mermaid_node_id("relationship_", relationship_id)
}

fn purpose_rule_node_id(memory_id: &str, purpose: &str, effect: &str) -> String {
    mermaid_node_id("purpose_", &format!("{memory_id}:{purpose}:{effect}"))
}

fn access_grant_node_id(grant_id: &str) -> String {
    mermaid_node_id("grant_", grant_id)
}

fn memory_use_node_id(memory_use_id: &str) -> String {
    mermaid_node_id("use_", memory_use_id)
}

fn memory_event_node_id(event_id: &str) -> String {
    mermaid_node_id("event_", event_id)
}

fn mermaid_node_id(prefix: &str, raw: &str) -> String {
    let mut id = String::with_capacity(prefix.len() + raw.len());
    id.push_str(prefix);
    for byte in raw.as_bytes() {
        match *byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' => id.push(*byte as char),
            _ => {
                write!(&mut id, "_{:02x}", *byte).expect("writing to a String cannot fail");
            }
        }
    }
    id
}

fn mermaid_label(value: &str) -> String {
    let mut label = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => label.push_str("&amp;"),
            '<' => label.push_str("&lt;"),
            '>' => label.push_str("&gt;"),
            '"' => label.push_str("&quot;"),
            '\n' => label.push_str("<br/>"),
            '\r' | '\t' => label.push(' '),
            _ => label.push(ch),
        }
    }
    label
}

fn mermaid_edge_label(value: &str) -> String {
    mermaid_label(&preview(value, 80)).replace('|', "/")
}
