package store

import (
	"context"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/schedule"
)

// RecurrenceLifecycle is the durable state of recurring Task authority.
type RecurrenceLifecycle string

const (
	RecurrenceActive RecurrenceLifecycle = "active"
	RecurrencePaused RecurrenceLifecycle = "paused"
	RecurrenceEnded  RecurrenceLifecycle = "ended"
)

// TaskCreateOptions contains Task placement and optional timing.
type TaskCreateOptions struct {
	ProjectID, ExecutorAgentID string
	CwdOverride                *string
	Schedule                   *schedule.Schedule
}

// TaskCommandResult is one committed scheduled Task command.
type TaskCommandResult struct {
	Task                 Task
	Event                WorkEvent
	RecurrenceID         string
	ObsoleteRecurrenceID string
	Replayed             bool
}

// LockTaskSchedules prevents a scheduler from observing a committed document publication gap.
func (s *Store) LockTaskSchedules() func() {
	s.taskScheduleMu.Lock()
	return s.taskScheduleMu.Unlock
}

// TaskCommand identifies one repeat-safe Task schedule command.
type TaskCommand struct {
	Name, ClientMutationID, RequestDigest, CorrelationID string
}

// TaskRecurrence is the continuing authority for future Task occurrences.
type TaskRecurrence struct {
	ID                            string
	WorkspaceID                   string
	ProjectID                     string
	Title                         string
	ExecutorAgentID               string
	ExecutorAcpConnectionRevision *int64
	CwdOverride                   *string
	StartsAt                      time.Time
	CronExpression                string
	TimeZone                      string
	MissedRunPolicy               schedule.MissedRunPolicy
	OverlapPolicy                 schedule.OverlapPolicy
	Lifecycle                     RecurrenceLifecycle
	Revision                      int64
	NextRunAt                     *time.Time
	PendingCoalescedAt            *time.Time
	CreatedAt                     time.Time
	UpdatedAt                     time.Time
}

// RecurrenceOccurrence is one immutable or coalesced cron slot result.
type RecurrenceOccurrence struct {
	RecurrenceRevision int64
	ScheduledFor       time.Time
	LocalSlot          string
	Trigger            string
	Resolution         string
	TaskID             string
	CreatedAt          time.Time
}

// RecurrenceChanges contains optional future-authority replacements.
type RecurrenceChanges struct {
	Title, ProjectID, CronExpression, TimeZone *string
	SetProject                                 bool
	StartsAt                                   *time.Time
	MissedRunPolicy                            *schedule.MissedRunPolicy
	OverlapPolicy                              *schedule.OverlapPolicy
}

// DueTask identifies one new occurrence whose TASK.md must copy the template.
type DueTask struct{ TaskID, RecurrenceID string }

