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

func TestUsageSettingsExposeAndSaveProgressAuditPreference(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	before, err := resolver.Store.HostedModelAssignments(ctx)
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)
	read := postGraphQL(t, server.URL, `query UsageSettings {
  usageSettings { progressAudit {
    modelPreference { providerKind providerAccountId selectionMode }
    modelOptions { providerKind providerAccountId status }
  } }
}`, nil)
	if len(read.Errors) != 0 {
		t.Fatalf("usage settings errors = %#v", read.Errors)
	}
	progress := read.Data["usageSettings"].(map[string]any)["progressAudit"].(map[string]any)
	if progress["modelPreference"].(map[string]any)["selectionMode"] != "NOEMA_RECOMMENDED" ||
		len(progress["modelOptions"].([]any)) == 0 {
		t.Fatalf("progress audit settings = %#v", progress)
	}
	saved := postGraphQL(t, server.URL, `mutation SaveAudit($input: SaveToolProgressAuditPreferenceInput!) {
  saveToolProgressAuditPreference(input: $input) { modelProfile reasoningEffort selectionMode fastMode }
}`, map[string]any{"input": map[string]any{
		"providerAccountId": "provider_account:openrouter:default",
		"selectionMode":     "EXPLICIT_PROFILE", "modelProfile": "openai/gpt-5.6-luna",
		"reasoningEffort": "HIGH", "fastMode": true,
	}})
	if len(saved.Errors) != 0 {
		t.Fatalf("save audit errors = %#v", saved.Errors)
	}
	after, err := resolver.Store.HostedModelAssignments(ctx)
	if err != nil {
		t.Fatal(err)
	}
	for index := range before {
		if before[index].Role == store.HostedModelToolProgressAudit {
			if after[index].ModelProfile != "openai/gpt-5.6-luna" || !after[index].FastMode {
				t.Fatalf("saved audit assignment = %#v", after[index])
			}
			continue
		}
		if !reflect.DeepEqual(before[index], after[index]) {
			t.Fatalf("assignment %s changed", before[index].Role)
		}
	}
}

func TestSaveProgressAuditPreferenceRejectsUnavailableAccount(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	before := progressAuditTestAssignment(t, resolver)
	if _, err := resolver.ProviderAccounts.ClearSecret(
		ctx, "provider_account:openrouter:default", time.Now(),
	); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.saveToolProgressAuditPreference(ctx, model.SaveToolProgressAuditPreferenceInput{
		ProviderAccountID: "provider_account:openrouter:default",
		SelectionMode:     model.ModelPreferenceSelectionModeNoemaRecommended,
	}); err == nil {
		t.Fatal("unavailable audit account was accepted")
	}
	if after := progressAuditTestAssignment(t, resolver); !reflect.DeepEqual(before, after) {
		t.Fatalf("audit assignment changed from %#v to %#v", before, after)
	}
}

func progressAuditTestAssignment(t *testing.T, resolver *Resolver) store.ModelAssignment {
	t.Helper()
	assignments, err := resolver.Store.HostedModelAssignments(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	for _, assignment := range assignments {
		if assignment.Role == store.HostedModelToolProgressAudit {
			return assignment
		}
	}
	t.Fatal("progress audit assignment is missing")
	return store.ModelAssignment{}
}
