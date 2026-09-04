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

	"github.com/kpsuperplane/noema/internal/home"
	_ "github.com/ncruces/go-sqlite3/driver"
)

const schemaVersion = 6

// Store is one open Noema database.
type Store struct {
	db *sql.DB
}

// Open opens a Go-created Noema database.
func Open(ctx context.Context, path string) (*Store, error) {
	if path == "" {
		return nil, errors.New("database path cannot be empty")
	}
	if err := filepathIsAbsolute(path); err != nil {
		return nil, err
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

	store := &Store{db: db}
	if err := store.initialize(ctx); err != nil {
		_ = db.Close()
		return nil, err
	}
	return store, nil
}

// Close closes the database.
func (s *Store) Close() error {
	return s.db.Close()
}

func filepathIsAbsolute(path string) error {
	if !filepath.IsAbs(path) {
		return errors.New("database path must be absolute")
	}
	return nil
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
	switch version {
	case 0:
		if _, err := tx.ExecContext(ctx, schemaSQL); err != nil {
			return fmt.Errorf("create schema: %w", err)
		}
		if _, err := tx.ExecContext(ctx, "PRAGMA user_version = 6"); err != nil {
			return fmt.Errorf("record schema version: %w", err)
		}
	case 1:
		if _, err := tx.ExecContext(ctx, schemaV2SQL); err != nil {
			return fmt.Errorf("apply schema version 2: %w", err)
		}
		if _, err := tx.ExecContext(ctx, schemaV3SQL); err != nil {
			return fmt.Errorf("apply schema version 3: %w", err)
		}
		if _, err := tx.ExecContext(ctx, schemaV4SQL); err != nil {
			return fmt.Errorf("apply schema version 4: %w", err)
		}
		if _, err := tx.ExecContext(ctx, schemaV5SQL); err != nil {
			return fmt.Errorf("apply schema version 5: %w", err)
		}
		if _, err := tx.ExecContext(ctx, schemaV6SQL); err != nil {
			return fmt.Errorf("apply schema version 6: %w", err)
		}
		if _, err := tx.ExecContext(ctx, "PRAGMA user_version = 6"); err != nil {
			return fmt.Errorf("record schema version: %w", err)
		}
	case 2:
		if _, err := tx.ExecContext(ctx, schemaV3SQL); err != nil {
			return fmt.Errorf("apply schema version 3: %w", err)
		}
		if _, err := tx.ExecContext(ctx, schemaV4SQL); err != nil {
			return fmt.Errorf("apply schema version 4: %w", err)
		}
		if _, err := tx.ExecContext(ctx, schemaV5SQL); err != nil {
			return fmt.Errorf("apply schema version 5: %w", err)
		}
		if _, err := tx.ExecContext(ctx, schemaV6SQL); err != nil {
			return fmt.Errorf("apply schema version 6: %w", err)
		}
		if _, err := tx.ExecContext(ctx, "PRAGMA user_version = 6"); err != nil {
			return fmt.Errorf("record schema version: %w", err)
		}
	case 3:
		if _, err := tx.ExecContext(ctx, schemaV4SQL); err != nil {
			return fmt.Errorf("apply schema version 4: %w", err)
		}
		if _, err := tx.ExecContext(ctx, schemaV5SQL); err != nil {
			return fmt.Errorf("apply schema version 5: %w", err)
		}
		if _, err := tx.ExecContext(ctx, schemaV6SQL); err != nil {
			return fmt.Errorf("apply schema version 6: %w", err)
		}
		if _, err := tx.ExecContext(ctx, "PRAGMA user_version = 6"); err != nil {
			return fmt.Errorf("record schema version: %w", err)
		}
	case 4:
		if _, err := tx.ExecContext(ctx, schemaV5SQL); err != nil {
			return fmt.Errorf("apply schema version 5: %w", err)
		}
		if _, err := tx.ExecContext(ctx, schemaV6SQL); err != nil {
			return fmt.Errorf("apply schema version 6: %w", err)
		}
		if _, err := tx.ExecContext(ctx, "PRAGMA user_version = 6"); err != nil {
			return fmt.Errorf("record schema version: %w", err)
		}
	case 5:
		if _, err := tx.ExecContext(ctx, schemaV6SQL); err != nil {
			return fmt.Errorf("apply schema version 6: %w", err)
		}
		if _, err := tx.ExecContext(ctx, "PRAGMA user_version = 6"); err != nil {
			return fmt.Errorf("record schema version: %w", err)
		}
	case schemaVersion:
	default:
		return fmt.Errorf("unsupported Go schema version %d", version)
	}

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit schema: %w", err)
	}
	return nil
}

const schemaSQL = `
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

` + schemaV2SQL + schemaV3SQL + schemaV4SQL + schemaV5SQL + schemaV6SQL

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