// CreateTaskWithOptions stores one captured Task with placement and timing.
func (s *Store) CreateTaskWithOptions(
	ctx context.Context, id, title string, command TaskCommand, options TaskCreateOptions, now time.Time,
) (TaskCommandResult, error) {
	if !validTaskID(id) || validateTaskCommand(command) != nil {
		return TaskCommandResult{}, errors.New("invalid Task create")
	}
	title = strings.TrimSpace(title)
	if title == "" {
		return TaskCommandResult{}, errors.New("task title cannot be empty")
	}
	if len(title) > 500 {
		return TaskCommandResult{}, errors.New("task title is too long")
	}
	if options.ExecutorAgentID == "" {
		options.ExecutorAgentID = TaskExecutorAgentID
	}
	if options.Schedule != nil {
		normalized, err := schedule.Normalize(*options.Schedule, now)
		if err != nil {
			return TaskCommandResult{}, err
		}
		options.Schedule = &normalized
	}
	if options.CwdOverride != nil {
		value := strings.TrimSpace(*options.CwdOverride)
		if value == "" || value != *options.CwdOverride || !filepath.IsAbs(value) {
			return TaskCommandResult{}, errors.New("Task cwdOverride must be absolute")
		}
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return TaskCommandResult{}, fmt.Errorf("begin Task creation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if replay, found, err := lookupTaskReceiptTx(ctx, tx, command); err != nil {
		return TaskCommandResult{}, err
	} else if found {
		return replay, nil
	}
	if err := validateTaskProjectTx(ctx, tx, options.ProjectID); err != nil {
		return TaskCommandResult{}, err
	}
	executorRevision, err := validateTaskExecutorTx(ctx, tx, options.ExecutorAgentID)
	if err != nil {
		return TaskCommandResult{}, err
	}
	task := Task{ID: id, ProjectID: options.ProjectID, Title: title, State: TaskCaptured,
		Revision: 1, ExecutorAgentID: options.ExecutorAgentID,
		ExecutorAcpConnectionRevision: executorRevision, CwdOverride: cloneString(options.CwdOverride),
		CreatedAt: now, UpdatedAt: now}
	applyScheduleToTask(&task, options.Schedule)
	if _, err := tx.ExecContext(ctx, `INSERT INTO tasks
(task_id, project_id, title, state, current_run_id, revision, executor_agent_id,
 executor_acp_connection_revision, cwd_override, scheduled_for_ms, schedule_time_zone,
 missed_run_policy, recurrence_scheduled_for_ms, created_at_ms, updated_at_ms)
VALUES (?, NULLIF(?, ''), ?, 'captured', NULL, 1, ?, ?, ?, ?, NULLIF(?, ''),
 NULLIF(?, ''), ?, ?, ?)`, task.ID, task.ProjectID, task.Title, task.ExecutorAgentID,
		nullableInt(task.ExecutorAcpConnectionRevision), nullableString(task.CwdOverride),
		nullableTime(task.ScheduledFor), task.ScheduleTimeZone, task.MissedRunPolicy,
		nullableTime(task.RecurrenceScheduledFor), millis(now), millis(now)); err != nil {
		return TaskCommandResult{}, fmt.Errorf("insert Task: %w", err)
	}
	recurrenceID, err := createTaskRecurrenceTx(ctx, tx, &task, options.Schedule, now)
	if err != nil {
		return TaskCommandResult{}, err
	}
	if recurrenceID != "" {
		task.RecurrenceID = recurrenceID
		revision := int64(1)
		task.RecurrenceRevision = &revision
	}
	if err := insertTaskEvent(ctx, tx, task, "task_captured"); err != nil {
		return TaskCommandResult{}, err
	}
	event, err := insertWorkEvent(ctx, tx, personalWorkspaceIDStore, task.ProjectID, task.ID, "", 1,
		"task.captured", "actor:human:local", nil, command.CorrelationID,
		map[string]any{"v": 1, "revision": int64(1)}, now)
	if err != nil {
		return TaskCommandResult{}, err
	}
	result := TaskCommandResult{Task: task, Event: event, RecurrenceID: recurrenceID}
	if err := storeTaskReceiptTx(ctx, tx, command, result, now); err != nil {
		return TaskCommandResult{}, err
	}
	if err := tx.Commit(); err != nil {
		return TaskCommandResult{}, fmt.Errorf("commit Task creation: %w", err)
	}
	return result, nil
}

// SetTaskSchedule creates or replaces future timing at one Task fence.
func (s *Store) SetTaskSchedule(
	ctx context.Context, taskID string, expectedRevision int64, value schedule.Schedule,
	requireExisting bool, command TaskCommand, now time.Time,
) (TaskCommandResult, error) {
	normalized, err := schedule.Normalize(value, now)
	if err != nil {
		return TaskCommandResult{}, err
	}
	value = normalized
	return s.taskScheduleCommand(ctx, taskID, expectedRevision, command, now,
		func(tx *sql.Tx, task *Task) (string, string, error) {
			existing := task.ScheduledFor != nil
			if existing != requireExisting || task.State != TaskCaptured || task.ScheduleProcessedAt != nil {
				return "", "", ErrInvalidTransition
			}
			obsolete := task.RecurrenceID
			if err := deleteTaskRecurrenceTx(ctx, tx, obsolete); err != nil {
				return "", "", err
			}
			applyScheduleToTask(task, &value)
			task.ScheduleProcessedAt = nil
			task.RecurrenceID = ""
			task.RecurrenceRevision = nil
			if _, err := tx.ExecContext(ctx, `UPDATE tasks SET scheduled_for_ms = ?,
schedule_time_zone = ?, missed_run_policy = ?, schedule_processed_at_ms = NULL,
recurrence_id = NULL, recurrence_revision = NULL, recurrence_scheduled_for_ms = ?,
revision = ?, updated_at_ms = ? WHERE task_id = ? AND revision = ?`,
				nullableTime(task.ScheduledFor), task.ScheduleTimeZone, task.MissedRunPolicy,
				nullableTime(task.RecurrenceScheduledFor), task.Revision, millis(task.UpdatedAt),
				task.ID, expectedRevision); err != nil {
				return "", "", err
			}
			recurrenceID, err := createTaskRecurrenceTx(ctx, tx, task, &value, now.UTC())
			if recurrenceID != "" {
				task.RecurrenceID = recurrenceID
				revision := int64(1)
				task.RecurrenceRevision = &revision
			}
			return recurrenceID, obsolete, err
		})
}

// UnscheduleTask removes future timing at one Task fence.
func (s *Store) UnscheduleTask(
	ctx context.Context, taskID string, expectedRevision int64, command TaskCommand, now time.Time,
) (TaskCommandResult, error) {
	return s.taskScheduleCommand(ctx, taskID, expectedRevision, command, now,
		func(tx *sql.Tx, task *Task) (string, string, error) {
			if task.ScheduledFor == nil || task.State != TaskCaptured || task.ScheduleProcessedAt != nil {
				return "", "", ErrInvalidTransition
			}
			obsolete := task.RecurrenceID
			if err := deleteTaskRecurrenceTx(ctx, tx, obsolete); err != nil {
				return "", "", err
			}
			clearTaskSchedule(task)
			_, err := tx.ExecContext(ctx, `UPDATE tasks SET scheduled_for_ms = NULL,
schedule_time_zone = NULL, missed_run_policy = NULL, schedule_processed_at_ms = NULL,
recurrence_id = NULL, recurrence_revision = NULL, recurrence_scheduled_for_ms = NULL,
revision = ?, updated_at_ms = ? WHERE task_id = ? AND revision = ?`, task.Revision,
				millis(task.UpdatedAt), task.ID, expectedRevision)
			return "", obsolete, err
		})
}

// RunScheduledTaskNow releases one scheduled Task for the future executor.
func (s *Store) RunScheduledTaskNow(
	ctx context.Context, taskID string, expectedRevision int64, command TaskCommand, now time.Time,
) (TaskCommandResult, error) {
	return s.taskScheduleCommand(ctx, taskID, expectedRevision, command, now,
		func(tx *sql.Tx, task *Task) (string, string, error) {
			if task.ScheduledFor == nil || task.ScheduleProcessedAt != nil || task.State != TaskCaptured {
				return "", "", ErrInvalidTransition
			}
			processed := now.UTC()
			task.ScheduleProcessedAt = &processed
			if task.RecurrenceID != "" {
				if _, err := tx.ExecContext(ctx, `UPDATE task_recurrence_occurrences
SET trigger_kind = 'manual' WHERE task_id = ?`, task.ID); err != nil {
					return "", "", err
				}
			}
			_, err := tx.ExecContext(ctx, `UPDATE tasks SET schedule_processed_at_ms = ?,
revision = ?, updated_at_ms = ? WHERE task_id = ? AND revision = ?`, millis(processed),
				task.Revision, millis(task.UpdatedAt), task.ID, expectedRevision)
			return task.RecurrenceID, "", err
		})
}

func (s *Store) taskScheduleCommand(
	ctx context.Context, taskID string, expectedRevision int64, command TaskCommand, now time.Time,
	change func(*sql.Tx, *Task) (string, string, error),
) (TaskCommandResult, error) {
	if !validTaskID(taskID) || expectedRevision <= 0 || validateTaskCommand(command) != nil {
		return TaskCommandResult{}, errors.New("invalid scheduled Task command")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return TaskCommandResult{}, err
	}
	defer func() { _ = tx.Rollback() }()
	if replay, found, err := lookupTaskReceiptTx(ctx, tx, command); err != nil {
		return TaskCommandResult{}, err
	} else if found {
		return replay, nil
	}
	task, err := scanTask(tx.QueryRowContext(ctx, taskSelect+" WHERE task_id = ?", taskID))
	if err != nil {
		return TaskCommandResult{}, err
	}
	if task.Revision != expectedRevision {
		return TaskCommandResult{}, ErrStaleRevision
	}
	task.Revision++
	task.UpdatedAt = now.UTC()
	recurrenceID, obsolete, err := change(tx, &task)
	if err != nil {
		return TaskCommandResult{}, err
	}
	if err := insertTaskEvent(ctx, tx, task, "task_updated"); err != nil {
		return TaskCommandResult{}, err
	}
	event, err := insertWorkEvent(ctx, tx, personalWorkspaceIDStore, task.ProjectID, task.ID, "",
		task.Revision, "task.updated", "actor:human:local", nil, command.CorrelationID,
		map[string]any{"v": 1, "revision": task.Revision, "changed_fields": []string{"schedule"}}, task.UpdatedAt)
	if err != nil {
		return TaskCommandResult{}, err
	}
	result := TaskCommandResult{Task: task, Event: event, RecurrenceID: recurrenceID,
		ObsoleteRecurrenceID: obsolete}
	if err := storeTaskReceiptTx(ctx, tx, command, result, now); err != nil {
		return TaskCommandResult{}, err
	}
	if err := tx.Commit(); err != nil {
		return TaskCommandResult{}, err
	}
	return result, nil
}

func createTaskRecurrenceTx(
	ctx context.Context, tx *sql.Tx, task *Task, value *schedule.Schedule, now time.Time,
) (string, error) {
	if value == nil || value.Recurrence == nil {
		return "", nil
	}
	id, err := newID("recurrence")
	if err != nil {
		return "", err
	}
	next, err := schedule.NextAtOrAfter(value.Recurrence.CronExpression, value.TimeZone,
		value.ScheduledFor.Add(time.Second))
	if err != nil {
		return "", err
	}
	if _, err := tx.ExecContext(ctx, `INSERT INTO task_recurrences
(recurrence_id, workspace_id, project_id, title, executor_agent_id,
 executor_acp_connection_revision, cwd_override, starts_at_ms, cron_expression,
 time_zone, missed_run_policy, overlap_policy, lifecycle, revision, next_run_at_ms,
 created_at_ms, updated_at_ms)
VALUES (?, 'workspace:personal', NULLIF(?, ''), ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', 1, ?, ?, ?)`,
		id, task.ProjectID, task.Title, task.ExecutorAgentID,
		nullableInt(task.ExecutorAcpConnectionRevision), nullableString(task.CwdOverride),
		millis(value.Recurrence.StartsAt), value.Recurrence.CronExpression, value.TimeZone,
		string(value.MissedRunPolicy), string(value.Recurrence.OverlapPolicy), millis(next),
		millis(now), millis(now)); err != nil {
		return "", fmt.Errorf("insert Task recurrence: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `UPDATE tasks SET recurrence_id = ?, recurrence_revision = 1,
recurrence_scheduled_for_ms = scheduled_for_ms WHERE task_id = ?`, id, task.ID); err != nil {
		return "", err
	}
	slot, err := schedule.LocalSlot(value.ScheduledFor, value.TimeZone)
	if err != nil {
		return "", err
	}
	occurrenceID, err := newID("occurrence")
	if err != nil {
		return "", err
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO task_recurrence_occurrences
(occurrence_id, recurrence_id, recurrence_revision, scheduled_for_ms, local_slot,
 trigger_kind, resolution, task_id, created_at_ms)
VALUES (?, ?, 1, ?, ?, 'scheduled', 'materialized', ?, ?)`, occurrenceID, id,
		millis(value.ScheduledFor), slot, task.ID, millis(now))
	return id, err
}

func deleteTaskRecurrenceTx(ctx context.Context, tx *sql.Tx, id string) error {
	if id == "" {
		return nil
	}
	if _, err := tx.ExecContext(ctx,
		"DELETE FROM task_recurrence_occurrences WHERE recurrence_id = ?", id); err != nil {
		return err
	}
	_, err := tx.ExecContext(ctx, "DELETE FROM task_recurrences WHERE recurrence_id = ?", id)
	return err
}

// TaskRecurrence returns one recurring authority.
func (s *Store) TaskRecurrence(ctx context.Context, id string) (TaskRecurrence, error) {
	return scanTaskRecurrence(s.db.QueryRowContext(ctx, recurrenceSelect+" WHERE recurrence_id = ?", id))
}

// TaskRecurrences returns current authorities in one optional Project scope.
func (s *Store) TaskRecurrences(ctx context.Context, projectID string, limit int) ([]TaskRecurrence, error) {
	if limit <= 0 || limit > 100 {
		return nil, errors.New("invalid recurrence limit")
	}
	rows, err := s.db.QueryContext(ctx, recurrenceSelect+`
 WHERE lifecycle != 'ended' AND (? = '' OR project_id = ?)
 ORDER BY CASE lifecycle WHEN 'active' THEN 0 ELSE 1 END, next_run_at_ms, recurrence_id LIMIT ?`,
		projectID, projectID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	values := make([]TaskRecurrence, 0)
	for rows.Next() {
		value, err := scanTaskRecurrence(rows)
		if err != nil {
			return nil, err
		}
		values = append(values, value)
	}
	return values, rows.Err()
}

// TaskRecurrenceOccurrences returns newest slot history.
func (s *Store) TaskRecurrenceOccurrences(ctx context.Context, id string, limit int) ([]RecurrenceOccurrence, error) {
	if limit <= 0 || limit > 100 {
		return nil, errors.New("invalid occurrence limit")
	}
	rows, err := s.db.QueryContext(ctx, `SELECT recurrence_revision, scheduled_for_ms,
local_slot, trigger_kind, resolution, COALESCE(task_id, ''), created_at_ms
FROM task_recurrence_occurrences WHERE recurrence_id = ?
ORDER BY created_at_ms DESC, occurrence_id DESC LIMIT ?`, id, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	values := make([]RecurrenceOccurrence, 0)
	for rows.Next() {
		var value RecurrenceOccurrence
		var scheduled, created int64
		if err := rows.Scan(&value.RecurrenceRevision, &scheduled, &value.LocalSlot,
			&value.Trigger, &value.Resolution, &value.TaskID, &created); err != nil {
			return nil, err
		}
		value.ScheduledFor, value.CreatedAt = fromMillis(scheduled), fromMillis(created)
		values = append(values, value)
	}
	return values, rows.Err()
}

// UpdateTaskRecurrence changes future recurring authority at one revision.
func (s *Store) UpdateTaskRecurrence(
	ctx context.Context, id string, expectedRevision int64, changes RecurrenceChanges,
	command TaskCommand, now time.Time,
) (TaskCommandResult, error) {
	return s.recurrenceCommand(ctx, id, expectedRevision, command, now,
		func(tx *sql.Tx, value *TaskRecurrence) error {
			if value.Lifecycle == RecurrenceEnded {
				return ErrInvalidTransition
			}
			if changes.SetProject {
				projectID := ""
				if changes.ProjectID != nil {
					projectID = *changes.ProjectID
				}
				if err := validateTaskProjectTx(ctx, tx, projectID); err != nil {
					return err
				}
				value.ProjectID = projectID
			}
			if changes.Title != nil {
				value.Title = strings.TrimSpace(*changes.Title)
				if value.Title == "" || len(value.Title) > 500 {
					return errors.New("invalid recurrence title")
				}
			}
			timingChanged := changes.StartsAt != nil || changes.CronExpression != nil || changes.TimeZone != nil
			if changes.StartsAt != nil {
				value.StartsAt = changes.StartsAt.UTC()
			}
			if changes.CronExpression != nil {
				value.CronExpression = *changes.CronExpression
			}
			if changes.TimeZone != nil {
				value.TimeZone = *changes.TimeZone
			}
			if changes.MissedRunPolicy != nil {
				value.MissedRunPolicy = *changes.MissedRunPolicy
			}
			if changes.OverlapPolicy != nil {
				value.OverlapPolicy = *changes.OverlapPolicy
			}
			if timingChanged {
				start := value.StartsAt
				if start.Before(now) {
					start = now
				}
				next, err := schedule.NextAtOrAfter(value.CronExpression, value.TimeZone, start)
				if err != nil {
					return err
				}
				value.NextRunAt = &next
			}
			_, err := tx.ExecContext(ctx, `UPDATE task_recurrences SET project_id = NULLIF(?, ''),
title = ?, starts_at_ms = ?, cron_expression = ?, time_zone = ?, missed_run_policy = ?,
overlap_policy = ?, next_run_at_ms = ?, revision = ?, updated_at_ms = ?
WHERE recurrence_id = ? AND revision = ?`, value.ProjectID, value.Title, millis(value.StartsAt),
				value.CronExpression, value.TimeZone, string(value.MissedRunPolicy),
				string(value.OverlapPolicy), nullableTime(value.NextRunAt), value.Revision,
				millis(value.UpdatedAt), value.ID, expectedRevision)
			return err
		})
}

// SetTaskRecurrenceLifecycle changes pause, resume, or end state.
func (s *Store) SetTaskRecurrenceLifecycle(
	ctx context.Context, id string, expectedRevision int64, next RecurrenceLifecycle,
	command TaskCommand, now time.Time,
) (TaskCommandResult, error) {
	return s.recurrenceCommand(ctx, id, expectedRevision, command, now,
		func(tx *sql.Tx, value *TaskRecurrence) error {
			valid := value.Lifecycle == RecurrenceActive && (next == RecurrencePaused || next == RecurrenceEnded) ||
				value.Lifecycle == RecurrencePaused && (next == RecurrenceActive || next == RecurrenceEnded)
			if !valid {
				return ErrInvalidTransition
			}
			if next == RecurrenceActive && value.MissedRunPolicy == schedule.MissedRunSkip &&
				value.NextRunAt != nil && value.NextRunAt.Before(now) {
				slotAuthority := *value
				slotAuthority.Revision = expectedRevision
				if err := recordOccurrenceTx(ctx, tx, &slotAuthority, *value.NextRunAt, "scheduled", "skipped", "", now); err != nil {
					return err
				}
				nextRun, err := schedule.NextAtOrAfter(value.CronExpression, value.TimeZone, now.Add(time.Second))
				if err != nil {
					return err
				}
				value.NextRunAt = &nextRun
			}
			value.Lifecycle = next
			if next == RecurrenceEnded {
				value.NextRunAt = nil
			}
			_, err := tx.ExecContext(ctx, `UPDATE task_recurrences SET lifecycle = ?,
next_run_at_ms = ?, revision = ?, updated_at_ms = ? WHERE recurrence_id = ? AND revision = ?`,
				string(next), nullableTime(value.NextRunAt), value.Revision, millis(value.UpdatedAt),
				value.ID, expectedRevision)
			return err
		})
}

// SkipTaskRecurrenceNext resolves and advances the next cron slot.
func (s *Store) SkipTaskRecurrenceNext(
	ctx context.Context, id string, expectedRevision int64, command TaskCommand, now time.Time,
) (TaskCommandResult, error) {
	return s.recurrenceCommand(ctx, id, expectedRevision, command, now,
		func(tx *sql.Tx, value *TaskRecurrence) error {
			if value.Lifecycle != RecurrenceActive || value.NextRunAt == nil {
				return ErrInvalidTransition
			}
			due := *value.NextRunAt
			slotAuthority := *value
			slotAuthority.Revision = expectedRevision
			if err := recordOccurrenceTx(ctx, tx, &slotAuthority, due, "scheduled", "skipped", "", now); err != nil {
				return err
			}
			next, err := schedule.NextAtOrAfter(value.CronExpression, value.TimeZone, due.Add(time.Second))
			if err != nil {
				return err
			}
			value.NextRunAt = &next
			_, err = tx.ExecContext(ctx, `UPDATE task_recurrences SET next_run_at_ms = ?,
revision = ?, updated_at_ms = ? WHERE recurrence_id = ? AND revision = ?`, millis(next),
				value.Revision, millis(value.UpdatedAt), value.ID, expectedRevision)
			return err
		})
}

// RunTaskRecurrenceNow creates one manual occurrence when no child is active.
func (s *Store) RunTaskRecurrenceNow(
	ctx context.Context, id, taskID string, expectedRevision int64, command TaskCommand, now time.Time,
) (TaskCommandResult, error) {
	if expectedRevision <= 0 || !validTaskID(taskID) || validateTaskCommand(command) != nil {
		return TaskCommandResult{}, ErrStaleRevision
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return TaskCommandResult{}, err
	}
	defer func() { _ = tx.Rollback() }()
	if replay, found, err := lookupTaskReceiptTx(ctx, tx, command); err != nil {
		return TaskCommandResult{}, err
	} else if found {
		return replay, nil
	}
	recurrence, err := scanTaskRecurrence(tx.QueryRowContext(ctx, recurrenceSelect+" WHERE recurrence_id = ?", id))
	if err != nil {
		return TaskCommandResult{}, err
	}
	if recurrence.Revision != expectedRevision {
		return TaskCommandResult{}, ErrStaleRevision
	}
	active, err := recurrenceHasActiveTaskTx(ctx, tx, id)
	if err != nil {
		return TaskCommandResult{}, err
	}
	if recurrence.Lifecycle == RecurrenceEnded || active {
		return TaskCommandResult{}, ErrInvalidTransition
	}
	task, event, err := materializeOccurrenceTx(ctx, tx, &recurrence, taskID, now.UTC(), "manual", false,
		"actor:human:local", command.CorrelationID, now.UTC())
	if err != nil {
		return TaskCommandResult{}, err
	}
	result := TaskCommandResult{Task: task, Event: event, RecurrenceID: id}
	if err := storeTaskReceiptTx(ctx, tx, command, result, now); err != nil {
		return TaskCommandResult{}, err
	}
	if err := tx.Commit(); err != nil {
		return TaskCommandResult{}, err
	}
	return result, nil
}

func (s *Store) recurrenceCommand(
	ctx context.Context, id string, expectedRevision int64, command TaskCommand, now time.Time,
	change func(*sql.Tx, *TaskRecurrence) error,
) (TaskCommandResult, error) {
	if expectedRevision <= 0 || validateTaskCommand(command) != nil {
		return TaskCommandResult{}, errors.New("invalid recurrence command")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return TaskCommandResult{}, err
	}
	defer func() { _ = tx.Rollback() }()
	if replay, found, err := lookupTaskReceiptTx(ctx, tx, command); err != nil {
		return TaskCommandResult{}, err
	} else if found {
		return replay, nil
	}
	value, err := scanTaskRecurrence(tx.QueryRowContext(ctx, recurrenceSelect+" WHERE recurrence_id = ?", id))
	if err != nil {
		return TaskCommandResult{}, err
	}
	if value.Revision != expectedRevision {
		return TaskCommandResult{}, ErrStaleRevision
	}
	value.Revision++
	value.UpdatedAt = now.UTC()
	if err := change(tx, &value); err != nil {
		return TaskCommandResult{}, err
	}
	task, err := latestRecurrenceTaskTx(ctx, tx, id)
	if err != nil {
		return TaskCommandResult{}, err
	}
	event, err := insertWorkEvent(ctx, tx, personalWorkspaceIDStore, value.ProjectID, task.ID, "",
		value.Revision, "task.recurrence_changed", "actor:human:local", nil, command.CorrelationID,
		map[string]any{"v": 1, "recurrence_id": id, "revision": value.Revision}, value.UpdatedAt)
	if err != nil {
		return TaskCommandResult{}, err
	}
	result := TaskCommandResult{Task: task, Event: event, RecurrenceID: id}
	if err := storeTaskReceiptTx(ctx, tx, command, result, now); err != nil {
		return TaskCommandResult{}, err
	}
	if err := tx.Commit(); err != nil {
		return TaskCommandResult{}, err
	}
	return result, nil
}

// NextTaskScheduleDeadline returns the first unprocessed schedule deadline.
func (s *Store) NextTaskScheduleDeadline(ctx context.Context) (*time.Time, error) {
	var value sql.NullInt64
	err := s.db.QueryRowContext(ctx, `SELECT MIN(value) FROM (
SELECT scheduled_for_ms AS value FROM tasks WHERE scheduled_for_ms IS NOT NULL
 AND schedule_processed_at_ms IS NULL AND state = 'captured'
UNION ALL
SELECT COALESCE(pending_coalesced_at_ms, next_run_at_ms) FROM task_recurrences recurrence
 WHERE lifecycle = 'active' AND (pending_coalesced_at_ms IS NULL OR NOT EXISTS (
  SELECT 1 FROM tasks WHERE recurrence_id = recurrence.recurrence_id
   AND state NOT IN ('completed', 'cancelled'))))`).Scan(&value)
	if err != nil {
		return nil, err
	}
	return nullTimePointer(value), nil
}

// ProcessDueTaskSchedules resolves all deadlines due at now in one transaction.
// It stages new occurrence documents before the database commit.
func (s *Store) ProcessDueTaskSchedules(
	ctx context.Context, now time.Time, recovering bool, stage func(DueTask) error,
) ([]DueTask, bool, error) {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, false, err
	}
	defer func() { _ = tx.Rollback() }()
	created := make([]DueTask, 0)
	changed := false
	for {
		var taskID string
		err := tx.QueryRowContext(ctx, `SELECT task_id FROM tasks WHERE scheduled_for_ms <= ?
AND schedule_processed_at_ms IS NULL AND state = 'captured'
ORDER BY scheduled_for_ms, task_id LIMIT 1`, millis(now)).Scan(&taskID)
		if err == nil {
			if err := processDueTaskTx(ctx, tx, taskID, now, recovering); err != nil {
				return nil, false, err
			}
			changed = true
			continue
		}
		if !errors.Is(err, sql.ErrNoRows) {
			return nil, false, err
		}
		var recurrenceID string
		err = tx.QueryRowContext(ctx, `SELECT recurrence_id FROM task_recurrences recurrence
WHERE lifecycle = 'active' AND ((pending_coalesced_at_ms IS NULL AND next_run_at_ms <= ?) OR
(pending_coalesced_at_ms <= ? AND NOT EXISTS (SELECT 1 FROM tasks WHERE recurrence_id = recurrence.recurrence_id
 AND state NOT IN ('completed', 'cancelled'))))
ORDER BY COALESCE(pending_coalesced_at_ms, next_run_at_ms), recurrence_id LIMIT 1`,
			millis(now), millis(now)).Scan(&recurrenceID)
		if errors.Is(err, sql.ErrNoRows) {
			break
		}
		if err != nil {
			return nil, false, err
		}
		createdTask, err := processDueRecurrenceTx(ctx, tx, recurrenceID, now, recovering)
		if err != nil {
			return nil, false, err
		}
		if createdTask != "" {
			created = append(created, DueTask{createdTask, recurrenceID})
		}
		changed = true
	}
	if stage != nil {
		for _, value := range created {
			if err := stage(value); err != nil {
				return created, changed, err
			}
		}
	}
	if err := tx.Commit(); err != nil {
		return created, changed, err
	}
	return created, changed, nil
}

func processDueTaskTx(ctx context.Context, tx *sql.Tx, id string, now time.Time, recovering bool) error {
	task, err := scanTask(tx.QueryRowContext(ctx, taskSelect+" WHERE task_id = ?", id))
	if err != nil {
		return err
	}
	task.Revision++
	task.UpdatedAt = now
	processed := now
	task.ScheduleProcessedAt = &processed
	kind := "task.schedule_released"
	if recovering && task.ScheduledFor.Before(now) && task.MissedRunPolicy == string(schedule.MissedRunSkip) {
		task.State = TaskCancelled
		kind = "task.cancelled"
	}
	_, err = tx.ExecContext(ctx, `UPDATE tasks SET state = ?, schedule_processed_at_ms = ?,
revision = ?, updated_at_ms = ? WHERE task_id = ? AND revision = ?`, task.State, millis(now),
		task.Revision, millis(now), id, task.Revision-1)
	if err != nil {
		return err
	}
	if err := insertTaskEvent(ctx, tx, task, strings.ReplaceAll(kind, ".", "_")); err != nil {
		return err
	}
	_, err = insertWorkEvent(ctx, tx, personalWorkspaceIDStore, task.ProjectID, id, "", task.Revision,
		kind, "actor:system:scheduler", nil, "correlation:schedule:"+id,
		map[string]any{"v": 1, "revision": task.Revision}, now)
	return err
}

func processDueRecurrenceTx(
	ctx context.Context, tx *sql.Tx, id string, now time.Time, recovering bool,
) (string, error) {
	value, err := scanTaskRecurrence(tx.QueryRowContext(ctx, recurrenceSelect+" WHERE recurrence_id = ?", id))
	if err != nil {
		return "", err
	}
	active, err := recurrenceHasActiveTaskTx(ctx, tx, id)
	if err != nil {
		return "", err
	}
	if value.PendingCoalescedAt != nil {
		if active {
			return "", nil
		}
		due := *value.PendingCoalescedAt
		task, _, err := materializeOccurrenceTx(ctx, tx, &value, "", due, "scheduled", true,
			"actor:system:scheduler", "correlation:schedule:"+id, now)
		if err != nil {
			return "", err
		}
		_, err = tx.ExecContext(ctx, `UPDATE task_recurrences SET pending_coalesced_at_ms = NULL,
updated_at_ms = ? WHERE recurrence_id = ? AND revision = ?`, millis(now), id, value.Revision)
		return task.ID, err
	}
	due := *value.NextRunAt
	missed := recovering && due.Before(now)
	resolution := "materialized"
	if missed && value.MissedRunPolicy == schedule.MissedRunSkip {
		resolution = "skipped"
	} else if active {
		switch value.OverlapPolicy {
		case schedule.OverlapSkip:
			resolution = "skipped"
		case schedule.OverlapQueueOne:
			resolution = "coalesced"
		}
	}
	slot, err := schedule.LocalSlot(due, value.TimeZone)
	if err != nil {
		return "", err
	}
	var duplicate bool
	if err := tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM task_recurrence_occurrences
WHERE recurrence_id = ? AND local_slot = ?)`, id, slot).Scan(&duplicate); err != nil {
		return "", err
	}
	if duplicate {
		resolution = "skipped"
	}
	lower := due.Add(time.Second)
	if missed {
		lower = now.Add(time.Second)
	}
	next, err := schedule.NextAtOrAfter(value.CronExpression, value.TimeZone, lower)
	if err != nil {
		return "", err
	}
	createdID := ""
	if resolution == "materialized" {
		task, _, err := materializeOccurrenceTx(ctx, tx, &value, "", due, "scheduled", false,
			"actor:system:scheduler", "correlation:schedule:"+id, now)
		if err != nil {
			return "", err
		}
		createdID = task.ID
	} else if !duplicate {
		if err := recordOccurrenceTx(ctx, tx, &value, due, "scheduled", resolution, "", now); err != nil {
			return "", err
		}
	}
	pending := value.PendingCoalescedAt
	if resolution == "coalesced" && pending == nil {
		pending = &due
	}
	_, err = tx.ExecContext(ctx, `UPDATE task_recurrences SET next_run_at_ms = ?,
pending_coalesced_at_ms = ?, updated_at_ms = ? WHERE recurrence_id = ? AND revision = ?`,
		millis(next), nullableTime(pending), millis(now), id, value.Revision)
	if err == nil && createdID == "" {
		task, taskErr := latestRecurrenceTaskTx(ctx, tx, id)
		if taskErr != nil {
			return "", taskErr
		}
		_, err = insertWorkEvent(ctx, tx, personalWorkspaceIDStore, value.ProjectID, task.ID, "",
			value.Revision, "task.recurrence_changed", "actor:system:scheduler", nil,
			"correlation:schedule:"+id, map[string]any{"v": 1, "recurrence_id": id,
				"revision": value.Revision, "resolution": resolution}, now)
	}
	return createdID, err
}

// RecurrenceTaskDocuments lists Task snapshots that must have a current TASK.md.
func (s *Store) RecurrenceTaskDocuments(ctx context.Context) ([]DueTask, error) {
	rows, err := s.db.QueryContext(ctx, `SELECT task_id, recurrence_id FROM tasks
WHERE recurrence_id IS NOT NULL ORDER BY created_at_ms, task_id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	values := make([]DueTask, 0)
	for rows.Next() {
		var value DueTask
		if err := rows.Scan(&value.TaskID, &value.RecurrenceID); err != nil {
			return nil, err
		}
		values = append(values, value)
	}
	return values, rows.Err()
}

func materializeOccurrenceTx(
	ctx context.Context, tx *sql.Tx, value *TaskRecurrence, id string, due time.Time, trigger string,
	release bool, actor, correlation string, now time.Time,
) (Task, WorkEvent, error) {
	var err error
	if id == "" {
		id, err = newID("task")
		if err != nil {
			return Task{}, WorkEvent{}, err
		}
	}
	task := Task{ID: id, ProjectID: value.ProjectID, Title: value.Title, State: TaskCaptured,
		Revision: 1, ExecutorAgentID: value.ExecutorAgentID,
		ExecutorAcpConnectionRevision: cloneInt(value.ExecutorAcpConnectionRevision),
		CwdOverride:                   cloneString(value.CwdOverride), ScheduledFor: &due,
		ScheduleTimeZone: value.TimeZone, MissedRunPolicy: string(value.MissedRunPolicy),
		ScheduleProcessedAt: &now, RecurrenceID: value.ID, RecurrenceRevision: &value.Revision,
		RecurrenceScheduledFor: &due, CreatedAt: now, UpdatedAt: now}
	_, err = tx.ExecContext(ctx, `INSERT INTO tasks
(task_id, project_id, title, state, revision, executor_agent_id,
 executor_acp_connection_revision, cwd_override, scheduled_for_ms, schedule_time_zone,
 missed_run_policy, schedule_processed_at_ms, recurrence_id, recurrence_revision,
 recurrence_scheduled_for_ms, created_at_ms, updated_at_ms)
VALUES (?, NULLIF(?, ''), ?, 'captured', 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		task.ID, task.ProjectID, task.Title, task.ExecutorAgentID,
		nullableInt(task.ExecutorAcpConnectionRevision), nullableString(task.CwdOverride), millis(due),
		task.ScheduleTimeZone, task.MissedRunPolicy, millis(now), value.ID, value.Revision,
		millis(due), millis(now), millis(now))
	if err != nil {
		return Task{}, WorkEvent{}, err
	}
	if release {
		slot, _ := schedule.LocalSlot(due, value.TimeZone)
		_, err = tx.ExecContext(ctx, `UPDATE task_recurrence_occurrences SET resolution = 'materialized',
task_id = ? WHERE recurrence_id = ? AND local_slot = ? AND resolution = 'coalesced'`, id, value.ID, slot)
	} else {
		err = recordOccurrenceTx(ctx, tx, value, due, trigger, "materialized", id, now)
	}
	if err != nil {
		return Task{}, WorkEvent{}, err
	}
	if err := insertTaskEvent(ctx, tx, task, "task_captured"); err != nil {
		return Task{}, WorkEvent{}, err
	}
	event, err := insertWorkEvent(ctx, tx, personalWorkspaceIDStore, task.ProjectID, id, "", 1,
		"task.captured", actor, nil, correlation, map[string]any{"v": 1, "revision": int64(1)}, now)
	return task, event, err
}

func recordOccurrenceTx(
	ctx context.Context, tx *sql.Tx, value *TaskRecurrence, due time.Time,
	trigger, resolution, taskID string, now time.Time,
) error {
	id, err := newID("occurrence")
	if err != nil {
		return err
	}
	slot := "manual:" + id
	if trigger == "scheduled" {
		slot, err = schedule.LocalSlot(due, value.TimeZone)
		if err != nil {
			return err
		}
	}
	_, err = tx.ExecContext(ctx, `INSERT OR IGNORE INTO task_recurrence_occurrences
(occurrence_id, recurrence_id, recurrence_revision, scheduled_for_ms, local_slot,
 trigger_kind, resolution, task_id, created_at_ms)
VALUES (?, ?, ?, ?, ?, ?, ?, NULLIF(?, ''), ?)`, id, value.ID, value.Revision,
		millis(due), slot, trigger, resolution, taskID, millis(now))
	return err
}

func recurrenceHasActiveTaskTx(ctx context.Context, tx *sql.Tx, id string) (bool, error) {
	var active bool
	err := tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM tasks WHERE recurrence_id = ?
AND state NOT IN ('completed', 'cancelled'))`, id).Scan(&active)
	return active, err
}

func latestRecurrenceTaskTx(ctx context.Context, tx *sql.Tx, id string) (Task, error) {
	return scanTask(tx.QueryRowContext(ctx, taskSelect+`
 WHERE recurrence_id = ? ORDER BY recurrence_scheduled_for_ms DESC, created_at_ms DESC LIMIT 1`, id))
}

const recurrenceSelect = `SELECT recurrence_id, workspace_id, COALESCE(project_id, ''), title,
executor_agent_id, executor_acp_connection_revision, cwd_override, starts_at_ms,
cron_expression, time_zone, missed_run_policy, overlap_policy, lifecycle, revision,
next_run_at_ms, pending_coalesced_at_ms, created_at_ms, updated_at_ms FROM task_recurrences`

func scanTaskRecurrence(row rowScanner) (TaskRecurrence, error) {
	var value TaskRecurrence
	var starts, created, updated int64
	var revision sql.NullInt64
	var cwd sql.NullString
	var next, pending sql.NullInt64
	if err := row.Scan(&value.ID, &value.WorkspaceID, &value.ProjectID, &value.Title,
		&value.ExecutorAgentID, &revision, &cwd, &starts, &value.CronExpression,
		&value.TimeZone, &value.MissedRunPolicy, &value.OverlapPolicy, &value.Lifecycle,
		&value.Revision, &next, &pending, &created, &updated); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return TaskRecurrence{}, ErrTaskNotFound
		}
		return TaskRecurrence{}, err
	}
	value.ExecutorAcpConnectionRevision = nullIntPointer(revision)
	value.CwdOverride = nullStringPointer(cwd)
	value.StartsAt, value.CreatedAt, value.UpdatedAt = fromMillis(starts), fromMillis(created), fromMillis(updated)
	value.NextRunAt, value.PendingCoalescedAt = nullTimePointer(next), nullTimePointer(pending)
	return value, nil
}

