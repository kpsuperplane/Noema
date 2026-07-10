CREATE TABLE schema_state (
  name TEXT PRIMARY KEY NOT NULL CHECK (name = 'sqlite_store_v2'),
  version INTEGER NOT NULL CHECK (version = 2),
  structural_fingerprint TEXT NOT NULL CHECK (structural_fingerprint <> ''),
  applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (applied_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', applied_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', applied_at) = applied_at))
);

CREATE TABLE humans (
  human_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT NOT NULL,
  primary_conversation_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  FOREIGN KEY (primary_conversation_id) REFERENCES conversations(conversation_id)
    ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE agents (
  agent_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at))
);

CREATE TABLE agent_runtime_preferences (
  agent_id TEXT PRIMARY KEY NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local')),
  provider_account_id TEXT NOT NULL,
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  FOREIGN KEY (agent_id) REFERENCES agents(agent_id) ON DELETE CASCADE,
  FOREIGN KEY (provider_account_id) REFERENCES provider_accounts(provider_account_id) ON DELETE CASCADE
);

CREATE TABLE auxiliary_model_preferences (
  task_id TEXT PRIMARY KEY NOT NULL CHECK (task_id IN ('web_fetch_summarizer', 'tool_progress_audit', 'memory_extraction')),
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local')),
  provider_account_id TEXT NOT NULL,
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  FOREIGN KEY (provider_account_id) REFERENCES provider_accounts(provider_account_id) ON DELETE CASCADE
);

CREATE TABLE provider_accounts (
  provider_account_id TEXT PRIMARY KEY NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'exa')),
  account_key TEXT NOT NULL,
  display_name TEXT NOT NULL,
  auth_method TEXT NOT NULL CHECK (auth_method IN ('oauth_device_code', 'secret_input', 'external_manual', 'none')),
  is_active INTEGER NOT NULL CHECK (is_active IN (0, 1)),
  is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
  status TEXT NOT NULL CHECK (status IN ('unknown', 'checking', 'authenticated', 'unauthenticated', 'unavailable')),
  last_checked_at TEXT CHECK (last_checked_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', last_checked_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', last_checked_at) = last_checked_at)),
  last_authenticated_at TEXT CHECK (last_authenticated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', last_authenticated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', last_authenticated_at) = last_authenticated_at)),
  last_error_code TEXT,
  last_error_message TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at))
);

CREATE UNIQUE INDEX provider_accounts_identity
ON provider_accounts(provider_kind, account_key);

CREATE TABLE provider_capability_bindings (
  binding_id TEXT PRIMARY KEY NOT NULL,
  tool_name TEXT NOT NULL CHECK (tool_name IN ('web.search', 'web.fetch')),
  capability_id TEXT NOT NULL,
  provider_account_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  FOREIGN KEY (provider_account_id) REFERENCES provider_accounts(provider_account_id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX provider_capability_bindings_identity
ON provider_capability_bindings(tool_name, capability_id);

CREATE TABLE conversations (
  conversation_id TEXT PRIMARY KEY NOT NULL,
  title TEXT,
  owner_human_id TEXT,
  owner_agent_id TEXT,
  owner_conversation_id TEXT,
  primary_human_id TEXT,
  primary_agent_id TEXT,
  is_primary INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0, 1)),
  provider TEXT NOT NULL CHECK (provider IN ('codex', 'openai', 'foundation_local')),
  model TEXT,
  cwd TEXT,
  lifecycle_status TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle_status IN ('active', 'archived')),
  agent_status TEXT NOT NULL DEFAULT 'idle' CHECK (agent_status IN ('idle', 'input_received', 'thinking', 'tool_running', 'waiting_for_previous_turn_completion', 'interrupting', 'error')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  deleted_at TEXT CHECK (deleted_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', deleted_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', deleted_at) = deleted_at)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  CHECK (
    (owner_human_id IS NOT NULL)
    + (owner_agent_id IS NOT NULL)
    + (owner_conversation_id IS NOT NULL) = 1
  ),
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (owner_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT,
  FOREIGN KEY (owner_conversation_id) REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY (primary_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (primary_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT,
  UNIQUE(primary_human_id, conversation_id)
);

CREATE UNIQUE INDEX conversations_one_primary_per_human
ON conversations(primary_human_id)
WHERE is_primary = 1 AND primary_human_id IS NOT NULL AND deleted_at IS NULL;

CREATE UNIQUE INDEX humans_primary_conversation_unique
ON humans(primary_conversation_id)
WHERE primary_conversation_id IS NOT NULL;

CREATE TABLE conversation_turns (
  turn_id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  trigger_item_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('input_received', 'running', 'waiting_for_tool', 'interrupted', 'completed', 'failed', 'cancelled')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (started_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', started_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', started_at) = started_at)),
  completed_at TEXT CHECK (completed_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', completed_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', completed_at) = completed_at)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE CASCADE,
  FOREIGN KEY (trigger_item_id) REFERENCES conversation_items(item_id) ON DELETE SET NULL
);

