use rusqlite_migration::{M, Migrations};

/// Current forward-only SQLite migration version.
pub const STORE_SCHEMA_VERSION: usize = 40;

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
        M::up(CAPABILITY_AUTH_REQUESTS_SQL),
        M::up(ADAPTER_DEFINITIONS_SQL),
        M::up(ADAPTER_CONNECTIONS_SQL),
        M::up(CAPABILITY_AUTH_REQUESTS_DRIFT_REPAIR_SQL),
        M::up(REVIEWED_ACTION_POLICY_SQL),
        M::up(MCP_DEFINITION_CONNECTION_SQL),
        M::up(MCP_SERVICE_DESCRIPTION_SQL),
        M::up(ADAPTER_ACCOUNT_LABEL_SQL),
        M::up(ADAPTER_ACCOUNT_LABEL_REPAIR_SQL),
        M::up(ADAPTER_CONNECTION_LABEL_SQL),
        M::up(OPENROUTER_PROVIDER_SQL),
        M::up(CONVERSATION_INTERACTIONS_SQL),
        M::up(MODEL_PREFERENCE_SELECTION_SQL),
        M::up(TASK_MODEL_POOL_SELECTION_INDEX_REPAIR_SQL),
        M::up(CONVERSATION_INTERACTION_CALL_STATUS_REPAIR_SQL),
        M::up(CONVERSATION_INTERACTION_RESULT_PROJECTION_REPAIR_SQL),
        M::up(HOSTED_WEB_SEARCH_ACTIVITY_REPAIR_SQL),
        M::up(TASK_GATE_SUGGESTED_ANSWERS_SQL),
        M::up(CLIENTS_SQL),
        M::up(WEB_PUSH_SQL),
        M::up(TASK_SCHEDULES_SQL),
        M::up(ACP_WORK_EXECUTORS_SQL),
        M::up(TASK_RECURRENCE_MANUAL_TRIGGER_SQL),
        M::up(TASK_RECURRENCE_HISTORY_INDEX_REPAIR_SQL),
        M::up(WEB_BROWSE_PROVIDER_AND_OBSERVATIONS_SQL),
        M::up(SYSTEM_PROVIDER_ACCOUNTS_SQL),
        M::up(APNS_NOTIFICATIONS_SQL),
        M::up(ACTION_REQUEST_SOURCE_SQL),
        M::up(LIVE_ACTIVITIES_SQL),
        M::up(TASK_SUBMISSION_CITATIONS_SQL),
        M::up(ADAPTER_OAUTH_AUTHORITIES_SQL),
        M::up(ADAPTER_CONNECTION_GRANTS_SQL),
        M::up(ADAPTER_GRANT_LABEL_SQL),
    ])
}

const ADAPTER_GRANT_LABEL_SQL: &str = r#"
ALTER TABLE adapter_oauth_grants ADD COLUMN account_label TEXT;
"#;

const ADAPTER_CONNECTION_GRANTS_SQL: &str = r#"
DROP INDEX adapter_connections_definition;
DROP TABLE adapter_connections;

CREATE TABLE adapter_connections (
  connection_id TEXT PRIMARY KEY NOT NULL
    CHECK (length(connection_id) = 32 AND connection_id = lower(connection_id)),
  connection_slug TEXT UNIQUE,
  semantic_digest TEXT
    CHECK (semantic_digest IS NULL OR (length(semantic_digest) = 64 AND semantic_digest = lower(semantic_digest))),
  connection_label TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'authentication_required', 'blocked')),
  connection_revision INTEGER CHECK (connection_revision IS NULL OR connection_revision > 0),
  credential_revision INTEGER CHECK (credential_revision IS NULL OR credential_revision > 0),
  policy_revision INTEGER CHECK (policy_revision IS NULL OR policy_revision > 0),
  credential_generation TEXT
    CHECK (credential_generation IS NULL OR (length(credential_generation) = 32 AND credential_generation = lower(credential_generation))),
  grant_id TEXT,
  allowed_operations_json TEXT NOT NULL CHECK (json_valid(allowed_operations_json)),
  descriptor_relative_path TEXT NOT NULL
    CHECK (descriptor_relative_path GLOB 'adapters/connections/*/connection.json'),
  credential_relative_path TEXT
    CHECK (credential_relative_path IS NULL OR credential_relative_path GLOB 'adapters/connections/*/credentials/*.json'),
  diagnostic_code TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (grant_id) REFERENCES adapter_oauth_grants(grant_id) ON DELETE RESTRICT,
  CHECK (grant_id IS NULL OR (credential_revision IS NULL AND credential_generation IS NULL AND credential_relative_path IS NULL)),
  CHECK ((credential_generation IS NULL) = (credential_relative_path IS NULL)),
  CHECK (
    (status = 'blocked' AND connection_slug IS NULL AND semantic_digest IS NULL
      AND connection_revision IS NULL AND credential_revision IS NULL
      AND policy_revision IS NULL AND credential_generation IS NULL AND grant_id IS NULL
      AND credential_relative_path IS NULL AND diagnostic_code IS NOT NULL)
    OR
    (status <> 'blocked' AND connection_slug IS NOT NULL AND semantic_digest IS NOT NULL
      AND connection_revision IS NOT NULL AND policy_revision IS NOT NULL AND diagnostic_code IS NULL)
  )
);

CREATE INDEX adapter_connections_definition
ON adapter_connections(semantic_digest, status);
CREATE INDEX adapter_connections_grant
ON adapter_connections(grant_id, status);
"#;

/// Rebuildable public projections for reusable OAuth authorities.
const ADAPTER_OAUTH_AUTHORITIES_SQL: &str = r#"
UPDATE capability_auth_requests
SET state = 'superseded',
    revision = revision + 1,
    supersession_reason = 'adapter_oauth_authority_replaced',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE adapter_connection_id IS NOT NULL
  AND state IN ('awaiting_user', 'authorizing', 'resuming');

CREATE TABLE adapter_oauth_profiles (
  profile_digest TEXT PRIMARY KEY NOT NULL
    CHECK (length(profile_digest) = 64 AND profile_digest = lower(profile_digest)),
  profile_id TEXT NOT NULL CHECK (trim(profile_id) <> ''),
  display_name TEXT NOT NULL CHECK (trim(display_name) <> ''),
  grant_audience TEXT NOT NULL CHECK (trim(grant_audience) <> ''),
  descriptor_relative_path TEXT NOT NULL
    CHECK (descriptor_relative_path GLOB 'adapters/oauth-profiles/*/profile.json'),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX adapter_oauth_profiles_identity
ON adapter_oauth_profiles(profile_id, profile_digest);

CREATE TABLE adapter_oauth_applications (
  application_id TEXT PRIMARY KEY NOT NULL
    CHECK (length(application_id) = 32 AND application_id = lower(application_id)),
  profile_digest TEXT NOT NULL,
  callback_mode TEXT NOT NULL CHECK (callback_mode IN ('loopback', 'hosted')),
  client_id TEXT NOT NULL CHECK (trim(client_id) <> ''),
  project_label TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'suspended')),
  revision INTEGER NOT NULL CHECK (revision > 0),
  credential_generation TEXT NOT NULL
    CHECK (length(credential_generation) = 32 AND credential_generation = lower(credential_generation)),
  descriptor_relative_path TEXT NOT NULL
    CHECK (descriptor_relative_path GLOB 'adapters/oauth-applications/*/application.json'),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (profile_digest) REFERENCES adapter_oauth_profiles(profile_digest) ON DELETE RESTRICT
);

CREATE INDEX adapter_oauth_applications_profile
ON adapter_oauth_applications(profile_digest, status, application_id);

CREATE TABLE adapter_external_accounts (
  account_id TEXT PRIMARY KEY NOT NULL
    CHECK (length(account_id) = 32 AND account_id = lower(account_id)),
  profile_digest TEXT NOT NULL,
  provider_subject TEXT NOT NULL CHECK (trim(provider_subject) <> ''),
  account_label TEXT,
  revision INTEGER NOT NULL CHECK (revision > 0),
  descriptor_relative_path TEXT NOT NULL
    CHECK (descriptor_relative_path GLOB 'adapters/external-accounts/*/account.json'),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (profile_digest) REFERENCES adapter_oauth_profiles(profile_digest) ON DELETE RESTRICT,
  UNIQUE(profile_digest, provider_subject)
);

CREATE INDEX adapter_external_accounts_profile
ON adapter_external_accounts(profile_digest, account_id);

CREATE TABLE adapter_oauth_grants (
  grant_id TEXT PRIMARY KEY NOT NULL
    CHECK (length(grant_id) = 32 AND grant_id = lower(grant_id)),
  application_id TEXT NOT NULL,
  account_id TEXT,
  audience TEXT NOT NULL CHECK (trim(audience) <> ''),
  desired_scopes_json TEXT NOT NULL CHECK (json_valid(desired_scopes_json)),
  granted_scopes_json TEXT NOT NULL CHECK (json_valid(granted_scopes_json)),
  authority_revision INTEGER NOT NULL CHECK (authority_revision > 0),
  token_revision INTEGER NOT NULL CHECK (token_revision > 0),
  status TEXT NOT NULL CHECK (status IN ('active', 'authentication_required', 'revoked', 'blocked')),
  descriptor_relative_path TEXT NOT NULL
    CHECK (descriptor_relative_path GLOB 'adapters/oauth-grants/*/grant.json'),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (application_id) REFERENCES adapter_oauth_applications(application_id) ON DELETE RESTRICT,
  FOREIGN KEY (account_id) REFERENCES adapter_external_accounts(account_id) ON DELETE RESTRICT,
  UNIQUE(application_id, account_id, audience)
);

CREATE INDEX adapter_oauth_grants_account
ON adapter_oauth_grants(account_id, status, grant_id);
CREATE INDEX adapter_oauth_grants_application
ON adapter_oauth_grants(application_id, status, grant_id);
"#;

/// Ordered verified web citations attached to immutable task submissions.
const TASK_SUBMISSION_CITATIONS_SQL: &str = r#"
CREATE TABLE task_submission_citations (
  submission_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL CHECK (ordinal >= 1),
  title TEXT NOT NULL CHECK (trim(title) <> ''),
  url TEXT NOT NULL CHECK (trim(url) <> ''),
  start_index INTEGER CHECK (start_index IS NULL OR start_index >= 0),
  end_index INTEGER CHECK (end_index IS NULL OR end_index >= 0),
  PRIMARY KEY (submission_id, ordinal),
  FOREIGN KEY (submission_id) REFERENCES task_submissions(submission_id) ON DELETE RESTRICT,
  CHECK (start_index IS NULL OR end_index IS NULL OR start_index < end_index)
);
"#;

/// Link each foreground action request to its exact saved approval item.
const ACTION_REQUEST_SOURCE_SQL: &str = r#"
ALTER TABLE governed_actions
ADD COLUMN approval_item_id TEXT
  REFERENCES conversation_items(item_id) ON DELETE RESTRICT;

UPDATE governed_actions
SET approval_item_id = (
  SELECT items.item_id
  FROM conversation_items items
  WHERE items.conversation_id = governed_actions.conversation_id
    AND items.turn_id = governed_actions.turn_id
    AND items.kind = 'approval_request'
    AND json_extract(items.payload_json, '$.metadata.action.id') = governed_actions.action_id
  ORDER BY items.sequence_index ASC
  LIMIT 1
)
WHERE conversation_id IS NOT NULL AND task_id IS NULL;

