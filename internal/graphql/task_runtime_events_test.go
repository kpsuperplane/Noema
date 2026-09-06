package graphql

import (
	"context"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestTaskRuntimeEventsWakeForDurableTranscript(t *testing.T) {
	r := readyAgentTestResolver(t)
	if _, err := r.SubscriptionRoot().TaskRuntimeEvents(context.Background(), "task:missing"); err == nil {
		t.Fatal("missing Task subscription succeeded")
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	captured, err := r.captureTask(ctx, model.CaptureTaskInput{
		WorkspaceID: personalWorkspaceID, Title: "Runtime events", TaskDocument: "Run",
		ExecutorAgentID: stringAddress("agent:task-executor"), ClientMutationID: "runtime-capture",
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, err = r.queueTask(ctx, model.QueueTaskInput{TaskID: captured.Task.TaskID,
		ExpectedRevision: 1, ExpectedGeneration: 1, ClientMutationID: "runtime-queue"}); err != nil {
		t.Fatal(err)
	}
	_, run, found, err := r.Store.ClaimTaskExecution(ctx, time.Now())
	if err != nil || !found {
		t.Fatalf("claim = %#v, %t, %v", run, found, err)
	}
	if err = r.Store.StartTaskExecution(ctx, run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	events, err := r.SubscriptionRoot().TaskRuntimeEvents(ctx, captured.Task.TaskID)
	if err != nil {
		t.Fatal(err)
	}
	otherID, _ := store.NewTaskID()
	if _, err = r.Store.CreateTask(ctx, otherID, "Other", "correlation:runtime:other", time.Now()); err != nil {
		t.Fatal(err)
	}
	select {
	case event := <-events:
		t.Fatalf("unrelated Task woke subscription: %#v", event)
	case <-time.After(25 * time.Millisecond):
	}
	items := []store.TaskRunItemInput{
		{Kind: "assistant_output", Status: "completed", Round: 1, Content: "First"},
		{Kind: "assistant_output", Status: "completed", Round: 1, Content: "Second"},
	}
	if err = r.Store.AppendTaskRunItems(ctx, run.ID, run.Generation, items, store.TaskRunUsage{}, time.Now()); err != nil {
		t.Fatal(err)
	}
	select {
	case event := <-events:
		if event.TaskID != captured.Task.TaskID || event.RunID == nil || *event.RunID != run.ID {
			t.Fatalf("runtime event = %#v", event)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("runtime event timed out")
	}
	page, err := r.Store.TaskRunItems(ctx, run.ID, 10, nil)
	if err != nil || len(page.Items) != 2 {
		t.Fatalf("durable replay = %#v, %v", page, err)
	}
	cancel()
	select {
	case _, open := <-events:
		if open {
			t.Fatal("runtime stream stayed open")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("runtime stream did not close")
	}
}
