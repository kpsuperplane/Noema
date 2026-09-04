package graphql

import (
	"context"
	"errors"
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
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
	task, err := r.Store.CreateTask(ctx, taskID, input.Title, time.Now())
	if err != nil {
		return nil, errors.Join(err, r.reconcileFailedTaskCreate(taskID))
	}
	if err := home.CommitTaskDocument(r.home, taskID); err != nil {
		return nil, err
	}
	events, err := r.Store.TaskEvents(ctx, task.ID, 0)
	if err != nil {
		return nil, err
	}
	if len(events) != 1 {
		return nil, fmt.Errorf("expected one capture event, got %d", len(events))
	}

	event := taskEventModel(events[0], task, input.WorkspaceID, document.Content)
	r.publishTaskEvent(task.ID, event)
	return &model.TaskCommandPayload{
		Task:             taskDetailModel(task, document),
		EventCursor:      event.Cursor,
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
		StageID:    "stage:personal:" + string(state),
		WorkflowID: "workflow:personal:default",
		Key:        string(state),
		Name:       strings.ToUpper(string(state[:1])) + string(state[1:]),
	}
	switch state {
	case store.TaskCaptured:
		stage.StageID = "stage:personal:inbox"
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

func taskEventModel(
	event store.TaskEvent,
	task store.Task,
	workspaceID string,
	document string,
) *model.TasksEvent {
	taskID := event.TaskID
	cursor := strconv.FormatInt(event.ID, 10)
	return &model.TasksEvent{
		Cursor:        cursor,
		EventID:       cursor,
		Kind:          strings.ReplaceAll(event.Kind, "_", "."),
		OccurredAt:    event.OccurredAt.Format(time.RFC3339Nano),
		WorkspaceID:   workspaceID,
		TaskID:        &taskID,
		Actor:         "local-human",
		CorrelationID: task.ID,
		Payload: map[string]any{
			"revision": event.Revision,
		},
		Task: taskSummaryModel(task, workspaceID, document),
	}
}

func parseEventCursor(after *string) (int64, error) {
	if after == nil || *after == "" {
		return 0, nil
	}
	cursor, err := strconv.ParseInt(*after, 10, 64)
	if err != nil || cursor < 0 {
		return 0, errors.New("invalid Task event cursor")
	}
	return cursor, nil
}

func (r *Resolver) taskEvents(
	ctx context.Context,
	taskID string,
	after *string,
) (<-chan *model.TasksEvent, error) {
	if r.Store == nil {
		return nil, errors.New("GraphQL Task store is unavailable")
	}
	cursor, err := parseEventCursor(after)
	if err != nil {
		return nil, err
	}

	r.subscriptionsMu.Lock()
	task, taskErr := r.Store.Task(ctx, taskID)
	var document home.TaskDocument
	if taskErr == nil {
		document, taskErr = home.ReadTaskDocument(r.home, taskID)
	}
	var events []store.TaskEvent
	if taskErr == nil {
		events, taskErr = r.Store.TaskEvents(ctx, taskID, cursor)
	}
	if taskErr != nil {
		r.subscriptionsMu.Unlock()
		return nil, taskErr
	}

	channel := make(chan *model.TasksEvent, len(events)+16)
	if r.subscriptions == nil {
		r.subscriptions = make(map[string]map[chan *model.TasksEvent]struct{})
	}
	if r.subscriptions[taskID] == nil {
		r.subscriptions[taskID] = make(map[chan *model.TasksEvent]struct{})
	}
	r.subscriptions[taskID][channel] = struct{}{}
	for _, event := range events {
		channel <- taskEventModel(event, task, personalWorkspaceID, document.Content)
	}
	r.subscriptionsMu.Unlock()

	go func() {
		<-ctx.Done()
		r.subscriptionsMu.Lock()
		delete(r.subscriptions[taskID], channel)
		if len(r.subscriptions[taskID]) == 0 {
			delete(r.subscriptions, taskID)
		}
		r.subscriptionsMu.Unlock()
		close(channel)
	}()
	return channel, nil
}

func (r *Resolver) publishTaskEvent(taskID string, event *model.TasksEvent) {
	r.subscriptionsMu.Lock()
	defer r.subscriptionsMu.Unlock()
	for channel := range r.subscriptions[taskID] {
		select {
		case channel <- event:
		default:
		}
	}
}
