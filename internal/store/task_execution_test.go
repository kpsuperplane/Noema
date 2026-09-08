package store

import (
	"context"
	"database/sql"
	"path/filepath"
	"reflect"
	"testing"
	"time"
)

func TestTaskExecutionPolicyUpgradeValidationAndClaimSnapshots(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v28.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec(schemaAtVersion(28) + `PRAGMA user_version=28;`); err != nil {
		t.Fatal(err)
	}
	_ = legacy.Close()
	database, err := Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	policy, err := database.TaskExecutionPolicy(t.Context())
	if err != nil || policy != (TaskExecutionPolicy{80, 400, 120, 20, 3, 3}) {
		t.Fatalf("default policy = %#v, %v", policy, err)
	}
	updated := TaskExecutionPolicy{42, 210, 90, 14, 0, 1}
	if policy, err = database.UpdateTaskExecutionPolicy(t.Context(), updated); err != nil || policy != updated {
		t.Fatalf("updated policy = %#v, %v", policy, err)
	}
	invalid := updated
	invalid.ProgressAuditInterval = 43
	if _, err = database.UpdateTaskExecutionPolicy(t.Context(), invalid); err == nil {
		t.Fatal("invalid policy was accepted")
	}
	account := createReadyModelAccount(t, database)
	if _, err = database.ConfirmHostedModelAssignments(t.Context(), account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	id, _ := NewTaskID()
	if _, err = database.CreateTask(t.Context(), id, "Snapshot", "correlation:policy:create", now); err != nil {
		t.Fatal(err)
	}
	if _, err = database.QueueTask(t.Context(), id, 1, 1, testTaskLifecycleCommand("queue_task", "policy"), now); err != nil {
		t.Fatal(err)
	}
	runs, err := database.TaskRuns(t.Context(), id, 10)
	if err != nil || len(runs) != 1 || runs[0].ExecutionPolicy != updated {
		t.Fatalf("queued policy = %#v, %v", runs, err)
	}
	_, run, found, err := database.ClaimTaskExecution(t.Context(), now)
	if err != nil || !found || run.ExecutionPolicy != updated {
		t.Fatalf("claimed policy = %#v, %t, %v", run.ExecutionPolicy, found, err)
	}
	if err = database.StartTaskExecution(t.Context(), run.ID, run.Generation, now); err != nil {
		t.Fatal(err)
	}
	if err = database.FinishTaskPlanning(t.Context(), run.ID, run.Generation, "simple", now); err != nil {
		t.Fatal(err)
	}
	runs, err = database.TaskRuns(t.Context(), id, 10)
	if err != nil || len(runs) != 2 || runs[0].ExecutionPolicy != updated {
		t.Fatalf("child queued policy = %#v, %v", runs, err)
	}
	refreshed := updated
	refreshed.MaxAutomaticRetries = 0
	if _, err = database.UpdateTaskExecutionPolicy(t.Context(), refreshed); err != nil {
		t.Fatal(err)
	}
	_, run, found, err = database.ClaimTaskExecution(t.Context(), now)
	if err != nil || !found || run.ExecutionPolicy != refreshed {
		t.Fatalf("refreshed claim policy = %#v, %t, %v", run.ExecutionPolicy, found, err)
	}
	if err = database.StartTaskExecution(t.Context(), run.ID, run.Generation, now); err != nil {
		t.Fatal(err)
	}
	if err = database.FailTaskExecution(t.Context(), run.ID, run.Generation, "failed", "Failed.", true, now); err != nil {
		t.Fatal(err)
	}
	task, err := database.Task(t.Context(), id)
	if err != nil || task.StageKey != "waiting" {
		t.Fatalf("zero-retry snapshot task = %#v, %v", task, err)
	}
}

func TestTaskExecutionRecoveryAndCurrentRunTransitions(t *testing.T) {
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(context.Background(), account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	id, _ := NewTaskID()
	if _, err := database.CreateTask(context.Background(), id, "Execution", "correlation:execution:create", now); err != nil {
		t.Fatal(err)
	}
	if _, err := database.QueueTask(context.Background(), id, 1, 1, testTaskLifecycleCommand("queue_task", "execution"), now); err != nil {
		t.Fatal(err)
	}
	_, run, found, err := database.ClaimTaskExecution(context.Background(), now)
	if err != nil || !found {
		t.Fatalf("claim = %#v, %t, %v", run, found, err)
	}
	if err := database.StartTaskExecution(context.Background(), run.ID, run.Generation, now); err != nil {
		t.Fatal(err)
	}
	if err := database.RecoverTaskExecutions(context.Background(), now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	_, recovered, found, err := database.ClaimTaskExecution(context.Background(), now.Add(2*time.Second))
	if err != nil || !found || recovered.ID != run.ID {
		t.Fatalf("recovered = %#v, %t, %v", recovered, found, err)
	}
	if err := database.StartTaskExecution(context.Background(), run.ID, run.Generation, now.Add(3*time.Second)); err != nil {
		t.Fatal(err)
	}
	if err := database.AppendTaskRunItems(context.Background(), run.ID, run.Generation, []TaskRunItemInput{
		{Kind: "context_checkpoint", Status: "completed", Round: 1, Payload: map[string]any{"summary": "hidden"}},
		{Kind: "assistant_output", Status: "completed", Round: 1, Content: "visible"},
	}, TaskRunUsage{}, now); err != nil {
		t.Fatal(err)
	}
	page, err := database.TaskRunItems(context.Background(), run.ID, 10, nil)
	if err != nil || len(page.Items) != 1 || page.Items[0].Kind != "assistant_output" {
		t.Fatalf("visible Task transcript = %#v, %v", page.Items, err)
	}
	replay, err := database.TaskRunReplayItems(context.Background(), run.ID)
	if err != nil || len(replay) != 2 || replay[0].Kind != "context_checkpoint" {
		t.Fatalf("Task replay = %#v, %v", replay, err)
	}
	if err := database.FinishTaskPlanning(context.Background(), run.ID, run.Generation, "simple", now.Add(4*time.Second)); err != nil {
		t.Fatal(err)
	}
	runs, err := database.TaskRuns(context.Background(), id, 10)
	if err != nil || len(runs) != 2 || runs[0].Kind != "executor" || runs[0].AttemptIndex != 0 || runs[0].ReviewRound != 1 || runs[0].ParentRunID != run.ID {
		t.Fatalf("child runs = %#v, %v", runs, err)
	}
	if err := database.FinishTaskPlanning(context.Background(), run.ID, run.Generation, "simple", now.Add(5*time.Second)); err != ErrStaleRun {
		t.Fatalf("stale transition = %v", err)
	}
}

func TestTaskInterventionUncertainResultOpensRecovery(t *testing.T) {
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(context.Background(), account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 12, 30, 0, 0, time.UTC)
	id, _ := NewTaskID()
	if _, err := database.CreateTask(ctx, id, "Uncertain adapter", "correlation:adapter:create", now); err != nil {
		t.Fatal(err)
	}
	if _, err := database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "adapter"), now); err != nil {
		t.Fatal(err)
	}
	_, planner, found, err := database.ClaimTaskExecution(ctx, now)
	if err != nil || !found {
		t.Fatalf("claim planner = %#v, %t, %v", planner, found, err)
	}
	if err = database.StartTaskExecution(ctx, planner.ID, planner.Generation, now); err != nil {
		t.Fatal(err)
	}
	if err = database.FinishTaskPlanning(ctx, planner.ID, planner.Generation, "simple", now); err != nil {
		t.Fatal(err)
	}
	_, run, found, err := database.ClaimTaskExecution(ctx, now)
	if err != nil || !found || run.Kind != "executor" {
		t.Fatalf("claim executor = %#v, %t, %v", run, found, err)
	}
	if err = database.StartTaskExecution(ctx, run.ID, run.Generation, now); err != nil {
		t.Fatal(err)
	}
	if err = database.AppendTaskRunItems(ctx, run.ID, run.Generation, []TaskRunItemInput{{
		Kind: "tool_call", Status: "running", Round: 1, Payload: map[string]any{"name": "example.write"},
	}}, TaskRunUsage{ToolCalls: 1}, now); err != nil {
		t.Fatal(err)
	}
	items, err := database.TaskRunReplayItems(ctx, run.ID)
	if err != nil || len(items) == 0 {
		t.Fatalf("run items = %#v, %v", items, err)
	}
	call := items[len(items)-1]
	if err = database.SuspendTaskExecution(ctx, run.ID, run.Generation, now); err != nil {
		t.Fatal(err)
	}
	if err = database.CompleteTaskIntervention(ctx, run.ID, run.Generation, TaskRunItemInput{
		Kind: "tool_result", Status: "failed", Round: 1, ParentID: call.ID,
		Payload: map[string]any{"name": "example.write", "result": map[string]any{"error": "outcome_uncertain"}, "success": false},
	}, true, now); err != nil {
		t.Fatal(err)
	}
	task, err := database.Task(ctx, id)
	if err != nil || task.StageKey != "waiting" || task.ActiveGateID == "" {
		t.Fatalf("Task recovery = %#v, %v", task, err)
	}
	gate, err := database.TaskGate(ctx, task.ActiveGateID)
	if err != nil || gate.RecoveryReason == nil || *gate.RecoveryReason != "unsafe_effect_uncertain" {
		t.Fatalf("recovery gate = %#v, %v", gate, err)
	}
}

func TestTaskRunEffectiveCwdPrecedence(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 14, 0, 0, 0, time.UTC)
	projectID, _ := NewProjectID()
	projectFolder := t.TempDir()
	if _, err := database.CreateProject(ctx, projectID, "workspace:personal", "Task workspace", "", &projectFolder,
		testProjectDigest("# Project\n"), false, testProjectCommand("project.create", "task-cwd", "task-cwd"), now); err != nil {
		t.Fatal(err)
	}
	tx, err := database.db.BeginTx(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback()
	override := t.TempDir()
	overrideEffective := filepath.Join(override, "task")
	projectEffective := filepath.Join(projectFolder, "task")
	for _, test := range []struct {
		name string
		task Task
		want *string
	}{
		{name: "override", task: Task{ProjectID: projectID, CwdOverride: &override}, want: &overrideEffective},
		{name: "project", task: Task{ProjectID: projectID}, want: &projectEffective},
		{name: "default", task: Task{}, want: nil},
	} {
		t.Run(test.name, func(t *testing.T) {
			got, err := taskRunEffectiveCwdTx(ctx, tx, test.task)
			if err != nil || !equalOptionalString(got, test.want) {
				t.Fatalf("effective CWD = %v, want %v: %v", got, test.want, err)
			}
		})
	}
}

func equalOptionalString(left, right *string) bool {
	return left == nil && right == nil || left != nil && right != nil && *left == *right
}

func TestCancelTaskRejectsLateRunChangesAcrossStates(t *testing.T) {
	for _, state := range []string{"queued", "running", "waiting"} {
		t.Run(state, func(t *testing.T) {
			database := openTestStore(t)
			account := createReadyModelAccount(t, database)
			ctx := t.Context()
			if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
				t.Fatal(err)
			}
			now := time.Date(2026, 9, 6, 0, 0, 0, 0, time.UTC)
			id, _ := NewTaskID()
			if _, err := database.CreateTask(ctx, id, "Cancel audit", "correlation:cancel", now); err != nil {
				t.Fatal(err)
			}
			queued, err := database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "queue"), now)
			if err != nil {
				t.Fatal(err)
			}
			runs, err := database.TaskRuns(ctx, id, 10)
			if err != nil || len(runs) != 1 {
				t.Fatalf("initial runs = %#v, %v", runs, err)
			}
			run := runs[0]
			if state != "queued" {
				_, claimed, found, err := database.ClaimTaskExecution(ctx, now)
				if err != nil || !found || claimed.ID != run.ID {
					t.Fatalf("claim = %#v, %t, %v", claimed, found, err)
				}
				if err := database.StartTaskExecution(ctx, run.ID, run.Generation, now); err != nil {
					t.Fatal(err)
				}
			}
			if state == "waiting" {
				if err := database.BlockTaskExecution(ctx, run.ID, run.Generation, "clarification", "Which value?", "Choose the required value.", []string{"alpha", "beta"}, now); err != nil {
					t.Fatal(err)
				}
			}
			before, err := database.Task(ctx, id)
			if err != nil {
				t.Fatal(err)
			}
			cancelled, err := database.CancelTask(ctx, id, before.Revision, before.Generation, "Stop audit", testTaskLifecycleCommand("cancel_task", "cancel"), now.Add(time.Second))
			if err != nil {
				t.Fatal(err)
			}
			if cancelled.Task.StageKey != "cancelled" || cancelled.Task.Generation != queued.Task.Generation+1 || cancelled.Task.CurrentRunID != "" || cancelled.Task.ActiveGateID != "" {
				t.Fatalf("cancelled = %#v", cancelled.Task)
			}
			if before.ActiveGateID != "" {
				gate, err := database.TaskGate(ctx, before.ActiveGateID)
				if err != nil || gate.State != "superseded" {
					t.Fatalf("cancelled gate = %#v, %v", gate, err)
				}
			}
			if err := database.FinishTaskPlanning(ctx, run.ID, run.Generation, "simple", now.Add(2*time.Second)); err != ErrStaleRun {
				t.Fatalf("late planning = %v", err)
			}
			if err := database.AppendTaskRunItems(ctx, run.ID, run.Generation, []TaskRunItemInput{{Kind: "assistant_message", Content: "Late result"}}, TaskRunUsage{}, now.Add(2*time.Second)); err != ErrStaleRun {
				t.Fatalf("late transcript = %v", err)
			}
			_, _, found, err := database.ClaimTaskExecution(ctx, now.Add(3*time.Second))
			if err != nil || found {
				t.Fatalf("cancelled work was claimed: %t, %v", found, err)
			}
			after, err := database.Task(ctx, id)
			if err != nil || after.Revision != cancelled.Task.Revision || after.Generation != cancelled.Task.Generation || after.StageKey != "cancelled" {
				t.Fatalf("late result changed cancellation: %#v, %v", after, err)
			}
			runs, err = database.TaskRuns(ctx, id, 10)
			if err != nil || len(runs) != 1 || runs[0].Status != "cancelled" {
				t.Fatalf("final runs = %#v, %v", runs, err)
			}
		})
	}
}

