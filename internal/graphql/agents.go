package graphql

import (
	"context"
	"errors"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) agents(ctx context.Context) ([]*model.Agent, error) {
	if r.ProviderAccounts == nil {
		return nil, errors.New("provider account service is unavailable")
	}
	agents, err := r.Store.Agents(ctx)
	if err != nil {
		return nil, err
	}
	assignments, err := r.Store.HostedModelAssignments(ctx)
	if err != nil {
		return nil, err
	}
	byRole := make(map[store.HostedModelRole]store.ModelAssignment, len(assignments))
	for _, assignment := range assignments {
		byRole[assignment.Role] = assignment
	}
	options, err := r.agentModelOptions(ctx)
	if err != nil {
		return nil, err
	}
	result := make([]*model.Agent, 0, len(agents))
	for _, agent := range agents {
		var preference *model.AgentModelPreference
		if role, _, ok := configurableAgentRole(agent.ID); ok {
			if assignment, exists := byRole[role]; exists {
				preference = agentModelPreference(assignment)
			}
		}
		result = append(result, &model.Agent{
			AgentID: agent.ID, DisplayName: agent.DisplayName,
			IsPrimary:       agent.ID == store.PrimaryAgentID,
			ModelPreference: preference, ModelOptions: options,
		})
	}
	return result, nil
}

func (r *Resolver) saveAgentModelPreference(
	ctx context.Context,
	input model.SaveAgentModelPreferenceInput,
) (*model.AgentModelPreference, error) {
	if input.AgentID == store.TaskExecutorAgentID {
		return nil, errors.New("Task Executor models are configured by complexity tier")
	}
	role, useCase, ok := configurableAgentRole(input.AgentID)
	if !ok {
		return nil, errors.New("Agent has no configurable model recommendation")
	}
	account, err := r.selectableModelAccount(ctx, input.ProviderAccountID)
	if err != nil {
		return nil, err
	}
	assignment, err := assignmentFromPreference(
		account, role, useCase, input.SelectionMode, input.ModelProfile,
		input.ReasoningEffort, input.FastMode,
	)
	if err != nil {
		return nil, err
	}
	saved, err := r.Store.SaveHostedModelAssignment(ctx, assignment)
	if err != nil {
		return nil, err
	}
	return agentModelPreference(saved), nil
}

func (r *Resolver) taskModelPools(
	ctx context.Context,
	complexity *model.TaskComplexity,
) ([]*model.TaskModelPoolEntry, error) {
	var filter *string
	if complexity != nil {
		value := strings.ToLower(complexity.String())
		filter = &value
	}
	entries, err := r.Store.TaskModelPoolEntries(ctx, filter)
	if err != nil {
		return nil, err
	}
	result := make([]*model.TaskModelPoolEntry, 0, len(entries))
	for _, entry := range entries {
		result = append(result, taskModelPoolEntry(entry))
	}
	return result, nil
}

func (r *Resolver) updateTaskModelPoolEntry(
	ctx context.Context,
	poolEntryID string,
	input model.TaskModelPoolEntryInput,
) (*model.TaskModelPoolEntry, error) {
	poolEntryID = strings.TrimSpace(poolEntryID)
	complexity := strings.ToLower(input.Complexity.String())
	role, useCase, ok := taskModelPoolRole(complexity)
	if !ok {
		return nil, errors.New("Task complexity is unsupported")
	}
	entries, err := r.Store.TaskModelPoolEntries(ctx, nil)
	if err != nil {
		return nil, err
	}
	var existing *store.TaskModelPoolEntry
	for index := range entries {
		if entries[index].ID == poolEntryID {
			existing = &entries[index]
			break
		}
	}
	if existing == nil {
		return nil, store.ErrTaskModelPoolEntryNotFound
	}
	if inputMatchesAssignment(input, existing.Assignment) && (existing.Enabled || !input.Enabled) {
		entry, err := r.Store.UpdateTaskModelPoolEntry(
			ctx, poolEntryID, complexity, input.Label, existing.Assignment,
			input.Enabled, input.SortOrder, time.Now(),
		)
		if err != nil {
			return nil, err
		}
		return taskModelPoolEntry(entry), nil
	}

	account, err := r.selectableModelAccount(ctx, strings.TrimSpace(input.ProviderAccountID))
	if err != nil {
		return nil, err
	}
	if strings.TrimSpace(strings.ToLower(input.ProviderKind)) != account.ProviderKind {
		return nil, errors.New("provider kind does not match provider account")
	}
	assignment, err := assignmentFromPreference(
		account, role, useCase, input.SelectionMode, input.ModelProfile,
		input.ReasoningEffort, input.FastMode,
	)
	if err != nil {
		return nil, err
	}
	entry, err := r.Store.UpdateTaskModelPoolEntry(
		ctx, poolEntryID, complexity, input.Label,
		assignment, input.Enabled, input.SortOrder, time.Now(),
	)
	if err != nil {
		return nil, err
	}
	return taskModelPoolEntry(entry), nil
}

