pub(super) const MEMORY_SUMMARY_BY_ID_SQL: &str = r"
SELECT
  mi.memory_id,
  mi.status,
  mi.memory_type,
  mi.owner_object_type,
  mi.owner_object_id,
  mi.sensitivity,
  mi.title,
  mi.content,
  mi.created_at,
  pe.source_object_type,
  pe.source_object_id,
  c.conversation_id
FROM memory_items mi
LEFT JOIN object_provenance_edges pe ON pe.edge_id = (
  SELECT edge_id
  FROM object_provenance_edges
  WHERE target_object_type = 'memory_item'
    AND target_object_id = mi.memory_id
    AND deleted_at IS NULL
  ORDER BY
    CASE source_object_type WHEN 'conversation_item' THEN 0 ELSE 1 END,
    created_at ASC,
    edge_id ASC
  LIMIT 1
)
LEFT JOIN conversation_items ci
  ON pe.source_object_type = 'conversation_item'
 AND ci.item_id = pe.source_object_id
LEFT JOIN conversations c
  ON ci.conversation_id = c.conversation_id
WHERE mi.memory_id = ?1
";

pub(super) const RECENT_MEMORY_SQL: &str = r"
SELECT
  mi.memory_id,
  mi.status,
  mi.memory_type,
  mi.owner_object_type,
  mi.owner_object_id,
  mi.sensitivity,
  mi.title,
  mi.content,
  mi.created_at,
  pe.source_object_type,
  pe.source_object_id,
  c.conversation_id
FROM memory_items mi
LEFT JOIN object_provenance_edges pe ON pe.edge_id = (
  SELECT edge_id
  FROM object_provenance_edges
  WHERE target_object_type = 'memory_item'
    AND target_object_id = mi.memory_id
    AND deleted_at IS NULL
  ORDER BY
    CASE source_object_type WHEN 'conversation_item' THEN 0 ELSE 1 END,
    created_at ASC,
    edge_id ASC
  LIMIT 1
)
LEFT JOIN conversation_items ci
  ON pe.source_object_type = 'conversation_item'
 AND ci.item_id = pe.source_object_id
LEFT JOIN conversations c
  ON ci.conversation_id = c.conversation_id
ORDER BY mi.created_at DESC, mi.rowid DESC
LIMIT ?1
";
