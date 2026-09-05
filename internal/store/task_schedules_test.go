package store

import (
	"context"
	"errors"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/schedule"
)

func TestReleasedSchedulesEnterExecutionQueueOnce(t *testing.T) {
	database := openTestStore(t)
	ctx, now := t.Context(), time.Now().UTC()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	ids := make(map[string]string)
	for _, name := range []string{"inbox", "future", "due", "manual"} {
		id, _ := NewTaskID()
		ids[name] = id
		created, err := database.CreateTaskWithOptions(ctx, id, name, testTaskCommand(name), TaskCreateOptions{}, now)
		if err != nil {
			t.Fatal(err)
		}
		if name == "inbox" {
			continue
		}
		instant := now.Add(time.Hour)
		if name == "due" {
			instant = now.Add(-time.Second)
		}
		scheduled, err := database.SetTaskSchedule(ctx, id, created.Task.Revision,
			schedule.Schedule{ScheduledFor: instant, TimeZone: "UTC", MissedRunPolicy: schedule.MissedRunOnce},
			false, testTaskCommand("schedule-"+name), now)
		if err != nil {
			t.Fatal(err)
		}
		if name == "manual" {
			if _, err := database.RunScheduledTaskNow(ctx, id, scheduled.Task.Revision, testTaskCommand("run-now"), now); err != nil {
				t.Fatal(err)
			}
		}
	}
	if _, _, err := database.ProcessDueTaskSchedules(ctx, now, false, nil); err != nil {
		t.Fatal(err)
	}
	// Repeating the handoff covers wakeups and recovery after a committed queue.
	for range 2 {
		if err := database.QueueReleasedTaskSchedules(ctx, now); err != nil {
			t.Fatal(err)
		}
	}
	for name, id := range ids {
		runs, err := database.TaskRuns(ctx, id, 10)
		want := 0
		if name == "due" || name == "manual" {
			want = 1
		}
		if err != nil || len(runs) != want {
			t.Fatalf("%s runs = %#v, %v", name, runs, err)
		}
		if want == 1 && (runs[0].Kind != "planner" || runs[0].Status != "queued") {
			t.Fatalf("%s run = %#v", name, runs[0])
		}
	}
}

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
		if _, _, err := database.ProcessDueTaskSchedules(ctx, now, true, nil); err != nil {
			t.Fatal(err)
		}
		task, _ := database.Task(ctx, id)
		if task.State != TaskCancelled || task.StageKey != "cancelled" || task.CancelledAt == nil || task.CompletedAt != nil || task.CurrentRunID != "" || task.ScheduleProcessedAt == nil {
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
		materialized, _, err := database.ProcessDueTaskSchedules(ctx, now, true, nil)
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
		materialized, _, err = database.ProcessDueTaskSchedules(ctx, now, false, nil)
		if err != nil || len(materialized) != 1 || materialized[0].RecurrenceID != created.RecurrenceID {
			t.Fatalf("released due = %#v, %v", materialized, err)
		}
		occurrences, _ := database.TaskRecurrenceOccurrences(ctx, created.RecurrenceID, 20)
		if occurrences[0].Resolution != "materialized" || occurrences[0].TaskID == "" {
			t.Fatalf("released occurrence = %#v", occurrences[0])
		}
	})
}