CREATE UNIQUE INDEX governed_actions_approval_item
ON governed_actions(approval_item_id)
WHERE approval_item_id IS NOT NULL;
"#;

/// Durable native Tasks Live Activity registrations, projections, and APNs
/// delivery attempts.  The client registration row also acts as the explicit
/// disabled tombstone; an absent row means the client has never configured a
/// push-to-start token and therefore retains the default enabled preference.
const LIVE_ACTIVITIES_SQL: &str = r#"
ALTER TABLE notification_projection_state
ADD COLUMN task_notification_sequence INTEGER NOT NULL DEFAULT 0
  CHECK (task_notification_sequence >= 0);
UPDATE notification_projection_state
SET task_notification_sequence = COALESCE(
  (SELECT MAX(event_sequence) FROM work_notification_outbox),
  0
)
WHERE state_id = 1;

ALTER TABLE apns_deliveries
ADD COLUMN route TEXT NOT NULL DEFAULT 'chat'
  CHECK (route IN ('chat', 'task'));
ALTER TABLE apns_deliveries
ADD COLUMN task_id TEXT
  CHECK ((route = 'chat' AND task_id IS NULL)
      OR (route = 'task' AND task_id GLOB 'task:*' AND length(task_id) BETWEEN 6 AND 256));

CREATE TABLE client_live_activity_registrations (
  client_id TEXT PRIMARY KEY NOT NULL,
  push_to_start_token BLOB
    CHECK (push_to_start_token IS NULL OR length(push_to_start_token) BETWEEN 1 AND 1024),
  environment TEXT
    CHECK (environment IS NULL OR environment IN ('development', 'production')),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK ((push_to_start_token IS NULL) = (environment IS NULL)),
  CHECK (enabled = 1 OR push_to_start_token IS NULL),
  FOREIGN KEY (client_id) REFERENCES clients(client_id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX client_live_activity_push_token
ON client_live_activity_registrations(environment, push_to_start_token)
WHERE push_to_start_token IS NOT NULL;

CREATE TABLE client_task_activities (
  client_id TEXT PRIMARY KEY NOT NULL,
  activity_id TEXT CHECK (activity_id IS NULL OR (activity_id GLOB 'live_activity:*' AND length(activity_id) <= 256)),
  task_session_id TEXT NOT NULL CHECK (task_session_id GLOB 'task_activity:*' AND length(task_session_id) <= 256),
  lifecycle TEXT NOT NULL CHECK (lifecycle IN ('starting', 'active', 'ending', 'dismissed')),
  update_token BLOB CHECK (update_token IS NULL OR length(update_token) BETWEEN 1 AND 1024),
  latest_projection_json TEXT NOT NULL DEFAULT '{}'
    CHECK (json_valid(latest_projection_json) AND length(CAST(latest_projection_json AS BLOB)) <= 4096),
  latest_projection_signature TEXT NOT NULL DEFAULT ''
    CHECK (latest_projection_signature = '' OR (length(latest_projection_signature) = 64 AND latest_projection_signature = lower(latest_projection_signature))),
  focused_task_id TEXT,
  session_started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  suppressed INTEGER NOT NULL DEFAULT 0 CHECK (suppressed IN (0, 1)),
  dismissed_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (update_token IS NULL OR activity_id IS NOT NULL),
  FOREIGN KEY (client_id) REFERENCES clients(client_id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX client_task_activity_id
ON client_task_activities(activity_id)
WHERE activity_id IS NOT NULL;
CREATE UNIQUE INDEX client_task_activity_update_token
ON client_task_activities(update_token)
WHERE update_token IS NOT NULL;

CREATE TABLE live_activity_deliveries (
  client_id TEXT NOT NULL,
  delivery_key TEXT NOT NULL CHECK (trim(delivery_key) <> '' AND length(delivery_key) <= 256),
  activity_id TEXT,
  token BLOB NOT NULL CHECK (length(token) BETWEEN 1 AND 1024),
  environment TEXT NOT NULL CHECK (environment IN ('development', 'production')),
  event TEXT NOT NULL CHECK (event IN ('start', 'update', 'end')),
  payload_json TEXT NOT NULL
    CHECK (json_valid(payload_json) AND length(CAST(payload_json AS BLOB)) <= 4096),
  urgency TEXT NOT NULL CHECK (urgency IN ('normal', 'high')),
  ttl_seconds INTEGER NOT NULL CHECK (ttl_seconds BETWEEN 0 AND 604800),
  status TEXT NOT NULL CHECK (status IN ('pending', 'delivered', 'suppressed', 'failed')),
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 4),
  available_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  last_error_code TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (client_id, delivery_key)
);

CREATE INDEX live_activity_deliveries_due
ON live_activity_deliveries(status, available_at, client_id, delivery_key)
WHERE status = 'pending';
"#;

/// Repair v30 databases migrated before manual occurrences changed history ordering.
const TASK_RECURRENCE_HISTORY_INDEX_REPAIR_SQL: &str = r#"
DROP INDEX task_recurrence_occurrences_history;
CREATE INDEX task_recurrence_occurrences_history
ON task_recurrence_occurrences(recurrence_id, created_at DESC, occurrence_id DESC);
"#;

const WEB_BROWSE_PROVIDER_AND_OBSERVATIONS_SQL: &str = r#"
ALTER TABLE provider_capability_bindings RENAME TO provider_capability_bindings_v31;
CREATE TABLE provider_capability_bindings (
  binding_id TEXT PRIMARY KEY NOT NULL,
  tool_name TEXT NOT NULL CHECK (tool_name IN ('web.search', 'web.fetch', 'web.browse')),
  capability_id TEXT NOT NULL,
  provider_account_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(tool_name, capability_id)
);
INSERT INTO provider_capability_bindings SELECT * FROM provider_capability_bindings_v31;
DROP TABLE provider_capability_bindings_v31;

ALTER TABLE observed_urls RENAME TO observed_urls_v31;
CREATE TABLE observed_urls (
  normalized_url TEXT PRIMARY KEY NOT NULL CHECK (trim(normalized_url) <> ''),
  source_kind TEXT NOT NULL CHECK (source_kind IN ('search_result', 'fetched_link', 'browser_link')),
  source_event_reference TEXT NOT NULL CHECK (trim(source_event_reference) <> ''),
  first_observed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  last_observed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO observed_urls SELECT * FROM observed_urls_v31;
DROP TABLE observed_urls_v31;
"#;

/// Persist selectable built-in web backends through the canonical account authority.
const SYSTEM_PROVIDER_ACCOUNTS_SQL: &str = r#"
ALTER TABLE provider_accounts RENAME TO provider_accounts_v32;
CREATE TABLE provider_accounts (
  provider_account_id TEXT PRIMARY KEY NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN (
    'codex', 'openai', 'foundation_local', 'local_models', 'openrouter', 'exa',
    'duckduckgo_public', 'direct_http', 'obscura'
  )),
  account_key TEXT NOT NULL,
  display_name TEXT NOT NULL,
  auth_method TEXT NOT NULL CHECK (auth_method IN ('oauth_device_code', 'oauth_pkce', 'secret_input', 'external_manual', 'none')),
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
INSERT INTO provider_accounts SELECT * FROM provider_accounts_v32;
DROP TABLE provider_accounts_v32;

INSERT INTO provider_accounts (
  provider_account_id, provider_kind, account_key, display_name, auth_method,
  is_active, is_default, status, metadata_json
) VALUES
  ('provider_account:duckduckgo_public:system', 'duckduckgo_public', 'system', 'DuckDuckGo public search', 'none', 1, 1, 'authenticated', '{}'),
  ('provider_account:direct_http:system', 'direct_http', 'system', 'Direct HTTP web fetch', 'none', 1, 1, 'authenticated', '{}'),
  ('provider_account:obscura:system', 'obscura', 'system', 'Obscura interactive browser', 'none', 1, 1, 'authenticated', '{}');
"#;

/// Distinguish cron slots from explicitly requested extra occurrences.
const TASK_RECURRENCE_MANUAL_TRIGGER_SQL: &str = r#"
ALTER TABLE task_recurrence_occurrences ADD COLUMN trigger_kind TEXT NOT NULL DEFAULT 'scheduled'
  CHECK (trigger_kind IN ('scheduled', 'manual'));
DROP INDEX task_recurrence_occurrences_history;
CREATE INDEX task_recurrence_occurrences_history
ON task_recurrence_occurrences(recurrence_id, created_at DESC, occurrence_id DESC);
"#;

/// Generic stdio ACP agents plus immutable Work executor snapshots.
const ACP_WORK_EXECUTORS_SQL: &str = r#"
ALTER TABLE projects ADD COLUMN folder TEXT
  CHECK (folder IS NULL OR trim(folder) <> '');

ALTER TABLE tasks ADD COLUMN executor_agent_id TEXT NOT NULL DEFAULT 'agent:task-executor'
  CHECK (trim(executor_agent_id) <> '');
ALTER TABLE tasks ADD COLUMN cwd_override TEXT
  CHECK (cwd_override IS NULL OR trim(cwd_override) <> '');
ALTER TABLE task_recurrences ADD COLUMN executor_agent_id TEXT NOT NULL DEFAULT 'agent:task-executor'
  CHECK (trim(executor_agent_id) <> '');
ALTER TABLE task_recurrences ADD COLUMN cwd_override TEXT
  CHECK (cwd_override IS NULL OR trim(cwd_override) <> '');

CREATE TABLE acp_agents (
  agent_id TEXT PRIMARY KEY NOT NULL CHECK (agent_id GLOB 'agent:*'),
  command TEXT NOT NULL CHECK (trim(command) <> ''),
  arguments_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(arguments_json) AND json_type(arguments_json) = 'array'),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  auth_status TEXT NOT NULL DEFAULT 'unknown' CHECK (auth_status IN ('unknown', 'none', 'required', 'authenticated', 'failed')),
  health_status TEXT NOT NULL DEFAULT 'unknown' CHECK (health_status IN ('unknown', 'healthy', 'unavailable')),
  implementation_name TEXT,
  implementation_version TEXT,
  capabilities_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(capabilities_json) AND json_type(capabilities_json) = 'object'),
  connection_revision INTEGER NOT NULL DEFAULT 1 CHECK (connection_revision >= 1),
  last_error TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (agent_id) REFERENCES agents(agent_id) ON DELETE RESTRICT
);

CREATE TABLE acp_auth_attempts (
  attempt_id TEXT PRIMARY KEY NOT NULL CHECK (attempt_id GLOB 'acp_auth:*'),
  agent_id TEXT NOT NULL,
  connection_revision INTEGER NOT NULL CHECK (connection_revision >= 1),
  method_id TEXT NOT NULL CHECK (trim(method_id) <> ''),
  state TEXT NOT NULL CHECK (state IN ('pending', 'completed', 'failed', 'cancelled')),
  authorization_url TEXT,
  safe_message TEXT,
  failure_code TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT,
  FOREIGN KEY (agent_id) REFERENCES acp_agents(agent_id) ON DELETE RESTRICT
);

CREATE TABLE acp_permission_consumptions (
  action_id TEXT NOT NULL,
  revision INTEGER NOT NULL CHECK (revision >= 1),
  consumed_by_run_id TEXT NOT NULL,
  consumed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (action_id, revision),
  FOREIGN KEY (action_id, revision) REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,
  FOREIGN KEY (consumed_by_run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT
);

ALTER TABLE task_execution_contracts ADD COLUMN executor_backend_kind TEXT NOT NULL DEFAULT 'provider'
  CHECK (executor_backend_kind IN ('provider', 'acp'));
ALTER TABLE task_execution_contracts ADD COLUMN executor_agent_id TEXT NOT NULL DEFAULT 'agent:task-executor'
  CHECK (trim(executor_agent_id) <> '');
ALTER TABLE task_execution_contracts ADD COLUMN executor_acp_connection_revision INTEGER
  CHECK (executor_acp_connection_revision IS NULL OR executor_acp_connection_revision >= 1);
ALTER TABLE task_execution_contracts ADD COLUMN executor_acp_launch_json TEXT
  CHECK (executor_acp_launch_json IS NULL OR json_valid(executor_acp_launch_json));
ALTER TABLE task_execution_contracts ADD COLUMN effective_cwd TEXT
  CHECK (effective_cwd IS NULL OR trim(effective_cwd) <> '');
ALTER TABLE task_execution_contracts ADD COLUMN project_folder_snapshot TEXT
  CHECK (project_folder_snapshot IS NULL OR trim(project_folder_snapshot) <> '');

ALTER TABLE agent_runs ADD COLUMN execution_backend_kind TEXT NOT NULL DEFAULT 'provider'
  CHECK (execution_backend_kind IN ('provider', 'acp'));
ALTER TABLE agent_runs ADD COLUMN acp_connection_revision INTEGER
  CHECK (acp_connection_revision IS NULL OR acp_connection_revision >= 1);
ALTER TABLE agent_runs ADD COLUMN acp_launch_json TEXT
  CHECK (acp_launch_json IS NULL OR json_valid(acp_launch_json));
ALTER TABLE agent_runs ADD COLUMN effective_cwd TEXT
  CHECK (effective_cwd IS NULL OR trim(effective_cwd) <> '');
ALTER TABLE agent_runs ADD COLUMN acp_session_id TEXT;

CREATE INDEX tasks_executor_agent ON tasks(executor_agent_id, stage_id, updated_at);
CREATE INDEX acp_auth_attempts_agent ON acp_auth_attempts(agent_id, created_at DESC);
"#;

/// Optional one-time timing on tasks plus continuing authority for Repeat.
const TASK_SCHEDULES_SQL: &str = r#"
ALTER TABLE tasks ADD COLUMN scheduled_for INTEGER;
ALTER TABLE tasks ADD COLUMN schedule_time_zone TEXT;
ALTER TABLE tasks ADD COLUMN missed_run_policy TEXT CHECK (missed_run_policy IN ('skip', 'run_once'));
ALTER TABLE tasks ADD COLUMN recurrence_id TEXT;
ALTER TABLE tasks ADD COLUMN recurrence_revision INTEGER CHECK (recurrence_revision IS NULL OR recurrence_revision >= 1);
ALTER TABLE tasks ADD COLUMN recurrence_scheduled_for INTEGER;

CREATE TABLE task_recurrences (
  recurrence_id TEXT PRIMARY KEY NOT NULL CHECK (recurrence_id GLOB 'recurrence:*'),
  workspace_id TEXT NOT NULL,
  project_id TEXT,
  title TEXT NOT NULL CHECK (trim(title) <> ''),
  description_markdown TEXT NOT NULL DEFAULT '',
  authorization_context_json TEXT NOT NULL CHECK (json_valid(authorization_context_json)),
  starts_at INTEGER NOT NULL,
  cron_expression TEXT NOT NULL CHECK (trim(cron_expression) <> ''),
  time_zone TEXT NOT NULL CHECK (trim(time_zone) <> ''),
  missed_run_policy TEXT NOT NULL CHECK (missed_run_policy IN ('skip', 'run_once')),
  overlap_policy TEXT NOT NULL CHECK (overlap_policy IN ('skip', 'queue_one', 'allow')),
  lifecycle TEXT NOT NULL CHECK (lifecycle IN ('active', 'paused', 'ended')),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  next_run_at INTEGER,
  pending_coalesced_at INTEGER,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE RESTRICT,
  FOREIGN KEY (workspace_id, project_id)
    REFERENCES projects(workspace_id, project_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
  CHECK (lifecycle <> 'active' OR next_run_at IS NOT NULL)
);

CREATE TABLE task_recurrence_occurrences (
  occurrence_id TEXT PRIMARY KEY NOT NULL CHECK (occurrence_id GLOB 'occurrence:*'),
  recurrence_id TEXT NOT NULL,
  recurrence_revision INTEGER NOT NULL CHECK (recurrence_revision >= 1),
  scheduled_for INTEGER NOT NULL,
  local_slot TEXT NOT NULL CHECK (trim(local_slot) <> ''),
  resolution TEXT NOT NULL CHECK (resolution IN ('materialized', 'skipped', 'coalesced')),
  task_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (recurrence_id) REFERENCES task_recurrences(recurrence_id) ON DELETE RESTRICT,
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  UNIQUE (recurrence_id, local_slot),
  CHECK ((resolution = 'materialized') = (task_id IS NOT NULL))
);

CREATE INDEX tasks_next_scheduled
ON tasks(scheduled_for, task_id)
WHERE scheduled_for IS NOT NULL AND queued_at IS NULL AND completed_at IS NULL AND cancelled_at IS NULL;

CREATE INDEX task_recurrences_next_due
ON task_recurrences(next_run_at, recurrence_id)
WHERE lifecycle = 'active';

CREATE INDEX task_recurrence_occurrences_history
ON task_recurrence_occurrences(recurrence_id, scheduled_for DESC, occurrence_id DESC);
"#;

/// Durable Web Push identity, per-installation subscriptions, and delivery state.
const WEB_PUSH_SQL: &str = r#"
CREATE TABLE web_push_identity (
  identity_id INTEGER PRIMARY KEY NOT NULL CHECK (identity_id = 1),
  private_key BLOB NOT NULL CHECK (length(private_key) = 32),
  public_key BLOB NOT NULL CHECK (length(public_key) = 65),
  primary_conversation_id TEXT,
  primary_sequence INTEGER NOT NULL DEFAULT 0 CHECK (primary_sequence >= 0),
  attention_seeded INTEGER NOT NULL DEFAULT 0 CHECK (attention_seeded IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (primary_conversation_id) REFERENCES conversations(conversation_id) ON DELETE SET NULL
);

CREATE TABLE web_push_subscriptions (
  subscription_id TEXT PRIMARY KEY NOT NULL CHECK (trim(subscription_id) <> '' AND length(subscription_id) <= 128),
  owner_human_id TEXT NOT NULL,
  endpoint TEXT NOT NULL UNIQUE CHECK (length(endpoint) BETWEEN 1 AND 2048),
  p256dh TEXT NOT NULL CHECK (length(p256dh) BETWEEN 40 AND 256),
  auth_secret TEXT NOT NULL CHECK (length(auth_secret) BETWEEN 16 AND 128),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE CASCADE
);

CREATE INDEX web_push_subscriptions_owner
ON web_push_subscriptions(owner_human_id, created_at, subscription_id);

CREATE TABLE web_push_deliveries (
  subscription_id TEXT NOT NULL,
  event_key TEXT NOT NULL CHECK (trim(event_key) <> '' AND length(event_key) <= 256),
  title TEXT NOT NULL CHECK (trim(title) <> '' AND length(title) <= 256),
  body TEXT NOT NULL CHECK (length(body) <= 2048),
  navigate_path TEXT NOT NULL CHECK (navigate_path GLOB '/*' AND length(navigate_path) <= 1024),
  urgency TEXT NOT NULL CHECK (urgency IN ('normal', 'high')),
  ttl_seconds INTEGER NOT NULL CHECK (ttl_seconds BETWEEN 0 AND 604800),
  status TEXT NOT NULL CHECK (status IN ('pending', 'delivered', 'suppressed', 'failed')),
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 4),
  available_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  last_error_code TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (subscription_id, event_key),
  FOREIGN KEY (subscription_id) REFERENCES web_push_subscriptions(subscription_id) ON DELETE CASCADE
);

CREATE INDEX web_push_deliveries_due
ON web_push_deliveries(status, available_at, subscription_id, event_key)
WHERE status = 'pending';

CREATE TABLE web_push_attention_seen (
  attention_key TEXT PRIMARY KEY NOT NULL CHECK (trim(attention_key) <> '' AND length(attention_key) <= 256),
  observed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
"#;

/// Durable local-human client credentials. The token itself never enters the
/// database; only its fixed-size SHA-256 digest is retained.
const CLIENTS_SQL: &str = r#"
CREATE TABLE clients (
  client_id TEXT PRIMARY KEY NOT NULL CHECK (trim(client_id) <> '' AND length(client_id) <= 128),
  owner_human_id TEXT NOT NULL,
  display_name TEXT NOT NULL CHECK (
    length(display_name) BETWEEN 1 AND 128 AND trim(display_name) = display_name
  ),
  token_hash BLOB NOT NULL CHECK (length(token_hash) = 32),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  revoked_at TEXT,
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT
);

CREATE INDEX clients_owner_created
ON clients(owner_human_id, created_at, client_id);
"#;

const TASK_GATE_SUGGESTED_ANSWERS_SQL: &str = r#"
ALTER TABLE task_gates
ADD COLUMN suggested_answers_json TEXT NOT NULL DEFAULT '[]'
CHECK (json_valid(suggested_answers_json) AND json_type(suggested_answers_json) = 'array');
"#;

/// Replace provenance-shaped model defaults with explicit preference intent.
const MODEL_PREFERENCE_SELECTION_SQL: &str = r#"
ALTER TABLE agent_runtime_preferences RENAME TO agent_runtime_preferences_v19;
CREATE TABLE agent_runtime_preferences (
  agent_id TEXT PRIMARY KEY NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models', 'openrouter')),
  provider_account_id TEXT NOT NULL,
  provider_instance_key TEXT NOT NULL CHECK (provider_instance_key <> ''),
  selection_mode TEXT NOT NULL CHECK (selection_mode IN ('noema_recommended', 'explicit_profile')),
  model_profile TEXT,
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (
    (selection_mode = 'noema_recommended'
      AND provider_kind IN ('codex', 'openai', 'openrouter')
      AND model_profile IS NULL AND reasoning_effort IS NULL)
    OR
    (selection_mode = 'explicit_profile'
      AND model_profile IS NOT NULL AND trim(model_profile) <> '')
  )
);
INSERT INTO agent_runtime_preferences (
  agent_id, provider_kind, provider_account_id, provider_instance_key,
  selection_mode, model_profile, reasoning_effort, created_at, updated_at
)
SELECT agent_id, provider_kind, provider_account_id, provider_instance_key,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN 'noema_recommended' ELSE 'explicit_profile' END,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN NULL ELSE model_profile END,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN NULL ELSE reasoning_effort END,
  created_at, updated_at
FROM agent_runtime_preferences_v19;
DROP TABLE agent_runtime_preferences_v19;
CREATE INDEX agent_runtime_preferences_instance ON agent_runtime_preferences(provider_instance_key);

ALTER TABLE auxiliary_model_preferences RENAME TO auxiliary_model_preferences_v19;
CREATE TABLE auxiliary_model_preferences (
  task_id TEXT PRIMARY KEY NOT NULL CHECK (task_id IN ('web_fetch_summarizer', 'tool_progress_audit', 'memory_extraction', 'action_reviewer')),
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models', 'openrouter')),
  provider_account_id TEXT NOT NULL,
  provider_instance_key TEXT NOT NULL CHECK (provider_instance_key <> ''),
  selection_mode TEXT NOT NULL CHECK (selection_mode IN ('noema_recommended', 'explicit_profile')),
  model_profile TEXT,
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (
    (selection_mode = 'noema_recommended'
      AND provider_kind IN ('codex', 'openai', 'openrouter')
      AND model_profile IS NULL AND reasoning_effort IS NULL)
    OR
    (selection_mode = 'explicit_profile'
      AND model_profile IS NOT NULL AND trim(model_profile) <> '')
  )
);
INSERT INTO auxiliary_model_preferences (
  task_id, provider_kind, provider_account_id, provider_instance_key,
  selection_mode, model_profile, reasoning_effort, created_at, updated_at
)
SELECT task_id, provider_kind, provider_account_id, provider_instance_key,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN 'noema_recommended' ELSE 'explicit_profile' END,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN NULL ELSE model_profile END,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN NULL ELSE reasoning_effort END,
  created_at, updated_at
FROM auxiliary_model_preferences_v19;
DROP TABLE auxiliary_model_preferences_v19;
CREATE INDEX auxiliary_model_preferences_instance ON auxiliary_model_preferences(provider_instance_key);

ALTER TABLE default_model_preference RENAME TO default_model_preference_v19;
CREATE TABLE default_model_preference (
  preference_id TEXT PRIMARY KEY NOT NULL CHECK (preference_id = 'default'),
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models', 'openrouter')),
  provider_account_id TEXT NOT NULL,
  provider_instance_key TEXT NOT NULL CHECK (provider_instance_key <> ''),
  selection_mode TEXT NOT NULL CHECK (selection_mode IN ('noema_recommended', 'explicit_profile')),
  model_profile TEXT,
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (
    (selection_mode = 'noema_recommended'
      AND provider_kind IN ('codex', 'openai', 'openrouter')
      AND model_profile IS NULL AND reasoning_effort IS NULL)
    OR
    (selection_mode = 'explicit_profile'
      AND model_profile IS NOT NULL AND trim(model_profile) <> '')
  )
);
INSERT INTO default_model_preference (
  preference_id, provider_kind, provider_account_id, provider_instance_key,
  selection_mode, model_profile, reasoning_effort, created_at, updated_at
)
SELECT preference_id, provider_kind, provider_account_id, provider_instance_key,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN 'noema_recommended' ELSE 'explicit_profile' END,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN NULL ELSE model_profile END,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN NULL ELSE reasoning_effort END,
  created_at, updated_at
FROM default_model_preference_v19;
DROP TABLE default_model_preference_v19;
CREATE INDEX default_model_preference_instance ON default_model_preference(provider_instance_key);

ALTER TABLE task_model_pool_entries RENAME TO task_model_pool_entries_v19;
CREATE TABLE task_model_pool_entries (
  pool_entry_id TEXT PRIMARY KEY NOT NULL,
  complexity TEXT NOT NULL CHECK (complexity IN ('simple', 'medium', 'difficult')),
  label TEXT,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models', 'openrouter')),
  provider_account_id TEXT NOT NULL,
  provider_instance_key TEXT NOT NULL CHECK (provider_instance_key <> ''),
  selection_mode TEXT NOT NULL CHECK (selection_mode IN ('noema_recommended', 'explicit_profile')),
  model_profile TEXT,
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (
    (selection_mode = 'noema_recommended'
      AND provider_kind IN ('codex', 'openai', 'openrouter')
      AND model_profile IS NULL AND reasoning_effort IS NULL)
    OR
    (selection_mode = 'explicit_profile'
      AND model_profile IS NOT NULL AND trim(model_profile) <> '')
  )
);
INSERT INTO task_model_pool_entries (
  pool_entry_id, complexity, label, provider_kind, provider_account_id,
  provider_instance_key, selection_mode, model_profile, reasoning_effort,
  enabled, sort_order, created_at, updated_at
)
SELECT pool_entry_id, complexity, label, provider_kind, provider_account_id,
  provider_instance_key,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN 'noema_recommended' ELSE 'explicit_profile' END,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN NULL ELSE model_profile END,
  CASE WHEN is_override = 0 AND provider_kind IN ('codex', 'openai', 'openrouter')
    THEN NULL ELSE reasoning_effort END,
  enabled, sort_order, created_at, updated_at
FROM task_model_pool_entries_v19;
DROP TABLE task_model_pool_entries_v19;
CREATE INDEX task_model_pool_entries_selection ON task_model_pool_entries(complexity, enabled, sort_order, label, pool_entry_id);
CREATE INDEX task_model_pool_entries_instance ON task_model_pool_entries(provider_instance_key) WHERE enabled = 1;
-- Legacy pools may contain several distinct defaults that all become delegated;
-- keep those rows lossless while retaining exact-profile uniqueness.
CREATE UNIQUE INDEX task_model_pool_entries_unique_selection ON task_model_pool_entries(
  complexity, provider_account_id, selection_mode,
  COALESCE(model_profile, pool_entry_id), COALESCE(reasoning_effort, '')
);
"#;

/// Converge databases that applied the earlier v20 delegated-selection index.
const TASK_MODEL_POOL_SELECTION_INDEX_REPAIR_SQL: &str = r#"
DROP INDEX task_model_pool_entries_unique_selection;
CREATE UNIQUE INDEX task_model_pool_entries_unique_selection ON task_model_pool_entries(
  complexity, provider_account_id, selection_mode,
  COALESCE(model_profile, pool_entry_id), COALESCE(reasoning_effort, '')
);
"#;

/// Terminalize provider calls whose durable interaction already has a result.
const CONVERSATION_INTERACTION_CALL_STATUS_REPAIR_SQL: &str = r#"
UPDATE conversation_items
SET status = (
      SELECT result.status
      FROM conversation_interactions AS interaction
      JOIN conversation_items AS result
        ON result.item_id = interaction.tool_result_item_id
      WHERE interaction.provider_call_item_id = conversation_items.item_id
        AND result.status IN ('completed', 'failed', 'cancelled', 'interrupted')
    ),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE kind = 'tool_call'
  AND status IN ('pending', 'running')
  AND EXISTS (
    SELECT 1
    FROM conversation_interactions AS interaction
    JOIN conversation_items AS result
      ON result.item_id = interaction.tool_result_item_id
    WHERE interaction.provider_call_item_id = conversation_items.item_id
      AND result.status IN ('completed', 'failed', 'cancelled', 'interrupted')
  );
"#;

/// Add replay fields missing from already-persisted interaction results.
const CONVERSATION_INTERACTION_RESULT_PROJECTION_REPAIR_SQL: &str = r#"
UPDATE conversation_items
SET payload_json = json_set(
      payload_json,
      '$.id', (
        SELECT 'tool_result:' || interaction.interaction_id
        FROM conversation_interactions AS interaction
        WHERE interaction.tool_result_item_id = conversation_items.item_id
      ),
      '$.activity_kind', 'tool_result',
      '$.title', (
        SELECT 'Tool result: ' || interaction.canonical_tool_name
        FROM conversation_interactions AS interaction
        WHERE interaction.tool_result_item_id = conversation_items.item_id
      )
    )
WHERE kind = 'tool_result'
  AND status IN ('completed', 'failed', 'cancelled', 'interrupted')
  AND EXISTS (
    SELECT 1
    FROM conversation_interactions AS interaction
    WHERE interaction.tool_result_item_id = conversation_items.item_id
  );
"#;

/// Keep provider-hosted searches visible without replaying them as model tool envelopes.
const HOSTED_WEB_SEARCH_ACTIVITY_REPAIR_SQL: &str = r#"
UPDATE conversation_items
SET kind = 'activity'
WHERE kind IN ('tool_call', 'tool_result')
  AND json_extract(metadata_json, '$.source') = 'provider_action'
  AND json_extract(payload_json, '$.metadata.action.name') = 'web.search'
  AND json_extract(payload_json, '$.metadata.action.provider_name')
      = json_extract(metadata_json, '$.provider');
"#;

/// Canonical durable authority for provider-native human interactions.
///
/// The provider/account/model fields are immutable references to the route
/// admitted for the suspended call. Request and projection JSON are validated
/// snapshots; credentials and other secret bodies never belong in this row.
const CONVERSATION_INTERACTIONS_SQL: &str = r#"
CREATE TABLE conversation_interactions (
  interaction_id TEXT PRIMARY KEY NOT NULL CHECK (trim(interaction_id) <> ''),
  conversation_id TEXT NOT NULL,
  originating_turn_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('multiple_choice', 'a2ui')),
  provider_call_id TEXT NOT NULL CHECK (trim(provider_call_id) <> ''),
  canonical_tool_name TEXT NOT NULL CHECK (trim(canonical_tool_name) <> ''),
  provider_tool_name TEXT NOT NULL CHECK (trim(provider_tool_name) <> ''),
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models', 'openrouter')),
  provider_account_id TEXT NOT NULL CHECK (trim(provider_account_id) <> ''),
  provider_instance_key TEXT NOT NULL CHECK (trim(provider_instance_key) <> ''),
  selection_mode TEXT NOT NULL CHECK (selection_mode IN ('explicit_profile', 'provider_default')),
  credential_revision INTEGER NOT NULL CHECK (credential_revision >= 0),
  model TEXT NOT NULL CHECK (trim(model) <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  tool_catalog_digest TEXT NOT NULL
    CHECK (length(tool_catalog_digest) = 64 AND tool_catalog_digest = lower(tool_catalog_digest)),
  request_json TEXT NOT NULL CHECK (json_valid(request_json)),
  projection_json TEXT NOT NULL CHECK (json_valid(projection_json)),
  provider_call_item_id TEXT NOT NULL,
  projection_item_id TEXT NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  lifecycle_status TEXT NOT NULL CHECK (lifecycle_status IN ('pending', 'answered', 'resuming', 'completed', 'failed')),
  resolution_item_id TEXT,
  tool_result_item_id TEXT,
  resolution_json TEXT CHECK (resolution_json IS NULL OR json_valid(resolution_json)),
  client_message_id TEXT CHECK (client_message_id IS NULL OR trim(client_message_id) <> ''),
  resume_claim_owner TEXT CHECK (resume_claim_owner IS NULL OR trim(resume_claim_owner) <> ''),
  resume_claim_token TEXT CHECK (resume_claim_token IS NULL OR trim(resume_claim_token) <> ''),
  resume_claim_expires_at TEXT,
  terminal_error TEXT,
  resolved_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY (originating_turn_id) REFERENCES conversation_turns(turn_id) ON DELETE RESTRICT,
  FOREIGN KEY (provider_call_item_id) REFERENCES conversation_items(item_id) ON DELETE RESTRICT,
  FOREIGN KEY (projection_item_id) REFERENCES conversation_items(item_id) ON DELETE RESTRICT,
  FOREIGN KEY (resolution_item_id) REFERENCES conversation_items(item_id) ON DELETE RESTRICT,
  FOREIGN KEY (tool_result_item_id) REFERENCES conversation_items(item_id) ON DELETE RESTRICT,
  CHECK (
    (lifecycle_status = 'pending'
      AND resolution_item_id IS NULL
      AND tool_result_item_id IS NULL
      AND resolution_json IS NULL
      AND client_message_id IS NULL
      AND resolved_at IS NULL)
    OR
    (lifecycle_status <> 'pending'
      AND resolution_item_id IS NOT NULL
      AND tool_result_item_id IS NOT NULL
      AND resolution_json IS NOT NULL
      AND client_message_id IS NOT NULL
      AND resolved_at IS NOT NULL)
  ),
  CHECK (
    (lifecycle_status = 'resuming'
      AND resume_claim_owner IS NOT NULL
      AND resume_claim_token IS NOT NULL
      AND resume_claim_expires_at IS NOT NULL)
    OR
    (lifecycle_status <> 'resuming'
      AND resume_claim_owner IS NULL
      AND resume_claim_token IS NULL
      AND resume_claim_expires_at IS NULL)
  ),
  CHECK (lifecycle_status NOT IN ('completed', 'failed') OR completed_at IS NOT NULL)
);

CREATE UNIQUE INDEX conversation_interactions_conversation_provider_call
ON conversation_interactions(conversation_id, provider_call_id);

CREATE UNIQUE INDEX conversation_interactions_conversation_client_message
ON conversation_interactions(conversation_id, client_message_id)
WHERE client_message_id IS NOT NULL;

CREATE INDEX conversation_interactions_conversation_status
ON conversation_interactions(conversation_id, lifecycle_status, updated_at, interaction_id);

CREATE INDEX conversation_interactions_resume_claim
ON conversation_interactions(lifecycle_status, resume_claim_expires_at, interaction_id)
WHERE lifecycle_status = 'resuming';
"#;

// SQLite cannot alter CHECK constraints in place. The v18 migration updates
// only the stored CREATE TABLE definitions, preserving every row, index, and
// foreign-key relationship while expanding the closed provider enums.
const OPENROUTER_PROVIDER_SQL: &str = r#"
PRAGMA writable_schema = ON;

UPDATE sqlite_schema
SET sql = replace(
  sql,
  '''codex'', ''openai'', ''foundation_local'', ''local_models''',
  '''codex'', ''openai'', ''foundation_local'', ''local_models'', ''openrouter'''
)
WHERE type = 'table' AND sql LIKE '%''codex'', ''openai'', ''foundation_local'', ''local_models''%';

UPDATE sqlite_schema
SET sql = replace(
  sql,
  '''codex'', ''openai'', ''foundation_local'', ''local_models'', ''exa''',
  '''codex'', ''openai'', ''foundation_local'', ''local_models'', ''openrouter'', ''exa'''
)
WHERE name = 'provider_accounts';

UPDATE sqlite_schema
SET sql = replace(
  sql,
  '''oauth_device_code'', ''secret_input'', ''external_manual'', ''none''',
  '''oauth_device_code'', ''oauth_pkce'', ''secret_input'', ''external_manual'', ''none'''
)
WHERE name = 'provider_accounts';

PRAGMA writable_schema = RESET;
"#;

const ADAPTER_CONNECTION_LABEL_SQL: &str =
    "ALTER TABLE adapter_connections RENAME COLUMN account_label TO connection_label;";

const ADAPTER_ACCOUNT_LABEL_SQL: &str = r#"
ALTER TABLE adapter_connections ADD COLUMN account_label TEXT
  CHECK (account_label IS NULL OR (length(CAST(account_label AS BLOB)) BETWEEN 1 AND 256 AND trim(account_label) = account_label));
"#;

// Some development databases applied v15 before its length check was corrected
// to count UTF-8 bytes. Rebuild the table append-only so both v15 shapes converge.
const ADAPTER_ACCOUNT_LABEL_REPAIR_SQL: &str = r#"
DROP INDEX adapter_connections_definition;
ALTER TABLE adapter_connections RENAME TO adapter_connections_v15;

CREATE TABLE adapter_connections (
  connection_id TEXT PRIMARY KEY NOT NULL
    CHECK (length(connection_id) = 32 AND connection_id = lower(connection_id)),
  connection_slug TEXT UNIQUE,
  semantic_digest TEXT
    CHECK (semantic_digest IS NULL OR (length(semantic_digest) = 64 AND semantic_digest = lower(semantic_digest))),
  account_id TEXT,
  account_kind TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'authentication_required', 'blocked')),
  connection_revision INTEGER CHECK (connection_revision IS NULL OR connection_revision > 0),
  credential_revision INTEGER CHECK (credential_revision IS NULL OR credential_revision >= 0),
  grant_revision INTEGER CHECK (grant_revision IS NULL OR grant_revision > 0),
  policy_revision INTEGER CHECK (policy_revision IS NULL OR policy_revision > 0),
  credential_generation TEXT
    CHECK (credential_generation IS NULL OR (length(credential_generation) = 32 AND credential_generation = lower(credential_generation))),
  granted_scopes_json TEXT NOT NULL,
  allowed_operations_json TEXT NOT NULL,
  descriptor_relative_path TEXT NOT NULL
    CHECK (descriptor_relative_path GLOB 'adapters/connections/*/connection.json'),
  credential_relative_path TEXT
    CHECK (credential_relative_path IS NULL OR credential_relative_path GLOB 'adapters/connections/*/credentials/*.json'),
  diagnostic_code TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  account_label TEXT
    CHECK (account_label IS NULL OR (length(CAST(account_label AS BLOB)) BETWEEN 1 AND 256 AND trim(account_label) = account_label)),
  CHECK (
    (status = 'blocked' AND connection_slug IS NULL AND semantic_digest IS NULL
      AND account_kind IS NULL AND connection_revision IS NULL AND credential_revision IS NULL
      AND grant_revision IS NULL AND policy_revision IS NULL AND credential_generation IS NULL
      AND credential_relative_path IS NULL AND diagnostic_code IS NOT NULL)
    OR
    (status <> 'blocked' AND connection_slug IS NOT NULL AND semantic_digest IS NOT NULL
      AND account_kind IS NOT NULL AND connection_revision IS NOT NULL AND credential_revision IS NOT NULL
      AND grant_revision IS NOT NULL AND policy_revision IS NOT NULL AND diagnostic_code IS NULL)
  )
);

INSERT INTO adapter_connections (
  connection_id, connection_slug, semantic_digest, account_id, account_kind, status,
  connection_revision, credential_revision, grant_revision, policy_revision,
  credential_generation, granted_scopes_json, allowed_operations_json,
  descriptor_relative_path, credential_relative_path, diagnostic_code, created_at,
  updated_at, account_label
)
SELECT
  connection_id, connection_slug, semantic_digest, account_id, account_kind, status,
  connection_revision, credential_revision, grant_revision, policy_revision,
  credential_generation, granted_scopes_json, allowed_operations_json,
  descriptor_relative_path, credential_relative_path, diagnostic_code, created_at,
  updated_at, account_label
FROM adapter_connections_v15;

DROP TABLE adapter_connections_v15;

CREATE INDEX adapter_connections_definition
ON adapter_connections(semantic_digest, status);
"#;

const MCP_SERVICE_DESCRIPTION_SQL: &str =
    "ALTER TABLE mcp_servers ADD COLUMN service_description TEXT;";

const MCP_DEFINITION_CONNECTION_SQL: &str = r#"
CREATE TABLE mcp_definitions (
  mcp_definition_id TEXT PRIMARY KEY NOT NULL CHECK (mcp_definition_id GLOB 'mcp_definition:*'),
  display_name TEXT NOT NULL CHECK (trim(display_name) <> ''),
  transport_kind TEXT NOT NULL CHECK (transport_kind IN ('stdio', 'streamable_http')),
  safe_config_json TEXT NOT NULL CHECK (json_valid(safe_config_json)),
  definition_revision TEXT NOT NULL UNIQUE CHECK (definition_revision GLOB 'mcp_definition_revision:*'),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

ALTER TABLE mcp_servers ADD COLUMN mcp_definition_id TEXT;
ALTER TABLE mcp_servers ADD COLUMN connection_config_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(connection_config_json));
ALTER TABLE mcp_servers ADD COLUMN connection_label TEXT CHECK (connection_label IS NULL OR trim(connection_label) <> '');

CREATE TEMP TABLE mcp_definition_migration (
  mcp_server_id TEXT PRIMARY KEY NOT NULL,
  mcp_definition_id TEXT NOT NULL,
  definition_revision TEXT NOT NULL
);

INSERT INTO mcp_definition_migration (mcp_server_id, mcp_definition_id, definition_revision)
SELECT
  mcp_server_id,
  'mcp_definition:' || lower(hex(randomblob(16))),
  'mcp_definition_revision:' || lower(hex(randomblob(16)))
FROM mcp_servers;

INSERT INTO mcp_definitions (
  mcp_definition_id, display_name, transport_kind, safe_config_json, definition_revision
)
SELECT
  mapping.mcp_definition_id,
  servers.display_name,
  servers.transport_kind,
  json_remove(servers.safe_config_json, '$.secret_refs', '$.secret_identity_revision'),
  mapping.definition_revision
FROM mcp_servers servers
JOIN mcp_definition_migration mapping USING (mcp_server_id);

UPDATE mcp_servers
SET
  mcp_definition_id = (
    SELECT mapping.mcp_definition_id
    FROM mcp_definition_migration mapping
    WHERE mapping.mcp_server_id = mcp_servers.mcp_server_id
  ),
  connection_config_json = json_object(
    'secret_refs', json_extract(safe_config_json, '$.secret_refs'),
    'secret_identity_revision', json_extract(safe_config_json, '$.secret_identity_revision')
  );

DROP TABLE mcp_definition_migration;

ALTER TABLE mcp_servers DROP COLUMN display_name;
ALTER TABLE mcp_servers DROP COLUMN transport_kind;
ALTER TABLE mcp_servers DROP COLUMN safe_config_json;

CREATE UNIQUE INDEX mcp_servers_definition_connection
ON mcp_servers(mcp_definition_id, mcp_server_id);

CREATE TRIGGER mcp_servers_definition_insert
BEFORE INSERT ON mcp_servers
WHEN NEW.mcp_definition_id IS NULL
  OR NOT EXISTS (
    SELECT 1 FROM mcp_definitions definitions
    WHERE definitions.mcp_definition_id = NEW.mcp_definition_id
  )
BEGIN
  SELECT RAISE(ABORT, 'MCP connection definition is unavailable');
END;

CREATE TRIGGER mcp_servers_definition_update
BEFORE UPDATE OF mcp_definition_id ON mcp_servers
WHEN NEW.mcp_definition_id IS NULL
  OR NOT EXISTS (
    SELECT 1 FROM mcp_definitions definitions
    WHERE definitions.mcp_definition_id = NEW.mcp_definition_id
  )
BEGIN
  SELECT RAISE(ABORT, 'MCP connection definition is unavailable');
END;
"#;

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

const CAPABILITY_AUTH_REQUESTS_SQL: &str = r#"
UPDATE agent_runs
SET status = 'failed',
    error_code = 'authentication_request_schema_replaced',
    error_message = 'A pending capability authentication request could not be recovered.',
    ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    lease_owner = NULL,
    lease_token = NULL,
    lease_expires_at = NULL,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE status = 'waiting_for_approval'
  AND run_id IN (
    SELECT run_id FROM mcp_auth_requests
    WHERE run_id IS NOT NULL
      AND state IN ('awaiting_user', 'authorizing', 'resuming')
  );

UPDATE governed_actions
SET state = 'outcome_uncertain',
    authentication_pending = 0,
    failure_code = 'authentication_request_schema_replaced',
    completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE authentication_pending = 1;

DROP TABLE mcp_auth_requests;

CREATE TABLE capability_auth_requests (
  request_id TEXT PRIMARY KEY NOT NULL CHECK (request_id GLOB 'cap_auth:*'),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
  owner_human_id TEXT NOT NULL,
  conversation_id TEXT,
  turn_id TEXT,
  task_id TEXT,
  run_id TEXT,
  task_generation INTEGER CHECK (task_generation IS NULL OR task_generation > 0),
  requesting_agent_id TEXT NOT NULL CHECK (trim(requesting_agent_id) <> ''),
  mcp_server_id TEXT,
  adapter_connection_id TEXT,
  challenge_kind TEXT NOT NULL CHECK (challenge_kind IN ('reauthenticate', 'replace_credential')),
  authority_revision TEXT NOT NULL CHECK (trim(authority_revision) <> ''),
  capability_name TEXT NOT NULL CHECK (trim(capability_name) <> ''),
  operation_token TEXT NOT NULL CHECK (trim(operation_token) <> ''),
  input_schema_json TEXT NOT NULL CHECK (json_valid(input_schema_json)),
  protected_arguments_ref TEXT NOT NULL CHECK (
    length(protected_arguments_ref) = 32
    AND protected_arguments_ref = lower(protected_arguments_ref)
    AND protected_arguments_ref NOT GLOB '*[^0-9a-f]*'
  ),
  arguments_sha256 TEXT NOT NULL CHECK (
    length(arguments_sha256) = 64
    AND arguments_sha256 = lower(arguments_sha256)
    AND arguments_sha256 NOT GLOB '*[^0-9a-f]*'
  ),
  provider_selection_digest TEXT NOT NULL CHECK (
    length(provider_selection_digest) = 64
    AND provider_selection_digest = lower(provider_selection_digest)
    AND provider_selection_digest NOT GLOB '*[^0-9a-f]*'
  ),
  output_index INTEGER NOT NULL CHECK (output_index >= 0),
  call_id TEXT,
  provider_call_id TEXT,
  provider_name TEXT,
  governed_action_id TEXT,
  governed_action_revision INTEGER,
  result_context_json TEXT NOT NULL CHECK (json_valid(result_context_json)),
  authentication_attempt_id TEXT,
  state TEXT NOT NULL CHECK (state IN (
    'awaiting_user', 'authorizing', 'resuming', 'completed', 'cancelled', 'superseded'
  )),
  output_json TEXT CHECK (output_json IS NULL OR json_valid(output_json)),
  failure_code TEXT,
  supersession_reason TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT,
  origin_resumed_at TEXT,
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT,
  FOREIGN KEY (governed_action_id, governed_action_revision)
    REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,
  CHECK ((mcp_server_id IS NOT NULL AND trim(mcp_server_id) <> '' AND adapter_connection_id IS NULL)
      OR (mcp_server_id IS NULL AND adapter_connection_id IS NOT NULL AND trim(adapter_connection_id) <> '')),
  CHECK ((conversation_id IS NOT NULL AND turn_id IS NOT NULL AND task_id IS NULL AND run_id IS NULL AND task_generation IS NULL)
      OR (conversation_id IS NULL AND turn_id IS NULL AND task_id IS NOT NULL AND run_id IS NOT NULL AND task_generation IS NOT NULL)),
  CHECK ((governed_action_id IS NULL) = (governed_action_revision IS NULL))
);

CREATE UNIQUE INDEX capability_auth_requests_conversation_call
ON capability_auth_requests(conversation_id, turn_id, output_index)
WHERE conversation_id IS NOT NULL AND governed_action_id IS NULL;

CREATE UNIQUE INDEX capability_auth_requests_run_call
ON capability_auth_requests(run_id, output_index)
WHERE run_id IS NOT NULL AND governed_action_id IS NULL;

CREATE UNIQUE INDEX capability_auth_requests_governed_action
ON capability_auth_requests(governed_action_id, governed_action_revision)
WHERE governed_action_id IS NOT NULL;

CREATE INDEX capability_auth_requests_attention
ON capability_auth_requests(owner_human_id, state, created_at, request_id)
WHERE state IN ('awaiting_user', 'authorizing');

CREATE INDEX capability_auth_requests_attempt
ON capability_auth_requests(authentication_attempt_id, state)
WHERE authentication_attempt_id IS NOT NULL;

CREATE TRIGGER capability_auth_requests_active_mcp_insert
BEFORE INSERT ON capability_auth_requests
WHEN NEW.mcp_server_id IS NOT NULL
 AND NOT EXISTS (SELECT 1 FROM mcp_servers WHERE mcp_server_id = NEW.mcp_server_id)
BEGIN
  SELECT RAISE(ABORT, 'active MCP authentication authority is unavailable');
END;

CREATE TRIGGER capability_auth_requests_active_mcp_update
BEFORE UPDATE OF mcp_server_id, state ON capability_auth_requests
WHEN NEW.mcp_server_id IS NOT NULL
 AND NEW.state IN ('awaiting_user', 'authorizing', 'resuming')
 AND NOT EXISTS (SELECT 1 FROM mcp_servers WHERE mcp_server_id = NEW.mcp_server_id)
BEGIN
  SELECT RAISE(ABORT, 'active MCP authentication authority is unavailable');
END;

CREATE TRIGGER mcp_servers_active_capability_auth_delete
BEFORE DELETE ON mcp_servers
WHEN EXISTS (
  SELECT 1 FROM capability_auth_requests
  WHERE mcp_server_id = OLD.mcp_server_id
    AND state IN ('awaiting_user', 'authorizing', 'resuming')
)
BEGIN
  SELECT RAISE(ABORT, 'active capability authentication must be terminalized before deletion');
END;
"#;

// Some development builds published the v8 capability-auth table with the
// old MCP foreign key and without origin_resumed_at. Keep the repair append-only
// so those already-versioned databases can migrate without losing rows.
const CAPABILITY_AUTH_REQUESTS_DRIFT_REPAIR_SQL: &str = r#"
DROP TRIGGER IF EXISTS capability_auth_requests_active_mcp_insert;
DROP TRIGGER IF EXISTS capability_auth_requests_active_mcp_update;
DROP TRIGGER IF EXISTS mcp_servers_active_capability_auth_delete;
DROP INDEX capability_auth_requests_conversation_call;
DROP INDEX capability_auth_requests_run_call;
DROP INDEX capability_auth_requests_governed_action;
DROP INDEX capability_auth_requests_attention;
DROP INDEX capability_auth_requests_attempt;
ALTER TABLE capability_auth_requests RENAME TO capability_auth_requests_drift;

CREATE TABLE capability_auth_requests (
  request_id TEXT PRIMARY KEY NOT NULL CHECK (request_id GLOB 'cap_auth:*'),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
  owner_human_id TEXT NOT NULL,
  conversation_id TEXT,
  turn_id TEXT,
  task_id TEXT,
  run_id TEXT,
  task_generation INTEGER CHECK (task_generation IS NULL OR task_generation > 0),
  requesting_agent_id TEXT NOT NULL CHECK (trim(requesting_agent_id) <> ''),
  mcp_server_id TEXT,
  adapter_connection_id TEXT,
  challenge_kind TEXT NOT NULL CHECK (challenge_kind IN ('reauthenticate', 'replace_credential')),
  authority_revision TEXT NOT NULL CHECK (trim(authority_revision) <> ''),
  capability_name TEXT NOT NULL CHECK (trim(capability_name) <> ''),
  operation_token TEXT NOT NULL CHECK (trim(operation_token) <> ''),
  input_schema_json TEXT NOT NULL CHECK (json_valid(input_schema_json)),
  protected_arguments_ref TEXT NOT NULL CHECK (
    length(protected_arguments_ref) = 32
    AND protected_arguments_ref = lower(protected_arguments_ref)
    AND protected_arguments_ref NOT GLOB '*[^0-9a-f]*'
  ),
  arguments_sha256 TEXT NOT NULL CHECK (
    length(arguments_sha256) = 64
    AND arguments_sha256 = lower(arguments_sha256)
    AND arguments_sha256 NOT GLOB '*[^0-9a-f]*'
  ),
  provider_selection_digest TEXT NOT NULL CHECK (
    length(provider_selection_digest) = 64
    AND provider_selection_digest = lower(provider_selection_digest)
    AND provider_selection_digest NOT GLOB '*[^0-9a-f]*'
  ),
  output_index INTEGER NOT NULL CHECK (output_index >= 0),
  call_id TEXT,
  provider_call_id TEXT,
  provider_name TEXT,
  governed_action_id TEXT,
  governed_action_revision INTEGER,
  result_context_json TEXT NOT NULL CHECK (json_valid(result_context_json)),
  authentication_attempt_id TEXT,
  state TEXT NOT NULL CHECK (state IN (
    'awaiting_user', 'authorizing', 'resuming', 'completed', 'cancelled', 'superseded'
  )),
  output_json TEXT CHECK (output_json IS NULL OR json_valid(output_json)),
  failure_code TEXT,
  supersession_reason TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT,
  origin_resumed_at TEXT,
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT,
  FOREIGN KEY (governed_action_id, governed_action_revision)
    REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,
  CHECK ((mcp_server_id IS NOT NULL AND trim(mcp_server_id) <> '' AND adapter_connection_id IS NULL)
      OR (mcp_server_id IS NULL AND adapter_connection_id IS NOT NULL AND trim(adapter_connection_id) <> '')),
  CHECK ((conversation_id IS NOT NULL AND turn_id IS NOT NULL AND task_id IS NULL AND run_id IS NULL AND task_generation IS NULL)
      OR (conversation_id IS NULL AND turn_id IS NULL AND task_id IS NOT NULL AND run_id IS NOT NULL AND task_generation IS NOT NULL)),
  CHECK ((governed_action_id IS NULL) = (governed_action_revision IS NULL))
);

INSERT INTO capability_auth_requests (
  request_id, revision, owner_human_id, conversation_id, turn_id, task_id, run_id,
  task_generation, requesting_agent_id, mcp_server_id, adapter_connection_id,
  challenge_kind, authority_revision, capability_name, operation_token,
  input_schema_json, protected_arguments_ref, arguments_sha256,
  provider_selection_digest, output_index, call_id, provider_call_id,
  provider_name, governed_action_id, governed_action_revision, result_context_json,
  authentication_attempt_id, state, output_json, failure_code, supersession_reason,
  created_at, updated_at, completed_at, origin_resumed_at
)
SELECT
  request_id, revision, owner_human_id, conversation_id, turn_id, task_id, run_id,
  task_generation, requesting_agent_id, mcp_server_id, adapter_connection_id,
  challenge_kind, authority_revision, capability_name, operation_token,
  input_schema_json, protected_arguments_ref, arguments_sha256,
  provider_selection_digest, output_index, call_id, provider_call_id,
  provider_name, governed_action_id, governed_action_revision, result_context_json,
  authentication_attempt_id, state, output_json, failure_code, supersession_reason,
  created_at, updated_at, completed_at, NULL
FROM capability_auth_requests_drift;

DROP TABLE capability_auth_requests_drift;

CREATE UNIQUE INDEX capability_auth_requests_conversation_call
ON capability_auth_requests(conversation_id, turn_id, output_index)
WHERE conversation_id IS NOT NULL AND governed_action_id IS NULL;

CREATE UNIQUE INDEX capability_auth_requests_run_call
ON capability_auth_requests(run_id, output_index)
WHERE run_id IS NOT NULL AND governed_action_id IS NULL;

CREATE UNIQUE INDEX capability_auth_requests_governed_action
ON capability_auth_requests(governed_action_id, governed_action_revision)
WHERE governed_action_id IS NOT NULL;

CREATE INDEX capability_auth_requests_attention
ON capability_auth_requests(owner_human_id, state, created_at, request_id)
WHERE state IN ('awaiting_user', 'authorizing');

CREATE INDEX capability_auth_requests_attempt
ON capability_auth_requests(authentication_attempt_id, state)
WHERE authentication_attempt_id IS NOT NULL;

CREATE TRIGGER capability_auth_requests_active_mcp_insert
BEFORE INSERT ON capability_auth_requests
WHEN NEW.mcp_server_id IS NOT NULL
 AND NOT EXISTS (SELECT 1 FROM mcp_servers WHERE mcp_server_id = NEW.mcp_server_id)
BEGIN
  SELECT RAISE(ABORT, 'active MCP authentication authority is unavailable');
END;

CREATE TRIGGER capability_auth_requests_active_mcp_update
BEFORE UPDATE OF mcp_server_id, state ON capability_auth_requests
WHEN NEW.mcp_server_id IS NOT NULL
 AND NEW.state IN ('awaiting_user', 'authorizing', 'resuming')
 AND NOT EXISTS (SELECT 1 FROM mcp_servers WHERE mcp_server_id = NEW.mcp_server_id)
BEGIN
  SELECT RAISE(ABORT, 'active MCP authentication authority is unavailable');
END;

CREATE TRIGGER mcp_servers_active_capability_auth_delete
BEFORE DELETE ON mcp_servers
WHEN EXISTS (
  SELECT 1 FROM capability_auth_requests
  WHERE mcp_server_id = OLD.mcp_server_id
    AND state IN ('awaiting_user', 'authorizing', 'resuming')
)
BEGIN
  SELECT RAISE(ABORT, 'active capability authentication must be terminalized before deletion');
END;
"#;

const REVIEWED_ACTION_POLICY_SQL: &str = r#"
DROP TRIGGER IF EXISTS capability_auth_requests_active_mcp_insert;
DROP TRIGGER IF EXISTS capability_auth_requests_active_mcp_update;
DROP TRIGGER IF EXISTS mcp_servers_active_capability_auth_delete;

INSERT INTO governed_action_events (
  event_id, action_id, action_revision, event_kind, actor_id, safe_payload_json
)
SELECT 'action_event:migration:' || action_id, action_id, revision, 'superseded',
       'system:schema_migration', '{"reason":"tool_policy_schema_replaced"}'
FROM governed_actions
WHERE state IN ('proposed', 'awaiting_approval', 'executable', 'executing');

UPDATE governed_action_approvals
SET state = 'superseded'
WHERE state IN ('pending', 'approved');

UPDATE governed_actions
SET state = 'superseded', authentication_pending = 0,
    failure_code = 'tool_policy_schema_replaced',
    completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE state IN ('proposed', 'awaiting_approval', 'executable', 'executing');

CREATE TEMP TABLE governed_action_assessments_backup AS SELECT * FROM governed_action_assessments;
CREATE TEMP TABLE governed_action_approvals_backup AS SELECT * FROM governed_action_approvals;
CREATE TEMP TABLE governed_action_events_backup AS SELECT * FROM governed_action_events;
CREATE TEMP TABLE capability_auth_requests_backup AS SELECT * FROM capability_auth_requests;

DROP TABLE capability_auth_requests;
DROP TABLE governed_action_events;
DROP TABLE governed_action_approvals;
DROP TABLE governed_action_assessments;

ALTER TABLE governed_actions RENAME TO governed_actions_effect_legacy;

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
  review_route TEXT NOT NULL CHECK (review_route IN ('human_review', 'llm_review')),
  read_only INTEGER CHECK (read_only IS NULL OR read_only IN (0, 1)),
  idempotent INTEGER CHECK (idempotent IS NULL OR idempotent IN (0, 1)),
  destructive INTEGER CHECK (destructive IS NULL OR destructive IN (0, 1)),
  open_world INTEGER CHECK (open_world IS NULL OR open_world IN (0, 1)),
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
  authentication_pending INTEGER NOT NULL DEFAULT 0 CHECK (authentication_pending IN (0, 1)),
  PRIMARY KEY (action_id, revision),
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT,
  CHECK ((task_id IS NULL AND run_id IS NULL) OR (task_id IS NOT NULL AND run_id IS NOT NULL)),
  CHECK ((read_only IS NULL AND idempotent IS NULL AND destructive IS NULL AND open_world IS NULL)
      OR (read_only IS NOT NULL AND idempotent IS NOT NULL AND destructive IS NOT NULL AND open_world IS NOT NULL))
);

INSERT INTO governed_actions (
  action_id, revision, owner_human_id, conversation_id, turn_id, task_id, run_id,
  requesting_agent_id, capability_name, operation_token, review_route,
  arguments_json, arguments_sha256, input_schema_json, authorization_context_json,
  safe_summary, state, output_json, failure_code, created_at, updated_at, completed_at,
  authentication_pending
)
SELECT action_id, revision, owner_human_id, conversation_id, turn_id, task_id, run_id,
       requesting_agent_id, capability_name, operation_token,
       CASE json_extract(authorization_context_json, '$.admission_policy')
         WHEN 'always_ask' THEN 'human_review'
         ELSE 'llm_review'
       END,
       arguments_json, arguments_sha256, input_schema_json, authorization_context_json,
       safe_summary, state, output_json, failure_code, created_at, updated_at, completed_at,
       authentication_pending
FROM governed_actions_effect_legacy;

DROP TABLE governed_actions_effect_legacy;

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
  action_id TEXT NOT NULL, action_revision INTEGER NOT NULL CHECK (action_revision >= 1),
  status TEXT NOT NULL CHECK (status IN ('completed', 'reviewer_unavailable', 'invalid_response')),
  reviewer_selection_json TEXT CHECK (reviewer_selection_json IS NULL OR json_valid(reviewer_selection_json)),
  authorization TEXT CHECK (authorization IS NULL OR authorization IN ('explicit', 'substantive', 'weak', 'absent')),
  risk TEXT CHECK (risk IS NULL OR risk IN ('low', 'medium', 'high', 'critical')),
  recommendation TEXT NOT NULL CHECK (recommendation IN ('auto_execute', 'require_approval')),
  reason_codes_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(reason_codes_json)),
  explanation TEXT NOT NULL CHECK (trim(explanation) <> ''),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (action_id, action_revision),
  FOREIGN KEY (action_id, action_revision) REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,
  CHECK (status = 'completed' OR recommendation = 'require_approval'),
  CHECK ((status = 'completed' AND reviewer_selection_json IS NOT NULL AND authorization IS NOT NULL AND risk IS NOT NULL)
    OR (status <> 'completed' AND authorization IS NULL AND risk IS NULL))
);
INSERT INTO governed_action_assessments SELECT * FROM governed_action_assessments_backup;

