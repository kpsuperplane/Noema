package graphql

import (
	"context"
	"errors"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/vektah/gqlparser/v2/gqlerror"
)

func (r *Resolver) updateInboxTask(ctx context.Context, input model.UpdateInboxTaskInput) (*model.TaskCommandPayload, error) {
	r.taskMu.Lock()
	defer r.taskMu.Unlock()
	if input.ProjectID != nil && input.ClearProject != nil && *input.ClearProject || input.CwdOverride != nil && input.ClearCwdOverride != nil && *input.ClearCwdOverride {
		return nil, taskInputError("replacement fields conflict")
	}
	changes := store.TaskUpdate{Title: input.Title, ProjectID: input.ProjectID, ExecutorAgentID: input.ExecutorAgentID, CwdOverride: input.CwdOverride}
	changes.SetProject = input.ProjectID != nil || input.ClearProject != nil && *input.ClearProject
	changes.SetCwd = input.CwdOverride != nil || input.ClearCwdOverride != nil && *input.ClearCwdOverride
	if changes.Title == nil && !changes.SetProject && changes.ExecutorAgentID == nil && !changes.SetCwd && input.TaskDocument == nil {
		return nil, taskInputError("the update is empty")
	}
	command, err := newTaskCommand("update_inbox_task", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	if replay, found, lookupErr := r.Store.LookupTaskCommandReceipt(ctx, command); lookupErr != nil || found {
		return r.replayTaskLifecycle(ctx, replay, found, input.ClientMutationID, command.RequestDigest, lookupErr)
	}
	var stage *home.TaskDocumentStage
	if input.TaskDocument != nil {
		if input.ExpectedTaskDocumentDigest == nil {
			return nil, taskInputError("the Task document digest is required")
		}
		prepared, prepareErr := home.PrepareTaskDocumentReplace(r.home, input.TaskID, *input.ExpectedTaskDocumentDigest, *input.TaskDocument, command.RequestDigest)
		if prepareErr != nil {
			return nil, taskInputError("the Task document replacement is invalid")
		}
		stage = &prepared
		changes.DocumentDigest = prepared.Document.Digest
	}
	result, err := r.Store.UpdateInboxTask(ctx, input.TaskID, int64(input.ExpectedRevision), int64(input.ExpectedGeneration), changes, command, time.Now())
	if err != nil {
		if stage != nil {
			if recovered, committed, recoveryErr := r.reconcileTaskDocumentCommand(ctx, command, *stage, err); committed {
				if recoveryErr != nil {
					return nil, taskLifecycleError(recoveryErr)
				}
				return r.taskCommandPayload(ctx, recovered, input.ClientMutationID)
			}
		}
		return nil, taskLifecycleError(err)
	}
	if stage != nil {
		if _, err = home.CommitTaskDocumentStage(r.home, *stage); err != nil {
			return nil, taskLifecycleError(err)
		}
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) queueTask(ctx context.Context, input model.QueueTaskInput) (*model.TaskCommandPayload, error) {
	r.taskMu.Lock()
	defer r.taskMu.Unlock()
	command, err := newTaskCommand("queue_task", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	result, err := r.Store.QueueTask(ctx, input.TaskID, int64(input.ExpectedRevision), int64(input.ExpectedGeneration), command, time.Now())
	if err != nil {
		return nil, taskLifecycleError(err)
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) answerTask(ctx context.Context, input model.AnswerTaskInput) (*model.TaskCommandPayload, error) {
	r.taskMu.Lock()
	defer r.taskMu.Unlock()
	answer := strings.TrimSpace(input.AnswerMarkdown)
	if answer == "" || len(answer) > 64<<10 {
		return nil, taskInputError("the answer is invalid")
	}
	gate, err := r.Store.TaskGate(ctx, input.GateID)
	if err != nil {
		return nil, taskLifecycleError(err)
	}
	if gate.Kind == "approval" && input.ApprovalDecision == nil {
		return nil, taskInputError("the approval decision is required")
	}
	if gate.Kind != "approval" && input.ApprovalDecision != nil || !gate.AllowsResolution("answer") {
		return nil, taskLifecycleError(store.ErrInvalidTransition)
	}
	command, err := newTaskCommand("answer_task", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	var approval *string
	if input.ApprovalDecision != nil {
		value := strings.ToLower(string(*input.ApprovalDecision))
		approval = &value
	}
	result, err := r.Store.ResolveTaskGate(ctx, input.TaskID, input.GateID, int64(input.ExpectedRevision), int64(input.ExpectedGeneration), answer, "answer", approval, command, time.Now())
	if err != nil {
		return nil, taskLifecycleError(err)
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) retryTask(ctx context.Context, input model.RetryTaskInput) (*model.TaskCommandPayload, error) {
	r.taskMu.Lock()
	defer r.taskMu.Unlock()
	note := "Retry requested."
	if input.RetryNote != nil {
		note = strings.TrimSpace(*input.RetryNote)
		if note == "" || len(note) > 64<<10 {
			return nil, taskInputError("the retry note is invalid")
		}
	}
	gate, err := r.Store.TaskGate(ctx, input.GateID)
	if err != nil {
		return nil, taskLifecycleError(err)
	}
	if !gate.AllowsResolution("retry") {
		return nil, taskLifecycleError(store.ErrInvalidTransition)
	}
	command, err := newTaskCommand("retry_task", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	result, err := r.Store.ResolveTaskGate(ctx, input.TaskID, input.GateID, int64(input.ExpectedRevision), int64(input.ExpectedGeneration), note, "retry", nil, command, time.Now())
	if err != nil {
		return nil, taskLifecycleError(err)
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) cancelTask(ctx context.Context, input model.CancelTaskInput) (*model.TaskCommandPayload, error) {
	r.taskMu.Lock()
	defer r.taskMu.Unlock()
	reason := ""
	if input.Reason != nil {
		reason = strings.TrimSpace(*input.Reason)
		if reason == "" {
			input.Reason = nil
		} else {
			input.Reason = &reason
		}
	}
	command, err := newTaskCommand("cancel_task", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	result, err := r.Store.CancelTask(ctx, input.TaskID, int64(input.ExpectedRevision), int64(input.ExpectedGeneration), reason, command, time.Now())
	if err != nil {
		return nil, taskLifecycleError(err)
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) reopenTask(ctx context.Context, input model.ReopenTaskInput) (*model.TaskCommandPayload, error) {
	r.taskMu.Lock()
	defer r.taskMu.Unlock()
	feedback := strings.TrimSpace(input.FeedbackMarkdown)
	if feedback == "" || len(feedback) > 64<<10 {
		return nil, taskInputError("the reopen direction is invalid")
	}
	direction := feedback
	var replacement *string
	if input.RequestMarkdown != nil {
		value := strings.TrimSpace(*input.RequestMarkdown)
		if value == "" {
			return nil, taskInputError("the replacement request is invalid")
		}
		replacement = &value
		direction += "\n\nUpdated request:\n\n" + value
	}
	command, err := newTaskCommand("reopen_task", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	if replay, found, lookupErr := r.Store.LookupTaskCommandReceipt(ctx, command); lookupErr != nil || found {
		return r.replayTaskLifecycle(ctx, replay, found, input.ClientMutationID, command.RequestDigest, lookupErr)
	}
	var stage *home.TaskDocumentStage
	documentDigest := ""
	if replacement != nil {
		current, readErr := home.ReadTaskDocument(r.home, input.TaskID)
		if readErr != nil {
			return nil, taskLifecycleError(readErr)
		}
		prepared, prepareErr := home.PrepareTaskDocumentReplace(r.home, input.TaskID, current.Digest, *replacement, command.RequestDigest)
		if prepareErr != nil {
			return nil, taskInputError("the Task document replacement is invalid")
		}
		stage = &prepared
		documentDigest = prepared.Document.Digest
	}
	var complexity *string
	if input.Complexity != nil {
		value := strings.ToLower(string(*input.Complexity))
		complexity = &value
	}
	result, err := r.Store.ReopenTask(ctx, input.TaskID, int64(input.ExpectedRevision), int64(input.ExpectedGeneration), direction, complexity, documentDigest, command, time.Now())
	if err != nil {
		if stage != nil {
			if recovered, committed, recoveryErr := r.reconcileTaskDocumentCommand(ctx, command, *stage, err); committed {
				if recoveryErr != nil {
					return nil, taskLifecycleError(recoveryErr)
				}
				return r.taskCommandPayload(ctx, recovered, input.ClientMutationID)
			}
		}
		return nil, taskLifecycleError(err)
	}
	if stage != nil {
		if _, err = home.CommitTaskDocumentStage(r.home, *stage); err != nil {
			return nil, taskLifecycleError(err)
		}
	}
	return r.taskCommandPayload(ctx, result, input.ClientMutationID)
}

func (r *Resolver) reconcileTaskDocumentCommand(ctx context.Context, command store.TaskCommand, stage home.TaskDocumentStage, commandErr error) (store.TaskCommandResult, bool, error) {
	receipt, found, err := r.Store.LookupTaskCommandReceipt(ctx, command)
	if err != nil || !found {
		_ = home.DiscardTaskDocumentStage(r.home, stage.TaskID, stage.RequestDigest)
		return store.TaskCommandResult{}, false, commandErr
	}
	if _, err = home.RecoverTaskDocumentStage(r.home, receipt.Task.ID, command.RequestDigest, receipt.DocumentDigest); err != nil {
		return store.TaskCommandResult{}, true, err
	}
	return receipt, true, nil
}

func (r *Resolver) replayTaskLifecycle(ctx context.Context, result store.TaskCommandResult, found bool, clientID, requestDigest string, err error) (*model.TaskCommandPayload, error) {
	if err != nil {
		return nil, taskLifecycleError(err)
	}
	if !found {
		return nil, taskLifecycleError(errors.New("Task receipt is unavailable"))
	}
	if result.DocumentDigest != "" {
		if _, err = home.RecoverTaskDocumentStage(r.home, result.Task.ID, requestDigest, result.DocumentDigest); errors.Is(err, home.ErrTaskDocumentStageStale) {
			_ = home.DiscardTaskDocumentStage(r.home, result.Task.ID, requestDigest)
		} else if err != nil {
			return nil, taskLifecycleError(err)
		}
	}
	return r.taskCommandPayload(ctx, result, clientID)
}

func taskLifecycleError(err error) error {
	if err == nil {
		return nil
	}
	code, message := "work_unavailable", "Task authority is unavailable"
	switch {
	case errors.Is(err, store.ErrStaleRevision):
		code, message = "stale_revision", "the authoritative Task revision is stale"
	case errors.Is(err, store.ErrInvalidTransition):
		code, message = "invalid_transition", "the requested Task action is not valid now"
	case errors.Is(err, store.ErrCommandConflict):
		code, message = "idempotency_conflict", "idempotency key conflicts"
	case errors.Is(err, store.ErrTaskNotFound):
		code, message = "task_unavailable", "task is unavailable"
	case errors.Is(err, store.ErrInvalidCursor):
		code, message = "invalid_cursor", "invalid task cursor"
	}
	result := gqlerror.Errorf("%s", message)
	result.Extensions = map[string]any{"code": code}
	return result
}

func taskInputError(message string) error {
	result := gqlerror.Errorf("%s", message)
	result.Extensions = map[string]any{"code": "invalid_input"}
	return result
}

func (r *Resolver) tasksOverview(ctx context.Context, workspace string, project *string) (*model.TasksOverview, error) {
	if workspace != personalWorkspaceID {
		return nil, taskInputError("the workspace is invalid")
	}
	filter := store.TaskListFilter{Scope: "active"}
	if project != nil {
		filter.ProjectID = *project
	}
	page, counts, needs, err := r.Store.TaskOverview(ctx, filter.ProjectID, 50)
	if err != nil {
		return nil, taskLifecycleError(err)
	}
	connection, err := r.taskConnection(ctx, page)
	if err != nil {
		return nil, err
	}
	stages := personalWorkflowStages()
	columns := []*model.TaskStageColumn{}
	for _, stage := range stages {
		if stage.Behavior == model.WorkflowStageBehaviorTerminalSuccess || stage.Behavior == model.WorkflowStageBehaviorTerminalCancelled {
			continue
		}
		columns = append(columns, &model.TaskStageColumn{Stage: stage, TaskCount: counts[stage.Key]})
	}
	return &model.TasksOverview{Workspace: personalWorkspace(), Workflow: &model.Workflow{WorkflowID: "workflow:personal:default", Name: "Personal", Stages: stages}, BoardColumns: columns, RecentTasks: connection, NeedsYouCount: needs}, nil
}

func (r *Resolver) tasks(ctx context.Context, input model.TaskListInput, first *int, after *string) (*model.TaskConnection, error) {
	if input.WorkspaceID != personalWorkspaceID {
		return nil, taskInputError("the workspace is invalid")
	}
	limit, err := taskPageSize(first)
	if err != nil {
		return nil, err
	}
	filter := store.TaskListFilter{Scope: strings.ToLower(string(input.Scope)), AttentionOnly: input.AttentionOnly}
	if input.Scope == model.TaskScopeActive {
		for _, behavior := range input.StageBehaviors {
			if behavior == model.WorkflowStageBehaviorTerminalSuccess || behavior == model.WorkflowStageBehaviorTerminalCancelled {
				result := gqlerror.Errorf("workflow filter is inconsistent")
				result.Extensions = map[string]any{"code": "workflow_mismatch"}
				return nil, result
			}
		}
	}
	if input.ProjectID != nil {
		filter.ProjectID = *input.ProjectID
	}
	for _, id := range input.StageIds {
		key, ok := strings.CutPrefix(id, "stage:personal:")
		if !ok {
			return nil, taskInputError("the stage is invalid")
		}
		filter.StageKeys = append(filter.StageKeys, key)
	}
	for _, behavior := range input.StageBehaviors {
		filter.StageKeys = append(filter.StageKeys, stageKeysForBehavior(behavior)...)
	}
	page, err := r.Store.ListTasks(ctx, filter, limit, after)
	if err != nil {
		return nil, taskLifecycleError(err)
	}
	return r.taskConnection(ctx, page)
}

func (r *Resolver) needsYou(ctx context.Context, workspace string, project *string, first *int, after *string) (*model.TaskAttentionConnection, error) {
	tasks, err := r.tasks(ctx, model.TaskListInput{
		WorkspaceID: workspace, ProjectID: project, AttentionOnly: true, Scope: model.TaskScopeActive,
	}, first, after)
	if err != nil {
		return nil, err
	}
	result := &model.TaskAttentionConnection{PageInfo: tasks.PageInfo}
	for _, edge := range tasks.Edges {
		if edge.Node.Attention == nil {
			return nil, errors.New("Task attention is unavailable")
		}
		result.Edges = append(result.Edges, &model.TaskAttentionEdge{Cursor: edge.Cursor, Node: edge.Node.Attention})
	}
	return result, nil
}

func (r *Resolver) taskHistory(ctx context.Context, workspace string, project *string, kind *model.TerminalTaskKind, first *int, after *string) (*model.TaskConnection, error) {
	input := model.TaskListInput{WorkspaceID: workspace, ProjectID: project, Scope: model.TaskScopeTerminal}
	if kind != nil {
		switch *kind {
		case model.TerminalTaskKindCompleted:
			input.StageIds = []string{"stage:personal:done"}
		case model.TerminalTaskKindCancelled:
			input.StageIds = []string{"stage:personal:cancelled"}
		}
	}
	return r.tasks(ctx, input, first, after)
}

func (r *Resolver) taskRunItems(ctx context.Context, runID string, first *int, after *string) (*model.TaskRunItemConnection, error) {
	limit, err := taskPageSize(first)
	if err != nil {
		return nil, err
	}
	page, err := r.Store.TaskRunItems(ctx, runID, limit, after)
	if err != nil {
		return nil, taskLifecycleError(err)
	}
	result := &model.TaskRunItemConnection{PageInfo: &model.PageInfo{EndCursor: page.EndCursor, HasNextPage: page.HasNextPage}}
	for i, item := range page.Items {
		node := taskRunItemModel(item)
		node.Cursor = page.Cursors[i]
		result.Edges = append(result.Edges, &model.TaskRunItemEdge{Cursor: page.Cursors[i], Node: node})
	}
	return result, nil
}

func (r *Resolver) taskConnection(ctx context.Context, page store.TaskPage) (*model.TaskConnection, error) {
	result := &model.TaskConnection{PageInfo: &model.PageInfo{EndCursor: page.EndCursor, HasNextPage: page.HasNextPage}}
	for i, task := range page.Tasks {
		document, err := home.ReadTaskDocument(r.home, task.ID)
		if err != nil {
			return nil, err
		}
		result.Edges = append(result.Edges, &model.TaskEdge{Cursor: page.Cursors[i], Node: r.taskSummaryModel(ctx, task, personalWorkspaceID, document.Content)})
	}
	return result, nil
}
func taskPageSize(first *int) (int, error) {
	if first == nil {
		return 50, nil
	}
	if *first < 1 || *first > 100 {
		return 0, taskInputError("the page size is invalid")
	}
	return *first, nil
}
func personalWorkspace() *model.Workspace {
	return &model.Workspace{WorkspaceID: personalWorkspaceID, Name: "Personal", Description: "", IsPersonal: true}
}
func personalWorkflowStages() []*model.WorkflowStage {
	keys := []string{"inbox", "queue", "doing", "waiting", "done", "cancelled"}
	result := make([]*model.WorkflowStage, len(keys))
	for i, key := range keys {
		result[i] = taskStageModel(store.Task{State: store.TaskCaptured, StageKey: key})
		result[i].DisplayOrder = (i + 1) * 10
	}
	return result
}
func stageKeysForBehavior(value model.WorkflowStageBehavior) []string {
	switch value {
	case model.WorkflowStageBehaviorIntake:
		return []string{"inbox"}
	case model.WorkflowStageBehaviorDispatch:
		return []string{"queue"}
	case model.WorkflowStageBehaviorActive:
		return []string{"doing"}
	case model.WorkflowStageBehaviorHumanGate:
		return []string{"waiting"}
	case model.WorkflowStageBehaviorTerminalSuccess:
		return []string{"done"}
	case model.WorkflowStageBehaviorTerminalCancelled:
		return []string{"cancelled"}
	}
	return nil
}

func (r *Resolver) hydrateTaskSummary(ctx context.Context, task store.Task, result *model.TaskSummary) {
	if task.CurrentRunID != "" {
		runs, err := r.Store.TaskRuns(ctx, task.ID, 50)
		if err == nil {
			for _, run := range runs {
				if run.ID == task.CurrentRunID {
					result.CurrentRun = currentRunModel(run)
					break
				}
			}
		}
	}
	if task.ActiveGateID != "" {
		if gate, err := r.Store.TaskGate(ctx, task.ActiveGateID); err == nil {
			result.ActiveGate = taskGateModel(gate)
			result.ValidActions = validTaskActions(task, &gate)
			result.Attention = taskAttentionModel(taskCardFromSummary(result), &gate)
		}
	}
}

func taskAttentionModel(task *model.TaskCard, gate *store.TaskGate) *model.TaskAttention {
	var kind model.TaskAttentionKind
	title := ""
	switch gate.Kind {
	case "clarification":
		kind, title = model.TaskAttentionKindClarificationRequired, "Clarification required"
	case "approval":
		kind, title = model.TaskAttentionKindApprovalRequired, "Approval required"
	case "recovery":
		kind, title = model.TaskAttentionKindRecoveryRequired, "Recovery decision required"
	default:
		return nil
	}
	return &model.TaskAttention{Kind: kind, Title: title, Summary: runePreview(gate.Prompt, 400),
		Gate: taskGateModel(*gate), Task: task, ValidActions: append([]model.ValidTaskAction(nil), task.ValidActions...)}
}

func taskCardFromSummary(value *model.TaskSummary) *model.TaskCard {
	return &model.TaskCard{TaskID: value.TaskID, Workspace: value.Workspace, Project: value.Project,
		Title: value.Title, TaskDocumentPreview: value.TaskDocumentPreview, Stage: value.Stage,
		Revision: value.Revision, Generation: value.Generation, ExecutorAgentID: value.ExecutorAgentID,
		ExecutorBackend: value.ExecutorBackend, CwdOverride: value.CwdOverride, EffectiveCwd: value.EffectiveCwd,
		EffectiveCwdSource: value.EffectiveCwdSource, CreatedAt: value.CreatedAt, UpdatedAt: value.UpdatedAt,
		Schedule: value.Schedule, CompletedAt: value.CompletedAt, CurrentRun: value.CurrentRun,
		ActiveGate: value.ActiveGate, ValidActions: append([]model.ValidTaskAction(nil), value.ValidActions...)}
}

func runePreview(value string, limit int) string {
	runes := []rune(value)
	if len(runes) > limit {
		runes = runes[:limit]
	}
	return string(runes)
}
func currentRunModel(run store.TaskRun) *model.CurrentRunSummary {
	return &model.CurrentRunSummary{RunID: run.ID, InstanceName: run.InstanceName, Kind: model.TaskRunKind(strings.ToUpper(run.Kind)), Status: model.TaskRunStatus(strings.ToUpper(run.Status)), AttemptIndex: int(run.AttemptIndex), QueuedAt: run.QueuedAt.Format(time.RFC3339Nano), StartedAt: formatOptionalTime(run.StartedAt), UpdatedAt: run.UpdatedAt.Format(time.RFC3339Nano), ActivityLabel: strings.ToUpper(run.Status[:1]) + run.Status[1:]}
}
func taskRunModel(run store.TaskRun) *model.TaskRun {
	return &model.TaskRun{RunID: run.ID, InstanceName: run.InstanceName, Kind: model.TaskRunKind(strings.ToUpper(run.Kind)), Status: model.TaskRunStatus(strings.ToUpper(run.Status)), AgentID: run.AgentID, TaskGeneration: int(run.Generation), AttemptIndex: int(run.AttemptIndex), ReviewRound: int(run.ReviewRound), ParentRunID: stringPointer(run.ParentRunID), Model: &model.TaskModelSnapshot{ProviderKind: run.ProviderKind, ProviderAccountID: run.ProviderAccountID, SelectionMode: model.ProviderSelectionMode(strings.ToUpper(run.SelectionMode)), ModelProfile: run.ModelProfile, ReasoningEffort: reasoningEffort(run.ReasoningEffort)}, ExecutorBackend: run.ExecutorBackend, ExecutorAgentID: run.ExecutorAgentID, EffectiveCwd: run.EffectiveCwd, AcpSessionID: run.AcpSessionID, ExecutionPolicy: taskExecutionPolicyModel(run.ExecutionPolicy), ErrorCode: run.ErrorCode, ErrorMessage: run.ErrorMessage, ProviderCallCount: int(run.ProviderCallCount), ToolCallCount: int(run.ToolCallCount), InputTokens: int(run.InputTokens), CachedInputTokens: int(run.CachedInputTokens), OutputTokens: int(run.OutputTokens), ActiveMilliseconds: int(run.ActiveMilliseconds), QueuedAt: run.QueuedAt.Format(time.RFC3339Nano), StartedAt: formatOptionalTime(run.StartedAt), EndedAt: formatOptionalTime(run.EndedAt), CreatedAt: run.CreatedAt.Format(time.RFC3339Nano), UpdatedAt: run.UpdatedAt.Format(time.RFC3339Nano)}
}
func taskGateModel(gate store.TaskGate) *model.TaskGate {
	return &model.TaskGate{GateID: gate.ID, TaskGeneration: int(gate.Generation), Kind: model.TaskGateKind(strings.ToUpper(gate.Kind)), State: model.TaskGateState(strings.ToUpper(gate.State)), RecoveryReason: recoveryReason(gate.RecoveryReason), RetryRunKind: runKind(gate.RetryRunKind), Prompt: gate.Prompt, ContextMarkdown: gate.Context, SuggestedAnswers: gate.SuggestedAnswers, OpenedBy: gate.OpenedBy, OriginatingRunID: gate.OriginatingRunID, OpenedAt: gate.OpenedAt.Format(time.RFC3339Nano), ResolvedBy: gate.ResolvedBy, ResolvedAt: formatOptionalTime(gate.ResolvedAt), Resolution: gate.ResolutionMessageID}
}
func taskMessageModel(value store.TaskMessage) *model.TaskMessage {
	return &model.TaskMessage{MessageID: value.ID, BodyMarkdown: value.Body, Author: value.Author, CreatedAt: value.CreatedAt.Format(time.RFC3339Nano)}
}
func taskRunItemModel(v store.TaskRunItem) *model.TaskRunItem {
	return &model.TaskRunItem{ItemID: v.ID, RunID: v.RunID, SequenceIndex: int(v.Sequence), RoundIndex: int(v.Round), Kind: model.TaskRunItemKind(strings.ToUpper(v.Kind)), Status: model.TaskRunItemStatus(strings.ToUpper(v.Status)), CorrelationID: v.CorrelationID, ParentItemID: v.ParentID, ContentText: v.Content, Payload: v.Payload, CreatedAt: v.CreatedAt.Format(time.RFC3339Nano), UpdatedAt: v.UpdatedAt.Format(time.RFC3339Nano)}
}
func formatOptionalTime(value *time.Time) *string {
	if value == nil {
		return nil
	}
	text := value.Format(time.RFC3339Nano)
	return &text
}
func stringPointer(value string) *string {
	if value == "" {
		return nil
	}
	return &value
}
func reasoningEffort(value *string) *model.ReasoningEffort {
	if value == nil || *value == "" {
		return nil
	}
	result := model.ReasoningEffort(strings.ToUpper(*value))
	return &result
}
func recoveryReason(value *string) *model.TaskRecoveryReason {
	if value == nil {
		return nil
	}
	result := model.TaskRecoveryReason(strings.ToUpper(*value))
	return &result
}
func runKind(value *string) *model.TaskRunKind {
	if value == nil {
		return nil
	}
	result := model.TaskRunKind(strings.ToUpper(*value))
	return &result
}
