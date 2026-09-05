package graphql

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/schedule"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/vektah/gqlparser/v2/gqlerror"
)

const personalWorkspaceID = "workspace:personal"

const taskDocumentPreviewLimit = 280

func (r *Resolver) captureTask(
	ctx context.Context,
	input model.CaptureTaskInput,
) (*model.TaskCommandPayload, error) {
	if r.Store == nil {
		return nil, errors.New("GraphQL Task store is unavailable")
	}
	unlockSchedules := r.Store.LockTaskSchedules()
	defer unlockSchedules()
	if input.WorkspaceID != personalWorkspaceID {
		return nil, errors.New("this migration slice supports only workspace:personal")
	}
	if input.ClientMutationID == "" {
		return nil, errors.New("clientMutationId cannot be empty")
	}
	command, err := newTaskCommand("capture_task", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	if replay, found, err := r.Store.LookupTaskCommandReceipt(ctx, command); err != nil {
		return nil, taskScheduleError(err)
	} else if found {
		if err := r.ensureRecurrenceDocument(replay); err != nil {
			return nil, err
		}
		r.Store.NotifyWork()
		return r.taskCommandPayload(ctx, replay, input.ClientMutationID)
	}

	taskID, err := store.NewTaskID()
	if err != nil {
		return nil, err
	}
	document, err := home.CreatePendingTaskDocument(r.home, taskID, input.TaskDocument)
	if err != nil {
		return nil, err
	}
	parsedSchedule, err := newTaskSchedule(input.Schedule, time.Now())
	if err != nil {
		_ = home.DiscardPendingTaskDocument(r.home, taskID)
		return nil, err
	}
	options := store.TaskCreateOptions{ExecutorAgentID: store.TaskExecutorAgentID,
		CwdOverride: input.CwdOverride, Schedule: parsedSchedule}
	if input.ProjectID != nil {
		options.ProjectID = *input.ProjectID
	}
	if input.ExecutorAgentID != nil {
		options.ExecutorAgentID = *input.ExecutorAgentID
	}
	result, err := r.Store.CreateTaskWithOptions(ctx, taskID, input.Title, command, options, time.Now())
	if err != nil {
		return nil, errors.Join(err, r.reconcileFailedTaskCreate(taskID))
	}
	if err := home.CommitTaskDocument(r.home, taskID); err != nil {
		return nil, err
	}
	if result.RecurrenceID != "" {
		if _, err := home.EnsureRecurrenceDocument(r.home, result.RecurrenceID, document.Content); err != nil {
			return nil, err
		}
	}
	r.Store.NotifyWork()
	cursor, err := store.EncodeWorkEventCursor(result.Event.ID)
	if err != nil {
		return nil, err
	}
	return &model.TaskCommandPayload{
		Task:             r.taskDetailModel(ctx, result.Task, document),
		EventCursor:      cursor,
		ClientMutationID: input.ClientMutationID,
	}, nil
}

func (r *Resolver) reconcileFailedTaskCreate(taskID string) error {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	stored, err := r.Store.TaskExists(ctx, taskID)
	if err != nil {
		return fmt.Errorf("reconcile failed Task create: %w", err)
	}
	if stored {
		return home.CommitTaskDocument(r.home, taskID)
	}
	return home.DiscardPendingTaskDocument(r.home, taskID)
}

func taskDetailModel(task store.Task, document home.TaskDocument) *model.TaskDetail {
	detail := &model.TaskDetail{
		TaskID:                   task.ID,
		Title:                    task.Title,
		TaskDocument:             document.Content,
		TaskDocumentDigest:       document.Digest,
		ResultMetadata:           map[string]any{},
		WorkspaceFiles:           []*model.TaskWorkspaceFile{},
		Stage:                    taskStageModel(task),
		Revision:                 int(task.Revision),
		Generation:               int(task.Generation),
		ExecutorAgentID:          task.ExecutorAgentID,
		ExecutorBackend:          taskExecutorBackend(task),
		CwdOverride:              task.CwdOverride,
		EffectiveCwdSource:       "default",
		CreatedAt:                task.CreatedAt.Format(time.RFC3339Nano),
		UpdatedAt:                task.UpdatedAt.Format(time.RFC3339Nano),
		Source:                   &model.TaskSource{},
		Messages:                 []*model.TaskMessage{},
		Runs:                     []*model.TaskRun{},
		ContributorInstanceNames: []string{},
		ValidActions:             validTaskActions(task),
	}
	detail.Schedule = taskScheduleModel(task)
	if task.CompletedAt != nil {
		completedAt := task.CompletedAt.Format(time.RFC3339Nano)
		detail.CompletedAt = &completedAt
	}
	return detail
}

func (r *Resolver) task(ctx context.Context, taskID string) (*model.TaskDetail, error) {
	task, err := r.Store.Task(ctx, taskID)
	if err != nil {
		return nil, err
	}
	document, err := home.ReadTaskDocument(r.home, taskID)
	if err != nil {
		return nil, err
	}
	return r.taskDetailModel(ctx, task, document), nil
}

func taskSummaryModel(task store.Task, workspaceID string, document string) *model.TaskSummary {
	result := &model.TaskSummary{
		TaskID: task.ID,
		Workspace: &model.Workspace{
			WorkspaceID: workspaceID,
			Name:        "Personal",
			Description: "",
			IsPersonal:  workspaceID == personalWorkspaceID,
		},
		Title:               task.Title,
		TaskDocumentPreview: taskDocumentPreview(document),
		Stage:               taskStageModel(task),
		Revision:            int(task.Revision),
		Generation:          int(task.Generation),
		ExecutorAgentID:     task.ExecutorAgentID,
		ExecutorBackend:     taskExecutorBackend(task),
		CwdOverride:         task.CwdOverride,
		EffectiveCwdSource:  "default",
		CreatedAt:           task.CreatedAt.Format(time.RFC3339Nano),
		UpdatedAt:           task.UpdatedAt.Format(time.RFC3339Nano),
		ValidActions:        validTaskActions(task),
	}
	result.Schedule = taskScheduleModel(task)
	if task.CompletedAt != nil {
		value := task.CompletedAt.Format(time.RFC3339Nano)
		result.CompletedAt = &value
	}
	return result
}

func (r *Resolver) taskSummaryModel(
	ctx context.Context, task store.Task, workspaceID string, document string,
) *model.TaskSummary {
	result := taskSummaryModel(task, workspaceID, document)
	if task.CwdOverride != nil {
		result.EffectiveCwd = task.CwdOverride
		result.EffectiveCwdSource = "task"
	}
	if task.ProjectID != "" {
		if project, err := r.Store.Project(ctx, task.ProjectID); err == nil {
			result.Project = projectModel(project)
			if result.EffectiveCwd == nil && project.Folder != nil {
				result.EffectiveCwd = project.Folder
				result.EffectiveCwdSource = "project"
			}
		}
	}
	r.hydrateTaskSummary(ctx, task, result)
	return result
}

func (r *Resolver) taskDetailModel(ctx context.Context, task store.Task, document home.TaskDocument) *model.TaskDetail {
	result := taskDetailModel(task, document)
	if task.CwdOverride != nil {
		result.EffectiveCwd = task.CwdOverride
		result.EffectiveCwdSource = "task"
	}
	if task.ProjectID != "" {
		if project, err := r.Store.Project(ctx, task.ProjectID); err == nil {
			result.Project = projectModel(project)
			if result.EffectiveCwd == nil && project.Folder != nil {
				result.EffectiveCwd = project.Folder
				result.EffectiveCwdSource = "project"
			}
		}
	}
	if runs, err := r.Store.TaskRuns(ctx, task.ID, 50); err == nil {
		for _, run := range runs {
			mapped := taskRunModel(run)
			result.Runs = append(result.Runs, mapped)
			if run.ID == task.CurrentRunID {
				result.CurrentRun = currentRunModel(run)
			}
		}
	}
	if messages, err := r.Store.TaskMessages(ctx, task.ID, 50); err == nil {
		for _, message := range messages {
			result.Messages = append(result.Messages, taskMessageModel(message))
		}
	}
	if task.ActiveGateID != "" {
		if gate, err := r.Store.TaskGate(ctx, task.ActiveGateID); err == nil {
			result.ActiveGate = taskGateModel(gate)
		}
	}
	return result
}

func taskExecutorBackend(task store.Task) string {
	if task.ExecutorAcpConnectionRevision != nil {
		return "acp"
	}
	return "provider"
}

func (r *Resolver) ensureRecurrenceDocument(result store.TaskCommandResult) error {
	if result.RecurrenceID == "" {
		return nil
	}
	document, err := home.ReadTaskDocument(r.home, result.Task.ID)
	if err != nil {
		return err
	}
	_, err = home.EnsureRecurrenceDocument(r.home, result.RecurrenceID, document.Content)
	return err
}

func taskScheduleModel(task store.Task) *model.TaskSchedule {
	if task.ScheduledFor == nil {
		return nil
	}
	result := &model.TaskSchedule{ScheduledFor: task.ScheduledFor.Format(time.RFC3339Nano),
		TimeZone: task.ScheduleTimeZone, MissedRunPolicy: missedRunModel(task.MissedRunPolicy)}
	if task.RecurrenceID != "" {
		result.RecurrenceID = &task.RecurrenceID
	}
	if task.RecurrenceRevision != nil {
		value := int(*task.RecurrenceRevision)
		result.RecurrenceRevision = &value
	}
	if task.RecurrenceScheduledFor != nil {
		value := task.RecurrenceScheduledFor.Format(time.RFC3339Nano)
		result.RecurrenceScheduledFor = &value
	}
	return result
}

func missedRunModel(value string) model.MissedRunPolicy {
	if value == string(schedule.MissedRunSkip) {
		return model.MissedRunPolicySkip
	}
	return model.MissedRunPolicyRunOnce
}

func taskDocumentPreview(document string) string {
	runes := []rune(document)
	if len(runes) > taskDocumentPreviewLimit {
		runes = runes[:taskDocumentPreviewLimit]
	}
	return string(runes)
}

func taskStageModel(task store.Task) *model.WorkflowStage {
	state, key := task.State, task.StageKey
	if key == "" {
		key = string(state)
		if state == store.TaskCaptured {
			key = "inbox"
		}
	}
	stage := &model.WorkflowStage{
		StageID:    "stage:personal:" + key,
		WorkflowID: "workflow:personal:default",
		Key:        key,
		Name:       strings.ToUpper(key[:1]) + key[1:],
	}
	switch key {
	case "inbox":
		stage.Name = "Inbox"
		stage.DisplayOrder = 10
		stage.Behavior = model.WorkflowStageBehaviorIntake
	case "queue":
		stage.DisplayOrder = 20
		stage.Behavior = model.WorkflowStageBehaviorDispatch
	case "doing":
		stage.DisplayOrder = 30
		stage.Behavior = model.WorkflowStageBehaviorActive
	case "waiting":
		stage.DisplayOrder = 40
		stage.Behavior = model.WorkflowStageBehaviorHumanGate
	case "done":
		stage.DisplayOrder = 50
		stage.Behavior = model.WorkflowStageBehaviorTerminalSuccess
	case "cancelled":
		stage.DisplayOrder = 60
		stage.Behavior = model.WorkflowStageBehaviorTerminalCancelled
	}
	return stage
}

func validTaskActions(task store.Task) []model.ValidTaskAction {
	switch task.StageKey {
	case "", "inbox":
		if task.ScheduledFor != nil && task.ScheduleProcessedAt == nil {
			return []model.ValidTaskAction{model.ValidTaskActionEdit, model.ValidTaskActionReschedule,
				model.ValidTaskActionUnschedule, model.ValidTaskActionRunNow, model.ValidTaskActionCancel}
		}
		return []model.ValidTaskAction{
			model.ValidTaskActionEdit,
			model.ValidTaskActionQueue,
			model.ValidTaskActionSchedule,
			model.ValidTaskActionCancel,
		}
	case "queue", "doing":
		return []model.ValidTaskAction{model.ValidTaskActionCancel}
	case "done", "cancelled":
		return []model.ValidTaskAction{model.ValidTaskActionReopen}
	case "waiting":
		if task.ActiveGateID != "" {
			return []model.ValidTaskAction{model.ValidTaskActionAnswer, model.ValidTaskActionRetry, model.ValidTaskActionCancel}
		}
		return []model.ValidTaskAction{model.ValidTaskActionCancel}
	default:
		return []model.ValidTaskAction{}
	}
}

func isTerminal(state store.TaskState) bool {
	return state == store.TaskCompleted || state == store.TaskCancelled
}

func parseEventCursor(after *string) (int64, bool, error) {
	if after == nil {
		return 0, false, nil
	}
	cursor, err := store.DecodeWorkEventCursor(*after)
	return cursor, true, err
}

func (r *Resolver) taskEvents(
	ctx context.Context,
	taskID string,
	after *string,
) (<-chan *model.TasksEvent, error) {
	if r.Store == nil {
		return nil, errors.New("GraphQL Task store is unavailable")
	}
	cursor, supplied, err := parseEventCursor(after)
	if err != nil {
		return nil, invalidEventCursorError()
	}
	if _, err := r.Store.Task(ctx, taskID); err != nil {
		return nil, err
	}
	if _, err := home.ReadTaskDocument(r.home, taskID); err != nil {
		return nil, err
	}
	if !supplied {
		cursor, err = r.Store.LatestTaskWorkEventSequence(ctx, taskID)
		if err != nil {
			return nil, err
		}
	}
	wake := r.Store.SubscribeWork(ctx)
	channel := make(chan *model.TasksEvent, 16)
	go func() {
		defer close(channel)
		for {
			events, queryErr := r.Store.WorkEventsForTask(ctx, taskID, cursor, 100)
			if queryErr != nil {
				return
			}
			for _, event := range events {
				current, currentErr := r.Store.Task(ctx, taskID)
				if currentErr != nil {
					return
				}
				currentDocument, currentErr := home.ReadTaskDocument(r.home, taskID)
				if currentErr != nil {
					return
				}
				summary := r.taskSummaryModel(ctx, current, personalWorkspaceID, currentDocument.Content)
				select {
				case channel <- workEventModel(event, summary):
					cursor = event.ID
				case <-ctx.Done():
					return
				}
			}
			if len(events) > 0 {
				continue
			}
			select {
			case <-ctx.Done():
				return
			case _, open := <-wake:
				if !open {
					return
				}
			}
		}
	}()
	return channel, nil
}

func workEventModel(event store.WorkEvent, task *model.TaskSummary) *model.TasksEvent {
	cursor, _ := store.EncodeWorkEventCursor(event.ID)
	result := &model.TasksEvent{
		Cursor:        cursor,
		EventID:       event.EventID,
		Kind:          event.Kind,
		OccurredAt:    event.OccurredAt.Format(time.RFC3339Nano),
		WorkspaceID:   event.WorkspaceID,
		Actor:         event.ActorID,
		CausationID:   event.CausationID,
		CorrelationID: event.CorrelationID,
		Payload:       event.Payload,
		Task:          task,
	}
	if event.ProjectID != "" {
		value := event.ProjectID
		result.ProjectID = &value
	}
	if event.TaskID != "" {
		value := event.TaskID
		result.TaskID = &value
	}
	if event.RunID != "" {
		value := event.RunID
		result.RunID = &value
	}
	return result
}

func (r *Resolver) tasksEvents(
	ctx context.Context,
	workspaceID string,
	after *string,
) (<-chan *model.TasksEvent, error) {
	cursor, supplied, err := parseEventCursor(after)
	if err != nil {
		return nil, invalidEventCursorError()
	}
	if workspaceID != personalWorkspaceID {
		return nil, errors.New("invalid Tasks event query")
	}
	if !supplied {
		cursor, err = r.Store.LatestWorkEventSequence(ctx, workspaceID)
		if err != nil {
			return nil, err
		}
	}
	wake := r.Store.SubscribeWork(ctx)
	channel := make(chan *model.TasksEvent, 16)
	go func() {
		defer close(channel)
		for {
			events, queryErr := r.Store.WorkEvents(ctx, workspaceID, cursor, 100)
			if queryErr != nil {
				return
			}
			for _, event := range events {
				var summary *model.TaskSummary
				if event.TaskID != "" {
					task, taskErr := r.Store.Task(ctx, event.TaskID)
					if taskErr == nil {
						if document, documentErr := home.ReadTaskDocument(r.home, event.TaskID); documentErr == nil {
							summary = r.taskSummaryModel(ctx, task, workspaceID, document.Content)
						}
					}
				}
				select {
				case channel <- workEventModel(event, summary):
					cursor = event.ID
				case <-ctx.Done():
					return
				}
			}
			if len(events) > 0 {
				continue
			}
			select {
			case <-ctx.Done():
				return
			case _, open := <-wake:
				if !open {
					return
				}
			}
		}
	}()
	return channel, nil
}

func invalidEventCursorError() error {
	err := gqlerror.Errorf("the Tasks event cursor is invalid")
	err.Extensions = map[string]any{"code": "invalid_cursor"}
	return err
}
