use crate::{
    context_graph::{
        ContextGraphFilter, ContextGraphSummary, GraphAccessGrant, GraphContextPacket,
        GraphContextPacketMemoryEdge, GraphContextPacketOmission, GraphEntityNode,
        GraphMemoryEvent, GraphMemoryNode, GraphMemoryUseRecord, GraphObjectLinkEdge,
        GraphParticipantEdge, GraphProvenanceEdge, GraphPurposeRule, GraphSubjectEdge,
        RelationshipSummary,
    },
    memory::{
        Effect, ExternalEgressPolicy, ParticipantRole, ParticipantVisibilityPolicy, Purpose,
        RelationshipStatus, RetrievalPolicyStatus, SubjectRole,
    },
    postgres_retrieval_policy_fingerprint,
};
use serde_json::Value;
use sqlx::Row;

use super::{error::MemoryPersistenceError, helpers::*, repository::PostgresMemoryRepository};

impl PostgresMemoryRepository {
    /// Inspect the persisted Postgres context graph for a run or context packet.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres reads fail or stored enum
    /// values are outside Noema's closed vocabularies.
    pub async fn inspect_context_graph_with_filter(
        &self,
        filter: &ContextGraphFilter,
        limit: Option<u32>,
    ) -> Result<ContextGraphSummary, MemoryPersistenceError> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let memories = self.postgres_graph_memory_nodes(limit, filter).await?;
        let entities = self.postgres_graph_entity_nodes(limit, filter).await?;
        let subject_edges = self.postgres_graph_subject_edges(limit, filter).await?;
        let participant_edges = self.postgres_graph_participant_edges(limit, filter).await?;
        let provenance_edges = self.postgres_graph_provenance_edges(limit, filter).await?;
        let object_link_edges = self.postgres_graph_object_link_edges(limit, filter).await?;
        let purpose_rules = self.postgres_graph_purpose_rules(limit, filter).await?;
        let access_grants = self.postgres_graph_access_grants(limit, filter).await?;
        let context_packets = self.postgres_graph_context_packets(limit, filter).await?;
        let context_packet_memory_edges = self
            .postgres_graph_context_packet_memory_edges(limit, filter)
            .await?;
        let context_packet_omissions = self
            .postgres_graph_context_packet_omissions(limit, filter)
            .await?;
        let memory_use_records = self
            .postgres_graph_memory_use_records(limit, filter)
            .await?;
        let object_events = self.postgres_graph_object_events(limit, filter).await?;
        let relationships = self
            .postgres_graph_relationship_edges(limit, filter)
            .await?;

