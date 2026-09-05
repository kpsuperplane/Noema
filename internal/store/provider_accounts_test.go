package store

import (
	"context"
	"encoding/json"
	"errors"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
)

func TestProviderMetadataPreservesProfilesAndChecksRevision(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Now().UTC()
	profiles := json.RawMessage(`[{"id":"gpt-test","label":"Test"}]`)
	account := provider.Account{
		ID: "provider_account:exa:acct_test", ProviderKind: "exa", AccountKey: "acct_test",
		DisplayName: "Test", AuthMethod: provider.AuthSecretInput, IsActive: true,
		Status: provider.StatusUnauthenticated, CreatedAt: now, UpdatedAt: now,
		Metadata: provider.AccountMetadata{"profiles": profiles},
	}
	if _, err := database.CreateProviderAccount(ctx, account); err != nil {
		t.Fatal(err)
	}
	updated, err := database.UpdateProviderCredential(
		ctx, account.ID, 0, provider.AuthSecretInput, true, nil, now.Add(time.Second),
	)
	if err != nil {
		t.Fatal(err)
	}
	if updated.Metadata.CredentialRevision() != 1 || !updated.Metadata.SecretConfigured() {
		t.Fatalf("credential metadata = %#v", updated.Metadata)
	}
	if string(updated.Metadata["profiles"]) != string(profiles) {
		t.Fatalf("profiles were not preserved: %s", updated.Metadata["profiles"])
	}
	if _, err := database.UpdateProviderCredential(
		ctx, account.ID, 0, provider.AuthSecretInput, false, nil, now,
	); !errors.Is(err, provider.ErrAccountConflict) {
		t.Fatalf("stale update error = %v", err)
	}
}

func TestBuiltinMetadataAndDeletionProtection(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Now()); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if len(accounts) != 7 || accounts[0].ProviderKind != "codex" || accounts[4].ProviderKind != "local_models" {
		t.Fatalf("initial built-ins = %#v", accounts)
	}
	var openAI provider.Account
	for _, account := range accounts {
		if account.ProviderKind == "openai" {
			openAI = account
		}
	}
	if openAI.AuthMethod != provider.AuthExternalManual {
		t.Fatalf("OpenAI auth method = %q", openAI.AuthMethod)
	}
	if _, err := database.DeleteProviderAccount(ctx, accounts[0].ID); !errors.Is(err, provider.ErrProtectedAccount) {
		t.Fatalf("built-in delete error = %v", err)
	}
}

func TestProviderCatalogParity(t *testing.T) {
	entries := provider.Catalog()
	want := []string{"codex", "openrouter", "exa", "kernel", "tinyfish", "firecrawl"}
	if len(entries) != len(want) {
		t.Fatalf("catalog length = %d", len(entries))
	}
	for index, kind := range want {
		if entries[index].ProviderKind != kind {
			t.Fatalf("catalog[%d] = %q", index, entries[index].ProviderKind)
		}
	}
	if entries[1].GeneratesAccountKey {
		t.Fatal("OpenRouter must use its default account")
	}
}
