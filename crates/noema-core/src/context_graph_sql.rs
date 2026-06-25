//! SQL projections for context graph inspection.

pub(crate) const RELATIONSHIP_BY_ID_SQL: &str = r"
SELECT
  r.relationship_id,
  r.owner_object_type,
  r.owner_object_id,
  r.subject_entity_id,
  subject.canonical_name,
  r.predicate,
  r.object_entity_id,
  object.canonical_name,
  r.memory_id,
  r.status,
  r.confidence,
  r.created_at
FROM relationships r
LEFT JOIN entities subject ON subject.entity_id = r.subject_entity_id
LEFT JOIN entities object ON object.entity_id = r.object_entity_id
WHERE r.relationship_id = ?1
";

pub(crate) const GRAPH_MEMORY_NODES_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
),
packet_memory_ids AS (
  SELECT memory_id FROM context_packet_memories
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
  WHERE (?2 IS NULL AND ?3 IS NULL)
     OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
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
  retrieval_hints,
  retrieval_policy_status,
  retrieval_policy_version,
  retrieval_policy_fingerprint,
  retrieval_policy_extractor_object_type,
  retrieval_policy_extractor_object_id,
  retrieval_policy_extractor_version,
  retrieval_policy_validated_at,
  participant_visibility_policy,
  external_egress_policy,
  created_at
FROM memory_items
WHERE memory_id IN (SELECT memory_id FROM inspected_memories)
ORDER BY created_at DESC, rowid DESC
LIMIT ?1
";

pub(crate) const GRAPH_ENTITY_NODES_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
),
selected_relationships AS (
  SELECT relationship_id FROM context_packet_omissions
  WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
    AND relationship_id IS NOT NULL
),
packet_memory_ids AS (
  SELECT memory_id FROM context_packet_memories
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
  WHERE (?2 IS NULL AND ?3 IS NULL)
     OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
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
    ?2 IS NULL
    AND ?3 IS NULL
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
LIMIT ?1
";

pub(crate) const GRAPH_SUBJECT_EDGES_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
),
packet_memory_ids AS (
  SELECT memory_id FROM context_packet_memories
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
  WHERE (?2 IS NULL AND ?3 IS NULL)
     OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
)
SELECT
  ms.memory_id,
  ms.entity_id,
  ms.role
FROM memory_subjects ms
JOIN memory_items mi ON mi.memory_id = ms.memory_id
WHERE (?2 IS NULL AND ?3 IS NULL)
   OR ms.memory_id IN (SELECT memory_id FROM inspected_memories)
ORDER BY mi.created_at DESC, ms.memory_id ASC, ms.entity_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_PARTICIPANT_EDGES_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
),
packet_memory_ids AS (
  SELECT memory_id FROM context_packet_memories
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
  WHERE (?2 IS NULL AND ?3 IS NULL)
     OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
)
SELECT
  mp.memory_id,
  mp.participant_object_type,
  mp.participant_object_id,
  mp.role
FROM memory_participants mp
JOIN memory_items mi ON mi.memory_id = mp.memory_id
WHERE (?2 IS NULL AND ?3 IS NULL)
   OR mp.memory_id IN (SELECT memory_id FROM inspected_memories)
ORDER BY mi.created_at DESC, mp.memory_id ASC, mp.participant_object_type ASC, mp.participant_object_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_PROVENANCE_EDGES_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
),
packet_memory_ids AS (
  SELECT memory_id FROM context_packet_memories
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
  WHERE (?2 IS NULL AND ?3 IS NULL)
     OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
)
SELECT
  target_object_id,
  source_object_type,
  source_object_id,
  relation,
  evidence_excerpt
FROM object_provenance_edges
WHERE (
    (?2 IS NULL AND ?3 IS NULL)
    OR target_object_id IN (SELECT memory_id FROM inspected_memories)
  )
  AND target_object_type = 'memory_item'
  AND deleted_at IS NULL
