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
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(complexity, provider_account_id, model_profile, reasoning_effort)
);

CREATE INDEX IF NOT EXISTS task_model_pool_entries_selection
ON task_model_pool_entries(complexity, enabled, sort_order, label, pool_entry_id);
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
    'intake', 'dispatch', 'active', 'human_gate', 'acceptance',
    'terminal_success', 'terminal_cancelled'
  )),
  board_visible INTEGER NOT NULL CHECK (board_visible IN (0, 1)),
  UNIQUE (workflow_id, stage_id),
  UNIQUE (workflow_id, stable_key),
  UNIQUE (workflow_id, ordinal),
  UNIQUE (workflow_id, system_behavior),
  CHECK (
    (system_behavior IN ('terminal_success', 'terminal_cancelled') AND board_visible = 0)
    OR
    (system_behavior NOT IN ('terminal_success', 'terminal_cancelled') AND board_visible = 1)
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
  accepted_submission_id TEXT,
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
    'task_created', 'task_waiting', 'task_review_ready', 'task_recovery', 'task_accepted'
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
  ('stage:personal:review',    'workflow:personal:default', 'review',    'Review',    50, 'acceptance',         1),
  ('stage:personal:completed', 'workflow:personal:default', 'completed', 'Completed', 60, 'terminal_success',   0),
  ('stage:personal:cancelled', 'workflow:personal:default', 'cancelled', 'Cancelled', 70, 'terminal_cancelled', 0)
ON CONFLICT (stage_id) DO NOTHING;

INSERT INTO task_execution_policy (
  policy_id, max_provider_continuations, max_tool_calls, max_active_minutes,
  progress_audit_interval, max_automatic_retries, max_review_rounds
) VALUES ('default', 80, 400, 120, 20, 3, 3)
ON CONFLICT (policy_id) DO NOTHING;

INSERT INTO schema_state (name, version, applied_at)
VALUES ('sqlite_store_v3', 3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT (name) DO NOTHING;
