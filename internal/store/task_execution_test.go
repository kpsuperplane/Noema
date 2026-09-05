package store

import (
	"context"
	"testing"
	"time"
)

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