CREATE INDEX conversation_turns_conversation_id
ON conversation_turns(conversation_id);

CREATE TABLE conversation_items (
  item_id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  turn_id TEXT,
  parent_item_id TEXT,
  sequence_index INTEGER NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('user_text', 'assistant_text', 'activity', 'a2ui_card', 'multiple_choice_prompt', 'multiple_choice_selection', 'tool_call', 'tool_result', 'reasoning', 'approval_request', 'approval_result', 'error_notice', 'artifact_reference')),
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled', 'interrupted')),
  author_human_id TEXT,
  author_agent_id TEXT,
  content_text TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  deleted_at TEXT CHECK (deleted_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', deleted_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', deleted_at) = deleted_at)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  UNIQUE(conversation_id, sequence_index),
  CHECK ((author_human_id IS NOT NULL) + (author_agent_id IS NOT NULL) = 1),
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE CASCADE,
  FOREIGN KEY (turn_id) REFERENCES conversation_turns(turn_id) ON DELETE SET NULL,
  FOREIGN KEY (parent_item_id) REFERENCES conversation_items(item_id) ON DELETE SET NULL,
  FOREIGN KEY (author_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (author_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT
);

CREATE INDEX conversation_items_conversation_sequence
ON conversation_items(conversation_id, sequence_index);

CREATE TABLE conversation_context_summaries (
  summary_id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local')),
  model_profile TEXT CHECK (model_profile IS NULL OR model_profile <> ''),
  summary_text TEXT NOT NULL,
  covered_item_start_sequence INTEGER NOT NULL CHECK (covered_item_start_sequence >= 1),
  covered_item_end_sequence INTEGER NOT NULL CHECK (covered_item_end_sequence >= covered_item_start_sequence),
  source_item_ids_json TEXT NOT NULL DEFAULT '[]',
  input_token_estimate INTEGER NOT NULL CHECK (input_token_estimate >= 0),
  summary_token_estimate INTEGER NOT NULL CHECK (summary_token_estimate >= 0),
  compaction_provider_kind TEXT NOT NULL CHECK (compaction_provider_kind IN ('codex', 'openai', 'foundation_local')),
  compaction_model_profile TEXT,
  status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'failed', 'superseded')),
  error_code TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE CASCADE
);

CREATE INDEX conversation_context_summaries_profile
ON conversation_context_summaries(conversation_id, provider_kind, model_profile, status, covered_item_end_sequence);

CREATE UNIQUE INDEX conversation_context_summaries_one_active
ON conversation_context_summaries(conversation_id, provider_kind, COALESCE(model_profile, ''))
WHERE status = 'active';

