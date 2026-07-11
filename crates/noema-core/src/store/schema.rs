/// Current schema version for pre-stable local SQLite data.
pub const STORE_SCHEMA_VERSION: i64 = 1;

/// SQLite bootstrap used by the Noema store.
pub const STORE_SCHEMA_SQL: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS schema_state (
  name TEXT PRIMARY KEY NOT NULL,
  version INTEGER NOT NULL CHECK (version >= 1),
  applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO schema_state (name, version, applied_at)
VALUES ('sqlite_store_v1', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(name) DO UPDATE SET
  version = excluded.version,
  applied_at = excluded.applied_at;

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
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local')),
  provider_account_id TEXT NOT NULL,
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS auxiliary_model_preferences (
  task_id TEXT PRIMARY KEY NOT NULL CHECK (task_id IN ('web_fetch_summarizer', 'tool_progress_audit', 'memory_extraction')),
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local')),
  provider_account_id TEXT NOT NULL,
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS provider_accounts (
  provider_account_id TEXT PRIMARY KEY NOT NULL,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'exa')),
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

CREATE TABLE IF NOT EXISTS conversations (
  conversation_id TEXT PRIMARY KEY NOT NULL,
  title TEXT,
  owner_object_type TEXT NOT NULL CHECK (owner_object_type IN ('human', 'agent', 'conversation', 'workspace', 'project', 'task', 'tool')),
  owner_object_id TEXT NOT NULL,
  primary_human_id TEXT,
  primary_agent_id TEXT,
  provider TEXT NOT NULL CHECK (provider IN ('codex', 'openai', 'foundation_local')),
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
  kind TEXT NOT NULL CHECK (kind IN ('user_text', 'assistant_text', 'activity', 'a2ui_card', 'multiple_choice_prompt', 'multiple_choice_selection', 'tool_call', 'tool_result', 'reasoning', 'approval_request', 'approval_result', 'error_notice', 'artifact_reference', 'task_reference')),
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
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local')),
  model_profile TEXT,
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
  provider_kind TEXT CHECK (provider_kind IS NULL OR provider_kind IN ('codex', 'openai', 'foundation_local')),
  model_profile TEXT,
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (mode = 'managed' OR base_url IS NOT NULL)
);

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

-- Background task orchestration is intentionally concrete in this first slice.
-- Projection rows are authoritative current state; event rows below provide the
-- append-only explanation used by the worker and inspection surfaces.
CREATE TABLE IF NOT EXISTS task_model_pool_entries (
  pool_entry_id TEXT PRIMARY KEY NOT NULL,
  complexity TEXT NOT NULL CHECK (complexity IN ('simple', 'medium', 'difficult')),
  label TEXT,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local')),
  provider_account_id TEXT NOT NULL,
  model_profile TEXT NOT NULL CHECK (model_profile <> ''),
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(complexity, provider_account_id, model_profile, reasoning_effort)
);

CREATE INDEX IF NOT EXISTS task_model_pool_entries_selection
ON task_model_pool_entries(complexity, enabled, sort_order, label, pool_entry_id);
CREATE UNIQUE INDEX IF NOT EXISTS task_model_pool_entries_unique_selection
ON task_model_pool_entries(
  complexity,
  provider_account_id,
  model_profile,
  COALESCE(reasoning_effort, '')
);

CREATE TABLE IF NOT EXISTS task_execution_policy (
  policy_id TEXT PRIMARY KEY NOT NULL CHECK (policy_id = 'default'),
  max_provider_continuations INTEGER NOT NULL CHECK (max_provider_continuations BETWEEN 1 AND 1000),
  max_tool_calls INTEGER NOT NULL CHECK (max_tool_calls BETWEEN 1 AND 10000),
  max_active_minutes INTEGER NOT NULL CHECK (max_active_minutes BETWEEN 1 AND 10080),
  progress_audit_interval INTEGER NOT NULL CHECK (
    progress_audit_interval > 0
    AND progress_audit_interval <= max_provider_continuations
  ),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO task_execution_policy (
  policy_id, max_provider_continuations, max_tool_calls,
  max_active_minutes, progress_audit_interval
) VALUES ('default', 80, 400, 120, 20)
ON CONFLICT(policy_id) DO NOTHING;

CREATE TABLE IF NOT EXISTS tasks (
  task_id TEXT PRIMARY KEY NOT NULL,
  title TEXT NOT NULL CHECK (title <> ''),
  request_markdown TEXT NOT NULL CHECK (request_markdown <> ''),
  complexity TEXT NOT NULL CHECK (complexity IN ('simple', 'medium', 'difficult')),
  status TEXT NOT NULL CHECK (status IN ('queued', 'executing', 'reviewing', 'revision_requested', 'waiting_for_human', 'completed', 'failed', 'cancelled')),
  owner_human_id TEXT NOT NULL,
  source_conversation_id TEXT,
  source_turn_id TEXT,
  source_item_id TEXT,
  created_by_agent_id TEXT NOT NULL,
  creation_tool_call_id TEXT,
  pool_entry_id TEXT NOT NULL,
  executor_provider_kind TEXT NOT NULL CHECK (executor_provider_kind IN ('codex', 'openai', 'foundation_local')),
  executor_provider_account_id TEXT NOT NULL,
  executor_selection_mode TEXT NOT NULL CHECK (executor_selection_mode IN ('explicit_profile', 'provider_default')),
  executor_model_profile TEXT,
  executor_reasoning_effort TEXT CHECK (executor_reasoning_effort IS NULL OR executor_reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  executor_selection_source TEXT,
  reviewer_provider_kind TEXT NOT NULL CHECK (reviewer_provider_kind IN ('codex', 'openai', 'foundation_local')),
  reviewer_provider_account_id TEXT NOT NULL,
  reviewer_selection_mode TEXT NOT NULL CHECK (reviewer_selection_mode IN ('explicit_profile', 'provider_default')),
  reviewer_model_profile TEXT,
  reviewer_reasoning_effort TEXT CHECK (reviewer_reasoning_effort IS NULL OR reviewer_reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  reviewer_selection_source TEXT,
  revision_index INTEGER NOT NULL DEFAULT 0 CHECK (revision_index >= 0),
  max_review_rounds INTEGER NOT NULL DEFAULT 3 CHECK (max_review_rounds > 0),
  final_submission_id TEXT,
  latest_run_id TEXT,
  blocked_question TEXT,
  blocked_context TEXT,
  terminal_reason TEXT,
  error_code TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS tasks_status_queue
ON tasks(status, updated_at, task_id);
CREATE INDEX IF NOT EXISTS tasks_owner_source
ON tasks(owner_human_id, source_conversation_id, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS tasks_creation_call
ON tasks(source_conversation_id, creation_tool_call_id)
WHERE source_conversation_id IS NOT NULL AND creation_tool_call_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS task_validation_criteria (
  criterion_id TEXT PRIMARY KEY NOT NULL,
  task_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL CHECK (ordinal >= 1),
  description TEXT NOT NULL CHECK (description <> ''),
  expected_evidence TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(task_id, ordinal),
  UNIQUE(task_id, description)
);

CREATE INDEX IF NOT EXISTS task_validation_criteria_task
ON task_validation_criteria(task_id, ordinal, criterion_id);

CREATE TABLE IF NOT EXISTS agent_runs (
  run_id TEXT PRIMARY KEY NOT NULL,
  task_id TEXT NOT NULL,
  run_kind TEXT NOT NULL CHECK (run_kind IN ('executor', 'reviewer')),
  agent_id TEXT NOT NULL,
  attempt_index INTEGER NOT NULL DEFAULT 0 CHECK (attempt_index >= 0),
  revision_index INTEGER NOT NULL DEFAULT 0 CHECK (revision_index >= 0),
  parent_run_id TEXT,
  triggering_submission_id TEXT,
  triggering_review_id TEXT,
  resume_message TEXT,
  provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local')),
  provider_account_id TEXT NOT NULL,
  selection_mode TEXT NOT NULL CHECK (selection_mode IN ('explicit_profile', 'provider_default')),
  model_profile TEXT,
  reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
  selection_source TEXT,
  actual_provider_kind TEXT,
  actual_model_profile TEXT,
  max_provider_continuations INTEGER NOT NULL CHECK (max_provider_continuations BETWEEN 1 AND 1000),
  max_tool_calls INTEGER NOT NULL CHECK (max_tool_calls BETWEEN 1 AND 10000),
  max_active_minutes INTEGER NOT NULL CHECK (max_active_minutes BETWEEN 1 AND 10080),
  progress_audit_interval INTEGER NOT NULL CHECK (
    progress_audit_interval > 0
    AND progress_audit_interval <= max_provider_continuations
  ),
  status TEXT NOT NULL CHECK (status IN ('queued', 'leased', 'running', 'completed', 'waiting_for_approval', 'interrupted', 'failed', 'cancelled')),
  priority INTEGER NOT NULL DEFAULT 0,
  queued_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  lease_owner TEXT,
  lease_token TEXT,
  lease_expires_at TEXT,
  heartbeat_at TEXT,
  started_at TEXT,
  ended_at TEXT,
  cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK (cancellation_requested IN (0, 1)),
  retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
  error_code TEXT,
  error_message TEXT,
  provider_call_count INTEGER NOT NULL DEFAULT 0 CHECK (provider_call_count >= 0),
  tool_call_count INTEGER NOT NULL DEFAULT 0 CHECK (tool_call_count >= 0),
  input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
  cached_input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (cached_input_tokens >= 0),
  output_tokens INTEGER NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
  active_milliseconds INTEGER NOT NULL DEFAULT 0 CHECK (active_milliseconds >= 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS agent_runs_queue
ON agent_runs(status, priority DESC, queued_at, run_id);
CREATE INDEX IF NOT EXISTS agent_runs_task_history
ON agent_runs(task_id, created_at, run_id);
CREATE INDEX IF NOT EXISTS agent_runs_expired_leases
ON agent_runs(status, lease_expires_at);
CREATE INDEX IF NOT EXISTS agent_runs_parent
ON agent_runs(parent_run_id, created_at);

CREATE TABLE IF NOT EXISTS agent_run_items (
  item_id TEXT PRIMARY KEY NOT NULL,
  run_id TEXT NOT NULL,
  sequence_index INTEGER NOT NULL CHECK (sequence_index >= 1),
  round_index INTEGER NOT NULL DEFAULT 0 CHECK (round_index >= 0),
  kind TEXT NOT NULL CHECK (kind IN ('model_input', 'assistant_output', 'tool_call', 'tool_result', 'progress_notice', 'context_checkpoint', 'task_submission', 'task_review', 'artifact_reference', 'failure', 'cancellation')),
  status TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled', 'skipped')),
  correlation_id TEXT,
  parent_item_id TEXT,
  content_text TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(run_id, sequence_index)
);

CREATE INDEX IF NOT EXISTS agent_run_items_run_sequence
ON agent_run_items(run_id, sequence_index, item_id);

CREATE TABLE IF NOT EXISTS task_submissions (
  submission_id TEXT PRIMARY KEY NOT NULL,
  task_id TEXT NOT NULL,
  executor_run_id TEXT NOT NULL,
  revision_index INTEGER NOT NULL CHECK (revision_index >= 0),
  summary TEXT NOT NULL CHECK (summary <> ''),
  result_markdown TEXT NOT NULL CHECK (result_markdown <> ''),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(task_id, revision_index)
);

CREATE TABLE IF NOT EXISTS task_submission_criteria (
  submission_id TEXT NOT NULL,
  criterion_id TEXT NOT NULL,
  evidence_markdown TEXT NOT NULL CHECK (evidence_markdown <> ''),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY(submission_id, criterion_id)
);

CREATE TABLE IF NOT EXISTS task_submission_artifacts (
  submission_id TEXT NOT NULL,
  artifact_id TEXT NOT NULL,
  artifact_version_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY(submission_id, artifact_id, artifact_version_id)
);

CREATE INDEX IF NOT EXISTS task_submissions_task
ON task_submissions(task_id, revision_index, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS task_submissions_task_revision
ON task_submissions(task_id, revision_index);

CREATE TABLE IF NOT EXISTS task_reviews (
  review_id TEXT PRIMARY KEY NOT NULL,
  task_id TEXT NOT NULL,
  reviewer_run_id TEXT NOT NULL,
  reviewed_submission_id TEXT NOT NULL,
  overall_verdict TEXT NOT NULL CHECK (overall_verdict IN ('approve', 'request_changes', 'needs_human')),
  overall_feedback TEXT NOT NULL CHECK (overall_feedback <> ''),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS task_review_criteria (
  review_id TEXT NOT NULL,
  criterion_id TEXT NOT NULL,
  outcome TEXT NOT NULL CHECK (outcome IN ('pass', 'fail', 'uncertain')),
  evidence_markdown TEXT,
  feedback TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY(review_id, criterion_id)
);

CREATE INDEX IF NOT EXISTS task_reviews_task
ON task_reviews(task_id, created_at, review_id);
CREATE UNIQUE INDEX IF NOT EXISTS task_reviews_submission
ON task_reviews(task_id, reviewed_submission_id);

CREATE TABLE IF NOT EXISTS task_events (
  event_id TEXT PRIMARY KEY NOT NULL,
  task_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL CHECK (sequence_number >= 1),
  event_kind TEXT NOT NULL CHECK (event_kind <> ''),
  actor_id TEXT NOT NULL,
  causation_id TEXT,
  correlation_id TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(task_id, sequence_number)
);

CREATE TABLE IF NOT EXISTS run_events (
  event_id TEXT PRIMARY KEY NOT NULL,
  run_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL CHECK (sequence_number >= 1),
  event_kind TEXT NOT NULL CHECK (event_kind <> ''),
  actor_id TEXT NOT NULL,
  causation_id TEXT,
  correlation_id TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(run_id, sequence_number)
);

CREATE INDEX IF NOT EXISTS task_events_task_sequence
ON task_events(task_id, sequence_number, event_id);
CREATE INDEX IF NOT EXISTS run_events_run_sequence
ON run_events(run_id, sequence_number, event_id);

"#;