func validateTaskProjectTx(ctx context.Context, tx *sql.Tx, id string) error {
	if id == "" {
		return nil
	}
	var archived sql.NullInt64
	err := tx.QueryRowContext(ctx, `SELECT archived_at_ms FROM projects
WHERE project_id = ? AND workspace_id = 'workspace:personal'`, id).Scan(&archived)
	if errors.Is(err, sql.ErrNoRows) {
		return ErrProjectNotFound
	}
	if err != nil {
		return err
	}
	if archived.Valid {
		return ErrInvalidTransition
	}
	return nil
}

func validateTaskExecutorTx(ctx context.Context, tx *sql.Tx, id string) (*int64, error) {
	var role sql.NullString
	var enabled sql.NullBool
	var revision sql.NullInt64
	err := tx.QueryRowContext(ctx, `SELECT a.system_role, c.enabled, c.connection_revision
FROM agents a LEFT JOIN acp_agents c ON c.agent_id = a.agent_id WHERE a.agent_id = ?`, id).
		Scan(&role, &enabled, &revision)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrAgentNotFound
	}
	if err != nil {
		return nil, err
	}
	if !role.Valid && (!enabled.Valid || !enabled.Bool) {
		return nil, ErrInvalidAcpAgent
	}
	return nullIntPointer(revision), nil
}