        Ok(ContextGraphSummary {
            memories,
            entities,
            subject_edges,
            participant_edges,
            provenance_edges,
            object_link_edges,
            purpose_rules,
            access_grants,
            context_packets,
            context_packet_memory_edges,
            context_packet_omissions,
            memory_use_records,
            object_events,
            relationships,
        })
    }

    async fn postgres_graph_memory_nodes(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphMemoryNode>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id
              FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            ),
            packet_memory_ids AS (
              SELECT memory_id FROM context_packet_memory_edges
              WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION
              SELECT memory_id FROM context_packet_omissions
              WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                AND memory_id IS NOT NULL
              UNION
              SELECT memory_id FROM memory_use_records
              WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION
              SELECT memory_id FROM relationships
              WHERE relationship_id IN (
                SELECT relationship_id FROM context_packet_omissions
                WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                  AND relationship_id IS NOT NULL
              )
                AND memory_id IS NOT NULL
            ),
            inspected_memories AS (
              SELECT memory_id
              FROM memory_items
              WHERE ($2 IS NULL AND $3 IS NULL)
                 OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
              ORDER BY created_at DESC, memory_id DESC
              LIMIT $1
            )
            SELECT
              memory_id,
              status,
              memory_type,
              owner_object_type,
              owner_object_id,
              sensitivity,
              title,
              content,
              retrieval_hints::text AS retrieval_hints,
              retrieval_policy_status,
              retrieval_policy_version::bigint AS retrieval_policy_version,
              retrieval_policy_fingerprint,
              retrieval_policy_extractor_object_type,
              retrieval_policy_extractor_object_id,
              retrieval_policy_extractor_version,
              retrieval_policy_validated_at::text AS retrieval_policy_validated_at,
              participant_visibility_policy,
              external_egress_policy,
              created_at::text AS created_at
            FROM memory_items
            WHERE memory_id IN (SELECT memory_id FROM inspected_memories)
            ORDER BY created_at DESC, memory_id DESC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        let mut memories = Vec::with_capacity(rows.len());
        for row in rows {
            let memory_id: String = row.try_get("memory_id")?;
            let stored_policy_status = parse_retrieval_policy_status(
                row.try_get::<String, _>("retrieval_policy_status")?
                    .as_str(),
            )?;
            let stored_fingerprint: Option<String> = row.try_get("retrieval_policy_fingerprint")?;
            let effective_policy_status = self
                .postgres_effective_retrieval_policy_status(
                    &memory_id,
                    stored_policy_status,
                    stored_fingerprint.as_deref(),
                )
                .await?;
            memories.push(GraphMemoryNode {
                memory_id,
                status: parse_memory_status(row.try_get::<String, _>("status")?.as_str())?,
                memory_type: parse_memory_type(row.try_get::<String, _>("memory_type")?.as_str())?,
                owner_object_type: row.try_get("owner_object_type")?,
                owner_object_id: row.try_get("owner_object_id")?,
                sensitivity: parse_sensitivity(row.try_get::<String, _>("sensitivity")?.as_str())?,
                title: row.try_get("title")?,
                content: row.try_get("content")?,
                retrieval_hints: row.try_get("retrieval_hints")?,
                retrieval_policy_status: stored_policy_status,
                retrieval_policy_effective_status: effective_policy_status,
                retrieval_policy_version: row.try_get("retrieval_policy_version")?,
                retrieval_policy_fingerprint: stored_fingerprint,
                retrieval_policy_extractor_object_type: row
                    .try_get("retrieval_policy_extractor_object_type")?,
                retrieval_policy_extractor_object_id: row
                    .try_get("retrieval_policy_extractor_object_id")?,
                retrieval_policy_extractor_version: row
                    .try_get("retrieval_policy_extractor_version")?,
                retrieval_policy_validated_at: row.try_get("retrieval_policy_validated_at")?,
                participant_visibility_policy: parse_participant_visibility_policy(
                    row.try_get::<String, _>("participant_visibility_policy")?
                        .as_str(),
                )?,
                external_egress_policy: parse_external_egress_policy(
                    row.try_get::<String, _>("external_egress_policy")?.as_str(),
                )?,
                created_at: row.try_get("created_at")?,
            });
        }
        Ok(memories)
    }

    async fn postgres_graph_entity_nodes(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphEntityNode>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id
              FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            ),
            selected_relationships AS (
              SELECT relationship_id FROM context_packet_omissions
              WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                AND relationship_id IS NOT NULL
            ),
            packet_memory_ids AS (
              SELECT memory_id FROM context_packet_memory_edges
              WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION
              SELECT memory_id FROM context_packet_omissions
              WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                AND memory_id IS NOT NULL
              UNION
              SELECT memory_id FROM memory_use_records
              WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION
              SELECT memory_id FROM relationships
              WHERE relationship_id IN (SELECT relationship_id FROM selected_relationships)
                AND memory_id IS NOT NULL
            ),
            inspected_memories AS (
              SELECT memory_id
              FROM memory_items
              WHERE ($2 IS NULL AND $3 IS NULL)
                 OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
              ORDER BY created_at DESC, memory_id DESC
              LIMIT $1
            )
            SELECT DISTINCT
              e.entity_id,
              e.entity_type,
              e.owner_object_type,
              e.owner_object_id,
              e.canonical_name,
              e.linked_object_type,
              e.linked_object_id
            FROM entities e
            WHERE (
                $2 IS NULL
                AND $3 IS NULL
                AND e.entity_id IN (
                  SELECT entity_id FROM memory_subjects
                  UNION
                  SELECT subject_entity_id FROM relationships
                  UNION
                  SELECT object_entity_id FROM relationships
                )
              )
              OR e.entity_id IN (
                SELECT entity_id FROM memory_subjects
                WHERE memory_id IN (SELECT memory_id FROM inspected_memories)
                UNION
                SELECT subject_entity_id FROM relationships
                WHERE memory_id IN (SELECT memory_id FROM inspected_memories)
                   OR relationship_id IN (SELECT relationship_id FROM selected_relationships)
                UNION
                SELECT object_entity_id FROM relationships
                WHERE memory_id IN (SELECT memory_id FROM inspected_memories)
                   OR relationship_id IN (SELECT relationship_id FROM selected_relationships)
              )
            ORDER BY e.created_at DESC, e.entity_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphEntityNode {
                    entity_id: row.try_get("entity_id")?,
                    entity_type: row.try_get("entity_type")?,
                    owner_object_type: row.try_get("owner_object_type")?,
                    owner_object_id: row.try_get("owner_object_id")?,
                    canonical_name: row.try_get("canonical_name")?,
                    linked_object_type: row.try_get("linked_object_type")?,
                    linked_object_id: row.try_get("linked_object_id")?,
                })
            })
            .collect()
    }

    async fn postgres_graph_subject_edges(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphSubjectEdge>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            ),
            packet_memory_ids AS (
              SELECT memory_id FROM context_packet_memory_edges WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM context_packet_omissions WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets) AND memory_id IS NOT NULL
              UNION SELECT memory_id FROM memory_use_records WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM relationships
              WHERE relationship_id IN (
                SELECT relationship_id FROM context_packet_omissions
                WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                  AND relationship_id IS NOT NULL
              )
                AND memory_id IS NOT NULL
            ),
            inspected_memories AS (
              SELECT memory_id FROM memory_items
              WHERE ($2 IS NULL AND $3 IS NULL) OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
              ORDER BY created_at DESC, memory_id DESC
              LIMIT $1
            )
            SELECT ms.memory_id, ms.entity_id, ms.role
            FROM memory_subjects ms
            JOIN memory_items mi ON mi.memory_id = ms.memory_id
            WHERE ($2 IS NULL AND $3 IS NULL)
               OR ms.memory_id IN (SELECT memory_id FROM inspected_memories)
            ORDER BY mi.created_at DESC, ms.memory_id ASC, ms.entity_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphSubjectEdge {
                    memory_id: row.try_get("memory_id")?,
                    entity_id: row.try_get("entity_id")?,
                    role: parse_subject_role(row.try_get::<String, _>("role")?.as_str())?,
                })
            })
            .collect()
    }

    async fn postgres_graph_participant_edges(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphParticipantEdge>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            ),
            packet_memory_ids AS (
              SELECT memory_id FROM context_packet_memory_edges WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM context_packet_omissions WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets) AND memory_id IS NOT NULL
              UNION SELECT memory_id FROM memory_use_records WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM relationships
              WHERE relationship_id IN (
                SELECT relationship_id FROM context_packet_omissions
                WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                  AND relationship_id IS NOT NULL
              )
                AND memory_id IS NOT NULL
            ),
            inspected_memories AS (
              SELECT memory_id FROM memory_items
              WHERE ($2 IS NULL AND $3 IS NULL) OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
              ORDER BY created_at DESC, memory_id DESC
              LIMIT $1
            )
            SELECT mp.memory_id, mp.participant_object_type, mp.participant_object_id, mp.role
            FROM memory_participants mp
            JOIN memory_items mi ON mi.memory_id = mp.memory_id
            WHERE ($2 IS NULL AND $3 IS NULL)
               OR mp.memory_id IN (SELECT memory_id FROM inspected_memories)
            ORDER BY mi.created_at DESC, mp.memory_id ASC, mp.participant_object_type ASC, mp.participant_object_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphParticipantEdge {
                    memory_id: row.try_get("memory_id")?,
                    participant_object_type: row.try_get("participant_object_type")?,
                    participant_object_id: row.try_get("participant_object_id")?,
                    role: parse_participant_role(row.try_get::<String, _>("role")?.as_str())?,
                })
            })
            .collect()
    }

    async fn postgres_graph_provenance_edges(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphProvenanceEdge>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            ),
            packet_memory_ids AS (
              SELECT memory_id FROM context_packet_memory_edges WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM context_packet_omissions WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets) AND memory_id IS NOT NULL
              UNION SELECT memory_id FROM memory_use_records WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM relationships
              WHERE relationship_id IN (
                SELECT relationship_id FROM context_packet_omissions
                WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                  AND relationship_id IS NOT NULL
              )
                AND memory_id IS NOT NULL
            ),
            inspected_memories AS (
              SELECT memory_id FROM memory_items
              WHERE ($2 IS NULL AND $3 IS NULL) OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
              ORDER BY created_at DESC, memory_id DESC
              LIMIT $1
            )
            SELECT target_object_id AS memory_id, source_object_type, source_object_id, relation, evidence_excerpt
            FROM object_provenance_edges
            WHERE (($2 IS NULL AND $3 IS NULL) OR target_object_id IN (SELECT memory_id FROM inspected_memories))
              AND target_object_type = 'memory_item'
              AND deleted_at IS NULL
            ORDER BY created_at DESC, edge_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphProvenanceEdge {
                    memory_id: row.try_get("memory_id")?,
                    source_object_type: row.try_get("source_object_type")?,
                    source_object_id: row.try_get("source_object_id")?,
                    relation: row.try_get("relation")?,
                    evidence_excerpt: row.try_get("evidence_excerpt")?,
                })
            })
            .collect()
    }

    async fn postgres_graph_object_link_edges(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphObjectLinkEdge>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            ),
            packet_memory_ids AS (
              SELECT memory_id FROM context_packet_memory_edges WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM context_packet_omissions WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets) AND memory_id IS NOT NULL
              UNION SELECT memory_id FROM memory_use_records WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM relationships
              WHERE relationship_id IN (
                SELECT relationship_id FROM context_packet_omissions
                WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                  AND relationship_id IS NOT NULL
              )
                AND memory_id IS NOT NULL
            ),
            inspected_memories AS (
              SELECT memory_id FROM memory_items
              WHERE ($2 IS NULL AND $3 IS NULL) OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
              ORDER BY created_at DESC, memory_id DESC
              LIMIT $1
            )
            SELECT
              link.memory_id,
              link.object_type,
              link.object_id,
              link.relation,
              link.authorized_object_type,
              link.authorized_object_id,
              link.resolver_object_type,
              link.resolver_object_id,
              link.resolver_version,
              link.source_run_id,
              link.created_at::text AS created_at
            FROM memory_retrieval_object_links link
            JOIN memory_items mi ON mi.memory_id = link.memory_id
            WHERE ($2 IS NULL AND $3 IS NULL)
               OR link.memory_id IN (SELECT memory_id FROM inspected_memories)
            ORDER BY mi.created_at DESC, link.memory_id ASC, link.object_type ASC, link.object_id ASC, link.relation ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphObjectLinkEdge {
                    memory_id: row.try_get("memory_id")?,
                    object_type: row.try_get("object_type")?,
                    object_id: row.try_get("object_id")?,
                    relation: row.try_get("relation")?,
                    authorized_object_type: row.try_get("authorized_object_type")?,
                    authorized_object_id: row.try_get("authorized_object_id")?,
                    resolver_object_type: row.try_get("resolver_object_type")?,
                    resolver_object_id: row.try_get("resolver_object_id")?,
                    resolver_version: row.try_get("resolver_version")?,
                    source_run_id: row.try_get("source_run_id")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    async fn postgres_graph_purpose_rules(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphPurposeRule>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            ),
            packet_memory_ids AS (
              SELECT memory_id FROM context_packet_memory_edges WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM context_packet_omissions WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets) AND memory_id IS NOT NULL
              UNION SELECT memory_id FROM memory_use_records WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM relationships
              WHERE relationship_id IN (
                SELECT relationship_id FROM context_packet_omissions
                WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                  AND relationship_id IS NOT NULL
              )
                AND memory_id IS NOT NULL
            ),
            inspected_memories AS (
              SELECT memory_id FROM memory_items
              WHERE ($2 IS NULL AND $3 IS NULL) OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
              ORDER BY created_at DESC, memory_id DESC
              LIMIT $1
            )
            SELECT rule.memory_id, rule.purpose, rule.effect, rule.created_by_object_type, rule.created_by_object_id, rule.created_at::text AS created_at
            FROM memory_retrieval_purpose_rules rule
            JOIN memory_items mi ON mi.memory_id = rule.memory_id
            WHERE ($2 IS NULL AND $3 IS NULL)
               OR rule.memory_id IN (SELECT memory_id FROM inspected_memories)
            ORDER BY mi.created_at DESC, rule.memory_id ASC, rule.purpose ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphPurposeRule {
                    memory_id: row.try_get("memory_id")?,
                    purpose: parse_purpose(row.try_get::<String, _>("purpose")?.as_str())?,
                    effect: parse_effect(row.try_get::<String, _>("effect")?.as_str())?,
                    created_by_object_type: row.try_get("created_by_object_type")?,
                    created_by_object_id: row.try_get("created_by_object_id")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    async fn postgres_graph_access_grants(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphAccessGrant>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            ),
            packet_memory_ids AS (
              SELECT memory_id FROM context_packet_memory_edges WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM context_packet_omissions WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets) AND memory_id IS NOT NULL
              UNION SELECT memory_id FROM memory_use_records WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM relationships
              WHERE relationship_id IN (
                SELECT relationship_id FROM context_packet_omissions
                WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                  AND relationship_id IS NOT NULL
              )
                AND memory_id IS NOT NULL
            ),
            inspected_memories AS (
              SELECT memory_id, owner_object_type, owner_object_id FROM memory_items
              WHERE ($2 IS NULL AND $3 IS NULL) OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
              ORDER BY created_at DESC, memory_id DESC
              LIMIT $1
            )
            SELECT
              grant.grant_id,
              grant.target_object_type,
              grant.target_object_id,
              grant.grantee_object_type,
              grant.grantee_object_id,
              grant.permission,
              grant.effect,
              grant.expires_at::text AS expires_at,
              grant.created_by_object_type,
              grant.created_by_object_id,
              grant.created_at::text AS created_at
            FROM object_access_grants grant
            WHERE (
                grant.target_object_type = 'memory_item'
                AND grant.target_object_id IN (SELECT memory_id FROM inspected_memories)
              )
               OR (
                 $2 IS NULL
                 AND $3 IS NULL
                 AND EXISTS (
                   SELECT 1 FROM inspected_memories inspected
                   WHERE inspected.owner_object_type = grant.target_object_type
                     AND inspected.owner_object_id = grant.target_object_id
                 )
               )
            ORDER BY grant.created_at DESC, grant.grant_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphAccessGrant {
                    grant_id: row.try_get("grant_id")?,
                    target_object_type: row.try_get("target_object_type")?,
                    target_object_id: row.try_get("target_object_id")?,
                    grantee_object_type: row.try_get("grantee_object_type")?,
                    grantee_object_id: row.try_get("grantee_object_id")?,
                    permission: row.try_get("permission")?,
                    effect: parse_effect(row.try_get::<String, _>("effect")?.as_str())?,
                    expires_at: row.try_get("expires_at")?,
                    created_by_object_type: row.try_get("created_by_object_type")?,
                    created_by_object_id: row.try_get("created_by_object_id")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    async fn postgres_graph_context_packets(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphContextPacket>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id
              FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            )
            SELECT
              context_packet_id,
              run_id,
              requesting_object_type,
              requesting_object_id,
              purpose,
              active_objects,
              agent_visible_omissions,
              created_at::text AS created_at
            FROM context_packets
            WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
            ORDER BY created_at DESC, context_packet_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphContextPacket {
                    context_packet_id: row.try_get("context_packet_id")?,
                    run_id: row.try_get("run_id")?,
                    requesting_object_type: row.try_get("requesting_object_type")?,
                    requesting_object_id: row.try_get("requesting_object_id")?,
                    purpose: parse_purpose(row.try_get::<String, _>("purpose")?.as_str())?,
                    active_objects: json_value_to_string(row.try_get("active_objects")?)?,
                    agent_visible_omissions: json_value_to_string(
                        row.try_get("agent_visible_omissions")?,
                    )?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    async fn postgres_graph_context_packet_memory_edges(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphContextPacketMemoryEdge>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            )
            SELECT
              edge.packet_memory_id,
              edge.context_packet_id,
              edge.memory_id,
              memory.sensitivity,
              edge.stage,
              edge.rank_score::bigint AS rank_score,
              edge.eligibility_reason,
              edge.rank_reasons,
              edge.created_at::text AS created_at
            FROM context_packet_memory_edges edge
            JOIN memory_items memory ON memory.memory_id = edge.memory_id
            WHERE edge.context_packet_id IN (SELECT context_packet_id FROM selected_packets)
            ORDER BY edge.created_at DESC, edge.packet_memory_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphContextPacketMemoryEdge {
                    packet_memory_id: row.try_get("packet_memory_id")?,
                    context_packet_id: row.try_get("context_packet_id")?,
                    memory_id: row.try_get("memory_id")?,
                    memory_sensitivity: parse_sensitivity(
                        row.try_get::<String, _>("sensitivity")?.as_str(),
                    )?,
                    stage: row.try_get("stage")?,
                    rank_score: row.try_get("rank_score")?,
                    eligibility_reason: row.try_get("eligibility_reason")?,
                    rank_reasons: json_value_to_string(row.try_get("rank_reasons")?)?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    async fn postgres_graph_context_packet_omissions(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphContextPacketOmission>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            )
            SELECT omission_id, context_packet_id, memory_id, relationship_id,
                   omission_sensitivity, agent_visible_reason, audit_reason,
                   created_at::text AS created_at, details
            FROM context_packet_omissions
            WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
            ORDER BY created_at DESC, omission_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphContextPacketOmission {
                    omission_id: row.try_get("omission_id")?,
                    context_packet_id: row.try_get("context_packet_id")?,
                    memory_id: row.try_get("memory_id")?,
                    relationship_id: row.try_get("relationship_id")?,
                    omission_sensitivity: parse_sensitivity(
                        row.try_get::<String, _>("omission_sensitivity")?.as_str(),
                    )?,
                    agent_visible_reason: row.try_get("agent_visible_reason")?,
                    audit_reason: row.try_get("audit_reason")?,
                    created_at: row.try_get("created_at")?,
                    details: json_value_to_string(row.try_get("details")?)?,
                })
            })
            .collect()
    }

    async fn postgres_graph_memory_use_records(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphMemoryUseRecord>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            )
            SELECT use_record.memory_use_id, use_record.context_packet_id,
                   use_record.run_id, use_record.memory_id, memory.sensitivity,
                   use_record.stage, use_record.agent_object_type,
                   use_record.agent_object_id, use_record.context_object_type,
                   use_record.context_object_id, use_record.purpose,
                   use_record.used_for_object_type, use_record.used_for_object_id,
                   use_record.policy_decision_id, use_record.created_at::text AS created_at,
                   use_record.details
            FROM memory_use_records use_record
            JOIN memory_items memory ON memory.memory_id = use_record.memory_id
            WHERE use_record.context_packet_id IN (SELECT context_packet_id FROM selected_packets)
               OR ($2 IS NULL AND $3 IS NULL AND use_record.context_packet_id IS NULL)
            ORDER BY use_record.created_at DESC, use_record.memory_use_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(GraphMemoryUseRecord {
                    memory_use_id: row.try_get("memory_use_id")?,
                    context_packet_id: row.try_get("context_packet_id")?,
                    run_id: row.try_get("run_id")?,
                    memory_id: row.try_get("memory_id")?,
                    memory_sensitivity: parse_sensitivity(
                        row.try_get::<String, _>("sensitivity")?.as_str(),
                    )?,
                    stage: row.try_get("stage")?,
                    agent_object_type: row.try_get("agent_object_type")?,
                    agent_object_id: row.try_get("agent_object_id")?,
                    context_object_type: row.try_get("context_object_type")?,
                    context_object_id: row.try_get("context_object_id")?,
                    purpose: parse_purpose(row.try_get::<String, _>("purpose")?.as_str())?,
                    used_for_object_type: row.try_get("used_for_object_type")?,
                    used_for_object_id: row.try_get("used_for_object_id")?,
                    policy_decision_id: row.try_get("policy_decision_id")?,
                    created_at: row.try_get("created_at")?,
                    details: json_value_to_string(row.try_get("details")?)?,
                })
            })
            .collect()
    }

    async fn postgres_graph_object_events(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<GraphMemoryEvent>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            ),
            packet_memory_ids AS (
              SELECT memory_id FROM context_packet_memory_edges WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM context_packet_omissions WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets) AND memory_id IS NOT NULL
              UNION SELECT memory_id FROM memory_use_records WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM relationships
              WHERE relationship_id IN (
                SELECT relationship_id FROM context_packet_omissions
                WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                  AND relationship_id IS NOT NULL
              )
                AND memory_id IS NOT NULL
            ),
            inspected_memories AS (
              SELECT memory_id, owner_object_type, owner_object_id FROM memory_items
              WHERE ($2 IS NULL AND $3 IS NULL) OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
              ORDER BY created_at DESC, memory_id DESC
              LIMIT $1
            )
            SELECT event.event_id, event.event_type, event.actor_object_type,
                   event.actor_object_id, event.target_object_type, event.target_object_id,
                   memory.sensitivity AS target_memory_sensitivity, event.reason,
                   event.created_at::text AS created_at, event.details
            FROM object_events event
            LEFT JOIN memory_items memory
              ON event.target_object_type = 'memory_item'
             AND event.target_object_id = memory.memory_id
            WHERE (
                event.target_object_type = 'memory_item'
                AND event.target_object_id IN (SELECT memory_id FROM inspected_memories)
              )
              OR (
                $2 IS NULL AND $3 IS NULL
                AND EXISTS (
                  SELECT 1 FROM inspected_memories inspected
                  WHERE inspected.owner_object_type = event.target_object_type
                    AND inspected.owner_object_id = event.target_object_id
                )
              )
            ORDER BY event.created_at DESC, event.event_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                let sensitivity: Option<String> = row.try_get("target_memory_sensitivity")?;
                Ok(GraphMemoryEvent {
                    event_id: row.try_get("event_id")?,
                    event_type: row.try_get("event_type")?,
                    actor_object_type: row.try_get("actor_object_type")?,
                    actor_object_id: row.try_get("actor_object_id")?,
                    target_object_type: row.try_get("target_object_type")?,
                    target_object_id: row.try_get("target_object_id")?,
                    target_memory_sensitivity: sensitivity
                        .as_deref()
                        .map(parse_sensitivity)
                        .transpose()?,
                    reason: row.try_get("reason")?,
                    created_at: row.try_get("created_at")?,
                    details: json_value_to_string(row.try_get("details")?)?,
                })
            })
            .collect()
    }

    async fn postgres_graph_relationship_edges(
        &self,
        limit: u32,
        filter: &ContextGraphFilter,
    ) -> Result<Vec<RelationshipSummary>, MemoryPersistenceError> {
        let rows = sqlx::query(
            r"
            WITH selected_packets AS (
              SELECT context_packet_id FROM context_packets
              WHERE ($2 IS NULL OR run_id = $2)
                AND ($3 IS NULL OR context_packet_id = $3)
              ORDER BY created_at DESC, context_packet_id ASC
              LIMIT $1
            ),
            selected_relationships AS (
              SELECT relationship_id FROM context_packet_omissions
              WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
                AND relationship_id IS NOT NULL
            ),
            packet_memory_ids AS (
              SELECT memory_id FROM context_packet_memory_edges WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM context_packet_omissions WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets) AND memory_id IS NOT NULL
              UNION SELECT memory_id FROM memory_use_records WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
              UNION SELECT memory_id FROM relationships WHERE relationship_id IN (SELECT relationship_id FROM selected_relationships) AND memory_id IS NOT NULL
            ),
            inspected_memories AS (
              SELECT memory_id FROM memory_items
              WHERE ($2 IS NULL AND $3 IS NULL) OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
              ORDER BY created_at DESC, memory_id DESC
              LIMIT $1
            )
            SELECT r.relationship_id, r.owner_object_type, r.owner_object_id,
                   r.subject_entity_id, subject.canonical_name AS subject_name,
                   r.predicate, r.object_entity_id, object.canonical_name AS object_name,
                   r.memory_id, r.status, r.confidence, r.created_at::text AS created_at
            FROM relationships r
            LEFT JOIN entities subject ON subject.entity_id = r.subject_entity_id
            LEFT JOIN entities object ON object.entity_id = r.object_entity_id
            WHERE ($2 IS NULL AND $3 IS NULL)
               OR r.memory_id IN (SELECT memory_id FROM inspected_memories)
               OR r.relationship_id IN (SELECT relationship_id FROM selected_relationships)
            ORDER BY r.created_at DESC, r.relationship_id ASC
            LIMIT $1
            ",
        )
        .bind(i64::from(limit))
        .bind(filter.run_id.as_deref())
        .bind(filter.context_packet_id.as_deref())
        .fetch_all(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(|row| {
                Ok(RelationshipSummary {
                    relationship_id: row.try_get("relationship_id")?,
                    owner_object_type: row.try_get("owner_object_type")?,
                    owner_object_id: row.try_get("owner_object_id")?,
                    subject_entity_id: row.try_get("subject_entity_id")?,
                    subject_name: row.try_get("subject_name")?,
                    predicate: row.try_get("predicate")?,
                    object_entity_id: row.try_get("object_entity_id")?,
                    object_name: row.try_get("object_name")?,
                    memory_id: row.try_get("memory_id")?,
                    status: parse_relationship_status(
                        row.try_get::<String, _>("status")?.as_str(),
                    )?,
                    confidence: row.try_get("confidence")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    async fn postgres_effective_retrieval_policy_status(
        &self,
        memory_id: &str,
        stored_status: RetrievalPolicyStatus,
        stored_fingerprint: Option<&str>,
    ) -> Result<RetrievalPolicyStatus, MemoryPersistenceError> {
        if stored_status != RetrievalPolicyStatus::Valid {
            return Ok(stored_status);
        }

        let Some(stored_fingerprint) = stored_fingerprint else {
            return Ok(RetrievalPolicyStatus::Stale);
        };

        if postgres_retrieval_policy_fingerprint::current_fingerprint(self.pool(), memory_id)
            .await?
            == stored_fingerprint
        {
            Ok(RetrievalPolicyStatus::Valid)
        } else {
            Ok(RetrievalPolicyStatus::Stale)
        }
    }
}

fn json_value_to_string(value: Value) -> Result<String, MemoryPersistenceError> {
    serde_json::to_string(&value).map_err(MemoryPersistenceError::Json)
}

fn parse_retrieval_policy_status(
    value: &str,
) -> Result<RetrievalPolicyStatus, MemoryPersistenceError> {
    match value {
        "valid" => Ok(RetrievalPolicyStatus::Valid),
        "stale" => Ok(RetrievalPolicyStatus::Stale),
        "invalid" => Ok(RetrievalPolicyStatus::Invalid),
        "needs_review" => Ok(RetrievalPolicyStatus::NeedsReview),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "retrieval policy status",
            value: value.to_string(),
        }),
    }
}

fn parse_participant_visibility_policy(
    value: &str,
) -> Result<ParticipantVisibilityPolicy, MemoryPersistenceError> {
    match value {
        "any_active_human" => Ok(ParticipantVisibilityPolicy::AnyActiveHuman),
        "all_original_humans" => Ok(ParticipantVisibilityPolicy::AllOriginalHumans),
        "owner_only" => Ok(ParticipantVisibilityPolicy::OwnerOnly),
        "explicit_grant_only" => Ok(ParticipantVisibilityPolicy::ExplicitGrantOnly),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "participant visibility policy",
            value: value.to_string(),
        }),
    }
}