CREATE TABLE artifacts (
  artifact_id TEXT PRIMARY KEY NOT NULL,
  owner_human_id TEXT,
  owner_agent_id TEXT,
  owner_conversation_id TEXT,
  title TEXT NOT NULL CHECK (title <> ''),
  description TEXT,
  artifact_kind TEXT NOT NULL CHECK (artifact_kind <> ''),
  storage_kind TEXT NOT NULL CHECK (storage_kind IN ('local_file', 'external_url')),
  current_version_id TEXT,
  created_by_human_id TEXT,
  created_by_agent_id TEXT,
  source_conversation_id TEXT,
  source_turn_id TEXT,
  source_item_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  deleted_at TEXT CHECK (deleted_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', deleted_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', deleted_at) = deleted_at)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  CHECK (
    (owner_human_id IS NOT NULL)
    + (owner_agent_id IS NOT NULL)
    + (owner_conversation_id IS NOT NULL) = 1
  ),
  CHECK ((created_by_human_id IS NOT NULL) + (created_by_agent_id IS NOT NULL) = 1),
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (owner_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT,
  FOREIGN KEY (owner_conversation_id) REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY (created_by_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (created_by_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT,
  FOREIGN KEY (artifact_id, current_version_id)
    REFERENCES artifact_versions(artifact_id, artifact_version_id)
    ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (source_conversation_id) REFERENCES conversations(conversation_id) ON DELETE SET NULL,
  FOREIGN KEY (source_turn_id) REFERENCES conversation_turns(turn_id) ON DELETE SET NULL,
  FOREIGN KEY (source_item_id) REFERENCES conversation_items(item_id) ON DELETE SET NULL
);

CREATE TABLE artifact_versions (
  artifact_version_id TEXT PRIMARY KEY NOT NULL,
  artifact_id TEXT NOT NULL,
  version_index INTEGER NOT NULL CHECK (version_index >= 1),
  title TEXT,
  local_relative_path TEXT,
  external_url TEXT,
  media_type TEXT,
  byte_size INTEGER CHECK (byte_size IS NULL OR byte_size >= 0),
  content_sha256 TEXT,
  created_by_human_id TEXT,
  created_by_agent_id TEXT,
  source_conversation_id TEXT,
  source_turn_id TEXT,
  source_item_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  UNIQUE(artifact_id, version_index),
  UNIQUE(artifact_id, artifact_version_id),
  CHECK ((created_by_human_id IS NOT NULL) + (created_by_agent_id IS NOT NULL) = 1),
  CHECK (
    (local_relative_path IS NOT NULL AND external_url IS NULL)
    OR (local_relative_path IS NULL AND external_url IS NOT NULL)
  ),
  FOREIGN KEY (artifact_id) REFERENCES artifacts(artifact_id) ON DELETE CASCADE,
  FOREIGN KEY (created_by_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (created_by_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT,
  FOREIGN KEY (source_conversation_id) REFERENCES conversations(conversation_id) ON DELETE SET NULL,
  FOREIGN KEY (source_turn_id) REFERENCES conversation_turns(turn_id) ON DELETE SET NULL,
  FOREIGN KEY (source_item_id) REFERENCES conversation_items(item_id) ON DELETE SET NULL
);

CREATE INDEX artifacts_owner
ON artifacts(owner_human_id, owner_agent_id, owner_conversation_id, deleted_at, updated_at);

CREATE INDEX artifact_versions_artifact
ON artifact_versions(artifact_id, version_index);

CREATE TABLE mcp_servers (
  mcp_server_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT NOT NULL,
  transport_kind TEXT NOT NULL CHECK (transport_kind IN ('stdio', 'sse', 'streamable_http')),
  safe_config_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(safe_config_json)),
  auth_status TEXT NOT NULL CHECK (auth_status IN ('none', 'needs_auth', 'authenticated', 'unavailable')),
  health_status TEXT NOT NULL CHECK (health_status IN ('unknown', 'healthy', 'unavailable')),
  enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
  metadata_fingerprint TEXT,
  last_discovered_at TEXT CHECK (last_discovered_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', last_discovered_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', last_discovered_at) = last_discovered_at)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  current_discovery_snapshot_id TEXT,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  FOREIGN KEY (current_discovery_snapshot_id) REFERENCES mcp_discovery_snapshots(snapshot_id)
    ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE mcp_tools (
  mcp_tool_id TEXT PRIMARY KEY NOT NULL,
  mcp_server_id TEXT NOT NULL,
  name TEXT NOT NULL,
  remote_name TEXT GENERATED ALWAYS AS (name) STORED,
  description TEXT,
  input_schema_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(input_schema_json)),
  output_schema_json TEXT CHECK (output_schema_json IS NULL OR json_valid(output_schema_json)),
  annotations_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(annotations_json)),
  metadata_fingerprint TEXT NOT NULL,
  discovered_at TEXT NOT NULL CHECK (discovered_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', discovered_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', discovered_at) = discovered_at)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  FOREIGN KEY (mcp_server_id) REFERENCES mcp_servers(mcp_server_id) ON DELETE CASCADE,
  UNIQUE(mcp_server_id, mcp_tool_id)
);

CREATE UNIQUE INDEX mcp_tools_server_remote_name
ON mcp_tools(mcp_server_id, remote_name);

