package store

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

// TaskUpdate contains the optional Inbox replacements for one Task.
type TaskUpdate struct {
	Title, ProjectID, ExecutorAgentID, CwdOverride *string
	SetProject, SetCwd                             bool
	DocumentDigest                                 string
}

// TaskRun is one bounded Task execution record.
type TaskRun struct {
	ID, TaskID, InstanceName, Kind, Status, AgentID string
	Generation, AttemptIndex, ReviewRound           int64
	ParentRunID                                     string
	ProviderKind, ProviderAccountID, SelectionMode  string
	ModelProfile, ReasoningEffort                   *string
	FastMode                                        bool
	ExecutorBackend, ExecutorAgentID                string
	EffectiveCwd                                    *string
	ErrorCode, ErrorMessage                         *string
	ProviderCallCount, ToolCallCount                int64
	InputTokens, CachedInputTokens, OutputTokens    int64
	ActiveMilliseconds                              int64
	QueuedAt, CreatedAt, UpdatedAt                  time.Time
	StartedAt, EndedAt                              *time.Time
}

// TaskGate is one durable human gate.
type TaskGate struct {
	ID, TaskID, Kind, State, Prompt, Context, OpenedBy string
	Generation                                         int64
	RecoveryReason, RetryRunKind, OriginatingRunID     *string
	SuggestedAnswers                                   []string
	OpenedAt                                           time.Time
	ResolvedBy, ResolutionMessageID                    *string
	ResolvedAt                                         *time.Time
}

// AllowsResolution applies the durable gate continuation policy.
func (g TaskGate) AllowsResolution(resolution string) bool {
	reason, retry := "", ""
	if g.RecoveryReason != nil {
		reason = *g.RecoveryReason
	}
	if g.RetryRunKind != nil {
		retry = *g.RetryRunKind
	}
	switch g.Kind {
	case "clarification", "approval":
		return reason == "" && retry == "" && resolution == "answer"
	case "recovery":
		switch reason {
		case "infrastructure_retries_exhausted":
			return retry != "" && (resolution == "answer" || resolution == "retry")
		case "review_rounds_exhausted":
			return retry == "executor" && resolution == "retry"
		case "unsafe_effect_uncertain":
			return retry != "" && resolution == "answer"
		case "configuration_unavailable":
			return retry != "" && resolution == "retry"
		}
	}
	return false
}

// TaskMessage is one human Task message.
type TaskMessage struct {
	ID, TaskID, Kind, Body, Author string
	Generation                     int64
	GateID, ApprovalDecision       *string
	CreatedAt                      time.Time
}

// TaskRunItem is one saved run transcript item.
type TaskRunItem struct {
	ID, RunID, Kind, Status          string
	Sequence, Round                  int64
	CorrelationID, ParentID, Content *string
	Payload                          map[string]any
	CreatedAt, UpdatedAt             time.Time
}

// TaskListFilter selects one stable Task page.
type TaskListFilter struct {
	ProjectID     string
	StageKeys     []string
	Scope         string
	AttentionOnly bool
}

// TaskPage is one stable Task list page.
type TaskPage struct {
	Tasks       []Task
	Cursors     []string
	EndCursor   *string
	HasNextPage bool
}

// TaskRunItemPage is one bounded run transcript page.
type TaskRunItemPage struct {
	Items       []TaskRunItem
	Cursors     []string
	EndCursor   *string
	HasNextPage bool
}

// TaskDocumentReceipt returns one committed staged document result.
func (s *Store) TaskDocumentReceipt(ctx context.Context, taskID, requestDigest string) (string, bool, error) {
	var response string
	err := s.db.QueryRowContext(ctx, `SELECT response_json FROM command_receipts
WHERE result_task_id=? AND request_digest=? ORDER BY result_event_id DESC LIMIT 1`, taskID, requestDigest).Scan(&response)
	if errors.Is(err, sql.ErrNoRows) {
		return "", false, nil
	}
	if err != nil {
		return "", false, err
	}
	var result TaskCommandResult
	if err := json.Unmarshal([]byte(response), &result); err != nil || result.Task.ID != taskID || result.DocumentDigest == "" {
		return "", false, errors.New("invalid Task document receipt")
	}
	return result.DocumentDigest, true, nil
}

