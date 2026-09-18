// Package store owns Go server structured state.
package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"sync"
	"time"

	"github.com/uptrace/bun"
	"github.com/uptrace/bun/dialect/sqlitedialect"
	"github.com/uptrace/bun/schema"

	"github.com/kpsuperplane/noema/internal/home"
	_ "github.com/ncruces/go-sqlite3/driver"
)

var migrations = [...]string{
	schemaV1SQL, schemaV2SQL, schemaV3SQL, schemaV4SQL, schemaV5SQL, schemaV6SQL,
	schemaV7SQL, schemaV8SQL, schemaV9SQL, schemaV10SQL, schemaV11SQL, schemaV12SQL,
	schemaV13SQL, schemaV14SQL, schemaV15SQL, schemaV16SQL, schemaV17SQL, schemaV18SQL,
	schemaV19SQL, schemaV20SQL, schemaV21SQL, schemaV22SQL, schemaV23SQL, schemaV24SQL,
	schemaV25SQL, schemaV26SQL, schemaV27SQL, schemaV28SQL, schemaV29SQL, schemaV30SQL,
	schemaV31SQL, schemaV32SQL, schemaV33SQL, schemaV34SQL, schemaV35SQL, schemaV36SQL, schemaV37SQL, schemaV38SQL, schemaV39SQL,
}

const schemaVersion = len(migrations)

// Store is one open Noema database.
type Store struct {
	db                  *bun.DB
	homeRoot            string
	workMu              sync.Mutex
	taskScheduleMu      sync.Mutex
	workSubscribers     map[chan struct{}]struct{}
	sessionRevocationMu sync.Mutex
	sessionRevocations  map[chan [32]byte]struct{}
	sessionCapacity     int
}

// Open opens a Go-created Noema database.
func Open(ctx context.Context, path string) (*Store, error) {
	if path == "" {
		return nil, errors.New("database path cannot be empty")
	}
	if !filepath.IsAbs(path) {
		return nil, errors.New("database path must be absolute")
	}

	values := url.Values{}
	values.Add("_pragma", "foreign_keys(1)")
	values.Add("_pragma", "journal_mode(wal)")
	values.Add("_pragma", "synchronous(normal)")
	values.Add("_pragma", "busy_timeout(5000)")
	values.Set("_txlock", "immediate")
	databaseURL := &url.URL{Scheme: "file", Path: filepath.ToSlash(path), RawQuery: values.Encode()}
	dsn := databaseURL.String()

	file, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR, 0o600)
	if err != nil {
		return nil, fmt.Errorf("create SQLite file: %w", err)
	}
	if err := file.Close(); err != nil {
		return nil, fmt.Errorf("close new SQLite file: %w", err)
	}
	if err := home.ProtectFile(path); err != nil {
		return nil, fmt.Errorf("protect SQLite file: %w", err)
	}

	db, err := sql.Open("sqlite3", dsn)
	if err != nil {
		return nil, fmt.Errorf("open SQLite: %w", err)
	}
	db.SetMaxOpenConns(4)
	db.SetMaxIdleConns(4)

	orm := bun.NewDB(db, sqlitedialect.New())
	configureMillisecondFields(orm)
	store := &Store{
		db: orm, homeRoot: storeHomeRoot(path), workSubscribers: make(map[chan struct{}]struct{}),
		sessionRevocations: make(map[chan [32]byte]struct{}),
		sessionCapacity:    browserSessionCapacity,
	}
	if err := store.initialize(ctx); err != nil {
		_ = db.Close()
		return nil, err
	}
	return store, nil
}

func storeHomeRoot(databasePath string) string {
	parent := filepath.Dir(filepath.Clean(databasePath))
	if filepath.Base(parent) == "db" {
		return filepath.Dir(parent)
	}
	return parent
}

// Close closes the database.
func (s *Store) Close() error {
	return s.db.Close()
}

