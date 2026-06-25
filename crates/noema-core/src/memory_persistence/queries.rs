pub(super) const MEMORY_SUMMARY_BY_ID_SQL: &str = r"
SELECT
  mi.memory_id,
  mi.status,
  mi.memory_type,
  mi.home_scope_id,
  mi.sensitivity,
  mi.title,
  mi.content,
  mi.created_at,
  pe.source_type,
  pe.source_id,
  e.raw_ref
FROM memory_items mi
LEFT JOIN memory_provenance_edges pe ON pe.edge_id = (
  SELECT edge_id
  FROM memory_provenance_edges
  WHERE memory_id = mi.memory_id
  ORDER BY
    CASE source_type WHEN 'episode' THEN 0 ELSE 1 END,
    created_at ASC,
    edge_id ASC
  LIMIT 1
)
LEFT JOIN episodes e
  ON pe.source_type = 'episode'
 AND e.episode_id = pe.source_id
WHERE mi.memory_id = ?1
";

pub(super) const RECENT_MEMORY_SQL: &str = r"
SELECT
  mi.memory_id,
  mi.status,
  mi.memory_type,
  mi.home_scope_id,
  mi.sensitivity,
  mi.title,
  mi.content,
  mi.created_at,
  pe.source_type,
  pe.source_id,
  e.raw_ref
FROM memory_items mi
LEFT JOIN memory_provenance_edges pe ON pe.edge_id = (
  SELECT edge_id
  FROM memory_provenance_edges
  WHERE memory_id = mi.memory_id
  ORDER BY
    CASE source_type WHEN 'episode' THEN 0 ELSE 1 END,
    created_at ASC,
    edge_id ASC
  LIMIT 1
)
LEFT JOIN episodes e
  ON pe.source_type = 'episode'
 AND e.episode_id = pe.source_id
ORDER BY mi.created_at DESC, mi.rowid DESC
LIMIT ?1
";
