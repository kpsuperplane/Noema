package graphql

import (
	"context"
	"errors"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) webFetchSettings(ctx context.Context) (*model.WebFetchSettings, error) {
	if r.ProviderAccounts == nil {
		return nil, errors.New("provider account service is unavailable")
	}
	assignments, err := r.Store.HostedModelAssignments(ctx)
	if err != nil {
		return nil, err
	}
	var preference *model.AgentModelPreference
	for _, assignment := range assignments {
		if assignment.Role == store.HostedModelWebFetchSummarizer {
			preference = agentModelPreference(assignment)
			break
		}
	}
	options, err := r.agentModelOptions(ctx)
	if err != nil {
		return nil, err
	}
	return &model.WebFetchSettings{Summarizer: &model.WebFetchSummarizerSettings{
		ModelPreference: preference,
		ModelOptions:    options,
	}}, nil
}

func (r *Resolver) saveWebFetchSummarizerPreference(
	ctx context.Context,
	input model.SaveWebFetchSummarizerPreferenceInput,
) (*model.AgentModelPreference, error) {
	account, err := r.selectableModelAccount(ctx, input.ProviderAccountID)
	if err != nil {
		return nil, err
	}
	assignment, err := assignmentFromPreference(
		account, store.HostedModelWebFetchSummarizer, provider.ModelUseWebFetchSummarizer,
		input.SelectionMode, input.ModelProfile, input.ReasoningEffort, input.FastMode,
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
