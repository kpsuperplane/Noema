package store

import (
	"context"
	"errors"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/schedule"
)

func TestScheduledTaskCommandsAndRecurrenceLifecycle(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	taskID, _ := NewTaskID()
	created, err := database.CreateTaskWithOptions(ctx, taskID, "Recurring Task", testTaskCommand("create"),
		TaskCreateOptions{}, now)
	if err != nil {
		t.Fatal(err)
	}
	first := now.Add(time.Hour)
	value := normalizedTestSchedule(t, first, schedule.MissedRunOnce, schedule.OverlapSkip)
	scheduled, err := database.SetTaskSchedule(ctx, taskID, created.Task.Revision, value, false,
		testTaskCommand("schedule"), now)
	if err != nil || scheduled.RecurrenceID == "" || scheduled.Task.Revision != 2 {
		t.Fatalf("scheduled = %#v, %v", scheduled, err)
	}
	if _, err := database.SetTaskSchedule(ctx, taskID, 1, value, true, testTaskCommand("stale"), now); !errors.Is(err, ErrStaleRevision) {
		t.Fatalf("stale schedule error = %v", err)
	}
	recurrence, err := database.TaskRecurrence(ctx, scheduled.RecurrenceID)
	if err != nil || recurrence.Revision != 1 || recurrence.Lifecycle != RecurrenceActive {
		t.Fatalf("recurrence = %#v, %v", recurrence, err)
	}
	title := "Changed recurring Task"
	updated, err := database.UpdateTaskRecurrence(ctx, recurrence.ID, 1,
		RecurrenceChanges{Title: &title}, testTaskCommand("update"), now.Add(time.Minute))
	if err != nil || updated.Event.Kind != "task.recurrence_changed" {
		t.Fatalf("updated recurrence = %#v, %v", updated, err)
	}
	if _, err := database.SetTaskRecurrenceLifecycle(ctx, recurrence.ID, 2, RecurrencePaused,
		testTaskCommand("pause"), now); err != nil {
		t.Fatal(err)
	}
	if _, err := database.SetTaskRecurrenceLifecycle(ctx, recurrence.ID, 3, RecurrenceActive,
		testTaskCommand("resume"), now); err != nil {
		t.Fatal(err)
	}
	if _, err := database.SkipTaskRecurrenceNext(ctx, recurrence.ID, 4,
		testTaskCommand("skip"), now); err != nil {
		t.Fatal(err)
	}
	occurrences, err := database.TaskRecurrenceOccurrences(ctx, recurrence.ID, 20)
	if err != nil || len(occurrences) != 2 ||
		(occurrences[0].Resolution != "skipped" && occurrences[1].Resolution != "skipped") {
		t.Fatalf("occurrences = %#v, %v", occurrences, err)
	}
	if _, err := database.SetTaskRecurrenceLifecycle(ctx, recurrence.ID, 5, RecurrenceEnded,
		testTaskCommand("end"), now); err != nil {
		t.Fatal(err)
	}
}

