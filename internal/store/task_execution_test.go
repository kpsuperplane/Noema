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