func TestRecurrenceOverlapPoliciesAcrossActiveSlots(t *testing.T) {
	for _, test := range []struct {
		name        string
		policy      schedule.OverlapPolicy
		activeSlots int
		skipFuture  bool
	}{
		{"skip", schedule.OverlapSkip, 2, false},
		{"queue_one", schedule.OverlapQueueOne, 2, false},
		{"allow", schedule.OverlapAllow, 2, false},
		{"queue_one preserves future skip", schedule.OverlapQueueOne, 1, true},
	} {
		t.Run(test.name, func(t *testing.T) {
			policy := test.policy
			database := openTestStore(t)
			ctx := t.Context()
			first := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
			id, err := NewTaskID()
			if err != nil {
				t.Fatal(err)
			}
			value := normalizedTestSchedule(t, first, schedule.MissedRunOnce, policy)
			created, err := database.CreateTaskWithOptions(ctx, id, "Overlap audit", testTaskCommand("overlap-create"),
				TaskCreateOptions{Schedule: &value}, first.Add(-time.Minute))
			if err != nil {
				t.Fatal(err)
			}
			if _, _, err := database.ProcessDueTaskSchedules(ctx, first, false, nil); err != nil {
				t.Fatal(err)
			}
			started, err := database.StartTask(ctx, id, "run:active", first)
			if err != nil {
				t.Fatal(err)
			}
			for minute := 1; minute <= test.activeSlots; minute++ {
				at := first.Add(time.Duration(minute) * time.Minute)
				newTasks, _, err := database.ProcessDueTaskSchedules(ctx, at, false, nil)
				want := 0
				if policy == schedule.OverlapAllow {
					want = 1
				}
				if err != nil || len(newTasks) != want {
					t.Fatalf("active slot %d = %#v, %v; want %d new Tasks", minute, newTasks, err, want)
				}
			}
			recurrence, err := database.TaskRecurrence(ctx, created.RecurrenceID)
			if err != nil {
				t.Fatal(err)
			}
			if policy == schedule.OverlapQueueOne {
				if recurrence.PendingCoalescedAt == nil || !recurrence.PendingCoalescedAt.Equal(first.Add(time.Minute)) {
					t.Fatalf("pending slot = %#v", recurrence.PendingCoalescedAt)
				}
			} else if recurrence.PendingCoalescedAt != nil {
				t.Fatalf("unexpected pending slot = %#v", recurrence.PendingCoalescedAt)
			}
			finishedAt := first.Add(time.Duration(test.activeSlots)*time.Minute + 30*time.Second)
			wantNext := first.Add(time.Duration(test.activeSlots+1) * time.Minute)
			if test.skipFuture {
				if _, err := database.SkipTaskRecurrenceNext(ctx, created.RecurrenceID, recurrence.Revision,
					testTaskCommand("skip-future"), finishedAt); err != nil {
					t.Fatal(err)
				}
				wantNext = wantNext.Add(time.Minute)
			}
			if _, err := database.FinishTask(ctx, id, started.CurrentRunID, TaskCompleted, finishedAt); err != nil {
				t.Fatal(err)
			}
			for wake := 0; wake < 2; wake++ {
				newTasks, _, err := database.ProcessDueTaskSchedules(ctx, finishedAt, false, nil)
				want := 0
				if policy == schedule.OverlapQueueOne && wake == 0 {
					want = 1
				}
				if err != nil || len(newTasks) != want {
					t.Fatalf("release wake %d = %#v, %v; want %d new Tasks", wake, newTasks, err, want)
				}
				if policy == schedule.OverlapQueueOne && wake == 0 {
					catchup, err := database.StartTask(ctx, newTasks[0].TaskID, "run:catchup", finishedAt)
					if err != nil {
						t.Fatal(err)
					}
					if _, err := database.FinishTask(ctx, catchup.ID, catchup.CurrentRunID, TaskCompleted, finishedAt); err != nil {
						t.Fatal(err)
					}
				}
			}
			recurrence, err = database.TaskRecurrence(ctx, created.RecurrenceID)
			if err != nil || recurrence.PendingCoalescedAt != nil || recurrence.NextRunAt == nil || !recurrence.NextRunAt.Equal(wantNext) {
				t.Fatalf("released recurrence retains past work = %#v, %v", recurrence, err)
			}
			occurrences, err := database.TaskRecurrenceOccurrences(ctx, created.RecurrenceID, 10)
			if err != nil {
				t.Fatal(err)
			}
			tasks := make(map[string]bool)
			for _, occurrence := range occurrences {
				if occurrence.TaskID != "" {
					if tasks[occurrence.TaskID] {
						t.Fatalf("duplicate Task in history: %s", occurrence.TaskID)
					}
					tasks[occurrence.TaskID] = true
				} else if policy == schedule.OverlapSkip && occurrence.Resolution != "skipped" {
					t.Fatalf("skip resolution = %#v", occurrence)
				}
			}
			wantTasks := map[schedule.OverlapPolicy]int{schedule.OverlapSkip: 1, schedule.OverlapQueueOne: 2, schedule.OverlapAllow: 3}[policy]
			if len(tasks) != wantTasks {
				t.Fatalf("Task count = %d, want %d; occurrences = %#v", len(tasks), wantTasks, occurrences)
			}
		})
	}
}