// UpdateInboxTask replaces selected capture fields in one transaction.
func (s *Store) UpdateInboxTask(ctx context.Context, id string, revision, generation int64,
	changes TaskUpdate, command TaskCommand, now time.Time) (TaskCommandResult, error) {
	return s.taskLifecycleCommand(ctx, command, func(tx *sql.Tx) (TaskCommandResult, error) {
		task, err := fencedTaskTx(ctx, tx, id, revision, generation)
		if err != nil {
			return TaskCommandResult{}, err
		}
		if task.StageKey != "inbox" {
			return TaskCommandResult{}, ErrInvalidTransition
		}
		if changes.Title != nil {
			value := strings.TrimSpace(*changes.Title)
			if value == "" || len(value) > 500 {
				return TaskCommandResult{}, errors.New("invalid Task title")
			}
			task.Title = value
		}
		if changes.SetProject {
			project := ""
			if changes.ProjectID != nil {
				project = *changes.ProjectID
			}
			if err := validateTaskProjectTx(ctx, tx, project); err != nil {
				return TaskCommandResult{}, err
			}
			task.ProjectID = project
		}
		if changes.ExecutorAgentID != nil {
			revision, err := validateTaskExecutorTx(ctx, tx, *changes.ExecutorAgentID)
			if err != nil {
				return TaskCommandResult{}, err
			}
			task.ExecutorAgentID, task.ExecutorAcpConnectionRevision = *changes.ExecutorAgentID, revision
		}
		if changes.SetCwd {
			task.CwdOverride = nil
			if changes.CwdOverride != nil {
				value := strings.TrimSpace(*changes.CwdOverride)
				if value == "" || value != *changes.CwdOverride || !filepath.IsAbs(value) {
					return TaskCommandResult{}, errors.New("invalid Task cwdOverride")
				}
				task.CwdOverride = &value
			}
		}
		task.Revision++
		task.UpdatedAt = now.UTC()
		result, err := tx.ExecContext(ctx, `UPDATE tasks SET title=?, project_id=NULLIF(?,''), executor_agent_id=?,
executor_acp_connection_revision=?, cwd_override=?, revision=?, updated_at_ms=?
WHERE task_id=? AND revision=? AND generation=?`, task.Title, task.ProjectID, task.ExecutorAgentID,
			nullableInt(task.ExecutorAcpConnectionRevision), nullableString(task.CwdOverride), task.Revision,
			millis(task.UpdatedAt), id, revision, generation)
		if err != nil {
			return TaskCommandResult{}, err
		}
		if count, _ := result.RowsAffected(); count != 1 {
			return TaskCommandResult{}, ErrStaleRevision
		}
		return finishTaskLifecycleTx(ctx, tx, task, command, "task.updated", "", changes.DocumentDigest, now)
	})
}

// QueueTask creates the initial Planner run for one Inbox Task.
func (s *Store) QueueTask(ctx context.Context, id string, revision, generation int64,
	command TaskCommand, now time.Time) (TaskCommandResult, error) {
	return s.queueTask(ctx, id, revision, generation, "planner", nil, command, now)
}

func (s *Store) queueTask(ctx context.Context, id string, revision, generation int64, kind string,
	parent *TaskRun, command TaskCommand, now time.Time) (TaskCommandResult, error) {
	return s.taskLifecycleCommand(ctx, command, func(tx *sql.Tx) (TaskCommandResult, error) {
		task, err := fencedTaskTx(ctx, tx, id, revision, generation)
		if err != nil {
			return TaskCommandResult{}, err
		}
		if kind == "planner" && (task.StageKey != "inbox" || task.ScheduledFor != nil && task.ScheduleProcessedAt == nil) {
			return TaskCommandResult{}, ErrInvalidTransition
		}
		run, err := insertQueuedTaskRun(ctx, tx, task, kind, parent, now)
		if err != nil {
			return TaskCommandResult{}, err
		}
		task.State, task.StageKey, task.CurrentRunID = TaskRunning, "queue", run.ID
		task.ActiveGateID = ""
		task.Revision++
		task.UpdatedAt = now.UTC()
		changed, err := tx.ExecContext(ctx, `UPDATE tasks SET state='running', stage_key='queue', current_run_id=?,
active_gate_id=NULL, revision=?, updated_at_ms=? WHERE task_id=? AND revision=? AND generation=?`,
			run.ID, task.Revision, millis(task.UpdatedAt), id, revision, generation)
		if err != nil {
			return TaskCommandResult{}, err
		}
		if count, _ := changed.RowsAffected(); count != 1 {
			return TaskCommandResult{}, ErrStaleRevision
		}
		return finishTaskLifecycleTx(ctx, tx, task, command, "task.queued", run.ID, "", now)
	})
}

