package runtime

import (
	"context"
	"fmt"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/schedule"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestRecurringTaskMonitorSilencesUnchangedAndReportsChangedEvidence(t *testing.T) {
	chat, database, _ := chatFixture(t)
	if err := chat.Close(); err != nil {
		t.Fatal(err)
	}

	ctx := context.Background()
	now := time.Date(2026, 9, 6, 12, 0, 0, 0, time.UTC)
	first := now.Add(-time.Minute)
	sourceTaskID, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := home.CreatePendingTaskDocument(chat.home, sourceTaskID, "# Monitor template\n"); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(chat.home, sourceTaskID); err != nil {
		t.Fatal(err)
	}
	scheduled, err := schedule.Normalize(schedule.Schedule{
		ScheduledFor:    first,
		TimeZone:        "UTC",
		MissedRunPolicy: schedule.MissedRunOnce,
		Recurrence: &schedule.Recurrence{
			StartsAt:       first,
			CronExpression: "* * * * *",
			OverlapPolicy:  schedule.OverlapAllow,
		},
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	created, err := database.CreateTaskWithOptions(ctx, sourceTaskID, "Source monitor", runtimeTaskCommand("capture-monitor", sourceTaskID),
		store.TaskCreateOptions{Schedule: &scheduled}, now)
	if err != nil || created.RecurrenceID == "" {
		t.Fatalf("create monitor recurrence = %#v, %v", created, err)
	}
	recurrenceID := created.RecurrenceID
	if _, err := home.EnsureRecurrenceDocument(chat.home, recurrenceID, "SOURCE_REVISION=1\n"); err != nil {
		t.Fatal(err)
	}

	var stateMu sync.Mutex
	phase := 0
	roleCalls := map[string]int{}
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		role := taskRequestRole(request.Tools)
		stateMu.Lock()
		roleCalls[role]++
		call := roleCalls[role]
		currentPhase := phase
		stateMu.Unlock()
		switch role {
		case "planner":
			if call == 1 {
				return taskToolResult("monitor-plan", taskFilesWrite, map[string]any{
					"path": "TASK.md", "content": fmt.Sprintf("# Monitor\n\nSOURCE_REVISION=%d\n", currentPhase+1),
				}), nil
			}
			return taskToolResult("monitor-plan-finish", taskFinishPlanning, map[string]any{"complexity": "simple"}), nil
		case "executor":
			if call == 1 {
				return taskToolResult("monitor-progress", taskFilesWrite, map[string]any{
					"path": "TASK.md", "content": fmt.Sprintf("# Monitor\n\nSOURCE_REVISION=%d\n", currentPhase+1),
				}), nil
			}
			if call == 2 {
				return taskToolResult("monitor-result", taskFilesWrite, map[string]any{
					"path": "RESULT.md", "content": fmt.Sprintf("Evidence: source revision %d changed.\n", currentPhase+1),
				}), nil
			}
			return taskToolResult("monitor-execution-finish", taskFinishExecution, map[string]any{}), nil
		case "reviewer":
			stateMu.Lock()
			notify := phase == 1
			stateMu.Unlock()
			return taskToolResult("monitor-review", taskFinishReview, map[string]any{
				"decision": "approve", "feedback": "The source evidence is complete.", "notify_human": notify,
			}), nil
		default:
			return provider.GenerationResult{}, fmt.Errorf("unexpected Task role %q", role)
		}
	})
	taskRuntime, err := NewTaskExecution(ctx, database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(taskRuntime.Close)

	materialize := func(at time.Time) store.Task {
		t.Helper()
		createdTasks, _, processErr := database.ProcessDueTaskSchedules(ctx, at, true, func(value store.DueTask) error {
			return home.StageRecurrenceDocumentToTask(chat.home, value.RecurrenceID, value.TaskID)
		})
		if processErr != nil || len(createdTasks) != 1 {
			t.Fatalf("materialize monitor at %s = %#v, %v", at, createdTasks, processErr)
		}
		if err := home.RecoverTaskDocuments(chat.home, func(id string) (bool, error) {
			return database.TaskExists(ctx, id)
		}); err != nil {
			t.Fatal(err)
		}
		task, err := database.Task(ctx, createdTasks[0].TaskID)
		if err != nil {
			t.Fatal(err)
		}
		queued, err := database.QueueTask(ctx, task.ID, task.Revision, task.Generation,
			runtimeTaskCommand("queue-monitor", task.ID), at)
		if err != nil {
			t.Fatal(err)
		}
		return queued.Task
	}

	firstTask := materialize(now)
	waitRuntimeTask(t, database, firstTask.ID, func(value store.Task) bool { return value.StageKey == "done" })
	firstResult, err := home.ReadTaskFile(chat.home, firstTask.ID, "RESULT.md")
	if err != nil || !strings.Contains(firstResult, "source revision 1") {
		t.Fatalf("unchanged monitor result = %q, %v", firstResult, err)
	}
	firstEvent := completedTaskWorkEvent(t, database, firstTask.ID)
	if boolField(firstEvent.Payload, "notify_human") {
		t.Fatal("unchanged monitor requested a human update")
	}

	current, err := home.ReadRecurrenceDocument(chat.home, recurrenceID)
	if err != nil {
		t.Fatal(err)
	}
	stage, err := home.PrepareRecurrenceDocumentReplace(chat.home, recurrenceID, current.Digest,
		"SOURCE_REVISION=2\n", strings.Repeat("b", 64))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := home.CommitRecurrenceDocumentStage(chat.home, stage); err != nil {
		t.Fatal(err)
	}
	recurrence, err := database.TaskRecurrence(ctx, recurrenceID)
	if err != nil || recurrence.NextRunAt == nil {
		t.Fatalf("next monitor occurrence = %#v, %v", recurrence, err)
	}
	stateMu.Lock()
	phase = 1
	roleCalls = map[string]int{}
	stateMu.Unlock()
	secondTask := materialize(recurrence.NextRunAt.Add(time.Second))
	waitRuntimeTask(t, database, secondTask.ID, func(value store.Task) bool { return value.StageKey == "done" })
	secondResult, err := home.ReadTaskFile(chat.home, secondTask.ID, "RESULT.md")
	if err != nil || !strings.Contains(secondResult, "source revision 2") {
		t.Fatalf("changed monitor result = %q, %v", secondResult, err)
	}
	secondEvent := completedTaskWorkEvent(t, database, secondTask.ID)
	if !boolField(secondEvent.Payload, "notify_human") {
		t.Fatal("changed monitor did not request one human update")
	}

	// The existing primary-notification projection tests cover the user-facing card.
	// This case verifies the monitor-specific changed-versus-unchanged decision and evidence.

}

func completedTaskWorkEvent(t *testing.T, database *store.Store, taskID string) store.WorkEvent {
	t.Helper()
	events, err := database.WorkEventsForTask(context.Background(), taskID, 0, 100)
	if err != nil {
		t.Fatal(err)
	}
	for _, event := range events {
		if event.Kind == "task.completed" {
			return event
		}
	}
	t.Fatalf("task %s has no completion event", taskID)
	return store.WorkEvent{}
}
