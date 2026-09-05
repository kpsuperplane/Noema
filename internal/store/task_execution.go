package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"strings"
	"time"
)

// AcpPermissionResult is one resolved exact ACP permission request.
type AcpPermissionResult struct {
	Found, Approved bool
	OptionID        string
}

// TaskRunItemInput is one durable provider or tool transcript item.
type TaskRunItemInput struct {
	Kind, Status, CorrelationID, ParentID, Content string
	Round                                          int64
	Payload                                        map[string]any
}

// TaskRunUsage adds one provider call's bounded usage to a run.
type TaskRunUsage struct {
	ProviderCalls, ToolCalls                     int64
	InputTokens, CachedInputTokens, OutputTokens int64
	ActiveMilliseconds                           int64
}

// RecoverTaskExecutions returns interrupted current runs to the queue.
func (s *Store) RecoverTaskExecutions(ctx context.Context, now time.Time) error {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer tx.Rollback()
	rows, err := tx.QueryContext(ctx, `SELECT r.run_id FROM task_runs r JOIN tasks t ON t.current_run_id=r.run_id
WHERE r.status IN ('leased','running') AND r.executor_backend='acp' AND r.task_generation=t.generation
AND EXISTS(SELECT 1 FROM task_run_items i WHERE i.run_id=r.run_id AND i.correlation_id LIKE 'acp:permission-used:%'
AND json_type(i.payload_json,'$.permission_fingerprint')='text')
AND NOT EXISTS(SELECT 1 FROM task_run_items call JOIN task_run_items result ON result.parent_item_id=call.item_id
WHERE call.run_id=r.run_id AND call.item_kind='tool_call' AND call.correlation_id='acp:terminal'
AND result.item_kind='tool_result' AND json_extract(result.payload_json,'$.success')=1)`)
	if err != nil {
		return err
	}
	var uncertain []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			_ = rows.Close()
			return err
		}
		uncertain = append(uncertain, id)
	}
	if err := rows.Err(); err != nil {
		_ = rows.Close()
		return err
	}
	if err := rows.Close(); err != nil {
		return err
	}
	for _, id := range uncertain {
		run, err := taskRunTx(ctx, tx, id)
		if err != nil {
			return err
		}
		task, err := scanTask(tx.QueryRowContext(ctx, taskSelect+" WHERE task_id=?", run.TaskID))
		if err != nil {
			return err
		}
		if task.CurrentRunID != run.ID || task.Generation != run.Generation {
			return ErrStaleRun
		}
		if err := markTaskExecutionUncertainTx(ctx, tx, &task, run, now); err != nil {
			return err
		}
	}
	if _, err = tx.ExecContext(ctx, `UPDATE task_runs SET status='queued',updated_at_ms=?
WHERE status IN ('leased','running') AND run_id IN
(SELECT current_run_id FROM tasks WHERE state='running' AND current_run_id IS NOT NULL)`, millis(now)); err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `UPDATE tasks SET stage_key='queue',updated_at_ms=?
WHERE state='running' AND current_run_id IN (SELECT run_id FROM task_runs WHERE status='queued')`, millis(now)); err != nil {
		return err
	}
	if err = tx.Commit(); err == nil {
		s.NotifyWork()
	}
	return err
}

// ClaimTaskExecution leases the oldest current Task run.
func (s *Store) ClaimTaskExecution(ctx context.Context, now time.Time) (Task, TaskRun, bool, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return Task{}, TaskRun{}, false, err
	}
	defer tx.Rollback()
	var runID string
	err = tx.QueryRowContext(ctx, `SELECT r.run_id FROM task_runs r JOIN tasks t ON t.current_run_id=r.run_id
WHERE r.status='queued' AND r.executor_backend IN ('provider','acp') AND r.task_generation=t.generation
ORDER BY r.queued_at_ms,r.run_id LIMIT 1`).Scan(&runID)
	if errors.Is(err, sql.ErrNoRows) {
		return Task{}, TaskRun{}, false, nil
	}
	if err != nil {
		return Task{}, TaskRun{}, false, err
	}
	run, err := taskRunTx(ctx, tx, runID)
	if err != nil {
		return Task{}, TaskRun{}, false, err
	}
	task, err := scanTask(tx.QueryRowContext(ctx, taskSelect+" WHERE task_id=?", run.TaskID))
	if err != nil {
		return Task{}, TaskRun{}, false, err
	}
	policy, err := taskExecutionPolicyTx(ctx, tx)
	if err != nil {
		return Task{}, TaskRun{}, false, err
	}
	changed, err := tx.ExecContext(ctx, `UPDATE task_runs SET status='leased',updated_at_ms=?,max_provider_continuations=?,
max_tool_calls=?,max_active_minutes=?,progress_audit_interval=?,max_automatic_retries=?,max_review_rounds=?
WHERE run_id=? AND status='queued' AND task_generation=? AND run_id=(SELECT current_run_id FROM tasks WHERE task_id=? AND generation=?)`,
		millis(now), policy.MaxProviderContinuations, policy.MaxToolCalls, policy.MaxActiveMinutes,
		policy.ProgressAuditInterval, policy.MaxAutomaticRetries, policy.MaxReviewRounds,
		run.ID, run.Generation, task.ID, run.Generation)
	if err != nil {
		return Task{}, TaskRun{}, false, err
	}
	if count, _ := changed.RowsAffected(); count != 1 {
		return Task{}, TaskRun{}, false, ErrStaleRun
	}
	run.Status, run.UpdatedAt, run.ExecutionPolicy = "leased", now.UTC(), policy
	if err = tx.Commit(); err != nil {
		return Task{}, TaskRun{}, false, err
	}
	return task, run, true, nil
}