// CancelTask stops all current work and advances the Task generation.
func (s *Store) CancelTask(ctx context.Context, id string, revision, generation int64,
	reason string, command TaskCommand, now time.Time) (TaskCommandResult, error) {
	return s.taskLifecycleCommand(ctx, command, func(tx *sql.Tx) (TaskCommandResult, error) {
		task, err := fencedTaskTx(ctx, tx, id, revision, generation)
		if err != nil {
			return TaskCommandResult{}, err
		}
		if task.StageKey == "done" || task.StageKey == "cancelled" {
			return TaskCommandResult{}, ErrInvalidTransition
		}
		_, err = tx.ExecContext(ctx, `UPDATE task_runs SET status='cancelled', ended_at_ms=?, updated_at_ms=?
WHERE task_id=? AND status IN ('queued','leased','running','waiting_for_approval')`, millis(now), millis(now), id)
		if err != nil {
			return TaskCommandResult{}, err
		}
		_, err = tx.ExecContext(ctx, `UPDATE task_gates SET gate_state='superseded', resolved_by='actor:human:local', resolved_at_ms=?
WHERE gate_id=? AND gate_state='open'`, millis(now), task.ActiveGateID)
		if err != nil {
			return TaskCommandResult{}, err
		}
		task.Generation++
		task.Revision++
		task.State, task.StageKey, task.CurrentRunID, task.ActiveGateID = TaskCancelled, "cancelled", "", ""
		task.CancelledAt, task.CompletedAt, task.UpdatedAt = timeAddress(now.UTC()), nil, now.UTC()
		_, err = tx.ExecContext(ctx, `UPDATE tasks SET state='cancelled', stage_key='cancelled', current_run_id=NULL,
active_gate_id=NULL, generation=?, revision=?, cancelled_at_ms=?, completed_at_ms=NULL, updated_at_ms=?
WHERE task_id=? AND revision=? AND generation=?`, task.Generation, task.Revision, millis(now), millis(now), id, revision, generation)
		if err != nil {
			return TaskCommandResult{}, err
		}
		return finishTaskLifecycleTx(ctx, tx, task, command, "task.cancelled", "", "", now,
			map[string]any{"reason_present": reason != ""})
	})
}

// ReopenTask creates one new Executor generation for terminal history.
func (s *Store) ReopenTask(ctx context.Context, id string, revision, generation int64, direction string,
	complexity *string, documentDigest string, command TaskCommand, now time.Time) (TaskCommandResult, error) {
	return s.taskLifecycleCommand(ctx, command, func(tx *sql.Tx) (TaskCommandResult, error) {
		task, err := fencedTaskTx(ctx, tx, id, revision, generation)
		if err != nil {
			return TaskCommandResult{}, err
		}
		if task.StageKey != "done" && task.StageKey != "cancelled" {
			return TaskCommandResult{}, ErrInvalidTransition
		}
		task.Generation++
		if complexity != nil {
			task.ExecutionComplexity = *complexity
		}
		messageID, err := insertTaskMessage(ctx, tx, task, "", "human_change_request", direction, nil, now)
		if err != nil {
			return TaskCommandResult{}, err
		}
		_ = messageID
		run, err := insertQueuedTaskRun(ctx, tx, task, "executor", nil, now)
		if err != nil {
			return TaskCommandResult{}, err
		}
		task.Revision++
		task.State, task.StageKey, task.CurrentRunID = TaskRunning, "queue", run.ID
		task.CompletedAt, task.CancelledAt, task.UpdatedAt = nil, nil, now.UTC()
		_, err = tx.ExecContext(ctx, `UPDATE tasks SET state='running', stage_key='queue', current_run_id=?, generation=?,
revision=?, active_gate_id=NULL, completed_at_ms=NULL, cancelled_at_ms=NULL, execution_complexity=NULLIF(?,''), updated_at_ms=?
WHERE task_id=? AND revision=? AND generation=?`, run.ID, task.Generation, task.Revision,
			task.ExecutionComplexity, millis(now), id, revision, generation)
		if err != nil {
			return TaskCommandResult{}, err
		}
		return finishTaskLifecycleTx(ctx, tx, task, command, "task.reopened", run.ID, documentDigest, now)
	})
}