CREATE TABLE tool_calibrations (
  calibration_id TEXT PRIMARY KEY NOT NULL,
  mcp_tool_id TEXT NOT NULL UNIQUE,
  read_classification TEXT NOT NULL CHECK (read_classification IN ('none', 'trusted', 'untrusted', 'mixed')),
  write_classification TEXT NOT NULL CHECK (write_classification IN ('none', 'trusted', 'untrusted', 'mixed')),
  export_classification TEXT NOT NULL CHECK (export_classification IN ('none', 'trusted', 'untrusted', 'mixed')),
  owner_extractors_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(owner_extractors_json) AND json_type(owner_extractors_json) = 'array'),
  status TEXT NOT NULL CHECK (status IN ('needs_review', 'blocked_unresolved_ownership', 'ready', 'disabled')),
  reviewed_by_human_id TEXT,
  reviewed_by_agent_id TEXT,
  reviewed_metadata_fingerprint TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  CHECK ((reviewed_by_human_id IS NOT NULL) + (reviewed_by_agent_id IS NOT NULL) <= 1),
  CHECK (status NOT IN ('blocked_unresolved_ownership', 'ready') OR reviewed_by_human_id IS NOT NULL OR reviewed_by_agent_id IS NOT NULL),
  FOREIGN KEY (mcp_tool_id) REFERENCES mcp_tools(mcp_tool_id) ON DELETE CASCADE,
  FOREIGN KEY (reviewed_by_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (reviewed_by_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT
);

CREATE TABLE trusted_identity_selectors (
  selector_id TEXT PRIMARY KEY NOT NULL,
  owner_human_id TEXT,
  owner_agent_id TEXT,
  selector_kind TEXT NOT NULL CHECK (selector_kind IN ('email', 'phone', 'domain')),
  normalized_value TEXT NOT NULL CHECK (normalized_value <> ''),
  effect TEXT NOT NULL CHECK (effect IN ('trust', 'restrict')),
  issuer_human_id TEXT,
  issuer_agent_id TEXT,
  revoked_at TEXT CHECK (revoked_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', revoked_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', revoked_at) = revoked_at)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  CHECK ((owner_human_id IS NOT NULL) + (owner_agent_id IS NOT NULL) = 1),
  CHECK ((issuer_human_id IS NOT NULL) + (issuer_agent_id IS NOT NULL) = 1),
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (owner_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT,
  FOREIGN KEY (issuer_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (issuer_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX trusted_identity_selectors_owner_value
ON trusted_identity_selectors(
  COALESCE(owner_human_id, ''), COALESCE(owner_agent_id, ''), selector_kind, normalized_value
);

CREATE TABLE approval_requests (
  approval_id TEXT PRIMARY KEY NOT NULL,
  action_summary TEXT NOT NULL,
  tool_invocation_id TEXT NOT NULL,
  mcp_server_id TEXT,
  mcp_tool_id TEXT,
  requester_human_id TEXT,
  requester_agent_id TEXT,
  owner_human_id TEXT,
  owner_agent_id TEXT,
  active_human_id TEXT,
  active_agent_id TEXT,
  destination_summary TEXT NOT NULL,
  data_source_summary TEXT NOT NULL,
  source_owner_identity TEXT NOT NULL,
  source_owner_trust TEXT NOT NULL CHECK (source_owner_trust IN ('trusted', 'untrusted', 'mixed', 'unresolved')),
  destination_owner_identity TEXT NOT NULL,
  destination_owner_trust TEXT NOT NULL CHECK (destination_owner_trust IN ('trusted', 'untrusted', 'mixed', 'unresolved')),
  export_summary TEXT NOT NULL,
  redacted_review_json TEXT NOT NULL DEFAULT '{}' CHECK (redacted_review_json = '{}'),
  attempt_fingerprint TEXT NOT NULL DEFAULT '',
  policy_fingerprint TEXT NOT NULL DEFAULT '',
  tool_fingerprint TEXT NOT NULL DEFAULT '',
  calibration_fingerprint TEXT,
  expires_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+5 minutes')) CHECK (expires_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', expires_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', expires_at) = expires_at)),
  status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'denied', 'cancelled', 'expired', 'consumed')),
  decision_human_id TEXT,
  decision_agent_id TEXT,
  decision_comment TEXT,
  decided_at TEXT CHECK (decided_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', decided_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', decided_at) = decided_at)),
  consumed_at TEXT CHECK (consumed_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', consumed_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', consumed_at) = consumed_at)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  UNIQUE(tool_invocation_id),
  CHECK ((requester_human_id IS NOT NULL) + (requester_agent_id IS NOT NULL) = 1),
  CHECK ((owner_human_id IS NOT NULL) + (owner_agent_id IS NOT NULL) = 1),
  CHECK ((active_human_id IS NOT NULL) + (active_agent_id IS NOT NULL) = 1),
  CHECK ((decision_human_id IS NOT NULL) + (decision_agent_id IS NOT NULL) <= 1),
  CHECK (
    (mcp_server_id IS NULL AND mcp_tool_id IS NULL)
    OR (mcp_server_id IS NOT NULL AND mcp_tool_id IS NOT NULL)
  ),
  CHECK (status = 'pending' OR decided_at IS NOT NULL),
  CHECK (status <> 'consumed' OR consumed_at IS NOT NULL),
  FOREIGN KEY (mcp_server_id) REFERENCES mcp_servers(mcp_server_id) ON DELETE SET NULL,
  FOREIGN KEY (mcp_server_id, mcp_tool_id)
    REFERENCES mcp_tools(mcp_server_id, mcp_tool_id) ON DELETE SET NULL,
  FOREIGN KEY (requester_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (requester_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT,
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (owner_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT,
  FOREIGN KEY (active_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (active_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT,
  FOREIGN KEY (decision_human_id) REFERENCES humans(human_id) ON DELETE SET NULL,
  FOREIGN KEY (decision_agent_id) REFERENCES agents(agent_id) ON DELETE SET NULL
);

CREATE TABLE policy_decisions (
  policy_decision_id TEXT PRIMARY KEY NOT NULL,
  owner_human_id TEXT,
  owner_agent_id TEXT,
  conversation_id TEXT,
  decision TEXT NOT NULL CHECK (decision IN ('allow', 'deny', 'require_approval')),
  reason_code TEXT NOT NULL,
  policy_fingerprint TEXT NOT NULL,
  tool_fingerprint TEXT,
  calibration_fingerprint TEXT,
  decided_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (decided_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', decided_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', decided_at) = decided_at)),
  CHECK ((owner_human_id IS NOT NULL) + (owner_agent_id IS NOT NULL) = 1),
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (owner_agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT,
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE SET NULL
);

CREATE TABLE mcp_discovery_snapshots (
  snapshot_id TEXT PRIMARY KEY NOT NULL,
  mcp_server_id TEXT NOT NULL,
  metadata_fingerprint TEXT NOT NULL,
  is_current INTEGER NOT NULL DEFAULT 0 CHECK (is_current IN (0, 1)),
  discovered_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (discovered_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', discovered_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', discovered_at) = discovered_at)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  FOREIGN KEY (mcp_server_id) REFERENCES mcp_servers(mcp_server_id) ON DELETE CASCADE,
  UNIQUE(mcp_server_id, snapshot_id)
);

CREATE UNIQUE INDEX mcp_discovery_snapshots_one_current
ON mcp_discovery_snapshots(mcp_server_id)
WHERE is_current = 1;

CREATE TABLE mcp_discovery_snapshot_tools (
  snapshot_id TEXT NOT NULL,
  mcp_tool_id TEXT NOT NULL,
  PRIMARY KEY (snapshot_id, mcp_tool_id),
  FOREIGN KEY (snapshot_id) REFERENCES mcp_discovery_snapshots(snapshot_id) ON DELETE CASCADE,
  FOREIGN KEY (mcp_tool_id) REFERENCES mcp_tools(mcp_tool_id) ON DELETE CASCADE
);

CREATE TABLE persistence_sagas (
  saga_id TEXT PRIMARY KEY NOT NULL,
  saga_kind TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('prepared', 'committing', 'completed', 'failed', 'needs_recovery')),
  object_fingerprint TEXT NOT NULL,
  safe_context_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(safe_context_json)),
  prepared_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (prepared_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', prepared_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', prepared_at) = prepared_at)),
  completed_at TEXT CHECK (completed_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', completed_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', completed_at) = completed_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at))
);