// StartTaskExecution changes one current lease to running.
func (s *Store) StartTaskExecution(ctx context.Context, runID string, generation int64, now time.Time) error {
	return s.taskRunTransaction(ctx, runID, generation, "leased", func(tx *sql.Tx, task *Task, run *TaskRun) error {
		changed, err := tx.ExecContext(ctx, `UPDATE task_runs SET status='running',started_at_ms=COALESCE(started_at_ms,?),updated_at_ms=? WHERE run_id=? AND status='leased'`, millis(now), millis(now), run.ID)
		if err != nil {
			return err
		}
		if count, _ := changed.RowsAffected(); count != 1 {
			return ErrStaleRun
		}
		task.Revision++
		task.StageKey, task.UpdatedAt = "doing", now.UTC()
		if err := updateCurrentTaskTx(ctx, tx, *task, run.ID, "doing", now); err != nil {
			return err
		}
		if err := insertTaskEvent(ctx, tx, *task, "run_started"); err != nil {
			return err
		}
		return appendTaskExecutionEvent(ctx, tx, *task, *run, "run.started", map[string]any{"run_kind": run.Kind}, now)
	})
}

// AppendTaskRunItems saves transcript items and run usage under the current-run fence.
func (s *Store) AppendTaskRunItems(ctx context.Context, runID string, generation int64, items []TaskRunItemInput, usage TaskRunUsage, now time.Time) error {
	if len(items) == 0 && usage == (TaskRunUsage{}) {
		return nil
	}
	return s.taskRunTransaction(ctx, runID, generation, "running", func(tx *sql.Tx, _ *Task, run *TaskRun) error {
		var next int64
		if err := tx.QueryRowContext(ctx, `SELECT COALESCE(MAX(sequence_index)+1,0) FROM task_run_items WHERE run_id=?`, run.ID).Scan(&next); err != nil {
			return err
		}
		for _, item := range items {
			if !validTaskRunItem(item) {
				return errors.New("invalid Task run item")
			}
			id, err := newID("run_item")
			if err != nil {
				return err
			}
			payload, err := json.Marshal(item.Payload)
			if err != nil {
				return err
			}
			_, err = tx.ExecContext(ctx, `INSERT INTO task_run_items
(item_id,run_id,sequence_index,round_index,item_kind,status,correlation_id,parent_item_id,content_text,payload_json,created_at_ms,updated_at_ms)
VALUES (?,?,?,?,?,?,NULLIF(?,''),NULLIF(?,''),NULLIF(?,''),?,?,?)`, id, run.ID, next, item.Round, item.Kind, item.Status,
				item.CorrelationID, item.ParentID, item.Content, string(payload), millis(now), millis(now))
			if err != nil {
				return err
			}
			if item.Kind == "tool_result" && item.ParentID != "" {
				if _, err = tx.ExecContext(ctx, `UPDATE task_run_items SET status=?,updated_at_ms=? WHERE item_id=? AND run_id=? AND item_kind='tool_call' AND status='running'`, item.Status, millis(now), item.ParentID, run.ID); err != nil {
					return err
				}
			}
			next++
		}
		_, err := tx.ExecContext(ctx, `UPDATE task_runs SET provider_call_count=provider_call_count+?,tool_call_count=tool_call_count+?,
input_tokens=input_tokens+?,cached_input_tokens=cached_input_tokens+?,output_tokens=output_tokens+?,active_milliseconds=active_milliseconds+?,updated_at_ms=?
WHERE run_id=? AND status='running'`, usage.ProviderCalls, usage.ToolCalls, usage.InputTokens, usage.CachedInputTokens,
			usage.OutputTokens, usage.ActiveMilliseconds, millis(now), run.ID)
		return err
	})
}

// RecordAcpSession saves one client-visible session identity under the current-run check.
func (s *Store) RecordAcpSession(ctx context.Context, runID string, generation int64, sessionID string, now time.Time) error {
	if strings.TrimSpace(sessionID) == "" || len(sessionID) > 512 {
		return errors.New("invalid ACP session identity")
	}
	return s.AppendTaskRunItems(ctx, runID, generation, []TaskRunItemInput{{
		Kind: "progress_notice", Status: "completed", CorrelationID: "acp:session",
		Content: "ACP session started.", Payload: map[string]any{"acp_session_id": sessionID},
	}}, TaskRunUsage{}, now)
}