// ResolveTaskGate saves an Answer or Retry and queues its continuation.
func (s *Store) ResolveTaskGate(ctx context.Context, id, gateID string, revision, generation int64,
	body, resolution string, approvalDecision *string, command TaskCommand, now time.Time) (TaskCommandResult, error) {
	return s.taskLifecycleCommand(ctx, command, func(tx *sql.Tx) (TaskCommandResult, error) {
		task, err := fencedTaskTx(ctx, tx, id, revision, generation)
		if err != nil {
			return TaskCommandResult{}, err
		}
		if task.StageKey != "waiting" || task.ActiveGateID != gateID {
			return TaskCommandResult{}, ErrInvalidTransition
		}
		gate, err := taskGateTx(ctx, tx, gateID)
		if err != nil || gate.State != "open" || gate.Generation != generation {
			return TaskCommandResult{}, ErrInvalidTransition
		}
		if !gate.AllowsResolution(resolution) || gate.Kind == "approval" != (approvalDecision != nil) {
			return TaskCommandResult{}, ErrInvalidTransition
		}
		if approvalDecision != nil && *approvalDecision != "approved" && *approvalDecision != "declined" {
			return TaskCommandResult{}, errors.New("invalid approval decision")
		}
		messageID, err := insertTaskMessage(ctx, tx, task, gateID, map[bool]string{true: "retry_note", false: "human_answer"}[resolution == "retry"], body, approvalDecision, now)
		if err != nil {
			return TaskCommandResult{}, err
		}
		_, err = tx.ExecContext(ctx, `UPDATE task_gates SET gate_state='resolved', resolved_by='actor:human:local',
resolved_at_ms=?, resolution_message_id=? WHERE gate_id=? AND gate_state='open'`, millis(now), messageID, gateID)
		if err != nil {
			return TaskCommandResult{}, err
		}
		kind := "executor"
		if gate.RetryRunKind != nil {
			kind = *gate.RetryRunKind
		}
		var parent *TaskRun
		if gate.OriginatingRunID != nil {
			value, loadErr := taskRunTx(ctx, tx, *gate.OriginatingRunID)
			if loadErr != nil {
				return TaskCommandResult{}, loadErr
			}
			parent = &value
			if parent.Status == "waiting_for_approval" {
				if _, err = tx.ExecContext(ctx, `UPDATE task_runs SET status='completed',ended_at_ms=?,updated_at_ms=? WHERE run_id=? AND status='waiting_for_approval'`, millis(now), millis(now), parent.ID); err != nil {
					return TaskCommandResult{}, err
				}
				parent.Status, parent.EndedAt, parent.UpdatedAt = "completed", timeAddress(now.UTC()), now.UTC()
			}
		}
		if parent != nil {
			kind = parent.Kind
		}
		run, err := insertQueuedTaskRun(ctx, tx, task, kind, parent, now)
		if err != nil {
			return TaskCommandResult{}, err
		}
		task.Revision++
		task.State, task.StageKey, task.CurrentRunID, task.ActiveGateID = TaskRunning, "queue", run.ID, ""
		task.UpdatedAt = now.UTC()
		_, err = tx.ExecContext(ctx, `UPDATE tasks SET state='running', stage_key='queue', current_run_id=?, active_gate_id=NULL,
revision=?, updated_at_ms=? WHERE task_id=? AND revision=? AND generation=?`, run.ID, task.Revision, millis(now), id, revision, generation)
		if err != nil {
			return TaskCommandResult{}, err
		}
		return finishTaskLifecycleTx(ctx, tx, task, command, "gate.resolved", run.ID, "", now)
	})
}

func (s *Store) taskLifecycleCommand(ctx context.Context, command TaskCommand,
	change func(*sql.Tx) (TaskCommandResult, error)) (TaskCommandResult, error) {
	if err := validateTaskCommand(command); err != nil {
		return TaskCommandResult{}, err
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return TaskCommandResult{}, err
	}
	defer tx.Rollback()
	if replay, found, err := lookupTaskReceiptTx(ctx, tx, command); err != nil || found {
		return replay, err
	}
	result, err := change(tx)
	if err != nil {
		return TaskCommandResult{}, err
	}
	if err := storeTaskReceiptTx(ctx, tx, command, result, time.Now()); err != nil {
		return TaskCommandResult{}, err
	}
	if err := tx.Commit(); err != nil {
		return TaskCommandResult{}, err
	}
	s.NotifyWork()
	return result, nil
}

