package store

import (
	"context"
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
	"strings"
	"time"
)

var (
	// ErrTaskNotFound means the task is unavailable to the caller.
	ErrTaskNotFound = errors.New("task not found")
	// ErrStaleRun means another run owns the current task state.
	ErrStaleRun = errors.New("stale task run")
)

// TaskState is a stored Task lifecycle state.
type TaskState string

const (
	// TaskCaptured is ready for later execution.
	TaskCaptured TaskState = "captured"
	// TaskRunning has one active run.
	TaskRunning TaskState = "running"
	// TaskCompleted finished successfully.
	TaskCompleted TaskState = "completed"
	// TaskFailed finished unsuccessfully.
	TaskFailed TaskState = "failed"
	// TaskCancelled was stopped before completion.
	TaskCancelled TaskState = "cancelled"
)

// Task is the stored state needed by the first migration slice.
type Task struct {
	ID           string
	Title        string
	State        TaskState
	CurrentRunID string
	Revision     int64
	CreatedAt    time.Time
	UpdatedAt    time.Time
}

// TaskEvent records one committed Task revision.
type TaskEvent struct {
	ID         int64
	TaskID     string
	Revision   int64
	Kind       string
	OccurredAt time.Time
}

// TaskStageID returns the current personal workflow stage identifier.
func TaskStageID(state TaskState) string {
	if state == TaskCaptured {
		return "stage:personal:inbox"
	}
	return "stage:personal:" + string(state)
}

// NewTaskID creates one portable Task identifier.
func NewTaskID() (string, error) {
	return newID("task")
}