func applyScheduleToTask(task *Task, value *schedule.Schedule) {
	if value == nil {
		return
	}
	task.ScheduledFor = cloneTime(&value.ScheduledFor)
	task.ScheduleTimeZone = value.TimeZone
	task.MissedRunPolicy = string(value.MissedRunPolicy)
	if value.Recurrence != nil {
		task.RecurrenceScheduledFor = cloneTime(&value.ScheduledFor)
	}
}

func clearTaskSchedule(task *Task) {
	task.ScheduledFor, task.ScheduleProcessedAt, task.RecurrenceScheduledFor = nil, nil, nil
	task.ScheduleTimeZone, task.MissedRunPolicy, task.RecurrenceID = "", "", ""
	task.RecurrenceRevision = nil
}

func nullableInt(value *int64) any {
	if value == nil {
		return nil
	}
	return *value
}

func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	copy := *value
	return &copy
}

func cloneInt(value *int64) *int64 {
	if value == nil {
		return nil
	}
	copy := *value
	return &copy
}

func cloneTime(value *time.Time) *time.Time {
	if value == nil {
		return nil
	}
	copy := value.UTC()
	return &copy
}

// LookupTaskCommandReceipt returns one earlier equal Task command result.
func (s *Store) LookupTaskCommandReceipt(
	ctx context.Context, command TaskCommand,
) (TaskCommandResult, bool, error) {
	if err := validateTaskCommand(command); err != nil {
		return TaskCommandResult{}, false, err
	}
	return lookupTaskReceiptTx(ctx, s.db, command)
}