func fencedTaskTx(ctx context.Context, tx *sql.Tx, id string, revision, generation int64) (Task, error) {
	if !validTaskID(id) || revision < 1 || generation < 1 {
		return Task{}, errors.New("invalid Task current-run check")
	}
	task, err := scanTask(tx.QueryRowContext(ctx, taskSelect+" WHERE task_id=?", id))
	if err != nil {
		return Task{}, err
	}
	if task.Revision != revision || task.Generation != generation {
		return Task{}, ErrStaleRevision
	}
	return task, nil
}

func finishTaskLifecycleTx(ctx context.Context, tx *sql.Tx, task Task, command TaskCommand,
	kind, runID, documentDigest string, now time.Time, details ...map[string]any) (TaskCommandResult, error) {
	if err := insertTaskEvent(ctx, tx, task, strings.ReplaceAll(kind, ".", "_")); err != nil {
		return TaskCommandResult{}, err
	}
	payload := map[string]any{"v": 1, "revision": task.Revision, "generation": task.Generation}
	if len(details) != 0 {
		for key, value := range details[0] {
			payload[key] = value
		}
	}
	event, err := insertWorkEvent(ctx, tx, personalWorkspaceIDStore, task.ProjectID, task.ID, runID,
		task.Revision, kind, "actor:human:local", nil, command.CorrelationID,
		payload, now.UTC())
	return TaskCommandResult{Task: task, Event: event, DocumentDigest: documentDigest}, err
}

func insertQueuedTaskRun(ctx context.Context, tx *sql.Tx, task Task, kind string, parent *TaskRun, now time.Time) (TaskRun, error) {
	id, err := newID("run")
	if err != nil {
		return TaskRun{}, err
	}
	attempt, review, parentID := int64(0), int64(0), ""
	if kind != "planner" {
		review = 1
	}
	if parent != nil {
		attempt, review, parentID = parent.AttemptIndex+1, parent.ReviewRound, parent.ID
	}
	backend := "provider"
	if task.ExecutorAcpConnectionRevision != nil {
		backend = "acp"
	}
	run := TaskRun{ID: id, TaskID: task.ID, InstanceName: "Task " + strings.ToUpper(kind[:1]) + kind[1:], Kind: kind,
		Status: "queued", AgentID: task.ExecutorAgentID, Generation: task.Generation, AttemptIndex: attempt,
		ReviewRound: review, ParentRunID: parentID, SelectionMode: "provider_default", ExecutorBackend: backend,
		ExecutorAgentID: task.ExecutorAgentID, EffectiveCwd: cloneString(task.CwdOverride), QueuedAt: now.UTC(), CreatedAt: now.UTC(), UpdatedAt: now.UTC()}
	if backend == "provider" {
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
		err = tx.QueryRowContext(ctx, `SELECT provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, fast_mode
FROM hosted_model_assignments WHERE role=?`, role).Scan(&run.ProviderKind, &run.ProviderAccountID, &run.SelectionMode, &model, &effort, &fast)
		if err != nil {
			return TaskRun{}, ErrInvalidModelAssignments
		}
		if run.SelectionMode == string(ModelSelectionNoemaRecommended) {
			run.SelectionMode = "provider_default"
		}
		run.ModelProfile, run.ReasoningEffort, run.FastMode = nullStringPointer(model), nullStringPointer(effort), fast == 1
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO task_runs (run_id,task_id,task_generation,instance_name,run_kind,status,agent_id,
attempt_index,review_round,parent_run_id,provider_kind,provider_account_id,selection_mode,model_profile,reasoning_effort,fast_mode,
executor_backend,executor_agent_id,effective_cwd,queued_at_ms,created_at_ms,updated_at_ms)
VALUES (?,?,?,?,?,'queued',?,?,?,NULLIF(?,''),?,?,?,?,?,?, ?,?,?, ?,?,?)`, run.ID, run.TaskID, run.Generation,
		run.InstanceName, run.Kind, run.AgentID, run.AttemptIndex, run.ReviewRound, run.ParentRunID,
		run.ProviderKind, run.ProviderAccountID, run.SelectionMode, nullableString(run.ModelProfile), nullableString(run.ReasoningEffort), run.FastMode,
		run.ExecutorBackend, run.ExecutorAgentID, nullableString(run.EffectiveCwd), millis(now), millis(now), millis(now))
	return run, err
}

func insertTaskMessage(ctx context.Context, tx *sql.Tx, task Task, gateID, kind, body string, approval *string, now time.Time) (string, error) {
	id, err := newID("task_message")
	if err != nil {
		return "", err
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO task_messages
(message_id,task_id,task_generation,gate_id,message_kind,body_markdown,approval_decision,author_actor_id,created_at_ms)
VALUES (?,?,?,NULLIF(?,''),?,?,?, 'actor:human:local',?)`, id, task.ID, task.Generation, gateID, kind, body, nullableString(approval), millis(now))
	return id, err
}