// ResolvedAcpPermission reads one parent-run approval for the current ACP run.
func (s *Store) ResolvedAcpPermission(ctx context.Context, runID, fingerprint string) (AcpPermissionResult, error) {
	var decision, option string
	err := s.db.QueryRowContext(ctx, `SELECT m.approval_decision,json_extract(i.payload_json,'$.allow_once_option_id')
FROM task_runs current
JOIN task_gates g ON g.originating_run_id=current.parent_run_id AND g.gate_kind='approval' AND g.gate_state='resolved'
JOIN task_messages m ON m.message_id=g.resolution_message_id
JOIN task_run_items i ON i.run_id=current.parent_run_id AND i.correlation_id=?
  AND json_type(i.payload_json,'$.allow_once_option_id')='text'
JOIN tasks t ON t.current_run_id=current.run_id AND t.generation=current.task_generation
WHERE current.run_id=? AND current.status='running' LIMIT 1`, "acp:permission:"+fingerprint, runID).Scan(&decision, &option)
	if errors.Is(err, sql.ErrNoRows) {
		return AcpPermissionResult{}, nil
	}
	if err != nil {
		return AcpPermissionResult{}, err
	}
	var consumed int
	if err := s.db.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM task_run_items
WHERE run_id=? AND correlation_id=? AND json_type(payload_json,'$.permission_fingerprint')='text')`,
		runID, "acp:permission-used:"+fingerprint).Scan(&consumed); err != nil {
		return AcpPermissionResult{}, err
	}
	return AcpPermissionResult{Found: true, Approved: decision == "approved" && consumed == 0, OptionID: option}, nil
}

// RecordAcpPermissionUse consumes one exact approval before ACP can perform its effect.
func (s *Store) RecordAcpPermissionUse(ctx context.Context, runID string, generation int64, fingerprint string, now time.Time) error {
	return s.AppendTaskRunItems(ctx, runID, generation, []TaskRunItemInput{{
		Kind: "progress_notice", Status: "completed", CorrelationID: "acp:permission-used:" + fingerprint,
		Content: "ACP permission used once.", Payload: map[string]any{"permission_fingerprint": fingerprint},
	}}, TaskRunUsage{}, now)
}

// TaskExecutionIsCurrent reports whether one run still owns the active generation.
func (s *Store) TaskExecutionIsCurrent(ctx context.Context, runID string, generation int64) (bool, error) {
	var count int
	err := s.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM tasks t JOIN task_runs r ON r.run_id=t.current_run_id
WHERE r.run_id=? AND r.task_generation=? AND t.generation=? AND r.status IN ('leased','running')`, runID, generation, generation).Scan(&count)
	return count == 1, err
}

// SuspendTaskExecution keeps one exact run current while human input is pending.
func (s *Store) SuspendTaskExecution(ctx context.Context, runID string, generation int64, now time.Time) error {
	result, err := s.db.ExecContext(ctx, `UPDATE task_runs SET status='waiting_for_approval',updated_at_ms=?
WHERE run_id=? AND task_generation=? AND status='running'
 AND EXISTS (SELECT 1 FROM tasks WHERE current_run_id=? AND generation=?)`, millis(now), runID, generation, runID, generation)
	if err != nil {
		return err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return ErrStaleRun
	}
	return nil
}

// ResumeTaskExecution queues one exact current run after its intervention closes.
func (s *Store) ResumeTaskExecution(ctx context.Context, runID string, generation int64, now time.Time) error {
	result, err := s.db.ExecContext(ctx, `UPDATE task_runs SET status='queued',queued_at_ms=?,updated_at_ms=?
WHERE run_id=? AND task_generation=? AND status='waiting_for_approval'
 AND EXISTS (SELECT 1 FROM tasks WHERE current_run_id=? AND generation=?)`, millis(now), millis(now), runID, generation, runID, generation)
	if err != nil {
		return err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return ErrStaleRun
	}
	s.NotifyWork()
	return nil
}

// CompleteTaskIntervention stores one exact tool result before it queues the run.
func (s *Store) CompleteTaskIntervention(ctx context.Context, runID string, generation int64, item TaskRunItemInput, uncertain bool, now time.Time) error {
	return s.completeTaskResult(ctx, runID, generation, "waiting_for_approval", item, uncertain, now)
}

// CompleteTaskUncertainResult atomically stores one result and opens recovery.
func (s *Store) CompleteTaskUncertainResult(ctx context.Context, runID string, generation int64, item TaskRunItemInput, now time.Time) error {
	return s.completeTaskResult(ctx, runID, generation, "running", item, true, now)
}