ORDER BY created_at DESC, edge_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_OBJECT_LINK_EDGES_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
),
packet_memory_ids AS (
  SELECT memory_id FROM context_packet_memories
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
  WHERE (?2 IS NULL AND ?3 IS NULL)
     OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
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
  link.created_at
FROM memory_retrieval_object_links link
JOIN memory_items mi ON mi.memory_id = link.memory_id
WHERE (?2 IS NULL AND ?3 IS NULL)
   OR link.memory_id IN (SELECT memory_id FROM inspected_memories)
ORDER BY mi.created_at DESC, link.memory_id ASC, link.object_type ASC, link.object_id ASC, link.relation ASC
LIMIT ?1
";

pub(crate) const GRAPH_PURPOSE_RULES_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
),
packet_memory_ids AS (
  SELECT memory_id FROM context_packet_memories
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
  WHERE (?2 IS NULL AND ?3 IS NULL)
     OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
)
SELECT
  rule.memory_id,
  rule.purpose,
  rule.effect,
  rule.created_by_object_type,
  rule.created_by_object_id,
  rule.created_at
FROM memory_retrieval_purpose_rules rule
JOIN memory_items mi ON mi.memory_id = rule.memory_id
WHERE (?2 IS NULL AND ?3 IS NULL)
   OR rule.memory_id IN (SELECT memory_id FROM inspected_memories)
ORDER BY mi.created_at DESC, rule.memory_id ASC, rule.purpose ASC
LIMIT ?1
";

pub(crate) const GRAPH_ACCESS_GRANTS_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
),
packet_memory_ids AS (
  SELECT memory_id FROM context_packet_memories
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
  SELECT memory_id, owner_object_type, owner_object_id
  FROM memory_items
  WHERE (?2 IS NULL AND ?3 IS NULL)
     OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
)
SELECT
  grant.grant_id,
  grant.target_object_type,
  grant.target_object_id,
  grant.grantee_object_type,
  grant.grantee_object_id,
  grant.permission,
  grant.effect,
  grant.expires_at,
  grant.created_by_object_type,
  grant.created_by_object_id,
  grant.created_at
FROM object_access_grants grant
WHERE (
    grant.target_object_type = 'memory_item'
    AND grant.target_object_id IN (SELECT memory_id FROM inspected_memories)
  )
   OR (
     ?2 IS NULL
     AND ?3 IS NULL
     AND EXISTS (
       SELECT 1
       FROM inspected_memories inspected
       WHERE inspected.owner_object_type = grant.target_object_type
         AND inspected.owner_object_id = grant.target_object_id
     )
   )
ORDER BY grant.created_at DESC, grant.grant_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_CONTEXT_PACKETS_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
)
SELECT
  context_packet_id,
  run_id,
  requesting_object_type,
  requesting_object_id,
  purpose,
  active_objects,
  agent_visible_omissions,
  created_at
FROM context_packets
WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
ORDER BY created_at DESC, context_packet_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_CONTEXT_PACKET_MEMORY_EDGES_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
)
SELECT
  edge.packet_memory_id,
  edge.context_packet_id,
  edge.memory_id,
  memory.sensitivity,
  edge.stage,
  edge.rank_score,
  edge.eligibility_reason,
  edge.rank_reasons,
  edge.created_at
FROM context_packet_memories edge
JOIN memory_items memory ON memory.memory_id = edge.memory_id
WHERE edge.context_packet_id IN (SELECT context_packet_id FROM selected_packets)
ORDER BY edge.created_at DESC, edge.packet_memory_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_CONTEXT_PACKET_OMISSIONS_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
)
SELECT
  omission.omission_id,
  omission.context_packet_id,
  omission.memory_id,
  omission.relationship_id,
  omission.omission_sensitivity,
  omission.agent_visible_reason,
  omission.audit_reason,
  omission.created_at,
  omission.details