func (s *Store) ListTasks(ctx context.Context, filter TaskListFilter, first int, after *string) (TaskPage, error) {
	return listTasks(ctx, s.db, filter, first, after)
}

func listTasks(ctx context.Context, query interface {
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
}, filter TaskListFilter, first int, after *string) (TaskPage, error) {
	if first < 1 || first > 100 || filter.Scope != "active" && filter.Scope != "terminal" && filter.Scope != "all" {
		return TaskPage{}, errors.New("invalid Task list")
	}
	hashBytes, _ := json.Marshal(filter)
	hash := sha256.Sum256(hashBytes)
	queryHash := hex.EncodeToString(hash[:])
	var afterTime int64
	var afterID string
	if after != nil {
		var err error
		afterTime, afterID, err = decodeTaskCursor(*after, queryHash)
		if err != nil {
			return TaskPage{}, err
		}
	}
	clauses := []string{"(?='' OR COALESCE(project_id,'')=?)", "(?=0 OR updated_at_ms<? OR (updated_at_ms=? AND task_id<?))"}
	arguments := []any{filter.ProjectID, filter.ProjectID, afterTime, afterTime, afterTime, afterID}
	if filter.Scope == "active" {
		clauses = append(clauses, "stage_key NOT IN ('done','cancelled')")
	}
	if filter.Scope == "terminal" {
		clauses = append(clauses, "stage_key IN ('done','cancelled')")
	}
	if filter.AttentionOnly {
		clauses = append(clauses, "active_gate_id IS NOT NULL")
	}
	if len(filter.StageKeys) > 0 {
		marks := make([]string, len(filter.StageKeys))
		for i, stage := range filter.StageKeys {
			marks[i] = "?"
			arguments = append(arguments, stage)
		}
		clauses = append(clauses, "stage_key IN ("+strings.Join(marks, ",")+")")
	}
	arguments = append(arguments, first+1)
	rows, err := query.QueryContext(ctx, taskSelect+" WHERE "+strings.Join(clauses, " AND ")+" ORDER BY updated_at_ms DESC, task_id DESC LIMIT ?", arguments...)
	if err != nil {
		return TaskPage{}, err
	}
	defer rows.Close()
	values := make([]Task, 0, first+1)
	for rows.Next() {
		value, err := scanTask(rows)
		if err != nil {
			return TaskPage{}, err
		}
		values = append(values, value)
	}
	page := TaskPage{HasNextPage: len(values) > first}
	if page.HasNextPage {
		values = values[:first]
	}
	page.Tasks = values
	for _, value := range values {
		page.Cursors = append(page.Cursors, encodeTaskCursor(queryHash, millis(value.UpdatedAt), value.ID))
	}
	if len(page.Cursors) > 0 {
		value := page.Cursors[len(page.Cursors)-1]
		page.EndCursor = &value
	}
	return page, rows.Err()
}

// TaskOverview returns recent active Tasks and exact stage counts from one snapshot.
func (s *Store) TaskOverview(ctx context.Context, projectID string, first int) (TaskPage, map[string]int, int, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{ReadOnly: true})
	if err != nil {
		return TaskPage{}, nil, 0, err
	}
	defer tx.Rollback()
	page, err := listTasks(ctx, tx, TaskListFilter{ProjectID: projectID, Scope: "active"}, first, nil)
	if err != nil {
		return TaskPage{}, nil, 0, err
	}
	rows, err := tx.QueryContext(ctx, `SELECT stage_key,COUNT(*),SUM(active_gate_id IS NOT NULL)
FROM tasks WHERE stage_key NOT IN ('done','cancelled') AND (?='' OR COALESCE(project_id,'')=?) GROUP BY stage_key`, projectID, projectID)
	if err != nil {
		return TaskPage{}, nil, 0, err
	}
	defer rows.Close()
	counts, needs := map[string]int{}, 0
	for rows.Next() {
		var stage string
		var count, attention int
		if err := rows.Scan(&stage, &count, &attention); err != nil {
			return TaskPage{}, nil, 0, err
		}
		counts[stage], needs = count, needs+attention
	}
	if err := rows.Err(); err != nil {
		_ = rows.Close()
		return TaskPage{}, nil, 0, err
	}
	if err := rows.Close(); err != nil {
		return TaskPage{}, nil, 0, err
	}
	return page, counts, needs, tx.Commit()
}