func (s *Store) completeTaskResult(ctx context.Context, runID string, generation int64, runStatus string, item TaskRunItemInput, uncertain bool, now time.Time) error {
	if item.Kind != "tool_result" || item.ParentID == "" || !validTaskRunItem(item) {
		return errors.New("invalid Task tool result")
	}
	err := s.taskRunTransaction(ctx, runID, generation, runStatus, func(tx *sql.Tx, task *Task, run *TaskRun) error {
		var next int64
		if err := tx.QueryRowContext(ctx, `SELECT COALESCE(MAX(sequence_index)+1,0) FROM task_run_items WHERE run_id=?`, run.ID).Scan(&next); err != nil {
			return err
		}
		id, err := newID("run_item")
		if err != nil {
			return err
		}
		payload, err := json.Marshal(item.Payload)
		if err != nil {
			return err
		}
		if _, err = tx.ExecContext(ctx, `INSERT INTO task_run_items
(item_id,run_id,sequence_index,round_index,item_kind,status,parent_item_id,payload_json,created_at_ms,updated_at_ms)
VALUES (?,?,?,?,?,?,?, ?,?,?)`, id, run.ID, next, item.Round, item.Kind, item.Status, item.ParentID, string(payload), millis(now), millis(now)); err != nil {
			return err
		}
		if _, err = tx.ExecContext(ctx, `UPDATE task_run_items SET status=?,updated_at_ms=? WHERE item_id=? AND run_id=? AND item_kind='tool_call' AND status='running'`, item.Status, millis(now), item.ParentID, run.ID); err != nil {
			return err
		}
		if uncertain {
			return markTaskExecutionUncertainTx(ctx, tx, task, *run, now)
		}
		_, err = tx.ExecContext(ctx, `UPDATE task_runs SET status='queued',queued_at_ms=?,updated_at_ms=? WHERE run_id=? AND status='waiting_for_approval'`, millis(now), millis(now), run.ID)
		return err
	})
	if err == nil {
		s.NotifyWork()
	}
	return err
}

