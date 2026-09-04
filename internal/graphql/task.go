package graphql

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
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
	if input.WorkspaceID != personalWorkspaceID {
		return nil, errors.New("this migration slice supports only workspace:personal")
	}
	if input.ProjectID != nil || input.Schedule != nil ||
		(input.ExecutorAgentID != nil && *input.ExecutorAgentID != "agent:task-executor") ||
		input.CwdOverride != nil {
		return nil, errors.New("this migration slice does not support Task placement or scheduling")
	}
	if input.ClientMutationID == "" {
		return nil, errors.New("clientMutationId cannot be empty")
	}

	taskID, err := store.NewTaskID()
	if err != nil {
		return nil, err
	}
	document, err := home.CreatePendingTaskDocument(r.home, taskID, input.TaskDocument)
	if err != nil {
		return nil, err
	}
	task, err := r.Store.CreateTask(ctx, taskID, input.Title,
		"correlation:graphql:"+input.ClientMutationID, time.Now())
	if err != nil {
		return nil, errors.Join(err, r.reconcileFailedTaskCreate(taskID))
	}
	if err := home.CommitTaskDocument(r.home, taskID); err != nil {
		return nil, err
	}
	workEvent, err := r.Store.LatestTaskWorkEvent(ctx, task.ID)
	if err != nil {
		return nil, err
	}
	cursor, err := store.EncodeWorkEventCursor(workEvent.ID)
	if err != nil {
		return nil, err
	}
	return &model.TaskCommandPayload{
		Task:             taskDetailModel(task, document),
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
		Stage:                    taskStageModel(task.State),
		Revision:                 int(task.Revision),
		Generation:               1,
		ExecutorAgentID:          "agent:task-executor",
		ExecutorBackend:          "go",
		EffectiveCwdSource:       "default",
		CreatedAt:                task.CreatedAt.Format(time.RFC3339Nano),
		UpdatedAt:                task.UpdatedAt.Format(time.RFC3339Nano),
		Source:                   &model.TaskSource{},
		Messages:                 []*model.TaskMessage{},
		Runs:                     []*model.TaskRun{},
		ContributorInstanceNames: []string{},
		ValidActions:             validTaskActions(task.State),
	}
	if isTerminal(task.State) {
		completedAt := detail.UpdatedAt
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
	return taskDetailModel(task, document), nil
}

func taskSummaryModel(task store.Task, workspaceID string, document string) *model.TaskSummary {
	return &model.TaskSummary{
		TaskID: task.ID,
		Workspace: &model.Workspace{
			WorkspaceID: workspaceID,
			Name:        "Personal",
			Description: "",
			IsPersonal:  workspaceID == personalWorkspaceID,
		},
		Title:               task.Title,
		TaskDocumentPreview: taskDocumentPreview(document),
		Stage:               taskStageModel(task.State),
		Revision:            int(task.Revision),
		Generation:          1,
		ExecutorAgentID:     "agent:task-executor",
		ExecutorBackend:     "go",
		EffectiveCwdSource:  "default",
		CreatedAt:           task.CreatedAt.Format(time.RFC3339Nano),
		UpdatedAt:           task.UpdatedAt.Format(time.RFC3339Nano),
		ValidActions:        validTaskActions(task.State),
	}
}

func taskDocumentPreview(document string) string {
	runes := []rune(document)
	if len(runes) > taskDocumentPreviewLimit {
		runes = runes[:taskDocumentPreviewLimit]
	}
	return string(runes)
}

func taskStageModel(state store.TaskState) *model.WorkflowStage {
	stage := &model.WorkflowStage{
		StageID:    store.TaskStageID(state),
		WorkflowID: "workflow:personal:default",
		Key:        string(state),
		Name:       strings.ToUpper(string(state[:1])) + string(state[1:]),
	}
	switch state {
	case store.TaskCaptured:
		stage.Key = "inbox"
		stage.Name = "Inbox"
		stage.Behavior = model.WorkflowStageBehaviorIntake
	case store.TaskRunning:
		stage.DisplayOrder = 1
		stage.Behavior = model.WorkflowStageBehaviorActive
	case store.TaskCompleted:
		stage.DisplayOrder = 2
		stage.Behavior = model.WorkflowStageBehaviorTerminalSuccess
	case store.TaskCancelled:
		stage.DisplayOrder = 3
		stage.Behavior = model.WorkflowStageBehaviorTerminalCancelled
	case store.TaskFailed:
		stage.DisplayOrder = 4
		stage.Behavior = model.WorkflowStageBehaviorHumanGate
	}
	return stage
}

func validTaskActions(state store.TaskState) []model.ValidTaskAction {
	switch state {
	case store.TaskCaptured:
		return []model.ValidTaskAction{
			model.ValidTaskActionEdit,
			model.ValidTaskActionQueue,
			model.ValidTaskActionSchedule,
			model.ValidTaskActionCancel,
		}
	case store.TaskRunning:
		return []model.ValidTaskAction{model.ValidTaskActionCancel}
	case store.TaskCompleted, store.TaskCancelled:
		return []model.ValidTaskAction{model.ValidTaskActionReopen}
	case store.TaskFailed:
		return []model.ValidTaskAction{model.ValidTaskActionRetry, model.ValidTaskActionCancel}
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
	task, taskErr := r.Store.Task(ctx, taskID)
	var document home.TaskDocument
	if taskErr == nil {
		document, taskErr = home.ReadTaskDocument(r.home, taskID)
	}
	if taskErr != nil {
		return nil, taskErr
	}
	if !supplied {
		cursor, err = r.Store.LatestTaskWorkEventSequence(ctx, taskID)
		if err != nil {
			return nil, err
		}
	}
	wake := r.Store.SubscribeWork(ctx)
	channel := make(chan *model.TasksEvent, 16)
	summary := taskSummaryModel(task, personalWorkspaceID, document.Content)
	go func() {
		defer close(channel)
		for {
			events, queryErr := r.Store.WorkEventsForTask(ctx, taskID, cursor, 100)
			if queryErr != nil {
				return
			}
			for _, event := range events {
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
							summary = taskSummaryModel(task, workspaceID, document.Content)
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