FROM context_packet_omissions omission
WHERE omission.context_packet_id IN (SELECT context_packet_id FROM selected_packets)
ORDER BY omission.created_at DESC, omission.omission_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_MEMORY_USE_RECORDS_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
)
SELECT
  use_record.memory_use_id,
  use_record.context_packet_id,
  use_record.run_id,
  use_record.memory_id,
  memory.sensitivity,
  use_record.stage,
  use_record.agent_object_type,
  use_record.agent_object_id,
  use_record.context_object_type,
  use_record.context_object_id,
  use_record.purpose,
  use_record.used_for_object_type,
  use_record.used_for_object_id,
  use_record.policy_decision_id,
  use_record.created_at,
  use_record.details
FROM memory_use_records use_record
JOIN memory_items memory ON memory.memory_id = use_record.memory_id
WHERE use_record.context_packet_id IN (SELECT context_packet_id FROM selected_packets)
   OR (
     ?2 IS NULL
     AND ?3 IS NULL
     AND use_record.context_packet_id IS NULL
   )
ORDER BY use_record.created_at DESC, use_record.memory_use_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_MEMORY_EVENTS_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
),
packet_memory_ids AS (
  SELECT memory_id FROM context_packet_memories
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
  SELECT memory_id, owner_object_type, owner_object_id
  FROM memory_items
  WHERE (?2 IS NULL AND ?3 IS NULL)
     OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
)
SELECT
  event.event_id,
  event.event_type,
  event.actor_object_type,
  event.actor_object_id,
  event.target_object_type,
  event.target_object_id,
  memory.sensitivity,
  event.reason,
  event.created_at,
  event.details
FROM object_events event
LEFT JOIN memory_items memory
  ON event.target_object_type = 'memory_item'
 AND memory.memory_id = event.target_object_id
WHERE (
    event.target_object_type = 'memory_item'
    AND event.target_object_id IN (SELECT memory_id FROM inspected_memories)
  )
   OR (
     ?2 IS NULL
     AND ?3 IS NULL
     AND EXISTS (
       SELECT 1
       FROM inspected_memories inspected
       WHERE inspected.owner_object_type = event.target_object_type
         AND inspected.owner_object_id = event.target_object_id
     )
   )
ORDER BY event.created_at DESC, event.event_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_RELATIONSHIP_EDGES_SQL: &str = r"
WITH selected_packets AS (
  SELECT context_packet_id
  FROM context_packets
  WHERE (?2 IS NULL OR run_id = ?2)
    AND (?3 IS NULL OR context_packet_id = ?3)
  ORDER BY created_at DESC, context_packet_id ASC
  LIMIT ?1
),
selected_relationships AS (
  SELECT relationship_id FROM context_packet_omissions
  WHERE context_packet_id IN (SELECT context_packet_id FROM selected_packets)
    AND relationship_id IS NOT NULL
),
packet_memory_ids AS (
  SELECT memory_id FROM context_packet_memories
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
  WHERE (?2 IS NULL AND ?3 IS NULL)
     OR memory_id IN (SELECT memory_id FROM packet_memory_ids)
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
)
SELECT
  r.relationship_id,
  r.owner_object_type,
  r.owner_object_id,
  r.subject_entity_id,
  subject.canonical_name,
  r.predicate,
  r.object_entity_id,
  object.canonical_name,
  r.memory_id,
  r.status,
  r.confidence,
  r.created_at
FROM relationships r
LEFT JOIN entities subject ON subject.entity_id = r.subject_entity_id
LEFT JOIN entities object ON object.entity_id = r.object_entity_id
WHERE (?2 IS NULL AND ?3 IS NULL)
   OR r.memory_id IN (SELECT memory_id FROM inspected_memories)
   OR r.relationship_id IN (SELECT relationship_id FROM selected_relationships)
ORDER BY r.created_at DESC, r.relationship_id ASC
LIMIT ?1
";