// TaskRunReplayItems returns one bounded run transcript in provider order.
func (s *Store) TaskRunReplayItems(ctx context.Context, runID string) ([]TaskRunItem, error) {
	rows, err := s.db.QueryContext(ctx, `SELECT item_id,run_id,sequence_index,round_index,item_kind,status,correlation_id,parent_item_id,content_text,payload_json,created_at_ms,updated_at_ms
FROM task_run_items WHERE run_id=? AND COALESCE(json_type(payload_json,'$.acp_launch'),'')<>'object'
ORDER BY sequence_index LIMIT 4097`, runID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	items := make([]TaskRunItem, 0)
	for rows.Next() {
		var item TaskRunItem
		var correlation, parent, content sql.NullString
		var payload string
		var created, updated int64
		if err := rows.Scan(&item.ID, &item.RunID, &item.Sequence, &item.Round, &item.Kind, &item.Status, &correlation, &parent, &content, &payload, &created, &updated); err != nil {
			return nil, err
		}
		item.CorrelationID, item.ParentID, item.Content = nullStringPointer(correlation), nullStringPointer(parent), nullStringPointer(content)
		item.CreatedAt, item.UpdatedAt = fromMillis(created), fromMillis(updated)
		if json.Unmarshal([]byte(payload), &item.Payload) != nil {
			return nil, errors.New("invalid Task run item payload")
		}
		items = append(items, item)
	}
	if len(items) > 4096 {
		return nil, errors.New("Task run transcript is too large")
	}
	return items, rows.Err()
}

// FinishTaskPlanning completes the Planner and queues the first Executor.
func (s *Store) FinishTaskPlanning(ctx context.Context, runID string, generation int64, complexity string, now time.Time) error {
	if complexity != "simple" && complexity != "medium" && complexity != "difficult" {
		return errors.New("invalid Task complexity")
	}
	return s.completeTaskRun(ctx, runID, generation, "planner", now, func(tx *sql.Tx, task *Task, run TaskRun) error {
		task.ExecutionComplexity = complexity
		if _, err := tx.ExecContext(ctx, `UPDATE tasks SET execution_complexity=? WHERE task_id=?`, complexity, task.ID); err != nil {
			return err
		}
		return queueTaskExecutionChild(ctx, tx, task, run, "executor", 0, 1, now)
	})
}

// FinishTaskExecution completes an Executor and queues review or continuation.
func (s *Store) FinishTaskExecution(ctx context.Context, runID string, generation int64, continueRun bool, now time.Time) error {
	return s.completeTaskRun(ctx, runID, generation, "executor", now, func(tx *sql.Tx, task *Task, run TaskRun) error {
		kind, review := "reviewer", max64(run.ReviewRound, 1)
		if continueRun {
			kind, review = "executor", run.ReviewRound
		}
		return queueTaskExecutionChild(ctx, tx, task, run, kind, 0, review, now)
	})
}

// FinishTaskReview applies one checked Reviewer decision.
func (s *Store) FinishTaskReview(ctx context.Context, runID string, generation int64, decision, feedback string, notifyHuman bool, now time.Time) error {
	feedback = strings.TrimSpace(feedback)
	if decision != "approve" && decision != "request_changes" && decision != "needs_human" || feedback == "" || len(feedback) > 20_000 {
		return errors.New("invalid Task review")
	}
	return s.completeTaskRun(ctx, runID, generation, "reviewer", now, func(tx *sql.Tx, task *Task, run TaskRun) error {
		switch decision {
		case "approve":
			task.Revision++
			task.State, task.StageKey, task.CurrentRunID = TaskCompleted, "done", ""
			task.CompletedAt, task.UpdatedAt = timeAddress(now.UTC()), now.UTC()
			changed, err := tx.ExecContext(ctx, `UPDATE tasks SET state='completed',stage_key='done',current_run_id=NULL,active_gate_id=NULL,
completed_at_ms=?,cancelled_at_ms=NULL,revision=?,updated_at_ms=? WHERE task_id=? AND current_run_id=? AND generation=?`,
				millis(now), task.Revision, millis(now), task.ID, run.ID, run.Generation)
			if err != nil {
				return err
			}
			if count, _ := changed.RowsAffected(); count != 1 {
				return ErrStaleRun
			}
			if err := insertTaskEvent(ctx, tx, *task, "task_completed"); err != nil {
				return err
			}
			return appendTaskExecutionEvent(ctx, tx, *task, run, "task.completed", map[string]any{"notify_human": notifyHuman}, now)
		case "request_changes":
			if run.ReviewRound >= run.ExecutionPolicy.MaxReviewRounds {
				return openTaskExecutionGate(ctx, tx, task, run, "recovery", feedback, "Review rounds are exhausted.", "review_rounds_exhausted", "executor", now)
			}
			return queueTaskExecutionChild(ctx, tx, task, run, "executor", 0, run.ReviewRound+1, now)
		default:
			return openTaskExecutionGate(ctx, tx, task, run, "clarification", feedback, "The Reviewer needs human input.", "", "", now)
		}
	})
}

// BlockTaskExecution opens a human clarification or approval gate.
func (s *Store) BlockTaskExecution(ctx context.Context, runID string, generation int64, kind, prompt, detail string, answers []string, now time.Time) error {
	prompt, detail = strings.TrimSpace(prompt), strings.TrimSpace(detail)
	if kind != "clarification" && kind != "approval" || prompt == "" || len(prompt) > 4_000 || len(detail) > 20_000 || len(answers) > 8 {
		return errors.New("invalid Task gate")
	}
	for _, answer := range answers {
		if strings.TrimSpace(answer) == "" || len(answer) > 1_000 {
			return errors.New("invalid Task gate answer")
		}
	}
	return s.taskRunTransaction(ctx, runID, generation, "running", func(tx *sql.Tx, task *Task, run *TaskRun) error {
		if run.Kind != "planner" && run.Kind != "executor" {
			return ErrInvalidTransition
		}
		return openTaskExecutionGateWithAnswers(ctx, tx, task, *run, kind, prompt, detail, "", "", answers, now)
	})
}

// FailTaskExecution retries one current run or opens its recovery gate.
func (s *Store) FailTaskExecution(ctx context.Context, runID string, generation int64, code, message string, retryable bool, now time.Time) error {
	message = strings.TrimSpace(message)
	if message == "" {
		message = "The provider run failed."
	}
	return s.taskRunTransaction(ctx, runID, generation, "running", func(tx *sql.Tx, task *Task, run *TaskRun) error {
		_, err := tx.ExecContext(ctx, `UPDATE task_runs SET status='failed',error_code=?,error_message=?,ended_at_ms=?,updated_at_ms=? WHERE run_id=? AND status='running'`, code, message, millis(now), millis(now), run.ID)
		if err != nil {
			return err
		}
		if retryable && run.AttemptIndex < run.ExecutionPolicy.MaxAutomaticRetries {
			return queueTaskExecutionChild(ctx, tx, task, *run, run.Kind, run.AttemptIndex+1, run.ReviewRound, now)
		}
		return openTaskExecutionGate(ctx, tx, task, *run, "recovery", "The run failed and needs a recovery decision.", message, "infrastructure_retries_exhausted", run.Kind, now)
	})
}

// MarkTaskExecutionUncertain opens recovery after an approved effect has an uncertain result.
func (s *Store) MarkTaskExecutionUncertain(ctx context.Context, runID string, generation int64, now time.Time) error {
	return s.taskRunTransaction(ctx, runID, generation, "running", func(tx *sql.Tx, task *Task, run *TaskRun) error {
		return markTaskExecutionUncertainTx(ctx, tx, task, *run, now)
	})
}

func markTaskExecutionUncertainTx(ctx context.Context, tx *sql.Tx, task *Task, run TaskRun, now time.Time) error {
	changed, err := tx.ExecContext(ctx, `UPDATE task_runs SET status='failed',error_code='outcome_uncertain',
 error_message='An approved operation may have completed.',ended_at_ms=?,updated_at_ms=?
WHERE run_id=? AND status IN ('leased','running','waiting_for_approval')`, millis(now), millis(now), run.ID)
	if err != nil {
		return err
	}
	if count, _ := changed.RowsAffected(); count != 1 {
		return ErrStaleRun
	}
	return openTaskExecutionGate(ctx, tx, task, run, "recovery",
		"Check the external result before continuing.", "An approved operation may have completed.",
		"unsafe_effect_uncertain", "executor", now)
}

func (s *Store) completeTaskRun(ctx context.Context, runID string, generation int64, kind string, now time.Time, next func(*sql.Tx, *Task, TaskRun) error) error {
	return s.taskRunTransaction(ctx, runID, generation, "running", func(tx *sql.Tx, task *Task, run *TaskRun) error {
		if run.Kind != kind {
			return ErrInvalidTransition
		}
		changed, err := tx.ExecContext(ctx, `UPDATE task_runs SET status='completed',ended_at_ms=?,updated_at_ms=? WHERE run_id=? AND status='running'`, millis(now), millis(now), run.ID)
		if err != nil {
			return err
		}
		if count, _ := changed.RowsAffected(); count != 1 {
			return ErrStaleRun
		}
		if err := appendTaskExecutionEvent(ctx, tx, *task, *run, "run.completed", map[string]any{"run_kind": run.Kind}, now); err != nil {
			return err
		}
		return next(tx, task, *run)
	})
}

func (s *Store) taskRunTransaction(ctx context.Context, runID string, generation int64, status string, change func(*sql.Tx, *Task, *TaskRun) error) error {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer tx.Rollback()
	run, err := taskRunTx(ctx, tx, runID)
	if err != nil {
		return err
	}
	task, err := scanTask(tx.QueryRowContext(ctx, taskSelect+" WHERE task_id=?", run.TaskID))
	if err != nil {
		return err
	}
	if generation != run.Generation || task.Generation != generation || task.CurrentRunID != run.ID || run.Status != status {
		return ErrStaleRun
	}
	if err := change(tx, &task, &run); err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		return err
	}
	s.NotifyWork()
	return nil
}