fn parse_external_egress_policy(
    value: &str,
) -> Result<ExternalEgressPolicy, MemoryPersistenceError> {
    match value {
        "allow" => Ok(ExternalEgressPolicy::Allow),
        "approval_required" => Ok(ExternalEgressPolicy::ApprovalRequired),
        "deny" => Ok(ExternalEgressPolicy::Deny),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "external egress policy",
            value: value.to_string(),
        }),
    }
}

fn parse_purpose(value: &str) -> Result<Purpose, MemoryPersistenceError> {
    match value {
        "answer_human_question" => Ok(Purpose::AnswerHumanQuestion),
        "draft_internal_content" => Ok(Purpose::DraftInternalContent),
        "general_personalization" => Ok(Purpose::GeneralPersonalization),
        "manage_task" => Ok(Purpose::ManageTask),
        "manage_calendar" => Ok(Purpose::ManageCalendar),
        "draft_external_content" => Ok(Purpose::DraftExternalContent),
        "use_tool" => Ok(Purpose::UseTool),
        "proactive_suggestion" => Ok(Purpose::ProactiveSuggestion),
        "external_action" => Ok(Purpose::ExternalAction),
        "debug_audit" => Ok(Purpose::DebugAudit),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "retrieval purpose",
            value: value.to_string(),
        }),
    }
}