func TestDueSchedulesRespectDaylightSavingTransitions(t *testing.T) {
	for _, test := range []struct {
		name, cron, first, transition, next string
	}{
		{"spring missing minute", "30 2 * * *", "2026-03-07T07:30:00Z", "2026-03-08T07:30:00Z", "2026-03-09T06:30:00Z"},
		{"fall repeated minute", "30 1 * * *", "2026-11-01T05:30:00Z", "2026-11-01T06:30:00Z", "2026-11-02T06:30:00Z"},
	} {
		t.Run(test.name, func(t *testing.T) {
			database := openTestStore(t)
			ctx := t.Context()
			account := createReadyModelAccount(t, database)
			if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
				t.Fatal(err)
			}
			instants := make([]time.Time, 0, 3)
			for _, value := range []string{test.first, test.transition, test.next} {
				instant, err := time.Parse(time.RFC3339, value)
				if err != nil {
					t.Fatal(err)
				}
				instants = append(instants, instant)
			}
			first, transition, next := instants[0], instants[1], instants[2]
			value, err := schedule.Normalize(schedule.Schedule{ScheduledFor: first, TimeZone: "America/New_York",
				MissedRunPolicy: schedule.MissedRunOnce, Recurrence: &schedule.Recurrence{StartsAt: first,
					CronExpression: test.cron, OverlapPolicy: schedule.OverlapAllow}}, first.Add(-time.Hour))
			if err != nil {
				t.Fatal(err)
			}
			id, err := NewTaskID()
			if err != nil {
				t.Fatal(err)
			}
			created, err := database.CreateTaskWithOptions(ctx, id, test.name, testTaskCommand("dst-create"),
				TaskCreateOptions{Schedule: &value}, first.Add(-time.Hour))
			if err != nil {
				t.Fatal(err)
			}
			for index, instant := range instants {
				materialized, _, err := database.ProcessDueTaskSchedules(ctx, instant, false, nil)
				wantNew := 0
				if index == 2 {
					wantNew = 1
				}
				if err != nil || len(materialized) != wantNew {
					t.Fatalf("due at %s = %#v, %v; want %d new Tasks", instant, materialized, err, wantNew)
				}
				if err := database.QueueReleasedTaskSchedules(ctx, instant); err != nil {
					t.Fatal(err)
				}
				occurrences, err := database.TaskRecurrenceOccurrences(ctx, created.RecurrenceID, 10)
				if err != nil || len(occurrences) != 1+wantNew {
					t.Fatalf("occurrences at %s = %#v, %v", instant, occurrences, err)
				}
				for _, occurrence := range occurrences {
					runs, err := database.TaskRuns(ctx, occurrence.TaskID, 10)
					if err != nil || len(runs) != 1 || runs[0].Kind != "planner" || runs[0].Status != "queued" {
						t.Fatalf("runs at %s = %#v, %v", instant, runs, err)
					}
				}
				if instant.Equal(transition) {
					recurrence, err := database.TaskRecurrence(ctx, created.RecurrenceID)
					if err != nil || recurrence.NextRunAt == nil || !recurrence.NextRunAt.Equal(next) {
						t.Fatalf("next run after transition = %#v, %v", recurrence, err)
					}
				}
			}
		})
	}
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
