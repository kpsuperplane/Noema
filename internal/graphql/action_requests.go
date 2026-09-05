package graphql

import (
	"context"
	"errors"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) pendingActionRequests(
	ctx context.Context, conversationID, taskID *string, first *int,
) ([]*model.GovernedAction, error) {
	limit := 50
	if first != nil {
		limit = *first
	}
	if limit < 1 || limit > 100 {
		return nil, errors.New("pendingGovernedActions first must be within 1..100")
	}
	actions, err := r.Store.PendingActionRequests(ctx, "human:local", conversationID, taskID, limit)
	if err != nil {
		return nil, err
	}
	result := make([]*model.GovernedAction, 0, len(actions))
	for _, action := range actions {
		projected, err := r.actionRequestModel(ctx, action)
		if err != nil {
			return nil, err
		}
		result = append(result, projected)
	}
	return result, nil
}

func (r *Resolver) pendingHumanInterventions(
	ctx context.Context, conversationID, taskID, projectID *string, first *int,
) ([]model.HumanIntervention, error) {
	limit := 50
	if first != nil {
		limit = *first
	}
	if limit < 1 || limit > 100 {
		return nil, errors.New("pendingHumanInterventions first must be within 1..100")
	}
	if projectID != nil {
		return []model.HumanIntervention{}, nil
	}
	actions, err := r.pendingActionRequests(ctx, conversationID, taskID, first)
	if err != nil {
		return nil, err
	}
	result := make([]model.HumanIntervention, 0, len(actions))
	for _, action := range actions {
		result = append(result, action)
	}
	mcpRequests, err := r.pendingMCPAuthentications(ctx, conversationID, taskID, first)
	if err != nil {
		return nil, err
	}
	result = append(result, mcpRequests...)
	if r.Adapters != nil {
		adapterRequests, adapterErr := r.pendingAdapterAuthentications(ctx, conversationID, taskID, limit)
		if adapterErr != nil {
			return nil, adapterErr
		}
		result = append(result, adapterRequests...)
	}
	setups, err := r.pendingMCPSetups(ctx, conversationID, first)
	if err != nil {
		return nil, err
	}
	result = append(result, setups...)
	if conversationID != nil && taskID == nil && r.Adapters != nil {
		definitions, definitionErr := r.adapterDefinitions(ctx)
		if definitionErr != nil {
			return nil, definitionErr
		}
		for _, definition := range definitions {
			if adapterDefinitionNeedsChatIntervention(definition) {
				result = append(result, definition)
			}
		}
	}
	if len(result) > limit {
		result = result[:limit]
	}
	return result, nil
}

func adapterDefinitionNeedsChatIntervention(definition *model.AdapterDefinition) bool {
	return !definition.Superseded && (!definition.Reviewed ||
		definition.Reviewed && definition.CredentialSetup != nil && definition.NextAction != nil && definition.NextAction.Kind == "set_up_credential")
}

func (r *Resolver) resolveActionRequest(
	ctx context.Context, input model.ResolveGovernedActionInput,
) (*model.GovernedAction, error) {
	decision := "decline"
	if input.Decision == model.GovernedActionDecisionApprove {
		decision = "approve"
	}
	current, err := r.Store.ActionRequest(ctx, input.ActionID, input.ExpectedRevision)
	if err != nil {
		return nil, err
	}
	var action store.ActionRequest
	if current.TaskID != "" {
		if r.TaskExecution == nil {
			return nil, errors.New("Task execution runtime is unavailable")
		}
		action, err = r.TaskExecution.ResolveActionRequest(ctx, input.ActionID, input.ExpectedRevision, "human:local", decision)
	} else {
		if r.Chat == nil {
			return nil, errors.New("Chat runtime is unavailable")
		}
		action, err = r.Chat.ResolveActionRequest(ctx, input.ActionID, input.ExpectedRevision, "human:local", decision)
	}
	if err != nil {
		return nil, err
	}
	return r.actionRequestModel(ctx, action)
}

func (r *Resolver) actionRequestModel(ctx context.Context, action store.ActionRequest) (*model.GovernedAction, error) {
	result, err := actionRequestModel(action)
	if err != nil || action.TaskID == "" || r.home == nil {
		return result, err
	}
	task, err := r.Store.Task(ctx, action.TaskID)
	if err != nil {
		return nil, err
	}
	document, err := home.ReadTaskDocument(r.home, task.ID)
	if err != nil {
		return nil, err
	}
	summary := r.taskSummaryModel(ctx, task, personalWorkspaceID, document.Content)
	result.Task = &model.TaskCard{TaskID: summary.TaskID, Workspace: summary.Workspace, Project: summary.Project,
		Title: summary.Title, TaskDocumentPreview: summary.TaskDocumentPreview, Stage: summary.Stage,
		Revision: summary.Revision, Generation: summary.Generation, ExecutorAgentID: summary.ExecutorAgentID,
		ExecutorBackend: summary.ExecutorBackend, CwdOverride: summary.CwdOverride, EffectiveCwd: summary.EffectiveCwd,
		EffectiveCwdSource: summary.EffectiveCwdSource, CreatedAt: summary.CreatedAt, UpdatedAt: summary.UpdatedAt,
		Schedule: summary.Schedule, CompletedAt: summary.CompletedAt, CurrentRun: summary.CurrentRun,
		ActiveGate: summary.ActiveGate, ValidActions: summary.ValidActions}
	return result, nil
}