func queueTaskExecutionChild(ctx context.Context, tx *sql.Tx, task *Task, parent TaskRun, kind string, attempt, review int64, now time.Time) error {
	run, err := insertTaskExecutionRun(ctx, tx, *task, kind, parent, attempt, review, now)
	if err != nil {
		return err
	}
	task.Revision++
	task.State, task.StageKey, task.CurrentRunID, task.ActiveGateID = TaskRunning, "queue", run.ID, ""
	task.UpdatedAt = now.UTC()
	if err := updateCurrentTaskTx(ctx, tx, *task, parent.ID, "queue", now); err != nil {
		return err
	}
	if err := insertTaskEvent(ctx, tx, *task, "run_queued"); err != nil {
		return err
	}
	return appendTaskExecutionEvent(ctx, tx, *task, run, "run.queued", map[string]any{"run_kind": kind, "parent_run_id": parent.ID}, now)
}

func insertTaskExecutionRun(ctx context.Context, tx *sql.Tx, task Task, kind string, parent TaskRun, attempt, review int64, now time.Time) (TaskRun, error) {
	id, err := newID("run")
	if err != nil {
		return TaskRun{}, err
	}
	effectiveCwd := cloneString(task.CwdOverride)
	if kind == "executor" {
		effectiveCwd, err = taskRunEffectiveCwdTx(ctx, tx, task)
		if err != nil {
			return TaskRun{}, err
		}
	}
	policy, err := taskExecutionPolicyTx(ctx, tx)
	if err != nil {
		return TaskRun{}, err
	}
	run := TaskRun{ID: id, TaskID: task.ID, InstanceName: "Task " + strings.ToUpper(kind[:1]) + kind[1:], Kind: kind,
		Status: "queued", AgentID: task.ExecutorAgentID, Generation: task.Generation, AttemptIndex: attempt, ReviewRound: review,
		ParentRunID: parent.ID, SelectionMode: "provider_default", ExecutorBackend: "provider", ExecutorAgentID: task.ExecutorAgentID,
		EffectiveCwd: effectiveCwd, ExecutionPolicy: policy, QueuedAt: now.UTC(), CreatedAt: now.UTC(), UpdatedAt: now.UTC()}
	var acpLaunch *AcpLaunch
	if kind == "executor" && task.ExecutorAcpConnectionRevision != nil {
		run.ExecutorBackend = "acp"
		acpLaunch, err = resolveAcpLaunchTx(ctx, tx, task)
		if err != nil {
			return TaskRun{}, err
		}
	} else if attempt > 0 && kind == parent.Kind {
		run.ProviderKind, run.ProviderAccountID, run.SelectionMode = parent.ProviderKind, parent.ProviderAccountID, parent.SelectionMode
		run.ModelProfile, run.ReasoningEffort, run.FastMode = cloneString(parent.ModelProfile), cloneString(parent.ReasoningEffort), parent.FastMode
	} else {
		role := HostedModelTaskReviewer
		if kind != "reviewer" {
			complexity := task.ExecutionComplexity
			if complexity == "" {
				complexity = "medium"
			}
			role, _, _ = taskModelPoolIdentity(complexity)
		}
		var model, effort sql.NullString
		var fast int
		if err = tx.QueryRowContext(ctx, `SELECT provider_kind,provider_account_id,selection_mode,model_profile,reasoning_effort,fast_mode FROM hosted_model_assignments WHERE role=?`, role).
			Scan(&run.ProviderKind, &run.ProviderAccountID, &run.SelectionMode, &model, &effort, &fast); err != nil {
			return TaskRun{}, ErrInvalidModelAssignments
		}
		if run.SelectionMode == string(ModelSelectionNoemaRecommended) {
			run.SelectionMode = "provider_default"
		}
		run.ModelProfile, run.ReasoningEffort, run.FastMode = nullStringPointer(model), nullStringPointer(effort), fast == 1
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO task_runs (run_id,task_id,task_generation,instance_name,run_kind,status,agent_id,attempt_index,
review_round,parent_run_id,provider_kind,provider_account_id,selection_mode,model_profile,reasoning_effort,fast_mode,executor_backend,
executor_agent_id,effective_cwd,max_provider_continuations,max_tool_calls,max_active_minutes,progress_audit_interval,
max_automatic_retries,max_review_rounds,queued_at_ms,created_at_ms,updated_at_ms)
VALUES (?,?,?,?,?,'queued',?,?,?,?, ?,?,?,?,?,?, ?,?,?, ?,?,?,?,?,?, ?,?,?)`,
		run.ID, run.TaskID, run.Generation, run.InstanceName, run.Kind, run.AgentID, run.AttemptIndex, run.ReviewRound, parent.ID,
		run.ProviderKind, run.ProviderAccountID, run.SelectionMode, nullableString(run.ModelProfile), nullableString(run.ReasoningEffort), run.FastMode,
		run.ExecutorBackend, run.ExecutorAgentID, nullableString(run.EffectiveCwd), policy.MaxProviderContinuations, policy.MaxToolCalls,
		policy.MaxActiveMinutes, policy.ProgressAuditInterval, policy.MaxAutomaticRetries, policy.MaxReviewRounds, millis(now), millis(now), millis(now))
	if err == nil && acpLaunch != nil {
		run.AcpLaunch = acpLaunch
		err = insertAcpLaunchTx(ctx, tx, run.ID, *acpLaunch, now)
	}
	return run, err
}

func resolveAcpLaunchTx(ctx context.Context, tx *sql.Tx, task Task) (*AcpLaunch, error) {
	if task.ExecutorAcpConnectionRevision == nil {
		return nil, nil
	}
	var launch AcpLaunch
	var arguments string
	var enabled int
	err := tx.QueryRowContext(ctx, `SELECT command,arguments_json,connection_revision,enabled
FROM acp_agents WHERE agent_id=?`, task.ExecutorAgentID).
		Scan(&launch.Command, &arguments, &launch.ConnectionRevision, &enabled)
	if err != nil || enabled != 1 || json.Unmarshal([]byte(arguments), &launch.Arguments) != nil {
		return nil, ErrInvalidAcpAgent
	}
	return &launch, nil
}

func insertAcpLaunchTx(ctx context.Context, tx *sql.Tx, runID string, launch AcpLaunch, now time.Time) error {
	id, err := newID("run_item")
	if err != nil {
		return err
	}
	payload, err := json.Marshal(map[string]any{"acp_launch": launch})
	if err != nil {
		return err
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO task_run_items
(item_id,run_id,sequence_index,round_index,item_kind,status,correlation_id,content_text,payload_json,created_at_ms,updated_at_ms)
VALUES (?,?,0,0,'progress_notice','completed','acp:launch','ACP Executor selected.',?,?,?)`,
		id, runID, string(payload), millis(now), millis(now))
	return err
}

