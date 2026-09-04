package graphql

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/schedule"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/vektah/gqlparser/v2/gqlerror"
)

func newTaskSchedule(input *model.NewTaskScheduleInput, now time.Time) (*schedule.Schedule, error) {
	if input == nil {
		return nil, nil
	}
	scheduledFor, err := schedule.ParseInstant(input.ScheduledFor)
	if err != nil {
		return nil, err
	}
	value := schedule.Schedule{ScheduledFor: scheduledFor, TimeZone: input.TimeZone,
		MissedRunPolicy: schedule.MissedRunOnce}
	if input.MissedRunPolicy != nil {
		value.MissedRunPolicy = missedRunPolicy(*input.MissedRunPolicy)
	}
	if input.Recurrence != nil {
		startsAt, err := schedule.ParseInstant(input.Recurrence.StartsAt)
		if err != nil {
			return nil, err
		}
		overlap := schedule.OverlapSkip
		if input.Recurrence.OverlapPolicy != nil {
			overlap = overlapPolicy(*input.Recurrence.OverlapPolicy)
		}
		value.Recurrence = &schedule.Recurrence{StartsAt: startsAt,
			CronExpression: input.Recurrence.CronExpression, OverlapPolicy: overlap}
	}
	normalized, err := schedule.Normalize(value, now)
	if err != nil {
		return nil, err
	}
	return &normalized, nil
}

func missedRunPolicy(value model.MissedRunPolicy) schedule.MissedRunPolicy {
	if value == model.MissedRunPolicySkip {
		return schedule.MissedRunSkip
	}
	return schedule.MissedRunOnce
}

func overlapPolicy(value model.OverlapPolicy) schedule.OverlapPolicy {
	switch value {
	case model.OverlapPolicyQueueOne:
		return schedule.OverlapQueueOne
	case model.OverlapPolicyAllow:
		return schedule.OverlapAllow
	default:
		return schedule.OverlapSkip
	}
}

