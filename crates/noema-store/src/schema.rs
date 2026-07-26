use rusqlite_migration::{M, Migrations};

/// Current forward-only SQLite migration version.
pub const STORE_SCHEMA_VERSION: usize = 7;

/// Marker used by the last exact-schema bootstrap before migrations existed.
pub(super) const LEGACY_SCHEMA_MARKER: &str = "sqlite_store_v9";

/// Frozen v9 baseline captured when forward migrations were introduced.
pub const LEGACY_V9_SCHEMA_SQL: &str = r#"

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
  is_override INTEGER NOT NULL DEFAULT 0 CHECK (is_override IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS agent_runtime_preferences_instance ON agent_runtime_preferences(provider_instance_key);

CREATE TABLE IF NOT EXISTS auxiliary_model_preferences (
  task_id TEXT PRIMARY KEY NOT NULL CHECK (task_id IN ('web_fetch_summarizer', 'tool_progress_audit', 'memory_extraction', 'action_reviewer')),
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  provider_account_id TEXT NOT NULL,
  provider_instance_key TEXT NOT NULL CHECK (provider_instance_key <> ''),
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  is_override INTEGER NOT NULL DEFAULT 0 CHECK (is_override IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS auxiliary_model_preferences_instance ON auxiliary_model_preferences(provider_instance_key);

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
  is_override INTEGER NOT NULL DEFAULT 0 CHECK (is_override IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS default_model_preference_instance ON default_model_preference(provider_instance_key);

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

-- The provider-backed task model pool is generic configuration retained
-- across the Work schema rewrite.
CREATE TABLE IF NOT EXISTS task_model_pool_entries (
  pool_entry_id TEXT PRIMARY KEY NOT NULL,
  complexity TEXT NOT NULL CHECK (complexity IN ('simple', 'medium', 'difficult')),
  label TEXT,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  provider_account_id TEXT NOT NULL,
  provider_instance_key TEXT NOT NULL CHECK (provider_instance_key <> ''),
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  is_override INTEGER NOT NULL DEFAULT 0 CHECK (is_override IN (0, 1)),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(complexity, provider_account_id, model_profile, reasoning_effort)
);

CREATE INDEX IF NOT EXISTS task_model_pool_entries_selection ON task_model_pool_entries(complexity, enabled, sort_order, label, pool_entry_id);
CREATE INDEX IF NOT EXISTS task_model_pool_entries_instance
ON task_model_pool_entries(provider_instance_key)
WHERE enabled = 1;
CREATE UNIQUE INDEX IF NOT EXISTS task_model_pool_entries_unique_selection
ON task_model_pool_entries(
  complexity,
  provider_account_id,
  model_profile,
  COALESCE(reasoning_effort, '')
);

-- Personal Work projection, immutable execution history, and global ledger.
INSERT INTO humans (human_id, display_name)
VALUES ('human:local', 'You')
ON CONFLICT(human_id) DO NOTHING;

CREATE TABLE workspaces (
  workspace_id TEXT PRIMARY KEY NOT NULL CHECK (workspace_id GLOB 'workspace:*'),
  name TEXT NOT NULL CHECK (trim(name) <> ''),
  description TEXT NOT NULL DEFAULT '',
  is_personal INTEGER NOT NULL DEFAULT 0 CHECK (is_personal IN (0, 1)),
  archived_at TEXT,
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (NOT (is_personal = 1 AND archived_at IS NOT NULL))
);

CREATE UNIQUE INDEX workspaces_one_personal
ON workspaces(is_personal) WHERE is_personal = 1;

CREATE TABLE workspace_memberships (
  workspace_id TEXT NOT NULL,
  human_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('owner', 'member')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, human_id),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE RESTRICT,
  FOREIGN KEY (human_id) REFERENCES humans(human_id) ON DELETE RESTRICT
);

CREATE INDEX workspace_memberships_human
ON workspace_memberships(human_id, workspace_id);

CREATE TABLE projects (
  project_id TEXT PRIMARY KEY NOT NULL CHECK (project_id GLOB 'project:*'),
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL CHECK (trim(name) <> ''),
  description TEXT NOT NULL DEFAULT '',
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  archived_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE (workspace_id, project_id),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE RESTRICT
);

CREATE INDEX projects_workspace_active
ON projects(workspace_id, archived_at, updated_at DESC, project_id DESC);

CREATE TABLE workflow_definitions (
  workflow_id TEXT PRIMARY KEY NOT NULL CHECK (workflow_id GLOB 'workflow:*'),
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL CHECK (trim(name) <> ''),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE (workspace_id, workflow_id),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX workflow_definitions_one_default_per_workspace
ON workflow_definitions(workspace_id) WHERE is_default = 1;

CREATE TABLE workflow_stages (
  stage_id TEXT PRIMARY KEY NOT NULL CHECK (stage_id GLOB 'stage:*'),
  workflow_id TEXT NOT NULL,
  stable_key TEXT NOT NULL CHECK (trim(stable_key) <> ''),
  display_name TEXT NOT NULL CHECK (trim(display_name) <> ''),
  ordinal INTEGER NOT NULL CHECK (ordinal >= 1),
  system_behavior TEXT NOT NULL CHECK (system_behavior IN (
    'intake', 'dispatch', 'active', 'human_gate', 'terminal_success',
    'terminal_cancelled'
  )),
  board_visible INTEGER NOT NULL CHECK (board_visible IN (0, 1)),
  UNIQUE (workflow_id, stage_id),
  UNIQUE (workflow_id, stable_key),
  UNIQUE (workflow_id, ordinal),
  UNIQUE (workflow_id, system_behavior),
  CHECK (
    (system_behavior = 'terminal_cancelled' AND board_visible = 0)
    OR
    (system_behavior != 'terminal_cancelled' AND board_visible = 1)
  ),
  FOREIGN KEY (workflow_id) REFERENCES workflow_definitions(workflow_id) ON DELETE RESTRICT
);

CREATE INDEX workflow_stages_board
ON workflow_stages(workflow_id, board_visible, ordinal, stage_id);

CREATE TABLE tasks (
  task_id TEXT PRIMARY KEY NOT NULL CHECK (task_id GLOB 'task:*'),
  workspace_id TEXT NOT NULL,
  project_id TEXT,
  workflow_id TEXT NOT NULL,
  stage_id TEXT NOT NULL,
  title TEXT NOT NULL CHECK (trim(title) <> ''),
  description_markdown TEXT NOT NULL DEFAULT '',
  authorization_context_json TEXT NOT NULL DEFAULT '{"kind":"none"}' CHECK (json_valid(authorization_context_json)),
  source_kind TEXT NOT NULL CHECK (source_kind IN (
    'chat_capture', 'chat_delegate', 'work_ui', 'system'
  )),
  source_conversation_id TEXT,
  source_turn_id TEXT,
  source_item_id TEXT,
  source_tool_call_id TEXT,
  created_by_actor_id TEXT NOT NULL CHECK (trim(created_by_actor_id) <> ''),
  generation INTEGER NOT NULL DEFAULT 1 CHECK (generation >= 1),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  current_contract_id TEXT,
  active_gate_id TEXT,
  latest_run_id TEXT,
  latest_submission_id TEXT,
  latest_review_id TEXT,
  completed_submission_id TEXT,
  queued_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT,
  cancelled_at TEXT,
  UNIQUE (workspace_id, task_id),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE RESTRICT,
  FOREIGN KEY (workspace_id, project_id)
    REFERENCES projects(workspace_id, project_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (workspace_id, workflow_id)
    REFERENCES workflow_definitions(workspace_id, workflow_id) ON DELETE RESTRICT,
  FOREIGN KEY (workflow_id, stage_id)
    REFERENCES workflow_stages(workflow_id, stage_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX tasks_source_tool_call
ON tasks(source_conversation_id, source_tool_call_id)
WHERE source_conversation_id IS NOT NULL AND source_tool_call_id IS NOT NULL;

CREATE INDEX tasks_board
ON tasks(workspace_id, stage_id, project_id, updated_at DESC, task_id DESC);

CREATE INDEX tasks_project_active
ON tasks(workspace_id, project_id, stage_id, updated_at DESC, task_id DESC)
WHERE completed_at IS NULL AND cancelled_at IS NULL;

CREATE INDEX tasks_terminal_history
ON tasks(
  workspace_id,
  COALESCE(completed_at, cancelled_at) DESC,
  task_id DESC
)
WHERE completed_at IS NOT NULL OR cancelled_at IS NOT NULL;

CREATE INDEX tasks_source_conversation
ON tasks(source_conversation_id, created_at DESC, task_id)
WHERE source_conversation_id IS NOT NULL;

CREATE TABLE task_execution_policy (
  policy_id TEXT PRIMARY KEY NOT NULL CHECK (policy_id = 'default'),
  max_provider_continuations INTEGER NOT NULL CHECK (max_provider_continuations BETWEEN 1 AND 1000),
  max_tool_calls INTEGER NOT NULL CHECK (max_tool_calls BETWEEN 1 AND 10000),
  max_active_minutes INTEGER NOT NULL CHECK (max_active_minutes BETWEEN 1 AND 10080),
  progress_audit_interval INTEGER NOT NULL CHECK (
    progress_audit_interval >= 1 AND progress_audit_interval <= max_provider_continuations
  ),
  max_automatic_retries INTEGER NOT NULL DEFAULT 3 CHECK (max_automatic_retries BETWEEN 0 AND 20),
  max_review_rounds INTEGER NOT NULL DEFAULT 3 CHECK (max_review_rounds BETWEEN 1 AND 20),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE task_execution_contracts (
  contract_id TEXT PRIMARY KEY NOT NULL CHECK (contract_id GLOB 'contract:*'),
  task_id TEXT NOT NULL,
  version INTEGER NOT NULL CHECK (version >= 1),
  task_generation INTEGER NOT NULL CHECK (task_generation >= 1),
  supersedes_contract_id TEXT,
  origin TEXT NOT NULL CHECK (origin IN ('delegated', 'planned', 'human_revision')),
  request_markdown TEXT NOT NULL CHECK (trim(request_markdown) <> ''),
  execution_plan_markdown TEXT CHECK (
    execution_plan_markdown IS NULL OR trim(execution_plan_markdown) <> ''
  ),
  complexity TEXT NOT NULL CHECK (complexity IN ('simple', 'medium', 'difficult')),
  executor_provider_kind TEXT NOT NULL CHECK (executor_provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  executor_provider_account_id TEXT NOT NULL CHECK (trim(executor_provider_account_id) <> ''),
  executor_provider_instance_key TEXT NOT NULL CHECK (trim(executor_provider_instance_key) <> ''),
  executor_selection_mode TEXT NOT NULL CHECK (executor_selection_mode IN ('explicit_profile', 'provider_default')),
  executor_model_profile TEXT,
  executor_reasoning_effort TEXT CHECK (executor_reasoning_effort IS NULL OR executor_reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  executor_selection_source TEXT,
  reviewer_provider_kind TEXT NOT NULL CHECK (reviewer_provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  reviewer_provider_account_id TEXT NOT NULL CHECK (trim(reviewer_provider_account_id) <> ''),
  reviewer_provider_instance_key TEXT NOT NULL CHECK (trim(reviewer_provider_instance_key) <> ''),
  reviewer_selection_mode TEXT NOT NULL CHECK (reviewer_selection_mode IN ('explicit_profile', 'provider_default')),
  reviewer_model_profile TEXT,
  reviewer_reasoning_effort TEXT CHECK (reviewer_reasoning_effort IS NULL OR reviewer_reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  reviewer_selection_source TEXT,
  max_provider_continuations INTEGER NOT NULL CHECK (max_provider_continuations BETWEEN 1 AND 1000),
  max_tool_calls INTEGER NOT NULL CHECK (max_tool_calls BETWEEN 1 AND 10000),
  max_active_minutes INTEGER NOT NULL CHECK (max_active_minutes BETWEEN 1 AND 10080),
  progress_audit_interval INTEGER NOT NULL CHECK (
    progress_audit_interval >= 1 AND progress_audit_interval <= max_provider_continuations
  ),
  max_automatic_retries INTEGER NOT NULL CHECK (max_automatic_retries BETWEEN 0 AND 20),
  max_review_rounds INTEGER NOT NULL CHECK (max_review_rounds BETWEEN 1 AND 20),
  workspace_id_snapshot TEXT NOT NULL CHECK (workspace_id_snapshot GLOB 'workspace:*'),
  workspace_name_snapshot TEXT NOT NULL CHECK (trim(workspace_name_snapshot) <> ''),
  workspace_description_snapshot TEXT NOT NULL,
  project_id_snapshot TEXT,
  project_name_snapshot TEXT,
  project_description_snapshot TEXT,
  created_by_actor_id TEXT NOT NULL CHECK (trim(created_by_actor_id) <> ''),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE (task_id, version),
  CHECK (origin <> 'planned' OR execution_plan_markdown IS NOT NULL),
  CHECK (
    (project_id_snapshot IS NULL AND project_name_snapshot IS NULL AND project_description_snapshot IS NULL)
    OR
    (project_id_snapshot IS NOT NULL AND project_id_snapshot GLOB 'project:*'
      AND project_name_snapshot IS NOT NULL AND trim(project_name_snapshot) <> ''
      AND project_description_snapshot IS NOT NULL)
  ),
  CHECK (executor_selection_mode = 'provider_default' OR (executor_model_profile IS NOT NULL AND trim(executor_model_profile) <> '')),
  CHECK (reviewer_selection_mode = 'provider_default' OR (reviewer_model_profile IS NOT NULL AND trim(reviewer_model_profile) <> '')),
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (supersedes_contract_id) REFERENCES task_execution_contracts(contract_id) ON DELETE RESTRICT
);

CREATE INDEX task_execution_contracts_task
ON task_execution_contracts(task_id, version DESC, contract_id);

CREATE INDEX task_execution_contracts_executor_instance
ON task_execution_contracts(executor_provider_instance_key);

CREATE INDEX task_execution_contracts_reviewer_instance
ON task_execution_contracts(reviewer_provider_instance_key);

CREATE TABLE task_contract_criteria (
  criterion_id TEXT PRIMARY KEY NOT NULL CHECK (criterion_id GLOB 'criterion:*'),
  contract_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL CHECK (ordinal >= 1),
  description TEXT NOT NULL CHECK (trim(description) <> ''),
  expected_evidence TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE (contract_id, ordinal),
  UNIQUE (contract_id, description),
  FOREIGN KEY (contract_id) REFERENCES task_execution_contracts(contract_id) ON DELETE RESTRICT
);

CREATE INDEX task_contract_criteria_contract
ON task_contract_criteria(contract_id, ordinal, criterion_id);

CREATE TABLE task_gates (
  gate_id TEXT PRIMARY KEY NOT NULL CHECK (gate_id GLOB 'gate:*'),
  task_id TEXT NOT NULL,
  task_generation INTEGER NOT NULL CHECK (task_generation >= 1),
  contract_id TEXT,
  gate_kind TEXT NOT NULL CHECK (gate_kind IN ('clarification', 'approval', 'recovery')),
  gate_state TEXT NOT NULL CHECK (gate_state IN ('open', 'resolved', 'superseded')),
  recovery_reason TEXT CHECK (recovery_reason IS NULL OR recovery_reason IN (
    'infrastructure_retries_exhausted', 'review_rounds_exhausted',
    'unsafe_effect_uncertain', 'configuration_unavailable', 'invariant_fault'
  )),
  retry_run_kind TEXT CHECK (retry_run_kind IS NULL OR retry_run_kind IN (
    'planner', 'executor', 'reviewer'
  )),
  prompt_markdown TEXT NOT NULL CHECK (trim(prompt_markdown) <> ''),
  context_markdown TEXT NOT NULL DEFAULT '',
  opened_by_actor_id TEXT NOT NULL CHECK (trim(opened_by_actor_id) <> ''),
  originating_run_id TEXT,
  resolved_by_actor_id TEXT,
  resolution_message_id TEXT,
  opened_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  resolved_at TEXT,
  CHECK (
    (gate_kind = 'recovery' AND recovery_reason IS NOT NULL)
    OR
    (gate_kind <> 'recovery' AND recovery_reason IS NULL AND retry_run_kind IS NULL)
  ),
  CHECK (
    gate_kind <> 'recovery'
    OR (recovery_reason = 'invariant_fault' AND retry_run_kind IS NULL)
    OR (recovery_reason <> 'invariant_fault' AND retry_run_kind IS NOT NULL)
  ),
  CHECK (
    (gate_state = 'open' AND resolved_by_actor_id IS NULL
      AND resolution_message_id IS NULL AND resolved_at IS NULL)
    OR
    (gate_state = 'resolved' AND resolved_by_actor_id IS NOT NULL
      AND resolution_message_id IS NOT NULL AND resolved_at IS NOT NULL)
    OR
    (gate_state = 'superseded' AND resolved_by_actor_id IS NOT NULL
      AND resolution_message_id IS NULL AND resolved_at IS NOT NULL)
  ),
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (contract_id) REFERENCES task_execution_contracts(contract_id) ON DELETE RESTRICT,
  FOREIGN KEY (originating_run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT,
  FOREIGN KEY (resolution_message_id) REFERENCES task_messages(message_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX task_gates_one_open_per_task
ON task_gates(task_id) WHERE gate_state = 'open';

CREATE INDEX task_gates_attention
ON task_gates(gate_state, gate_kind, opened_at, task_id)
WHERE gate_state = 'open';

CREATE TABLE task_messages (
  message_id TEXT PRIMARY KEY NOT NULL CHECK (message_id GLOB 'task_message:*'),
  task_id TEXT NOT NULL,
  task_generation INTEGER NOT NULL CHECK (task_generation >= 1),
  contract_id TEXT,
  gate_id TEXT,
  review_id TEXT,
  message_kind TEXT NOT NULL CHECK (message_kind IN ('human_answer', 'human_change_request', 'retry_note')),
  body_markdown TEXT NOT NULL CHECK (trim(body_markdown) <> ''),
  approval_decision TEXT CHECK (approval_decision IS NULL OR approval_decision IN ('approved', 'declined')),
  author_actor_id TEXT NOT NULL CHECK (trim(author_actor_id) <> ''),
  consumed_by_run_id TEXT,
  consumed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (contract_id) REFERENCES task_execution_contracts(contract_id) ON DELETE RESTRICT,
  FOREIGN KEY (gate_id) REFERENCES task_gates(gate_id) ON DELETE RESTRICT,
  FOREIGN KEY (review_id) REFERENCES task_reviews(review_id) ON DELETE RESTRICT,
  FOREIGN KEY (consumed_by_run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT,
  CHECK ((consumed_by_run_id IS NULL AND consumed_at IS NULL) OR (consumed_by_run_id IS NOT NULL AND consumed_at IS NOT NULL))
);

CREATE INDEX task_messages_pending
ON task_messages(task_id, task_generation, created_at, message_id)
WHERE consumed_at IS NULL;

CREATE INDEX task_messages_history
ON task_messages(task_id, created_at, message_id);

CREATE TABLE agent_runs (
  run_id TEXT PRIMARY KEY NOT NULL CHECK (run_id GLOB 'run:*'),
  instance_name TEXT NOT NULL CHECK (trim(instance_name) <> ''),
  task_id TEXT NOT NULL,
  task_generation INTEGER NOT NULL CHECK (task_generation >= 1),
  contract_id TEXT,
  run_kind TEXT NOT NULL CHECK (run_kind IN ('planner', 'executor', 'reviewer')),
  agent_id TEXT NOT NULL CHECK (trim(agent_id) <> ''),
  attempt_index INTEGER NOT NULL DEFAULT 0 CHECK (attempt_index >= 0),
  review_round INTEGER NOT NULL DEFAULT 0 CHECK (review_round >= 0),
  parent_run_id TEXT,
  triggering_submission_id TEXT,
  triggering_review_id TEXT,
  actual_provider_kind TEXT,
  actual_model_profile TEXT,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
  provider_account_id TEXT NOT NULL CHECK (trim(provider_account_id) <> ''),
  provider_instance_key TEXT NOT NULL CHECK (trim(provider_instance_key) <> ''),
  selection_mode TEXT NOT NULL CHECK (selection_mode IN ('explicit_profile', 'provider_default')),
  model_profile TEXT,
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  selection_source TEXT,
  max_provider_continuations INTEGER NOT NULL CHECK (max_provider_continuations BETWEEN 1 AND 1000),
  max_tool_calls INTEGER NOT NULL CHECK (max_tool_calls BETWEEN 1 AND 10000),
  max_active_minutes INTEGER NOT NULL CHECK (max_active_minutes BETWEEN 1 AND 10080),
  progress_audit_interval INTEGER NOT NULL CHECK (
    progress_audit_interval >= 1 AND progress_audit_interval <= max_provider_continuations
  ),
  max_automatic_retries INTEGER NOT NULL CHECK (max_automatic_retries BETWEEN 0 AND 20),
  max_review_rounds INTEGER NOT NULL CHECK (max_review_rounds BETWEEN 1 AND 20),
  status TEXT NOT NULL CHECK (status IN (
    'queued', 'leased', 'running', 'completed', 'waiting_for_approval',
    'interrupted', 'failed', 'cancelled'
  )),
  queued_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  lease_owner TEXT,
  lease_token TEXT,
  lease_expires_at TEXT,
  heartbeat_at TEXT,
  started_at TEXT,
  ended_at TEXT,
  cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK (cancellation_requested IN (0, 1)),
  error_code TEXT,
  error_message TEXT,
  provider_call_count INTEGER NOT NULL DEFAULT 0 CHECK (provider_call_count >= 0),
  tool_call_count INTEGER NOT NULL DEFAULT 0 CHECK (tool_call_count >= 0),
  input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
  cached_input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (cached_input_tokens >= 0),
  output_tokens INTEGER NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
  active_milliseconds INTEGER NOT NULL DEFAULT 0 CHECK (active_milliseconds >= 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (
    (run_kind = 'planner' AND contract_id IS NULL)
    OR
    (run_kind IN ('executor', 'reviewer') AND contract_id IS NOT NULL)
  ),
  CHECK (selection_mode = 'provider_default' OR (model_profile IS NOT NULL AND trim(model_profile) <> '')),
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (contract_id) REFERENCES task_execution_contracts(contract_id) ON DELETE RESTRICT,
  FOREIGN KEY (parent_run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX agent_runs_one_runnable_per_task
ON agent_runs(task_id)
WHERE status IN ('queued', 'leased', 'running');

CREATE INDEX agent_runs_fifo_claim
ON agent_runs(status, queued_at, run_id)
WHERE status = 'queued';

CREATE INDEX agent_runs_task_history
ON agent_runs(task_id, created_at, run_id);

CREATE UNIQUE INDEX agent_runs_instance_name
ON agent_runs(instance_name);

CREATE INDEX agent_runs_expired_leases
ON agent_runs(status, lease_expires_at, run_id)
WHERE status IN ('leased', 'running');

CREATE INDEX agent_runs_contract
ON agent_runs(contract_id, created_at, run_id)
WHERE contract_id IS NOT NULL;

CREATE TABLE agent_run_items (
  item_id TEXT PRIMARY KEY NOT NULL CHECK (item_id GLOB 'run_item:*'),
  run_id TEXT NOT NULL,
  sequence_index INTEGER NOT NULL CHECK (
    (kind = 'context_checkpoint' AND sequence_index = 0)
    OR (kind <> 'context_checkpoint' AND sequence_index >= 1)
  ),
  round_index INTEGER NOT NULL DEFAULT 0 CHECK (round_index >= 0),
  kind TEXT NOT NULL CHECK (kind IN (
    'model_input', 'assistant_output', 'tool_call', 'tool_result', 'progress_notice',
    'context_checkpoint', 'task_submission', 'task_review', 'artifact_reference',
    'failure', 'cancellation'
  )),
  status TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled', 'skipped')),
  correlation_id TEXT,
  parent_item_id TEXT,
  content_text TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE (run_id, sequence_index),
  FOREIGN KEY (run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT
);

CREATE INDEX agent_run_items_run_sequence
ON agent_run_items(run_id, sequence_index, item_id);

CREATE TABLE runtime_debug_spans (
  span_id TEXT PRIMARY KEY NOT NULL CHECK (span_id GLOB 'debug_span:*'),
  conversation_turn_id TEXT,
  agent_run_id TEXT,
  category TEXT NOT NULL CHECK (category IN ('provider', 'tool', 'runtime', 'persistence')),
  name TEXT NOT NULL CHECK (trim(name) <> ''),
  status TEXT NOT NULL DEFAULT 'running' CHECK (status IN (
    'running', 'completed', 'failed', 'cancelled', 'interrupted'
  )),
  duration_milliseconds INTEGER CHECK (duration_milliseconds IS NULL OR duration_milliseconds >= 0),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
  started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  ended_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK ((conversation_turn_id IS NOT NULL) <> (agent_run_id IS NOT NULL)),
  FOREIGN KEY (conversation_turn_id) REFERENCES conversation_turns(turn_id) ON DELETE CASCADE,
  FOREIGN KEY (agent_run_id) REFERENCES agent_runs(run_id) ON DELETE CASCADE
);

CREATE INDEX runtime_debug_spans_turn
ON runtime_debug_spans(conversation_turn_id, started_at, span_id)
WHERE conversation_turn_id IS NOT NULL;

CREATE INDEX runtime_debug_spans_run
ON runtime_debug_spans(agent_run_id, started_at, span_id)
WHERE agent_run_id IS NOT NULL;

CREATE TABLE task_submissions (
  submission_id TEXT PRIMARY KEY NOT NULL CHECK (submission_id GLOB 'submission:*'),
  task_id TEXT NOT NULL,
  contract_id TEXT NOT NULL,
  executor_run_id TEXT NOT NULL,
  review_round INTEGER NOT NULL CHECK (review_round >= 1),
  summary TEXT NOT NULL CHECK (trim(summary) <> ''),
  result_markdown TEXT NOT NULL CHECK (trim(result_markdown) <> ''),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE (executor_run_id),
  UNIQUE (task_id, contract_id, review_round),
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (contract_id) REFERENCES task_execution_contracts(contract_id) ON DELETE RESTRICT,
  FOREIGN KEY (executor_run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT
);

CREATE TABLE task_submission_criteria (
  submission_id TEXT NOT NULL,
  criterion_id TEXT NOT NULL,
  evidence_markdown TEXT NOT NULL CHECK (trim(evidence_markdown) <> ''),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (submission_id, criterion_id),
  FOREIGN KEY (submission_id) REFERENCES task_submissions(submission_id) ON DELETE RESTRICT,
  FOREIGN KEY (criterion_id) REFERENCES task_contract_criteria(criterion_id) ON DELETE RESTRICT
);

CREATE TABLE task_submission_artifacts (
  submission_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL CHECK (ordinal >= 1),
  artifact_id TEXT NOT NULL,
  artifact_version_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (submission_id, ordinal),
  UNIQUE (submission_id, artifact_id, artifact_version_id),
  FOREIGN KEY (submission_id) REFERENCES task_submissions(submission_id) ON DELETE RESTRICT,
  FOREIGN KEY (artifact_id) REFERENCES artifacts(artifact_id) ON DELETE RESTRICT,
  FOREIGN KEY (artifact_version_id) REFERENCES artifact_versions(artifact_version_id) ON DELETE RESTRICT
);

CREATE INDEX task_submissions_task_history
ON task_submissions(task_id, created_at, submission_id);

CREATE TABLE task_reviews (
  review_id TEXT PRIMARY KEY NOT NULL CHECK (review_id GLOB 'review:*'),
  task_id TEXT NOT NULL,
  contract_id TEXT NOT NULL,
  reviewer_run_id TEXT NOT NULL,
  reviewed_submission_id TEXT NOT NULL,
  review_attempt_index INTEGER NOT NULL CHECK (review_attempt_index >= 1),
  supersedes_review_id TEXT,
  overall_verdict TEXT NOT NULL CHECK (overall_verdict IN ('approve', 'request_changes', 'needs_human')),
  human_gate_kind TEXT CHECK (human_gate_kind IS NULL OR human_gate_kind IN ('clarification', 'approval')),
  overall_feedback TEXT NOT NULL CHECK (trim(overall_feedback) <> ''),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE (reviewer_run_id),
  UNIQUE (reviewed_submission_id, review_attempt_index),
  CHECK (
    (overall_verdict = 'needs_human' AND human_gate_kind IS NOT NULL)
    OR
    (overall_verdict <> 'needs_human' AND human_gate_kind IS NULL)
  ),
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (contract_id) REFERENCES task_execution_contracts(contract_id) ON DELETE RESTRICT,
  FOREIGN KEY (reviewer_run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT,
  FOREIGN KEY (reviewed_submission_id) REFERENCES task_submissions(submission_id) ON DELETE RESTRICT,
  FOREIGN KEY (supersedes_review_id) REFERENCES task_reviews(review_id) ON DELETE RESTRICT
);

CREATE TABLE task_review_criteria (
  review_id TEXT NOT NULL,
  criterion_id TEXT NOT NULL,
  outcome TEXT NOT NULL CHECK (outcome IN ('pass', 'fail', 'uncertain')),
  evidence_markdown TEXT,
  feedback TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (review_id, criterion_id),
  FOREIGN KEY (review_id) REFERENCES task_reviews(review_id) ON DELETE RESTRICT,
  FOREIGN KEY (criterion_id) REFERENCES task_contract_criteria(criterion_id) ON DELETE RESTRICT
);

CREATE INDEX task_reviews_task_history
ON task_reviews(task_id, created_at, review_id);

CREATE TABLE work_events (
  event_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE CHECK (event_id GLOB 'event:*'),
  event_kind TEXT NOT NULL CHECK (trim(event_kind) <> ''),
  workspace_id TEXT NOT NULL,
  project_id TEXT,
  task_id TEXT,
  run_id TEXT,
  actor_id TEXT NOT NULL CHECK (trim(actor_id) <> ''),
  causation_id TEXT,
  correlation_id TEXT NOT NULL CHECK (trim(correlation_id) <> ''),
  payload_json TEXT NOT NULL DEFAULT '{"v":1}' CHECK (json_valid(payload_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE RESTRICT,
  FOREIGN KEY (workspace_id, project_id)
    REFERENCES projects(workspace_id, project_id) ON DELETE RESTRICT,
  FOREIGN KEY (workspace_id, task_id)
    REFERENCES tasks(workspace_id, task_id) ON DELETE RESTRICT,
  FOREIGN KEY (run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT
);

CREATE INDEX work_events_workspace_cursor
ON work_events(workspace_id, event_sequence);

CREATE INDEX work_events_task_cursor
ON work_events(task_id, event_sequence)
WHERE task_id IS NOT NULL;

CREATE INDEX work_events_project_cursor
ON work_events(project_id, event_sequence)
WHERE project_id IS NOT NULL;

CREATE INDEX work_events_run_cursor
ON work_events(run_id, event_sequence)
WHERE run_id IS NOT NULL;

CREATE TABLE work_notification_outbox (
  notification_id TEXT PRIMARY KEY NOT NULL CHECK (notification_id GLOB 'notification:*'),
  event_sequence INTEGER NOT NULL,
  destination_kind TEXT NOT NULL CHECK (destination_kind = 'human_primary_conversation'),
  destination_id TEXT NOT NULL CHECK (trim(destination_id) <> ''),
  notification_kind TEXT NOT NULL CHECK (notification_kind IN (
    'task_created', 'task_waiting', 'task_recovery', 'task_completed'
  )),
  payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'leased', 'delivered', 'failed')),
  available_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  lease_owner TEXT,
  lease_token TEXT,
  lease_expires_at TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 4294967295),
  last_error_code TEXT,
  last_error_message TEXT,
  delivered_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE (event_sequence, destination_kind, destination_id, notification_kind),
  FOREIGN KEY (event_sequence) REFERENCES work_events(event_sequence) ON DELETE RESTRICT,
  FOREIGN KEY (destination_id) REFERENCES humans(human_id) ON DELETE RESTRICT
);

CREATE INDEX work_notification_outbox_claim
ON work_notification_outbox(status, available_at, notification_id)
WHERE status IN ('pending', 'failed');

CREATE INDEX work_notification_outbox_expired
ON work_notification_outbox(status, lease_expires_at, notification_id)
WHERE status = 'leased';

CREATE TABLE work_command_receipts (
  actor_id TEXT NOT NULL CHECK (trim(actor_id) <> ''),
  command_name TEXT NOT NULL CHECK (trim(command_name) <> ''),
  idempotency_key TEXT NOT NULL CHECK (trim(idempotency_key) <> ''),
  request_fingerprint TEXT NOT NULL CHECK (length(request_fingerprint) = 64 AND request_fingerprint = lower(request_fingerprint)),
  result_task_id TEXT,
  result_project_id TEXT,
  result_event_sequence INTEGER NOT NULL,
  response_json TEXT NOT NULL CHECK (json_valid(response_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (actor_id, command_name, idempotency_key),
  FOREIGN KEY (result_task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (result_project_id) REFERENCES projects(project_id) ON DELETE RESTRICT,
  FOREIGN KEY (result_event_sequence) REFERENCES work_events(event_sequence) ON DELETE RESTRICT
);

CREATE TABLE governed_actions (
  action_id TEXT NOT NULL CHECK (action_id GLOB 'action:*'),
  revision INTEGER NOT NULL CHECK (revision >= 1),
  owner_human_id TEXT NOT NULL,
  conversation_id TEXT,
  turn_id TEXT,
  task_id TEXT,
  run_id TEXT,
  requesting_agent_id TEXT NOT NULL CHECK (trim(requesting_agent_id) <> ''),
  capability_name TEXT NOT NULL CHECK (trim(capability_name) <> ''),
  operation_token TEXT NOT NULL CHECK (trim(operation_token) <> ''),
  effect TEXT NOT NULL CHECK (effect IN ('write', 'export', 'write_export')),
  arguments_json TEXT NOT NULL CHECK (json_valid(arguments_json)),
  arguments_sha256 TEXT NOT NULL CHECK (length(arguments_sha256) = 64 AND arguments_sha256 = lower(arguments_sha256)),
  input_schema_json TEXT NOT NULL CHECK (json_valid(input_schema_json)),
  authorization_context_json TEXT NOT NULL CHECK (json_valid(authorization_context_json)),
  safe_summary TEXT NOT NULL CHECK (trim(safe_summary) <> ''),
  state TEXT NOT NULL CHECK (state IN (
    'proposed', 'awaiting_approval', 'executable', 'executing', 'succeeded',
    'failed', 'outcome_uncertain', 'declined', 'superseded', 'cancelled'
  )),
  output_json TEXT CHECK (output_json IS NULL OR json_valid(output_json)),
  failure_code TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT,
  PRIMARY KEY (action_id, revision),
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT,
  CHECK ((task_id IS NULL AND run_id IS NULL) OR (task_id IS NOT NULL AND run_id IS NOT NULL))
);

CREATE INDEX governed_actions_attention
ON governed_actions(owner_human_id, state, created_at, action_id)
WHERE state = 'awaiting_approval';

CREATE INDEX governed_actions_conversation
ON governed_actions(conversation_id, created_at, action_id)
WHERE conversation_id IS NOT NULL;

CREATE INDEX governed_actions_task
ON governed_actions(task_id, created_at, action_id)
WHERE task_id IS NOT NULL;

CREATE TABLE governed_action_assessments (
  action_id TEXT NOT NULL,
  action_revision INTEGER NOT NULL CHECK (action_revision >= 1),
  status TEXT NOT NULL CHECK (status IN ('completed', 'reviewer_unavailable', 'invalid_response')),
  reviewer_selection_json TEXT CHECK (reviewer_selection_json IS NULL OR json_valid(reviewer_selection_json)),
  authorization TEXT CHECK (authorization IS NULL OR authorization IN ('explicit', 'substantive', 'weak', 'absent')),
  risk TEXT CHECK (risk IS NULL OR risk IN ('low', 'medium', 'high', 'critical')),
  recommendation TEXT NOT NULL CHECK (recommendation IN ('auto_execute', 'require_approval')),
  reason_codes_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(reason_codes_json)),
  explanation TEXT NOT NULL CHECK (trim(explanation) <> ''),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (action_id, action_revision),
  FOREIGN KEY (action_id, action_revision)
    REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,
  CHECK (status = 'completed' OR recommendation = 'require_approval'),
  CHECK ((status = 'completed' AND reviewer_selection_json IS NOT NULL AND authorization IS NOT NULL AND risk IS NOT NULL)
    OR (status <> 'completed' AND authorization IS NULL AND risk IS NULL))
);

CREATE TABLE governed_action_approvals (
  action_id TEXT NOT NULL,
  action_revision INTEGER NOT NULL CHECK (action_revision >= 1),
  state TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'declined', 'consumed', 'revoked', 'superseded')),
  decided_by_human_id TEXT,
  decided_at TEXT,
  consumed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (action_id, action_revision),
  FOREIGN KEY (action_id, action_revision)
    REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,
  FOREIGN KEY (decided_by_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  CHECK ((state = 'pending' AND decided_by_human_id IS NULL AND decided_at IS NULL AND consumed_at IS NULL)
    OR (state IN ('approved', 'declined', 'revoked') AND decided_by_human_id IS NOT NULL AND decided_at IS NOT NULL AND consumed_at IS NULL)
    OR (state = 'consumed' AND decided_by_human_id IS NOT NULL AND decided_at IS NOT NULL AND consumed_at IS NOT NULL)
    OR (state = 'superseded' AND consumed_at IS NULL))
);

CREATE TABLE governed_action_events (
  event_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE CHECK (event_id GLOB 'action_event:*'),
  action_id TEXT NOT NULL,
  action_revision INTEGER NOT NULL CHECK (action_revision >= 1),
  event_kind TEXT NOT NULL CHECK (event_kind IN (
    'proposed', 'reviewed', 'approval_requested', 'approved', 'declined',
    'execution_started', 'succeeded', 'failed', 'outcome_uncertain', 'superseded', 'cancelled'
  )),
  actor_id TEXT NOT NULL CHECK (trim(actor_id) <> ''),
  safe_payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(safe_payload_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (action_id, action_revision)
    REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT
);

CREATE INDEX governed_action_events_action
ON governed_action_events(action_id, action_revision, event_sequence);

CREATE TABLE observed_urls (
  normalized_url TEXT PRIMARY KEY NOT NULL CHECK (trim(normalized_url) <> ''),
  source_kind TEXT NOT NULL CHECK (source_kind IN ('search_result', 'fetched_link')),
  source_event_reference TEXT NOT NULL CHECK (trim(source_event_reference) <> ''),
  first_observed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  last_observed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO workspaces (workspace_id, name, description, is_personal)
VALUES ('workspace:personal', 'Personal', '', 1)
ON CONFLICT (workspace_id) DO NOTHING;

INSERT INTO workspace_memberships (workspace_id, human_id, role)
VALUES ('workspace:personal', 'human:local', 'owner')
ON CONFLICT (workspace_id, human_id) DO NOTHING;

INSERT INTO workflow_definitions (workflow_id, workspace_id, name, is_default)
VALUES ('workflow:personal:default', 'workspace:personal', 'Personal workflow', 1)
ON CONFLICT (workflow_id) DO NOTHING;

INSERT INTO workflow_stages (
  stage_id, workflow_id, stable_key, display_name, ordinal, system_behavior, board_visible
) VALUES
  ('stage:personal:inbox',     'workflow:personal:default', 'inbox',     'Inbox',     10, 'intake',             1),
  ('stage:personal:queue',     'workflow:personal:default', 'queue',     'Queue',     20, 'dispatch',           1),
  ('stage:personal:doing',     'workflow:personal:default', 'doing',     'Doing',     30, 'active',             1),
  ('stage:personal:waiting',   'workflow:personal:default', 'waiting',   'Waiting',   40, 'human_gate',         1),
  ('stage:personal:done',      'workflow:personal:default', 'done',      'Done',      50, 'terminal_success',   1),
  ('stage:personal:cancelled', 'workflow:personal:default', 'cancelled', 'Cancelled', 60, 'terminal_cancelled', 0)
ON CONFLICT (stage_id) DO NOTHING;

INSERT INTO task_execution_policy (
  policy_id, max_provider_continuations, max_tool_calls, max_active_minutes,
  progress_audit_interval, max_automatic_retries, max_review_rounds
) VALUES ('default', 80, 400, 120, 20, 3, 3)
ON CONFLICT (policy_id) DO NOTHING;

INSERT INTO schema_state (name, version, applied_at)
VALUES ('sqlite_store_v9', 9, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT (name) DO NOTHING;
"#;

/// The first migration captures the last pre-migration schema exactly. The
/// second replaces its custom marker table with SQLite's `user_version`, which
/// `rusqlite_migration` owns for every later schema change.
pub(super) fn store_migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(LEGACY_V9_SCHEMA_SQL),
        M::up(LEGACY_ADOPTION_SQL),
        M::up(HUMAN_PASSKEYS_SQL),
        M::up(MCP_TOOL_POLICY_SQL),
        M::up(MCP_AUTH_REQUESTS_SQL),
        M::up(MCP_AUTH_REQUESTS_REPAIR_SQL),
        M::up(MCP_AUTH_RESULT_CONTEXT_SQL),
    ])
}

pub(super) const LEGACY_ADOPTION_SQL: &str = "DROP TABLE schema_state;";

const HUMAN_PASSKEYS_SQL: &str = r#"
CREATE TABLE human_passkeys (
  human_id TEXT PRIMARY KEY NOT NULL,
  credential_json TEXT NOT NULL CHECK (json_valid(credential_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (human_id) REFERENCES humans(human_id) ON DELETE CASCADE
);
"#;

const MCP_TOOL_POLICY_SQL: &str = r#"
DROP TABLE tool_calibrations;

ALTER TABLE mcp_servers ADD COLUMN data_sharing_policy TEXT
  CHECK (data_sharing_policy IS NULL OR data_sharing_policy IN ('allow_automatically', 'review_every_call'));
ALTER TABLE mcp_servers ADD COLUMN unsafe_action_policy TEXT
  CHECK (unsafe_action_policy IS NULL OR unsafe_action_policy IN ('always_ask', 'reviewer_may_approve', 'never_ask'));
ALTER TABLE mcp_servers ADD COLUMN policy_revision INTEGER NOT NULL DEFAULT 0 CHECK (policy_revision >= 0);

UPDATE mcp_servers SET enabled = 0;

CREATE TABLE mcp_tool_policies (
  mcp_tool_id TEXT PRIMARY KEY NOT NULL,
  read_only INTEGER CHECK (read_only IS NULL OR read_only IN (0, 1)),
  read_only_source TEXT CHECK (read_only_source IS NULL OR read_only_source IN ('annotation', 'model', 'safe_default', 'human')),
  idempotent INTEGER CHECK (idempotent IS NULL OR idempotent IN (0, 1)),
  idempotent_source TEXT CHECK (idempotent_source IS NULL OR idempotent_source IN ('annotation', 'model', 'safe_default', 'human')),
  destructive INTEGER CHECK (destructive IS NULL OR destructive IN (0, 1)),
  destructive_source TEXT CHECK (destructive_source IS NULL OR destructive_source IN ('annotation', 'model', 'safe_default', 'human')),
  open_world INTEGER CHECK (open_world IS NULL OR open_world IN (0, 1)),
  open_world_source TEXT CHECK (open_world_source IS NULL OR open_world_source IN ('annotation', 'model', 'safe_default', 'human')),
  status TEXT NOT NULL CHECK (status IN ('pending', 'ready', 'defaulted', 'disabled')),
  policy_revision INTEGER NOT NULL DEFAULT 1 CHECK (policy_revision > 0),
  metadata_fingerprint TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK ((read_only IS NULL) = (read_only_source IS NULL)),
  CHECK ((idempotent IS NULL) = (idempotent_source IS NULL)),
  CHECK ((destructive IS NULL) = (destructive_source IS NULL)),
  CHECK ((open_world IS NULL) = (open_world_source IS NULL)),
  FOREIGN KEY (mcp_tool_id) REFERENCES mcp_tools(mcp_tool_id) ON DELETE CASCADE
);

INSERT INTO mcp_tool_policies (
  mcp_tool_id, read_only, read_only_source, idempotent, idempotent_source,
  destructive, destructive_source, open_world, open_world_source,
  status, policy_revision, metadata_fingerprint
)
SELECT
  mcp_tool_id,
  CASE WHEN json_type(annotations_json, '$.readOnlyHint') IN ('true', 'false')
    THEN json_extract(annotations_json, '$.readOnlyHint') END,
  CASE WHEN json_type(annotations_json, '$.readOnlyHint') IN ('true', 'false')
    THEN 'annotation' END,
  CASE WHEN json_type(annotations_json, '$.idempotentHint') IN ('true', 'false')
    THEN json_extract(annotations_json, '$.idempotentHint') END,
  CASE WHEN json_type(annotations_json, '$.idempotentHint') IN ('true', 'false')
    THEN 'annotation' END,
  CASE WHEN json_type(annotations_json, '$.destructiveHint') IN ('true', 'false')
    THEN json_extract(annotations_json, '$.destructiveHint') END,
  CASE WHEN json_type(annotations_json, '$.destructiveHint') IN ('true', 'false')
    THEN 'annotation' END,
  CASE WHEN json_type(annotations_json, '$.openWorldHint') IN ('true', 'false')
    THEN json_extract(annotations_json, '$.openWorldHint') END,
  CASE WHEN json_type(annotations_json, '$.openWorldHint') IN ('true', 'false')
    THEN 'annotation' END,
  CASE WHEN json_type(annotations_json, '$.readOnlyHint') IN ('true', 'false')
    AND json_type(annotations_json, '$.idempotentHint') IN ('true', 'false')
    AND json_type(annotations_json, '$.destructiveHint') IN ('true', 'false')
    AND json_type(annotations_json, '$.openWorldHint') IN ('true', 'false')
    THEN 'ready' ELSE 'pending' END,
  1,
  metadata_fingerprint
FROM mcp_tools;
"#;

// Migration 5 is frozen to the shape that reached installed databases. Later
// MCP authentication schema changes belong in the repair migration below.
const MCP_AUTH_REQUESTS_SQL: &str = r#"
CREATE TABLE mcp_auth_requests (
  request_id TEXT PRIMARY KEY NOT NULL CHECK (request_id GLOB 'mcp_auth:*'),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
  owner_human_id TEXT NOT NULL,
  conversation_id TEXT,
  turn_id TEXT,
  task_id TEXT,
  run_id TEXT,
  requesting_agent_id TEXT NOT NULL CHECK (trim(requesting_agent_id) <> ''),
  mcp_server_id TEXT NOT NULL,
  capability_name TEXT NOT NULL CHECK (trim(capability_name) <> ''),
  operation_token TEXT NOT NULL CHECK (trim(operation_token) <> ''),
  input_schema_json TEXT NOT NULL CHECK (json_valid(input_schema_json)),
  arguments_json TEXT NOT NULL CHECK (json_valid(arguments_json)),
  arguments_sha256 TEXT NOT NULL CHECK (length(arguments_sha256) = 64 AND arguments_sha256 = lower(arguments_sha256)),
  output_index INTEGER NOT NULL CHECK (output_index >= 0),
  call_id TEXT,
  provider_call_id TEXT,
  provider_name TEXT,
  governed_action_id TEXT,
  governed_action_revision INTEGER,
  oauth_attempt_id TEXT,
  state TEXT NOT NULL CHECK (state IN (
    'awaiting_user', 'authorizing', 'resuming', 'completed', 'cancelled', 'superseded'
  )),
  output_json TEXT CHECK (output_json IS NULL OR json_valid(output_json)),
  failure_code TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT,
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT,
  FOREIGN KEY (mcp_server_id) REFERENCES mcp_servers(mcp_server_id) ON DELETE CASCADE,
  FOREIGN KEY (governed_action_id, governed_action_revision)
    REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,
  CHECK ((conversation_id IS NOT NULL AND turn_id IS NOT NULL AND task_id IS NULL AND run_id IS NULL)
      OR (conversation_id IS NULL AND turn_id IS NULL AND task_id IS NOT NULL AND run_id IS NOT NULL)),
  CHECK ((governed_action_id IS NULL) = (governed_action_revision IS NULL))
);

CREATE UNIQUE INDEX mcp_auth_requests_conversation_call
ON mcp_auth_requests(conversation_id, turn_id, output_index)
WHERE conversation_id IS NOT NULL;

CREATE UNIQUE INDEX mcp_auth_requests_run_call
ON mcp_auth_requests(run_id, output_index)
WHERE run_id IS NOT NULL;

CREATE INDEX mcp_auth_requests_attention
ON mcp_auth_requests(owner_human_id, state, created_at, request_id)
WHERE state IN ('awaiting_user', 'authorizing');

CREATE INDEX mcp_auth_requests_attempt
ON mcp_auth_requests(oauth_attempt_id, state)
WHERE oauth_attempt_id IS NOT NULL;
"#;

const MCP_AUTH_REQUESTS_REPAIR_SQL: &str = r#"
ALTER TABLE governed_actions
ADD COLUMN authentication_pending INTEGER NOT NULL DEFAULT 0
CHECK (authentication_pending IN (0, 1));

DROP INDEX mcp_auth_requests_conversation_call;
DROP INDEX mcp_auth_requests_run_call;
DROP INDEX mcp_auth_requests_attention;
DROP INDEX mcp_auth_requests_attempt;
ALTER TABLE mcp_auth_requests RENAME TO mcp_auth_requests_v5;

CREATE TABLE mcp_auth_requests (
  request_id TEXT PRIMARY KEY NOT NULL CHECK (request_id GLOB 'mcp_auth:*'),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
  owner_human_id TEXT NOT NULL,
  conversation_id TEXT,
  turn_id TEXT,
  task_id TEXT,
  run_id TEXT,
  task_generation INTEGER CHECK (task_generation IS NULL OR task_generation > 0),
  requesting_agent_id TEXT NOT NULL CHECK (trim(requesting_agent_id) <> ''),
  mcp_server_id TEXT NOT NULL,
  capability_name TEXT NOT NULL CHECK (trim(capability_name) <> ''),
  operation_token TEXT NOT NULL CHECK (trim(operation_token) <> ''),
  input_schema_json TEXT NOT NULL CHECK (json_valid(input_schema_json)),
  arguments_json TEXT NOT NULL CHECK (json_valid(arguments_json)),
  arguments_sha256 TEXT NOT NULL CHECK (length(arguments_sha256) = 64 AND arguments_sha256 = lower(arguments_sha256)),
  output_index INTEGER NOT NULL CHECK (output_index >= 0),
  call_id TEXT,
  provider_call_id TEXT,
  provider_name TEXT,
  governed_action_id TEXT,
  governed_action_revision INTEGER,
  oauth_attempt_id TEXT,
  state TEXT NOT NULL CHECK (state IN (
    'awaiting_user', 'authorizing', 'resuming', 'completed', 'cancelled', 'superseded'
  )),
  output_json TEXT CHECK (output_json IS NULL OR json_valid(output_json)),
  failure_code TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT,
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT,
  FOREIGN KEY (mcp_server_id) REFERENCES mcp_servers(mcp_server_id) ON DELETE CASCADE,
  FOREIGN KEY (governed_action_id, governed_action_revision)
    REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,
  CHECK ((conversation_id IS NOT NULL AND turn_id IS NOT NULL AND task_id IS NULL AND run_id IS NULL AND task_generation IS NULL)
      OR (conversation_id IS NULL AND turn_id IS NULL AND task_id IS NOT NULL AND run_id IS NOT NULL AND task_generation IS NOT NULL)),
  CHECK ((governed_action_id IS NULL) = (governed_action_revision IS NULL))
);

INSERT INTO mcp_auth_requests (
  request_id, revision, owner_human_id, conversation_id, turn_id, task_id, run_id,
  task_generation, requesting_agent_id, mcp_server_id, capability_name, operation_token,
  input_schema_json, arguments_json, arguments_sha256, output_index, call_id,
  provider_call_id, provider_name, governed_action_id, governed_action_revision,
  oauth_attempt_id, state, output_json, failure_code, created_at, updated_at, completed_at
)
SELECT
  request_id, revision, owner_human_id, conversation_id, turn_id, task_id, run_id,
  (SELECT task_generation FROM agent_runs WHERE run_id = mcp_auth_requests_v5.run_id),
  requesting_agent_id, mcp_server_id, capability_name, operation_token, input_schema_json,
  arguments_json, arguments_sha256, output_index, call_id, provider_call_id, provider_name,
  governed_action_id, governed_action_revision, oauth_attempt_id, state, output_json,
  failure_code, created_at, updated_at, completed_at
FROM mcp_auth_requests_v5;

DROP TABLE mcp_auth_requests_v5;

CREATE UNIQUE INDEX mcp_auth_requests_conversation_call
ON mcp_auth_requests(conversation_id, turn_id, output_index)
WHERE conversation_id IS NOT NULL AND governed_action_id IS NULL;

CREATE UNIQUE INDEX mcp_auth_requests_run_call
ON mcp_auth_requests(run_id, output_index)
WHERE run_id IS NOT NULL AND governed_action_id IS NULL;

CREATE UNIQUE INDEX mcp_auth_requests_governed_action
ON mcp_auth_requests(governed_action_id, governed_action_revision)
WHERE governed_action_id IS NOT NULL;

CREATE INDEX mcp_auth_requests_attention
ON mcp_auth_requests(owner_human_id, state, created_at, request_id)
WHERE state IN ('awaiting_user', 'authorizing');

CREATE INDEX mcp_auth_requests_attempt
ON mcp_auth_requests(oauth_attempt_id, state)
WHERE oauth_attempt_id IS NOT NULL;
"#;

const MCP_AUTH_RESULT_CONTEXT_SQL: &str = r#"
ALTER TABLE mcp_auth_requests
ADD COLUMN result_context_json TEXT
CHECK (result_context_json IS NULL OR json_valid(result_context_json));
"#;
