pub(super) const MEMORY_SCHEMA_SQL: &str = r"
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

CREATE TABLE IF NOT EXISTS humans (
  human_id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  handle TEXT,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at TEXT,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS agents (
  agent_id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  handle TEXT,
  model_default TEXT,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at TEXT,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS tools (
  tool_id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  tool_kind TEXT NOT NULL,
  is_enabled INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at TEXT,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS conversations (
  conversation_id TEXT PRIMARY KEY,
  title TEXT,
  owner_object_type TEXT NOT NULL,
  owner_object_id TEXT NOT NULL,
  primary_human_id TEXT REFERENCES humans(human_id) ON DELETE SET NULL,
  primary_agent_id TEXT REFERENCES agents(agent_id) ON DELETE SET NULL,
  provider TEXT NOT NULL DEFAULT 'codex',
  model TEXT,
  provider_thread_id TEXT,
  cwd TEXT,
  lifecycle_status TEXT NOT NULL DEFAULT 'active'
    CHECK (lifecycle_status IN ('active','archived','deleted')),
  agent_status TEXT NOT NULL DEFAULT 'idle'
    CHECK (agent_status IN ('idle','input_received','thinking','tool_running','waiting_for_previous_turn_completion','interrupting','error')),
  retention_policy TEXT NOT NULL DEFAULT 'normal'
    CHECK (retention_policy IN ('ephemeral','normal','retain','do_not_store')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at TEXT,
  deleted_by_object_type TEXT,
  deleted_by_object_id TEXT,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS conversation_turns (
  turn_id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id) ON DELETE CASCADE,
  trigger_item_id TEXT,
  status TEXT NOT NULL DEFAULT 'input_received'
    CHECK (status IN ('input_received','running','waiting_for_tool','interrupted','completed','failed','cancelled')),
  started_at TEXT,
  completed_at TEXT,
  interrupted_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS conversation_items (
  item_id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id) ON DELETE CASCADE,
  turn_id TEXT REFERENCES conversation_turns(turn_id) ON DELETE SET NULL,
  parent_item_id TEXT REFERENCES conversation_items(item_id) ON DELETE SET NULL,
  kind TEXT NOT NULL
    CHECK (kind IN ('user_text','assistant_text','activity','a2ui_card','tool_call','tool_result','approval_request','approval_result','error_notice')),
  status TEXT NOT NULL DEFAULT 'completed'
    CHECK (status IN ('pending','running','completed','failed','cancelled','interrupted')),
  author_object_type TEXT NOT NULL,
  author_object_id TEXT NOT NULL,
  content_text TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at TEXT,
  deleted_by_object_type TEXT,
  deleted_by_object_id TEXT,
  redacted_at TEXT,
  redaction_reason TEXT,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  CHECK (content_text IS NOT NULL OR payload_json != '{}')
) STRICT;

CREATE TABLE IF NOT EXISTS memory_items (
  memory_id TEXT PRIMARY KEY,
  owner_object_type TEXT NOT NULL,
  owner_object_id TEXT NOT NULL,
  memory_type TEXT NOT NULL
    CHECK (memory_type IN ('fact','preference','person','organization','project','place','routine','goal','open_loop','procedure','constraint','trigger','decision','skill','policy','note','other')),
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  structured_value TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(structured_value)),
  retrieval_hints TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(retrieval_hints)),
  status TEXT NOT NULL DEFAULT 'candidate'
    CHECK (status IN ('candidate','active','confirmed','inferred','stale','superseded','archived','deleted','disputed')),
  confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
  sensitivity TEXT NOT NULL DEFAULT 'normal'
    CHECK (sensitivity IN ('public','normal','private','sensitive','secret')),
  proactivity_level INTEGER NOT NULL DEFAULT 2 CHECK (proactivity_level BETWEEN 0 AND 6),
  retrieval_policy_status TEXT NOT NULL DEFAULT 'needs_review'
    CHECK (retrieval_policy_status IN ('valid','stale','invalid','needs_review')),
  retrieval_policy_version INTEGER NOT NULL DEFAULT 1 CHECK (retrieval_policy_version >= 1),
  retrieval_policy_fingerprint TEXT,
  retrieval_policy_extractor_object_type TEXT,
  retrieval_policy_extractor_object_id TEXT,
  retrieval_policy_extractor_version TEXT,
  retrieval_policy_validated_at TEXT,
  participant_visibility_policy TEXT NOT NULL DEFAULT 'explicit_grant_only'
    CHECK (participant_visibility_policy IN ('any_active_human','all_original_humans','owner_only','explicit_grant_only')),
  external_egress_policy TEXT NOT NULL DEFAULT 'approval_required'
    CHECK (external_egress_policy IN ('allow','approval_required','deny')),
  created_by_object_type TEXT NOT NULL,
  created_by_object_id TEXT NOT NULL,
  authority_level TEXT NOT NULL DEFAULT 'agent_inference'
    CHECK (authority_level IN ('human_correction','explicit_human_statement','workspace_policy','project_decision','document_source','repeated_observation','agent_inference','weak_inference','system_rule')),
  extraction_method TEXT NOT NULL DEFAULT 'llm_extracted'
    CHECK (extraction_method IN ('explicit_human','llm_extracted','deterministic_rule','imported','human_edited','agent_summary','system_generated')),
  observed_at TEXT,
  valid_from TEXT,
  valid_to TEXT,
  expires_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at TEXT,
  redacted_at TEXT,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  CHECK (
    retrieval_policy_status != 'valid'
    OR (
      retrieval_policy_fingerprint IS NOT NULL
      AND retrieval_policy_extractor_object_type IS NOT NULL
      AND retrieval_policy_extractor_object_id IS NOT NULL
      AND retrieval_policy_extractor_version IS NOT NULL
      AND retrieval_policy_validated_at IS NOT NULL
    )
  )
) STRICT;

CREATE TABLE IF NOT EXISTS entities (
  entity_id TEXT PRIMARY KEY,
  owner_object_type TEXT NOT NULL,
  owner_object_id TEXT NOT NULL,
  entity_type TEXT NOT NULL
    CHECK (entity_type IN ('human','agent','person','organization','project','workspace','conversation','document','tool','place','task','goal','concept','other')),
  canonical_name TEXT NOT NULL,
  aliases TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(aliases)),
  linked_object_type TEXT,
  linked_object_id TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  CHECK (
    (linked_object_type IS NULL AND linked_object_id IS NULL)
    OR (linked_object_type IS NOT NULL AND linked_object_id IS NOT NULL)
  )
) STRICT;

CREATE TABLE IF NOT EXISTS relationships (
  relationship_id TEXT PRIMARY KEY,
  owner_object_type TEXT NOT NULL,
  owner_object_id TEXT NOT NULL,
  subject_entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
  predicate TEXT NOT NULL,
  object_entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
  memory_id TEXT REFERENCES memory_items(memory_id) ON DELETE RESTRICT,
  status TEXT NOT NULL DEFAULT 'candidate'
    CHECK (status IN ('candidate','active','confirmed','superseded','archived','deleted','disputed')),
  confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
  valid_from TEXT,
  valid_to TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  CHECK (status = 'candidate' OR memory_id IS NOT NULL)
) STRICT;

CREATE TABLE IF NOT EXISTS memory_subjects (
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('about','claimant','affected','owner','assignee','source','target')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (memory_id, entity_id, role)
) STRICT;

CREATE TABLE IF NOT EXISTS memory_participants (
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  participant_object_type TEXT NOT NULL,
  participant_object_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('human_in_scope','agent_in_scope','originator','observer')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  PRIMARY KEY (memory_id, participant_object_type, participant_object_id, role)
) STRICT;

CREATE TABLE IF NOT EXISTS memory_retrieval_purpose_rules (
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  purpose TEXT NOT NULL CHECK (purpose IN ('answer_human_question','draft_internal_content','general_personalization','manage_task','manage_calendar','draft_external_content','use_tool','proactive_suggestion','external_action','debug_audit')),
  effect TEXT NOT NULL CHECK (effect IN ('allow','deny')),
  created_by_object_type TEXT,
  created_by_object_id TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  PRIMARY KEY (memory_id, purpose)
) STRICT;

CREATE TABLE IF NOT EXISTS memory_retrieval_object_links (
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  object_type TEXT NOT NULL,
  object_id TEXT NOT NULL,
  relation TEXT NOT NULL CHECK (relation IN ('active_context','required_for','relevant_to','open_loop_for','created_from')),
  resolver_object_type TEXT,
  resolver_object_id TEXT,
  resolver_version TEXT,
  source_run_id TEXT,
  authorized_object_type TEXT,
  authorized_object_id TEXT,
  created_by_object_type TEXT,
  created_by_object_id TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  PRIMARY KEY (memory_id, object_type, object_id, relation)
) STRICT;

CREATE TABLE IF NOT EXISTS object_provenance_edges (
  edge_id TEXT PRIMARY KEY,
  target_object_type TEXT NOT NULL,
  target_object_id TEXT NOT NULL,
  source_object_type TEXT NOT NULL,
  source_object_id TEXT NOT NULL,
  relation TEXT NOT NULL CHECK (relation IN ('derived_from','quoted_from','summarized_from','contradicted_by','supersedes','supports','weakly_supports')),
  evidence_excerpt TEXT,
  created_by_object_type TEXT,
  created_by_object_id TEXT,
  deleted_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS object_access_grants (
  grant_id TEXT PRIMARY KEY,
  target_object_type TEXT NOT NULL,
  target_object_id TEXT NOT NULL,
  grantee_object_type TEXT NOT NULL,
  grantee_object_id TEXT NOT NULL,
  permission TEXT NOT NULL CHECK (permission IN ('read','write','propose','confirm','delete','use_for_retrieval','use_for_proactivity','use_for_external_action')),
  effect TEXT NOT NULL CHECK (effect IN ('allow','deny')),
  expires_at TEXT,
  created_by_object_type TEXT,
  created_by_object_id TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS object_events (
  event_id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  actor_object_type TEXT,
  actor_object_id TEXT,
  target_object_type TEXT,
  target_object_id TEXT,
  reason TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;

CREATE TABLE IF NOT EXISTS object_links (
  link_id TEXT PRIMARY KEY,
  source_object_type TEXT NOT NULL,
  source_object_id TEXT NOT NULL,
  target_object_type TEXT NOT NULL,
  target_object_id TEXT NOT NULL,
  relation TEXT NOT NULL,
  created_by_object_type TEXT,
  created_by_object_id TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  UNIQUE(source_object_type, source_object_id, target_object_type, target_object_id, relation)
) STRICT;

CREATE TABLE IF NOT EXISTS context_packets (
  context_packet_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  requesting_object_type TEXT NOT NULL,
  requesting_object_id TEXT NOT NULL,
  purpose TEXT NOT NULL CHECK (purpose IN ('answer_human_question','draft_internal_content','general_personalization','manage_task','manage_calendar','draft_external_content','use_tool','proactive_suggestion','external_action','debug_audit')),
  active_objects TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(active_objects)),
  agent_visible_omissions TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(agent_visible_omissions)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS context_packet_memories (
  packet_memory_id TEXT PRIMARY KEY,
  context_packet_id TEXT NOT NULL REFERENCES context_packets(context_packet_id) ON DELETE CASCADE,
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  stage TEXT NOT NULL CHECK (stage IN ('included_in_packet','shown_to_agent','used_in_reply','used_for_action','used_for_proactivity')),
  rank_score INTEGER CHECK (rank_score IS NULL OR rank_score >= 0),
  eligibility_reason TEXT,
  rank_reasons TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(rank_reasons)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;

CREATE TABLE IF NOT EXISTS context_packet_omissions (
  omission_id TEXT PRIMARY KEY,
  context_packet_id TEXT NOT NULL REFERENCES context_packets(context_packet_id) ON DELETE CASCADE,
  memory_id TEXT REFERENCES memory_items(memory_id) ON DELETE SET NULL,
  relationship_id TEXT REFERENCES relationships(relationship_id) ON DELETE SET NULL,
  omission_sensitivity TEXT NOT NULL DEFAULT 'normal' CHECK (omission_sensitivity IN ('public','normal','private','sensitive','secret')),
  agent_visible_reason TEXT NOT NULL,
  audit_reason TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;

CREATE TABLE IF NOT EXISTS memory_use_records (
  memory_use_id TEXT PRIMARY KEY,
  context_packet_id TEXT REFERENCES context_packets(context_packet_id) ON DELETE CASCADE,
  run_id TEXT NOT NULL,
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  stage TEXT NOT NULL CHECK (stage IN ('retrieved','included_in_packet','shown_to_agent','used_in_reply','used_for_action','used_for_proactivity')),
  agent_object_type TEXT,
  agent_object_id TEXT,
  context_object_type TEXT,
  context_object_id TEXT,
  purpose TEXT NOT NULL CHECK (purpose IN ('answer_human_question','draft_internal_content','general_personalization','manage_task','manage_calendar','draft_external_content','use_tool','proactive_suggestion','external_action','debug_audit')),
  used_for_object_type TEXT,
  used_for_object_id TEXT,
  policy_decision_id TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_humans_handle ON humans(handle);
CREATE INDEX IF NOT EXISTS idx_agents_handle ON agents(handle);
CREATE INDEX IF NOT EXISTS idx_tools_kind ON tools(tool_kind, is_enabled);
CREATE INDEX IF NOT EXISTS idx_conversations_owner ON conversations(owner_object_type, owner_object_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_conversations_primary_human ON conversations(primary_human_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_conversations_primary_agent ON conversations(primary_agent_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_conversation_turns_conversation ON conversation_turns(conversation_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_conversation_turns_status ON conversation_turns(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_conversation_items_conversation ON conversation_items(conversation_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_conversation_items_turn ON conversation_items(turn_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_conversation_items_parent ON conversation_items(parent_item_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_conversation_items_author ON conversation_items(author_object_type, author_object_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_items_owner ON memory_items(owner_object_type, owner_object_id, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_items_created_at ON memory_items(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_items_status ON memory_items(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_items_policy_status ON memory_items(retrieval_policy_status, sensitivity);
CREATE INDEX IF NOT EXISTS idx_entities_owner_type ON entities(owner_object_type, owner_object_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_canonical_name ON entities(entity_type, canonical_name);
CREATE INDEX IF NOT EXISTS idx_memory_subjects_entity_role ON memory_subjects(entity_id, role, memory_id);
CREATE INDEX IF NOT EXISTS idx_memory_subjects_memory_role ON memory_subjects(memory_id, role);
CREATE INDEX IF NOT EXISTS idx_relationships_subject ON relationships(subject_entity_id, predicate);
CREATE INDEX IF NOT EXISTS idx_relationships_object ON relationships(object_entity_id, predicate);
CREATE INDEX IF NOT EXISTS idx_relationships_memory ON relationships(memory_id, status);
CREATE INDEX IF NOT EXISTS idx_memory_participants_object ON memory_participants(participant_object_type, participant_object_id, role);
CREATE INDEX IF NOT EXISTS idx_memory_retrieval_purpose_rules ON memory_retrieval_purpose_rules(purpose, effect, memory_id);
CREATE INDEX IF NOT EXISTS idx_memory_retrieval_object_links ON memory_retrieval_object_links(object_type, object_id, relation);
CREATE INDEX IF NOT EXISTS idx_object_provenance_target ON object_provenance_edges(target_object_type, target_object_id, deleted_at, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_object_provenance_source ON object_provenance_edges(source_object_type, source_object_id, deleted_at, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_object_access_grants_grantee ON object_access_grants(grantee_object_type, grantee_object_id, permission, effect);
CREATE INDEX IF NOT EXISTS idx_object_access_grants_target ON object_access_grants(target_object_type, target_object_id);
CREATE INDEX IF NOT EXISTS idx_object_events_target_time ON object_events(target_object_type, target_object_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_object_events_actor_time ON object_events(actor_object_type, actor_object_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_object_links_source_relation ON object_links(source_object_type, source_object_id, relation);
CREATE INDEX IF NOT EXISTS idx_object_links_target_relation ON object_links(target_object_type, target_object_id, relation);
CREATE INDEX IF NOT EXISTS idx_context_packets_run ON context_packets(run_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_context_packet_memories_packet ON context_packet_memories(context_packet_id, stage);
CREATE INDEX IF NOT EXISTS idx_context_packet_memories_memory ON context_packet_memories(memory_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_context_packet_omissions_packet ON context_packet_omissions(context_packet_id, audit_reason);
CREATE INDEX IF NOT EXISTS idx_context_packet_omissions_memory ON context_packet_omissions(memory_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_use_records_run ON memory_use_records(run_id, stage, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_use_records_memory ON memory_use_records(memory_id, stage, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_use_records_context ON memory_use_records(context_object_type, context_object_id, created_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
  memory_id UNINDEXED,
  title,
  content,
  retrieval_hints,
  tokenize = 'porter unicode61'
);
";