func TestTaskRolesRecoverSavedProgressAfterStoreReopen(t *testing.T) {
	ctx := t.Context()
	path := filepath.Join(t.TempDir(), "restart.sqlite3")
	database, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if database != nil {
			_ = database.Close()
		}
	})
	account := createReadyModelAccount(t, database)
	if _, err = database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 9, 6, 2, 0, 0, 0, time.UTC)
	id, _ := NewTaskID()
	if _, err = database.CreateTask(ctx, id, "Restart café 日本語", "correlation:restart", now); err != nil {
		t.Fatal(err)
	}
	if _, err = database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "restart"), now); err != nil {
		t.Fatal(err)
	}
	for index, role := range []string{"planner", "executor", "reviewer"} {
		_, run, found, err := database.ClaimTaskExecution(ctx, now)
		if err != nil || !found || run.Kind != role {
			t.Fatalf("%s claim = %#v, %t, %v", role, run, found, err)
		}
		if err = database.StartTaskExecution(ctx, run.ID, run.Generation, now); err != nil {
			t.Fatal(err)
		}
		if err = database.AppendTaskRunItems(ctx, run.ID, run.Generation, []TaskRunItemInput{
			{Kind: "context_checkpoint", Status: "completed", Round: 1, Payload: map[string]any{"summary": role + " saved café 日本語"}},
			{Kind: "assistant_output", Status: "completed", Round: 1, Content: role + " visible café 日本語"},
		}, TaskRunUsage{ProviderCalls: 1, InputTokens: 21, OutputTokens: 8}, now); err != nil {
			t.Fatal(err)
		}
		saved, err := database.TaskRunReplayItems(ctx, run.ID)
		if err != nil || len(saved) != 2 {
			t.Fatalf("%s saved replay = %#v, %v", role, saved, err)
		}
		if err = database.Close(); err != nil {
			t.Fatal(err)
		}
		database, err = Open(ctx, path)
		if err != nil {
			t.Fatal(err)
		}
		// A second recovery call must not create another run.
		for range 2 {
			if err = database.RecoverTaskExecutions(ctx, now.Add(time.Second)); err != nil {
				t.Fatal(err)
			}
		}
		current, recovered, found, err := database.ClaimTaskExecution(ctx, now.Add(2*time.Second))
		if err != nil || !found || recovered.ID != run.ID || recovered.Kind != role || recovered.Generation != run.Generation || current.CurrentRunID != run.ID {
			t.Fatalf("%s recovered = %#v, %#v, %t, %v", role, current, recovered, found, err)
		}
		if recovered.ProviderCallCount != 1 || recovered.InputTokens != 21 || recovered.OutputTokens != 8 || recovered.ParentRunID != run.ParentRunID {
			t.Fatalf("%s lost saved run fields: %#v", role, recovered)
		}
		replay, err := database.TaskRunReplayItems(ctx, run.ID)
		if err != nil || !reflect.DeepEqual(replay, saved) {
			t.Fatalf("%s changed replay = %#v, %v", role, replay, err)
		}
		runs, err := database.TaskRuns(ctx, id, 10)
		if err != nil || len(runs) != index+1 {
			t.Fatalf("%s duplicate runs = %#v, %v", role, runs, err)
		}
		if _, _, found, err = database.ClaimTaskExecution(ctx, now); err != nil || found {
			t.Fatalf("%s duplicate claim = %t, %v", role, found, err)
		}
		if err = database.StartTaskExecution(ctx, recovered.ID, recovered.Generation, now); err != nil {
			t.Fatal(err)
		}
		switch role {
		case "planner":
			err = database.FinishTaskPlanning(ctx, run.ID, run.Generation, "simple", now)
		case "executor":
			err = database.FinishTaskExecution(ctx, run.ID, run.Generation, false, now)
		case "reviewer":
			err = database.FinishTaskReview(ctx, run.ID, run.Generation, "approve", "Accepted café 日本語", false, now)
		}
		if err != nil {
			t.Fatalf("%s could not finish: %v", role, err)
		}
	}
	current, err := database.Task(ctx, id)
	if err != nil || current.StageKey != "done" || current.CompletedAt == nil || current.CurrentRunID != "" {
		t.Fatalf("Task remained active = %#v, %v", current, err)
	}
	runs, err := database.TaskRuns(ctx, id, 10)
	if err != nil || len(runs) != 3 {
		t.Fatalf("final runs = %#v, %v", runs, err)
	}
	for _, run := range runs {
		if run.Status != "completed" {
			t.Fatalf("run remained active = %#v", run)
		}
	}
}

