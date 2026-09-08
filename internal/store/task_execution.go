package store

import (
	"context"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"strings"
	"time"

	"github.com/uptrace/bun"
)

// TaskRunItemInput is one durable provider or tool transcript item.
type TaskRunItemInput struct {
	ID, Kind, Status, CorrelationID, ParentID, Content string
	Round                                              int64
	Payload                                            map[string]any
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
	if _, err = tx.ExecContext(ctx, `UPDATE task_runs SET status='queued',updated_at_ms=?
WHERE executor_backend='provider' AND status IN ('leased','running') AND run_id IN
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
WHERE r.status='queued' AND r.executor_backend='provider' AND t.executor_acp_connection_revision IS NULL AND r.task_generation=t.generation
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
	task, err := taskTx(ctx, tx, run.TaskID)
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
	return s.taskRunTransaction(ctx, runID, generation, "leased", func(tx bun.Tx, task *Task, run *TaskRun) error {
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
	return s.taskRunTransaction(ctx, runID, generation, "running", func(tx bun.Tx, _ *Task, run *TaskRun) error {
		var next int64
		if err := tx.QueryRowContext(ctx, `SELECT COALESCE(MAX(sequence_index)+1,0) FROM task_run_items WHERE run_id=?`, run.ID).Scan(&next); err != nil {
			return err
		}
		for _, item := range items {
			if !validTaskRunItem(item) {
				return errors.New("invalid Task run item")
			}
			id := item.ID
			var err error
			if id == "" {
				id, err = newID("run_item")
				if err != nil {
					return err
				}
			} else if !validTaskRunItemID(id) {
				return errors.New("invalid Task run item id")
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
	err := s.taskRunTransaction(ctx, runID, generation, runStatus, func(tx bun.Tx, task *Task, run *TaskRun) error {
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

// TaskRunContinuationItems admits the bounded Executor action lineage that a
// new current run may send to its provider. The lineage ends after the latest
// successful TASK.md save and excludes terminal control calls.
func (s *Store) TaskRunContinuationItems(ctx context.Context, runID string) ([]TaskRunItem, error) {
	run, err := taskRunTx(ctx, s.db, runID)
	if err != nil {
		return nil, err
	}
	if run.Kind != "executor" {
		return []TaskRunItem{}, nil
	}
	currentTask, err := taskTx(ctx, s.db, run.TaskID)
	if err != nil {
		return nil, err
	}
	if run.Status != "running" || currentTask.CurrentRunID != run.ID || currentTask.Generation != run.Generation {
		return nil, ErrStaleRun
	}

	const (
		maxLineageRuns        = 4
		maxItemsPerLineageRun = 24
		maxLineageItems       = 24
	)
	executorRuns := make([]TaskRun, 0, maxLineageRuns)
	seen := make(map[string]struct{}, maxLineageRuns)
	for current := run; current.ID != ""; {
		if _, exists := seen[current.ID]; exists {
			return nil, errors.New("Task run parent cycle")
		}
		seen[current.ID] = struct{}{}
		if current.TaskID != run.TaskID {
			return nil, errors.New("Task run parent crosses task boundary")
		}
		if current.Generation != run.Generation {
			break
		}
		if current.Kind == "executor" {
			executorRuns = append(executorRuns, current)
			if len(executorRuns) == maxLineageRuns {
				break
			}
		}
		if current.ParentRunID == "" {
			break
		}
		current, err = taskRunTx(ctx, s.db, current.ParentRunID)
		if err != nil {
			return nil, err
		}
	}

	items := make([]TaskRunItem, 0, maxLineageItems)
	for index := len(executorRuns) - 1; index >= 0; index-- {
		runItems, err := s.taskRunContinuationItemsForRun(ctx, executorRuns[index].ID, maxItemsPerLineageRun)
		if err != nil {
			return nil, err
		}
		items = append(items, runItems...)
	}
	afterSave := 0
	for index := len(items) - 1; index >= 0; index-- {
		if !successfulTaskDocumentSave(items[index]) {
			continue
		}
		afterSave = index + 1
		correlation := items[index].CorrelationID
		for next := index + 1; next < len(items); next++ {
			if correlation != nil && items[next].CorrelationID != nil &&
				*items[next].CorrelationID == *correlation && itemContent(items[next]) == "task.files.write" {
				afterSave = next + 1
			}
		}
		break
	}
	items = items[afterSave:]
	filtered := make([]TaskRunItem, 0, len(items))
	for _, item := range items {
		if isTaskExecutionControlItem(item) {
			continue
		}
		filtered = append(filtered, item)
	}
	filtered = continuationLineageOrder(filtered)
	if len(filtered) > maxLineageItems {
		filtered = filtered[len(filtered)-maxLineageItems:]
	}
	return filtered, nil
}

// TaskRunExecutionContext is the fenced context admitted before a provider
// receives a continuation run.
type TaskRunExecutionContext struct {
	Task    Task
	Run     TaskRun
	Lineage []TaskRunItem
}

// AdmitTaskRunExecutionContext reads one current continuation through the
// production run fence and returns the bounded action lineage for admission.
func (s *Store) AdmitTaskRunExecutionContext(ctx context.Context, runID string, generation int64) (TaskRunExecutionContext, error) {
	run, err := taskRunTx(ctx, s.db, runID)
	if err != nil {
		return TaskRunExecutionContext{}, err
	}
	task, err := taskTx(ctx, s.db, run.TaskID)
	if err != nil {
		return TaskRunExecutionContext{}, err
	}
	if run.Kind != "executor" || run.ParentRunID == "" || run.Status != "running" ||
		run.Generation != generation || task.Generation != generation || task.CurrentRunID != run.ID {
		return TaskRunExecutionContext{}, ErrStaleRun
	}
	lineage, err := s.TaskRunContinuationItems(ctx, run.ID)
	if err != nil {
		return TaskRunExecutionContext{}, err
	}
	return TaskRunExecutionContext{Task: task, Run: run, Lineage: lineage}, nil
}

func (s *Store) taskRunContinuationItemsForRun(ctx context.Context, runID string, limit int) ([]TaskRunItem, error) {
	rows, err := s.db.QueryContext(ctx, `SELECT item_id,run_id,sequence_index,round_index,item_kind,status,correlation_id,parent_item_id,content_text,payload_json,created_at_ms,updated_at_ms
FROM task_run_items WHERE run_id=? AND item_kind IN ('tool_call','tool_result')
ORDER BY sequence_index DESC,item_id DESC LIMIT ?`, runID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	items := make([]TaskRunItem, 0, limit)
	for rows.Next() {
		var item TaskRunItem
		var correlation, parent, content sql.NullString
		var payload string
		var created, updated int64
		if err := rows.Scan(&item.ID, &item.RunID, &item.Sequence, &item.Round, &item.Kind, &item.Status,
			&correlation, &parent, &content, &payload, &created, &updated); err != nil {
			return nil, err
		}
		item.CorrelationID, item.ParentID, item.Content = nullStringPointer(correlation), nullStringPointer(parent), nullStringPointer(content)
		item.CreatedAt, item.UpdatedAt = fromMillis(created), fromMillis(updated)
		if err := json.Unmarshal([]byte(payload), &item.Payload); err != nil {
			return nil, errors.New("invalid Task continuation item payload")
		}
		items = append(items, item)
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}
	for left, right := 0, len(items)-1; left < right; left, right = left+1, right-1 {
		items[left], items[right] = items[right], items[left]
	}
	return items, nil
}

func successfulTaskDocumentSave(item TaskRunItem) bool {
	if item.Kind != "tool_result" || item.Status != "completed" || itemContent(item) != "task.files.write" {
		return false
	}
	success, _ := item.Payload["success"].(bool)
	payload, _ := item.Payload["payload"].(map[string]any)
	path, _ := payload["path"].(string)
	return success && path == "TASK.md"
}

func isTaskExecutionControlItem(item TaskRunItem) bool {
	switch itemContent(item) {
	case "task.continue_execution", "task.finish_execution", "task.report_blocked":
		return true
	default:
		return false
	}
}

func itemContent(item TaskRunItem) string {
	if item.Content == nil {
		return ""
	}
	return *item.Content
}

// continuationLineageOrder presents each completed tool action in the same
// result-before-call order as the Rust context record.
func continuationLineageOrder(items []TaskRunItem) []TaskRunItem {
	calls := make(map[string]struct{})
	results := make(map[string]TaskRunItem)
	for _, item := range items {
		if item.Kind == "tool_call" {
			calls[item.ID] = struct{}{}
		}
		if item.Kind == "tool_result" && item.ParentID != nil {
			results[*item.ParentID] = item
		}
	}
	ordered := make([]TaskRunItem, 0, len(items))
	for _, item := range items {
		if item.Kind == "tool_result" && item.ParentID != nil {
			if _, hasCall := calls[*item.ParentID]; hasCall {
				continue
			}
		}
		if item.Kind == "tool_call" {
			if result, exists := results[item.ID]; exists {
				ordered = append(ordered, result, item)
				continue
			}
		}
		ordered = append(ordered, item)
	}
	return ordered
}

// FinishTaskPlanning completes the Planner and queues the first Executor.
func (s *Store) FinishTaskPlanning(ctx context.Context, runID string, generation int64, complexity string, now time.Time) error {
	if complexity != "simple" && complexity != "medium" && complexity != "difficult" {
		return errors.New("invalid Task complexity")
	}
	return s.completeTaskRun(ctx, runID, generation, "planner", now, func(tx bun.Tx, task *Task, run TaskRun) error {
		task.ExecutionComplexity = complexity
		if _, err := tx.ExecContext(ctx, `UPDATE tasks SET execution_complexity=? WHERE task_id=?`, complexity, task.ID); err != nil {
			return err
		}
		return queueTaskExecutionChild(ctx, tx, task, run, "executor", 0, 1, now)
	})
}

// FinishTaskExecution completes an Executor and queues review or continuation.
func (s *Store) FinishTaskExecution(ctx context.Context, runID string, generation int64, continueRun bool, now time.Time) error {
	return s.completeTaskRun(ctx, runID, generation, "executor", now, func(tx bun.Tx, task *Task, run TaskRun) error {
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
	return s.completeTaskRun(ctx, runID, generation, "reviewer", now, func(tx bun.Tx, task *Task, run TaskRun) error {
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
	return s.taskRunTransaction(ctx, runID, generation, "running", func(tx bun.Tx, task *Task, run *TaskRun) error {
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
	return s.taskRunTransaction(ctx, runID, generation, "running", func(tx bun.Tx, task *Task, run *TaskRun) error {
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
	return s.taskRunTransaction(ctx, runID, generation, "running", func(tx bun.Tx, task *Task, run *TaskRun) error {
		return markTaskExecutionUncertainTx(ctx, tx, task, *run, now)
	})
}

func markTaskExecutionUncertainTx(ctx context.Context, tx bun.Tx, task *Task, run TaskRun, now time.Time) error {
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

func (s *Store) completeTaskRun(ctx context.Context, runID string, generation int64, kind string, now time.Time, next func(bun.Tx, *Task, TaskRun) error) error {
	return s.taskRunTransaction(ctx, runID, generation, "running", func(tx bun.Tx, task *Task, run *TaskRun) error {
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

func (s *Store) taskRunTransaction(ctx context.Context, runID string, generation int64, status string, change func(bun.Tx, *Task, *TaskRun) error) error {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer tx.Rollback()
	run, err := taskRunTx(ctx, tx, runID)
	if err != nil {
		return err
	}
	task, err := taskTx(ctx, tx, run.TaskID)
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

func queueTaskExecutionChild(ctx context.Context, tx bun.Tx, task *Task, parent TaskRun, kind string, attempt, review int64, now time.Time) error {
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

func insertTaskExecutionRun(ctx context.Context, tx bun.Tx, task Task, kind string, parent TaskRun, attempt, review int64, now time.Time) (TaskRun, error) {
	if task.ExecutorAcpConnectionRevision != nil || parent.ExecutorBackend != "provider" {
		return TaskRun{}, ErrUnsupportedTaskExecutor
	}
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
	if attempt > 0 && kind == parent.Kind {
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
	return run, err
}

func openTaskExecutionGate(ctx context.Context, tx bun.Tx, task *Task, run TaskRun, kind, prompt, detail, reason, retryKind string, now time.Time) error {
	return openTaskExecutionGateWithAnswers(ctx, tx, task, run, kind, prompt, detail, reason, retryKind, nil, now)
}

func openTaskExecutionGateWithAnswers(ctx context.Context, tx bun.Tx, task *Task, run TaskRun, kind, prompt, detail, reason, retryKind string, answers []string, now time.Time) error {
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

func updateCurrentTaskTx(ctx context.Context, tx bun.Tx, task Task, expectedRunID, stage string, now time.Time) error {
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

func appendTaskExecutionEvent(ctx context.Context, tx bun.Tx, task Task, run TaskRun, kind string, details map[string]any, now time.Time) error {
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
		item.Kind == "task_review" || item.Kind == "failure" || item.Kind == "cancellation" || item.Kind == "context_checkpoint"
	validStatus := item.Status == "pending" || item.Status == "running" || item.Status == "completed" || item.Status == "failed" || item.Status == "skipped"
	return validKind && validStatus && item.Round >= 0 && len(item.Content) <= 512<<10
}

func validTaskRunItemID(id string) bool {
	value, ok := strings.CutPrefix(id, "run_item:")
	if !ok || len(value) != 32 {
		return false
	}
	decoded, err := hex.DecodeString(value)
	return err == nil && hex.EncodeToString(decoded) == value
}

func max64(left, right int64) int64 {
	if left > right {
		return left
	}
	return right
}