// TaskRecurrenceReceiptResult returns one committed recurrence document update.
func (s *Store) TaskRecurrenceReceiptResult(
	ctx context.Context, recurrenceID, requestDigest string,
) (TaskCommandResult, bool, error) {
	decoded, err := hex.DecodeString(requestDigest)
	if recurrenceID == "" || err != nil || len(decoded) != 32 || hex.EncodeToString(decoded) != requestDigest {
		return TaskCommandResult{}, false, errors.New("invalid recurrence receipt lookup")
	}
	var response string
	err = s.db.QueryRowContext(ctx, `SELECT response_json FROM command_receipts
WHERE actor_id = 'actor:human:local' AND command_name = 'update_task_recurrence'
 AND request_digest = ? ORDER BY result_event_id DESC LIMIT 1`, requestDigest).Scan(&response)
	if errors.Is(err, sql.ErrNoRows) {
		return TaskCommandResult{}, false, nil
	}
	if err != nil {
		return TaskCommandResult{}, false, err
	}
	var result TaskCommandResult
	if err := json.Unmarshal([]byte(response), &result); err != nil {
		return TaskCommandResult{}, true, err
	}
	if result.RecurrenceID != recurrenceID {
		return TaskCommandResult{}, true, errors.New("recurrence receipt does not match its stage")
	}
	result.Replayed = true
	return result, true, nil
}

