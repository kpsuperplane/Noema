
CREATE TABLE IF NOT EXISTS schema_state (
  name TEXT PRIMARY KEY NOT NULL,
  version INTEGER NOT NULL CHECK (version >= 1),
  applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS humans (
  human_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT NOT NULL,
  primary_conversation_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS agents (
  agent_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT,
  system_role TEXT CHECK (system_role IS NULL OR system_role IN ('primary', 'task_executor', 'task_reviewer')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS agent_runtime_preferences (
  agent_id TEXT PRIMARY KEY NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  provider_account_id TEXT NOT NULL,
  provider_instance_key TEXT NOT NULL CHECK (provider_instance_key <> ''),
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS agent_runtime_preferences_instance
ON agent_runtime_preferences(provider_instance_key);

CREATE TABLE IF NOT EXISTS auxiliary_model_preferences (
  task_id TEXT PRIMARY KEY NOT NULL CHECK (task_id IN ('web_fetch_summarizer', 'tool_progress_audit', 'memory_extraction')),
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  provider_account_id TEXT NOT NULL,
  provider_instance_key TEXT NOT NULL CHECK (provider_instance_key <> ''),
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS auxiliary_model_preferences_instance
ON auxiliary_model_preferences(provider_instance_key);

CREATE TABLE IF NOT EXISTS provider_accounts (
  provider_account_id TEXT PRIMARY KEY NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models', 'exa')),
  account_key TEXT NOT NULL,
  display_name TEXT NOT NULL,
  auth_method TEXT NOT NULL CHECK (auth_method IN ('oauth_device_code', 'secret_input', 'external_manual', 'none')),
  is_active INTEGER NOT NULL CHECK (is_active IN (0, 1)),
  is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
  status TEXT NOT NULL CHECK (status IN ('unknown', 'checking', 'authenticated', 'unauthenticated', 'unavailable')),
  last_checked_at TEXT,
  last_authenticated_at TEXT,
  last_error_code TEXT,
  last_error_message TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(provider_kind, account_key)
);

CREATE TABLE IF NOT EXISTS provider_capability_bindings (
  binding_id TEXT PRIMARY KEY NOT NULL,
  tool_name TEXT NOT NULL CHECK (tool_name IN ('web.search', 'web.fetch')),
  capability_id TEXT NOT NULL,
  provider_account_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(tool_name, capability_id)
);

CREATE TABLE IF NOT EXISTS local_model_installations (
  installation_id TEXT PRIMARY KEY NOT NULL,
  provider_instance_key TEXT NOT NULL CHECK (provider_instance_key <> ''),
  model_id TEXT NOT NULL CHECK (model_id <> ''),
  display_name TEXT NOT NULL CHECK (display_name <> ''),
  source_kind TEXT NOT NULL CHECK (source_kind IN ('catalog', 'hugging_face', 'local_file')),
  source_repo TEXT,
  source_revision TEXT,
  source_file TEXT,
  sha256 TEXT CHECK (sha256 IS NULL OR (length(sha256) = 64 AND sha256 = lower(sha256))),
  download_gb REAL NOT NULL CHECK (download_gb > 0),
  expected_bytes INTEGER CHECK (expected_bytes IS NULL OR expected_bytes > 0),
  downloaded_bytes INTEGER NOT NULL DEFAULT 0 CHECK (downloaded_bytes >= 0),
  license TEXT,
  backend TEXT NOT NULL CHECK (backend IN ('metal', 'cuda', 'vulkan', 'cpu')),
  status TEXT NOT NULL CHECK (status IN ('queued', 'downloading', 'verifying', 'installed', 'failed', 'cancelled')),
  blob_relative_path TEXT,
  is_active INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)),
  runtime_retired_at TEXT,
  retirement_claimed_at TEXT,
  error_code TEXT,
  error_message TEXT,
  installed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (downloaded_bytes <= COALESCE(expected_bytes, downloaded_bytes)),
  CHECK (is_active = 0 OR (runtime_retired_at IS NULL AND retirement_claimed_at IS NULL)),
  CHECK (status <> 'installed' OR (sha256 IS NOT NULL AND blob_relative_path IS NOT NULL AND installed_at IS NOT NULL)),
  CHECK (source_kind <> 'catalog' OR (source_repo IS NOT NULL AND source_revision IS NOT NULL AND source_file IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS local_model_installations_model
ON local_model_installations(model_id, status, updated_at);
CREATE UNIQUE INDEX IF NOT EXISTS local_model_installations_provider_instance_key
ON local_model_installations(provider_instance_key);
CREATE UNIQUE INDEX IF NOT EXISTS local_model_installations_one_active
ON local_model_installations(is_active) WHERE is_active = 1;
CREATE INDEX IF NOT EXISTS local_model_installations_retirement_claim
ON local_model_installations(retirement_claimed_at) WHERE retirement_claimed_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS local_model_installations_runtime_retired
ON local_model_installations(runtime_retired_at) WHERE runtime_retired_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS local_model_events (
  cursor INTEGER PRIMARY KEY AUTOINCREMENT,
  installation_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('queued', 'progress', 'verifying', 'installed', 'failed', 'cancelled', 'removed', 'activated')),
  downloaded_bytes INTEGER CHECK (downloaded_bytes IS NULL OR downloaded_bytes >= 0),
  expected_bytes INTEGER CHECK (expected_bytes IS NULL OR expected_bytes > 0),
  message TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS local_model_events_installation_cursor
ON local_model_events(installation_id, cursor);

CREATE TABLE IF NOT EXISTS default_model_preference (
  preference_id TEXT PRIMARY KEY NOT NULL CHECK (preference_id = 'default'),
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  provider_account_id TEXT NOT NULL,
  provider_instance_key TEXT NOT NULL CHECK (provider_instance_key <> ''),
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS default_model_preference_instance
ON default_model_preference(provider_instance_key);

CREATE TABLE IF NOT EXISTS conversations (
  conversation_id TEXT PRIMARY KEY NOT NULL,
  title TEXT,
  owner_object_type TEXT NOT NULL CHECK (owner_object_type IN ('human', 'agent', 'conversation', 'workspace', 'project', 'task', 'tool')),
  owner_object_id TEXT NOT NULL,
  primary_human_id TEXT,
  primary_agent_id TEXT,
  provider TEXT NOT NULL CHECK (provider IN ('codex', 'openai', 'foundation_local', 'local_models')),
  model TEXT,
  cwd TEXT,
  lifecycle_status TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle_status IN ('active', 'archived')),
  agent_status TEXT NOT NULL DEFAULT 'idle' CHECK (agent_status IN ('idle', 'input_received', 'thinking', 'tool_running', 'waiting_for_previous_turn_completion', 'interrupting', 'error')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  deleted_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS conversation_turns (
  turn_id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  trigger_item_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('input_received', 'running', 'waiting_for_tool', 'interrupted', 'completed', 'failed', 'cancelled')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS conversation_turns_conversation_id
ON conversation_turns(conversation_id);

CREATE TABLE IF NOT EXISTS conversation_items (
  item_id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  turn_id TEXT,
  parent_item_id TEXT,
  sequence_index INTEGER NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('user_text', 'assistant_text', 'activity', 'a2ui_card', 'multiple_choice_prompt', 'multiple_choice_selection', 'tool_call', 'tool_result', 'reasoning', 'model_context_update', 'approval_request', 'approval_result', 'error_notice', 'artifact_reference', 'task_reference')),
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled', 'interrupted')),
  author_actor_id TEXT NOT NULL,
  content_text TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  deleted_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(conversation_id, sequence_index)
);

CREATE INDEX IF NOT EXISTS conversation_items_conversation_sequence
ON conversation_items(conversation_id, sequence_index);

CREATE TABLE IF NOT EXISTS conversation_context_summaries (
  summary_id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  model_profile TEXT,
  summary_text TEXT NOT NULL,
  covered_item_start_sequence INTEGER NOT NULL CHECK (covered_item_start_sequence >= 1),
  covered_item_end_sequence INTEGER NOT NULL CHECK (covered_item_end_sequence >= covered_item_start_sequence),
  source_item_ids_json TEXT NOT NULL DEFAULT '[]',
  input_token_estimate INTEGER NOT NULL CHECK (input_token_estimate >= 0),
  summary_token_estimate INTEGER NOT NULL CHECK (summary_token_estimate >= 0),
  compaction_provider_kind TEXT NOT NULL CHECK (compaction_provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  compaction_model_profile TEXT,
  status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'failed', 'superseded')),
  error_code TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS conversation_context_summaries_profile
ON conversation_context_summaries(conversation_id, provider_kind, model_profile, status, covered_item_end_sequence);

CREATE TABLE IF NOT EXISTS artifacts (
  artifact_id TEXT PRIMARY KEY NOT NULL,
  owner_object_type TEXT NOT NULL CHECK (owner_object_type IN ('human', 'agent', 'conversation', 'workspace', 'project', 'task', 'tool')),
  owner_object_id TEXT NOT NULL,
  title TEXT NOT NULL CHECK (title <> ''),
  description TEXT,
  artifact_kind TEXT NOT NULL CHECK (artifact_kind <> ''),
  storage_kind TEXT NOT NULL CHECK (storage_kind IN ('local_file', 'external_url')),
  current_version_id TEXT,
  created_by_actor_id TEXT NOT NULL,
  source_conversation_id TEXT,
  source_turn_id TEXT,
  source_item_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  deleted_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS artifact_versions (
  artifact_version_id TEXT PRIMARY KEY NOT NULL,
  artifact_id TEXT NOT NULL,
  version_index INTEGER NOT NULL CHECK (version_index >= 1),
  title TEXT,
  local_relative_path TEXT,
  external_url TEXT,
  media_type TEXT,
  byte_size INTEGER CHECK (byte_size IS NULL OR byte_size >= 0),
  content_sha256 TEXT,
  created_by_actor_id TEXT NOT NULL,
  source_conversation_id TEXT,
  source_turn_id TEXT,
  source_item_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(artifact_id, version_index),
  CHECK (
    (local_relative_path IS NOT NULL AND external_url IS NULL)
    OR (local_relative_path IS NULL AND external_url IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS artifacts_owner
ON artifacts(owner_object_type, owner_object_id, deleted_at, updated_at);

CREATE INDEX IF NOT EXISTS artifact_versions_artifact
ON artifact_versions(artifact_id, version_index);

CREATE TABLE IF NOT EXISTS mcp_servers (
  mcp_server_id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT NOT NULL,
  transport_kind TEXT NOT NULL CHECK (transport_kind IN ('stdio', 'streamable_http')),
  safe_config_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(safe_config_json)),
  auth_status TEXT NOT NULL CHECK (auth_status IN ('none', 'needs_auth', 'authenticated', 'unavailable')),
  health_status TEXT NOT NULL CHECK (health_status IN ('unknown', 'healthy', 'unavailable')),
  enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
  metadata_fingerprint TEXT,
  last_discovered_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS mcp_tools (
  mcp_tool_id TEXT PRIMARY KEY NOT NULL,
  mcp_server_id TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  input_schema_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(input_schema_json)),
  output_schema_json TEXT CHECK (output_schema_json IS NULL OR json_valid(output_schema_json)),
  annotations_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(annotations_json)),
  metadata_fingerprint TEXT NOT NULL,
  discovered_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(mcp_server_id, name)
);

CREATE TABLE IF NOT EXISTS tool_calibrations (
  calibration_id TEXT PRIMARY KEY NOT NULL,
  mcp_tool_id TEXT NOT NULL UNIQUE,
  read_classification TEXT NOT NULL CHECK (read_classification IN ('none', 'trusted', 'untrusted', 'mixed')),
  write_classification TEXT NOT NULL CHECK (write_classification IN ('none', 'trusted', 'untrusted', 'mixed')),
  export_classification TEXT NOT NULL CHECK (export_classification IN ('none', 'trusted', 'untrusted', 'mixed')),
  status TEXT NOT NULL CHECK (status IN ('needs_review', 'blocked_unresolved_ownership', 'ready', 'disabled')),
  reviewed_by TEXT,
  reviewed_metadata_fingerprint TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS memory_service_settings (
  settings_id TEXT PRIMARY KEY NOT NULL CHECK (settings_id = 'default'),
  mode TEXT NOT NULL CHECK (mode IN ('managed', 'external')),
  base_url TEXT,
  port INTEGER CHECK (port IS NULL OR (port > 0 AND port <= 65535)),
  provider_account_id TEXT,
  provider_instance_key TEXT CHECK (provider_instance_key IS NULL OR provider_instance_key <> ''),
  provider_kind TEXT CHECK (provider_kind IS NULL OR provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  model_profile TEXT,
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (mode = 'managed' OR base_url IS NOT NULL),
  CHECK (
    (provider_account_id IS NULL AND provider_instance_key IS NULL AND provider_kind IS NULL AND model_profile IS NULL)
    OR
    (provider_account_id IS NOT NULL AND provider_instance_key IS NOT NULL AND provider_kind IS NOT NULL AND model_profile IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS memory_service_settings_instance
ON memory_service_settings(provider_instance_key)
WHERE provider_instance_key IS NOT NULL;

INSERT INTO memory_service_settings (settings_id, mode, base_url, port)
VALUES ('default', 'managed', NULL, NULL)
ON CONFLICT(settings_id) DO NOTHING;

CREATE TABLE IF NOT EXISTS memory_article_cache (
  scope_id TEXT PRIMARY KEY NOT NULL,
  fact_fingerprint TEXT NOT NULL,
  article_markdown TEXT NOT NULL,
  generated_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);


