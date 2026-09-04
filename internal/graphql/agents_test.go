package graphql

import (
	"context"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestAgentsExposePreferencesButKeepExecutorTierControlled(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	agents, err := resolver.agents(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(agents) != 3 {
		t.Fatalf("Agents count = %d", len(agents))
	}
	byID := make(map[string]*model.Agent, len(agents))
	for _, agent := range agents {
		byID[agent.AgentID] = agent
	}
	primary := byID[store.PrimaryAgentID]
	if primary == nil || !primary.IsPrimary || primary.ModelPreference == nil {
		t.Fatalf("primary Agent = %#v", primary)
	}
	if primary.ModelPreference.SelectionMode != model.ModelPreferenceSelectionModeNoemaRecommended {
		t.Fatalf("primary preference = %#v", primary.ModelPreference)
	}
	executor := byID[store.TaskExecutorAgentID]
	if executor == nil || executor.ModelPreference != nil {
		t.Fatalf("Task Executor Agent = %#v", executor)
	}
	if len(primary.ModelOptions) < 3 {
		t.Fatalf("model option count = %d", len(primary.ModelOptions))
	}
	var openRouterOption *model.AgentModelProviderOption
	for _, option := range primary.ModelOptions {
		if option.ProviderKind == "openrouter" {
			openRouterOption = option
		}
	}
	if openRouterOption == nil || openRouterOption.DisabledReason != nil || len(openRouterOption.Profiles) != 2 {
		t.Fatalf("ready OpenRouter option = %#v", openRouterOption)
	}
}

func TestSaveAgentModelPreferenceValidatesAgentAndProfile(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	accountID := "provider_account:openrouter:default"
	profile := "openai/gpt-5.6-luna"
	effort := model.ReasoningEffortHigh

	saved, err := resolver.saveAgentModelPreference(ctx, model.SaveAgentModelPreferenceInput{
		AgentID: store.PrimaryAgentID, ProviderAccountID: accountID,
		SelectionMode: model.ModelPreferenceSelectionModeExplicitProfile,
		ModelProfile:  &profile, ReasoningEffort: &effort, FastMode: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if saved.ModelProfile == nil || *saved.ModelProfile != profile || !saved.FastMode {
		t.Fatalf("saved preference = %#v", saved)
	}

	_, err = resolver.saveAgentModelPreference(ctx, model.SaveAgentModelPreferenceInput{
		AgentID: store.PrimaryAgentID, ProviderAccountID: accountID,
		SelectionMode: model.ModelPreferenceSelectionModeExplicitProfile,
		ModelProfile:  &profile,
	})
	if err == nil || !strings.Contains(err.Error(), "reasoning effort is required") {
		t.Fatalf("missing effort error = %v", err)
	}
	_, err = resolver.saveAgentModelPreference(ctx, model.SaveAgentModelPreferenceInput{
		AgentID: store.TaskExecutorAgentID, ProviderAccountID: accountID,
		SelectionMode: model.ModelPreferenceSelectionModeNoemaRecommended,
	})
	if err == nil || !strings.Contains(err.Error(), "complexity tier") {
		t.Fatalf("Task Executor error = %v", err)
	}
}

func TestUpdateTaskModelPoolEntrySavesOneTier(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	profile := "openai/gpt-5.6-luna"
	effort := model.ReasoningEffortLow
	label := "Routine"
	updated, err := resolver.updateTaskModelPoolEntry(
		context.Background(), "task_pool:setting:medium", model.TaskModelPoolEntryInput{
			Complexity: model.TaskComplexityMedium, Label: &label,
			ProviderKind: "openrouter", ProviderAccountID: "provider_account:openrouter:default",
			SelectionMode: model.ModelPreferenceSelectionModeExplicitProfile,
			ModelProfile:  &profile, ReasoningEffort: &effort,
			FastMode: true, Enabled: true, SortOrder: 12,
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if updated.Complexity != model.TaskComplexityMedium || updated.Label == nil ||
		*updated.Label != label || updated.SortOrder != 12 || updated.ModelProfile == nil {
		t.Fatalf("updated pool entry = %#v", updated)
	}
	complexity := model.TaskComplexityMedium
	entries, err := resolver.taskModelPools(context.Background(), &complexity)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].PoolEntryID != updated.PoolEntryID {
		t.Fatalf("filtered pool entries = %#v", entries)
	}
}

func TestDisableTaskModelPoolEntryKeepsUnavailableRoute(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	accountID := "provider_account:openrouter:default"
	if _, err := resolver.ProviderAccounts.ClearSecret(ctx, accountID, time.Now()); err != nil {
		t.Fatal(err)
	}
	updated, err := resolver.updateTaskModelPoolEntry(
		ctx, "task_pool:setting:simple", model.TaskModelPoolEntryInput{
			Complexity:   model.TaskComplexitySimple,
			ProviderKind: "openrouter", ProviderAccountID: accountID,
			SelectionMode: model.ModelPreferenceSelectionModeNoemaRecommended,
			Enabled:       false,
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if updated.Enabled {
		t.Fatalf("disabled pool entry = %#v", updated)
	}
}

func readyAgentTestResolver(t *testing.T) *Resolver {
	t.Helper()
	resolver := openProviderTestResolver(t)
	ctx := context.Background()
	secret, err := provider.NewSecret("test-openrouter-key")
	if err != nil {
		t.Fatal(err)
	}
	profiles := []provider.ModelProfile{
		{ID: "openai/gpt-5.6-luna", Label: "GPT-5.6 Luna", ReasoningEfforts: []string{"low", "high"}},
		{ID: "openai/gpt-5.6-sol", Label: "GPT-5.6 Sol", ReasoningEfforts: []string{"medium"}},
	}
	account, err := resolver.ProviderAccounts.PublishVerifiedSecret(
		ctx, "provider_account:openrouter:default", 0, provider.AuthSecretInput,
		secret, profiles, time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	assignments := make([]store.ModelAssignment, 0, len(store.HostedModelRoles()))
	for _, role := range store.HostedModelRoles() {
		assignments = append(assignments, store.ModelAssignment{
			Role: role, ProviderKind: account.ProviderKind, ProviderAccountID: account.ID,
			SelectionMode: store.ModelSelectionNoemaRecommended,
		})
	}
	if _, err := resolver.Store.ConfirmHostedModelAssignments(ctx, account.ID, assignments); err != nil {
		t.Fatal(err)
	}
	return resolver
}