func TestDueSchedulesApplyMissedAndOverlapPolicies(t *testing.T) {
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 12, 5, 0, 0, time.UTC)

	t.Run("one-time missed skip", func(t *testing.T) {
		database := openTestStore(t)
		id, _ := NewTaskID()
		instant := now.Add(-time.Minute)
		value, err := schedule.Normalize(schedule.Schedule{ScheduledFor: instant, TimeZone: "UTC",
			MissedRunPolicy: schedule.MissedRunSkip}, now)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := database.CreateTaskWithOptions(ctx, id, "Missed", testTaskCommand("missed-create"),
			TaskCreateOptions{Schedule: &value}, now.Add(-time.Hour)); err != nil {
			t.Fatal(err)
		}
		if _, err := database.ProcessDueTaskSchedules(ctx, now, true, nil); err != nil {
			t.Fatal(err)
		}
		task, _ := database.Task(ctx, id)
		if task.State != TaskCancelled || task.ScheduleProcessedAt == nil {
			t.Fatalf("missed Task = %#v", task)
		}
	})

	t.Run("queue one releases after active Task", func(t *testing.T) {
		database := openTestStore(t)
		id, _ := NewTaskID()
		first := now.Add(-2 * time.Minute)
		value := normalizedTestSchedule(t, first, schedule.MissedRunOnce, schedule.OverlapQueueOne)
		created, err := database.CreateTaskWithOptions(ctx, id, "Queued recurrence", testTaskCommand("queue-create"),
			TaskCreateOptions{Schedule: &value}, now.Add(-time.Hour))
		if err != nil {
			t.Fatal(err)
		}
		if _, err := database.StartTask(ctx, id, "run:early", now.Add(-time.Minute)); !errors.Is(err, ErrInvalidTransition) {
			t.Fatalf("early scheduled start error = %v", err)
		}
		materialized, err := database.ProcessDueTaskSchedules(ctx, now, true, nil)
		if err != nil || len(materialized) != 0 {
			t.Fatalf("first due = %#v, %v", materialized, err)
		}
		recurrence, _ := database.TaskRecurrence(ctx, created.RecurrenceID)
		if recurrence.PendingCoalescedAt == nil {
			t.Fatal("queue-one did not retain one occurrence")
		}
		started, err := database.StartTask(ctx, id, "run:test", now)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := database.FinishTask(ctx, id, started.CurrentRunID, TaskCompleted, now); err != nil {
			t.Fatal(err)
		}
		materialized, err = database.ProcessDueTaskSchedules(ctx, now, false, nil)
		if err != nil || len(materialized) != 1 || materialized[0].RecurrenceID != created.RecurrenceID {
			t.Fatalf("released due = %#v, %v", materialized, err)
		}
		occurrences, _ := database.TaskRecurrenceOccurrences(ctx, created.RecurrenceID, 20)
		if occurrences[0].Resolution != "materialized" || occurrences[0].TaskID == "" {
			t.Fatalf("released occurrence = %#v", occurrences[0])
		}
	})
}

func TestTaskPlacementSnapshotsAcpRevisionAndGuardsDeletion(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	agent, err := database.CreateAcpAgent(ctx, "Local", "agent", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	id, _ := NewTaskID()
	first := now.Add(time.Hour)
	value := normalizedTestSchedule(t, first, schedule.MissedRunOnce, schedule.OverlapAllow)
	created, err := database.CreateTaskWithOptions(ctx, id, "Placed", testTaskCommand("placed-create"),
		TaskCreateOptions{ExecutorAgentID: agent.AgentID, Schedule: &value}, now)
	if err != nil || created.Task.ExecutorAcpConnectionRevision == nil ||
		*created.Task.ExecutorAcpConnectionRevision != agent.ConnectionRevision {
		t.Fatalf("placed Task = %#v, %v", created.Task, err)
	}
	recurrence, _ := database.TaskRecurrence(ctx, created.RecurrenceID)
	if recurrence.ExecutorAcpConnectionRevision == nil ||
		*recurrence.ExecutorAcpConnectionRevision != agent.ConnectionRevision {
		t.Fatalf("recurrence executor = %#v", recurrence)
	}
	if _, err := database.DeleteAcpAgent(ctx, agent.AgentID, agent.ConnectionRevision); !errors.Is(err, ErrAcpAgentInUse) {
		t.Fatalf("delete referenced Agent error = %v", err)
	}
}

func normalizedTestSchedule(
	t *testing.T, first time.Time, missed schedule.MissedRunPolicy, overlap schedule.OverlapPolicy,
) schedule.Schedule {
	t.Helper()
	value, err := schedule.Normalize(schedule.Schedule{ScheduledFor: first, TimeZone: "UTC",
		MissedRunPolicy: missed, Recurrence: &schedule.Recurrence{StartsAt: first,
			CronExpression: "* * * * *", OverlapPolicy: overlap}}, first)
	if err != nil {
		t.Fatal(err)
	}
	return value
}

func testTaskCommand(name string) TaskCommand {
	return TaskCommand{Name: name, ClientMutationID: name,
		RequestDigest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
		CorrelationID: "correlation:test:" + name}
}
