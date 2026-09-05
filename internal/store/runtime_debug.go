package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"
)

const runtimeDebugSpanLimit = 128

// RuntimeDebugScope selects one Chat turn or Task run profile.
type RuntimeDebugScope struct {
	Kind string
	ID   string
}

// RuntimeDebugMetadata is safe diagnostic data for one measured operation.
type RuntimeDebugMetadata struct {
	Provider, Model, Phase, ToolName, CorrelationID string
	ResponseIndex, RoundIndex                       *int
	InputTokens, CachedInputTokens                  *int
	OutputTokens, TotalTokens                       *int
}

// RuntimeDebugSpan is one measured operation in a profile.
type RuntimeDebugSpan struct {
	ID, Category, Name, Status string
	StartedAt                  time.Time
	EndedAt                    *time.Time
	DurationMilliseconds       *int64
	Metadata                   RuntimeDebugMetadata
}

// RuntimeDebugProfile is one stored scope and its bounded spans.
type RuntimeDebugProfile struct {
	Scope                RuntimeDebugScope
	OwnerHumanID, Status string
	StartedAt            time.Time
	EndedAt              *time.Time
	Spans                []RuntimeDebugSpan
}

// BeginRuntimeDebugSpan starts one durable measured operation.
func (s *Store) BeginRuntimeDebugSpan(ctx context.Context, scope RuntimeDebugScope, category, name string,
	metadata RuntimeDebugMetadata, now time.Time) (string, error) {
	name = strings.TrimSpace(name)
	if scope.ID == "" || name == "" || len(name) > 128 || !validDebugCategory(category) {
		return "", errors.New("invalid runtime debug span")
	}
	id, err := newID("debug_span")
	if err != nil {
		return "", err
	}
	encoded, err := json.Marshal(metadata)
	if err != nil {
		return "", err
	}
	turnID, runID := any(nil), any(nil)
	switch scope.Kind {
	case "conversation_turn":
		turnID = scope.ID
	case "task_run":
		runID = scope.ID
	default:
		return "", errors.New("invalid runtime debug scope")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return "", err
	}
	defer func() { _ = tx.Rollback() }()
	if _, err = tx.ExecContext(ctx, `INSERT INTO runtime_debug_spans
(span_id,conversation_turn_id,task_run_id,category,name,metadata_json,started_at_ms)
VALUES (?,?,?,?,?,?,?)`, id, turnID, runID, category, name, string(encoded), millis(now.UTC())); err != nil {
		return "", fmt.Errorf("begin runtime debug span: %w", err)
	}
	column := "conversation_turn_id"
	if runID != nil {
		column = "task_run_id"
	}
	_, err = tx.ExecContext(ctx, `DELETE FROM runtime_debug_spans WHERE span_id IN (
SELECT span_id FROM runtime_debug_spans WHERE `+column+`=? AND span_id<>?
ORDER BY started_at_ms DESC,span_id DESC LIMIT -1 OFFSET ?)`, scope.ID, id, runtimeDebugSpanLimit-1)
	if err != nil {
		return "", fmt.Errorf("bound runtime debug spans: %w", err)
	}
	if err = tx.Commit(); err != nil {
		return "", err
	}
	return id, nil
}

// FinishRuntimeDebugSpan closes one running measured operation.
func (s *Store) FinishRuntimeDebugSpan(ctx context.Context, id, status string, metadata RuntimeDebugMetadata,
	duration time.Duration, now time.Time) error {
	if id == "" || !validDebugStatus(status) || status == "running" {
		return errors.New("invalid runtime debug span completion")
	}
	encoded, err := json.Marshal(metadata)
	if err != nil {
		return err
	}
	durationMS := max(duration.Milliseconds(), 0)
	result, err := s.db.ExecContext(ctx, `UPDATE runtime_debug_spans
SET status=?,duration_ms=?,metadata_json=?,ended_at_ms=? WHERE span_id=? AND status='running'`,
		status, durationMS, string(encoded), millis(now.UTC()), id)
	if err != nil {
		return fmt.Errorf("finish runtime debug span: %w", err)
	}
	if count, _ := result.RowsAffected(); count != 1 {
		return errors.New("runtime debug span is unavailable")
	}
	return nil
}

// RuntimeDebugProfile returns one owner-authorized profile source record.
func (s *Store) RuntimeDebugProfile(ctx context.Context, scope RuntimeDebugScope) (*RuntimeDebugProfile, error) {
	if scope.ID == "" {
		return nil, nil
	}
	profile := RuntimeDebugProfile{Scope: scope}
	var started int64
	var ended sql.NullInt64
	var row *sql.Row
	switch scope.Kind {
	case "conversation_turn":
		row = s.db.QueryRowContext(ctx, `SELECT c.owner_human_id,t.status,t.started_at_ms,t.completed_at_ms
FROM conversation_turns t JOIN conversations c ON c.conversation_id=t.conversation_id WHERE t.turn_id=?`, scope.ID)
	case "task_run":
		row = s.db.QueryRowContext(ctx, `SELECT 'human:local',status,COALESCE(started_at_ms,queued_at_ms),ended_at_ms
FROM task_runs WHERE run_id=?`, scope.ID)
	default:
		return nil, errors.New("invalid runtime debug scope")
	}
	if err := row.Scan(&profile.OwnerHumanID, &profile.Status, &started, &ended); errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	} else if err != nil {
		return nil, err
	}
	profile.StartedAt = fromMillis(started)
	profile.EndedAt = nullTimePointer(ended)
	column := "conversation_turn_id"
	if scope.Kind == "task_run" {
		column = "task_run_id"
	}
	rows, err := s.db.QueryContext(ctx, `SELECT span_id,category,name,status,duration_ms,metadata_json,started_at_ms,ended_at_ms
FROM runtime_debug_spans WHERE `+column+`=? ORDER BY started_at_ms,span_id LIMIT ?`, scope.ID, runtimeDebugSpanLimit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	for rows.Next() {
		var span RuntimeDebugSpan
		var duration, ended sql.NullInt64
		var encoded string
		var began int64
		if err := rows.Scan(&span.ID, &span.Category, &span.Name, &span.Status, &duration, &encoded, &began, &ended); err != nil {
			return nil, err
		}
		if err := json.Unmarshal([]byte(encoded), &span.Metadata); err != nil {
			return nil, fmt.Errorf("decode runtime debug metadata: %w", err)
		}
		span.StartedAt = fromMillis(began)
		span.EndedAt = nullTimePointer(ended)
		if duration.Valid {
			span.DurationMilliseconds = &duration.Int64
		}
		profile.Spans = append(profile.Spans, span)
	}
	return &profile, rows.Err()
}

func validDebugCategory(value string) bool {
	return value == "provider" || value == "tool" || value == "runtime" || value == "persistence"
}

func validDebugStatus(value string) bool {
	return value == "running" || value == "completed" || value == "failed" || value == "cancelled" || value == "interrupted"
}