CREATE TABLE governed_action_approvals (
  action_id TEXT NOT NULL, action_revision INTEGER NOT NULL CHECK (action_revision >= 1),
  state TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'declined', 'consumed', 'revoked', 'superseded')),
  decided_by_human_id TEXT, decided_at TEXT, consumed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (action_id, action_revision),
  FOREIGN KEY (action_id, action_revision) REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,
  FOREIGN KEY (decided_by_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  CHECK ((state = 'pending' AND decided_by_human_id IS NULL AND decided_at IS NULL AND consumed_at IS NULL)
    OR (state IN ('approved', 'declined', 'revoked') AND decided_by_human_id IS NOT NULL AND decided_at IS NOT NULL AND consumed_at IS NULL)
    OR (state = 'consumed' AND decided_by_human_id IS NOT NULL AND decided_at IS NOT NULL AND consumed_at IS NOT NULL)
    OR (state = 'superseded' AND consumed_at IS NULL))
);
INSERT INTO governed_action_approvals SELECT * FROM governed_action_approvals_backup;

CREATE TABLE governed_action_events (
  event_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE CHECK (event_id GLOB 'action_event:*'),
  action_id TEXT NOT NULL, action_revision INTEGER NOT NULL CHECK (action_revision >= 1),
  event_kind TEXT NOT NULL CHECK (event_kind IN (
    'proposed', 'reviewed', 'approval_requested', 'approved', 'declined',
    'execution_started', 'succeeded', 'failed', 'outcome_uncertain', 'superseded', 'cancelled'
  )),
  actor_id TEXT NOT NULL CHECK (trim(actor_id) <> ''),
  safe_payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(safe_payload_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (action_id, action_revision) REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT
);
INSERT INTO governed_action_events SELECT * FROM governed_action_events_backup;
CREATE INDEX governed_action_events_action
ON governed_action_events(action_id, action_revision, event_sequence);

CREATE TABLE capability_auth_requests (
  request_id TEXT PRIMARY KEY NOT NULL CHECK (request_id GLOB 'cap_auth:*'),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0), owner_human_id TEXT NOT NULL,
  conversation_id TEXT, turn_id TEXT, task_id TEXT, run_id TEXT,
  task_generation INTEGER CHECK (task_generation IS NULL OR task_generation > 0),
  requesting_agent_id TEXT NOT NULL CHECK (trim(requesting_agent_id) <> ''),
  mcp_server_id TEXT, adapter_connection_id TEXT,
  challenge_kind TEXT NOT NULL CHECK (challenge_kind IN ('reauthenticate', 'replace_credential')),
  authority_revision TEXT NOT NULL CHECK (trim(authority_revision) <> ''),
  capability_name TEXT NOT NULL CHECK (trim(capability_name) <> ''),
  operation_token TEXT NOT NULL CHECK (trim(operation_token) <> ''),
  input_schema_json TEXT NOT NULL CHECK (json_valid(input_schema_json)),
  protected_arguments_ref TEXT NOT NULL CHECK (length(protected_arguments_ref) = 32 AND protected_arguments_ref = lower(protected_arguments_ref) AND protected_arguments_ref NOT GLOB '*[^0-9a-f]*'),
  arguments_sha256 TEXT NOT NULL CHECK (length(arguments_sha256) = 64 AND arguments_sha256 = lower(arguments_sha256) AND arguments_sha256 NOT GLOB '*[^0-9a-f]*'),
  provider_selection_digest TEXT NOT NULL CHECK (length(provider_selection_digest) = 64 AND provider_selection_digest = lower(provider_selection_digest) AND provider_selection_digest NOT GLOB '*[^0-9a-f]*'),
  output_index INTEGER NOT NULL CHECK (output_index >= 0), call_id TEXT, provider_call_id TEXT,
  provider_name TEXT, governed_action_id TEXT, governed_action_revision INTEGER,
  result_context_json TEXT NOT NULL CHECK (json_valid(result_context_json)),
  authentication_attempt_id TEXT,
  state TEXT NOT NULL CHECK (state IN ('awaiting_user', 'authorizing', 'resuming', 'completed', 'cancelled', 'superseded')),
  output_json TEXT CHECK (output_json IS NULL OR json_valid(output_json)), failure_code TEXT,
  supersession_reason TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT, origin_resumed_at TEXT,
  FOREIGN KEY (owner_human_id) REFERENCES humans(human_id) ON DELETE RESTRICT,
  FOREIGN KEY (conversation_id) REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE RESTRICT,
  FOREIGN KEY (run_id) REFERENCES agent_runs(run_id) ON DELETE RESTRICT,
  FOREIGN KEY (governed_action_id, governed_action_revision) REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,
  CHECK ((mcp_server_id IS NOT NULL AND trim(mcp_server_id) <> '' AND adapter_connection_id IS NULL)
      OR (mcp_server_id IS NULL AND adapter_connection_id IS NOT NULL AND trim(adapter_connection_id) <> '')),
  CHECK ((conversation_id IS NOT NULL AND turn_id IS NOT NULL AND task_id IS NULL AND run_id IS NULL AND task_generation IS NULL)
      OR (conversation_id IS NULL AND turn_id IS NULL AND task_id IS NOT NULL AND run_id IS NOT NULL AND task_generation IS NOT NULL)),
  CHECK ((governed_action_id IS NULL) = (governed_action_revision IS NULL))
);
INSERT INTO capability_auth_requests SELECT * FROM capability_auth_requests_backup;

