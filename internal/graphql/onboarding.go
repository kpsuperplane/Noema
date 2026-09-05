package graphql

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) onboardingStatus(ctx context.Context) (*model.OnboardingStatus, error) {
	status := &model.OnboardingStatus{
		Steps: []*model.OnboardingStep{localModelOnboardingStep(false)},
	}
	assignments, err := r.Store.HostedModelAssignments(ctx)
	if err != nil || len(assignments) == 0 {
		return status, err
	}
	primary := assignments[0]
	for _, assignment := range assignments {
		if assignment.Role == store.HostedModelNoema {
			primary = assignment
			break
		}
	}
	if r.ProviderAccounts == nil {
		return nil, errors.New("provider account service is unavailable")
	}
	account, err := r.ProviderAccounts.LoadAccount(ctx, primary.ProviderAccountID)
	if err != nil {
		return nil, err
	}
	ready := account.IsActive && account.Status == provider.StatusAuthenticated
	status.IsUserOnboarded = ready
	status.Steps = append(status.Steps, providerOnboardingStep(account, ready))
	return status, nil
}

func (r *Resolver) onboardingModelSetup(
	ctx context.Context,
	providerAccountID string,
) (*model.OnboardingModelSetup, error) {
	if r.ProviderAccounts == nil {
		return nil, errors.New("provider account service is unavailable")
	}
	account, err := r.currentModelAccount(ctx, providerAccountID)
	if err != nil {
		return nil, err
	}
	if !account.IsActive || account.Status != provider.StatusAuthenticated {
		return nil, errors.New("provider account is not authenticated")
	}
	recommendations := provider.ModelRecommendations(account.ProviderKind)
	if len(recommendations) == 0 {
		return nil, errors.New("provider account cannot serve hosted models")
	}
	profiles, err := account.Metadata.ModelProfiles()
	if err != nil {
		return nil, err
	}
	if len(profiles) == 0 {
		return nil, errors.New("provider account has no available models")
	}

	profileModels := make([]*model.AgentModelProfileOption, 0, len(profiles))
	for _, profile := range profiles {
		profileModels = append(profileModels, modelProfileOption(profile))
	}
	recommendationModels := make([]*model.AgentModelRecommendation, 0, len(recommendations))
	for _, recommendation := range recommendations {
		recommendationModels = append(
			recommendationModels,
			modelRecommendation(recommendation, profiles),
		)
	}
	recommended := &model.OnboardingModelSelection{
		SelectionMode: model.ModelPreferenceSelectionModeNoemaRecommended,
	}
	return &model.OnboardingModelSetup{
		ProviderKind: account.ProviderKind, ProviderAccountID: account.ID,
		ProviderDisplayName: account.DisplayName, Profiles: profileModels,
		Recommendations: recommendationModels,
		ProposedSelections: &model.OnboardingModelSelections{
			Noema: cloneSelection(recommended), SimpleTasks: cloneSelection(recommended),
			MediumTasks: cloneSelection(recommended), DifficultTasks: cloneSelection(recommended),
			TaskReviewer: cloneSelection(recommended), WebFetchSummarizer: cloneSelection(recommended),
			ToolProgressAudit: cloneSelection(recommended), ActionReviewer: cloneSelection(recommended),
			MemoryConsolidation: cloneSelection(recommended),
		},
	}, nil
}

