package graphql

import (
	"context"
	"errors"
	"sort"
	"strings"

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
	if taskID != nil {
		task, err := r.Store.Task(ctx, *taskID)
		if err != nil {
			return nil, err
		}
		if task.ActiveGateID != "" {
			document, err := home.ReadTaskDocument(r.home, task.ID)
			if err != nil {
				return nil, err
			}
			summary := r.taskSummaryModel(ctx, task, personalWorkspaceID, document.Content)
			if summary.Attention != nil {
				result = append(result, summary.Attention)
			}
		}
	}

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
		oauth, oauthErr := r.adapterOauthState(ctx)
		if oauthErr != nil {
			return nil, oauthErr
		}
		reviews, accountSetups, clientSetups := projectAdapterInterventions(definitions, oauth)
		for _, definition := range reviews {
			result = append(result, definition)
		}
		for _, setup := range accountSetups {
			result = append(result, setup)
		}
		for _, setup := range clientSetups {
			result = append(result, setup)
		}
	}
	if len(result) > limit {
		result = result[:limit]
	}
	return result, nil
}

func projectAdapterInterventions(
	definitions []*model.AdapterDefinition,
	oauth *model.AdapterOauthState,
) ([]model.HumanIntervention, []model.HumanIntervention, []model.HumanIntervention) {
	if oauth == nil {
		return nil, nil, nil
	}
	profiles := make(map[string]*model.AdapterOauthProfile, len(oauth.Profiles))
	applications := make(map[string]*model.AdapterOauthApplication, len(oauth.Applications))
	grants := make(map[string]*model.AdapterAuthorizationGrant, len(oauth.Grants))
	for _, profile := range oauth.Profiles {
		profiles[profile.ProfileDigest] = profile
	}
	for _, application := range oauth.Applications {
		applications[application.ApplicationID] = application
	}
	for _, grant := range oauth.Grants {
		grants[grant.GrantID] = grant
	}
	clientSetups := make(map[string]*model.AdapterOauthClientSetupIntervention)
	accountSetups := make(map[string]*model.AdapterOauthAccountSetupIntervention)
	var reviews []model.HumanIntervention
	for _, definition := range definitions {
		if definition == nil || definition.Superseded {
			continue
		}
		needsImport := definition.Reviewed && definition.ConnectionCount == 0 &&
			definition.NextAction != nil && definition.NextAction.Kind == "import_application"
		if needsImport && definition.OauthProfileDigest != nil {
			profile := profiles[*definition.OauthProfileDigest]
			if profile != nil && profile.CredentialSetup != nil {
				setup := clientSetups[*definition.OauthProfileDigest]
				if setup == nil {
					setup = &model.AdapterOauthClientSetupIntervention{
						ProfileDigest:   *definition.OauthProfileDigest,
						DisplayName:     profile.DisplayName,
						CredentialSetup: profile.CredentialSetup,
					}
					clientSetups[*definition.OauthProfileDigest] = setup
				}
				setup.DependentDefinitions = append(setup.DependentDefinitions, &model.AdapterOauthClientSetupDependency{
					SemanticDigest: definition.SemanticDigest,
					DisplayName:    definition.DisplayName,
				})
				continue
			}
		}

		needsDefinition := !definition.Reviewed ||
			(definition.ConnectionCount == 0 && definition.CredentialSetup != nil)
		for _, connection := range definition.Connections {
			if adapterConnectionNeedsChatIntervention(connection) {
				needsDefinition = true
				break
			}
		}
		if !needsDefinition {
			continue
		}
		action := definition.NextAction
		if action != nil && (action.Kind == "attach_account" || action.Kind == "add_account" || action.Kind == "add_access" || action.Kind == "reconnect_account") {
			if setupKey, providerDisplayName, accountLabel, ok := adapterAccountSetupIdentity(action, applications, grants); ok {
				setup := accountSetups[setupKey]
				if setup == nil {
					setup = &model.AdapterOauthAccountSetupIntervention{
						SetupKey: setupKey, ProviderDisplayName: providerDisplayName,
						AccountLabel: accountLabel, NextAction: action,
					}
					accountSetups[setupKey] = setup
				} else if adapterAccountActionRank(action) < adapterAccountActionRank(setup.NextAction) {
					setup.NextAction = action
				}
				setup.DependentDefinitions = append(setup.DependentDefinitions, &model.AdapterOauthAccountSetupDependency{
					SemanticDigest: definition.SemanticDigest,
					DisplayName:    definition.DisplayName,
					Action:         action,
				})
				continue
			}
		}
		reviews = append(reviews, definition)
	}

	clientKeys := make([]string, 0, len(clientSetups))
	for key := range clientSetups {
		clientKeys = append(clientKeys, key)
	}
	sort.Strings(clientKeys)
	accountKeys := make([]string, 0, len(accountSetups))
	for key := range accountSetups {
		accountKeys = append(accountKeys, key)
	}
	sort.Strings(accountKeys)
	for _, setup := range clientSetups {
		sort.Slice(setup.DependentDefinitions, func(i, j int) bool {
			left, right := setup.DependentDefinitions[i], setup.DependentDefinitions[j]
			if left.DisplayName == right.DisplayName {
				return left.SemanticDigest < right.SemanticDigest
			}
			return left.DisplayName < right.DisplayName
		})
	}
	for _, setup := range accountSetups {
		sort.Slice(setup.DependentDefinitions, func(i, j int) bool {
			left, right := setup.DependentDefinitions[i], setup.DependentDefinitions[j]
			if left.DisplayName == right.DisplayName {
				return left.SemanticDigest < right.SemanticDigest
			}
			return left.DisplayName < right.DisplayName
		})
	}
	accountInterventions := make([]model.HumanIntervention, 0, len(accountKeys))
	for _, key := range accountKeys {
		accountInterventions = append(accountInterventions, accountSetups[key])
	}
	clientInterventions := make([]model.HumanIntervention, 0, len(clientKeys))
	for _, key := range clientKeys {
		clientInterventions = append(clientInterventions, clientSetups[key])
	}
	return reviews, accountInterventions, clientInterventions
}