func inputMatchesAssignment(input model.TaskModelPoolEntryInput, assignment store.ModelAssignment) bool {
	if strings.TrimSpace(strings.ToLower(input.ProviderKind)) != assignment.ProviderKind ||
		strings.TrimSpace(input.ProviderAccountID) != assignment.ProviderAccountID ||
		input.FastMode != assignment.FastMode {
		return false
	}
	switch input.SelectionMode {
	case model.ModelPreferenceSelectionModeNoemaRecommended:
		return assignment.SelectionMode == store.ModelSelectionNoemaRecommended &&
			input.ModelProfile == nil && input.ReasoningEffort == nil
	case model.ModelPreferenceSelectionModeExplicitProfile:
		if assignment.SelectionMode != store.ModelSelectionExplicitProfile || input.ModelProfile == nil ||
			strings.TrimSpace(*input.ModelProfile) != assignment.ModelProfile {
			return false
		}
		if input.ReasoningEffort == nil {
			return assignment.ReasoningEffort == ""
		}
		return strings.ToLower(input.ReasoningEffort.String()) == string(assignment.ReasoningEffort)
	default:
		return false
	}
}

func (r *Resolver) agentModelOptions(ctx context.Context) ([]*model.AgentModelProviderOption, error) {
	accounts, err := r.ProviderAccounts.Accounts(ctx)
	if err != nil {
		return nil, err
	}
	result := make([]*model.AgentModelProviderOption, 0, len(accounts))
	for _, account := range accounts {
		if !account.IsDefault {
			continue
		}
		profiles, err := account.Metadata.ModelProfiles()
		if err != nil {
			return nil, err
		}
		disabledReason := modelAccountDisabledReason(account)
		profileModels := make([]*model.AgentModelProfileOption, 0, len(profiles))
		for _, profile := range profiles {
			option := modelProfileOption(profile)
			option.DisabledReason = disabledReason
			profileModels = append(profileModels, option)
		}
		recommendations := provider.ModelRecommendations(account.ProviderKind)
		recommendationModels := make([]*model.AgentModelRecommendation, 0, len(recommendations))
		for _, recommendation := range recommendations {
			item := modelRecommendation(recommendation, profiles)
			if disabledReason != nil {
				item.DisabledReason = disabledReason
			}
			recommendationModels = append(recommendationModels, item)
		}
		result = append(result, &model.AgentModelProviderOption{
			ProviderKind: account.ProviderKind, ProviderAccountID: account.ID,
			ProviderDisplayName: account.DisplayName,
			Status:              providerAccountStatusModel(account.Status), Profiles: profileModels,
			Recommendations: recommendationModels, DisabledReason: disabledReason,
		})
	}
	return result, nil
}

func (r *Resolver) selectableModelAccount(
	ctx context.Context,
	accountID string,
) (provider.Account, error) {
	if r.ProviderAccounts == nil {
		return provider.Account{}, errors.New("provider account service is unavailable")
	}
	account, err := r.ProviderAccounts.LoadAccount(ctx, accountID)
	if err != nil {
		return provider.Account{}, err
	}
	if !account.IsActive || !account.IsDefault {
		return provider.Account{}, errors.New("provider account is not selectable")
	}
	if account.Status != provider.StatusAuthenticated {
		return provider.Account{}, errors.New("provider account is not authenticated")
	}
	if len(provider.ModelRecommendations(account.ProviderKind)) == 0 {
		return provider.Account{}, errors.New("provider cannot serve hosted models")
	}
	return account, nil
}

func assignmentFromPreference(
	account provider.Account,
	role store.HostedModelRole,
	useCase provider.ModelUseCase,
	selectionMode model.ModelPreferenceSelectionMode,
	modelProfile *string,
	reasoningEffort *model.ReasoningEffort,
	fastMode bool,
) (store.ModelAssignment, error) {
	assignment := store.ModelAssignment{
		Role: role, ProviderKind: account.ProviderKind, ProviderAccountID: account.ID,
		FastMode: fastMode,
	}
	profiles, err := account.Metadata.ModelProfiles()
	if err != nil {
		return store.ModelAssignment{}, err
	}
	switch selectionMode {
	case model.ModelPreferenceSelectionModeNoemaRecommended:
		if modelProfile != nil || reasoningEffort != nil {
			return store.ModelAssignment{}, errors.New("Noema Recommended does not accept a model or reasoning effort")
		}
		recommendation, ok := recommendationFor(account.ProviderKind, useCase)
		if !ok {
			return store.ModelAssignment{}, errors.New("provider has no Noema recommendation for this use case")
		}
		if err := validateProfileSelection(profiles, recommendation.ModelProfile, optionalEffort(recommendation.ReasoningEffort)); err != nil {
			return store.ModelAssignment{}, err
		}
		assignment.SelectionMode = store.ModelSelectionNoemaRecommended
	case model.ModelPreferenceSelectionModeExplicitProfile:
		if modelProfile == nil || strings.TrimSpace(*modelProfile) == "" {
			return store.ModelAssignment{}, errors.New("explicit model profile is required")
		}
		if err := validateProfileSelection(profiles, *modelProfile, reasoningEffort); err != nil {
			return store.ModelAssignment{}, err
		}
		assignment.SelectionMode = store.ModelSelectionExplicitProfile
		assignment.ModelProfile = *modelProfile
		if reasoningEffort != nil {
			assignment.ReasoningEffort = store.ModelReasoningEffort(strings.ToLower(reasoningEffort.String()))
		}
	default:
		return store.ModelAssignment{}, errors.New("model selection mode is unsupported")
	}
	return assignment, nil
}