func (r *Resolver) confirmOnboardingModelSelections(
	ctx context.Context,
	input model.ConfirmOnboardingModelSelectionsInput,
) (*model.OnboardingStatus, error) {
	setup, err := r.onboardingModelSetup(ctx, input.ProviderAccountID)
	if err != nil {
		return nil, err
	}
	if input.ActionReviewer == nil {
		return nil, errors.New("action reviewer model is required")
	}
	requested := []struct {
		role      store.HostedModelRole
		useCase   provider.ModelUseCase
		selection *model.OnboardingModelSelectionInput
	}{
		{store.HostedModelNoema, provider.ModelUsePrimary, input.Noema},
		{store.HostedModelSimpleTasks, provider.ModelUseTaskSimple, input.SimpleTasks},
		{store.HostedModelMediumTasks, provider.ModelUseTaskMedium, input.MediumTasks},
		{store.HostedModelDifficultTasks, provider.ModelUseTaskDifficult, input.DifficultTasks},
		{store.HostedModelTaskReviewer, provider.ModelUseTaskReviewer, input.TaskReviewer},
		{store.HostedModelWebFetchSummarizer, provider.ModelUseWebFetchSummarizer, input.WebFetchSummarizer},
		{store.HostedModelToolProgressAudit, provider.ModelUseToolProgressAudit, input.ToolProgressAudit},
		{store.HostedModelActionReviewer, provider.ModelUseActionReviewer, input.ActionReviewer},
		{store.HostedModelMemoryConsolidation, provider.ModelUseMemoryConsolidation, input.MemoryConsolidation},
	}
	assignments := make([]store.ModelAssignment, 0, len(requested))
	for _, request := range requested {
		assignment, err := modelAssignment(setup, request.role, request.useCase, request.selection)
		if err != nil {
			return nil, err
		}
		assignments = append(assignments, assignment)
	}
	if _, err := r.Store.ConfirmHostedModelAssignments(ctx, input.ProviderAccountID, assignments); err != nil {
		return nil, err
	}
	return r.onboardingStatus(ctx)
}

func (r *Resolver) ensurePrimaryConversation(
	ctx context.Context,
	cwd *string,
) (*model.PrimaryConversation, error) {
	status, err := r.onboardingStatus(ctx)
	if err != nil {
		return nil, err
	}
	if !status.IsUserOnboarded {
		return nil, errors.New("Noema onboarding is incomplete")
	}
	providerKind, err := r.primaryModelProviderKind(ctx)
	if err != nil {
		return nil, err
	}
	if providerKind == "" {
		return nil, errors.New("primary agent provider is not initialized")
	}
	workingDirectory := ""
	if cwd != nil {
		workingDirectory = *cwd
	}
	conversation, err := r.Store.EnsurePrimaryConversation(ctx, providerKind, workingDirectory, time.Now())
	if err != nil {
		return nil, err
	}
	return r.primaryConversationModel(ctx, conversation)
}

func (r *Resolver) primaryModelProviderKind(ctx context.Context) (string, error) {
	assignments, err := r.Store.HostedModelAssignments(ctx)
	if err != nil {
		return "", err
	}
	for _, assignment := range assignments {
		if assignment.Role == store.HostedModelNoema {
			return assignment.ProviderKind, nil
		}
	}
	return "", nil
}

func localModelOnboardingStep(ready bool) *model.OnboardingStep {
	status := model.OnboardingStepStatusBlocked
	accountStatus := model.ProviderAccountStatusUnknown
	if ready {
		status = model.OnboardingStepStatusComplete
		accountStatus = model.ProviderAccountStatusAuthenticated
	}
	return &model.OnboardingStep{
		ID: "install_local_model", Status: status, ProviderKind: "local_models",
		ProviderAccountID: "provider_account:local_models:default", AccountKey: "default",
		DisplayName: "Local models", ProviderAccountStatus: accountStatus,
		AuthMethod: model.ProviderAuthMethodNone,
	}
}

func providerOnboardingStep(account provider.Account, ready bool) *model.OnboardingStep {
	status := model.OnboardingStepStatusBlocked
	if ready {
		status = model.OnboardingStepStatusComplete
	}
	return &model.OnboardingStep{
		ID: "connect_provider_account", Status: status, ProviderKind: account.ProviderKind,
		ProviderAccountID: account.ID, AccountKey: account.AccountKey, DisplayName: account.DisplayName,
		ProviderAccountStatus: providerAccountStatusModel(account.Status),
		AuthMethod:            providerAuthMethodModel(account.AuthMethod),
	}
}