DROP TABLE governed_action_assessments_backup;
DROP TABLE governed_action_approvals_backup;
DROP TABLE governed_action_events_backup;
DROP TABLE capability_auth_requests_backup;

CREATE UNIQUE INDEX capability_auth_requests_conversation_call ON capability_auth_requests(conversation_id, turn_id, output_index) WHERE conversation_id IS NOT NULL AND governed_action_id IS NULL;
CREATE UNIQUE INDEX capability_auth_requests_run_call ON capability_auth_requests(run_id, output_index) WHERE run_id IS NOT NULL AND governed_action_id IS NULL;
CREATE UNIQUE INDEX capability_auth_requests_governed_action ON capability_auth_requests(governed_action_id, governed_action_revision) WHERE governed_action_id IS NOT NULL;
CREATE INDEX capability_auth_requests_attention ON capability_auth_requests(owner_human_id, state, created_at, request_id) WHERE state IN ('awaiting_user', 'authorizing');
CREATE INDEX capability_auth_requests_attempt ON capability_auth_requests(authentication_attempt_id, state) WHERE authentication_attempt_id IS NOT NULL;

CREATE TRIGGER capability_auth_requests_active_mcp_insert BEFORE INSERT ON capability_auth_requests
WHEN NEW.mcp_server_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM mcp_servers WHERE mcp_server_id = NEW.mcp_server_id)
BEGIN SELECT RAISE(ABORT, 'active MCP authentication authority is unavailable'); END;
CREATE TRIGGER capability_auth_requests_active_mcp_update BEFORE UPDATE OF mcp_server_id, state ON capability_auth_requests
WHEN NEW.mcp_server_id IS NOT NULL AND NEW.state IN ('awaiting_user', 'authorizing', 'resuming')
 AND NOT EXISTS (SELECT 1 FROM mcp_servers WHERE mcp_server_id = NEW.mcp_server_id)