func (s *Store) initialize(ctx context.Context) error {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return fmt.Errorf("begin schema transaction: %w", err)
	}
	defer func() { _ = tx.Rollback() }()

	var version int
	if err := tx.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		return fmt.Errorf("read schema version: %w", err)
	}
	if version < 0 || version > schemaVersion {
		return fmt.Errorf("unsupported Go schema version %d", version)
	}
	for next := version + 1; next <= schemaVersion; next++ {
		if _, err := tx.ExecContext(ctx, migrations[next-1]); err != nil {
			return fmt.Errorf("apply schema version %d: %w", next, err)
		}
		if version > 0 && next == 9 {
			if err := backfillTaskWorkEvents(ctx, tx); err != nil {
				return fmt.Errorf("backfill Task work events: %w", err)
			}
		}
	}
	if _, err := tx.ExecContext(ctx, ensureBuiltInAgentsSQL); err != nil {
		return fmt.Errorf("repair built-in Agents: %w", err)
	}
	if _, err := tx.ExecContext(ctx, recoverMcpOAuthSQL); err != nil {
		return fmt.Errorf("recover MCP OAuth: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='awaiting_user',
	 oauth_attempt_id=NULL,adapter_attempt_id=NULL,failure_code='server_restarted' WHERE state='authorizing'`); err != nil {
		return fmt.Errorf("recover MCP authentication: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='cancelled',
 oauth_attempt_id=NULL,failure_code='outcome_uncertain' WHERE state='resuming'`); err != nil {
		return fmt.Errorf("recover MCP call resumption: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='cancelled',
 oauth_attempt_id=NULL,adapter_attempt_id=NULL,failure_code='task_cancelled',updated_at_ms=?
 WHERE task_id IS NOT NULL AND state IN ('awaiting_user','authorizing','resuming')
 AND NOT EXISTS (
   SELECT 1 FROM tasks t JOIN task_runs r ON r.run_id=mcp_auth_requests.run_id
   WHERE t.task_id=mcp_auth_requests.task_id AND t.generation=mcp_auth_requests.task_generation
     AND t.current_run_id=mcp_auth_requests.run_id AND r.task_id=t.task_id
     AND r.task_generation=t.generation AND r.status IN ('running','waiting_for_approval')
 )`, millis(time.Now().UTC())); err != nil {
		return fmt.Errorf("close stale Task authentication: %w", err)
	}
	if version != schemaVersion {
		if _, err := tx.ExecContext(ctx, fmt.Sprintf("PRAGMA user_version = %d", schemaVersion)); err != nil {
			return fmt.Errorf("record schema version: %w", err)
		}
	}

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit schema: %w", err)
	}
	return nil
}

const schemaV1SQL = `
CREATE TABLE tasks (
    task_id TEXT PRIMARY KEY,
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 500),
    state TEXT NOT NULL CHECK (state IN ('captured', 'running', 'completed', 'failed', 'cancelled')),
    current_run_id TEXT,
    revision INTEGER NOT NULL CHECK (revision > 0),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    CHECK ((state = 'running') = (current_run_id IS NOT NULL))
) STRICT;

CREATE TABLE task_events (
    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
    task_revision INTEGER NOT NULL,
    kind TEXT NOT NULL,
    occurred_at_ms INTEGER NOT NULL,
    UNIQUE (task_id, task_revision)
) STRICT;

`

const schemaV2SQL = `
CREATE TABLE human_passkeys (
    credential_id TEXT PRIMARY KEY,
    credential_json TEXT NOT NULL
        CHECK (json_valid(credential_json) AND json_type(credential_json) = 'object'),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE browser_sessions (
    session_hash BLOB PRIMARY KEY CHECK (length(session_hash) = 32),
    state TEXT NOT NULL CHECK (state IN ('anonymous', 'authenticated')),
    passkey_id TEXT REFERENCES human_passkeys(credential_id) ON DELETE CASCADE,
    recent_passkey_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    CHECK (expires_at_ms > created_at_ms),
    CHECK (
        (state = 'anonymous' AND passkey_id IS NULL AND recent_passkey_at_ms IS NULL)
        OR (state = 'authenticated' AND passkey_id IS NOT NULL
            AND recent_passkey_at_ms IS NOT NULL)
    )
) STRICT;

CREATE INDEX browser_sessions_expiry ON browser_sessions(expires_at_ms);
CREATE INDEX browser_sessions_passkey ON browser_sessions(passkey_id);
`

const schemaV3SQL = `
CREATE TABLE clients (
    client_id TEXT PRIMARY KEY CHECK (length(client_id) BETWEEN 1 AND 128),
    display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 128),
    created_at INTEGER NOT NULL,
    revoked_at INTEGER
) STRICT;

CREATE TABLE native_oauth_codes (
    code_hash BLOB PRIMARY KEY CHECK (length(code_hash) = 32),
    client_id TEXT NOT NULL REFERENCES clients(client_id) ON DELETE CASCADE,
    redirect_uri TEXT NOT NULL CHECK (length(redirect_uri) BETWEEN 1 AND 2048),
    pkce_challenge TEXT NOT NULL CHECK (length(pkce_challenge) = 43),
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    CHECK (expires_at > created_at)
) STRICT;
CREATE INDEX native_oauth_codes_expiry ON native_oauth_codes(expires_at);

CREATE TABLE native_oauth_families (
    family_id TEXT PRIMARY KEY CHECK (length(family_id) = 32 AND family_id = lower(family_id)),
    client_id TEXT NOT NULL REFERENCES clients(client_id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    idle_expires_at INTEGER NOT NULL,
    absolute_expires_at INTEGER NOT NULL,
    revoked_at INTEGER,
    revoke_reason TEXT CHECK (revoke_reason IN ('client', 'global', 'replay', 'expired')),
    CHECK (idle_expires_at > created_at),
    CHECK (absolute_expires_at >= idle_expires_at),
    CHECK ((revoked_at IS NULL) = (revoke_reason IS NULL))
) STRICT;
CREATE INDEX native_oauth_families_client_active
ON native_oauth_families(client_id, absolute_expires_at) WHERE revoked_at IS NULL;

CREATE TABLE native_oauth_refresh_tokens (
    token_hash BLOB PRIMARY KEY CHECK (length(token_hash) = 32),
    family_id TEXT NOT NULL REFERENCES native_oauth_families(family_id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    status TEXT NOT NULL CHECK (status IN ('active', 'used')),
    issued_at INTEGER NOT NULL,
    used_at INTEGER,
    UNIQUE (family_id, sequence),
    CHECK ((status = 'active' AND used_at IS NULL) OR (status = 'used' AND used_at IS NOT NULL))
) STRICT;
CREATE UNIQUE INDEX native_oauth_refresh_tokens_one_active
ON native_oauth_refresh_tokens(family_id) WHERE status = 'active';

CREATE TABLE native_oauth_access_tokens (
    token_hash BLOB PRIMARY KEY CHECK (length(token_hash) = 32),
    family_id TEXT NOT NULL REFERENCES native_oauth_families(family_id) ON DELETE CASCADE,
    issued_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    revoked_at INTEGER,
    CHECK (expires_at > issued_at)
) STRICT;
CREATE INDEX native_oauth_access_tokens_family_active
ON native_oauth_access_tokens(family_id, expires_at) WHERE revoked_at IS NULL;

CREATE TABLE native_oauth_browser_requests (
    session_hash BLOB PRIMARY KEY REFERENCES browser_sessions(session_hash) ON DELETE CASCADE,
    query TEXT NOT NULL CHECK (length(query) BETWEEN 1 AND 8192),
    csrf TEXT CHECK (csrf IS NULL OR length(csrf) = 43)
) STRICT;
`

const schemaV4SQL = `
CREATE TABLE provider_accounts (
    provider_account_id TEXT PRIMARY KEY CHECK (length(provider_account_id) BETWEEN 1 AND 256),
    provider_kind TEXT NOT NULL CHECK (length(provider_kind) BETWEEN 1 AND 64),
    account_key TEXT NOT NULL CHECK (length(account_key) BETWEEN 1 AND 128),
    display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 128),
    auth_method TEXT NOT NULL CHECK (auth_method IN (
        'oauth_device_code', 'oauth_pkce', 'secret_input', 'external_manual', 'none'
    )),
    is_active INTEGER NOT NULL CHECK (is_active IN (0, 1)),
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    status TEXT NOT NULL CHECK (status IN (
        'unknown', 'checking', 'authenticated', 'unauthenticated', 'unavailable'
    )),
    last_checked_at_ms INTEGER,
    last_authenticated_at_ms INTEGER,
    last_error_code TEXT CHECK (last_error_code IS NULL OR length(last_error_code) <= 128),
    last_error_message TEXT CHECK (last_error_message IS NULL OR length(last_error_message) <= 1000),
    metadata_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE (provider_kind, account_key)
) STRICT;

CREATE INDEX provider_accounts_active_order
ON provider_accounts(is_active, provider_kind, display_name, account_key);
`

const schemaV5SQL = `
CREATE TABLE conversations (
    conversation_id TEXT PRIMARY KEY
        CHECK (length(conversation_id) = 45
            AND substr(conversation_id, 1, 13) = 'conversation:'
            AND substr(conversation_id, 14) NOT GLOB '*[^0-9a-f]*'),
    owner_human_id TEXT NOT NULL CHECK (owner_human_id = 'human:local'),
    provider TEXT NOT NULL CHECK (length(provider) BETWEEN 1 AND 64),
    cwd TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE local_human_state (
    state_id INTEGER PRIMARY KEY CHECK (state_id = 1),
    primary_conversation_id TEXT UNIQUE
        REFERENCES conversations(conversation_id) ON DELETE SET NULL
) STRICT;

INSERT INTO local_human_state (state_id, primary_conversation_id) VALUES (1, NULL);
`

const schemaV6SQL = `
CREATE TABLE hosted_model_assignments (
    role TEXT PRIMARY KEY CHECK (role IN (
        'noema', 'simple_tasks', 'medium_tasks', 'difficult_tasks', 'task_reviewer',
        'web_fetch_summarizer', 'tool_progress_audit', 'action_reviewer',
        'memory_consolidation'
    )),
    provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'openrouter')),
    provider_account_id TEXT NOT NULL
        REFERENCES provider_accounts(provider_account_id) ON DELETE RESTRICT,
    selection_mode TEXT NOT NULL
        CHECK (selection_mode IN ('noema_recommended', 'explicit_profile')),
    model_profile TEXT,
    reasoning_effort TEXT CHECK (
        reasoning_effort IS NULL
        OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')
    ),
    fast_mode INTEGER NOT NULL CHECK (fast_mode IN (0, 1)),
    CHECK (
        (selection_mode = 'noema_recommended'
            AND model_profile IS NULL AND reasoning_effort IS NULL)
        OR
        (selection_mode = 'explicit_profile'
            AND model_profile IS NOT NULL AND trim(model_profile) <> '')
    )
) STRICT;
`

const schemaV7SQL = `
ALTER TABLE conversations ADD COLUMN agent_status TEXT NOT NULL DEFAULT 'idle'
    CHECK (agent_status IN (
        'idle', 'input_received', 'thinking', 'tool_running',
        'waiting_for_previous_turn_completion', 'interrupting', 'error'
    ));

CREATE TABLE conversation_turns (
    turn_id TEXT PRIMARY KEY
        CHECK (length(turn_id) = 37 AND substr(turn_id, 1, 5) = 'turn:'),
    conversation_id TEXT NOT NULL
        REFERENCES conversations(conversation_id) ON DELETE CASCADE,
    trigger_item_id TEXT REFERENCES conversation_items(item_id) ON DELETE RESTRICT,
    status TEXT NOT NULL CHECK (status IN (
        'input_received', 'running', 'waiting_for_tool', 'interrupted',
        'completed', 'failed', 'cancelled'
    )),
    metadata_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
    started_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX conversation_turns_conversation
ON conversation_turns(conversation_id, created_at_ms);

CREATE TABLE conversation_items (
    item_id TEXT PRIMARY KEY
        CHECK (length(item_id) = 37 AND substr(item_id, 1, 5) = 'item:'),
    conversation_id TEXT NOT NULL
        REFERENCES conversations(conversation_id) ON DELETE CASCADE,
    turn_id TEXT REFERENCES conversation_turns(turn_id) ON DELETE RESTRICT,
    parent_item_id TEXT REFERENCES conversation_items(item_id) ON DELETE RESTRICT,
    sequence_index INTEGER NOT NULL CHECK (sequence_index > 0),
    kind TEXT NOT NULL CHECK (kind IN (
        'user_text', 'assistant_text', 'activity', 'a2ui_card',
        'multiple_choice_prompt', 'multiple_choice_selection', 'tool_call',
        'tool_result', 'reasoning', 'model_context_update', 'approval_request',
        'approval_result', 'error_notice', 'artifact_reference', 'task_reference'
    )),
    status TEXT NOT NULL CHECK (status IN (
        'pending', 'running', 'completed', 'failed', 'cancelled', 'interrupted'
    )),
    author_actor_id TEXT NOT NULL CHECK (length(author_actor_id) BETWEEN 1 AND 128),
    content_text TEXT,
    provider_content_text TEXT,
    payload_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(payload_json) AND json_type(payload_json) = 'object'),
    metadata_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
    deleted_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE (conversation_id, sequence_index)
) STRICT;

CREATE INDEX conversation_items_conversation_sequence
ON conversation_items(conversation_id, sequence_index);
`

const schemaV8SQL = `
PRAGMA defer_foreign_keys = ON;

CREATE TABLE conversation_turns_v8 (
    turn_id TEXT PRIMARY KEY
        CHECK (length(turn_id) = 37 AND substr(turn_id, 1, 5) = 'turn:'),
    conversation_id TEXT NOT NULL
        REFERENCES conversations(conversation_id) ON DELETE CASCADE,
    trigger_item_id TEXT,
    status TEXT NOT NULL CHECK (status IN (
        'input_received', 'running', 'waiting_for_tool', 'interrupted',
        'completed', 'failed', 'cancelled'
    )),
    metadata_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
    started_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE (conversation_id, turn_id),
    FOREIGN KEY (conversation_id, trigger_item_id)
        REFERENCES conversation_items_v8(conversation_id, item_id) ON DELETE NO ACTION
) STRICT;

CREATE TABLE conversation_items_v8 (
    item_id TEXT PRIMARY KEY
        CHECK (length(item_id) = 37 AND substr(item_id, 1, 5) = 'item:'),
    conversation_id TEXT NOT NULL
        REFERENCES conversations(conversation_id) ON DELETE CASCADE,
    turn_id TEXT,
    parent_item_id TEXT,
    sequence_index INTEGER NOT NULL CHECK (sequence_index > 0),
    kind TEXT NOT NULL CHECK (kind IN (
        'user_text', 'assistant_text', 'activity', 'a2ui_card',
        'multiple_choice_prompt', 'multiple_choice_selection', 'tool_call',
        'tool_result', 'reasoning', 'model_context_update', 'approval_request',
        'approval_result', 'error_notice', 'artifact_reference', 'task_reference'
    )),
    status TEXT NOT NULL CHECK (status IN (
        'pending', 'running', 'completed', 'failed', 'cancelled', 'interrupted'
    )),
    author_actor_id TEXT NOT NULL CHECK (length(author_actor_id) BETWEEN 1 AND 128),
    content_text TEXT,
    provider_content_text TEXT,
    payload_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(payload_json) AND json_type(payload_json) = 'object'),
    metadata_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
    deleted_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE (conversation_id, item_id),
    UNIQUE (conversation_id, sequence_index),
    CHECK (
        provider_content_text IS NULL OR (
            kind = 'assistant_text' AND content_text IS NOT NULL
            AND provider_content_text <> content_text
        )
    ),
    FOREIGN KEY (conversation_id, turn_id)
        REFERENCES conversation_turns_v8(conversation_id, turn_id) ON DELETE NO ACTION,
    FOREIGN KEY (conversation_id, parent_item_id)
        REFERENCES conversation_items_v8(conversation_id, item_id) ON DELETE NO ACTION
) STRICT;

INSERT INTO conversation_turns_v8 SELECT * FROM conversation_turns;
INSERT INTO conversation_items_v8 SELECT * FROM conversation_items;

UPDATE conversation_turns SET trigger_item_id = NULL;
UPDATE conversation_items SET parent_item_id = NULL;
DELETE FROM conversation_items;
DELETE FROM conversation_turns;
DROP TABLE conversation_items;
DROP TABLE conversation_turns;
ALTER TABLE conversation_turns_v8 RENAME TO conversation_turns;
ALTER TABLE conversation_items_v8 RENAME TO conversation_items;

CREATE INDEX conversation_turns_conversation
ON conversation_turns(conversation_id, created_at_ms);
CREATE UNIQUE INDEX conversation_turns_one_active
ON conversation_turns(conversation_id)
WHERE status IN ('input_received', 'running', 'waiting_for_tool');
CREATE INDEX conversation_items_conversation_sequence
ON conversation_items(conversation_id, sequence_index);
`

const schemaV9SQL = `
CREATE TABLE projects (
    project_id TEXT PRIMARY KEY
        CHECK (length(project_id) = 40 AND substr(project_id, 1, 8) = 'project:'),
    workspace_id TEXT NOT NULL CHECK (workspace_id = 'workspace:personal'),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    description TEXT NOT NULL DEFAULT '',
    folder TEXT CHECK (folder IS NULL OR length(trim(folder)) > 0),
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
    archived_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX projects_workspace_active
ON projects(workspace_id, archived_at_ms, updated_at_ms DESC, project_id DESC);

CREATE TABLE work_events (
    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
	event_key TEXT NOT NULL UNIQUE
		CHECK (length(event_key) = 38 AND substr(event_key, 1, 6) = 'event:'),
    workspace_id TEXT NOT NULL CHECK (workspace_id = 'workspace:personal'),
    project_id TEXT REFERENCES projects(project_id) ON DELETE RESTRICT,
    task_id TEXT REFERENCES tasks(task_id) ON DELETE RESTRICT,
	run_id TEXT,
    subject_revision INTEGER NOT NULL CHECK (subject_revision > 0),
    kind TEXT NOT NULL CHECK (length(trim(kind)) > 0),
	actor_id TEXT NOT NULL CHECK (length(trim(actor_id)) > 0),
	causation_id TEXT,
	correlation_id TEXT NOT NULL CHECK (length(trim(correlation_id)) > 0),
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
	occurred_at_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX work_events_workspace_cursor ON work_events(workspace_id, event_id);
CREATE INDEX work_events_project_cursor ON work_events(project_id, event_id) WHERE project_id IS NOT NULL;
CREATE INDEX work_events_task_cursor ON work_events(task_id, event_id) WHERE task_id IS NOT NULL;
CREATE INDEX work_events_run_cursor ON work_events(run_id, event_id) WHERE run_id IS NOT NULL;

CREATE TABLE command_receipts (
    actor_id TEXT NOT NULL CHECK (length(trim(actor_id)) > 0),
    command_name TEXT NOT NULL CHECK (length(trim(command_name)) > 0),
    client_mutation_id TEXT NOT NULL CHECK (length(trim(client_mutation_id)) > 0),
    request_digest TEXT NOT NULL CHECK (length(request_digest) = 64 AND request_digest = lower(request_digest)),
    result_project_id TEXT REFERENCES projects(project_id) ON DELETE RESTRICT,
    result_task_id TEXT REFERENCES tasks(task_id) ON DELETE RESTRICT,
    result_event_id INTEGER NOT NULL REFERENCES work_events(event_id) ON DELETE RESTRICT,
    response_json TEXT NOT NULL CHECK (json_valid(response_json)),
	created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (actor_id, command_name, client_mutation_id)
) STRICT;
`

const schemaV10SQL = `
CREATE TABLE agents (
    agent_id TEXT PRIMARY KEY CHECK (substr(agent_id, 1, 6) = 'agent:'),
    display_name TEXT CHECK (
        display_name IS NULL OR length(trim(display_name)) BETWEEN 1 AND 128
    ),
    system_role TEXT UNIQUE CHECK (
        system_role IS NULL OR system_role IN ('primary', 'task_executor', 'task_reviewer')
    ),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE acp_agents (
    agent_id TEXT PRIMARY KEY REFERENCES agents(agent_id) ON DELETE RESTRICT,
    command TEXT NOT NULL CHECK (length(trim(command)) BETWEEN 1 AND 4096),
    arguments_json TEXT NOT NULL DEFAULT '[]'
        CHECK (json_valid(arguments_json) AND json_type(arguments_json) = 'array'),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    auth_status TEXT NOT NULL DEFAULT 'unknown'
        CHECK (auth_status IN ('unknown', 'none', 'required', 'authenticated', 'failed')),
    health_status TEXT NOT NULL DEFAULT 'unknown'
        CHECK (health_status IN ('unknown', 'healthy', 'unavailable')),
    implementation_name TEXT CHECK (
        implementation_name IS NULL OR length(implementation_name) <= 256
    ),
    implementation_version TEXT CHECK (
        implementation_version IS NULL OR length(implementation_version) <= 256
    ),
    capabilities_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(capabilities_json) AND json_type(capabilities_json) = 'object'),
    connection_revision INTEGER NOT NULL DEFAULT 1 CHECK (connection_revision > 0),
    last_error TEXT CHECK (last_error IS NULL OR length(last_error) <= 512),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE acp_auth_attempts (
    attempt_id TEXT PRIMARY KEY CHECK (substr(attempt_id, 1, 9) = 'acp_auth:'),
    agent_id TEXT NOT NULL REFERENCES acp_agents(agent_id) ON DELETE CASCADE,
    connection_revision INTEGER NOT NULL CHECK (connection_revision > 0),
    method_id TEXT NOT NULL CHECK (length(trim(method_id)) BETWEEN 1 AND 256),
    state TEXT NOT NULL CHECK (state IN ('pending', 'completed', 'failed')),
    safe_message TEXT CHECK (safe_message IS NULL OR length(safe_message) <= 512),
    created_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER
) STRICT;

CREATE INDEX acp_auth_attempts_agent ON acp_auth_attempts(agent_id, created_at_ms DESC);
CREATE UNIQUE INDEX acp_auth_attempts_one_pending
ON acp_auth_attempts(agent_id) WHERE state = 'pending';

CREATE TABLE task_model_pool_settings (
    pool_entry_id TEXT PRIMARY KEY CHECK (pool_entry_id IN (
        'task_pool:setting:simple',
        'task_pool:setting:medium',
        'task_pool:setting:difficult'
    )),
    complexity TEXT NOT NULL UNIQUE CHECK (complexity IN ('simple', 'medium', 'difficult')),
    label TEXT CHECK (label IS NULL OR length(trim(label)) BETWEEN 1 AND 128),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;

INSERT INTO task_model_pool_settings (
    pool_entry_id, complexity, enabled, sort_order, created_at_ms, updated_at_ms
) VALUES
    ('task_pool:setting:simple', 'simple', 1, 0, CAST(strftime('%s', 'now') AS INTEGER) * 1000, CAST(strftime('%s', 'now') AS INTEGER) * 1000),
    ('task_pool:setting:medium', 'medium', 1, 0, CAST(strftime('%s', 'now') AS INTEGER) * 1000, CAST(strftime('%s', 'now') AS INTEGER) * 1000),
    ('task_pool:setting:difficult', 'difficult', 1, 0, CAST(strftime('%s', 'now') AS INTEGER) * 1000, CAST(strftime('%s', 'now') AS INTEGER) * 1000);
`

const ensureBuiltInAgentsSQL = `
INSERT INTO agents (agent_id, display_name, system_role, created_at_ms, updated_at_ms)
VALUES
    ('agent:primary', NULL, 'primary', CAST(strftime('%s', 'now') AS INTEGER) * 1000, CAST(strftime('%s', 'now') AS INTEGER) * 1000),
    ('agent:task-executor', 'Task Executor', 'task_executor', CAST(strftime('%s', 'now') AS INTEGER) * 1000, CAST(strftime('%s', 'now') AS INTEGER) * 1000),
    ('agent:task-reviewer', 'Task Reviewer', 'task_reviewer', CAST(strftime('%s', 'now') AS INTEGER) * 1000, CAST(strftime('%s', 'now') AS INTEGER) * 1000)
ON CONFLICT(agent_id) DO UPDATE SET
    system_role = excluded.system_role,
    updated_at_ms = excluded.updated_at_ms;
`

const recoverMcpOAuthSQL = `
UPDATE mcp_oauth_attempts
SET status = 'failed', failure_code = 'server_restarted', updated_at_ms = CAST(strftime('%s', 'now') AS INTEGER) * 1000
WHERE status = 'waiting_for_user';
`

func backfillTaskWorkEvents(ctx context.Context, tx bun.Tx) error {
	rows, err := tx.QueryContext(ctx, "PRAGMA table_info(task_events)")
	if err != nil {
		return err
	}
	columns := make(map[string]bool)
	for rows.Next() {
		var cid, notnull, primary int
		var name, kind string
		var defaultValue any
		if err := rows.Scan(&cid, &name, &kind, &notnull, &defaultValue, &primary); err != nil {
			_ = rows.Close()
			return err
		}
		columns[name] = true
	}
	if err := rows.Close(); err != nil {
		return err
	}
	for _, required := range []string{"event_id", "task_id", "task_revision", "kind", "occurred_at_ms"} {
		if !columns[required] {
			return nil
		}
	}
	_, err = tx.ExecContext(ctx, `INSERT OR IGNORE INTO work_events
(event_id, event_key, workspace_id, task_id, subject_revision, kind, actor_id, correlation_id, payload_json, occurred_at_ms)
SELECT event_id, printf('event:%032x', event_id), 'workspace:personal', task_id, task_revision,
       replace(kind, '_', '.'),
       CASE WHEN kind = 'task_captured' THEN 'actor:human:local' ELSE 'actor:system:runtime' END,
       'correlation:migrated:' || task_id, json_object('v', 1, 'revision', task_revision), occurred_at_ms
FROM task_events ORDER BY event_id`)
	return err
}

// Existing SQLite timestamps remain integer milliseconds. Configure the mapped
// models before the database is shared with callers.
func configureMillisecondFields(db *bun.DB) {
	for _, model := range []any{Task{}, TaskRun{}, TaskEvent{}} {
		for _, field := range db.Dialect().Tables().Get(reflect.TypeOf(model)).Fields {
			if field.IndirectType != reflect.TypeFor[time.Time]() {
				continue
			}
			field.Scan = func(dest reflect.Value, source any) error {
				if source == nil {
					dest.SetZero()
					return nil
				}
				ms, ok := source.(int64)
				if !ok {
					return fmt.Errorf("invalid millisecond timestamp: %T", source)
				}
				if dest.Kind() == reflect.Pointer {
					dest.Set(reflect.New(dest.Type().Elem()))
					dest = dest.Elem()
				}
				dest.Set(reflect.ValueOf(fromMillis(ms)))
				return nil
			}
			field.Append = func(_ schema.QueryGen, b []byte, value reflect.Value) []byte {
				if value.Kind() == reflect.Pointer {
					value = value.Elem()
				}
				return strconv.AppendInt(b, millis(value.Interface().(time.Time)), 10)
			}
		}
	}
}