func hydrateAcpRun(ctx context.Context, q interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}, run *TaskRun) error {
	if run.ExecutorBackend != "acp" {
		return nil
	}
	var launchJSON string
	err := q.QueryRowContext(ctx, `SELECT json_extract(payload_json,'$.acp_launch') FROM task_run_items
WHERE run_id=? AND correlation_id='acp:launch' AND json_type(payload_json,'$.acp_launch')='object'
ORDER BY sequence_index LIMIT 1`, run.ID).Scan(&launchJSON)
	if err != nil {
		return err
	}
	var launch AcpLaunch
	if json.Unmarshal([]byte(launchJSON), &launch) != nil || launch.ConnectionRevision < 1 || launch.Command == "" {
		return errors.New("invalid ACP launch snapshot")
	}
	run.AcpLaunch = &launch
	var session string
	err = q.QueryRowContext(ctx, `SELECT json_extract(payload_json,'$.acp_session_id') FROM task_run_items
WHERE run_id=? AND correlation_id='acp:session' AND json_type(payload_json,'$.acp_session_id')='text'
ORDER BY sequence_index DESC LIMIT 1`, run.ID).Scan(&session)
	if err == nil {
		run.AcpSessionID = &session
		return nil
	}
	if errors.Is(err, sql.ErrNoRows) {
		return nil
	}
	return err
}

