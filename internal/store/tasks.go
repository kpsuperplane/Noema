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

	"github.com/uptrace/bun"
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
	bun.BaseModel                 `bun:"table:tasks"`
	ID                            string `bun:"task_id,pk"`
	ProjectID                     string
	Title                         string
	State                         TaskState
	CurrentRunID                  string
	Revision                      int64
	ExecutorAgentID               string
	ExecutorAcpConnectionRevision *int64
	CwdOverride                   *string
	ScheduledFor                  *time.Time `bun:"scheduled_for_ms"`
	ScheduleTimeZone              string
	MissedRunPolicy               string
	ScheduleProcessedAt           *time.Time `bun:"schedule_processed_at_ms"`
	RecurrenceID                  string
	RecurrenceRevision            *int64
	RecurrenceScheduledFor        *time.Time `bun:"recurrence_scheduled_for_ms"`
	Generation                    int64
	StageKey                      string
	ActiveGateID                  string
	CompletedAt                   *time.Time `bun:"completed_at_ms"`
	CancelledAt                   *time.Time `bun:"cancelled_at_ms"`
	ExecutionComplexity           string
	Source                        ArtifactSource `bun:"embed:source_"`
	SourceToolCallID              string
	SourceClientTimeZone          string
	CreatedAt                     time.Time `bun:"created_at_ms"`
	UpdatedAt                     time.Time `bun:"updated_at_ms"`
}

// TaskEvent records one committed Task revision.
type TaskEvent struct {
	bun.BaseModel `bun:"table:task_events"`
	ID            int64 `bun:"event_id,pk"`
	TaskID        string
	Revision      int64 `bun:"task_revision"`
	Kind          string
	OccurredAt    time.Time `bun:"occurred_at_ms"`
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
func (s *Store) CreateTask(ctx context.Context, id string, title string, correlationID string, now time.Time) (Task, error) {
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
	if strings.TrimSpace(correlationID) == "" {
		return Task{}, errors.New("task correlation id cannot be empty")
	}

	now = now.UTC()
	task := Task{
		ID:              id,
		Title:           title,
		State:           TaskCaptured,
		Revision:        1,
		Generation:      1,
		StageKey:        "inbox",
		ExecutorAgentID: TaskExecutorAgentID,
		CreatedAt:       now,
		UpdatedAt:       now,
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
	if _, err := insertWorkEvent(ctx, tx, "workspace:personal", "", task.ID, "", task.Revision,
		"task.captured", "actor:human:local", nil, correlationID,
		map[string]any{"v": 1, "revision": task.Revision}, task.UpdatedAt); err != nil {
		return Task{}, err
	}
	if err := tx.Commit(); err != nil {
		return Task{}, fmt.Errorf("commit task creation: %w", err)
	}
	s.NotifyWork()
	return task, nil
}

// Task returns one stored Task.
func (s *Store) Task(ctx context.Context, id string) (Task, error) {
	return taskTx(ctx, s.db, id)
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
	events := make([]TaskEvent, 0)
	err := s.db.NewSelect().Model(&events).Where("task_id = ? AND event_id > ?", taskID, after).
		Order("event_id").Scan(ctx)
	return events, err
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
	if expectedState == TaskCaptured && nextState == TaskRunning {
		var scheduled bool
		if err := tx.QueryRowContext(ctx, `SELECT scheduled_for_ms IS NOT NULL
AND schedule_processed_at_ms IS NULL FROM tasks WHERE task_id = ?`, taskID).Scan(&scheduled); err != nil {
			if errors.Is(err, sql.ErrNoRows) {
				return Task{}, ErrTaskNotFound
			}
			return Task{}, fmt.Errorf("inspect Task schedule: %w", err)
		}
		if scheduled {
			return Task{}, ErrInvalidTransition
		}
	}

	nextStage := map[TaskState]string{TaskCaptured: "inbox", TaskRunning: "doing", TaskCompleted: "done", TaskFailed: "waiting", TaskCancelled: "cancelled"}[nextState]
	var completedAt, cancelledAt any
	if nextState == TaskCompleted {
		completedAt = millis(now)
	}
	if nextState == TaskCancelled {
		cancelledAt = millis(now)
	}
	result, err := tx.ExecContext(ctx, `
UPDATE tasks
SET state = ?, current_run_id = NULLIF(?, ''), stage_key = ?, completed_at_ms = ?,
    cancelled_at_ms = ?, revision = revision + 1, updated_at_ms = ?
WHERE task_id = ? AND state = ? AND COALESCE(current_run_id, '') = ?`,
		nextState, nextRunID, nextStage, completedAt, cancelledAt, millis(now), taskID, expectedState, expectedRunID)
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

	task, err := taskTx(ctx, tx, taskID)
	if err != nil {
		return Task{}, err
	}
	if err := insertTaskEvent(ctx, tx, task, eventKind); err != nil {
		return Task{}, err
	}
	if _, err := insertWorkEvent(ctx, tx, "workspace:personal", "", task.ID, firstNonemptyStore(expectedRunID, nextRunID), task.Revision,
		strings.ReplaceAll(eventKind, "_", "."), "actor:system:runtime", nil, "correlation:task:"+task.ID,
		map[string]any{"v": 1, "revision": task.Revision}, task.UpdatedAt); err != nil {
		return Task{}, err
	}
	if err := tx.Commit(); err != nil {
		return Task{}, fmt.Errorf("commit task change: %w", err)
	}
	s.NotifyWork()
	return task, nil
}

func firstNonemptyStore(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return ""
}

type rowScanner interface {
	Scan(dest ...any) error
}

func taskTx(ctx context.Context, db bun.IDB, id string) (Task, error) {
	var task Task
	err := db.NewSelect().Model(&task).Where("task_id = ?", id).Scan(ctx)
	if errors.Is(err, sql.ErrNoRows) {
		return Task{}, ErrTaskNotFound
	}
	return task, err
}

func nullStringPointer(value sql.NullString) *string {
	if !value.Valid {
		return nil
	}
	return &value.String
}

func nullIntPointer(value sql.NullInt64) *int64 {
	if !value.Valid {
		return nil
	}
	return &value.Int64
}

func nullTimePointer(value sql.NullInt64) *time.Time {
	if !value.Valid {
		return nil
	}
	instant := fromMillis(value.Int64)
	return &instant
}

func insertTaskEvent(ctx context.Context, tx bun.Tx, task Task, kind string) error {
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