fn parse_effect(value: &str) -> Result<Effect, MemoryPersistenceError> {
    match value {
        "allow" => Ok(Effect::Allow),
        "deny" => Ok(Effect::Deny),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "policy effect",
            value: value.to_string(),
        }),
    }
}

fn parse_participant_role(value: &str) -> Result<ParticipantRole, MemoryPersistenceError> {
    match value {
        "human_in_scope" => Ok(ParticipantRole::HumanInScope),
        "agent_in_scope" => Ok(ParticipantRole::AgentInScope),
        "originator" => Ok(ParticipantRole::Originator),
        "observer" => Ok(ParticipantRole::Observer),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "participant role",
            value: value.to_string(),
        }),
    }
}

fn parse_subject_role(value: &str) -> Result<SubjectRole, MemoryPersistenceError> {
    match value {
        "about" => Ok(SubjectRole::About),
        "claimant" => Ok(SubjectRole::Claimant),
        "affected" => Ok(SubjectRole::Affected),
        "owner" => Ok(SubjectRole::Owner),
        "assignee" => Ok(SubjectRole::Assignee),
        "source" => Ok(SubjectRole::Source),
        "target" => Ok(SubjectRole::Target),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "subject role",
            value: value.to_string(),
        }),
    }
}

fn parse_relationship_status(value: &str) -> Result<RelationshipStatus, MemoryPersistenceError> {
    match value {
        "candidate" => Ok(RelationshipStatus::Candidate),
        "active" => Ok(RelationshipStatus::Active),
        "confirmed" => Ok(RelationshipStatus::Confirmed),
        "superseded" => Ok(RelationshipStatus::Superseded),
        "archived" => Ok(RelationshipStatus::Archived),
        "deleted" => Ok(RelationshipStatus::Deleted),
        "disputed" => Ok(RelationshipStatus::Disputed),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "relationship status",
            value: value.to_string(),
        }),
    }
}
