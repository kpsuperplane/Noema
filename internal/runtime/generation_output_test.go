package runtime

import (
	"errors"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestGenerationOutputSnapshotsPreserveSectionsAndInterruptedText(t *testing.T) {
	var snapshots [][]provider.GenerationOutput
	stream := newGenerationOutputStream(func(output []provider.GenerationOutput) error { snapshots = append(snapshots, output); return nil })
	stream.event(provider.StreamEvent{Kind: provider.TextDelta, Index: 1, Delta: "I'll ", Phase: "commentary"})
	stream.lastFlush = time.Now().Add(time.Hour) // Hold ordinary writes; completion must still flush.
	stream.event(provider.StreamEvent{Kind: provider.TextDelta, Index: 1, Delta: "check."})
	if len(snapshots) != 1 {
		t.Fatal("delta bypassed write cadence")
	}
	stream.event(provider.StreamEvent{Kind: provider.MessageCompleted, Index: 1, Text: "I'll check.", Phase: "commentary"})
	stream.event(provider.StreamEvent{Kind: provider.ReasoningDelta, Index: 2, SectionIndex: 0, Delta: "First"})
	stream.event(provider.StreamEvent{Kind: provider.ReasoningCompleted, Index: 2, SectionIndex: 0, Text: "First"})
	stream.event(provider.StreamEvent{Kind: provider.ReasoningDelta, Index: 2, SectionIndex: 1, Delta: "Partial"})
	result := provider.GenerationResult{}
	if err := stream.finish(&result, errors.New("interrupted")); err != nil {
		t.Fatal(err)
	}
	if len(result.Output) != 3 || result.Output[0].Text != "I'll check." || result.Output[0].Phase != "commentary" || result.Output[1].Text != "First" || result.Output[2].Text != "Partial" || result.Output[2].Status != "failed" {
		t.Fatalf("output = %#v", result.Output)
	}
	if result.Output[0].Status != "completed" || result.Output[1].Status != "completed" {
		t.Fatal("completed sections lost")
	}
	if snapshots[0][0].Text != "I'll " {
		t.Fatal("previous snapshot was mutated")
	}
}

func TestTaskOutputStreamUpdatesOneDurableItemAndPreservesReplay(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Show progress")
	_, run, found, err := database.ClaimTaskExecution(t.Context(), time.Now())
	if err != nil || !found {
		t.Fatalf("claim: %v", err)
	}
	if err = database.StartTaskExecution(t.Context(), run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	runtime := &TaskExecution{database: database}
	stream, id, err := runtime.taskOutputStream(t.Context(), run, 0)
	if err != nil {
		t.Fatal(err)
	}
	stream.event(provider.StreamEvent{Kind: provider.TextDelta, Delta: "Checking", Phase: "commentary"})
	first, err := database.TaskRunItems(t.Context(), run.ID, 10, nil)
	if err != nil || len(first.Items) != 1 {
		t.Fatalf("live snapshot = %#v, %v", first, err)
	}
	result := provider.GenerationResult{Text: "Checking sources.", Output: []provider.GenerationOutput{{Kind: "message", Text: "Checking sources.", Phase: "commentary"}}}
	if err = stream.finish(&result, nil); err != nil {
		t.Fatal(err)
	}
	items := taskAssistantItems(result, 0)
	items[0].ID = id
	if err = database.AppendTaskRunItems(t.Context(), run.ID, run.Generation, items, store.TaskRunUsage{ProviderCalls: 1}, time.Now()); err != nil {
		t.Fatal(err)
	}
	page, err := database.TaskRunItems(t.Context(), run.ID, 10, nil)
	if err != nil || len(page.Items) != 1 || page.Items[0].ID != first.Items[0].ID || !page.Items[0].UpdatedAt.After(first.Items[0].UpdatedAt) {
		t.Fatalf("snapshot update = %#v, %v", page, err)
	}
	messages := taskOutputMessages(result)
	if len(messages) != 1 || messages[0].Phase != "commentary" || messages[0].Content != result.Text {
		t.Fatalf("replay = %#v", messages)
	}
	if err = database.FinishTaskPlanning(t.Context(), run.ID, run.Generation, "simple", time.Now()); err != nil {
		t.Fatal(err)
	}
	if err = database.AppendTaskRunItems(t.Context(), run.ID, run.Generation, items, store.TaskRunUsage{}, time.Now()); err != store.ErrStaleRun {
		t.Fatalf("late output accepted: %v, task %s", err, task.ID)
	}
}
