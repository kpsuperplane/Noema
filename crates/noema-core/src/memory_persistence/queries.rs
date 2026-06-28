pub(super) const POSTGRES_MEMORY_SUMMARY_BY_ID_SQL: &str = r"
SELECT
  mi.memory_id,
  mi.status,
  mi.memory_type,
  mi.owner_object_type,
  mi.owner_object_id,
  mi.sensitivity,
  mi.title,
  mi.content,
  COALESCE(subjects.subject_entity_ids, ARRAY[]::text[]) AS subject_entity_ids,
  mi.memory_dedupe_fingerprint,
  mi.created_at::text,
  pe.source_object_type,
  pe.source_object_id,
  c.conversation_id
FROM memory_items mi
LEFT JOIN LATERAL (
  SELECT edge_id, source_object_type, source_object_id
  FROM object_provenance_edges
  WHERE target_object_type = 'memory_item'
    AND target_object_id = mi.memory_id
    AND deleted_at IS NULL
  ORDER BY
    CASE source_object_type WHEN 'conversation_item' THEN 0 ELSE 1 END,
    created_at ASC,
    edge_id ASC
  LIMIT 1
) pe ON TRUE
LEFT JOIN conversation_items ci
  ON pe.source_object_type = 'conversation_item'
 AND ci.item_id = pe.source_object_id
LEFT JOIN conversations c
  ON ci.conversation_id = c.conversation_id
LEFT JOIN LATERAL (
  SELECT array_agg(DISTINCT ms.entity_id ORDER BY ms.entity_id) AS subject_entity_ids
  FROM memory_subjects ms
  WHERE ms.memory_id = mi.memory_id
) subjects ON TRUE
WHERE mi.memory_id = $1
";

pub(super) const POSTGRES_MEMORY_SUMMARY_BY_DEDUPE_FINGERPRINT_SQL: &str = r"
SELECT
  mi.memory_id,
  mi.status,
  mi.memory_type,
  mi.owner_object_type,
  mi.owner_object_id,
  mi.sensitivity,
  mi.title,
  mi.content,
  COALESCE(subjects.subject_entity_ids, ARRAY[]::text[]) AS subject_entity_ids,
  mi.memory_dedupe_fingerprint,
  mi.created_at::text,
  pe.source_object_type,
  pe.source_object_id,
  c.conversation_id
FROM memory_items mi
LEFT JOIN LATERAL (
  SELECT edge_id, source_object_type, source_object_id
  FROM object_provenance_edges
  WHERE target_object_type = 'memory_item'
    AND target_object_id = mi.memory_id
    AND deleted_at IS NULL
  ORDER BY
    CASE source_object_type WHEN 'conversation_item' THEN 0 ELSE 1 END,
    created_at ASC,
    edge_id ASC
  LIMIT 1
) pe ON TRUE
LEFT JOIN conversation_items ci
  ON pe.source_object_type = 'conversation_item'
 AND ci.item_id = pe.source_object_id
LEFT JOIN conversations c
  ON ci.conversation_id = c.conversation_id
LEFT JOIN LATERAL (
  SELECT array_agg(DISTINCT ms.entity_id ORDER BY ms.entity_id) AS subject_entity_ids
  FROM memory_subjects ms
  WHERE ms.memory_id = mi.memory_id
) subjects ON TRUE
WHERE mi.memory_dedupe_fingerprint = $1
  AND mi.deleted_at IS NULL
  AND mi.status IN ('candidate', 'active', 'confirmed', 'inferred')
ORDER BY mi.created_at ASC, mi.memory_id ASC
LIMIT 1
";

pub(super) const POSTGRES_RECENT_MEMORY_SQL: &str = r"
SELECT
  mi.memory_id,
  mi.status,
  mi.memory_type,
  mi.owner_object_type,
  mi.owner_object_id,
  mi.sensitivity,
  mi.title,
  mi.content,
  COALESCE(subjects.subject_entity_ids, ARRAY[]::text[]) AS subject_entity_ids,
  mi.memory_dedupe_fingerprint,
  mi.created_at::text,
  pe.source_object_type,
  pe.source_object_id,
  c.conversation_id
FROM memory_items mi
LEFT JOIN LATERAL (
  SELECT edge_id, source_object_type, source_object_id
  FROM object_provenance_edges
  WHERE target_object_type = 'memory_item'
    AND target_object_id = mi.memory_id
    AND deleted_at IS NULL
  ORDER BY
    CASE source_object_type WHEN 'conversation_item' THEN 0 ELSE 1 END,
    created_at ASC,
    edge_id ASC
  LIMIT 1
) pe ON TRUE
LEFT JOIN conversation_items ci
  ON pe.source_object_type = 'conversation_item'
 AND ci.item_id = pe.source_object_id
LEFT JOIN conversations c
  ON ci.conversation_id = c.conversation_id
LEFT JOIN LATERAL (
  SELECT array_agg(DISTINCT ms.entity_id ORDER BY ms.entity_id) AS subject_entity_ids
  FROM memory_subjects ms
  WHERE ms.memory_id = mi.memory_id
) subjects ON TRUE
ORDER BY mi.created_at DESC, mi.memory_id DESC
LIMIT $1
";
