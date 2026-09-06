package main

import (
	"context"
	"errors"
	"os"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const openAIEnvironmentAccountID = "provider_account:openai:default"

type openAIEnvironment struct {
	apiKey, selectedProvider, model, reasoningEffort, organizationID, projectID string
}

func readOpenAIEnvironment() (openAIEnvironment, error) {
	values := openAIEnvironment{
		apiKey: os.Getenv("NOEMA_OPENAI__API_KEY"), selectedProvider: os.Getenv("NOEMA_PROVIDER"),
		model: os.Getenv("NOEMA_MODEL"), reasoningEffort: os.Getenv("NOEMA_REASONING_EFFORT"),
		organizationID: os.Getenv("NOEMA_OPENAI__ORGANIZATION_ID"), projectID: os.Getenv("NOEMA_OPENAI__PROJECT_ID"),
	}
	if err := os.Unsetenv("NOEMA_OPENAI__API_KEY"); err != nil {
		return openAIEnvironment{}, errors.New("remove OpenAI credential from process environment")
	}
	return values, nil
}

func configureEnvironmentProvider(
	ctx context.Context,
	accounts *provider.AccountService,
	database *store.Store,
	values openAIEnvironment,
	now time.Time,
) error {
	if strings.TrimSpace(values.apiKey) == "" {
		return nil
	}
	if selected := strings.TrimSpace(values.selectedProvider); selected != "" && selected != "openai" {
		return nil
	}
	existing, err := database.HostedModelAssignments(ctx)
	if err != nil || len(existing) != 0 {
		return err
	}
	secret, err := provider.NewSecret(values.apiKey)
	if err != nil {
		return err
	}
	if _, err := accounts.ImportOpenAISecret(ctx, secret, values.organizationID, values.projectID, now); err != nil {
		return err
	}

	model, effort := strings.TrimSpace(values.model), strings.TrimSpace(values.reasoningEffort)
	if model == "" && effort != "" {
		return errors.New("NOEMA_REASONING_EFFORT requires NOEMA_MODEL")
	}
	assignments := make([]store.ModelAssignment, 0, len(store.HostedModelRoles()))
	for _, role := range store.HostedModelRoles() {
		assignment := store.ModelAssignment{
			Role: role, ProviderKind: "openai", ProviderAccountID: openAIEnvironmentAccountID,
			SelectionMode: store.ModelSelectionNoemaRecommended,
		}
		if model != "" {
			if effort == "" {
				return errors.New("NOEMA_MODEL requires NOEMA_REASONING_EFFORT")
			}
			assignment.SelectionMode = store.ModelSelectionExplicitProfile
			assignment.ModelProfile = model
			assignment.ReasoningEffort = store.ModelReasoningEffort(effort)
		}
		assignments = append(assignments, assignment)
	}
	_, err = database.ConfirmHostedModelAssignments(ctx, openAIEnvironmentAccountID, assignments)
	return err
}
