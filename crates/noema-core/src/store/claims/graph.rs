use std::collections::HashMap;

use crate::memory::Sensitivity;
use crate::store::{NoemaStore, StoreError};

use super::{
    inspection::memory_claim_record,
    labels::default_memory_graph_statuses,
    model::{MemoryGraph, MemoryGraphEdge, MemoryGraphFilter, MemoryGraphNode, MemoryGraphSummary},
    rows::InspectionClaimRow,
};

impl NoemaStore {
    /// Return a bounded memory graph read model for memory-management inspection.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when stored enum data is invalid or the embedded
    /// store read fails.
    pub async fn memory_graph(&self, filter: MemoryGraphFilter) -> Result<MemoryGraph, StoreError> {
        let query = filter
            .query
            .as_deref()
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(str::to_ascii_lowercase);
        let statuses = filter
            .statuses
            .filter(|statuses| !statuses.is_empty())
            .unwrap_or_else(default_memory_graph_statuses);
        let status_values = statuses
            .iter()
            .map(|status| status.as_str().to_string())
            .collect::<Vec<_>>();
        let limit = clamp_memory_graph_limit(filter.limit);
        let fetch_limit = limit.saturating_add(1);

        let mut sql = String::from(
            r#"
            SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
              status, sensitivity, confidence, created_at, updated_at
            FROM claims
            WHERE status IN $statuses
            "#,
        );
        if filter.predicate_id.is_some() {
            sql.push_str("AND predicate_id = $predicate_id\n");
        }
        if filter.sensitivity.is_some() {
            sql.push_str("AND sensitivity = $sensitivity\n");
        }
        sql.push_str(
            r#"
            ORDER BY created_at DESC, claim_id ASC
            "#,
        );
        sql.push_str("LIMIT $limit\n");
        sql.push(';');

        let mut statement = self
            .db()
            .query(sql)
            .bind(("statuses", status_values))
            .bind(("limit", fetch_limit));
        if let Some(predicate_id) = filter.predicate_id {
            statement = statement.bind(("predicate_id", predicate_id));
        }
        if let Some(sensitivity) = filter.sensitivity {
            statement = statement.bind(("sensitivity", sensitivity.as_str().to_string()));
        }
        let mut response = statement.await?;
        let rows: Vec<InspectionClaimRow> = response.take(0)?;
        let predicates = self.inspection_predicates_for_claims(&rows).await?;
        let entities = self.inspection_entities_for_claims(&rows).await?;
        let evidence_counts = self.inspection_evidence_counts_for_claims(&rows).await?;
        let mut claims = Vec::new();
        let mut truncated = rows.len() > limit;

        for row in rows {
            let claim = memory_claim_record(row, &predicates, &entities, &evidence_counts)?;
            if let Some(query) = query.as_deref()
                && !claim.matches_graph_query(query)
            {
                continue;
            }
            if claims.len() >= limit {
                truncated = true;
                break;
            }
            claims.push(claim);
        }

        let mut node_map = HashMap::<String, MemoryGraphNode>::new();
        let mut edges = Vec::new();

        for claim in claims {
            let source_node_id = entity_node_id(&claim.subject_entity_id);
            let target_entity_id = claim
                .object_entity_id
                .as_deref()
                .unwrap_or(&claim.subject_entity_id);
            let target_node_id = entity_node_id(target_entity_id);

            upsert_memory_graph_node(
                &mut node_map,
                &source_node_id,
                &claim.subject_entity_id,
                &claim.subject_entity_name,
                &claim.subject_entity_type,
                claim.sensitivity,
            );
            if let (Some(object_entity_id), Some(object_name), Some(object_type)) = (
                claim.object_entity_id.as_deref(),
                claim.object_entity_name.as_deref(),
                claim.object_entity_type.as_deref(),
            ) {
                upsert_memory_graph_node(
                    &mut node_map,
                    &target_node_id,
                    object_entity_id,
                    object_name,
                    object_type,
                    claim.sensitivity,
                );
            }

            edges.push(MemoryGraphEdge {
                claim_id: claim.claim_id,
                source_node_id,
                target_node_id,
                predicate_id: claim.predicate_id,
                predicate_label: claim.predicate_label,
                fact: claim.fact,
                status: claim.status,
                sensitivity: claim.sensitivity,
                confidence: claim.confidence,
                evidence_count: claim.evidence_count,
                created_at: claim.created_at,
                updated_at: claim.updated_at,
            });
        }

        let mut nodes = node_map.into_values().collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        edges.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));

        Ok(MemoryGraph {
            summary: MemoryGraphSummary {
                returned_claim_count: edges.len() as i64,
                returned_node_count: nodes.len() as i64,
                limit,
                truncated,
            },
            nodes,
            edges,
        })
    }
}

fn entity_node_id(entity_id: &str) -> String {
    format!("entity:{entity_id}")
}

const DEFAULT_MEMORY_GRAPH_LIMIT: usize = 150;
const MAX_MEMORY_GRAPH_LIMIT: usize = 500;

fn clamp_memory_graph_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_MEMORY_GRAPH_LIMIT)
        .clamp(1, MAX_MEMORY_GRAPH_LIMIT)
}

fn upsert_memory_graph_node(
    nodes: &mut HashMap<String, MemoryGraphNode>,
    node_id: &str,
    entity_id: &str,
    label: &str,
    entity_type: &str,
    sensitivity: Sensitivity,
) {
    nodes
        .entry(node_id.to_string())
        .and_modify(|node| {
            node.claim_count += 1;
            node.max_sensitivity = stricter_sensitivity(node.max_sensitivity, sensitivity);
        })
        .or_insert_with(|| MemoryGraphNode {
            node_id: node_id.to_string(),
            entity_id: entity_id.to_string(),
            label: label.to_string(),
            entity_type: entity_type.to_string(),
            max_sensitivity: sensitivity,
            claim_count: 1,
        });
}

fn stricter_sensitivity(left: Sensitivity, right: Sensitivity) -> Sensitivity {
    if sensitivity_rank(left) >= sensitivity_rank(right) {
        left
    } else {
        right
    }
}

const fn sensitivity_rank(sensitivity: Sensitivity) -> u8 {
    match sensitivity {
        Sensitivity::Public => 0,
        Sensitivity::Normal => 1,
        Sensitivity::Private => 2,
        Sensitivity::Sensitive => 3,
        Sensitivity::Secret => 4,
    }
}
