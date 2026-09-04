package provider_test

import (
	"bytes"
	"context"
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestSecretAccountFilesAndRollback(t *testing.T) {
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
	secret, _ := provider.NewSecret("first-key")
	account, err := service.CreateSecretAccount(ctx, "openrouter", "", secret, time.Now())
	if err != nil {
		t.Fatalf("create secret account: %v", err)
	}
	if account.ID != "provider_account:openrouter:default" || account.Metadata.CredentialRevision() != 1 {
		t.Fatalf("created account = %#v", account)
	}
	path := filepath.Join(root, "providers", "openrouter", "default", "api_key.json")
	if runtime.GOOS != "windows" {
		info, err := os.Stat(path)
		if err != nil || info.Mode().Perm() != 0o600 {
			t.Fatalf("credential mode = %v, %v", info, err)
		}
	}
	oversized, _ := provider.NewSecret(strings.Repeat("x", (1<<20)+1))
	if _, err := service.SaveSecret(ctx, account.ID, oversized, time.Now()); err == nil {
		t.Fatal("oversized provider secret was stored")
	}

	failing := &failingCredentialPersistence{Store: database}
	rollbackService, _ := provider.NewAccountService(root, failing)
	replacement, _ := provider.NewSecret("replacement-key")
	if _, err := rollbackService.SaveSecret(ctx, account.ID, replacement, time.Now()); err == nil {
		t.Fatal("failed metadata update must fail")
	}
	loaded, err := service.LoadSecret(ctx, account.ID)
	if err != nil {
		t.Fatalf("load restored secret: %v", err)
	}
	if err := loaded.Use(func(value string) error {
		if value != "first-key" {
			t.Fatalf("restored secret = %q", value)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	current, err := database.ProviderAccount(ctx, account.ID)
	if err != nil || current.Metadata.CredentialRevision() != 1 {
		t.Fatalf("credential revision changed after rollback: %#v, %v", current, err)
	}
	if err := database.Close(); err != nil {
		t.Fatal(err)
	}
	stored, err := os.ReadFile(filepath.Join(root, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(stored, []byte("first-key")) || bytes.Contains(stored, []byte("replacement-key")) {
		t.Fatal("SQLite contains provider credential material")
	}
}

func TestAccountMutationsSerializeAndProtectBuiltins(t *testing.T) {
	ctx := context.Background()
	root := t.TempDir()
	database, err := store.Open(ctx, filepath.Join(root, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	tracking := &trackingCredentialPersistence{Store: database}
	service, _ := provider.NewAccountService(root, tracking)
	if err := service.Initialize(ctx, time.Now()); err != nil {
		t.Fatal(err)
	}
	if _, err := service.DeleteAccount(ctx, "provider_account:codex:default"); !errors.Is(err, provider.ErrProtectedAccount) {
		t.Fatalf("built-in delete error = %v", err)
	}
	secret, _ := provider.NewSecret("initial")
	account, err := service.CreateSecretAccount(ctx, "exa", "Personal", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}

	var wait sync.WaitGroup
	for _, value := range []string{"second", "third"} {
		wait.Add(1)
		go func() {
			defer wait.Done()
			secret, _ := provider.NewSecret(value)
			_, _ = service.SaveSecret(ctx, account.ID, secret, time.Now())
		}()
	}
	wait.Wait()
	if tracking.maximum.Load() != 1 {
		t.Fatalf("simultaneous metadata updates = %d", tracking.maximum.Load())
	}
	current, err := database.ProviderAccount(ctx, account.ID)
	if err != nil || current.Metadata.CredentialRevision() != 3 {
		t.Fatalf("serialized revision = %d, %v", current.Metadata.CredentialRevision(), err)
	}
}

type failingCredentialPersistence struct{ *store.Store }

func (f *failingCredentialPersistence) UpdateProviderCredential(
	context.Context, string, uint64, provider.AuthMethod, bool, provider.AccountMetadata, time.Time,
) (provider.Account, error) {
	return provider.Account{}, errors.New("injected metadata failure")
}

type trackingCredentialPersistence struct {
	*store.Store
	active  atomic.Int32
	maximum atomic.Int32
}

func (p *trackingCredentialPersistence) UpdateProviderCredential(
	ctx context.Context, id string, revision uint64, method provider.AuthMethod,
	configured bool, metadata provider.AccountMetadata, now time.Time,
) (provider.Account, error) {
	active := p.active.Add(1)
	defer p.active.Add(-1)
	for current := p.maximum.Load(); active > current; current = p.maximum.Load() {
		if p.maximum.CompareAndSwap(current, active) {
			break
		}
	}
	time.Sleep(10 * time.Millisecond)
	return p.Store.UpdateProviderCredential(ctx, id, revision, method, configured, metadata, now)
}