CREATE TABLE memory_service_settings (
  settings_id TEXT PRIMARY KEY NOT NULL CHECK (settings_id = 'default'),
  mode TEXT NOT NULL CHECK (mode IN ('managed', 'external')),
  base_url TEXT,
  port INTEGER CHECK (port IS NULL OR (port > 0 AND port <= 65535)),
  provider_account_id TEXT,
  provider_kind TEXT CHECK (provider_kind IS NULL OR provider_kind IN ('codex', 'openai', 'foundation_local')),
  model_profile TEXT,
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at)),
  CHECK (mode = 'managed' OR base_url IS NOT NULL),
  FOREIGN KEY (provider_account_id) REFERENCES provider_accounts(provider_account_id) ON DELETE SET NULL
);

INSERT INTO memory_service_settings (settings_id, mode, base_url, port)
VALUES ('default', 'managed', NULL, NULL)
ON CONFLICT(settings_id) DO NOTHING;

CREATE TABLE memory_article_cache (
  scope_id TEXT PRIMARY KEY NOT NULL,
  fact_fingerprint TEXT NOT NULL,
  article_markdown TEXT NOT NULL,
  generated_at TEXT NOT NULL CHECK (generated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', generated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', generated_at) = generated_at)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) = created_at)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at IS NULL OR (strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS NOT NULL AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) = updated_at))
);

