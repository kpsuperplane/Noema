package store

import (
	"context"
	"database/sql"
	"path/filepath"
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

func TestACPTaskRunCapturesLaunchAndRecovers(t *testing.T) {
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(context.Background(), account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 9, 5, 13, 0, 0, 0, time.UTC)
	agent, err := database.CreateAcpAgent(context.Background(), "ACP", "first-command", []string{"--stdio"}, now)
	if err != nil {
		t.Fatal(err)
	}
	id, _ := NewTaskID()
	task, err := database.CreateTask(context.Background(), id, "ACP Execution", "correlation:acp:create", now)
	if err != nil {
		t.Fatal(err)
	}
	override := t.TempDir()
	updated, err := database.UpdateInboxTask(context.Background(), id, task.Revision, task.Generation,
		TaskUpdate{ExecutorAgentID: &agent.AgentID, CwdOverride: &override, SetCwd: true}, testTaskLifecycleCommand("update_task", "acp"), now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = database.QueueTask(context.Background(), id, updated.Task.Revision, 1, testTaskLifecycleCommand("queue_task", "acp"), now); err != nil {
		t.Fatal(err)
	}
	_, planner, found, err := database.ClaimTaskExecution(context.Background(), now)
	if err != nil || !found || planner.ExecutorBackend != "provider" {
		t.Fatalf("planner = %#v, %t, %v", planner, found, err)
	}
	if err = database.StartTaskExecution(context.Background(), planner.ID, 1, now); err != nil {
		t.Fatal(err)
	}
	if _, err = database.UpdateAcpAgent(context.Background(), agent.AgentID, 1, "ACP changed", "second-command", nil, true, now); err != nil {
		t.Fatal(err)
	}
	if err = database.FinishTaskPlanning(context.Background(), planner.ID, 1, "simple", now); err != nil {
		t.Fatal(err)
	}
	if _, err = database.UpdateAcpAgent(context.Background(), agent.AgentID, 2, "ACP changed again", "third-command", nil, true, now); err != nil {
		t.Fatal(err)
	}
	_, executor, found, err := database.ClaimTaskExecution(context.Background(), now)
	if err != nil || !found || executor.ExecutorBackend != "acp" || executor.AcpLaunch == nil ||
		executor.AcpLaunch.Command != "second-command" || executor.AcpLaunch.ConnectionRevision != 2 || len(executor.AcpLaunch.Arguments) != 0 ||
		executor.EffectiveCwd == nil || *executor.EffectiveCwd != override {
		t.Fatalf("ACP launch = %#v, %t, %v", executor, found, err)
	}
	if err = database.StartTaskExecution(context.Background(), executor.ID, 1, now); err != nil {
		t.Fatal(err)
	}
	if err = database.RecordAcpSession(context.Background(), executor.ID, 1, "session:before-restart", now); err != nil {
		t.Fatal(err)
	}
	if err = database.RecoverTaskExecutions(context.Background(), now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	_, recovered, found, err := database.ClaimTaskExecution(context.Background(), now.Add(2*time.Second))
	if err != nil || !found || recovered.ID != executor.ID || recovered.AcpLaunch == nil ||
		recovered.AcpLaunch.Command != "second-command" || recovered.AcpSessionID == nil || *recovered.AcpSessionID != "session:before-restart" {
		t.Fatalf("recovered ACP run = %#v, %t, %v", recovered, found, err)
	}
	if err = database.StartTaskExecution(context.Background(), recovered.ID, 1, now.Add(3*time.Second)); err != nil {
		t.Fatal(err)
	}
	if err = database.RecordAcpPermissionUse(context.Background(), recovered.ID, 1, "exact-effect", now.Add(3*time.Second)); err != nil {
		t.Fatal(err)
	}
	if err = database.RecoverTaskExecutions(context.Background(), now.Add(4*time.Second)); err != nil {
		t.Fatal(err)
	}
	uncertainTask, err := database.Task(context.Background(), id)
	if err != nil || uncertainTask.StageKey != "waiting" || uncertainTask.ActiveGateID == "" {
		t.Fatalf("uncertain Task = %#v, %v", uncertainTask, err)
	}
	gate, err := database.TaskGate(context.Background(), uncertainTask.ActiveGateID)
	if err != nil || gate.RecoveryReason == nil || *gate.RecoveryReason != "unsafe_effect_uncertain" {
		t.Fatalf("uncertain gate = %#v, %v", gate, err)
	}
	if _, _, found, err = database.ClaimTaskExecution(context.Background(), now.Add(5*time.Second)); err != nil || found {
		t.Fatalf("uncertain ACP run was reclaimed: %t, %v", found, err)
	}
}

func TestTaskRunEffectiveCwdPrecedence(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 14, 0, 0, 0, time.UTC)
	projectID, _ := NewProjectID()
	projectFolder := t.TempDir()
	if _, err := database.CreateProject(ctx, projectID, "workspace:personal", "ACP workspace", "", &projectFolder,
		testProjectDigest("# Project\n"), false, testProjectCommand("project.create", "acp-cwd", "acp-cwd"), now); err != nil {
		t.Fatal(err)
	}
	tx, err := database.db.BeginTx(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback()
	override := t.TempDir()
	for _, test := range []struct {
		name string
		task Task
		want *string
	}{
		{name: "override", task: Task{ProjectID: projectID, CwdOverride: &override}, want: &override},
		{name: "project", task: Task{ProjectID: projectID}, want: &projectFolder},
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