func encodeTaskCursor(hash string, updated int64, id string) string {
	return base64.RawURLEncoding.EncodeToString([]byte(hash + "\x00" + strconv.FormatInt(updated, 10) + "\x00" + id))
}
func decodeTaskCursor(cursor, hash string) (int64, string, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(cursor)
	parts := strings.Split(string(decoded), "\x00")
	if err != nil || len(parts) != 3 || parts[0] != hash || !validTaskID(parts[2]) {
		return 0, "", ErrInvalidCursor
	}
	value, err := strconv.ParseInt(parts[1], 10, 64)
	if err != nil || value < 1 {
		return 0, "", ErrInvalidCursor
	}
	return value, parts[2], nil
}

func (s *Store) TaskRuns(ctx context.Context, taskID string, limit int) ([]TaskRun, error) {
	rows, err := s.db.QueryContext(ctx, taskRunSelect+" WHERE task_id=? ORDER BY created_at_ms DESC,run_id DESC LIMIT ?", taskID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	values := []TaskRun{}
	for rows.Next() {
		value, err := scanTaskRun(rows)
		if err != nil {
			return nil, err
		}
		values = append(values, value)
	}
	return values, rows.Err()
}
func (s *Store) TaskMessages(ctx context.Context, taskID string, limit int) ([]TaskMessage, error) {
	rows, err := s.db.QueryContext(ctx, `SELECT message_id,task_id,task_generation,gate_id,message_kind,body_markdown,approval_decision,author_actor_id,created_at_ms FROM task_messages WHERE task_id=? ORDER BY created_at_ms DESC,message_id DESC LIMIT ?`, taskID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	values := []TaskMessage{}
	for rows.Next() {
		var v TaskMessage
		var gate, approval sql.NullString
		var created int64
		if err := rows.Scan(&v.ID, &v.TaskID, &v.Generation, &gate, &v.Kind, &v.Body, &approval, &v.Author, &created); err != nil {
			return nil, err
		}
		v.GateID = nullStringPointer(gate)
		v.ApprovalDecision = nullStringPointer(approval)
		v.CreatedAt = fromMillis(created)
		values = append(values, v)
	}
	return values, rows.Err()
}
func (s *Store) TaskGate(ctx context.Context, id string) (TaskGate, error) {
	return taskGateTx(ctx, s.db, id)
}
func taskGateTx(ctx context.Context, q interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}, id string) (TaskGate, error) {
	var v TaskGate
	var recovery, retry, origin, resolved, resolution sql.NullString
	var answers string
	var opened int64
	var resolvedAt sql.NullInt64
	err := q.QueryRowContext(ctx, `SELECT gate_id,task_id,task_generation,gate_kind,gate_state,recovery_reason,retry_run_kind,prompt,context_markdown,suggested_answers_json,opened_by,originating_run_id,opened_at_ms,resolved_by,resolved_at_ms,resolution_message_id FROM task_gates WHERE gate_id=?`, id).Scan(&v.ID, &v.TaskID, &v.Generation, &v.Kind, &v.State, &recovery, &retry, &v.Prompt, &v.Context, &answers, &v.OpenedBy, &origin, &opened, &resolved, &resolvedAt, &resolution)
	if err != nil {
		return TaskGate{}, err
	}
	v.RecoveryReason = nullStringPointer(recovery)
	v.RetryRunKind = nullStringPointer(retry)
	v.OriginatingRunID = nullStringPointer(origin)
	v.ResolvedBy = nullStringPointer(resolved)
	v.ResolutionMessageID = nullStringPointer(resolution)
	v.ResolvedAt = nullTimePointer(resolvedAt)
	v.OpenedAt = fromMillis(opened)
	_ = json.Unmarshal([]byte(answers), &v.SuggestedAnswers)
	return v, nil
}
func taskRunTx(ctx context.Context, q interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}, id string) (TaskRun, error) {
	return scanTaskRun(q.QueryRowContext(ctx, taskRunSelect+" WHERE run_id=?", id))
}