func openTaskExecutionGate(ctx context.Context, tx *sql.Tx, task *Task, run TaskRun, kind, prompt, detail, reason, retryKind string, now time.Time) error {
	return openTaskExecutionGateWithAnswers(ctx, tx, task, run, kind, prompt, detail, reason, retryKind, nil, now)
}

func openTaskExecutionGateWithAnswers(ctx context.Context, tx *sql.Tx, task *Task, run TaskRun, kind, prompt, detail, reason, retryKind string, answers []string, now time.Time) error {
	id, err := newID("gate")
	if err != nil {
		return err
	}
	encoded, _ := json.Marshal(answers)
	_, err = tx.ExecContext(ctx, `INSERT INTO task_gates (gate_id,task_id,task_generation,gate_kind,gate_state,recovery_reason,retry_run_kind,
prompt,context_markdown,suggested_answers_json,opened_by,originating_run_id,opened_at_ms) VALUES (?,?,?,?,'open',NULLIF(?,''),NULLIF(?,''),?,?,?,'actor:agent:task-runtime',?,?)`,
		id, task.ID, task.Generation, kind, reason, retryKind, prompt, detail, string(encoded), run.ID, millis(now))
	if err != nil {
		return err
	}
	if kind != "recovery" {
		if _, err = tx.ExecContext(ctx, `UPDATE task_runs SET status='waiting_for_approval',ended_at_ms=NULL,updated_at_ms=? WHERE run_id=?`, millis(now), run.ID); err != nil {
			return err
		}
	}
	task.Revision++
	task.State, task.StageKey, task.ActiveGateID, task.CurrentRunID = TaskFailed, "waiting", id, ""
	task.UpdatedAt = now.UTC()
	changed, err := tx.ExecContext(ctx, `UPDATE tasks SET state='failed',stage_key='waiting',current_run_id=NULL,active_gate_id=?,revision=?,updated_at_ms=?
WHERE task_id=? AND current_run_id=? AND generation=?`, id, task.Revision, millis(now), task.ID, run.ID, run.Generation)
	if err != nil {
		return err
	}
	if count, _ := changed.RowsAffected(); count != 1 {
		return ErrStaleRun
	}
	if err := insertTaskEvent(ctx, tx, *task, "gate_opened"); err != nil {
		return err
	}
	return appendTaskExecutionEvent(ctx, tx, *task, run, "gate.opened", map[string]any{"gate_id": id, "gate_kind": kind, "recovery_reason": reason}, now)
}

func updateCurrentTaskTx(ctx context.Context, tx *sql.Tx, task Task, expectedRunID, stage string, now time.Time) error {
	changed, err := tx.ExecContext(ctx, `UPDATE tasks SET state='running',stage_key=?,current_run_id=?,active_gate_id=NULL,revision=?,updated_at_ms=?
WHERE task_id=? AND current_run_id=? AND generation=?`, stage, task.CurrentRunID, task.Revision, millis(now), task.ID, expectedRunID, task.Generation)
	if err != nil {
		return err
	}
	if count, _ := changed.RowsAffected(); count != 1 {
		return ErrStaleRun
	}
	return nil
}

func appendTaskExecutionEvent(ctx context.Context, tx *sql.Tx, task Task, run TaskRun, kind string, details map[string]any, now time.Time) error {
	payload := map[string]any{"v": 1, "revision": task.Revision, "generation": task.Generation}
	for key, value := range details {
		payload[key] = value
	}
	_, err := insertWorkEvent(ctx, tx, personalWorkspaceIDStore, task.ProjectID, task.ID, run.ID, task.Revision,
		kind, "actor:agent:task-runtime", nil, "correlation:run:"+run.ID, payload, now)
	return err
}

func validTaskRunItem(item TaskRunItemInput) bool {
	validKind := item.Kind == "model_input" || item.Kind == "assistant_output" || item.Kind == "tool_call" ||
		item.Kind == "tool_result" || item.Kind == "progress_notice" || item.Kind == "task_submission" ||
		item.Kind == "task_review" || item.Kind == "failure" || item.Kind == "cancellation"
	validStatus := item.Status == "pending" || item.Status == "running" || item.Status == "completed" || item.Status == "failed" || item.Status == "skipped"
	return validKind && validStatus && item.Round >= 0 && len(item.Content) <= 512<<10
}

func max64(left, right int64) int64 {
	if left > right {
		return left
	}
	return right
}