func (r *Resolver) setTaskSchedule(
	ctx context.Context, input model.ScheduleTaskInput, requireExisting bool,
) (*model.TaskCommandPayload, error) {
	if input.ExpectedGeneration != 1 || input.ClientMutationID == "" || input.Schedule == nil {
		return nil, errors.New("invalid scheduled Task command")
	}
	value, err := newTaskSchedule(input.Schedule, time.Now())
	if err != nil {
		return nil, err
	}
	name := "schedule_task"
	if requireExisting {
		name = "reschedule_task"
	}
	command, err := newTaskCommand(name, input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	result, err := r.Store.SetTaskSchedule(ctx, input.TaskID, int64(input.ExpectedRevision), *value,
		requireExisting, command, time.Now())
	if err != nil {
		return nil, taskScheduleError(err)
	}
	if result.RecurrenceID != "" {
		document, readErr := home.ReadTaskDocument(r.home, result.Task.ID)
		if readErr != nil {
			return nil, readErr
		}
		if _, err := home.EnsureRecurrenceDocument(r.home, result.RecurrenceID, document.Content); err != nil {
			return nil, err
		}
	}
	if result.ObsoleteRecurrenceID != "" {
		if err := home.DeleteRecurrenceDocument(r.home, result.ObsoleteRecurrenceID); err != nil {
			return nil, err
		}
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) unscheduleTask(
	ctx context.Context, input model.UnscheduleTaskInput,
) (*model.TaskCommandPayload, error) {
	if input.ExpectedGeneration != 1 || input.ClientMutationID == "" {
		return nil, errors.New("invalid unschedule command")
	}
	command, err := newTaskCommand("unschedule_task", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	result, err := r.Store.UnscheduleTask(ctx, input.TaskID, int64(input.ExpectedRevision), command, time.Now())
	if err != nil {
		return nil, taskScheduleError(err)
	}
	if result.ObsoleteRecurrenceID != "" {
		if err := home.DeleteRecurrenceDocument(r.home, result.ObsoleteRecurrenceID); err != nil {
			return nil, err
		}
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) runScheduledTaskNow(
	ctx context.Context, input model.RunScheduledTaskNowInput,
) (*model.TaskCommandPayload, error) {
	if input.ExpectedGeneration != 1 || input.ClientMutationID == "" {
		return nil, errors.New("invalid run scheduled Task command")
	}
	command, err := newTaskCommand("run_scheduled_task_now", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	result, err := r.Store.RunScheduledTaskNow(ctx, input.TaskID, int64(input.ExpectedRevision), command, time.Now())
	if err != nil {
		return nil, taskScheduleError(err)
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) updateTaskRecurrence(
	ctx context.Context, input model.UpdateTaskRecurrenceInput,
) (*model.TaskCommandPayload, error) {
	if input.ClientMutationID == "" {
		return nil, errors.New("clientMutationId cannot be empty")
	}
	changes := store.RecurrenceChanges{Title: input.Title, CronExpression: input.CronExpression,
		TimeZone: input.TimeZone}
	if input.ClearProject != nil && *input.ClearProject {
		changes.SetProject = true
		changes.ProjectID = nil
	} else if input.ProjectID != nil {
		changes.SetProject = true
		changes.ProjectID = input.ProjectID
	}
	if input.StartsAt != nil {
		value, err := schedule.ParseInstant(*input.StartsAt)
		if err != nil {
			return nil, err
		}
		changes.StartsAt = &value
	}
	if input.CronExpression != nil {
		value := strings.Join(strings.Fields(*input.CronExpression), " ")
		changes.CronExpression = &value
	}
	if input.TimeZone != nil {
		value := strings.TrimSpace(*input.TimeZone)
		changes.TimeZone = &value
	}
	if input.MissedRunPolicy != nil {
		value := missedRunPolicy(*input.MissedRunPolicy)
		changes.MissedRunPolicy = &value
	}
	if input.OverlapPolicy != nil {
		value := overlapPolicy(*input.OverlapPolicy)
		changes.OverlapPolicy = &value
	}
	command, err := newTaskCommand("update_task_recurrence", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	if replay, found, err := r.Store.LookupTaskCommandReceipt(ctx, command); err != nil {
		return nil, taskScheduleError(err)
	} else if found {
		return r.taskCommandPayload(ctx, replay, input.ClientMutationID)
	}
	var previous, replacement home.TaskDocument
	var documentChanged bool
	if input.TaskDocument != nil {
		if input.ExpectedTaskDocumentDigest == nil {
			return nil, errors.New("expectedTaskDocumentDigest is required")
		}
		var err error
		previous, err = home.ReadRecurrenceDocument(r.home, input.RecurrenceID)
		if err != nil {
			return nil, err
		}
		replacement, err = home.WriteRecurrenceDocument(r.home, input.RecurrenceID,
			*input.TaskDocument, input.ExpectedTaskDocumentDigest)
		if err != nil {
			return nil, taskScheduleError(err)
		}
		documentChanged = true
	}
	result, err := r.Store.UpdateTaskRecurrence(ctx, input.RecurrenceID,
		int64(input.ExpectedRevision), changes, command, time.Now())
	if err != nil {
		if documentChanged {
			_, rollbackErr := home.WriteRecurrenceDocument(r.home, input.RecurrenceID,
				previous.Content, &replacement.Digest)
			err = errors.Join(err, rollbackErr)
		}
		return nil, taskScheduleError(err)
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) taskRecurrenceLifecycle(
	ctx context.Context, input model.TaskRecurrenceCommandInput, lifecycle store.RecurrenceLifecycle,
) (*model.TaskCommandPayload, error) {
	if input.ClientMutationID == "" {
		return nil, errors.New("clientMutationId cannot be empty")
	}
	command, err := newTaskCommand("set_task_recurrence_"+string(lifecycle), input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	result, err := r.Store.SetTaskRecurrenceLifecycle(ctx, input.RecurrenceID,
		int64(input.ExpectedRevision), lifecycle, command, time.Now())
	if err != nil {
		return nil, taskScheduleError(err)
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) skipTaskRecurrenceNext(
	ctx context.Context, input model.TaskRecurrenceCommandInput,
) (*model.TaskCommandPayload, error) {
	if input.ClientMutationID == "" {
		return nil, errors.New("clientMutationId cannot be empty")
	}
	command, err := newTaskCommand("skip_task_recurrence_next", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	result, err := r.Store.SkipTaskRecurrenceNext(ctx, input.RecurrenceID,
		int64(input.ExpectedRevision), command, time.Now())
	if err != nil {
		return nil, taskScheduleError(err)
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) runTaskRecurrenceNow(
	ctx context.Context, input model.TaskRecurrenceCommandInput,
) (*model.TaskCommandPayload, error) {
	if input.ClientMutationID == "" {
		return nil, errors.New("clientMutationId cannot be empty")
	}
	command, err := newTaskCommand("run_task_recurrence_now", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	result, err := r.Store.RunTaskRecurrenceNow(ctx, input.RecurrenceID,
		int64(input.ExpectedRevision), command, time.Now())
	if err != nil {
		return nil, taskScheduleError(err)
	}
	if err := home.CopyRecurrenceDocumentToTask(r.home, input.RecurrenceID, result.Task.ID); err != nil {
		return nil, err
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) taskCommandPayload(
	ctx context.Context, result store.TaskCommandResult, clientID string,
) (*model.TaskCommandPayload, error) {
	document, err := home.ReadTaskDocument(r.home, result.Task.ID)
	if err != nil {
		return nil, err
	}
	cursor, err := store.EncodeWorkEventCursor(result.Event.ID)
	if err != nil {
		return nil, err
	}
	return &model.TaskCommandPayload{Task: r.taskDetailModel(ctx, result.Task, document),
		EventCursor: cursor, ClientMutationID: clientID}, nil
}

func (r *Resolver) taskSchedulePreview(
	input model.TaskSchedulePreviewInput,
) (*model.TaskSchedulePreview, error) {
	start, err := schedule.ParseInstant(input.StartsAt)
	if err != nil {
		return nil, err
	}
	values := []time.Time{start}
	if input.CronExpression != nil {
		values, err = schedule.Preview(*input.CronExpression, input.TimeZone, start)
	} else {
		_, err = schedule.Normalize(schedule.Schedule{ScheduledFor: start,
			TimeZone: input.TimeZone, MissedRunPolicy: schedule.MissedRunOnce}, time.Now())
	}
	if err != nil {
		return nil, err
	}
	result := &model.TaskSchedulePreview{ResolvedStart: values[0].Format(time.RFC3339Nano),
		Occurrences: make([]string, len(values))}
	for index, value := range values {
		result.Occurrences[index] = value.Format(time.RFC3339Nano)
	}
	return result, nil
}

func (r *Resolver) taskRecurrence(
	ctx context.Context, recurrenceID string, first *int,
) (*model.TaskRecurrence, error) {
	limit, err := recurrenceLimit(first)
	if err != nil {
		return nil, err
	}
	value, err := r.Store.TaskRecurrence(ctx, recurrenceID)
	if err != nil {
		return nil, err
	}
	document, err := home.ReadRecurrenceDocument(r.home, recurrenceID)
	if err != nil {
		return nil, err
	}
	occurrences, err := r.Store.TaskRecurrenceOccurrences(ctx, recurrenceID, limit)
	if err != nil {
		return nil, err
	}
	result := recurrenceModel(value)
	result.TaskDocument, result.TaskDocumentDigest = document.Content, document.Digest
	result.Occurrences = make([]*model.RecurrenceOccurrence, len(occurrences))
	for index, occurrence := range occurrences {
		result.Occurrences[index] = occurrenceModel(occurrence)
	}
	return result, nil
}

func (r *Resolver) taskRecurrences(
	ctx context.Context, workspaceID string, projectID *string, first *int,
) ([]*model.TaskRecurrenceSummary, error) {
	if workspaceID != personalWorkspaceID {
		return nil, errors.New("invalid recurrence workspace")
	}
	limit, err := recurrenceLimit(first)
	if err != nil {
		return nil, err
	}
	project := ""
	if projectID != nil {
		project = *projectID
	}
	values, err := r.Store.TaskRecurrences(ctx, project, limit)
	if err != nil {
		return nil, err
	}
	result := make([]*model.TaskRecurrenceSummary, len(values))
	for index, value := range values {
		item := recurrenceModel(value)
		result[index] = &model.TaskRecurrenceSummary{RecurrenceID: item.RecurrenceID,
			Title: item.Title, CronExpression: item.CronExpression, Lifecycle: item.Lifecycle,
			NextRunAt: item.NextRunAt, UpdatedAt: value.UpdatedAt.Format(time.RFC3339Nano)}
	}
	return result, nil
}

func recurrenceModel(value store.TaskRecurrence) *model.TaskRecurrence {
	result := &model.TaskRecurrence{RecurrenceID: value.ID, Title: value.Title,
		StartsAt: value.StartsAt.Format(time.RFC3339Nano), CronExpression: value.CronExpression,
		TimeZone: value.TimeZone, MissedRunPolicy: missedRunModel(string(value.MissedRunPolicy)),
		OverlapPolicy: overlapPolicyModel(value.OverlapPolicy), Lifecycle: recurrenceLifecycleModel(value.Lifecycle),
		Revision: int(value.Revision), Occurrences: []*model.RecurrenceOccurrence{}}
	if value.NextRunAt != nil {
		formatted := value.NextRunAt.Format(time.RFC3339Nano)
		result.NextRunAt = &formatted
	}
	if value.PendingCoalescedAt != nil {
		formatted := value.PendingCoalescedAt.Format(time.RFC3339Nano)
		result.PendingCoalescedAt = &formatted
	}
	return result
}

func occurrenceModel(value store.RecurrenceOccurrence) *model.RecurrenceOccurrence {
	result := &model.RecurrenceOccurrence{RecurrenceRevision: int(value.RecurrenceRevision),
		ScheduledFor: value.ScheduledFor.Format(time.RFC3339Nano), LocalSlot: value.LocalSlot,
		Trigger:    model.RecurrenceOccurrenceTrigger(strings.ToUpper(value.Trigger)),
		Resolution: model.RecurrenceOccurrenceResolution(strings.ToUpper(value.Resolution)),
		CreatedAt:  value.CreatedAt.Format(time.RFC3339Nano)}
	if value.TaskID != "" {
		result.TaskID = &value.TaskID
	}
	return result
}

func overlapPolicyModel(value schedule.OverlapPolicy) model.OverlapPolicy {
	return model.OverlapPolicy(strings.ToUpper(string(value)))
}

func recurrenceLifecycleModel(value store.RecurrenceLifecycle) model.RecurrenceLifecycle {
	return model.RecurrenceLifecycle(strings.ToUpper(string(value)))
}

func recurrenceLimit(first *int) (int, error) {
	if first == nil {
		return 20, nil
	}
	if *first < 1 || *first > 100 {
		return 0, errors.New("recurrence history limit must be from 1 through 100")
	}
	return *first, nil
}

func newTaskCommand(name, clientID string, input any) (store.TaskCommand, error) {
	encoded, err := json.Marshal(input)
	if err != nil {
		return store.TaskCommand{}, err
	}
	digest := sha256.Sum256(encoded)
	return store.TaskCommand{Name: name, ClientMutationID: clientID,
		RequestDigest: hex.EncodeToString(digest[:]), CorrelationID: "correlation:graphql:" + clientID}, nil
}

func taskScheduleError(err error) error {
	if err == nil {
		return nil
	}
	code, message := "work_unavailable", "Task schedule authority is unavailable"
	switch {
	case errors.Is(err, store.ErrStaleRevision):
		code, message = "stale_revision", "the authoritative Task revision is stale"
	case errors.Is(err, store.ErrInvalidTransition):
		code, message = "invalid_transition", "the requested Task schedule action is not valid now"
	case errors.Is(err, store.ErrCommandConflict):
		code, message = "idempotency_conflict", "the command key conflicts with an earlier request"
	case errors.Is(err, home.ErrRecurrenceDocumentChanged):
		code, message = "stale_document", "the authoritative recurrence Task document changed"
	case errors.Is(err, schedule.ErrInvalid):
		code, message = "invalid_input", err.Error()
	}
	result := gqlerror.Errorf("%s", message)
	result.Extensions = map[string]any{"code": code}
	return result
}
