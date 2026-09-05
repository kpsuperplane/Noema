package graphql

import (
	"context"
	"net/http/httptest"
	"reflect"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestPrivacySettingsExposeReviewerPreferenceAndOptions(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)

	response := postGraphQL(t, server.URL, `query PrivacySettings {
  privacySettings {
    reviewer {
      modelPreference { providerKind providerAccountId modelProfile reasoningEffort selectionMode fastMode }
      modelOptions { providerKind providerAccountId providerDisplayName status disabledReason }
    }
  }
}`, nil)
	if len(response.Errors) != 0 {
		t.Fatalf("Privacy Settings errors = %#v", response.Errors)
	}
	reviewer := response.Data["privacySettings"].(map[string]any)["reviewer"].(map[string]any)
	preference := reviewer["modelPreference"].(map[string]any)
	if preference["providerKind"] != "openrouter" ||
		preference["selectionMode"] != "NOEMA_RECOMMENDED" {
		t.Fatalf("reviewer preference = %#v", preference)
	}
	options := reviewer["modelOptions"].([]any)
	if len(options) == 0 {
		t.Fatal("reviewer model options are empty")
	}
}

func TestSaveActionReviewerPreferenceChangesOnlyReviewerRole(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	before, err := resolver.Store.HostedModelAssignments(ctx)
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)

	response := postGraphQL(t, server.URL, `mutation SaveReviewer($input: SaveActionReviewerPreferenceInput!) {
  saveActionReviewerPreference(input: $input) {
    providerKind providerAccountId modelProfile reasoningEffort selectionMode fastMode
  }
}`, map[string]any{"input": map[string]any{
		"providerAccountId": "provider_account:openrouter:default",
		"selectionMode":     "EXPLICIT_PROFILE", "modelProfile": "openai/gpt-5.6-luna",
		"reasoningEffort": "HIGH", "fastMode": true,
	}})
	if len(response.Errors) != 0 {
		t.Fatalf("save reviewer errors = %#v", response.Errors)
	}
	saved := response.Data["saveActionReviewerPreference"].(map[string]any)
	if saved["modelProfile"] != "openai/gpt-5.6-luna" ||
		saved["reasoningEffort"] != "HIGH" || saved["fastMode"] != true {
		t.Fatalf("saved reviewer preference = %#v", saved)
	}
	after, err := resolver.Store.HostedModelAssignments(ctx)
	if err != nil {
		t.Fatal(err)
	}
	for index := range before {
		if before[index].Role == store.HostedModelActionReviewer {
			if reflect.DeepEqual(before[index], after[index]) ||
				after[index].SelectionMode != store.ModelSelectionExplicitProfile {
				t.Fatalf("reviewer assignment = %#v", after[index])
			}
			continue
		}
		if !reflect.DeepEqual(before[index], after[index]) {
			t.Fatalf("assignment %s changed from %#v to %#v", before[index].Role, before[index], after[index])
		}
	}
}

func TestSaveActionReviewerPreferenceRejectsUnavailableAccountWithoutChange(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	before := reviewerAssignment(t, resolver, ctx)
	if _, err := resolver.ProviderAccounts.ClearSecret(
		ctx, "provider_account:openrouter:default", time.Now(),
	); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.saveActionReviewerPreference(ctx, model.SaveActionReviewerPreferenceInput{
		ProviderAccountID: "provider_account:openrouter:default",
		SelectionMode:     model.ModelPreferenceSelectionModeNoemaRecommended,
	}); err == nil {
		t.Fatal("unavailable reviewer account was accepted")
	}
	if after := reviewerAssignment(t, resolver, ctx); !reflect.DeepEqual(before, after) {
		t.Fatalf("reviewer changed from %#v to %#v", before, after)
	}
}

func reviewerAssignment(t *testing.T, resolver *Resolver, ctx context.Context) store.ModelAssignment {
	t.Helper()
	assignments, err := resolver.Store.HostedModelAssignments(ctx)
	if err != nil {
		t.Fatal(err)
	}
	for _, assignment := range assignments {
		if assignment.Role == store.HostedModelActionReviewer {
			return assignment
		}
	}
	t.Fatal("action reviewer assignment is missing")
	return store.ModelAssignment{}
}
