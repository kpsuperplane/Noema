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

const schemaVersion = 1

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
		if _, err := tx.ExecContext(ctx, "PRAGMA user_version = 1"); err != nil {
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
`
