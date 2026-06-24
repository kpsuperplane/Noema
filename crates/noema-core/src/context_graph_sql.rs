//! SQL projections for context graph inspection.

pub(crate) const RELATIONSHIP_BY_ID_SQL: &str = r"
SELECT
  r.relationship_id,
  r.home_scope_id,
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
SELECT
  memory_id,
  status,
  memory_type,
  home_scope_id,
  sensitivity,
  title,
  retrieval_hints,
  retrieval_policy_status,
  retrieval_policy_version,
  retrieval_policy_fingerprint,
  retrieval_policy_extractor_principal_id,
  retrieval_policy_extractor_version,
  retrieval_policy_validated_at,
  participant_visibility_policy,
  external_egress_policy,
  created_at
FROM memory_items
ORDER BY created_at DESC, rowid DESC
LIMIT ?1
";

pub(crate) const GRAPH_ENTITY_NODES_SQL: &str = r"
SELECT DISTINCT
  e.entity_id,
  e.entity_type,
  e.home_scope_id,
  e.canonical_name,
  e.linked_principal_id
FROM entities e
WHERE e.entity_id IN (
  SELECT entity_id FROM memory_subjects
  UNION
  SELECT subject_entity_id FROM relationships
  UNION
  SELECT object_entity_id FROM relationships
)
ORDER BY e.created_at DESC, e.entity_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_SUBJECT_EDGES_SQL: &str = r"
SELECT
  ms.memory_id,
  ms.entity_id,
  ms.role
FROM memory_subjects ms
JOIN memory_items mi ON mi.memory_id = ms.memory_id
ORDER BY mi.created_at DESC, ms.memory_id ASC, ms.entity_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_PARTICIPANT_EDGES_SQL: &str = r"
SELECT
  mp.memory_id,
  mp.principal_id,
  mp.role
FROM memory_participants mp
JOIN memory_items mi ON mi.memory_id = mp.memory_id
ORDER BY mi.created_at DESC, mp.memory_id ASC, mp.principal_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_PROVENANCE_EDGES_SQL: &str = r"
SELECT
  memory_id,
  source_type,
  source_id,
  relation,
  evidence_excerpt
FROM memory_provenance_edges
ORDER BY created_at DESC, edge_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_OBJECT_LINK_EDGES_SQL: &str = r"
SELECT
  link.memory_id,
  link.object_type,
  link.object_id,
  link.relation,
  link.authorized_scope_id,
  link.resolver_principal_id,
  link.resolver_version,
  link.source_run_id,
  link.created_at
FROM memory_retrieval_object_links link
JOIN memory_items mi ON mi.memory_id = link.memory_id
ORDER BY mi.created_at DESC, link.memory_id ASC, link.object_type ASC, link.object_id ASC, link.relation ASC
LIMIT ?1
";

pub(crate) const GRAPH_PURPOSE_RULES_SQL: &str = r"
SELECT
  rule.memory_id,
  rule.purpose,
  rule.effect,
  rule.created_by_principal_id,
  rule.created_at
FROM memory_retrieval_purpose_rules rule
JOIN memory_items mi ON mi.memory_id = rule.memory_id
ORDER BY mi.created_at DESC, rule.memory_id ASC, rule.purpose ASC
LIMIT ?1
";

pub(crate) const GRAPH_ACCESS_GRANTS_SQL: &str = r"
WITH inspected_memories AS (
  SELECT memory_id, home_scope_id
  FROM memory_items
  ORDER BY created_at DESC, rowid DESC
  LIMIT ?1
)
SELECT
  grant.grant_id,
  grant.memory_id,
  grant.scope_id,
  grant.principal_id,
  grant.permission,
  grant.effect,
  grant.expires_at,
  grant.created_by_principal_id,
  grant.created_at
FROM memory_access_grants grant
WHERE grant.memory_id IN (SELECT memory_id FROM inspected_memories)
   OR grant.scope_id IN (SELECT home_scope_id FROM inspected_memories)
ORDER BY grant.created_at DESC, grant.grant_id ASC
LIMIT ?1
";

pub(crate) const GRAPH_RELATIONSHIP_EDGES_SQL: &str = r"
SELECT
  r.relationship_id,
  r.home_scope_id,
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
ORDER BY r.created_at DESC, r.relationship_id ASC
LIMIT ?1
";