func actionRequestModel(action store.ActionRequest) (*model.GovernedAction, error) {
	state, err := actionStateModel(action.State)
	if err != nil {
		return nil, err
	}
	reviewRoute := model.ExecutionReviewRouteLlmReview
	if action.ReviewRoute == store.ActionHumanReview {
		reviewRoute = model.ExecutionReviewRouteHumanReview
	}
	var conversationID, taskID, runID *string
	if action.ConversationID != "" {
		conversationID = &action.ConversationID
	}
	if action.TaskID != "" {
		taskID, runID = &action.TaskID, &action.RunID
	}
	destination, _ := action.AuthorizationContext["destination"].(map[string]any)
	service, _ := action.AuthorizationContext["service"].(map[string]any)
	serviceName := textField(service, "display_name", "External service")
	target := &model.ActionRequestTarget{ServiceName: actionString(serviceName)}
	if value := textField(destination, "service_id", ""); value != "" {
		target.ServiceID = actionString(value)
	}
	if value := textField(destination, "connection_id", ""); value != "" {
		target.ConnectionID = actionString(value)
	}
	if value := textField(service, "connection_label", ""); value != "" {
		target.ConnectionLabel = actionString(value)
	}
	result := &model.GovernedAction{
		ActionID: action.ID, Revision: action.Revision, ConversationID: conversationID, TaskID: taskID, RunID: runID,
		CapabilityName: action.CapabilityName, ReviewRoute: reviewRoute,
		Behavior: &model.ToolBehavior{
			ReadOnly: action.Behavior.ReadOnly, Idempotent: action.Behavior.RepeatSafe,
			Destructive: action.Behavior.Destructive, OpenWorld: action.Behavior.OpenWorld,
		},
		SafeSummary: action.SafeSummary, Target: target,
		Disclosure: &model.ActionRequestDisclosure{
			Recipient: serviceName, ContentSummary: serviceName + " receives the request data shown in Review details.",
		},
		Consequence: "This can change data outside Noema in " + serviceName + ".", Destination: destination,
		Arguments: action.Arguments, State: state,
	}
	if action.Assessment != nil {
		assessment, err := actionAssessmentModel(*action.Assessment)
		if err != nil {
			return nil, err
		}
		result.Assessment = assessment
	}
	if action.Output != nil {
		result.Output = action.Output
	}
	if action.FailureCode != "" {
		result.FailureCode = actionString(action.FailureCode)
	}
	return result, nil
}

func textField(value map[string]any, key, fallback string) string {
	text, _ := value[key].(string)
	if text == "" {
		return fallback
	}
	return text
}

func actionAssessmentModel(value store.ActionAssessment) (*model.GovernedActionAssessment, error) {
	status := model.GovernedAssessmentStatusCompleted
	switch value.Status {
	case "completed":
	case "reviewer_unavailable":
		status = model.GovernedAssessmentStatusReviewerUnavailable
	case "invalid_response":
		status = model.GovernedAssessmentStatusInvalidResponse
	default:
		return nil, errors.New("stored action assessment status is invalid")
	}
	result := &model.GovernedActionAssessment{Status: status,
		ReasonCodes: append([]string(nil), value.ReasonCodes...), Explanation: value.Explanation}
	if value.Authorization != "" {
		authorizations := map[string]model.GovernedAuthorization{
			"explicit": model.GovernedAuthorizationExplicit, "substantive": model.GovernedAuthorizationSubstantive,
			"weak": model.GovernedAuthorizationWeak, "absent": model.GovernedAuthorizationAbsent,
		}
		authorization, ok := authorizations[value.Authorization]
		if !ok {
			return nil, errors.New("stored action authorization is invalid")
		}
		result.Authorization = &authorization
	}
	if value.Risk != "" {
		risks := map[string]model.GovernedRisk{
			"low": model.GovernedRiskLow, "medium": model.GovernedRiskMedium,
			"high": model.GovernedRiskHigh, "critical": model.GovernedRiskCritical,
		}
		risk, ok := risks[value.Risk]
		if !ok {
			return nil, errors.New("stored action risk is invalid")
		}
		result.Risk = &risk
	}
	return result, nil
}

func actionStateModel(value store.ActionRequestState) (model.GovernedActionState, error) {
	states := map[store.ActionRequestState]model.GovernedActionState{
		store.ActionProposed:         model.GovernedActionStateProposed,
		store.ActionAwaitingApproval: model.GovernedActionStateAwaitingApproval,
		store.ActionExecutable:       model.GovernedActionStateExecutable,
		store.ActionExecuting:        model.GovernedActionStateExecuting,
		store.ActionSucceeded:        model.GovernedActionStateSucceeded,
		store.ActionFailed:           model.GovernedActionStateFailed,
		store.ActionOutcomeUncertain: model.GovernedActionStateOutcomeUncertain,
		store.ActionDeclined:         model.GovernedActionStateDeclined,
		store.ActionSuperseded:       model.GovernedActionStateSuperseded,
		store.ActionCancelled:        model.GovernedActionStateCancelled,
	}
	state, ok := states[value]
	if !ok {
		return "", errors.New("stored action state is invalid")
	}
	return state, nil
}

func actionString(value string) *string { return &value }