BEGIN SELECT RAISE(ABORT, 'active MCP authentication authority is unavailable'); END;
CREATE TRIGGER mcp_servers_active_capability_auth_delete BEFORE DELETE ON mcp_servers
WHEN EXISTS (SELECT 1 FROM capability_auth_requests WHERE mcp_server_id = OLD.mcp_server_id AND state IN ('awaiting_user', 'authorizing', 'resuming'))
BEGIN SELECT RAISE(ABORT, 'active capability authentication must be terminalized before deletion'); END;
"#;

const ADAPTER_DEFINITIONS_SQL: &str = r#"
CREATE TABLE adapter_definitions (
  semantic_digest TEXT PRIMARY KEY NOT NULL
    CHECK (length(semantic_digest) = 64 AND semantic_digest = lower(semantic_digest)),
  definition_id TEXT,
  adapter_id TEXT,
  source_digest TEXT
    CHECK (source_digest IS NULL OR (length(source_digest) = 64 AND source_digest = lower(source_digest))),
  manifest_relative_path TEXT NOT NULL CHECK (manifest_relative_path GLOB 'adapters/definitions/*/manifest.json'),
  provenance_relative_path TEXT NOT NULL CHECK (provenance_relative_path GLOB 'adapters/definitions/*/provenance.json'),
  compile_status TEXT NOT NULL CHECK (compile_status IN ('compiled', 'blocked')),
  review_status TEXT NOT NULL CHECK (review_status IN ('reviewed', 'pending', 'unknown')),
  diagnostic_code TEXT,
  operation_count INTEGER NOT NULL CHECK (operation_count >= 0),
  compiler_version TEXT NOT NULL CHECK (trim(compiler_version) <> ''),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (
    (compile_status = 'compiled' AND definition_id IS NOT NULL AND adapter_id IS NOT NULL
      AND diagnostic_code IS NULL AND review_status IN ('reviewed', 'pending'))
    OR
    (compile_status = 'blocked' AND definition_id IS NULL AND adapter_id IS NULL
      AND diagnostic_code IS NOT NULL AND review_status = 'unknown' AND operation_count = 0)
  )
);