func TestReviewRecoveryResumesRecordedExecutorOnce(t *testing.T) {
	ctx := t.Context()
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	policy, err := database.TaskExecutionPolicy(ctx)
	if err != nil {
		t.Fatal(err)
	}
	policy.MaxReviewRounds = 1
	if _, err = database.UpdateTaskExecutionPolicy(ctx, policy); err != nil {
		t.Fatal(err)
	}
	now := time.Now()
	id, _ := NewTaskID()
	if _, err = database.CreateTask(ctx, id, "Review recovery", "correlation:review-retry", now); err != nil {
		t.Fatal(err)
	}
	if _, err = database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "review-retry"), now); err != nil {
		t.Fatal(err)
	}
	var reviewer TaskRun
	for _, role := range []string{"planner", "executor", "reviewer"} {
		_, run, found, err := database.ClaimTaskExecution(ctx, now)
		if err != nil || !found || run.Kind != role {
			t.Fatalf("%s claim = %#v, %t, %v", role, run, found, err)
		}
		if err = database.StartTaskExecution(ctx, run.ID, run.Generation, now); err != nil {
			t.Fatal(err)
		}
		switch role {
		case "planner":
			err = database.FinishTaskPlanning(ctx, run.ID, run.Generation, "simple", now)
		case "executor":
			err = database.FinishTaskExecution(ctx, run.ID, run.Generation, false, now)
		case "reviewer":
			reviewer = run
			err = database.FinishTaskReview(ctx, run.ID, run.Generation, "request_changes", "Include café 日本語", false, now)
		}
		if err != nil {
			t.Fatal(err)
		}
	}
	current, err := database.Task(ctx, id)
	if err != nil || current.StageKey != "waiting" {
		t.Fatalf("review recovery = %#v, %v", current, err)
	}
	gate, err := database.TaskGate(ctx, current.ActiveGateID)
	if err != nil || gate.RetryRunKind == nil || *gate.RetryRunKind != "executor" {
		t.Fatalf("retry role = %#v, %v", gate, err)
	}
	command := testTaskLifecycleCommand("retry_task", "review-recovery")
	for range 2 {
		if _, err = database.ResolveTaskGate(ctx, id, gate.ID, current.Revision, current.Generation, "Retry café 日本語", "retry", nil, command, now); err != nil {
			t.Fatal(err)
		}
	}
	runs, err := database.TaskRuns(ctx, id, 10)
	if err != nil || len(runs) != 4 {
		t.Fatalf("recovery run count = %d, %v", len(runs), err)
	}
	_, resumed, found, err := database.ClaimTaskExecution(ctx, now)
	if err != nil || !found || resumed.Kind != "executor" || resumed.ParentRunID != reviewer.ID {
		t.Fatalf("recovery role=%s, expected=executor, parentMatches=%t, found=%t, error=%v", resumed.Kind, resumed.ParentRunID == reviewer.ID, found, err)
	}
	messages, err := database.TaskMessages(ctx, id, 10)
	if err != nil || len(messages) != 1 || messages[0].Body != "Retry café 日本語" {
		t.Fatalf("retry answer = %#v, %v", messages, err)
	}
}
