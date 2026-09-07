package provider_test

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

// TestRustProviderSecretLifecycle ports save/load/clear, blank-input, and
// private-file assertions for a user-managed provider account.
func TestRustProviderSecretLifecycle(t *testing.T) {
	ctx := context.Background()
	root := t.TempDir()
	database, err := store.Open(ctx, filepath.Join(root, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	service, err := provider.NewAccountService(root, database)
	if err != nil {
		t.Fatal(err)
	}
	secret, err := provider.NewSecret("stored-secret")
	if err != nil {
		t.Fatal(err)
	}
	account, err := service.CreateSecretAccount(ctx, "exa", "Research", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "providers", "exa", account.AccountKey, "api_key.json")
	if _, err := os.Stat(path); err != nil {
		t.Fatal(err)
	}
	loaded, err := service.LoadSecret(ctx, account.ID)
	if err != nil {
		t.Fatal(err)
	}
	if err := loaded.Use(func(value string) error {
		if value != "stored-secret" {
			t.Fatalf("stored secret = %q", value)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	if _, err := service.SaveSecret(ctx, account.ID, provider.Secret{}, time.Now()); err == nil {
		t.Fatal("zero provider secret was accepted")
	}

	cleared, err := service.ClearSecret(ctx, account.ID, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if cleared.Metadata.SecretConfigured() || cleared.Metadata.CredentialRevision() != 2 {
		t.Fatalf("cleared account metadata = %#v", cleared.Metadata)
	}
	if _, err := os.Stat(path); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("cleared credential path error = %v", err)
	}
	if _, err := service.LoadSecret(ctx, account.ID); err == nil {
		t.Fatal("cleared provider secret remained readable")
	}

	accountInfo, err := os.Stat(filepath.Dir(path))
	if err == nil && accountInfo.Mode().Perm() != 0o700 {
		t.Fatalf("provider account directory mode = %o", accountInfo.Mode().Perm())
	}
}

func TestRustProviderSecretMutationPreservesMetadataAndRollsBackDelete(t *testing.T) {
	ctx := context.Background()
	root := t.TempDir()
	database, err := store.Open(ctx, filepath.Join(root, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	now := time.Now().UTC()
	account := provider.Account{
		ID: "provider_account:exa:team", ProviderKind: "exa", AccountKey: "team",
		DisplayName: "Research", AuthMethod: provider.AuthSecretInput, IsActive: true,
		Status: provider.StatusAuthenticated,
		Metadata: provider.AccountMetadata{
			"base_url":           []byte(`"https://example.test"`),
			"credentialRevision": []byte(`4`),
		},
		CreatedAt: now, UpdatedAt: now,
	}
	if _, err := database.CreateProviderAccount(ctx, account); err != nil {
		t.Fatal(err)
	}
	service, err := provider.NewAccountService(root, database)
	if err != nil {
		t.Fatal(err)
	}
	secret, err := provider.NewSecret("replacement-secret")
	if err != nil {
		t.Fatal(err)
	}
	updated, err := service.SaveSecret(ctx, account.ID, secret, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if string(updated.Metadata["base_url"]) != `"https://example.test"` ||
		updated.Metadata.CredentialRevision() != 5 || !updated.Metadata.SecretConfigured() {
		t.Fatalf("saved account metadata = %#v", updated.Metadata)
	}

	path := filepath.Join(root, "providers", "exa", "team", "api_key.json")
	original := []byte("credential bytes")
	if err := home.AtomicWritePrivate(path, original); err != nil {
		t.Fatal(err)
	}
	failingService, err := provider.NewAccountService(root, failingDeletePersistence{Store: database})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := failingService.DeleteAccount(ctx, account.ID); !errors.Is(err, errInjectedDelete) {
		t.Fatalf("delete failure = %v", err)
	}
	restored, err := os.ReadFile(path)
	if err != nil || string(restored) != string(original) {
		t.Fatalf("restored credential bytes = %q, %v", restored, err)
	}
	if _, err := database.ProviderAccount(ctx, account.ID); err != nil {
		t.Fatalf("durable account after failed delete = %v", err)
	}
}

type failingDeletePersistence struct{ *store.Store }

var errInjectedDelete = errors.New("delete failed")

func (failingDeletePersistence) DeleteProviderAccount(context.Context, string) (bool, error) {
	return false, errInjectedDelete
}
