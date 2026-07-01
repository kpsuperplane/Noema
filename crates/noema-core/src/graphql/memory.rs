use std::collections::HashMap;

use async_graphql::{InputObject, Json, Result, SimpleObject};
use serde_json::Value;

use crate::{
    ClaimStatus, MemoryClaimDetail, MemoryClaimEvidence, MemoryClaimRecord, MemoryGraph,
    MemoryGraphEdge, MemoryGraphNode, MemoryGraphSummary, PredicateProposalRecord,
    memory::Sensitivity,
};

use super::{errors::graphql_error, schema::GraphqlState};

/// Graph-memory claim exposed for memory-management inspection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryClaim")]
pub struct GraphqlMemoryClaim {
    /// Stable claim id.
    pub claim_id: String,
    /// Conservative fact preview. Non-public facts are redacted in list views.
    pub fact: String,
    /// Whether the fact was redacted at the GraphQL boundary.
    pub fact_redacted: bool,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Subject entity display name.
    pub subject_entity_name: String,
    /// Subject entity type.
    pub subject_entity_type: String,
    /// Object entity id when present.
    pub object_entity_id: Option<String>,
    /// Object entity display name when present.
    pub object_entity_name: Option<String>,
    /// Object entity type when present.
    pub object_entity_type: Option<String>,
    /// Claim lifecycle status.
    pub status: String,
    /// Claim sensitivity.
    pub sensitivity: String,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of supporting evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

impl From<MemoryClaimRecord> for GraphqlMemoryClaim {
    fn from(claim: MemoryClaimRecord) -> Self {
        let fact_redacted = claim.sensitivity != Sensitivity::Public;
        Self {
            claim_id: claim.claim_id,
            fact: graphql_list_fact(&claim.fact, claim.sensitivity),
            fact_redacted,
            predicate_id: claim.predicate_id,
            predicate_label: claim.predicate_label,
            subject_entity_id: claim.subject_entity_id,
            subject_entity_name: graphql_list_display_name(
                claim.subject_entity_name,
                claim.sensitivity,
            ),
            subject_entity_type: claim.subject_entity_type,
            object_entity_id: claim.object_entity_id,
            object_entity_name: claim
                .object_entity_name
                .map(|name| graphql_list_display_name(name, claim.sensitivity)),
            object_entity_type: claim.object_entity_type,
            status: claim_status_label(claim.status).to_string(),
            sensitivity: sensitivity_label(claim.sensitivity).to_string(),
            confidence: claim.confidence,
            evidence_count: claim.evidence_count,
            created_at: claim.created_at,
            updated_at: claim.updated_at,
        }
    }
}

fn graphql_list_fact(fact: &str, sensitivity: Sensitivity) -> String {
    match sensitivity {
        Sensitivity::Public => fact.to_string(),
        Sensitivity::Normal
        | Sensitivity::Private
        | Sensitivity::Sensitive
        | Sensitivity::Secret => "[redacted; use memoryClaim(claimId) for detail]".to_string(),
    }
}

fn graphql_list_display_name(name: String, sensitivity: Sensitivity) -> String {
    match sensitivity {
        Sensitivity::Public => name,
        Sensitivity::Normal
        | Sensitivity::Private
        | Sensitivity::Sensitive
        | Sensitivity::Secret => "[redacted; use memoryClaim(claimId) for detail]".to_string(),
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

fn sensitivity_label(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
}

/// Supporting evidence exposed for explicit memory-management claim detail inspection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryClaimEvidence")]
pub struct GraphqlMemoryClaimEvidence {
    /// Stable evidence relation id if available.
    pub evidence_id: Option<String>,
    /// Source conversation item id if available.
    pub source_item_id: Option<String>,
    /// Evidence authority.
    pub authority: String,
    /// Evidence excerpt when available.
    pub excerpt: Option<String>,
    /// Observation timestamp when available.
    pub observed_at: Option<String>,
    /// Evidence creation timestamp.
    pub created_at: String,
}

impl From<MemoryClaimEvidence> for GraphqlMemoryClaimEvidence {
    fn from(evidence: MemoryClaimEvidence) -> Self {
        Self {
            evidence_id: evidence.evidence_id,
            source_item_id: evidence.source_item_id,
            authority: evidence.authority,
            excerpt: evidence.excerpt,
            observed_at: evidence.observed_at,
            created_at: evidence.created_at,
        }
    }
}

/// Graph-memory claim detail exposed for memory-management inspection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryClaimDetail")]
pub struct GraphqlMemoryClaimDetail {
    /// Stable claim id.
    pub claim_id: String,
    /// Human-readable fact text.
    pub fact: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Subject entity display name.
    pub subject_entity_name: String,
    /// Subject entity type.
    pub subject_entity_type: String,
    /// Object entity id when present.
    pub object_entity_id: Option<String>,
    /// Object entity display name when present.
    pub object_entity_name: Option<String>,
    /// Object entity type when present.
    pub object_entity_type: Option<String>,
    /// Claim lifecycle status.
    pub status: String,
    /// Claim sensitivity.
    pub sensitivity: String,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of supporting evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
    /// Support evidence rows.
    pub evidence: Vec<GraphqlMemoryClaimEvidence>,
}

impl From<MemoryClaimDetail> for GraphqlMemoryClaimDetail {
    fn from(detail: MemoryClaimDetail) -> Self {
        let claim = detail.claim;
        Self {
            claim_id: claim.claim_id,
            fact: claim.fact,
            predicate_id: claim.predicate_id,
            predicate_label: claim.predicate_label,
            subject_entity_id: claim.subject_entity_id,
            subject_entity_name: claim.subject_entity_name,
            subject_entity_type: claim.subject_entity_type,
            object_entity_id: claim.object_entity_id,
            object_entity_name: claim.object_entity_name,
            object_entity_type: claim.object_entity_type,
            status: claim_status_label(claim.status).to_string(),
            sensitivity: sensitivity_label(claim.sensitivity).to_string(),
            confidence: claim.confidence,
            evidence_count: claim.evidence_count,
            created_at: claim.created_at,
            updated_at: claim.updated_at,
            evidence: detail.evidence.into_iter().map(Into::into).collect(),
        }
    }
}

/// Predicate proposal exposed for memory-management inspection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "PredicateProposal")]
pub struct GraphqlPredicateProposal {
    /// Stable proposal id.
    pub proposal_id: String,
    /// Human-readable proposed predicate label.
    pub label: String,
    /// Human-readable proposed predicate description.
    pub description: String,
    /// Proposed predicate catalog fields.
    pub proposed_predicate: Json<Value>,
    /// Claim candidate preserved alongside the proposed predicate.
    pub proposed_claim: Json<Value>,
    /// Proposal lifecycle status.
    pub status: String,
    /// Source conversation item id if available.
    pub source_item_id: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl From<PredicateProposalRecord> for GraphqlPredicateProposal {
    fn from(proposal: PredicateProposalRecord) -> Self {
        Self {
            proposal_id: proposal.proposal_id,
            label: proposal.label,
            description: proposal.description,
            proposed_predicate: Json(proposal.proposed_predicate),
            proposed_claim: Json(proposal.proposed_claim),
            status: proposal.status,
            source_item_id: proposal.source_item_id,
            created_at: proposal.created_at,
            updated_at: proposal.updated_at,
        }
    }
}

/// Input filters for bounded graph-memory inspection.
#[derive(Clone, Debug, Default, InputObject)]
#[graphql(name = "MemoryGraphInput")]
pub struct GraphqlMemoryGraphInput {
    /// Optional text query matched against predicate labels and public content.
    pub query: Option<String>,
    /// Optional claim lifecycle statuses. Defaults are owned by the store.
    pub statuses: Option<Vec<String>>,
    /// Optional predicate id.
    pub predicate_id: Option<String>,
    /// Optional exact sensitivity filter.
    pub sensitivity: Option<String>,
    /// Optional bounded result limit.
    pub limit: Option<i32>,
}

/// Bounded graph-memory projection for local memory-management inspection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryGraph")]
pub struct GraphqlMemoryGraph {
    /// Entity nodes incident to returned claim edges.
    pub nodes: Vec<GraphqlMemoryGraphNode>,
    /// Claim edges connecting returned entity nodes.
    pub edges: Vec<GraphqlMemoryGraphEdge>,
    /// Summary metadata for the bounded result.
    pub summary: GraphqlMemoryGraphSummary,
}

/// Entity node in the graph-memory projection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryGraphNode")]
pub struct GraphqlMemoryGraphNode {
    /// Opaque graph node id for this response.
    pub node_id: String,
    /// Opaque entity reference for this graph response.
    pub entity_id: String,
    /// Human-readable entity label for local graph inspection.
    pub label: String,
    /// Entity type string.
    pub entity_type: String,
    /// Whether this node label was redacted at the GraphQL boundary.
    pub redacted: bool,
    /// Count of returned incident claim edges.
    pub claim_count: i64,
}

/// Claim edge in the graph-memory projection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryGraphEdge")]
pub struct GraphqlMemoryGraphEdge {
    /// Stable claim id.
    pub claim_id: String,
    /// Opaque source graph node id matching a returned node.
    pub source_node_id: String,
    /// Opaque target graph node id matching a returned node.
    pub target_node_id: String,
    /// Predicate id.
    pub predicate_id: String,
    /// Predicate label.
    pub predicate_label: String,
    /// Human-readable fact text for local graph inspection.
    pub fact: String,
    /// Whether the fact was redacted at the GraphQL boundary.
    pub fact_redacted: bool,
    /// Claim lifecycle status.
    pub status: String,
    /// Claim sensitivity.
    pub sensitivity: String,
    /// Claim confidence.
    pub confidence: Option<f64>,
    /// Count of supporting evidence rows.
    pub evidence_count: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Summary metadata for a bounded graph-memory result.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "MemoryGraphSummary")]
pub struct GraphqlMemoryGraphSummary {
    /// Number of claim edges returned.
    pub returned_claim_count: i64,
    /// Number of entity nodes returned.
    pub returned_node_count: i64,
    /// Effective result limit.
    pub limit: i32,
    /// Whether the bounded candidate read window hit the effective limit.
    pub truncated: bool,
}

impl From<MemoryGraph> for GraphqlMemoryGraph {
    fn from(graph: MemoryGraph) -> Self {
        let MemoryGraph {
            nodes,
            edges,
            summary,
        } = graph;
        let node_id_map = opaque_memory_graph_node_ids(&nodes);
        Self {
            nodes: nodes
                .into_iter()
                .map(|node| GraphqlMemoryGraphNode::from_store_node(node, &node_id_map))
                .collect(),
            edges: edges
                .into_iter()
                .map(|edge| GraphqlMemoryGraphEdge::from_store_edge(edge, &node_id_map))
                .collect(),
            summary: summary.into(),
        }
    }
}

fn opaque_memory_graph_node_ids(nodes: &[MemoryGraphNode]) -> HashMap<String, String> {
    let mut node_ids = nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    node_ids.sort();
    node_ids
        .into_iter()
        .enumerate()
        .map(|(index, node_id)| (node_id, format!("memory-node:{}", index + 1)))
        .collect()
}

fn mapped_memory_graph_node_id(
    node_id_map: &HashMap<String, String>,
    store_node_id: &str,
) -> String {
    node_id_map
        .get(store_node_id)
        .expect("memory graph node must exist in response node map")
        .clone()
}

fn mapped_memory_graph_edge_endpoint_id(
    node_id_map: &HashMap<String, String>,
    store_node_id: &str,
) -> String {
    node_id_map
        .get(store_node_id)
        .expect("memory graph edge endpoint must exist in returned nodes")
        .clone()
}

impl GraphqlMemoryGraphNode {
    fn from_store_node(node: MemoryGraphNode, node_id_map: &HashMap<String, String>) -> Self {
        let node_id = mapped_memory_graph_node_id(node_id_map, &node.node_id);
        Self {
            entity_id: node_id.clone(),
            node_id,
            label: node.label,
            entity_type: node.entity_type,
            redacted: false,
            claim_count: node.claim_count,
        }
    }
}

impl GraphqlMemoryGraphEdge {
    fn from_store_edge(edge: MemoryGraphEdge, node_id_map: &HashMap<String, String>) -> Self {
        Self {
            claim_id: edge.claim_id,
            source_node_id: mapped_memory_graph_edge_endpoint_id(node_id_map, &edge.source_node_id),
            target_node_id: mapped_memory_graph_edge_endpoint_id(node_id_map, &edge.target_node_id),
            predicate_id: edge.predicate_id,
            predicate_label: edge.predicate_label,
            fact: edge.fact,
            fact_redacted: false,
            status: claim_status_label(edge.status).to_string(),
            sensitivity: sensitivity_label(edge.sensitivity).to_string(),
            confidence: edge.confidence,
            evidence_count: edge.evidence_count,
            created_at: edge.created_at,
            updated_at: edge.updated_at,
        }
    }
}

impl From<MemoryGraphSummary> for GraphqlMemoryGraphSummary {
    fn from(summary: MemoryGraphSummary) -> Self {
        Self {
            returned_claim_count: summary.returned_claim_count,
            returned_node_count: summary.returned_node_count,
            limit: i32::try_from(summary.limit).unwrap_or(i32::MAX),
            truncated: summary.truncated,
        }
    }
}

pub(super) async fn memory_claims(
    state: &GraphqlState,
    query: Option<String>,
    status: Option<String>,
    predicate_id: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<GraphqlMemoryClaim>> {
    let limit = match limit {
        Some(value) if value < 1 => {
            return Err(async_graphql::Error::new(
                "memoryClaims limit must be at least 1",
            ));
        }
        Some(value) => Some(
            usize::try_from(value)
                .map_err(|_| async_graphql::Error::new("memoryClaims limit is too large"))?,
        ),
        None => None,
    };
    let status = status
        .as_deref()
        .map(parse_graphql_claim_status)
        .transpose()?;
    let claims = state
        .store()?
        .list_claims(crate::MemoryClaimFilter {
            query,
            status,
            predicate_id,
            limit,
        })
        .await
        .map_err(graphql_error)?;

    Ok(claims.into_iter().map(Into::into).collect())
}

pub(super) async fn memory_claim(
    state: &GraphqlState,
    claim_id: String,
) -> Result<Option<GraphqlMemoryClaimDetail>> {
    let detail = state
        .store()?
        .get_claim_detail(&claim_id)
        .await
        .map_err(graphql_error)?;
    Ok(detail.map(Into::into))
}

pub(super) async fn memory_predicate_proposals(
    state: &GraphqlState,
    status: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<GraphqlPredicateProposal>> {
    let limit = match limit {
        Some(value) if value < 1 => {
            return Err(async_graphql::Error::new(
                "memoryPredicateProposals limit must be at least 1",
            ));
        }
        Some(value) => Some(usize::try_from(value).map_err(|_| {
            async_graphql::Error::new("memoryPredicateProposals limit is too large")
        })?),
        None => None,
    };
    let proposals = state
        .store()?
        .list_predicate_proposals(crate::PredicateProposalFilter { status, limit })
        .await
        .map_err(graphql_error)?;

    Ok(proposals.into_iter().map(Into::into).collect())
}

pub(super) async fn memory_predicate_proposal(
    state: &GraphqlState,
    proposal_id: String,
) -> Result<Option<GraphqlPredicateProposal>> {
    let proposal = state
        .store()?
        .get_predicate_proposal(&proposal_id)
        .await
        .map_err(graphql_error)?;
    Ok(proposal.map(Into::into))
}

pub(super) async fn memory_graph(
    state: &GraphqlState,
    input: Option<GraphqlMemoryGraphInput>,
) -> Result<GraphqlMemoryGraph> {
    let input = input.unwrap_or_default();
    let limit = parse_memory_graph_limit(input.limit)?;
    let statuses = input
        .statuses
        .map(|statuses| {
            if statuses.is_empty() {
                return Err(async_graphql::Error::new(
                    "memoryGraph statuses must not be empty",
                ));
            }
            statuses
                .iter()
                .map(|status| parse_graphql_claim_status(status))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    let sensitivity = input
        .sensitivity
        .as_deref()
        .map(parse_graphql_sensitivity)
        .transpose()?;
    let graph = state
        .store()?
        .memory_graph(crate::MemoryGraphFilter {
            query: input.query,
            statuses,
            predicate_id: input.predicate_id,
            sensitivity,
            limit,
        })
        .await
        .map_err(graphql_error)?;

    Ok(graph.into())
}

fn parse_graphql_claim_status(value: &str) -> Result<crate::ClaimStatus> {
    match value {
        "candidate" => Ok(crate::ClaimStatus::Candidate),
        "active" => Ok(crate::ClaimStatus::Active),
        "confirmed" => Ok(crate::ClaimStatus::Confirmed),
        "disputed" => Ok(crate::ClaimStatus::Disputed),
        "superseded" => Ok(crate::ClaimStatus::Superseded),
        "archived" => Ok(crate::ClaimStatus::Archived),
        "deleted" => Ok(crate::ClaimStatus::Deleted),
        _ => Err(async_graphql::Error::new(format!(
            "unknown memory claim status: {value}"
        ))),
    }
}

fn parse_graphql_sensitivity(value: &str) -> Result<crate::memory::Sensitivity> {
    match value {
        "public" => Ok(crate::memory::Sensitivity::Public),
        "normal" => Ok(crate::memory::Sensitivity::Normal),
        "private" => Ok(crate::memory::Sensitivity::Private),
        "sensitive" => Ok(crate::memory::Sensitivity::Sensitive),
        "secret" => Ok(crate::memory::Sensitivity::Secret),
        _ => Err(async_graphql::Error::new(format!(
            "unknown memory sensitivity: {value}"
        ))),
    }
}

fn parse_memory_graph_limit(limit: Option<i32>) -> Result<Option<usize>> {
    match limit {
        Some(value) if value < 1 => Err(async_graphql::Error::new(
            "memoryGraph limit must be at least 1",
        )),
        Some(value) if value > 500 => Err(async_graphql::Error::new(
            "memoryGraph limit must be at most 500",
        )),
        Some(value) => Ok(Some(usize::try_from(value).map_err(|_| {
            async_graphql::Error::new("memoryGraph limit is too large")
        })?)),
        None => Ok(None),
    }
}