CREATE TRIGGER schema_state_reject_update
BEFORE UPDATE ON schema_state
BEGIN
  SELECT RAISE(ABORT, 'schema_state marker is immutable');
END;
CREATE TRIGGER schema_state_reject_reinsert
BEFORE INSERT ON schema_state
WHEN EXISTS (SELECT 1 FROM schema_state)
BEGIN
  SELECT RAISE(ABORT, 'schema_state marker already exists');
END;
CREATE TRIGGER schema_state_reject_delete
BEFORE DELETE ON schema_state
BEGIN
  SELECT RAISE(ABORT, 'schema_state marker is immutable');
END;

CREATE TRIGGER conversation_turn_trigger_item_same_conversation_insert
BEFORE INSERT ON conversation_turns
WHEN NEW.trigger_item_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM conversation_items
  WHERE item_id = NEW.trigger_item_id AND conversation_id = NEW.conversation_id
)
BEGIN
  SELECT RAISE(ABORT, 'turn trigger item must belong to the same conversation');
END;

CREATE TRIGGER human_primary_conversation_same_human_insert
BEFORE INSERT ON humans
WHEN NEW.primary_conversation_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM conversations
  WHERE conversation_id = NEW.primary_conversation_id
    AND primary_human_id = NEW.human_id
    AND is_primary = 1
)
BEGIN
  SELECT RAISE(ABORT, 'primary conversation must belong to the human');
END;

CREATE TRIGGER human_primary_conversation_same_human_update
BEFORE UPDATE OF human_id, primary_conversation_id ON humans
WHEN NEW.primary_conversation_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM conversations
  WHERE conversation_id = NEW.primary_conversation_id
    AND primary_human_id = NEW.human_id
    AND is_primary = 1
)
BEGIN
  SELECT RAISE(ABORT, 'primary conversation must belong to the human');
END;

CREATE TRIGGER mcp_current_snapshot_same_server_insert
BEFORE INSERT ON mcp_servers
WHEN NEW.current_discovery_snapshot_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM mcp_discovery_snapshots
  WHERE snapshot_id = NEW.current_discovery_snapshot_id
    AND mcp_server_id = NEW.mcp_server_id
    AND is_current = 1
)
BEGIN
  SELECT RAISE(ABORT, 'current MCP snapshot must belong to the server');
END;

CREATE TRIGGER mcp_current_snapshot_same_server_update
BEFORE UPDATE OF mcp_server_id, current_discovery_snapshot_id ON mcp_servers
WHEN NEW.current_discovery_snapshot_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM mcp_discovery_snapshots
  WHERE snapshot_id = NEW.current_discovery_snapshot_id
    AND mcp_server_id = NEW.mcp_server_id
    AND is_current = 1
)
BEGIN
  SELECT RAISE(ABORT, 'current MCP snapshot must belong to the server');
END;

CREATE TRIGGER conversation_turn_trigger_item_same_conversation_update
BEFORE UPDATE OF conversation_id, trigger_item_id ON conversation_turns
WHEN NEW.trigger_item_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM conversation_items
  WHERE item_id = NEW.trigger_item_id AND conversation_id = NEW.conversation_id
)
BEGIN
  SELECT RAISE(ABORT, 'turn trigger item must belong to the same conversation');
END;

CREATE TRIGGER conversation_item_links_same_conversation_insert
BEFORE INSERT ON conversation_items
WHEN (NEW.turn_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_turns
    WHERE turn_id = NEW.turn_id AND conversation_id = NEW.conversation_id
  )) OR (NEW.parent_item_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_items
    WHERE item_id = NEW.parent_item_id AND conversation_id = NEW.conversation_id
  ))
BEGIN
  SELECT RAISE(ABORT, 'conversation item links must belong to the same conversation');
END;

CREATE TRIGGER conversation_item_links_same_conversation_update
BEFORE UPDATE OF conversation_id, turn_id, parent_item_id ON conversation_items
WHEN (NEW.turn_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_turns
    WHERE turn_id = NEW.turn_id AND conversation_id = NEW.conversation_id
  )) OR (NEW.parent_item_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_items
    WHERE item_id = NEW.parent_item_id AND conversation_id = NEW.conversation_id
  ))
BEGIN
  SELECT RAISE(ABORT, 'conversation item links must belong to the same conversation');