func modelProfileOption(profile provider.ModelProfile) *model.AgentModelProfileOption {
	efforts := make([]model.ReasoningEffort, 0, len(profile.ReasoningEfforts))
	for _, effort := range profile.ReasoningEfforts {
		if converted, ok := reasoningEffortModel(effort); ok {
			efforts = append(efforts, converted)
		}
	}
	var defaultEffort *model.ReasoningEffort
	if converted, ok := reasoningEffortModel(profile.DefaultReasoningEffort); ok {
		defaultEffort = &converted
	}
	return &model.AgentModelProfileOption{
		ID: profile.ID, Label: profile.Label, ReasoningEfforts: efforts,
		DefaultReasoningEffort: defaultEffort,
	}
}

func modelRecommendation(
	recommendation provider.ModelRecommendation,
	profiles []provider.ModelProfile,
) *model.AgentModelRecommendation {
	result := &model.AgentModelRecommendation{
		UseCase: modelUseCaseModel(recommendation.UseCase), ModelProfile: recommendation.ModelProfile,
	}
	if effort, ok := reasoningEffortModel(recommendation.ReasoningEffort); ok {
		result.ReasoningEffort = &effort
	}
	for _, profile := range profiles {
		if profile.ID != recommendation.ModelProfile {
			continue
		}
		for _, effort := range profile.ReasoningEfforts {
			if effort == recommendation.ReasoningEffort {
				return result
			}
		}
		if recommendation.ReasoningEffort == "" {
			return result
		}
		reason := "Recommended reasoning effort is unavailable."
		result.DisabledReason = &reason
		return result
	}
	reason := "Recommended model is unavailable."
	result.DisabledReason = &reason
	return result
}

func modelAssignment(
	setup *model.OnboardingModelSetup,
	role store.HostedModelRole,
	useCase provider.ModelUseCase,
	selection *model.OnboardingModelSelectionInput,
) (store.ModelAssignment, error) {
	if selection == nil {
		return store.ModelAssignment{}, fmt.Errorf("%s model selection is required", role)
	}
	assignment := store.ModelAssignment{
		Role: role, ProviderKind: setup.ProviderKind, ProviderAccountID: setup.ProviderAccountID,
		FastMode: selection.FastMode,
	}
	switch selection.SelectionMode {
	case model.ModelPreferenceSelectionModeNoemaRecommended:
		if selection.ModelProfile != nil || selection.ReasoningEffort != nil {
			return store.ModelAssignment{}, errors.New("Noema Recommended does not accept a model or reasoning effort")
		}
		for _, recommendation := range setup.Recommendations {
			if recommendation.UseCase == modelUseCaseModel(useCase) {
				if recommendation.DisabledReason != nil {
					return store.ModelAssignment{}, errors.New(*recommendation.DisabledReason)
				}
				assignment.SelectionMode = store.ModelSelectionNoemaRecommended
				return assignment, nil
			}
		}
		return store.ModelAssignment{}, errors.New("provider has no Noema recommendation for this use case")
	case model.ModelPreferenceSelectionModeExplicitProfile:
		if selection.ModelProfile == nil || strings.TrimSpace(*selection.ModelProfile) == "" {
			return store.ModelAssignment{}, errors.New("explicit model profile is required")
		}
		assignment.SelectionMode = store.ModelSelectionExplicitProfile
		assignment.ModelProfile = *selection.ModelProfile
		if selection.ReasoningEffort != nil {
			assignment.ReasoningEffort = store.ModelReasoningEffort(strings.ToLower(selection.ReasoningEffort.String()))
		}
		return assignment, nil
	default:
		return store.ModelAssignment{}, errors.New("model selection mode is unsupported")
	}
}

func cloneSelection(source *model.OnboardingModelSelection) *model.OnboardingModelSelection {
	copy := *source
	return &copy
}

func reasoningEffortModel(effort string) (model.ReasoningEffort, bool) {
	value := model.ReasoningEffort(strings.ToUpper(effort))
	return value, value.IsValid()
}

func modelUseCaseModel(useCase provider.ModelUseCase) model.NoemaModelUseCase {
	return model.NoemaModelUseCase(strings.ToUpper(string(useCase)))
}
