package graphql

import (
	"context"
	"errors"

	"github.com/kpsuperplane/noema/internal/graphql/model"
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
		projected, err := actionRequestModel(action)
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
	mcpRequests, err := r.pendingMCPAuthentications(ctx, conversationID, first)
	if err != nil {
		return nil, err
	}
	result = append(result, mcpRequests...)
	setups, err := r.pendingMCPSetups(ctx, conversationID, first)
	if err != nil {
		return nil, err
	}
	result = append(result, setups...)
	return result, nil
}

func (r *Resolver) resolveActionRequest(
	ctx context.Context, input model.ResolveGovernedActionInput,
) (*model.GovernedAction, error) {
	if r.Chat == nil {
		return nil, errors.New("chat runtime is unavailable")
	}
	decision := "decline"
	if input.Decision == model.GovernedActionDecisionApprove {
		decision = "approve"
	}
	action, err := r.Chat.ResolveActionRequest(
		ctx, input.ActionID, input.ExpectedRevision, "human:local", decision,
	)
	if err != nil {
		return nil, err
	}
	return actionRequestModel(action)
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
	conversationID := action.ConversationID
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
		ActionID: action.ID, Revision: action.Revision, ConversationID: &conversationID,
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
