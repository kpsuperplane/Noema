package graphql

import (
	"context"
	"fmt"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestTaskScheduleGraphQLCommandsAndRecurrenceAuthority(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	first := time.Now().UTC().Truncate(time.Minute).Add(2 * time.Minute)
	cron := fmt.Sprintf("%d %d * * *", first.Minute(), first.Hour())
	created, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID,
		Title: "Recurring", TaskDocument: "# Recurring\n", ClientMutationID: "capture-recurring",
		Schedule: &model.NewTaskScheduleInput{ScheduledFor: first.Format(time.RFC3339), TimeZone: "UTC",
			Recurrence: &model.NewTaskRecurrenceInput{StartsAt: first.Format(time.RFC3339), CronExpression: cron}}})
	if err != nil || created.Task.Schedule == nil || created.Task.Schedule.RecurrenceID == nil {
		t.Fatalf("created scheduled Task = %#v, %v", created, err)
	}
	replayed, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID,
		Title: "Recurring", TaskDocument: "# Recurring\n", ClientMutationID: "capture-recurring",
		Schedule: &model.NewTaskScheduleInput{ScheduledFor: first.Format(time.RFC3339), TimeZone: "UTC",
			Recurrence: &model.NewTaskRecurrenceInput{StartsAt: first.Format(time.RFC3339), CronExpression: cron}}})
	if err != nil || replayed.Task.TaskID != created.Task.TaskID {
		t.Fatalf("replayed capture = %#v, %v", replayed, err)
	}
	recurrenceID := *created.Task.Schedule.RecurrenceID
	recurrence, err := resolver.taskRecurrence(ctx, recurrenceID, nil)
	if err != nil || recurrence.TaskDocument != "# Recurring\n" || len(recurrence.Occurrences) != 1 {
		t.Fatalf("recurrence = %#v, %v", recurrence, err)
	}
	nextDocument := "# Changed\n"
	title := "Changed"
	updated, err := resolver.updateTaskRecurrence(ctx, model.UpdateTaskRecurrenceInput{
		RecurrenceID: recurrenceID, ExpectedRevision: 1, Title: &title, TaskDocument: &nextDocument,
		ExpectedTaskDocumentDigest: &recurrence.TaskDocumentDigest, ClientMutationID: "update"})
	if err != nil || updated.ClientMutationID != "update" {
		t.Fatalf("updated recurrence = %#v, %v", updated, err)
	}
	command := model.TaskRecurrenceCommandInput{RecurrenceID: recurrenceID, ExpectedRevision: 2,
		ClientMutationID: "pause"}
	if _, err := resolver.taskRecurrenceLifecycle(ctx, command, store.RecurrencePaused); err != nil {
		t.Fatal(err)
	}
	command.ExpectedRevision, command.ClientMutationID = 3, "resume"
	if _, err := resolver.taskRecurrenceLifecycle(ctx, command, store.RecurrenceActive); err != nil {
		t.Fatal(err)
	}
	command.ExpectedRevision, command.ClientMutationID = 4, "skip"
	if _, err := resolver.skipTaskRecurrenceNext(ctx, command); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.runScheduledTaskNow(ctx, model.RunScheduledTaskNowInput{TaskID: created.Task.TaskID,
		ExpectedRevision: created.Task.Revision, ExpectedGeneration: 1, ClientMutationID: "run-first"}); err != nil {
		t.Fatal(err)
	}
	started, err := resolver.Store.StartTask(ctx, created.Task.TaskID, "run:test", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.FinishTask(ctx, created.Task.TaskID, started.CurrentRunID,
		store.TaskCompleted, time.Now()); err != nil {
		t.Fatal(err)
	}
	command.ExpectedRevision, command.ClientMutationID = 5, "run-extra"
	manual, err := resolver.runTaskRecurrenceNow(ctx, command)
	if err != nil || manual.Task.TaskDocument != nextDocument || manual.Task.TaskID == created.Task.TaskID {
		t.Fatalf("manual occurrence = %#v, %v", manual, err)
	}
	replayedManual, err := resolver.runTaskRecurrenceNow(ctx, command)
	if err != nil || replayedManual.Task.TaskID != manual.Task.TaskID {
		t.Fatalf("replayed manual occurrence = %#v, %v", replayedManual, err)
	}
	listed, err := resolver.taskRecurrences(ctx, personalWorkspaceID, nil, nil)
	if err != nil || len(listed) != 1 || listed[0].Title != title {
		t.Fatalf("listed recurrences = %#v, %v", listed, err)
	}
	oneTime, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID,
		Title: "One time", TaskDocument: "# One time\n", ClientMutationID: "capture-once"})
	if err != nil {
		t.Fatal(err)
	}
	scheduled, err := resolver.setTaskSchedule(ctx, model.ScheduleTaskInput{TaskID: oneTime.Task.TaskID,
		ExpectedRevision: 1, ExpectedGeneration: 1, ClientMutationID: "schedule-once",
		Schedule: &model.NewTaskScheduleInput{ScheduledFor: first.Format(time.RFC3339), TimeZone: "UTC"}}, false)
	if err != nil || scheduled.Task.Schedule == nil {
		t.Fatalf("scheduled one-time Task = %#v, %v", scheduled, err)
	}
	second := first.Add(time.Hour)
	rescheduled, err := resolver.setTaskSchedule(ctx, model.ScheduleTaskInput{TaskID: oneTime.Task.TaskID,
		ExpectedRevision: 2, ExpectedGeneration: 1, ClientMutationID: "reschedule-once",
		Schedule: &model.NewTaskScheduleInput{ScheduledFor: second.Format(time.RFC3339), TimeZone: "UTC"}}, true)
	if err != nil || rescheduled.Task.Schedule.ScheduledFor != second.Format(time.RFC3339) {
		t.Fatalf("rescheduled one-time Task = %#v, %v", rescheduled, err)
	}
	unscheduled, err := resolver.unscheduleTask(ctx, model.UnscheduleTaskInput{TaskID: oneTime.Task.TaskID,
		ExpectedRevision: 3, ExpectedGeneration: 1, ClientMutationID: "unschedule-once"})
	if err != nil || unscheduled.Task.Schedule != nil {
		t.Fatalf("unscheduled Task = %#v, %v", unscheduled, err)
	}
}