func validateProfileSelection(
	profiles []provider.ModelProfile,
	profileID string,
	reasoningEffort *model.ReasoningEffort,
) error {
	for _, profile := range profiles {
		if profile.ID != profileID {
			continue
		}
		if len(profile.ReasoningEfforts) == 0 {
			if reasoningEffort != nil {
				return errors.New("reasoning effort is not available for selected model profile")
			}
			return nil
		}
		if reasoningEffort == nil {
			return errors.New("reasoning effort is required for selected model profile")
		}
		requested := strings.ToLower(reasoningEffort.String())
		for _, supported := range profile.ReasoningEfforts {
			if requested == supported {
				return nil
			}
		}
		return errors.New("reasoning effort is not available for selected model profile")
	}
	return errors.New("model profile is not available for provider")
}

func configurableAgentRole(agentID string) (store.HostedModelRole, provider.ModelUseCase, bool) {
	switch agentID {
	case store.PrimaryAgentID:
		return store.HostedModelNoema, provider.ModelUsePrimary, true
	case store.TaskReviewerAgentID:
		return store.HostedModelTaskReviewer, provider.ModelUseTaskReviewer, true
	default:
		return "", "", false
	}
}

func taskModelPoolRole(complexity string) (store.HostedModelRole, provider.ModelUseCase, bool) {
	switch complexity {
	case "simple":
		return store.HostedModelSimpleTasks, provider.ModelUseTaskSimple, true
	case "medium":
		return store.HostedModelMediumTasks, provider.ModelUseTaskMedium, true
	case "difficult":
		return store.HostedModelDifficultTasks, provider.ModelUseTaskDifficult, true
	default:
		return "", "", false
	}
}

func recommendationFor(kind string, useCase provider.ModelUseCase) (provider.ModelRecommendation, bool) {
	for _, recommendation := range provider.ModelRecommendations(kind) {
		if recommendation.UseCase == useCase {
			return recommendation, true
		}
	}
	return provider.ModelRecommendation{}, false
}

func optionalEffort(value string) *model.ReasoningEffort {
	if effort, ok := reasoningEffortModel(value); ok {
		return &effort
	}
	return nil
}

func modelAccountDisabledReason(account provider.Account) *string {
	var reason string
	switch account.Status {
	case provider.StatusAuthenticated:
		return nil
	case provider.StatusChecking:
		reason = "Provider status is still checking."
	case provider.StatusUnauthenticated:
		reason = "Provider account is not authenticated."
	case provider.StatusUnavailable:
		reason = "Provider is unavailable on this machine."
	default:
		reason = "Provider status has not been checked."
	}
	return &reason
}

func agentModelPreference(assignment store.ModelAssignment) *model.AgentModelPreference {
	result := &model.AgentModelPreference{
		ProviderKind: assignment.ProviderKind, ProviderAccountID: assignment.ProviderAccountID,
		SelectionMode: model.ModelPreferenceSelectionModeNoemaRecommended,
		FastMode:      assignment.FastMode,
	}
	if assignment.SelectionMode == store.ModelSelectionExplicitProfile {
		result.SelectionMode = model.ModelPreferenceSelectionModeExplicitProfile
		result.ModelProfile = &assignment.ModelProfile
		if effort, ok := reasoningEffortModel(string(assignment.ReasoningEffort)); ok {
			result.ReasoningEffort = &effort
		}
	}
	return result
}

func taskModelPoolEntry(entry store.TaskModelPoolEntry) *model.TaskModelPoolEntry {
	preference := agentModelPreference(entry.Assignment)
	return &model.TaskModelPoolEntry{
		PoolEntryID: entry.ID, Complexity: model.TaskComplexity(strings.ToUpper(entry.Complexity)),
		Label: entry.Label, ProviderKind: preference.ProviderKind,
		ProviderAccountID: preference.ProviderAccountID, ModelProfile: preference.ModelProfile,
		ReasoningEffort: preference.ReasoningEffort, FastMode: preference.FastMode,
		SelectionMode: preference.SelectionMode, Enabled: entry.Enabled, SortOrder: entry.SortOrder,
		CreatedAt: entry.CreatedAt.Format(time.RFC3339Nano),
		UpdatedAt: entry.UpdatedAt.Format(time.RFC3339Nano),
	}
}
