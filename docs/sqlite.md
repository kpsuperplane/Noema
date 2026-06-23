# Canonical SQLite Schema

## Purpose

Canonical SQLite schema for Noema structured state.

```sql
-- Noema canonical SQLite schema
-- Canonical structured state lives in db/noema.sqlite.
-- Derived indexes, caches, and vector stores live under system/.

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;

CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

CREATE TABLE principals (
  principal_id TEXT PRIMARY KEY,
  principal_type TEXT NOT NULL CHECK (principal_type IN ('human','agent','group','system','tool','service','importer')),
  display_name TEXT NOT NULL,
  handle TEXT,
  owner_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE scopes (
  scope_id TEXT PRIMARY KEY,
  scope_type TEXT NOT NULL CHECK (scope_type IN ('system','human','workspace','project','conversation','agent','relationship','tool','custom')),
  parent_scope_id TEXT REFERENCES scopes(scope_id) ON DELETE SET NULL,
  owner_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  slug TEXT NOT NULL,
  description TEXT,
  default_visibility TEXT NOT NULL DEFAULT 'private' CHECK (default_visibility IN ('private','shared','public')),
  default_proactivity_level INTEGER NOT NULL DEFAULT 2 CHECK (default_proactivity_level BETWEEN 0 AND 6),
  is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  UNIQUE(parent_scope_id, slug)
) STRICT;

CREATE TABLE sources (
  source_id TEXT PRIMARY KEY,
  source_type TEXT NOT NULL CHECK (source_type IN ('chat','email','calendar','file','browser','tool','system','import','human_profile','api','other')),
  source_name TEXT NOT NULL,
  external_ref TEXT,
  owner_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  trust_level INTEGER NOT NULL DEFAULT 3 CHECK (trust_level BETWEEN 0 AND 5),
  retention_policy TEXT NOT NULL DEFAULT 'normal' CHECK (retention_policy IN ('ephemeral','normal','retain','do_not_store')),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE episodes (
  episode_id TEXT PRIMARY KEY,
  home_scope_id TEXT REFERENCES scopes(scope_id) ON DELETE SET NULL,
  source_id TEXT REFERENCES sources(source_id) ON DELETE SET NULL,
  episode_type TEXT NOT NULL CHECK (episode_type IN ('conversation','message','tool_call','file_read','email_seen','calendar_event','task_completed','correction','decision','external_event','system_event','imported_record','other')),
  title TEXT,
  summary TEXT,
  raw_ref TEXT,
  content_hash TEXT,
  occurred_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE messages (
  message_id TEXT PRIMARY KEY,
  episode_id TEXT NOT NULL REFERENCES episodes(episode_id) ON DELETE CASCADE,
  author_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  role TEXT NOT NULL CHECK (role IN ('human','assistant','agent','tool','system','developer','observer')),
  content TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE memory_items (
  memory_id TEXT PRIMARY KEY,
  home_scope_id TEXT NOT NULL REFERENCES scopes(scope_id) ON DELETE CASCADE,
  memory_type TEXT NOT NULL CHECK (memory_type IN ('fact','preference','person','organization','project','place','routine','goal','open_loop','procedure','constraint','trigger','decision','skill','policy','note','other')),
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  structured_value TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(structured_value)),
  status TEXT NOT NULL DEFAULT 'candidate' CHECK (status IN ('candidate','active','confirmed','inferred','stale','superseded','archived','deleted','disputed')),
  confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
  sensitivity TEXT NOT NULL DEFAULT 'normal' CHECK (sensitivity IN ('public','normal','private','sensitive','secret')),
  proactivity_level INTEGER NOT NULL DEFAULT 2 CHECK (proactivity_level BETWEEN 0 AND 6),
  created_by_principal_id TEXT NOT NULL REFERENCES principals(principal_id) ON DELETE RESTRICT,
  owner_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  authority_level TEXT NOT NULL DEFAULT 'agent_inference' CHECK (authority_level IN ('human_correction','explicit_human_statement','workspace_policy','project_decision','document_source','repeated_observation','agent_inference','weak_inference','system_rule')),
  extraction_method TEXT NOT NULL DEFAULT 'llm_extracted' CHECK (extraction_method IN ('explicit_human','llm_extracted','deterministic_rule','imported','human_edited','agent_summary','system_generated')),
  observed_at TEXT,
  valid_from TEXT,
  valid_to TEXT,
  expires_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE entities (
  entity_id TEXT PRIMARY KEY,
  home_scope_id TEXT REFERENCES scopes(scope_id) ON DELETE SET NULL,
  entity_type TEXT NOT NULL CHECK (entity_type IN ('human','agent','person','organization','project','workspace','conversation','document','tool','place','task','goal','concept','other')),
  canonical_name TEXT NOT NULL,
  aliases TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(aliases)),
  linked_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE memory_subjects (
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('about','claimant','affected','participant','owner','assignee','source','target')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (memory_id, entity_id, role)
) STRICT;

CREATE TABLE relationships (
  relationship_id TEXT PRIMARY KEY,
  home_scope_id TEXT NOT NULL REFERENCES scopes(scope_id) ON DELETE CASCADE,
  subject_entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
  predicate TEXT NOT NULL,
  object_entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
  memory_id TEXT REFERENCES memory_items(memory_id) ON DELETE SET NULL,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('candidate','active','confirmed','superseded','archived','deleted','disputed')),
  confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
  valid_from TEXT,
  valid_to TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE memory_provenance_edges (
  edge_id TEXT PRIMARY KEY,
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  source_type TEXT NOT NULL CHECK (source_type IN ('episode','message','document','tool_result','memory','import','rule','source')),
  source_id TEXT NOT NULL,
  relation TEXT NOT NULL CHECK (relation IN ('derived_from','quoted_from','summarized_from','contradicted_by','supersedes','supports','weakly_supports')),
  evidence_excerpt TEXT,
  created_by_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE memory_access_grants (
  grant_id TEXT PRIMARY KEY,
  memory_id TEXT REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  scope_id TEXT REFERENCES scopes(scope_id) ON DELETE CASCADE,
  principal_id TEXT NOT NULL REFERENCES principals(principal_id) ON DELETE CASCADE,
  permission TEXT NOT NULL CHECK (permission IN ('read','write','propose','confirm','delete','use_for_retrieval','use_for_proactivity','use_for_external_action')),
  effect TEXT NOT NULL CHECK (effect IN ('allow','deny')),
  expires_at TEXT,
  created_by_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  CHECK (memory_id IS NOT NULL OR scope_id IS NOT NULL)
) STRICT;

CREATE TABLE memory_versions (
  version_id TEXT PRIMARY KEY,
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  previous_content TEXT,
  new_content TEXT,
  previous_structured_value TEXT CHECK (previous_structured_value IS NULL OR json_valid(previous_structured_value)),
  new_structured_value TEXT CHECK (new_structured_value IS NULL OR json_valid(new_structured_value)),
  changed_by_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  change_reason TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE memory_events (
  event_id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL CHECK (event_type IN ('created','promoted','edited','merged','archived','deleted','retrieved','shown_to_model','used_in_reply','used_for_action','exported','confirmed','disputed','superseded','restored')),
  actor_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  memory_id TEXT REFERENCES memory_items(memory_id) ON DELETE SET NULL,
  scope_id TEXT REFERENCES scopes(scope_id) ON DELETE SET NULL,
  reason TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;

CREATE TABLE proactive_rules (
  rule_id TEXT PRIMARY KEY,
  scope_id TEXT REFERENCES scopes(scope_id) ON DELETE CASCADE,
  memory_type TEXT,
  condition_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(condition_json)),
  allowed_action_level INTEGER NOT NULL DEFAULT 3 CHECK (allowed_action_level BETWEEN 0 AND 6),
  requires_confirmation INTEGER NOT NULL DEFAULT 1 CHECK (requires_confirmation IN (0,1)),
  notification_channel TEXT,
  cooldown_seconds INTEGER CHECK (cooldown_seconds IS NULL OR cooldown_seconds >= 0),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE deletion_tombstones (
  tombstone_id TEXT PRIMARY KEY,
  object_type TEXT NOT NULL,
  object_id TEXT NOT NULL,
  deleted_by_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  deleted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  reason TEXT,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  UNIQUE(object_type, object_id)
) STRICT;

CREATE INDEX idx_memory_home_scope ON memory_items(home_scope_id, status, updated_at DESC);
CREATE INDEX idx_memory_type_status ON memory_items(memory_type, status);
CREATE INDEX idx_memory_owner ON memory_items(owner_principal_id);
CREATE INDEX idx_memory_created_by ON memory_items(created_by_principal_id);
CREATE INDEX idx_memory_authority ON memory_items(authority_level);
CREATE INDEX idx_memory_validity ON memory_items(valid_from, valid_to);
CREATE INDEX idx_memory_proactivity ON memory_items(proactivity_level, status);
CREATE INDEX idx_episodes_scope_time ON episodes(home_scope_id, occurred_at DESC);
CREATE INDEX idx_messages_episode_time ON messages(episode_id, occurred_at ASC);
CREATE INDEX idx_entities_scope_type ON entities(home_scope_id, entity_type);
CREATE INDEX idx_relationships_subject ON relationships(subject_entity_id, predicate);
CREATE INDEX idx_relationships_object ON relationships(object_entity_id, predicate);
CREATE INDEX idx_provenance_memory ON memory_provenance_edges(memory_id);
CREATE INDEX idx_access_principal ON memory_access_grants(principal_id, permission, effect);
CREATE INDEX idx_events_memory_time ON memory_events(memory_id, created_at DESC);

-- Optional rebuildable search indexes, preferably stored under system/indexes/.
CREATE VIRTUAL TABLE memory_fts USING fts5(
  memory_id UNINDEXED,
  title,
  content,
  tokenize = 'porter unicode61'
);

CREATE VIRTUAL TABLE episode_fts USING fts5(
  episode_id UNINDEXED,
  title,
  summary,
  tokenize = 'porter unicode61'
);
```