const taskRunSelect = `SELECT run_id,task_id,instance_name,run_kind,status,agent_id,task_generation,attempt_index,review_round,COALESCE(parent_run_id,''),provider_kind,provider_account_id,selection_mode,model_profile,reasoning_effort,fast_mode,executor_backend,executor_agent_id,effective_cwd,error_code,error_message,provider_call_count,tool_call_count,input_tokens,cached_input_tokens,output_tokens,active_milliseconds,queued_at_ms,started_at_ms,ended_at_ms,created_at_ms,updated_at_ms FROM task_runs`

func scanTaskRun(row rowScanner) (TaskRun, error) {
	var v TaskRun
	var model, effort, cwd, code, message sql.NullString
	var started, ended sql.NullInt64
	var fast int
	var queued, created, updated int64
	err := row.Scan(&v.ID, &v.TaskID, &v.InstanceName, &v.Kind, &v.Status, &v.AgentID, &v.Generation, &v.AttemptIndex, &v.ReviewRound, &v.ParentRunID, &v.ProviderKind, &v.ProviderAccountID, &v.SelectionMode, &model, &effort, &fast, &v.ExecutorBackend, &v.ExecutorAgentID, &cwd, &code, &message, &v.ProviderCallCount, &v.ToolCallCount, &v.InputTokens, &v.CachedInputTokens, &v.OutputTokens, &v.ActiveMilliseconds, &queued, &started, &ended, &created, &updated)
	if err != nil {
		return v, err
	}
	v.ModelProfile = nullStringPointer(model)
	v.ReasoningEffort = nullStringPointer(effort)
	v.EffectiveCwd = nullStringPointer(cwd)
	v.ErrorCode = nullStringPointer(code)
	v.ErrorMessage = nullStringPointer(message)
	v.FastMode = fast == 1
	v.QueuedAt = fromMillis(queued)
	v.StartedAt = nullTimePointer(started)
	v.EndedAt = nullTimePointer(ended)
	v.CreatedAt = fromMillis(created)
	v.UpdatedAt = fromMillis(updated)
	return v, nil
}

func (s *Store) TaskRunItems(ctx context.Context, runID string, first int, after *string) (TaskRunItemPage, error) {
	if first < 1 || first > 100 {
		return TaskRunItemPage{}, errors.New("invalid run item page")
	}
	before := int64(1 << 62)
	if after != nil {
		decoded, err := base64.RawURLEncoding.DecodeString(*after)
		if err != nil {
			return TaskRunItemPage{}, ErrInvalidCursor
		}
		before, err = strconv.ParseInt(string(decoded), 10, 64)
		if err != nil {
			return TaskRunItemPage{}, ErrInvalidCursor
		}
	}
	rows, err := s.db.QueryContext(ctx, `SELECT item_id,run_id,sequence_index,round_index,item_kind,status,correlation_id,parent_item_id,content_text,payload_json,created_at_ms,updated_at_ms FROM task_run_items WHERE run_id=? AND sequence_index<? ORDER BY sequence_index DESC LIMIT ?`, runID, before, first+1)
	if err != nil {
		return TaskRunItemPage{}, err
	}
	defer rows.Close()
	values := []TaskRunItem{}
	for rows.Next() {
		var v TaskRunItem
		var correlation, parent, content sql.NullString
		var payload string
		var created, updated int64
		if err := rows.Scan(&v.ID, &v.RunID, &v.Sequence, &v.Round, &v.Kind, &v.Status, &correlation, &parent, &content, &payload, &created, &updated); err != nil {
			return TaskRunItemPage{}, err
		}
		v.CorrelationID = nullStringPointer(correlation)
		v.ParentID = nullStringPointer(parent)
		v.Content = nullStringPointer(content)
		v.CreatedAt = fromMillis(created)
		v.UpdatedAt = fromMillis(updated)
		if json.Unmarshal([]byte(payload), &v.Payload) != nil {
			return TaskRunItemPage{}, errors.New("invalid run item payload")
		}
		values = append(values, v)
	}
	page := TaskRunItemPage{HasNextPage: len(values) > first}
	if page.HasNextPage {
		values = values[:first]
	}
	page.Items = values
	for _, v := range values {
		page.Cursors = append(page.Cursors, base64.RawURLEncoding.EncodeToString([]byte(strconv.FormatInt(v.Sequence, 10))))
	}
	if len(page.Cursors) > 0 {
		value := page.Cursors[len(page.Cursors)-1]
		page.EndCursor = &value
	}
	return page, rows.Err()
}

func timeAddress(value time.Time) *time.Time { copy := value; return &copy }
