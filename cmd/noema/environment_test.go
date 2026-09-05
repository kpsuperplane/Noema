package main

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestEnvironmentOpenAISetupStoresProtectedSecretAndAssignments(t *testing.T) {
	root, database, accounts := openEnvironmentProviderTest(t)
	t.Setenv("NOEMA_OPENAI__API_KEY", "environment-secret-sentinel")
	t.Setenv("NOEMA_OPENAI__ORGANIZATION_ID", " org-environment ")
	t.Setenv("NOEMA_OPENAI__PROJECT_ID", " project-environment ")
	values, err := readOpenAIEnvironment()
	if err != nil {
		t.Fatal(err)
	}
	if _, exists := os.LookupEnv("NOEMA_OPENAI__API_KEY"); exists {
		t.Fatal("OpenAI credential remains in the process environment")
	}
	if err := configureEnvironmentProvider(t.Context(), accounts, database, values, time.Now()); err != nil {
		t.Fatal(err)
	}
	account, err := accounts.LoadAccount(t.Context(), openAIEnvironmentAccountID)
	if err != nil || account.Status != provider.StatusAuthenticated || !account.Metadata.SecretConfigured() {
		t.Fatalf("OpenAI account = %#v, %v", account, err)
	}
	for key, want := range map[string]string{
		"organization_id": "org-environment", "project_id": "project-environment",
	} {
		var got string
		if err := json.Unmarshal(account.Metadata[key], &got); err != nil || got != want {
			t.Fatalf("OpenAI metadata %s = %q, %v", key, got, err)
		}
	}
	secret, err := accounts.LoadSecret(t.Context(), account.ID)
	if err != nil {
		t.Fatal(err)
	}
	if err := secret.Use(func(value string) error {
		if value != "environment-secret-sentinel" {
			t.Fatalf("stored secret = %q", value)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	assignments, err := database.HostedModelAssignments(t.Context())
	if err != nil || len(assignments) != len(store.HostedModelRoles()) {
		t.Fatalf("model assignments = %#v, %v", assignments, err)
	}
	for _, assignment := range assignments {
		if assignment.ProviderKind != "openai" || assignment.SelectionMode != store.ModelSelectionNoemaRecommended {
			t.Fatalf("model assignment = %#v", assignment)
		}
	}
	credentialPath := filepath.Join(root, "providers", "openai", "default", "api_key.json")
	if runtime.GOOS != "windows" {
		info, err := os.Stat(credentialPath)
		if err != nil || info.Mode().Perm() != 0o600 {
			t.Fatalf("credential permissions = %v, %v", info, err)
		}
	}
	if err := database.Close(); err != nil {
		t.Fatal(err)
	}
	databaseBytes, err := os.ReadFile(filepath.Join(root, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(databaseBytes, []byte("environment-secret-sentinel")) {
		t.Fatal("SQLite contains the environment credential")
	}
}

func TestEnvironmentOpenAISetupUsesExplicitCurrentProfile(t *testing.T) {
	_, database, accounts := openEnvironmentProviderTest(t)
	values := openAIEnvironment{
		selectedProvider: "openai", apiKey: "profile-secret",
		model: "gpt-5.6-terra", reasoningEffort: "high",
	}
	if err := configureEnvironmentProvider(t.Context(), accounts, database, values, time.Now()); err != nil {
		t.Fatal(err)
	}
	assignments, err := database.HostedModelAssignments(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	for _, assignment := range assignments {
		if assignment.SelectionMode != store.ModelSelectionExplicitProfile ||
			assignment.ModelProfile != "gpt-5.6-terra" || assignment.ReasoningEffort != store.ModelReasoningHigh {
			t.Fatalf("explicit model assignment = %#v", assignment)
		}
	}
}

func TestEnvironmentOpenAISetupRejectsInvalidExplicitSelection(t *testing.T) {
	for name, selection := range map[string]openAIEnvironment{
		"missing effort": {model: "gpt-5.6-terra"},
		"missing model":  {reasoningEffort: "medium"},
		"unknown model":  {model: "unknown-model", reasoningEffort: "medium"},
	} {
		t.Run(name, func(t *testing.T) {
			_, database, accounts := openEnvironmentProviderTest(t)
			selection.apiKey = "invalid-selection-secret"
			if err := configureEnvironmentProvider(t.Context(), accounts, database, selection, time.Now()); err == nil {
				t.Fatal("invalid environment selection was accepted")
			}
			assignments, err := database.HostedModelAssignments(t.Context())
			if err != nil || len(assignments) != 0 {
				t.Fatalf("assignments after invalid selection = %#v, %v", assignments, err)
			}
		})
	}
}

func TestEnvironmentOpenAISetupDoesNotReplaceStoredCredential(t *testing.T) {
	_, database, accounts := openEnvironmentProviderTest(t)
	original, _ := provider.NewSecret("stored-secret")
	if _, err := accounts.ImportOpenAISecret(t.Context(), original, "stored-org", "", time.Now()); err != nil {
		t.Fatal(err)
	}
	values := openAIEnvironment{apiKey: "replacement-secret", organizationID: "replacement-org"}
	if err := configureEnvironmentProvider(t.Context(), accounts, database, values, time.Now()); err != nil {
		t.Fatal(err)
	}
	account, err := accounts.LoadAccount(t.Context(), openAIEnvironmentAccountID)
	if err != nil || account.Metadata.CredentialRevision() != 1 {
		t.Fatalf("stored account = %#v, %v", account, err)
	}
	secret, err := accounts.LoadSecret(t.Context(), account.ID)
	if err != nil {
		t.Fatal(err)
	}
	_ = secret.Use(func(value string) error {
		if value != "stored-secret" {
			t.Fatalf("stored credential was replaced: %q", value)
		}
		return nil
	})
}

func openEnvironmentProviderTest(t *testing.T) (string, *store.Store, *provider.AccountService) {
	t.Helper()
	root := t.TempDir()
	database, err := store.Open(context.Background(), filepath.Join(root, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	accounts, err := provider.NewAccountService(root, database)
	if err != nil {
		t.Fatal(err)
	}
	if err := accounts.Initialize(context.Background(), time.Now()); err != nil {
		t.Fatal(err)
	}
	return root, database, accounts
}