CREATE INDEX adapter_definitions_adapter
ON adapter_definitions(adapter_id, compile_status, review_status);

CREATE INDEX adapter_definitions_identity
ON adapter_definitions(definition_id, semantic_digest);
"#;

const ADAPTER_CONNECTIONS_SQL: &str = r#"
CREATE TABLE adapter_connections (
  connection_id TEXT PRIMARY KEY NOT NULL
    CHECK (length(connection_id) = 32 AND connection_id = lower(connection_id)),
  connection_slug TEXT UNIQUE,
  semantic_digest TEXT
    CHECK (semantic_digest IS NULL OR (length(semantic_digest) = 64 AND semantic_digest = lower(semantic_digest))),
  account_id TEXT,
  account_kind TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'authentication_required', 'blocked')),
  connection_revision INTEGER CHECK (connection_revision IS NULL OR connection_revision > 0),
  credential_revision INTEGER CHECK (credential_revision IS NULL OR credential_revision >= 0),
  grant_revision INTEGER CHECK (grant_revision IS NULL OR grant_revision > 0),
  policy_revision INTEGER CHECK (policy_revision IS NULL OR policy_revision > 0),
  credential_generation TEXT
    CHECK (credential_generation IS NULL OR (length(credential_generation) = 32 AND credential_generation = lower(credential_generation))),
  granted_scopes_json TEXT NOT NULL,
  allowed_operations_json TEXT NOT NULL,
  descriptor_relative_path TEXT NOT NULL
    CHECK (descriptor_relative_path GLOB 'adapters/connections/*/connection.json'),
  credential_relative_path TEXT
    CHECK (credential_relative_path IS NULL OR credential_relative_path GLOB 'adapters/connections/*/credentials/*.json'),
  diagnostic_code TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (
    (status = 'blocked' AND connection_slug IS NULL AND semantic_digest IS NULL
      AND account_kind IS NULL AND connection_revision IS NULL AND credential_revision IS NULL
      AND grant_revision IS NULL AND policy_revision IS NULL AND credential_generation IS NULL
      AND credential_relative_path IS NULL AND diagnostic_code IS NOT NULL)
    OR
    (status <> 'blocked' AND connection_slug IS NOT NULL AND semantic_digest IS NOT NULL
      AND account_kind IS NOT NULL AND connection_revision IS NOT NULL AND credential_revision IS NOT NULL
      AND grant_revision IS NOT NULL AND policy_revision IS NOT NULL AND diagnostic_code IS NULL)
  )
);