END;

CREATE TRIGGER artifact_provenance_same_conversation_insert
BEFORE INSERT ON artifacts
WHEN (NEW.source_conversation_id IS NOT NULL AND NEW.source_turn_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_turns
    WHERE turn_id = NEW.source_turn_id AND conversation_id = NEW.source_conversation_id
  )) OR (NEW.source_conversation_id IS NOT NULL AND NEW.source_item_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_items
    WHERE item_id = NEW.source_item_id AND conversation_id = NEW.source_conversation_id
  )) OR (NEW.source_turn_id IS NOT NULL AND NEW.source_item_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_turns AS turns
    JOIN conversation_items AS items ON items.conversation_id = turns.conversation_id
    WHERE turns.turn_id = NEW.source_turn_id AND items.item_id = NEW.source_item_id
  ))
BEGIN
  SELECT RAISE(ABORT, 'artifact provenance must belong to the source conversation');
END;

CREATE TRIGGER artifact_provenance_same_conversation_update
BEFORE UPDATE OF source_conversation_id, source_turn_id, source_item_id ON artifacts
WHEN (NEW.source_conversation_id IS NOT NULL AND NEW.source_turn_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_turns
    WHERE turn_id = NEW.source_turn_id AND conversation_id = NEW.source_conversation_id
  )) OR (NEW.source_conversation_id IS NOT NULL AND NEW.source_item_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_items
    WHERE item_id = NEW.source_item_id AND conversation_id = NEW.source_conversation_id
  )) OR (NEW.source_turn_id IS NOT NULL AND NEW.source_item_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_turns AS turns
    JOIN conversation_items AS items ON items.conversation_id = turns.conversation_id
    WHERE turns.turn_id = NEW.source_turn_id AND items.item_id = NEW.source_item_id
  ))
BEGIN
  SELECT RAISE(ABORT, 'artifact provenance must belong to the source conversation');
END;

CREATE TRIGGER artifact_version_provenance_same_conversation_insert
BEFORE INSERT ON artifact_versions
WHEN (NEW.source_conversation_id IS NOT NULL AND NEW.source_turn_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_turns
    WHERE turn_id = NEW.source_turn_id AND conversation_id = NEW.source_conversation_id
  )) OR (NEW.source_conversation_id IS NOT NULL AND NEW.source_item_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_items
    WHERE item_id = NEW.source_item_id AND conversation_id = NEW.source_conversation_id
  )) OR (NEW.source_turn_id IS NOT NULL AND NEW.source_item_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_turns AS turns
    JOIN conversation_items AS items ON items.conversation_id = turns.conversation_id
    WHERE turns.turn_id = NEW.source_turn_id AND items.item_id = NEW.source_item_id
  ))
BEGIN
  SELECT RAISE(ABORT, 'artifact version provenance must belong to the source conversation');
END;
CREATE TRIGGER artifact_version_provenance_same_conversation_update
BEFORE UPDATE OF source_conversation_id, source_turn_id, source_item_id ON artifact_versions
WHEN (NEW.source_conversation_id IS NOT NULL AND NEW.source_turn_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_turns
    WHERE turn_id = NEW.source_turn_id AND conversation_id = NEW.source_conversation_id
  )) OR (NEW.source_conversation_id IS NOT NULL AND NEW.source_item_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_items
    WHERE item_id = NEW.source_item_id AND conversation_id = NEW.source_conversation_id
  )) OR (NEW.source_turn_id IS NOT NULL AND NEW.source_item_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_turns AS turns
    JOIN conversation_items AS items ON items.conversation_id = turns.conversation_id
    WHERE turns.turn_id = NEW.source_turn_id AND items.item_id = NEW.source_item_id
  ))
BEGIN
  SELECT RAISE(ABORT, 'artifact version provenance must belong to the source conversation');
END;

CREATE TRIGGER mcp_snapshot_membership_same_server_insert
BEFORE INSERT ON mcp_discovery_snapshot_tools
WHEN NOT EXISTS (
  SELECT 1
  FROM mcp_discovery_snapshots AS snapshots
  JOIN mcp_tools AS tools ON tools.mcp_server_id = snapshots.mcp_server_id
  WHERE snapshots.snapshot_id = NEW.snapshot_id
    AND tools.mcp_tool_id = NEW.mcp_tool_id
)
BEGIN
  SELECT RAISE(ABORT, 'snapshot membership tool must belong to the snapshot server');
END;