// CreateTask stores one captured Task and its first event.
func (s *Store) CreateTask(ctx context.Context, id string, title string, now time.Time) (Task, error) {
	if !validTaskID(id) {
		return Task{}, errors.New("invalid task id")
	}
	title = strings.TrimSpace(title)
	if title == "" {
		return Task{}, errors.New("task title cannot be empty")
	}
	if len(title) > 500 {
		return Task{}, errors.New("task title is too long")
	}

	now = now.UTC()
	task := Task{
		ID:        id,
		Title:     title,
		State:     TaskCaptured,
		Revision:  1,
		CreatedAt: now,
		UpdatedAt: now,
	}

	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return Task{}, fmt.Errorf("begin task creation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()

	if _, err := tx.ExecContext(ctx, `
INSERT INTO tasks (
    task_id, title, state, current_run_id, revision, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, NULL, 1, ?, ?)`, id, title, TaskCaptured, millis(now), millis(now)); err != nil {
		return Task{}, fmt.Errorf("insert task: %w", err)
	}
	if err := insertTaskEvent(ctx, tx, task, "task_captured"); err != nil {
		return Task{}, err
	}
	if err := tx.Commit(); err != nil {
		return Task{}, fmt.Errorf("commit task creation: %w", err)
	}
	return task, nil
}

// Task returns one stored Task.
func (s *Store) Task(ctx context.Context, id string) (Task, error) {
	return scanTask(s.db.QueryRowContext(ctx, `
SELECT task_id, title, state, COALESCE(current_run_id, ''), revision,
       created_at_ms, updated_at_ms
FROM tasks
WHERE task_id = ?`, id))
}

// TaskExists reports whether one Task row is committed.
func (s *Store) TaskExists(ctx context.Context, id string) (bool, error) {
	if !validTaskID(id) {
		return false, errors.New("invalid task id")
	}
	var exists bool
	if err := s.db.QueryRowContext(
		ctx,
		"SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id = ?)",
		id,
	).Scan(&exists); err != nil {
		return false, fmt.Errorf("inspect task existence: %w", err)
	}
	return exists, nil
}

// StartTask assigns one current run to a captured Task.
func (s *Store) StartTask(
	ctx context.Context,
	taskID string,
	runID string,
	now time.Time,
) (Task, error) {
	if runID == "" {
		return Task{}, errors.New("run id cannot be empty")
	}
	return s.changeTask(ctx, taskID, "", runID, TaskCaptured, TaskRunning, "task_started", now)
}

// FinishTask commits a terminal state for the current run.
func (s *Store) FinishTask(
	ctx context.Context,
	taskID string,
	runID string,
	state TaskState,
	now time.Time,
) (Task, error) {
	if state != TaskCompleted && state != TaskFailed && state != TaskCancelled {
		return Task{}, errors.New("finish state must be terminal")
	}
	return s.changeTask(ctx, taskID, runID, "", TaskRunning, state, "task_"+string(state), now)
}

// TaskEvents returns stored events after one event identifier.
func (s *Store) TaskEvents(ctx context.Context, taskID string, after int64) ([]TaskEvent, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT event_id, task_id, task_revision, kind, occurred_at_ms
FROM task_events
WHERE task_id = ? AND event_id > ?
ORDER BY event_id`, taskID, after)
	if err != nil {
		return nil, fmt.Errorf("query task events: %w", err)
	}
	defer rows.Close()

	events := make([]TaskEvent, 0)
	for rows.Next() {
		var event TaskEvent
		var occurredAt int64
		if err := rows.Scan(
			&event.ID,
			&event.TaskID,
			&event.Revision,
			&event.Kind,
			&occurredAt,
		); err != nil {
			return nil, fmt.Errorf("scan task event: %w", err)
		}
		event.OccurredAt = fromMillis(occurredAt)
		events = append(events, event)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("read task events: %w", err)
	}
	return events, nil
}

func (s *Store) changeTask(
	ctx context.Context,
	taskID string,
	expectedRunID string,
	nextRunID string,
	expectedState TaskState,
	nextState TaskState,
	eventKind string,
	now time.Time,
) (Task, error) {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return Task{}, fmt.Errorf("begin task change: %w", err)
	}
	defer func() { _ = tx.Rollback() }()

	result, err := tx.ExecContext(ctx, `
UPDATE tasks
SET state = ?, current_run_id = NULLIF(?, ''), revision = revision + 1, updated_at_ms = ?
WHERE task_id = ? AND state = ? AND COALESCE(current_run_id, '') = ?`,
		nextState, nextRunID, millis(now), taskID, expectedState, expectedRunID)
	if err != nil {
		return Task{}, fmt.Errorf("change task: %w", err)
	}
	changed, err := result.RowsAffected()
	if err != nil {
		return Task{}, fmt.Errorf("inspect task change: %w", err)
	}
	if changed == 0 {
		var exists int
		if err := tx.QueryRowContext(ctx, "SELECT 1 FROM tasks WHERE task_id = ?", taskID).Scan(&exists); err != nil {
			if errors.Is(err, sql.ErrNoRows) {
				return Task{}, ErrTaskNotFound
			}
			return Task{}, fmt.Errorf("inspect task: %w", err)
		}
		return Task{}, ErrStaleRun
	}

	task, err := scanTask(tx.QueryRowContext(ctx, `
SELECT task_id, title, state, COALESCE(current_run_id, ''), revision,
       created_at_ms, updated_at_ms
FROM tasks
WHERE task_id = ?`, taskID))
	if err != nil {
		return Task{}, err
	}
	if err := insertTaskEvent(ctx, tx, task, eventKind); err != nil {
		return Task{}, err
	}
	if err := tx.Commit(); err != nil {
		return Task{}, fmt.Errorf("commit task change: %w", err)
	}
	return task, nil
}

type rowScanner interface {
	Scan(dest ...any) error
}

func scanTask(row rowScanner) (Task, error) {
	var task Task
	var createdAt int64
	var updatedAt int64
	if err := row.Scan(
		&task.ID,
		&task.Title,
		&task.State,
		&task.CurrentRunID,
		&task.Revision,
		&createdAt,
		&updatedAt,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return Task{}, ErrTaskNotFound
		}
		return Task{}, fmt.Errorf("scan task: %w", err)
	}
	task.CreatedAt = fromMillis(createdAt)
	task.UpdatedAt = fromMillis(updatedAt)
	return task, nil
}

func insertTaskEvent(ctx context.Context, tx *sql.Tx, task Task, kind string) error {
	if _, err := tx.ExecContext(ctx, `
INSERT INTO task_events (task_id, task_revision, kind, occurred_at_ms)
VALUES (?, ?, ?, ?)`, task.ID, task.Revision, kind, millis(task.UpdatedAt)); err != nil {
		return fmt.Errorf("insert task event: %w", err)
	}
	return nil
}

func newID(prefix string) (string, error) {
	var value [16]byte
	if _, err := rand.Read(value[:]); err != nil {
		return "", fmt.Errorf("create %s id: %w", prefix, err)
	}
	return prefix + ":" + hex.EncodeToString(value[:]), nil
}

func validTaskID(id string) bool {
	value, ok := strings.CutPrefix(id, "task:")
	if !ok || len(value) != 32 {
		return false
	}
	decoded, err := hex.DecodeString(value)
	return err == nil && hex.EncodeToString(decoded) == value
}

func millis(value time.Time) int64 {
	return value.UnixMilli()
}

func fromMillis(value int64) time.Time {
	return time.UnixMilli(value).UTC()
}
