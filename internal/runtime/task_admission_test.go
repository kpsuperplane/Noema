package runtime

import (
	"testing"
	"time"
)

func TestTaskAdmissionExcludesSettlingTaskButClaimsOtherTasks(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "First task")
	worker := &TaskExecution{database: database}
	_, predecessor, found, err := worker.claimTaskExecution(t.Context())
	if err != nil || !found {
		t.Fatalf("claim predecessor: found=%v err=%v", found, err)
	}
	current, err := database.Task(t.Context(), task.ID)
	if err != nil {
		t.Fatal(err)
	}
	cancelled, err := database.CancelTask(t.Context(), task.ID, current.Revision, current.Generation,
		"Replace execution", runtimeTaskCommand("cancel-admission", "admission"), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	complexity := "simple"
	reopened, err := database.ReopenTask(t.Context(), task.ID, cancelled.Task.Revision, cancelled.Task.Generation,
		"Continue execution", &complexity, "", runtimeTaskCommand("reopen-admission", "admission"), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	// The execution remains active until the provider cleanup returns.
	cleanup := make(chan struct{})
	settled := make(chan struct{})
	go func() { <-cleanup; worker.markInactiveTask(task.ID); close(settled) }()
	defer func() {
		select {
		case <-settled:
		default:
			close(cleanup)
			<-settled
		}
	}()
	if _, _, found, err = worker.claimTaskExecution(t.Context()); err != nil || found {
		t.Fatalf("claimed successor during cleanup: found=%v err=%v", found, err)
	}
	other := createQueuedRuntimeTask(t, database, chat.home, "Independent task")
	claimed, _, found, err := worker.claimTaskExecution(t.Context())
	if err != nil || !found || claimed.ID != other.ID {
		t.Fatalf("independent task blocked: task=%s found=%v err=%v", claimed.ID, found, err)
	}
	close(cleanup)
	<-settled
	claimed, successor, found, err := worker.claimTaskExecution(t.Context())
	if err != nil || !found || claimed.ID != task.ID || successor.ID != reopened.Task.CurrentRunID || successor.ID == predecessor.ID {
		t.Fatalf("successor not admitted after cleanup: task=%s run=%s found=%v err=%v", claimed.ID, successor.ID, found, err)
	}
}