func lookupTaskReceiptTx(
	ctx context.Context, query interface {
		QueryRowContext(context.Context, string, ...any) *sql.Row
	}, command TaskCommand,
) (TaskCommandResult, bool, error) {
	var digest, response string
	err := query.QueryRowContext(ctx, `SELECT request_digest, response_json FROM command_receipts
WHERE actor_id = 'actor:human:local' AND command_name = ? AND client_mutation_id = ?`,
		command.Name, command.ClientMutationID).Scan(&digest, &response)
	if errors.Is(err, sql.ErrNoRows) {
		return TaskCommandResult{}, false, nil
	}
	if err != nil {
		return TaskCommandResult{}, false, err
	}
	if digest != command.RequestDigest {
		return TaskCommandResult{}, true, ErrCommandConflict
	}
	var result TaskCommandResult
	if err := json.Unmarshal([]byte(response), &result); err != nil {
		return TaskCommandResult{}, true, err
	}
	result.Replayed = true
	return result, true, nil
}

func storeTaskReceiptTx(
	ctx context.Context, tx *sql.Tx, command TaskCommand, result TaskCommandResult, now time.Time,
) error {
	response, err := json.Marshal(result)
	if err != nil {
		return err
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO command_receipts
(actor_id, command_name, client_mutation_id, request_digest, result_project_id,
 result_task_id, result_event_id, response_json, created_at_ms)
VALUES ('actor:human:local', ?, ?, ?, NULLIF(?, ''), ?, ?, ?, ?)`, command.Name,
		command.ClientMutationID, command.RequestDigest, result.Task.ProjectID, result.Task.ID,
		result.Event.ID, string(response), millis(now.UTC()))
	return err
}

func validateTaskCommand(command TaskCommand) error {
	decoded, err := hex.DecodeString(command.RequestDigest)
	if strings.TrimSpace(command.Name) == "" || strings.TrimSpace(command.ClientMutationID) == "" ||
		len(decoded) != 32 || hex.EncodeToString(decoded) != command.RequestDigest ||
		!strings.HasPrefix(command.CorrelationID, "correlation:") {
		return errors.New("invalid Task command")
	}
	return err
}

const personalWorkspaceIDStore = "workspace:personal"