func adapterAccountSetupIdentity(
	action *model.AdapterNextAction,
	applications map[string]*model.AdapterOauthApplication,
	grants map[string]*model.AdapterAuthorizationGrant,
) (string, string, *string, bool) {
	if action.GrantID != nil {
		grant := grants[*action.GrantID]
		if grant == nil {
			return "", "", nil, false
		}
		return "grant:" + grant.GrantID, grant.ProviderDisplayName, grant.AccountLabel, true
	}
	if action.ApplicationID == nil {
		return "", "", nil, false
	}
	application := applications[*action.ApplicationID]
	if application == nil {
		return "", "", nil, false
	}
	return "application:" + application.ApplicationID, application.ProviderDisplayName, nil, true
}

func adapterAccountActionRank(action *model.AdapterNextAction) int {
	if action == nil {
		return 99
	}
	switch action.Kind {
	case "reconnect_account":
		return 0
	case "add_access":
		return 1
	case "add_account":
		return 2
	default:
		return 3
	}
}

func adapterDefinitionNeedsChatIntervention(definition *model.AdapterDefinition) bool {
	if definition.Superseded {
		return false
	}
	if !definition.Reviewed || definition.Reviewed && definition.CredentialSetup != nil && definition.NextAction != nil && definition.NextAction.Kind == "set_up_credential" {
		return true
	}
	for _, connection := range definition.Connections {
		if adapterConnectionNeedsChatIntervention(connection) {
			return true
		}
	}
	return false
}

// adapterConnectionNeedsChatIntervention keeps an OAuth connection without a
// reusable grant in the human intervention stream. A grant that needs repair
// has a deterministic reconnect action and does not need a duplicate setup.
func adapterConnectionNeedsChatIntervention(connection *model.AdapterConnection) bool {
	return connection != nil &&
		((connection.Status == "active" && !connection.PolicyConfigured) ||
			(connection.Status == "authentication_required" && connection.GrantID == nil))
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
	if err == nil && result != nil && (action.CapabilityName == "web.browse.interact" || action.CapabilityName == "web.browse.history") {
		available := r.Chat != nil && r.Chat.BrowserActionSessionAvailable(ctx, action)
		result.BrowserSessionAvailable = &available
	}
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
	return actionRequestModelWithBrowserSession(action, nil)
}

func actionRequestModelWithBrowserSession(action store.ActionRequest, available *bool) (*model.GovernedAction, error) {
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
	serviceName := textField(service, "display_name", "")
	arguments := action.Arguments
	if review, ok := action.AuthorizationContext["browser_review_context"].(map[string]any); ok {
		displayed := make(map[string]any, len(action.Arguments)+len(review))
		for key, value := range action.Arguments {
			displayed[key] = value
		}
		for key, value := range review {
			displayed[key] = value
		}
		arguments = displayed
	}
	target := &model.ActionRequestTarget{}
	if serviceName != "" {
		target.ServiceName = actionString(serviceName)
	}
	if value := textField(destination, "service_id", ""); value != "" {
		target.ServiceID = actionString(value)
	}
	if value := textField(destination, "connection_id", ""); value != "" {
		target.ConnectionID = actionString(value)
	}
	if value := textField(destination, "account_id", ""); value != "" {
		target.AccountID = actionString(value)
	}
	if value := textField(service, "connection_label", ""); value != "" {
		target.ConnectionLabel = actionString(value)
	}
	if target.ServiceName == nil && target.ConnectionLabel == nil && target.ServiceID == nil && target.ConnectionID == nil && target.AccountID == nil {
		target = nil
	}
	targetName := action.CapabilityName
	if target != nil {
		switch {
		case target.ConnectionLabel != nil:
			targetName = *target.ConnectionLabel
		case target.ServiceName != nil:
			targetName = *target.ServiceName
		case target.ServiceID != nil:
			targetName = *target.ServiceID
		}
	}
	result := &model.GovernedAction{
		ActionID: action.ID, Revision: action.Revision, ConversationID: conversationID, TaskID: taskID, RunID: runID,
		CapabilityName: action.CapabilityName, ReviewRoute: reviewRoute,
		Behavior: &model.ToolBehavior{
			ReadOnly: action.Behavior.ReadOnly, Idempotent: action.Behavior.RepeatSafe,
			Destructive: action.Behavior.Destructive, OpenWorld: action.Behavior.OpenWorld,
		},
		SafeSummary: action.SafeSummary, Target: target,
		Consequence: actionConsequence(action, targetName), Destination: destination,
		Arguments: arguments, BrowserSessionAvailable: available, State: state,
	}
	if !strings.HasPrefix(action.CapabilityName, "enable.") && (target != nil || action.Behavior.OpenWorld) {
		result.Disclosure = &model.ActionRequestDisclosure{Recipient: targetName, ContentSummary: "the reviewed request data"}
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

func actionConsequence(action store.ActionRequest, target string) string {
	if strings.HasPrefix(action.CapabilityName, "enable.") {
		return "Noema can use this action again later."
	}
	if action.Behavior.ReadOnly {
		return target + " receives the request data shown in Review details."
	}
	if action.Behavior.Destructive {
		return "This can remove or overwrite data in " + target + "."
	}
	if action.Behavior.OpenWorld {
		return "This changes data outside Noema in " + target + "."
	}
	return "This changes data in " + target + "."
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