CREATE TRIGGER mcp_snapshot_membership_same_server_update
BEFORE UPDATE OF snapshot_id, mcp_tool_id ON mcp_discovery_snapshot_tools
WHEN NOT EXISTS (
  SELECT 1
  FROM mcp_discovery_snapshots AS snapshots
  JOIN mcp_tools AS tools ON tools.mcp_server_id = snapshots.mcp_server_id
  WHERE snapshots.snapshot_id = NEW.snapshot_id
    AND tools.mcp_tool_id = NEW.mcp_tool_id
)
BEGIN
  SELECT RAISE(ABORT, 'snapshot membership tool must belong to the snapshot server');
END;

CREATE TRIGGER policy_owner_matches_conversation_insert
BEFORE INSERT ON policy_decisions
WHEN NEW.conversation_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM conversations
  WHERE conversation_id = NEW.conversation_id
    AND owner_human_id IS NEW.owner_human_id
    AND owner_agent_id IS NEW.owner_agent_id
)
BEGIN
  SELECT RAISE(ABORT, 'policy owner must match the conversation owner');
END;

CREATE TRIGGER policy_owner_matches_conversation_update
BEFORE UPDATE OF conversation_id, owner_human_id, owner_agent_id ON policy_decisions
WHEN NEW.conversation_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM conversations
  WHERE conversation_id = NEW.conversation_id
    AND owner_human_id IS NEW.owner_human_id
    AND owner_agent_id IS NEW.owner_agent_id
)
BEGIN
  SELECT RAISE(ABORT, 'policy owner must match the conversation owner');
END;

CREATE TRIGGER conversation_relationships_immutable
BEFORE UPDATE OF owner_human_id, owner_agent_id, owner_conversation_id, primary_human_id, is_primary
ON conversations
WHEN OLD.owner_human_id IS NOT NEW.owner_human_id
  OR OLD.owner_agent_id IS NOT NEW.owner_agent_id
  OR OLD.owner_conversation_id IS NOT NEW.owner_conversation_id
  OR OLD.primary_human_id IS NOT NEW.primary_human_id
  OR OLD.is_primary IS NOT NEW.is_primary
BEGIN
  SELECT RAISE(ABORT, 'conversation ownership and primary identity are immutable');
END;

CREATE TRIGGER conversation_turn_conversation_immutable
BEFORE UPDATE OF conversation_id ON conversation_turns
WHEN OLD.conversation_id <> NEW.conversation_id
BEGIN
  SELECT RAISE(ABORT, 'turn conversation is immutable');
END;

CREATE TRIGGER conversation_item_conversation_immutable
BEFORE UPDATE OF conversation_id ON conversation_items
WHEN OLD.conversation_id <> NEW.conversation_id
BEGIN
  SELECT RAISE(ABORT, 'item conversation is immutable');
END;

CREATE TRIGGER mcp_tool_server_immutable
BEFORE UPDATE OF mcp_server_id ON mcp_tools
WHEN OLD.mcp_server_id <> NEW.mcp_server_id
BEGIN
  SELECT RAISE(ABORT, 'MCP tool server is immutable');
END;

CREATE TRIGGER approval_links_clear_before_mcp_tool_delete
BEFORE DELETE ON mcp_tools
BEGIN
  UPDATE approval_requests
  SET mcp_server_id = NULL, mcp_tool_id = NULL
  WHERE mcp_server_id = OLD.mcp_server_id AND mcp_tool_id = OLD.mcp_tool_id;
END;

CREATE TRIGGER approval_links_clear_before_mcp_server_delete
BEFORE DELETE ON mcp_servers
BEGIN
  UPDATE approval_requests
  SET mcp_server_id = NULL, mcp_tool_id = NULL
  WHERE mcp_server_id = OLD.mcp_server_id;
END;

CREATE TRIGGER mcp_snapshot_server_immutable
BEFORE UPDATE OF mcp_server_id ON mcp_discovery_snapshots
WHEN OLD.mcp_server_id <> NEW.mcp_server_id
BEGIN
  SELECT RAISE(ABORT, 'MCP snapshot server is immutable');
END;

CREATE TRIGGER mcp_referenced_snapshot_stays_current
BEFORE UPDATE OF is_current ON mcp_discovery_snapshots
WHEN OLD.is_current = 1 AND NEW.is_current = 0 AND EXISTS (
  SELECT 1 FROM mcp_servers
  WHERE current_discovery_snapshot_id = OLD.snapshot_id
)
BEGIN
  SELECT RAISE(ABORT, 'referenced MCP snapshot must remain current');
END;