CREATE INDEX adapter_connections_definition
ON adapter_connections(semantic_digest, status);
"#;

/// Shared notification projection state, paired-client registrations, and bounded APNs delivery state.
const APNS_NOTIFICATIONS_SQL: &str = r#"
CREATE TABLE notification_projection_state (
  state_id INTEGER PRIMARY KEY NOT NULL CHECK (state_id = 1),
  primary_conversation_id TEXT,
  primary_sequence INTEGER NOT NULL DEFAULT 0 CHECK (primary_sequence >= 0),
  attention_seeded INTEGER NOT NULL DEFAULT 0 CHECK (attention_seeded IN (0, 1)),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (primary_conversation_id) REFERENCES conversations(conversation_id) ON DELETE SET NULL
);
INSERT INTO notification_projection_state (state_id, primary_conversation_id, primary_sequence, attention_seeded, updated_at)
SELECT 1, primary_conversation_id, primary_sequence, attention_seeded, updated_at
FROM web_push_identity WHERE identity_id = 1
UNION ALL
SELECT 1, NULL, 0, 0, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE NOT EXISTS (SELECT 1 FROM web_push_identity WHERE identity_id = 1);
ALTER TABLE web_push_attention_seen RENAME TO notification_attention_seen;
CREATE TABLE web_push_identity_v34 (
  identity_id INTEGER PRIMARY KEY NOT NULL CHECK (identity_id = 1),
  private_key BLOB NOT NULL CHECK (length(private_key) = 32),
  public_key BLOB NOT NULL CHECK (length(public_key) = 65),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO web_push_identity_v34 (identity_id, private_key, public_key, created_at, updated_at)
SELECT identity_id, private_key, public_key, created_at, updated_at FROM web_push_identity;
DROP TABLE web_push_identity;
ALTER TABLE web_push_identity_v34 RENAME TO web_push_identity;
CREATE TABLE client_notification_registrations (
  client_id TEXT PRIMARY KEY NOT NULL,
  device_token BLOB NOT NULL CHECK (length(device_token) BETWEEN 1 AND 1024),
  environment TEXT NOT NULL CHECK (environment IN ('development', 'production')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (client_id) REFERENCES clients(client_id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX client_notification_registrations_token
ON client_notification_registrations(environment, device_token);
CREATE TABLE apns_deliveries (
  client_id TEXT NOT NULL,
  event_key TEXT NOT NULL CHECK (trim(event_key) <> '' AND length(event_key) <= 256),
  title TEXT NOT NULL CHECK (trim(title) <> '' AND length(title) <= 256),
  body TEXT NOT NULL CHECK (length(body) <= 2048),
  urgency TEXT NOT NULL CHECK (urgency IN ('normal', 'high')),
  ttl_seconds INTEGER NOT NULL CHECK (ttl_seconds BETWEEN 0 AND 604800),
  status TEXT NOT NULL CHECK (status IN ('pending', 'delivered', 'suppressed', 'failed')),
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 4),
  available_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  last_error_code TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (client_id, event_key),
  FOREIGN KEY (client_id) REFERENCES client_notification_registrations(client_id) ON DELETE CASCADE
);
CREATE INDEX apns_deliveries_due
ON apns_deliveries(status, available_at, client_id, event_key)
WHERE status = 'pending';
"#;
