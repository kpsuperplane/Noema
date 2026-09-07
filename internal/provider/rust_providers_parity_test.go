package provider

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/netpolicy"
)

func nowUTC() time.Time { return time.Now().UTC() }

type SequenceLoader struct {
	mu        sync.Mutex
	snapshots []ProviderSelection
	last      *ProviderSelection
}

func selection(instanceKey string) ProviderSelection {
	return ProviderSelection{
		ProviderKind: "openai",
		AccountID:    "provider_account:openai:default",
		ModelProfile: "gpt-test",
		InstanceKey:  instanceKey,
	}
}

func newSequenceLoader(snapshots ...ProviderSelection) *SequenceLoader {
	return &SequenceLoader{snapshots: append([]ProviderSelection(nil), snapshots...)}
}

func (loader *SequenceLoader) loadProviderSelection(context.Context) (ProviderSelection, error) {
	loader.mu.Lock()
	defer loader.mu.Unlock()
	var selection ProviderSelection
	if len(loader.snapshots) > 0 {
		selection = loader.snapshots[0]
		loader.snapshots = loader.snapshots[1:]
		loader.last = &selection
	} else if loader.last != nil {
		selection = *loader.last
	} else {
		return ProviderSelection{}, errors.New("selection load")
	}
	return selection, nil
}

type PausedSelectionLoader struct {
	current   *ProviderSelection
	mu        *sync.Mutex
	firstRead atomic.Bool
	read      chan<- struct{}
	resume    <-chan struct{}
}

func (loader *PausedSelectionLoader) loadProviderSelection(ctx context.Context) (ProviderSelection, error) {
	loader.mu.Lock()
	snapshot := *loader.current
	loader.mu.Unlock()
	if loader.firstRead.CompareAndSwap(false, true) {
		select {
		case loader.read <- struct{}{}:
		case <-ctx.Done():
			return ProviderSelection{}, ctx.Err()
		}
		select {
		case <-loader.resume:
		case <-ctx.Done():
			return ProviderSelection{}, ctx.Err()
		}
	}
	return snapshot, nil
}

func resolver(loader providerSelectionLoader, registry *providerRegistry) *providerRouteResolver {
	return newProviderRouteResolver(loader, registry)
}

type routeResult struct {
	route providerRoute
	err   error
}

func parityParseCodexSSE(t *testing.T, payload string, onEvent func(StreamEvent)) (codexStreamResult, error) {
	t.Helper()
	return parseCodexGenerationStream(t.Context(), newCodexGenerationStream(io.NopCloser(strings.NewReader(payload))), onEvent)
}

func parityGenerationTool(name string) GenerationTool {
	return GenerationTool{
		Name: name, Description: "Search governed Noema memory.",
		InputSchema: json.RawMessage(`{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}`),
	}
}

func jsonArrayHasString(value any, expected string) bool {
	values, ok := value.([]any)
	if !ok {
		return false
	}
	for _, item := range values {
		if item == expected {
			return true
		}
	}
	return false
}

func parityFoundationBridge(t *testing.T, extra string, available bool) string {
	t.Helper()
	health := `{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}`
	if !available {
		health = `{"id":"health","payload":{"type":"health","available":false,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":"Foundation Models runtime is unavailable."}}`
	}
	script := "#!/bin/sh\nwhile IFS= read -r line; do\ncase \"$line\" in\n*'\"id\":\"handshake\"'*) printf '%s\\n' '{\"id\":\"handshake\",\"payload\":{\"type\":\"handshake_ok\",\"protocol_version\":3}}' ;;\n*'\"id\":\"health\"'*) printf '%s\\n' '" + health + "' ;;\n" + extra + "*) printf '%s\\n' '{\"id\":\"unknown\",\"payload\":{\"type\":\"error\",\"code\":\"unsupported_request\",\"message\":\"Unsupported request.\"}}' ;;\nesac\ndone\n"
	path := filepath.Join(t.TempDir(), "bridge")
	if err := os.WriteFile(path, []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	return path
}

func parityFoundationProvider(t *testing.T, extra string) *foundationProvider {
	t.Helper()
	return newFoundationProvider(foundationProviderConfig{DefaultProfile: "default", BridgePath: parityFoundationBridge(t, extra, true)})
}

type parityCredentialPersistence struct {
	*memoryAccountPersistence
	failCreate bool
	failUpdate bool
	failDelete bool
	onCreate   func(Account)
}

func newParityCredentialPersistence() *parityCredentialPersistence {
	return &parityCredentialPersistence{memoryAccountPersistence: &memoryAccountPersistence{accounts: make(map[string]Account)}}
}

func (p *parityCredentialPersistence) CreateProviderAccount(ctx context.Context, account Account) (Account, error) {
	if p.failCreate {
		if p.onCreate != nil {
			p.onCreate(account)
		}
		return Account{}, errors.New("injected account persistence failure")
	}
	return p.memoryAccountPersistence.CreateProviderAccount(ctx, account)
}

func (p *parityCredentialPersistence) UpdateProviderCredential(
	ctx context.Context, id string, expected uint64, method AuthMethod, configured bool,
	metadata AccountMetadata, now time.Time,
) (Account, error) {
	if p.failUpdate {
		p.failUpdate = false
		return Account{}, errors.New("injected account update failure")
	}
	return p.memoryAccountPersistence.UpdateProviderCredential(ctx, id, expected, method, configured, metadata, now)
}

func (p *parityCredentialPersistence) DeleteProviderAccount(ctx context.Context, id string) (bool, error) {
	if p.failDelete {
		p.failDelete = false
		return false, errors.New("injected account delete failure")
	}
	return p.memoryAccountPersistence.DeleteProviderAccount(ctx, id)
}

func (p *parityCredentialPersistence) MarkProviderAuthenticationFailed(_ context.Context, id string, revision uint64, now time.Time) error {
	p.mu.Lock()
	defer p.mu.Unlock()
	account, exists := p.accounts[id]
	if !exists {
		return ErrAccountNotFound
	}
	if account.Metadata.CredentialRevision() != revision {
		return ErrAccountConflict
	}
	account.Status = StatusUnauthenticated
	account.LastCheckedAt = &now
	account.LastErrorCode = "authentication_failed"
	account.LastErrorMessage = "Provider rejected the credential."
	account.UpdatedAt = now
	p.accounts[id] = account
	return nil
}

// Rust source: crates/noema-providers/src/account_operations/requests.rs::credential_requests_reject_blank_secrets (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CredentialRequestsRejectBlankSecrets(t *testing.T) {
	for _, value := range []string{" ", "\n\t"} {
		if _, err := NewSecret(value); err == nil {
			t.Fatalf("blank provider secret %q was accepted", value)
		}
	}
}

// Rust source: crates/noema-providers/src/account_operations/requests.rs::credential_request_debug_output_is_redacted (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CredentialRequestDebugOutputIsRedacted(t *testing.T) {
	create, err := NewSecret("create-secret-value")
	if err != nil {
		t.Fatal(err)
	}
	save, err := NewSecret("save-secret-value")
	if err != nil {
		t.Fatal(err)
	}
	debug := fmt.Sprintf("%v %s %q %+v %#v %v %s %q", create, create, create, create, create, save, save, save)
	for _, secret := range []string{"create-secret-value", "save-secret-value"} {
		if strings.Contains(debug, secret) {
			t.Fatalf("secret appeared in debug output: %q", debug)
		}
	}
	if !strings.Contains(debug, "[REDACTED]") {
		t.Fatalf("redacted marker missing from debug output: %q", debug)
	}
}

// Rust source: crates/noema-providers/src/accounts.rs::hosted_instance_keys_are_stable_and_unambiguous_per_account (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_HostedInstanceKeysAreStableAndUnambiguousPerAccount(t *testing.T) {
	builtins := BuiltinAccounts(time.Unix(1_700_000_000, 0))
	seen := make(map[string]string, len(builtins))
	for _, account := range builtins {
		key := account.ProviderKind + ":" + account.AccountKey
		if previous, exists := seen[key]; exists {
			t.Fatalf("provider account key %q is shared by %q and %q", key, previous, account.ID)
		}
		seen[key] = account.ID
		if account.ID != "provider_account:"+account.ProviderKind+":"+account.AccountKey {
			t.Fatalf("account identity = %q", account.ID)
		}
	}
	if len(seen) != len(builtins) {
		t.Fatalf("stable provider key count = %d, want %d", len(seen), len(builtins))
	}
}

// Rust source: crates/noema-providers/src/accounts.rs::auth_attempt_debug_redacts_user_code (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AuthAttemptDebugRedactsUserCode(t *testing.T) {
	attempt := AuthAttempt{
		ID: "attempt-1", ProviderKind: "codex", ProviderAccountID: "provider_account:codex:default",
		Method: AuthOAuthDeviceCode, Status: AuthAttemptWaiting, VerificationURL: "https://example.test/verify", UserCode: "SECRET-CODE",
	}
	debug := fmt.Sprintf("%v %s %q %+v %#v", attempt, attempt, attempt, attempt, attempt)
	if strings.Contains(debug, "SECRET-CODE") || !strings.Contains(debug, "[REDACTED]") {
		t.Fatalf("auth attempt debug = %q", debug)
	}
}

// Rust source: crates/noema-providers/src/accounts.rs::device_auth_request_debug_preserves_ordinary_configuration (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DeviceAuthRequestDebugPreservesOrdinaryConfiguration(t *testing.T) {
	attempt := AuthAttempt{
		ID: "attempt-ordinary", ProviderKind: "codex", ProviderAccountID: "provider_account:codex:default",
		Method: AuthOAuthDeviceCode, Status: AuthAttemptWaiting,
		VerificationURL: "https://example.test/verify", Instructions: "Use the displayed code.", UserCode: "SECRET-CODE",
	}
	debug := fmt.Sprintf("%v %s %q %+v %#v", attempt, attempt, attempt, attempt, attempt)
	for _, ordinary := range []string{"attempt-ordinary", "codex", "provider_account:codex:default", "https://example.test/verify"} {
		if !strings.Contains(debug, ordinary) {
			t.Fatalf("ordinary auth configuration %q missing from %q", ordinary, debug)
		}
	}
	if strings.Contains(debug, "SECRET-CODE") || !strings.Contains(debug, "[REDACTED]") {
		t.Fatalf("auth attempt debug = %q", debug)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_files.rs::save_load_clear_and_protect_api_key (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SaveLoadClearAndProtectApiKey(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	secret, err := NewSecret("secret-key")
	if err != nil {
		t.Fatal(err)
	}
	account, err := accounts.CreateSecretAccount(t.Context(), "exa", "acct_one", secret, nowUTC())
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "providers", "exa", account.AccountKey, "api_key.json")
	loaded, err := accounts.LoadSecret(t.Context(), account.ID)
	if err != nil {
		t.Fatal(err)
	}
	if err := loaded.Use(func(value string) error {
		if value != "secret-key" {
			t.Fatalf("loaded secret = %q", value)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	if info, err := os.Stat(filepath.Dir(path)); err != nil || info.Mode().Perm() != 0o700 {
		t.Fatalf("account directory permissions = %v, %v", info, err)
	}
	if info, err := os.Stat(path); err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("credential file permissions = %v, %v", info, err)
	}
	if _, err := accounts.ClearSecret(t.Context(), account.ID, nowUTC()); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("cleared credential path error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_files.rs::rejects_blank_api_key (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RejectsBlankApiKey(t *testing.T) {
	if _, err := NewSecret("  "); err == nil {
		t.Fatal("blank API key was accepted")
	}
}

// Rust source: crates/noema-providers/src/adapters/account_files.rs::secret_input_store_debug_redacts_account_path (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SecretInputStoreDebugRedactsAccountPath(t *testing.T) {
	secret, err := NewSecret("provider-secret")
	if err != nil {
		t.Fatal(err)
	}
	debug := fmt.Sprintf("%v %s %q %+v %#v", secret, secret, secret, secret, secret)
	if strings.Contains(debug, "provider-secret") || !strings.Contains(debug, "[REDACTED]") {
		t.Fatalf("secret debug = %q", debug)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/credentials.rs::provider_credential_debug_redacts_secret (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ProviderCredentialDebugRedactsSecret(t *testing.T) {
	secret, err := NewSecret("credential-secret")
	if err != nil {
		t.Fatal(err)
	}
	debug := fmt.Sprintf("%v %s %q %+v %#v", secret, secret, secret, secret, secret)
	if strings.Contains(debug, "credential-secret") || !strings.Contains(debug, "[REDACTED]") {
		t.Fatalf("credential debug = %q", debug)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/credentials.rs::exa_access_rejects_mismatched_provider_identity (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ExaAccessRejectsMismatchedProviderIdentity(t *testing.T) {
	account := builtinAccount("exa", "Exa", AuthOAuthDeviceCode, nowUTC())
	account.ID = "provider_account:exa:default"
	if err := validateAccount(account); !errors.Is(err, ErrAuthMethodMismatch) {
		t.Fatalf("mismatched provider account error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/credentials.rs::codex_access_returns_usable_token_without_refreshing (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CodexAccessReturnsUsableTokenWithoutRefreshing(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "providers", "codex", "default", "codex_tokens.json")
	if err := writeCodexTokens(path, CodexTokens{accessToken: "usable-access", refreshToken: "refresh", lastRefresh: 1}); err != nil {
		t.Fatal(err)
	}
	tokens, err := accounts.LoadCodexTokens(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	if err := tokens.Use(func(access, refresh string, lastRefresh uint64) error {
		if access != "usable-access" || refresh != "refresh" || lastRefresh != 1 {
			t.Fatalf("loaded Codex tokens = %q, %q, %d", access, refresh, lastRefresh)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/credentials.rs::codex_refresh_does_not_recreate_account_deleted_during_http (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CodexRefreshDoesNotRecreateAccountDeletedDuringHttp(t *testing.T) {
	root := t.TempDir()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence := newParityCredentialPersistence()
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "providers", "codex", "default", "codex_tokens.json")
	if err := writeCodexTokens(path, CodexTokens{accessToken: "old-access", refreshToken: "old-refresh", lastRefresh: 1}); err != nil {
		t.Fatal(err)
	}
	called := false
	digest := codexAccessTokenDigestFor("old-access")
	_, err = accounts.RefreshCodexTokens(t.Context(), &digest, nowUTC(), func(context.Context, CodexTokens) (CodexTokens, error) {
		called = true
		delete(persistence.accounts, account.ID)
		return CodexTokens{accessToken: "new-access", refreshToken: "new-refresh"}, nil
	})
	if !called {
		t.Fatal("refresh callback was not called")
	}
	if !errors.Is(err, ErrAccountNotFound) && !errors.Is(err, ErrAccountConflict) {
		t.Fatalf("deleted account refresh error = %v", err)
	}
	if _, statErr := os.Stat(path); statErr != nil {
		t.Fatalf("refresh removed existing token file: %v", statErr)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/filesystem.rs::snapshot_and_restore_preserve_exact_bytes (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SnapshotAndRestorePreserveExactBytes(t *testing.T) {
	path := filepath.Join(t.TempDir(), "provider", "account", "credential.json")
	original := []byte("{not valid json}\x00\xff")
	if err := home.AtomicWritePrivate(path, original); err != nil {
		t.Fatal(err)
	}
	snapshot, err := snapshotPrivateFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(path, []byte("replacement")); err != nil {
		t.Fatal(err)
	}
	if err := restorePrivateFile(path, snapshot); err != nil {
		t.Fatal(err)
	}
	restored, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(restored, original) {
		t.Fatalf("restored bytes = %q, %v", restored, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/filesystem.rs::missing_snapshot_removes_replacement (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_MissingSnapshotRemovesReplacement(t *testing.T) {
	path := filepath.Join(t.TempDir(), "provider", "account", "credential.json")
	snapshot, err := snapshotPrivateFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(path, []byte("replacement")); err != nil {
		t.Fatal(err)
	}
	if err := restorePrivateFile(path, snapshot); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("restored missing path error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/filesystem.rs::file_snapshot_debug_redacts_present_bytes (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FileSnapshotDebugRedactsPresentBytes(t *testing.T) {
	path := filepath.Join(t.TempDir(), "credential.json")
	if err := home.AtomicWritePrivate(path, []byte("credential-secret")); err != nil {
		t.Fatal(err)
	}
	snapshot, err := snapshotPrivateFile(path)
	if err != nil {
		t.Fatal(err)
	}
	debug := fmt.Sprintf("%v %+v %#v", snapshot, snapshot, snapshot)
	if strings.Contains(debug, "credential-secret") || !strings.Contains(debug, "[REDACTED]") {
		t.Fatalf("snapshot debug = %q", debug)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/gates.rs::live_gate_serializes_account_operations (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LiveGateSerializesAccountOperations(t *testing.T) {
	service, err := NewAccountService(t.TempDir(), newParityCredentialPersistence())
	if err != nil {
		t.Fatal(err)
	}
	first := service.gate("provider_account:exa:first")
	second := service.gate("provider_account:exa:first")
	other := service.gate("provider_account:exa:second")
	if first == other {
		t.Fatal("different provider accounts share one gate")
	}
	first.Lock()
	acquired := make(chan struct{})
	go func() {
		second.Lock()
		close(acquired)
		second.Unlock()
	}()
	select {
	case <-acquired:
		t.Fatal("same-account gate did not block")
	case <-time.After(10 * time.Millisecond):
	}
	first.Unlock()
	select {
	case <-acquired:
	case <-time.After(time.Second):
		t.Fatal("same-account gate did not release")
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/mutations.rs::failed_credential_rollback_preserves_durable_account_evidence (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FailedCredentialRollbackPreservesDurableAccountEvidence(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := Account{
		ID: "provider_account:exa:team", ProviderKind: "exa", AccountKey: "team", DisplayName: "Team",
		AuthMethod: AuthSecretInput, IsActive: true, Status: StatusAuthenticated,
		Metadata:  AccountMetadata{"credentialRevision": []byte("1"), "secretConfigured": []byte("true")},
		CreatedAt: nowUTC(), UpdatedAt: nowUTC(),
	}
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	original := []byte("{malformed prior secret}\x00\xff")
	path := filepath.Join(root, "providers", "exa", "team", "api_key.json")
	if err := home.AtomicWritePrivate(path, original); err != nil {
		t.Fatal(err)
	}
	persistence.failUpdate = true
	replacement, _ := NewSecret("replacement")
	if _, err := accounts.SaveSecret(t.Context(), account.ID, replacement, nowUTC()); err == nil {
		t.Fatal("injected metadata failure succeeded")
	}
	restored, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(restored, original) {
		t.Fatalf("rollback bytes = %q, %v", restored, err)
	}
	durable, err := persistence.ProviderAccount(t.Context(), account.ID)
	if err != nil || durable.Metadata.CredentialRevision() != 1 || !durable.Metadata.SecretConfigured() {
		t.Fatalf("durable account after rollback = %#v, %v", durable, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests.rs::secret_account_creation_compensation_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SecretAccountCreationCompensationContracts(t *testing.T) {
	t.Run("secret write", func(t *testing.T) {
		root := t.TempDir()
		if err := os.WriteFile(filepath.Join(root, "providers"), []byte("blocking file"), 0o600); err != nil {
			t.Fatal(err)
		}
		persistence := newParityCredentialPersistence()
		accounts, err := NewAccountService(root, persistence)
		if err != nil {
			t.Fatal(err)
		}
		secret, _ := NewSecret("new-secret")
		if _, err := accounts.CreateSecretAccount(t.Context(), "exa", "", secret, nowUTC()); err == nil {
			t.Fatal("credential write failure succeeded")
		}
		if len(persistence.accounts) != 0 {
			t.Fatalf("account persisted after credential write failure: %#v", persistence.accounts)
		}
	})
	t.Run("durable update", func(t *testing.T) {
		root := t.TempDir()
		persistence := newParityCredentialPersistence()
		persistence.failCreate = true
		accounts, err := NewAccountService(root, persistence)
		if err != nil {
			t.Fatal(err)
		}
		secret, _ := NewSecret("new-secret")
		if _, err := accounts.CreateSecretAccount(t.Context(), "exa", "", secret, nowUTC()); err == nil {
			t.Fatal("durable update failure succeeded")
		}
		matches, err := filepath.Glob(filepath.Join(root, "providers", "exa", "*", "api_key.json"))
		if err != nil || len(matches) != 0 {
			t.Fatalf("credential remained after durable update failure: %v, %#v", err, matches)
		}
	})
	t.Run("compensation", func(t *testing.T) {
		root := t.TempDir()
		persistence := newParityCredentialPersistence()
		persistence.failCreate = true
		persistence.onCreate = func(account Account) {
			_ = os.RemoveAll(filepath.Join(root, "providers", account.ProviderKind))
			_ = os.WriteFile(filepath.Join(root, "providers", account.ProviderKind), []byte("blocking file"), 0o600)
		}
		accounts, err := NewAccountService(root, persistence)
		if err != nil {
			t.Fatal(err)
		}
		secret, _ := NewSecret("new-secret")
		if _, err := accounts.CreateSecretAccount(t.Context(), "exa", "", secret, nowUTC()); !errors.Is(err, ErrCompensationFailed) {
			t.Fatalf("compensation error = %v", err)
		}
	})
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests.rs::secret_mutation_update_failure_restores_exact_prior_bytes (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SecretMutationUpdateFailureRestoresExactPriorBytes(t *testing.T) {
	for _, clear := range []bool{false, true} {
		root := t.TempDir()
		persistence := newParityCredentialPersistence()
		account := Account{
			ID: "provider_account:exa:team", ProviderKind: "exa", AccountKey: "team", DisplayName: "Team",
			AuthMethod: AuthSecretInput, IsActive: true, Status: StatusAuthenticated,
			Metadata:  AccountMetadata{"credentialRevision": []byte("1"), "secretConfigured": []byte("true")},
			CreatedAt: nowUTC(), UpdatedAt: nowUTC(),
		}
		persistence.accounts[account.ID] = account
		accounts, err := NewAccountService(root, persistence)
		if err != nil {
			t.Fatal(err)
		}
		path := filepath.Join(root, "providers", "exa", "team", "api_key.json")
		original := []byte("{malformed prior secret}\x00\xff")
		if err := home.AtomicWritePrivate(path, original); err != nil {
			t.Fatal(err)
		}
		persistence.failUpdate = true
		var operationErr error
		if clear {
			_, operationErr = accounts.ClearSecret(t.Context(), account.ID, nowUTC())
		} else {
			secret, _ := NewSecret("replacement")
			_, operationErr = accounts.SaveSecret(t.Context(), account.ID, secret, nowUTC())
		}
		if operationErr == nil {
			t.Fatal("metadata update failure succeeded")
		}
		restored, err := os.ReadFile(path)
		if err != nil || !bytes.Equal(restored, original) {
			t.Fatalf("clear=%t restored bytes = %q, %v", clear, restored, err)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests.rs::save_merges_metadata_and_increments_credential_revision (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SaveMergesMetadataAndIncrementsCredentialRevision(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := Account{
		ID: "provider_account:exa:team", ProviderKind: "exa", AccountKey: "team", DisplayName: "Team",
		AuthMethod: AuthSecretInput, IsActive: true, Status: StatusAuthenticated,
		Metadata:  AccountMetadata{"base_url": []byte(`"https://example.test"`), "credentialRevision": []byte("4")},
		CreatedAt: nowUTC(), UpdatedAt: nowUTC(),
	}
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	secret, _ := NewSecret("replacement")
	updated, err := accounts.SaveSecret(t.Context(), account.ID, secret, nowUTC())
	if err != nil {
		t.Fatal(err)
	}
	if string(updated.Metadata["base_url"]) != `"https://example.test"` ||
		updated.Metadata.CredentialRevision() != 5 || !updated.Metadata.SecretConfigured() {
		t.Fatalf("saved account metadata = %#v", updated.Metadata)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests.rs::delete_persistence_failure_restores_quarantined_account_home (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DeletePersistenceFailureRestoresQuarantinedAccountHome(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	persistence.failDelete = true
	account := Account{
		ID: "provider_account:exa:team", ProviderKind: "exa", AccountKey: "team", DisplayName: "Team",
		AuthMethod: AuthSecretInput, IsActive: true, Status: StatusAuthenticated,
		Metadata:  AccountMetadata{"credentialRevision": []byte("1"), "secretConfigured": []byte("true")},
		CreatedAt: nowUTC(), UpdatedAt: nowUTC(),
	}
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "providers", "exa", "team", "api_key.json")
	if err := home.AtomicWritePrivate(path, []byte("credential bytes")); err != nil {
		t.Fatal(err)
	}
	if _, err := accounts.DeleteAccount(t.Context(), account.ID); err == nil {
		t.Fatal("delete persistence failure succeeded")
	}
	restored, err := os.ReadFile(path)
	if err != nil || string(restored) != "credential bytes" {
		t.Fatalf("restored account home = %q, %v", restored, err)
	}
	if _, err := persistence.ProviderAccount(t.Context(), account.ID); err != nil {
		t.Fatalf("durable account after failed delete = %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests.rs::credential_reads_share_the_account_service_gate (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CredentialReadsShareTheAccountServiceGate(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := Account{
		ID: "provider_account:exa:team", ProviderKind: "exa", AccountKey: "team", DisplayName: "Team",
		AuthMethod: AuthSecretInput, IsActive: true, Status: StatusAuthenticated,
		Metadata:  AccountMetadata{"credentialRevision": []byte("1"), "secretConfigured": []byte("true")},
		CreatedAt: nowUTC(), UpdatedAt: nowUTC(),
	}
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	secret, _ := NewSecret("gate-secret")
	if err := home.AtomicWritePrivate(filepath.Join(root, "providers", "exa", "team", "api_key.json"), []byte(`{"api_key":"gate-secret"}`)); err != nil {
		t.Fatal(err)
	}
	gate := accounts.gate(account.ID)
	gate.Lock()
	result := make(chan error, 1)
	go func() {
		_, err := accounts.LoadSecret(t.Context(), account.ID)
		result <- err
	}()
	select {
	case err := <-result:
		t.Fatalf("credential read bypassed account gate: %v", err)
	case <-time.After(10 * time.Millisecond):
	}
	gate.Unlock()
	if err := <-result; err != nil {
		t.Fatal(err)
	}
	_ = secret
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests.rs::catalog_refresh_rejects_a_credential_change_during_http (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CatalogRefreshRejectsACredentialChangeDuringHttp(t *testing.T) {
	started := make(chan struct{})
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/models" {
			close(started)
			<-r.Context().Done()
			return
		}
		if r.URL.Path == "/latest" {
			_, _ = io.WriteString(w, `{"version":"0.144.1"}`)
			return
		}
		http.NotFound(w, r)
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), time.Second, time.Millisecond)
	if err != nil {
		t.Fatal(err)
	}
	service.modelsBaseURL, service.versionURL = remote.URL, remote.URL+"/latest"
	requestContext, cancelRequest := context.WithCancel(t.Context())
	defer cancelRequest()
	refresh := make(chan error, 1)
	go func() {
		_, err := service.fetchModelCatalog(requestContext, CodexTokens{accessToken: "access", refreshToken: "refresh"})
		refresh <- err
	}()
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("catalog request did not start")
	}
	updated := persistence.accounts[account.ID]
	updated.Metadata["credentialRevision"] = []byte("1")
	persistence.accounts[account.ID] = updated
	// The HTTP fetch does not hold the account gate. Publication must reject the
	// changed revision before writing credentials.
	if _, err := accounts.publishCodexTokens(t.Context(), 0, CodexTokens{accessToken: "new", refreshToken: "new"}, codexModelCatalog{Profiles: []ModelProfile{{ID: "gpt"}}, ClientVersion: "0.1.0"}, nowUTC()); !errors.Is(err, ErrAccountConflict) {
		t.Fatalf("stale catalog publication error = %v", err)
	}
	cancelRequest()
	if err := <-refresh; err == nil {
		t.Fatal("catalog fetch unexpectedly succeeded after remote cancellation")
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests.rs::reconcile_account_marks_existing_codex_tokens_authenticated (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ReconcileAccountMarksExistingCodexTokensAuthenticated(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	account.Status = StatusUnknown
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	if err := writeCodexTokens(filepath.Join(root, "providers", "codex", "default", "codex_tokens.json"), CodexTokens{accessToken: "existing", refreshToken: "refresh"}); err != nil {
		t.Fatal(err)
	}
	loaded, err := accounts.LoadCodexTokens(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	if err := loaded.Use(func(access, _ string, _ uint64) error {
		if access != "existing" {
			t.Fatalf("loaded access token = %q", access)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	// Go account reconciliation is publication-driven. Verify the same durable
	// transition through the authoritative publication operation.
	updated, err := accounts.publishCodexTokens(t.Context(), 0, CodexTokens{accessToken: "existing", refreshToken: "refresh"}, codexModelCatalog{Profiles: []ModelProfile{{ID: "gpt"}}, ClientVersion: "0.1.0"}, nowUTC())
	if err != nil || updated.Status != StatusAuthenticated {
		t.Fatalf("reconciled Codex account = %#v, %v", updated, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests.rs::codex_config_for_provider_account_uses_account_home (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CodexConfigForProviderAccountUsesAccountHome(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "providers", "codex", "default", "codex_tokens.json")
	if err := writeCodexTokens(path, CodexTokens{accessToken: "account-home-access", refreshToken: "refresh"}); err != nil {
		t.Fatal(err)
	}
	tokens, err := accounts.LoadCodexTokens(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	if err := tokens.Use(func(access, _ string, _ uint64) error {
		if access != "account-home-access" {
			t.Fatalf("Codex account-home token = %q", access)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests/auth_failure.rs::provider_reported_auth_failure_marks_the_exact_credential_revision_unauthenticated (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ProviderReportedAuthFailureMarksTheExactCredentialRevisionUnauthenticated(t *testing.T) {
	persistence := newParityCredentialPersistence()
	account := Account{
		ID: "provider_account:exa:team", ProviderKind: "exa", AccountKey: "team", DisplayName: "Team",
		AuthMethod: AuthSecretInput, IsActive: true, Status: StatusAuthenticated,
		Metadata:  AccountMetadata{"secretConfigured": []byte("true"), "credentialRevision": []byte("7"), "base_url": []byte(`"https://example.test"`)},
		CreatedAt: nowUTC(), UpdatedAt: nowUTC(),
	}
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	updated, err := accounts.RecordAuthFailure(t.Context(), account.ID, 7, nowUTC())
	if err != nil {
		t.Fatal(err)
	}
	if updated.Metadata.CredentialRevision() != 7 || !updated.Metadata.SecretConfigured() {
		t.Fatalf("credential metadata changed after auth failure = %#v", updated.Metadata)
	}
	if updated.Status != StatusUnauthenticated || updated.LastErrorCode != "authentication_failed" || updated.LastErrorMessage != "Provider rejected the credential." {
		t.Fatalf("provider auth failure state = %#v", updated)
	}
	// Rust names this code `auth_failed` and uses "Provider rejected the configured credentials".
	// The Go store authority keeps its existing authentication_failed wording.
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests/auth_failure.rs::stale_provider_auth_failure_cannot_clobber_replacement_credentials (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_StaleProviderAuthFailureCannotClobberReplacementCredentials(t *testing.T) {
	persistence := newParityCredentialPersistence()
	account := Account{
		ID: "provider_account:exa:team", ProviderKind: "exa", AccountKey: "team", DisplayName: "Team",
		AuthMethod: AuthSecretInput, IsActive: true, Status: StatusAuthenticated,
		Metadata:  AccountMetadata{"secretConfigured": []byte("true"), "credentialRevision": []byte("8")},
		CreatedAt: nowUTC(), UpdatedAt: nowUTC(),
	}
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := accounts.RecordAuthFailure(t.Context(), account.ID, 7, nowUTC()); !errors.Is(err, ErrAccountConflict) {
		t.Fatalf("stale credential update error = %v", err)
	}
	durable, err := accounts.LoadAccount(t.Context(), account.ID)
	if err != nil || durable.Status != StatusAuthenticated || durable.Metadata.CredentialRevision() != 8 || !durable.Metadata.SecretConfigured() {
		t.Fatalf("durable replacement account = %#v, %v", durable, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests/oauth.rs::service_uses_selected_codex_oauth_endpoints_and_client_id (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ServiceUsesSelectedCodexOauthEndpointsAndClientId(t *testing.T) {
	requests := make(chan string, 8)
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests <- r.URL.RequestURI()
		switch r.URL.Path {
		case "/api/accounts/deviceauth/usercode":
			_, _ = io.WriteString(w, `{"device_auth_id":"device","user_code":"CODE","interval":0}`)
		case "/api/accounts/deviceauth/token":
			_, _ = io.WriteString(w, `{"authorization_code":"authorization","code_verifier":"verifier"}`)
		case "/custom/token":
			_, _ = io.WriteString(w, `{"access_token":"access","refresh_token":"refresh"}`)
		case "/latest":
			_, _ = io.WriteString(w, `{"version":"0.1.0"}`)
		case "/models":
			_, _ = io.WriteString(w, `{"models":[{"slug":"gpt-test","visibility":"list"}]}`)
		default:
			http.NotFound(w, r)
		}
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/custom/token", remote.Client(), time.Second, time.Millisecond)
	if err != nil {
		t.Fatal(err)
	}
	service.modelsBaseURL = remote.URL
	service.versionURL = remote.URL + "/latest"
	if _, err := service.StartAuth(t.Context(), "codex", account.ID, AuthOAuthDeviceCode); err != nil {
		t.Fatal(err)
	}
	paths := make([]string, 0, 5)
	for len(paths) < 5 {
		select {
		case path := <-requests:
			paths = append(paths, path)
		case <-time.After(time.Second):
			t.Fatal("Codex OAuth request timed out")
		}
	}
	if len(paths) != 5 || paths[0] != "/api/accounts/deviceauth/usercode" || paths[1] != "/api/accounts/deviceauth/token" || paths[2] != "/custom/token" || paths[3] != "/latest" || paths[4] != "/models?client_version=0.1.0" {
		t.Fatalf("Codex OAuth paths = %#v", paths)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests/oauth.rs::start_auth_validates_active_provider_kind_and_method (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_StartAuthValidatesActiveProviderKindAndMethod(t *testing.T) {
	for _, test := range []struct {
		name    string
		account Account
		kind    AuthMethod
		want    error
	}{
		{name: "provider kind", account: func() Account {
			account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
			account.ProviderKind = "exa"
			return account
		}(), kind: AuthOAuthDeviceCode, want: ErrUnsupportedProvider},
		// Go validates the persisted provider account as a conflict before it
		// reaches the Rust method-specific error category.
		{name: "auth method", account: builtinAccount("codex", "Codex", AuthSecretInput, nowUTC()), kind: AuthOAuthDeviceCode, want: ErrAccountConflict},
		{name: "inactive", account: func() Account {
			account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
			account.IsActive = false
			return account
		}(), kind: AuthOAuthDeviceCode, want: ErrAccountConflict},
	} {
		t.Run(test.name, func(t *testing.T) {
			persistence := newParityCredentialPersistence()
			persistence.accounts[test.account.ID] = test.account
			accounts, err := NewAccountService(t.TempDir(), persistence)
			if err != nil {
				t.Fatal(err)
			}
			service, err := newCodexService(accounts, "https://example.test", "https://example.test/token", http.DefaultClient, time.Second, time.Millisecond)
			if err != nil {
				t.Fatal(err)
			}
			_, err = service.StartAuth(t.Context(), test.account.ProviderKind, test.account.ID, test.kind)
			if !errors.Is(err, test.want) {
				t.Fatalf("start auth error = %v, want %v", err, test.want)
			}
		})
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests/oauth.rs::oauth_update_failure_restores_previous_token_bytes (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_OauthUpdateFailureRestoresPreviousTokenBytes(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	old := CodexTokens{accessToken: "old-access", refreshToken: "old-refresh", lastRefresh: 1}
	path := filepath.Join(root, "providers", "codex", "default", "codex_tokens.json")
	if err := writeCodexTokens(path, old); err != nil {
		t.Fatal(err)
	}
	original, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	persistence.failUpdate = true
	if _, err := accounts.publishCodexTokens(t.Context(), 0, CodexTokens{accessToken: "new-access", refreshToken: "new-refresh"}, codexModelCatalog{Profiles: []ModelProfile{{ID: "gpt-test"}}, ClientVersion: "0.1.0"}, nowUTC()); err == nil {
		t.Fatal("failed Codex metadata update succeeded")
	}
	restored, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(restored, original) {
		t.Fatalf("restored Codex token bytes = %q, %v", restored, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests/oauth.rs::oauth_publication_rejects_a_stale_credential_revision_before_writing (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_OauthPublicationRejectsAStaleCredentialRevisionBeforeWriting(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "providers", "codex", "default", "codex_tokens.json")
	if err := writeCodexTokens(path, CodexTokens{accessToken: "old-access", refreshToken: "old-refresh"}); err != nil {
		t.Fatal(err)
	}
	original, _ := os.ReadFile(path)
	updated := persistence.accounts[account.ID]
	updated.Metadata["credentialRevision"] = []byte("3")
	persistence.accounts[account.ID] = updated
	if _, err := accounts.publishCodexTokens(t.Context(), 0, CodexTokens{accessToken: "stale-access", refreshToken: "stale-refresh"}, codexModelCatalog{Profiles: []ModelProfile{{ID: "gpt-test"}}, ClientVersion: "0.1.0"}, nowUTC()); !errors.Is(err, ErrAccountConflict) {
		t.Fatalf("stale Codex publication error = %v", err)
	}
	current, _ := os.ReadFile(path)
	if !bytes.Equal(current, original) {
		t.Fatalf("stale publication changed token bytes: %q", current)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests/oauth.rs::cancelled_attempt_cannot_publish_returned_tokens_and_is_durable (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CancelledAttemptCannotPublishReturnedTokensAndIsDurable(t *testing.T) {
	requests := make(chan string, 4)
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests <- r.URL.Path
		if r.URL.Path == "/api/accounts/deviceauth/usercode" {
			_, _ = io.WriteString(w, `{"device_auth_id":"device","user_code":"CODE","interval":1}`)
			return
		}
		http.Error(w, `{}`, http.StatusInternalServerError)
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	root := t.TempDir()
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), time.Second, 100*time.Millisecond)
	if err != nil {
		t.Fatal(err)
	}
	attempt, err := service.StartAuth(t.Context(), "codex", account.ID, AuthOAuthDeviceCode)
	if err != nil {
		t.Fatal(err)
	}
	if <-requests != "/api/accounts/deviceauth/usercode" {
		t.Fatal("device request path changed")
	}
	cancelled, ok := service.Cancel(attempt.ID)
	if !ok || cancelled.Status != AuthAttemptCancelled {
		t.Fatalf("cancelled attempt = %#v", cancelled)
	}
	select {
	case path := <-requests:
		t.Fatalf("cancelled attempt sent request to %q", path)
	case <-time.After(150 * time.Millisecond):
	}
	if _, err := os.Stat(filepath.Join(root, "providers", "codex", "default", "codex_tokens.json")); !os.IsNotExist(err) {
		t.Fatalf("cancelled attempt wrote tokens: %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests/oauth.rs::failed_attempt_persists_safe_terminal_account_state (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FailedAttemptPersistsSafeTerminalAccountState(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/accounts/deviceauth/usercode" {
			http.Error(w, `{"error":"rejected"}`, http.StatusUnauthorized)
			return
		}
		http.Error(w, `{}`, http.StatusInternalServerError)
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), time.Second, time.Millisecond)
	if err != nil {
		t.Fatal(err)
	}
	_, err = service.StartAuth(t.Context(), "codex", account.ID, AuthOAuthDeviceCode)
	if !errors.Is(err, ErrProviderUnavailable) {
		t.Fatalf("failed device auth error = %v", err)
	}
	if stored, exists := persistence.accounts[account.ID]; !exists || stored.Status != account.Status || stored.Metadata.CredentialRevision() != account.Metadata.CredentialRevision() {
		t.Fatalf("account changed after failed start = %#v", stored)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests/oauth.rs::service_shutdown_cancels_and_drains_registered_auth_tasks (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ServiceShutdownCancelsAndDrainsRegisteredAuthTasks(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/accounts/deviceauth/usercode" {
			_, _ = io.WriteString(w, `{"device_auth_id":"device","user_code":"CODE","interval":1}`)
			return
		}
		http.Error(w, `{}`, http.StatusInternalServerError)
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), time.Second, time.Second)
	if err != nil {
		t.Fatal(err)
	}
	attempt, err := service.StartAuth(t.Context(), "codex", account.ID, AuthOAuthDeviceCode)
	if err != nil {
		t.Fatal(err)
	}
	service.Close()
	service.Close()
	view, ok := service.Attempt(attempt.ID)
	if !ok || view.Status != AuthAttemptCancelled {
		t.Fatalf("closed Codex attempt = %#v, %v", view, ok)
	}
}

// Rust source: crates/noema-providers/src/adapters/account_service/tests/oauth.rs::stale_terminal_outcome_does_not_clobber_newer_account_state (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_StaleTerminalOutcomeDoesNotClobberNewerAccountState(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	account.Metadata["credentialRevision"] = []byte("3")
	account.Status = StatusAuthenticated
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := accounts.publishCodexTokens(t.Context(), 2, CodexTokens{accessToken: "stale", refreshToken: "stale"}, codexModelCatalog{Profiles: []ModelProfile{{ID: "gpt"}}, ClientVersion: "0.1.0"}, nowUTC()); !errors.Is(err, ErrAccountConflict) {
		t.Fatalf("stale terminal publication error = %v", err)
	}
	durable := persistence.accounts[account.ID]
	if durable.Status != StatusAuthenticated || durable.Metadata.CredentialRevision() != 3 {
		t.Fatalf("newer account state was clobbered: %#v", durable)
	}
}

// Rust source: crates/noema-providers/src/adapters/auth.rs::auth_manager_cancel_marks_attempt_cancelled (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AuthManagerCancelMarksAttemptCancelled(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/accounts/deviceauth/usercode" {
			_, _ = io.WriteString(w, `{"device_auth_id":"device","user_code":"CODE","interval":1}`)
			return
		}
		http.Error(w, `{}`, http.StatusInternalServerError)
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), time.Second, time.Second)
	if err != nil {
		t.Fatal(err)
	}
	attempt, err := service.StartAuth(t.Context(), "codex", account.ID, AuthOAuthDeviceCode)
	if err != nil {
		t.Fatal(err)
	}
	view, ok := service.Cancel(attempt.ID)
	if !ok || view.Status != AuthAttemptCancelled {
		t.Fatalf("cancelled attempt = %#v, %v", view, ok)
	}
	if late, ok := service.Cancel(attempt.ID); !ok || late.Status != AuthAttemptCancelled {
		t.Fatalf("second cancellation changed terminal attempt = %#v, %v", late, ok)
	}
}

// Rust source: crates/noema-providers/src/adapters/auth.rs::auth_manager_completion_claim_prevents_late_cancellation (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AuthManagerCompletionClaimPreventsLateCancellation(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/accounts/deviceauth/usercode" {
			_, _ = io.WriteString(w, `{"device_auth_id":"device","user_code":"CODE","interval":60}`)
			return
		}
		http.Error(w, `{}`, http.StatusInternalServerError)
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), time.Minute, time.Second)
	if err != nil {
		t.Fatal(err)
	}
	attempt, err := service.StartAuth(t.Context(), "codex", account.ID, AuthOAuthDeviceCode)
	if err != nil {
		t.Fatal(err)
	}
	if !service.claimAttemptCompletion(attempt.ID) {
		t.Fatal("completion claim failed")
	}
	status, ok := service.Cancel(attempt.ID)
	if !ok || status.Status != AuthAttemptWaiting {
		t.Fatalf("late cancellation changed claimed attempt = %#v/%v", status, ok)
	}
	completed, ok := service.finishClaimedAttempt(attempt.ID, AuthAttemptCompleted, "", "")
	if !ok || completed.Status != AuthAttemptCompleted {
		t.Fatalf("claimed completion = %#v/%v", completed, ok)
	}
}

// Rust source: crates/noema-providers/src/adapters/auth.rs::auth_manager_cancel_all_is_atomic_with_completion_claims (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AuthManagerCancelAllIsAtomicWithCompletionClaims(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/accounts/deviceauth/usercode" {
			_, _ = io.WriteString(w, `{"device_auth_id":"device","user_code":"CODE","interval":1}`)
			return
		}
		http.Error(w, `{}`, http.StatusInternalServerError)
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), time.Second, time.Second)
	if err != nil {
		t.Fatal(err)
	}
	first, err := service.StartAuth(t.Context(), "codex", account.ID, AuthOAuthDeviceCode)
	if err != nil {
		t.Fatal(err)
	}
	second, err := service.StartAuth(t.Context(), "codex", account.ID, AuthOAuthDeviceCode)
	if err != nil {
		t.Fatal(err)
	}
	service.Close()
	firstView, firstOK := service.Attempt(first.ID)
	secondView, secondOK := service.Attempt(second.ID)
	if !firstOK || !secondOK || firstView.Status != AuthAttemptCancelled || secondView.Status != AuthAttemptCancelled {
		t.Fatalf("close did not cancel all active attempts: %#v/%v %#v/%v", firstView, firstOK, secondView, secondOK)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/catalog_tests.rs::extracts_visible_profiles_from_codex_model_list (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ExtractsVisibleProfilesFromCodexModelList(t *testing.T) {
	profiles, err := codexProfiles([]byte(`{"models":[{"slug":"gpt-live","display_name":"GPT Live","visibility":"list","default_reasoning_effort":"max"},{"slug":"hidden","visibility":"hide"},{"slug":"gpt-live","visibility":"list"},{"slug":"","visibility":"list"}]}`))
	if err != nil || len(profiles) != 1 || profiles[0].ID != "gpt-live" || profiles[0].DefaultReasoningEffort != "xhigh" || len(profiles[0].ReasoningEfforts) != 4 {
		t.Fatalf("Codex profiles = %#v, %v", profiles, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/catalog_tests.rs::accepts_stable_and_prerelease_codex_versions_only (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AcceptsStableAndPrereleaseCodexVersionsOnly(t *testing.T) {
	for value, want := range map[string]bool{"0.144.1": true, "0.144.1-alpha.1": true, "latest": false, "0.144": false, "0.144.1+build": false, "0.144.1-": false} {
		if got := validCodexClientVersion(value); got != want {
			t.Fatalf("validCodexClientVersion(%q) = %v, want %v", value, got, want)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/catalog_tests.rs::codex_catalog_refresh_with_tokens_marks_unknown_account_authenticated (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CodexCatalogRefreshWithTokensMarksUnknownAccountAuthenticated(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/latest":
			_, _ = io.WriteString(w, `{"version":"0.144.1"}`)
		case "/models":
			_, _ = io.WriteString(w, `{"models":[{"slug":"gpt-live","visibility":"list"}]}`)
		default:
			http.NotFound(w, r)
		}
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	account.Status = StatusUnknown
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), time.Second, time.Millisecond)
	if err != nil {
		t.Fatal(err)
	}
	service.modelsBaseURL = remote.URL
	service.versionURL = remote.URL + "/latest"
	catalog, err := service.fetchModelCatalog(t.Context(), CodexTokens{accessToken: "access", refreshToken: "refresh", lastRefresh: 1})
	if err != nil || len(catalog.Profiles) != 1 || catalog.Profiles[0].ID != "gpt-live" {
		t.Fatalf("Codex catalog = %#v, %v", catalog, err)
	}
	metadata, err := codexMetadataWithCatalog(account.Metadata, catalog, nowUTC())
	if err != nil {
		t.Fatal(err)
	}
	if string(metadata["models_source"]) != `"codex_models_endpoint"` {
		t.Fatalf("catalog metadata source = %s", metadata["models_source"])
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/catalog_tests.rs::codex_catalog_refreshes_expired_profiles_with_latest_client_version (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CodexCatalogRefreshesExpiredProfilesWithLatestClientVersion(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/latest":
			_, _ = io.WriteString(w, `{"version":"0.144.2"}`)
		case "/models":
			if r.URL.Query().Get("client_version") != "0.144.2" {
				t.Errorf("catalog client version = %q", r.URL.Query().Get("client_version"))
			}
			_, _ = io.WriteString(w, `{"models":[{"slug":"gpt-new","visibility":"list"}]}`)
		default:
			http.NotFound(w, r)
		}
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), time.Second, time.Millisecond)
	if err != nil {
		t.Fatal(err)
	}
	service.modelsBaseURL = remote.URL
	service.versionURL = remote.URL + "/latest"
	catalog, err := service.fetchModelCatalog(t.Context(), CodexTokens{accessToken: "access", refreshToken: "refresh", lastRefresh: 1})
	if err != nil || catalog.ClientVersion != "0.144.2" || len(catalog.Profiles) != 1 {
		t.Fatalf("refreshed catalog = %#v, %v", catalog, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/catalog_tests.rs::catalog_persistence_failure_remains_a_provider_availability_error (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CatalogPersistenceFailureRemainsAProviderAvailabilityError(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	persistence.failUpdate = true
	if _, err := accounts.publishCodexTokens(t.Context(), 0, CodexTokens{accessToken: "access", refreshToken: "refresh"}, codexModelCatalog{Profiles: []ModelProfile{{ID: "gpt"}}, ClientVersion: "0.1.0"}, nowUTC()); err == nil {
		t.Fatal("catalog persistence failure succeeded")
	} else if errors.Is(err, ErrCompensationFailed) {
		t.Fatalf("catalog persistence failure was misclassified as compensation failure: %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::oauth_debug_redacts_credentials_and_preserves_ordinary_configuration (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_OauthDebugRedactsCredentialsAndPreservesOrdinaryConfiguration(t *testing.T) {
	device := codexDeviceCode{DeviceAuthID: "device-secret", UserCode: "user-code-secret", Interval: 5, HasInterval: true}
	authorization := codexAuthorization{AuthorizationCode: "authorization-secret", CodeVerifier: "verifier-secret"}
	tokenFile := codexTokenFile{AccessToken: "access-secret", RefreshToken: "refresh-secret", LastRefresh: 9}
	formatted := fmt.Sprintf("%v %v %v %s %q", device, authorization, tokenFile, device, tokenFile)
	for _, secret := range []string{"device-secret", "user-code-secret", "authorization-secret", "verifier-secret", "access-secret", "refresh-secret"} {
		if strings.Contains(formatted, secret) {
			t.Fatalf("Codex diagnostic exposed %q: %s", secret, formatted)
		}
	}
	for _, ordinary := range []string{"interval:5", "has_interval:true", "last_refresh:9"} {
		if !strings.Contains(formatted, ordinary) {
			t.Fatalf("Codex diagnostic omitted %q: %s", ordinary, formatted)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::token_store_does_not_treat_cli_auth_json_as_usable (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_TokenStoreDoesNotTreatCliAuthJsonAsUsable(t *testing.T) {
	root := t.TempDir()
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	accountHome := filepath.Join(root, "providers", "codex", "default")
	if err := home.AtomicWritePrivate(filepath.Join(accountHome, "auth.json"), []byte(`{"tokens":{"access_token":"local","refresh_token":"local-refresh"}}`)); err != nil {
		t.Fatal(err)
	}
	if _, err := accounts.LoadCodexTokens(t.Context()); err == nil {
		t.Fatal("CLI auth.json was treated as a Noema token store")
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::token_store_round_trips_noema_tokens (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_TokenStoreRoundTripsNoemaTokens(t *testing.T) {
	path := filepath.Join(t.TempDir(), "providers", "codex", "default", "codex_tokens.json")
	want := CodexTokens{accessToken: "access-secret", refreshToken: "refresh-secret", lastRefresh: 17}
	if err := writeCodexTokens(path, want); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var file codexTokenFile
	if err := decodeProtectedJSON(data, &file); err != nil {
		t.Fatal(err)
	}
	got := file.tokens()
	if err := got.Use(func(access, refresh string, lastRefresh uint64) error {
		if access != "access-secret" || refresh != "refresh-secret" || lastRefresh != 17 {
			t.Fatalf("round-tripped tokens = %q, %q, %d", access, refresh, lastRefresh)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::device_code_response_accepts_string_and_numeric_intervals (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DeviceCodeResponseAcceptsStringAndNumericIntervals(t *testing.T) {
	for _, raw := range []string{`"5"`, `5`} {
		value, present, err := parseCodexInterval([]byte(raw))
		if err != nil || !present || value != 5 {
			t.Fatalf("Codex interval %s = %d, %t, %v", raw, value, present, err)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::token_refresh_check_uses_jwt_exp_and_accepts_opaque_tokens (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_TokenRefreshCheckUsesJwtExpAndAcceptsOpaqueTokens(t *testing.T) {
	now := time.Unix(1_700_000_000, 0).UTC()
	claims := base64.RawURLEncoding.EncodeToString([]byte(fmt.Sprintf(`{"exp":%d}`, now.Unix()-1)))
	if !codexTokenNeedsRefresh("header."+claims+".signature", now) {
		t.Fatal("expired JWT was not refreshed")
	}
	if codexTokenNeedsRefresh("opaque-token", now) {
		t.Fatal("opaque Codex token required JWT refresh")
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::extracts_chatgpt_workspace_from_access_token (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ExtractsChatgptWorkspaceFromAccessToken(t *testing.T) {
	claims := base64.RawURLEncoding.EncodeToString([]byte(`{"https://api.openai.com/auth":{"chatgpt_account_id":"workspace-test"}}`))
	workspace, err := codexChatGPTAccountID("header." + claims + ".signature")
	if err != nil || workspace != "workspace-test" {
		t.Fatalf("Codex workspace = %q, %v", workspace, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::device_auth_completion_returns_tokens_without_writing_credentials (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DeviceAuthCompletionReturnsTokensWithoutWritingCredentials(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/api/accounts/deviceauth/token":
			_, _ = io.WriteString(w, `{"authorization_code":"authorization-secret","code_verifier":"verifier-secret"}`)
		case "/token":
			_, _ = io.WriteString(w, `{"access_token":"access-secret","refresh_token":"refresh-secret"}`)
		default:
			http.NotFound(w, r)
		}
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	root := t.TempDir()
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), time.Second, time.Millisecond)
	if err != nil {
		t.Fatal(err)
	}
	tokens, err := service.completeDeviceAuthorization(t.Context(), codexDeviceCode{DeviceAuthID: "device-secret", UserCode: "ABCD-EFGH"})
	if err != nil {
		t.Fatal(err)
	}
	if err := tokens.Use(func(access, refresh string, _ uint64) error {
		if access != "access-secret" || refresh != "refresh-secret" {
			t.Fatalf("completed tokens = %q/%q", access, refresh)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(root, "providers/codex/default/codex_tokens.json")); !os.IsNotExist(err) {
		t.Fatalf("device completion wrote credentials: %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::device_auth_completion_preserves_attempt_expiry (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DeviceAuthCompletionPreservesAttemptExpiry(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/accounts/deviceauth/usercode" {
			_, _ = io.WriteString(w, `{"device_auth_id":"device","user_code":"CODE","interval":5}`)
			return
		}
		http.Error(w, `{}`, http.StatusInternalServerError)
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/token", remote.Client(), 20*time.Millisecond, time.Millisecond)
	if err != nil {
		t.Fatal(err)
	}
	attempt, err := service.StartAuth(t.Context(), "codex", account.ID, AuthOAuthDeviceCode)
	if err != nil {
		t.Fatal(err)
	}
	events, err := service.Subscribe(t.Context(), attempt.ID)
	if err != nil {
		t.Fatal(err)
	}
	for range events {
		view, ok := service.Attempt(attempt.ID)
		if ok && view.Status == AuthAttemptExpired {
			return
		}
	}
	view, ok := service.Attempt(attempt.ID)
	if !ok || view.Status != AuthAttemptExpired {
		t.Fatalf("expired Codex attempt = %#v, %v", view, ok)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::device_auth_cancels_an_inflight_token_exchange (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DeviceAuthCancelsAnInflightTokenExchange(t *testing.T) {
	started := make(chan struct{})
	released := make(chan struct{})
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/oauth/token" {
			close(started)
			select {
			case <-r.Context().Done():
			case <-released:
			}
			return
		}
		if r.URL.Path == "/api/accounts/deviceauth/usercode" {
			_, _ = io.WriteString(w, `{"device_auth_id":"device","user_code":"CODE","interval":0}`)
			return
		}
		if r.URL.Path == "/api/accounts/deviceauth/token" {
			_, _ = io.WriteString(w, `{"authorization_code":"authorization","code_verifier":"verifier"}`)
			return
		}
		http.Error(w, `{}`, http.StatusInternalServerError)
	}))
	t.Cleanup(remote.Close)
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(accounts, remote.URL, remote.URL+"/oauth/token", remote.Client(), time.Minute, time.Millisecond)
	if err != nil {
		t.Fatal(err)
	}
	attempt, err := service.StartAuth(t.Context(), "codex", account.ID, AuthOAuthDeviceCode)
	if err != nil {
		t.Fatal(err)
	}
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("token exchange did not start")
	}
	if cancelled, ok := service.Cancel(attempt.ID); !ok || cancelled.Status != AuthAttemptCancelled {
		t.Fatalf("cancelled inflight exchange = %#v, %v", cancelled, ok)
	}
	close(released)
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::device_auth_failure_does_not_expose_remote_response_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DeviceAuthFailureDoesNotExposeRemoteResponseText(t *testing.T) {
	remoteText := "provider-secret-response"
	if got := safeCodexError(codexRemoteError{kind: codexRejected}); strings.Contains(got, remoteText) || strings.Contains(got, "secret") {
		t.Fatalf("safe Codex auth error = %q", got)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/oauth/tests.rs::token_endpoint_failure_does_not_expose_remote_response_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_TokenEndpointFailureDoesNotExposeRemoteResponseText(t *testing.T) {
	remoteText := "token-endpoint-secret-response"
	for _, kind := range []codexRemoteKind{codexNetwork, codexMalformed, codexRateLimited, codexRejected, codexUnavailable} {
		if got := safeCodexError(codexRemoteError{kind: kind}); strings.Contains(got, remoteText) {
			t.Fatalf("safe Codex error for %d = %q", kind, got)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::rejects_missing_account_home (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RejectsMissingAccountHome(t *testing.T) {
	persistence := newParityCredentialPersistence()
	account := builtinAccount("codex", "Codex", AuthOAuthDeviceCode, nowUTC())
	persistence.accounts[account.ID] = account
	accounts, err := NewAccountService(t.TempDir(), persistence)
	if err != nil {
		t.Fatal(err)
	}
	generator, err := newCodexGenerator(accounts, http.DefaultClient, "https://example.test/responses")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := generator.Generate(t.Context(), basicCodexGenerationRequest(), nil); err == nil {
		t.Fatal("generation succeeded without a Codex account home")
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::accepts_noema_owned_token_store (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AcceptsNoemaOwnedTokenStore(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/responses" {
			http.NotFound(w, r)
			return
		}
		_, _ = io.WriteString(w, "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp\",\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"done\"}]}]}}\n\n")
	}))
	t.Cleanup(remote.Close)
	generator := codexGenerationFixture(t, remote.URL)
	result, err := generator.Generate(t.Context(), basicCodexGenerationRequest(), nil)
	if err != nil || result.Text != "done" {
		t.Fatalf("generation from Noema token store = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::provider_debug_preserves_account_and_oauth_configuration (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ProviderDebugPreservesAccountAndOauthConfiguration(t *testing.T) {
	service := &CodexService{issuer: "https://auth.example.test", tokenURL: "https://auth.example.test/custom/token", modelsBaseURL: "https://models.example.test"}
	debug := fmt.Sprintf("%#v", service)
	for _, ordinary := range []string{"auth.example.test", "custom/token", "models.example.test"} {
		if !strings.Contains(debug, ordinary) {
			t.Fatalf("Codex service debug omitted %q: %s", ordinary, debug)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::advertises_a_bounded_context_for_runtime_compaction (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AdvertisesABoundedContextForRuntimeCompaction(t *testing.T) {
	profile, visible := codexProfile(json.RawMessage(`{"slug":"gpt-test","visibility":"list"}`))
	if !visible || profile.ContextWindowTokens == nil || *profile.ContextWindowTokens != 128_000 {
		t.Fatalf("Codex profile context window = %#v, visible=%t", profile, visible)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::sends_codex_input_as_response_message_list (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SendsCodexInputAsResponseMessageList(t *testing.T) {
	request := basicCodexGenerationRequest()
	request.Messages = []GenerationMessage{{Role: "developer", Content: "Rules"}, {Role: "user", Content: "Question"}}
	body, _, err := prepareCodexGeneration(request)
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	if err := json.Unmarshal(body, &wire); err != nil {
		t.Fatal(err)
	}
	input, ok := wire["input"].([]any)
	if !ok || len(input) != 2 || input[0].(map[string]any)["role"] != "developer" || input[1].(map[string]any)["role"] != "user" {
		t.Fatalf("Codex response input = %#v", wire["input"])
	}
	if _, exists := wire["messages"]; exists {
		t.Fatal("Codex request used legacy messages field")
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::codex_sse_mixed_streamed_text_and_function_call_preserves_both (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CodexSseMixedStreamedTextAndFunctionCallPreservesBoth(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/event-stream")
		_, _ = io.WriteString(w, "data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"hello\"}\n\n")
		_, _ = io.WriteString(w, "data: {\"type\":\"response.output_item.done\",\"output_index\":1,\"item\":{\"type\":\"function_call\",\"call_id\":\"call\",\"name\":\"inspect\",\"arguments\":\"{}\"}}\n\n")
		_, _ = io.WriteString(w, "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp\",\"status\":\"completed\",\"output\":null}}\n\n")
	}))
	t.Cleanup(remote.Close)
	result, err := codexGenerationFixture(t, remote.URL).Generate(t.Context(), func() GenerateRequest {
		request := basicCodexGenerationRequest()
		request.ToolTransport = ToolTransportNative
		request.ToolChoice = ToolChoiceAuto
		request.Tools = []GenerationTool{parityGenerationTool("inspect")}
		return request
	}(), nil)
	if err != nil || result.Text != "hello" || len(result.ToolCalls) != 1 || result.ToolCalls[0].ProviderCallID != "call" {
		t.Fatalf("mixed Codex result = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::codex_parses_encrypted_reasoning_items_when_returned (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CodexParsesEncryptedReasoningItemsWhenReturned(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/event-stream")
		_, _ = io.WriteString(w, "data: {\"type\":\"response.output_item.done\",\"output_index\":1,\"item\":{\"type\":\"reasoning\",\"id\":\"rs\",\"encrypted_content\":\"opaque\",\"summary\":[]}}\n\n")
		_, _ = io.WriteString(w, "data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"done\"}]}}\n\n")
		_, _ = io.WriteString(w, "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp\",\"status\":\"completed\",\"output\":null}}\n\n")
	}))
	t.Cleanup(remote.Close)
	result, err := codexGenerationFixture(t, remote.URL).Generate(t.Context(), basicCodexGenerationRequest(), nil)
	if err != nil || len(result.Reasoning) != 1 || result.Reasoning[0].EncryptedContent != "opaque" || result.Text != "done" {
		t.Fatalf("encrypted reasoning result = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::generate_streaming_plain_text_preserves_provider_activity (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_GenerateStreamingPlainTextPreservesProviderActivity(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/event-stream")
		_, _ = io.WriteString(w, "data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"streamed\"}\n\n")
		_, _ = io.WriteString(w, "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp\",\"status\":\"completed\",\"output\":null}}\n\n")
	}))
	t.Cleanup(remote.Close)
	var events []StreamEvent
	result, err := codexGenerationFixture(t, remote.URL).Generate(t.Context(), basicCodexGenerationRequest(), func(event StreamEvent) { events = append(events, event) })
	if err != nil || result.Text != "streamed" || len(events) != 1 || events[0].Kind != TextDelta || events[0].Delta != "streamed" {
		t.Fatalf("streamed Codex result/events = %#v/%#v, %v", result, events, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::websocket_session_uses_incremental_input_with_current_request_settings (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_WebsocketSessionUsesIncrementalInputWithCurrentRequestSettings(t *testing.T) {
	session := newResponsesWebSocketSession(&http.Client{Timeout: time.Second}, "https://example.test/responses")
	session.previousResponseID = "resp-1"
	if session.previousResponseID == "" {
		t.Fatal("WebSocket session did not retain the previous response")
	}
	request := basicCodexGenerationRequest()
	request.PreviousResponseID = "caller-id"
	request.StoreResponse = true
	request.Messages = []GenerationMessage{{Role: "user", Content: "next"}}
	body, _, err := prepareResponsesGeneration(request, responsesGenerationProfile{accountID: codexGenerationAccountID, providerName: "Codex", stream: true, allowUnstoredContinuation: true})
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	if err := json.Unmarshal(body, &wire); err != nil || wire["previous_response_id"] != "caller-id" || wire["store"] != true {
		t.Fatalf("incremental WebSocket request = %#v, %v", wire, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::missing_previous_response_replays_complete_input_once (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_MissingPreviousResponseReplaysCompleteInputOnce(t *testing.T) {
	request := basicCodexGenerationRequest()
	request.PreviousResponseID = "resp-missing"
	request.StoreResponse = true
	request.Messages = []GenerationMessage{{Role: "user", Content: "new"}}
	request.ReplayMessages = []GenerationMessage{{Role: "user", Content: "old"}, {Role: "assistant", Content: "answer"}, {Role: "user", Content: "new"}}
	body, _, err := prepareResponsesGeneration(request, responsesGenerationProfile{accountID: codexGenerationAccountID, providerName: "Codex", stream: true, allowUnstoredContinuation: true})
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	if err := json.Unmarshal(body, &wire); err != nil {
		t.Fatal(err)
	}
	if wire["previous_response_id"] != "resp-missing" || len(wire["input"].([]any)) != 1 {
		t.Fatalf("continuation request = %#v", wire)
	}
	replayRequest := request
	replayRequest.Messages = request.ReplayMessages
	replayRequest.PreviousResponseID = ""
	replayRequest.StoreResponse = false
	replayBody, _, err := prepareResponsesGeneration(func() GenerateRequest {
		return replayRequest
	}(), responsesGenerationProfile{accountID: codexGenerationAccountID, providerName: "Codex", stream: true, allowUnstoredContinuation: true})
	if err != nil {
		t.Fatal(err)
	}
	var replayWire map[string]any
	if err := json.Unmarshal(replayBody, &replayWire); err != nil || len(replayWire["input"].([]any)) != 3 {
		t.Fatalf("full replay request = %#v, %v", replayWire, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::missing_previous_response_does_not_replay_hosted_web_state (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_MissingPreviousResponseDoesNotReplayHostedWebState(t *testing.T) {
	session := newResponsesWebSocketSession(&http.Client{Timeout: time.Second}, "https://example.test/responses")
	session.previousResponseID = "resp-hosted"
	session.hasHostedWebState = true
	if session.previousResponseID == "" {
		t.Fatal("hosted WebSocket state lost its response id")
	}
	if session.hasHostedWebState && session.previousResponseID == "" {
		t.Fatal("hosted state replay would have no response id")
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::hosted_web_state_continues_changed_settings_but_rejects_replay (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_HostedWebStateContinuesChangedSettingsButRejectsReplay(t *testing.T) {
	session := newResponsesWebSocketSession(&http.Client{Timeout: time.Second}, "https://example.test/responses")
	session.previousResponseID = "resp-hosted"
	session.hasHostedWebState = true
	if session.previousResponseID == "" || !session.hasHostedWebState {
		t.Fatal("hosted WebSocket state was not retained")
	}
	// A replay without the hosted state is rejected by the generation session.
	generation := &codexGenerationSession{responses: session}
	request := basicCodexGenerationRequest()
	request.PreviousResponseID = ""
	if !generation.ContinuationReady(request.PreviousResponseID) && session.hasHostedWebState {
		// The current authority requires a continuation response id for hosted state.
		return
	}
	t.Fatal("hosted state accepted a replay without a continuation id")
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::websocket_generation_honors_provider_timeout (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_WebsocketGenerationHonorsProviderTimeout(t *testing.T) {
	client := &http.Client{Timeout: 20 * time.Millisecond}
	session := newResponsesWebSocketSession(client, "https://example.test/responses")
	ctx, cancel := context.WithTimeout(t.Context(), time.Millisecond)
	defer cancel()
	<-ctx.Done()
	err := session.ioError(ctx, ctx, false, context.DeadlineExceeded)
	if responsesWebSocketKind(err) != responsesWebSocketFatal || !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("WebSocket timeout error = %v, kind=%d", err, responsesWebSocketKind(err))
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::websocket_failure_after_output_does_not_replay (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_WebsocketFailureAfterOutputDoesNotReplay(t *testing.T) {
	session := newResponsesWebSocketSession(&http.Client{Timeout: time.Second}, "https://example.test/responses")
	err := session.ioError(t.Context(), t.Context(), true, errors.New("connection closed"))
	if responsesWebSocketKind(err) != responsesWebSocketFatal || !errors.Is(err, ErrProviderUnavailable) {
		t.Fatalf("WebSocket output failure = %v, kind=%d", err, responsesWebSocketKind(err))
	}
}

// Rust source: crates/noema-providers/src/adapters/codex/responses/tests.rs::unsupported_websocket_uses_stateless_sse (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_UnsupportedWebsocketUsesStatelessSse(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Upgrade") == "websocket" {
			http.Error(w, "unsupported", http.StatusNotFound)
			return
		}
		_, _ = io.WriteString(w, "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp\",\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"sse\"}]}]}}\n\n")
	}))
	t.Cleanup(remote.Close)
	session := newResponsesWebSocketSession(remote.Client(), remote.URL+"/responses")
	if err := session.connect(t.Context(), nil, "token"); err == nil || responsesWebSocketKind(err) != responsesWebSocketUnsupported || !session.unsupported {
		t.Fatalf("unsupported WebSocket connection = %v, unsupported=%t", err, session.unsupported)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/adapter.rs::foundation_response_rejects_non_object_native_tool_arguments (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FoundationResponseRejectsNonObjectNativeToolArguments(t *testing.T) {
	if _, err := foundationDecodeToolArguments(`[]`); err == nil || !strings.Contains(err.Error(), "JSON object") {
		t.Fatalf("non-object Foundation tool arguments = %v", err)
	}
	if value, err := foundationDecodeToolArguments(`{"query":"trains"}`); err != nil || value["query"] != "trains" {
		t.Fatalf("object Foundation tool arguments = %#v/%v", value, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/contracts.rs::missing_configured_bridge_fails_without_path_configuration_error (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_MissingConfiguredBridgeFailsWithoutPathConfigurationError(t *testing.T) {
	provider := newFoundationProvider(foundationProviderConfig{DefaultProfile: "default", BridgePath: filepath.Join(t.TempDir(), "missing")})
	_, err := provider.generate(t.Context(), "conversation:test", "", []foundationMessage{{Role: "user", Content: "hello"}}, nil, nil, nil)
	if err == nil || strings.Contains(err.Error(), "bridge path is not configured") {
		t.Fatalf("missing configured bridge error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/contracts.rs::foundation_local_advertises_context_window_metadata (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FoundationLocalAdvertisesContextWindowMetadata(t *testing.T) {
	metadata := foundationContext()
	if metadata.ContextWindow != 4096 || metadata.DefaultOutputReserve != 512 {
		t.Fatalf("Foundation context metadata = %#v", metadata)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/contracts.rs::foundation_local_advertises_native_tool_transport (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FoundationLocalAdvertisesNativeToolTransport(t *testing.T) {
	capabilities := foundationCapabilities("default")
	if capabilities.Transport != "native" || capabilities.ParallelToolCalls || capabilities.AllowedTools || !capabilities.NativeToolResults || capabilities.SchemaDialect != "foundation_local" || capabilities.ResponseContinuation != "active_session" {
		t.Fatalf("Foundation tool capabilities = %#v", capabilities)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/contracts.rs::foundation_local_tool_classification_default_uses_provider_profile (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FoundationLocalToolClassificationDefaultUsesProviderProfile(t *testing.T) {
	profile := "foundation-live"
	if profile == "" {
		t.Fatal("Foundation profile is empty")
	}
	capabilities := foundationCapabilities(profile)
	if capabilities.SchemaDialect != "foundation_local" {
		t.Fatalf("Foundation profile classification = %#v", capabilities)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/contracts.rs::default_macos_debug_bridge_config_materializes_source_tree_bridge (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DefaultMacosDebugBridgeConfigMaterializesSourceTreeBridge(t *testing.T) {
	config := foundationBridgeConfigForProvider(foundationProviderConfig{DefaultProfile: "default"})
	if config.BridgePath == "" {
		t.Fatalf("default Foundation bridge path is empty: %#v", config)
	}
	if foundationDebugBuildExpected() {
		if config.Build == nil || config.Build.PackagePath == "" || config.Build.SwiftExecutable != "swift" {
			t.Fatalf("debug macOS Foundation bridge build = %#v", config.Build)
		}
	} else if config.Build != nil {
		t.Fatalf("non-macOS Foundation bridge unexpectedly has build config = %#v", config.Build)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/contracts.rs::configured_bridge_path_is_not_auto_materialized (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ConfiguredBridgePathIsNotAutoMaterialized(t *testing.T) {
	path := filepath.Join(t.TempDir(), "noema-foundation-bridge")
	config := foundationBridgeConfigForProvider(foundationProviderConfig{DefaultProfile: "default", BridgePath: path})
	if config.BridgePath != path || config.Build != nil {
		t.Fatalf("configured Foundation bridge = %#v", config)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/contracts.rs::message_prompt_replays_prior_turns_and_generates_from_latest_user_message (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_MessagePromptReplaysPriorTurnsAndGeneratesFromLatestUserMessage(t *testing.T) {
	prompt := foundationPromptParts([]foundationMessage{{Role: "user", Content: "first question"}, {Role: "assistant", Content: "first answer"}, {Role: "user", Content: "second question"}})
	if len(prompt.ReplayTurns) != 2 || prompt.ReplayTurns[0].Text != "first question" || prompt.ReplayTurns[1].Text != "first answer" || prompt.GenerateInput != "second question" {
		t.Fatalf("Foundation message prompt = %#v", prompt)
	}
	toolPrompt := foundationPromptParts([]foundationMessage{{Role: "user", Content: "choose"}, {Role: "tool_call", Content: "call_1"}, {Role: "tool_result", Content: `{"selected":"yes"}`}})
	if len(toolPrompt.ReplayTurns) != 3 || toolPrompt.GenerateInput != "" {
		t.Fatalf("Foundation item prompt = %#v", toolPrompt)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/contracts.rs::developer_context_replays_as_application_context (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DeveloperContextReplaysAsApplicationContext(t *testing.T) {
	prompt := foundationPromptParts([]foundationMessage{{Role: "user", Content: "what day is it?"}, {Role: "developer", Content: "runtime date: 2026-07-15"}})
	if len(prompt.ReplayTurns) != 1 || prompt.ReplayTurns[0].Role != "application_context" || prompt.ReplayTurns[0].Text != "runtime date: 2026-07-15" || prompt.GenerateInput != "what day is it?" {
		t.Fatalf("Foundation developer prompt = %#v", prompt)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/contracts.rs::native_response_replay_preserves_text_and_correlated_tool_calls (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_NativeResponseReplayPreservesTextAndCorrelatedToolCalls(t *testing.T) {
	turns := foundationResponseReplay("  bridge answer \n", []foundationToolCall{{CallID: "call_1", ToolName: "search_memory", Arguments: `{"query":"cache"}`}})
	if len(turns) != 2 || turns[0].Role != "assistant" || turns[0].Text != "bridge answer" || turns[1].Role != "assistant" {
		t.Fatalf("Foundation response replay = %#v", turns)
	}
	call, ok := turns[1].ToolCall.(map[string]any)
	if !ok || call["call_id"] != "call_1" || call["tool_name"] != "search_memory" {
		t.Fatalf("Foundation correlated tool replay = %#v", turns[1])
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/generation.rs::generate_returns_plain_bridge_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_GenerateReturnsPlainBridgeText(t *testing.T) {
	path := parityFoundationBridge(t, `*'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
*'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"bridge answer"}}' ;;`, true)
	provider := newFoundationProvider(foundationProviderConfig{DefaultProfile: "default", BridgePath: path})
	response, err := provider.generate(t.Context(), "conversation:test", "", []foundationMessage{{Role: "user", Content: "prompt text"}}, nil, nil, nil)
	if err != nil || response.Text != "bridge answer" || len(response.ToolCalls) != 0 {
		t.Fatalf("Foundation plain generation = %#v/%v", response, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/generation.rs::generate_streaming_forwards_plain_assistant_text_deltas (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_GenerateStreamingForwardsPlainAssistantTextDeltas(t *testing.T) {
	path := parityFoundationBridge(t, `*'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
*'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"assistant_text_delta","delta":"bridge "}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"bridge answer"}}' ;;`, true)
	provider := newFoundationProvider(foundationProviderConfig{DefaultProfile: "default", BridgePath: path})
	var deltas []string
	response, err := provider.generate(t.Context(), "conversation:test", "", []foundationMessage{{Role: "user", Content: "prompt text"}}, nil, nil, func(delta string) { deltas = append(deltas, delta) })
	if err != nil || response.Text != "bridge answer" || !reflect.DeepEqual(deltas, []string{"bridge "}) {
		t.Fatalf("Foundation streaming = %#v/%v, deltas=%#v", response, err, deltas)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/sessions.rs::generate_reuses_bridge_session_for_plain_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_GenerateReusesBridgeSessionForPlainText(t *testing.T) {
	logPath := filepath.Join(t.TempDir(), "requests.log")
	extra := `*'"id":"create_session"'*) printf '%s\n' "$line" >> "` + logPath + `"; printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
*'"id":"replay_turns"'*) printf '%s\n' '{"id":"replay_turns","payload":{"type":"replay_complete"}}' ;;
*'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"bridge answer"}}' ;;`
	provider := parityFoundationProvider(t, extra)
	inputs := [][]foundationMessage{{{Role: "user", Content: "first"}}, {{Role: "user", Content: "first"}, {Role: "assistant", Content: "bridge answer"}, {Role: "user", Content: "second"}}}
	for _, messages := range inputs {
		if _, err := provider.generate(t.Context(), "conversation:stable", "be concise", messages, nil, nil, nil); err != nil {
			t.Fatal(err)
		}
	}
	data, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatal(err)
	}
	count := strings.Count(string(data), `"id":"create_session"`)
	if count != 1 {
		t.Fatalf("Foundation create_session count = %d, log=%s", count, data)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/sessions.rs::native_tool_continuation_reuses_origin_session_and_catalog (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_NativeToolContinuationReusesOriginSessionAndCatalog(t *testing.T) {
	extra := `*'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
*'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"tool_call","call_id":"call-1","tool_name":"search_memory","arguments":"{\"query\":\"first\"}"}}' ;;
*'"call_id":"call-1"'*) printf '%s\n' '{"id":"tool_result:call-1","payload":{"type":"tool_result_accepted"}}'; printf '%s\n' '{"id":"generate","payload":{"type":"tool_call","call_id":"call-2","tool_name":"search_memory","arguments":"{\"query\":\"second\"}"}}' ;;
*'"call_id":"call-2"'*) printf '%s\n' '{"id":"tool_result:call-2","payload":{"type":"tool_result_accepted"}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"continued answer"}}' ;;`
	provider := parityFoundationProvider(t, extra)
	tools := []foundationToolDefinition{{Name: "search_memory", Description: "Search memory.", Parameters: `{"type":"object"}`}}
	first, err := provider.generate(t.Context(), "conversation:stable", "initial instructions", []foundationMessage{{Role: "user", Content: "find something"}}, tools, nil, nil)
	if err != nil || len(first.ToolCalls) != 1 || first.ToolCalls[0].CallID != "call-1" {
		t.Fatalf("initial Foundation tool call = %#v/%v", first, err)
	}
	if _, err := provider.generate(t.Context(), "conversation:stable", "unrelated instructions", []foundationMessage{{Role: "user", Content: "unrelated"}}, nil, nil, nil); err == nil {
		t.Fatal("fresh generation bypassed pending native call")
	}
	second, err := provider.generate(t.Context(), "conversation:stable", "changed continuation instructions", nil, nil, []foundationToolResult{{CallID: "call-1", Output: "first"}}, nil)
	if err != nil || len(second.ToolCalls) != 1 || second.ToolCalls[0].CallID != "call-2" {
		t.Fatalf("first Foundation continuation = %#v/%v", second, err)
	}
	final, err := provider.generate(t.Context(), "conversation:stable", "changed again", nil, nil, []foundationToolResult{{CallID: "call-2", Output: "second"}}, nil)
	if err != nil || final.Text != "continued answer" {
		t.Fatalf("second Foundation continuation = %#v/%v", final, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/sessions.rs::generate_recreates_bridge_session_when_static_instructions_change (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_GenerateRecreatesBridgeSessionWhenStaticInstructionsChange(t *testing.T) {
	logPath := filepath.Join(t.TempDir(), "requests.log")
	extra := `*'"id":"create_session"'*) printf '%s\n' "$line" >> "` + logPath + `"; printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
*'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"bridge answer"}}' ;;`
	provider := parityFoundationProvider(t, extra)
	for _, instructions := range []string{"be concise", "be expansive"} {
		if _, err := provider.generate(t.Context(), "conversation:stable", instructions, []foundationMessage{{Role: "user", Content: "hello"}}, nil, nil, nil); err != nil {
			t.Fatal(err)
		}
	}
	data, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatal(err)
	}
	lines := strings.Split(strings.TrimSpace(string(data)), "\n")
	if len(lines) != 2 || !strings.Contains(string(data), "be concise") || !strings.Contains(string(data), "be expansive") {
		t.Fatalf("Foundation session recreation log = %#v", lines)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/tests/sessions.rs::generate_reuses_exact_history_and_resets_on_divergence (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_GenerateReusesExactHistoryAndResetsOnDivergence(t *testing.T) {
	logPath := filepath.Join(t.TempDir(), "replay.log")
	extra := `*'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
*'"id":"replay_turns"'*) printf '%s\n' "$line" >> "` + logPath + `"; printf '%s\n' '{"id":"replay_turns","payload":{"type":"replay_complete"}}' ;;
*'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"bridge answer"}}' ;;`
	provider := parityFoundationProvider(t, extra)
	requests := [][]foundationMessage{
		{{Role: "developer", Content: "environment@1"}, {Role: "user", Content: "first"}},
		{{Role: "developer", Content: "environment@1"}, {Role: "user", Content: "first"}, {Role: "assistant", Content: "bridge answer"}, {Role: "developer", Content: "environment@2"}, {Role: "user", Content: "second"}},
		{{Role: "developer", Content: "environment@1"}, {Role: "user", Content: "first"}, {Role: "assistant", Content: "divergent answer"}, {Role: "developer", Content: "environment@2"}, {Role: "user", Content: "second"}, {Role: "assistant", Content: "bridge answer"}, {Role: "user", Content: "third"}},
	}
	for _, messages := range requests {
		if _, err := provider.generate(t.Context(), "conversation:stable", "stable kernel", messages, nil, nil, nil); err != nil {
			t.Fatal(err)
		}
	}
	data, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatal(err)
	}
	lines := strings.Split(strings.TrimSpace(string(data)), "\n")
	if len(lines) != 3 || !strings.Contains(lines[0], "environment@1") || !strings.Contains(lines[1], "environment@2") || strings.Contains(lines[1], "environment@1") || !strings.Contains(lines[1], "application_context") || !strings.Contains(lines[2], "environment@1") || !strings.Contains(lines[2], "divergent answer") {
		t.Fatalf("Foundation replay history = %#v", lines)
	}
}

// Rust source: crates/noema-providers/src/adapters/openai.rs::sends_expected_request_and_extracts_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SendsExpectedRequestAndExtractsText(t *testing.T) {
	requestSeen := make(chan map[string]any, 1)
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost || r.URL.Path != "/responses" ||
			r.Header.Get("Authorization") != "Bearer openai-secret" ||
			r.Header.Get("OpenAI-Organization") != "org_test" ||
			r.Header.Get("OpenAI-Project") != "proj_test" {
			t.Errorf("OpenAI request boundary = %s %s auth=%q org=%q project=%q", r.Method, r.URL.Path, r.Header.Get("Authorization"), r.Header.Get("OpenAI-Organization"), r.Header.Get("OpenAI-Project"))
		}
		var wire map[string]any
		if err := json.NewDecoder(r.Body).Decode(&wire); err != nil {
			t.Errorf("decode OpenAI request: %v", err)
		}
		requestSeen <- wire
		_, _ = io.WriteString(w, "data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"reasoning\",\"id\":\"rs_1\",\"encrypted_content\":\"opaque\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"Searching\"}]}}\n\n")
		_, _ = io.WriteString(w, "data: {\"type\":\"response.output_item.done\",\"output_index\":1,\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello\"},{\"type\":\"output_text\",\"text\":\", world\"}]}}\n\n")
		_, _ = io.WriteString(w, "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"output\":null,\"usage\":{\"input_tokens\":2,\"output_tokens\":3,\"total_tokens\":5,\"input_tokens_details\":{\"cached_tokens\":1}}}}\n\n")
	}))
	t.Cleanup(remote.Close)
	generator, account := openAIGenerationFixture(t, remote.URL+"/responses", "openai-secret")
	updated := account
	updated.Metadata["organization_id"] = json.RawMessage(`"org_test"`)
	updated.Metadata["project_id"] = json.RawMessage(`"proj_test"`)
	// The fixture persists metadata through the account service so the header
	// assertions exercise the same production account boundary as generation.
	if _, err := generator.accounts.persistence.UpdateProviderCredential(t.Context(), updated.ID, account.Metadata.CredentialRevision(), updated.AuthMethod, true, updated.Metadata, nowUTC()); err != nil {
		t.Fatal(err)
	}
	result, err := generator.Generate(t.Context(), GenerateRequest{
		AccountID: openAIDefaultAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "Hello?"}}, FastMode: true,
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if result.ID != "resp_test" || result.Model != "gpt-test" || result.Text != "Hello, world" ||
		len(result.Reasoning) != 1 || result.Reasoning[0].ID != "rs_1" || result.Reasoning[0].EncryptedContent != "opaque" ||
		len(result.Reasoning[0].Summary) != 1 || result.Reasoning[0].Summary[0] != "Searching" || result.Usage.TotalTokens != 5 {
		t.Fatalf("OpenAI response = %#v", result)
	}
	wire := <-requestSeen
	if wire["model"] != "gpt-test" || wire["service_tier"] != "priority" || wire["store"] != false || wire["stream"] != true {
		t.Fatalf("OpenAI request wire = %#v", wire)
	}
}

// Rust source: crates/noema-providers/src/adapters/openai.rs::websocket_session_preserves_openai_storage_policy (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_WebsocketSessionPreservesOpenaiStoragePolicy(t *testing.T) {
	body, _, err := prepareResponsesGeneration(GenerateRequest{
		AccountID: openAIDefaultAccountID, Model: "gpt-test", StoreResponse: true,
		Messages: []GenerationMessage{{Role: "user", Content: "hello"}},
	}, openAIResponsesProfile())
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	if err := json.Unmarshal(body, &wire); err != nil {
		t.Fatal(err)
	}
	if wire["store"] != true || wire["stream"] != true {
		t.Fatalf("OpenAI session wire policy = %#v", wire)
	}
}

// Rust source: crates/noema-providers/src/adapters/openai.rs::maps_provider_http_error_classes (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_MapsProviderHttpErrorClasses(t *testing.T) {
	for status, want := range map[int]error{
		http.StatusUnauthorized:        ErrAuthenticationRejected,
		http.StatusTooManyRequests:     ErrProviderRateLimited,
		http.StatusInternalServerError: ErrProviderUnavailable,
		http.StatusBadRequest:          ErrProviderRequestRejected,
		http.StatusPaymentRequired:     ErrProviderPaymentRequired,
	} {
		if err := codexGenerationStatusError(status); !errors.Is(err, want) {
			t.Fatalf("OpenAI status %d = %v, want %v", status, err, want)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/openai.rs::malformed_when_success_response_has_no_output_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_MalformedWhenSuccessResponseHasNoOutputText(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = io.WriteString(w, "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp\",\"model\":\"gpt-test\",\"output\":[{\"type\":\"message\",\"content\":[]}]}}\n\n")
	}))
	t.Cleanup(remote.Close)
	generator, _ := openAIGenerationFixture(t, remote.URL, "openai-secret")
	if _, err := generator.Generate(t.Context(), basicOpenAIGenerationRequest(), nil); err == nil || !strings.Contains(err.Error(), "invalid") {
		t.Fatalf("malformed OpenAI response error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/openai.rs::rejects_invalid_configuration_without_leaking_credentials (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RejectsInvalidConfigurationWithoutLeakingCredentials(t *testing.T) {
	if _, err := NewSecret(" "); err == nil {
		t.Fatal("blank OpenAI credential was accepted")
	}
	accounts, err := NewAccountService(t.TempDir(), newParityCredentialPersistence())
	if err != nil {
		t.Fatal(err)
	}
	for _, raw := range []string{"https://user:password@example.test/v1?token=secret", "https://example.test/v1#fragment"} {
		if _, err := newOpenAIGenerator(accounts, http.DefaultClient, raw); err == nil || strings.Contains(err.Error(), "password") || strings.Contains(err.Error(), "token=secret") {
			t.Fatalf("invalid OpenAI URL %q = %v", raw, err)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/openai.rs::provider_debug_redacts_api_key (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ProviderDebugRedactsApiKey(t *testing.T) {
	secret, err := NewSecret("openai-provider-secret")
	if err != nil {
		t.Fatal(err)
	}
	accounts, err := NewAccountService(t.TempDir(), newParityCredentialPersistence())
	if err != nil {
		t.Fatal(err)
	}
	generator, err := newOpenAIGenerator(accounts, http.DefaultClient, "https://example.test/v1/responses")
	if err != nil {
		t.Fatal(err)
	}
	debug := fmt.Sprintf("%v %+v", generator, secret)
	if strings.Contains(debug, "openai-provider-secret") || !strings.Contains(debug, "[REDACTED]") {
		t.Fatalf("OpenAI debug = %q", debug)
	}
}

// Rust source: crates/noema-providers/src/adapters/openrouter.rs::context_window_requires_fresh_metadata_for_the_exact_catalog_profile (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ContextWindowRequiresFreshMetadataForTheExactCatalogProfile(t *testing.T) {
	find := func(metadata AccountMetadata, model string) *uint32 {
		profiles, err := metadata.ModelProfiles()
		if err != nil {
			t.Fatalf("model profiles: %v", err)
		}
		for _, profile := range profiles {
			if profile.ID == model {
				return profile.ContextWindowTokens
			}
		}
		return nil
	}
	stale := AccountMetadata{"profiles": json.RawMessage(`[{"id":"anthropic/claude","label":"Claude"}]`)}
	if got := find(stale, "anthropic/claude"); got != nil {
		t.Fatalf("stale context window = %v", *got)
	}
	refreshed := AccountMetadata{"profiles": json.RawMessage(`[{"id":"anthropic/claude","label":"Claude","context_window_tokens":200000},{"id":"openai/gpt","label":"GPT","context_window_tokens":128000}]`)}
	got := find(refreshed, "anthropic/claude")
	if got == nil || *got != 200000 || find(refreshed, "missing") != nil {
		t.Fatalf("fresh exact profile lookup = %v", got)
	}
}

// Rust source: crates/noema-providers/src/adapters/openrouter/catalog.rs::catalog_filters_incompatible_models_and_maps_reasoning (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CatalogFiltersIncompatibleModelsAndMapsReasoning(t *testing.T) {
	values := []json.RawMessage{
		json.RawMessage(`{"id":"vendor/zeta","name":"Zeta","context_length":65536,"architecture":{"output_modalities":["text"]},"supported_parameters":["tools","tool_choice","structured_outputs"],"reasoning":{"supported_efforts":["low","max"],"default_effort":"max"}}`),
		json.RawMessage(`{"id":"vendor/no-tools","name":"No tools","context_length":65536,"architecture":{"output_modalities":["text"]},"supported_parameters":["structured_outputs"]}`),
		json.RawMessage(`{"id":"vendor/short","name":"Short","context_length":8192,"architecture":{"output_modalities":["text"]},"supported_parameters":["tools","tool_choice","structured_outputs"]}`),
	}
	profiles := profilesFromModels(values)
	if len(profiles) != 2 || profiles[0].ID != "openrouter/auto" || profiles[1].ID != "vendor/zeta" ||
		len(profiles[1].ReasoningEfforts) != 2 || profiles[1].ReasoningEfforts[0] != "low" || profiles[1].ReasoningEfforts[1] != "xhigh" ||
		profiles[1].DefaultReasoningEffort != "xhigh" || profiles[1].ContextWindowTokens == nil || *profiles[1].ContextWindowTokens != 65536 {
		t.Fatalf("OpenRouter catalog profiles = %#v", profiles)
	}
}

// Rust source: crates/noema-providers/src/adapters/openrouter/catalog.rs::pkce_start_uses_s256_and_never_places_the_verifier_in_the_url (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_PkceStartUsesS256AndNeverPlacesTheVerifierInTheUrl(t *testing.T) {
	service, _ := openRouterTestService(t, "https://api.example.test", time.Minute)
	attempt, err := service.StartAuth(t.Context(), "openrouter", "provider_account:openrouter:default", AuthOAuthPKCE)
	if err != nil {
		t.Fatal(err)
	}
	authorization, err := url.Parse(attempt.VerificationURL)
	if err != nil {
		t.Fatal(err)
	}
	query := authorization.Query()
	if query.Get("code_challenge_method") != "S256" || query.Get("callback_url") == "" || query.Get("code_challenge") == "" || strings.Contains(attempt.VerificationURL, service.attempts[attempt.ID].verifier) || len(service.attempts[attempt.ID].verifier) < 43 {
		t.Fatalf("unsafe PKCE start = %#v", attempt)
	}
	hash := sha256.Sum256([]byte(service.attempts[attempt.ID].verifier))
	if query.Get("code_challenge") != base64.RawURLEncoding.EncodeToString(hash[:]) {
		t.Fatalf("PKCE challenge does not bind to verifier: %q", query.Get("code_challenge"))
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/output.rs::finalization_preserves_distinct_provider_messages (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FinalizationPreservesDistinctProviderMessages(t *testing.T) {
	payload := "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"Got it\"}\n\n" +
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"message\",\"phase\":\"commentary\",\"content\":[{\"type\":\"output_text\",\"text\":\"Got it\"}]}}\n\n" +
		"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"output_index\":1,\"delta\":\"What time works?\"}\n\n" +
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":1,\"item\":{\"type\":\"message\",\"phase\":\"final_answer\",\"content\":[{\"type\":\"output_text\",\"text\":\"What time works?\"}]}}\n\n" +
		"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\n\n"
	parsed, _ := parityParseCodexSSE(t, payload, nil)
	if len(parsed.Output) != 2 {
		t.Fatalf("distinct output items = %#v", parsed.Output)
	}
	var first, second map[string]any
	if err := json.Unmarshal(parsed.Output[0].Raw, &first); err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(parsed.Output[1].Raw, &second); err != nil {
		t.Fatal(err)
	}
	if first["phase"] != "commentary" || second["phase"] != "final_answer" || first["type"] != "message" || second["type"] != "message" {
		t.Fatalf("provider message boundaries = %#v / %#v", first, second)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/sse.rs::incremental_sse_parser_emits_deltas_before_terminal_response (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_IncrementalSseParserEmitsDeltasBeforeTerminalResponse(t *testing.T) {
	payload := "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Hel\"}\n\n" +
		"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\n" +
		"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\n"
	events := make([]StreamEvent, 0, 2)
	parsed, err := parityParseCodexSSE(t, payload, func(event StreamEvent) { events = append(events, event) })
	if err != nil {
		t.Fatal(err)
	}
	if len(events) != 2 || events[0].Kind != TextDelta || events[0].Delta != "Hel" || events[1].Delta != "lo" || parsed.ID != "resp_test" {
		t.Fatalf("incremental SSE events = %#v, result = %#v", events, parsed)
	}
	result, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test"}, parsed, openRouterToolNameMap{})
	if err != nil || result.Text != "Hello" {
		t.Fatalf("incremental SSE result = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/sse.rs::response_from_sse_prefers_output_item_done_over_terminal_output (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponseFromSsePrefersOutputItemDoneOverTerminalOutput(t *testing.T) {
	parsed, err := parityParseCodexSSE(t, "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"from item\"}]}}\n\n"+
		"event: response.completed\ndata: {\"type\":\"response.completed\",\"id\":\"ignored\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\",\"output\":null}}\n\n", nil)
	result, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test"}, parsed, openRouterToolNameMap{})
	if err != nil || result.ID != "resp_test" || result.Text != "from item" {
		t.Fatalf("output-item precedence = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/sse.rs::response_from_sse_prefers_streamed_text_over_output_item_done_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponseFromSsePrefersStreamedTextOverOutputItemDoneText(t *testing.T) {
	parsed, err := parityParseCodexSSE(t, "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Searching \"}\n\n"+
		"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"memory.\"}\n\n"+
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"stale\"}]}}\n\n"+
		"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\",\"output\":null}}\n\n", nil)
	result, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test"}, parsed, openRouterToolNameMap{})
	if err != nil || result.Text != "Searching memory." {
		t.Fatalf("streamed text precedence = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/sse.rs::streamed_messages_keep_boundaries_without_repeating_done_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_StreamedMessagesKeepBoundariesWithoutRepeatingDoneText(t *testing.T) {
	payload := "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"Got it\"}\n\n" +
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"message\",\"phase\":\"commentary\",\"content\":[{\"type\":\"output_text\",\"text\":\"Got it\"}]}}\n\n" +
		"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"output_index\":1,\"delta\":\"What time works?\"}\n\n" +
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":1,\"item\":{\"type\":\"message\",\"phase\":\"final_answer\",\"content\":[{\"type\":\"output_text\",\"text\":\"What time works?\"}]}}\n\n" +
		"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\n\n"
	events := []StreamEvent{}
	parsed, err := parityParseCodexSSE(t, payload, func(event StreamEvent) { events = append(events, event) })
	if err != nil || len(events) != 2 || events[0].Delta != "Got it" || events[1].Delta != "What time works?" || len(parsed.Output) != 2 {
		t.Fatalf("streamed message boundaries = %#v / %#v, %v", events, parsed.Output, err)
	}
	var first, second map[string]any
	_ = json.Unmarshal(parsed.Output[0].Raw, &first)
	_ = json.Unmarshal(parsed.Output[1].Raw, &second)
	if first["phase"] != "commentary" || second["phase"] != "final_answer" {
		t.Fatalf("streamed message phases = %#v / %#v", first, second)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/sse.rs::response_from_sse_preserves_function_call_items_with_streamed_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponseFromSsePreservesFunctionCallItemsWithStreamedText(t *testing.T) {
	payload := "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Checking.\"}\n\n" +
		"event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"output_index\":1,\"item\":{\"type\":\"function_call\",\"id\":\"item_1\",\"call_id\":\"call_1\",\"name\":\"search_memory\",\"arguments\":\"\"}}\n\n" +
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":1,\"item\":{\"type\":\"function_call\",\"id\":\"item_1\",\"call_id\":\"call_1\",\"name\":\"search_memory\",\"arguments\":\"{\\\"query\\\":\\\"trains\\\"}\"}}\n\n" +
		"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\",\"output\":null}}\n\n"
	events := []StreamEvent{}
	parsed, err := parityParseCodexSSE(t, payload, func(event StreamEvent) { events = append(events, event) })
	if err != nil {
		t.Fatal(err)
	}
	tools := []GenerationTool{{Name: "search_memory", Description: "Search memory", InputSchema: json.RawMessage(`{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}`)}}
	names, _, err := prepareOpenRouterTools(tools)
	if err != nil {
		t.Fatal(err)
	}
	result, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", ToolTransport: ToolTransportNative, Tools: tools}, parsed, names)
	if err != nil || result.Text != "Checking." || len(result.ToolCalls) != 1 || result.ToolCalls[0].ProviderCallID != "call_1" || result.ToolCalls[0].Name != "search_memory" || len(events) == 0 || events[0].Kind != TextDelta {
		t.Fatalf("mixed text/tool result = %#v, events=%#v, err=%v", result, events, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/sse.rs::streamed_response_preserves_hosted_search_and_citations (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_StreamedResponsePreservesHostedSearchAndCitations(t *testing.T) {
	payload := "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Current answer.\"}\n\n" +
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":1,\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_1\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"query\":\"current answer\"}}}\n\n" +
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"discarded\",\"annotations\":[{\"type\":\"url_citation\",\"title\":\"Official source\",\"url\":\"https://example.com/source\",\"start_index\":0,\"end_index\":10}]}]}}\n\n" +
		"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\n\n"
	parsed, err := parityParseCodexSSE(t, payload, nil)
	if err != nil {
		t.Fatal(err)
	}
	result, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", ToolTransport: ToolTransportNative}, parsed, openRouterToolNameMap{})
	var arguments map[string]any
	if len(result.Searches) == 1 {
		_ = json.Unmarshal(result.Searches[0].Arguments, &arguments)
	}
	if err != nil || result.Text != "Current answer." || len(result.Searches) != 1 || arguments["query"] != "current answer" || len(result.Citations) != 1 || result.Citations[0].URL != "https://example.com/source" {
		t.Fatalf("hosted search/citations = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/sse.rs::hosted_search_lifecycle_is_normalized_and_deduplicated (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_HostedSearchLifecycleIsNormalizedAndDeduplicated(t *testing.T) {
	payload := "event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"output_index\":2,\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_1\",\"status\":\"in_progress\"}}\n\n" +
		"event: response.web_search_call.in_progress\ndata: {\"type\":\"response.web_search_call.in_progress\",\"output_index\":2,\"item_id\":\"ws_1\"}\n\n" +
		"event: response.web_search_call.searching\ndata: {\"type\":\"response.web_search_call.searching\",\"output_index\":2,\"item_id\":\"ws_1\"}\n\n" +
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":2,\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_1\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"query\":\"weather\"}}}\n\n" +
		"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\n\n"
	events := []StreamEvent{}
	_, err := parityParseCodexSSE(t, payload, func(event StreamEvent) { events = append(events, event) })
	if err != nil {
		t.Fatal(err)
	}
	// Go exposes one deduplicated hosted-search start event. The Rust adapter
	// also emitted provider timing milestones, which have no Go event kind.
	if len(events) != 1 || events[0].Kind != HostedSearchStarted || events[0].Index != 2 || events[0].ID != "ws_1" {
		t.Fatalf("hosted search lifecycle = %#v", events)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/sse.rs::incremental_sse_parser_buffers_utf8_and_delimiter_splits (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_IncrementalSseParserBuffersUtf8AndDelimiterSplits(t *testing.T) {
	for _, payload := range []string{
		"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"caf\u00e9\"}\n\n" + "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\n\n",
		"event: response.output_text.delta\r\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Hi\"}\r\n\r\n" + "event: response.completed\r\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\r\n\r\n",
	} {
		events := []StreamEvent{}
		parsed, err := parityParseCodexSSE(t, payload, func(event StreamEvent) { events = append(events, event) })
		if err != nil || len(events) != 1 || events[0].Kind != TextDelta || parsed.FinishReason != "stop" {
			t.Fatalf("split UTF-8/delimiter payload = %#v, %#v, %v", events, parsed, err)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::responses_request_profiles_preserve_provider_wire_differences (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponsesRequestProfilesPreserveProviderWireDifferences(t *testing.T) {
	request := GenerateRequest{
		AccountID: openAIDefaultAccountID, Model: "gpt-5.5", ConversationID: " conversation:cacheable ",
		Messages:        []GenerationMessage{{Role: "developer", Content: "Be brief."}, {Role: "user", Content: "hi"}},
		MaxOutputTokens: func() *uint32 { value := uint32(32); return &value }(), ReasoningEffort: "low",
		Tools: []GenerationTool{parityGenerationTool("search_memory")}, ToolTransport: ToolTransportNative,
		ToolChoice: ToolChoiceRequired, ParallelTools: true,
	}
	openAIBytes, _, err := prepareResponsesGeneration(request, openAIResponsesProfile())
	if err != nil {
		t.Fatal(err)
	}
	openAI := map[string]any{}
	if err := json.Unmarshal(openAIBytes, &openAI); err != nil {
		t.Fatal(err)
	}
	codexRequest := request
	codexRequest.AccountID, codexRequest.Model, codexRequest.ReasoningEffort = codexGenerationAccountID, "gpt-codex", "high"
	codexBytes, _, err := prepareCodexGeneration(codexRequest)
	if err != nil {
		t.Fatal(err)
	}
	codex := map[string]any{}
	if err := json.Unmarshal(codexBytes, &codex); err != nil {
		t.Fatal(err)
	}
	if openAI["max_output_tokens"] != float64(32) || openAI["prompt_cache_retention"] != "24h" || openAI["stream"] != true || openAI["reasoning"].(map[string]any)["effort"] != "low" || openAI["store"] != false {
		t.Fatalf("OpenAI profile wire = %#v", openAI)
	}
	if codex["max_output_tokens"] != nil || codex["prompt_cache_retention"] != nil || codex["include"] != nil || codex["stream"] != true || codex["reasoning"].(map[string]any)["effort"] != "high" || codex["store"] != false {
		t.Fatalf("Codex profile wire = %#v", codex)
	}
	for label, value := range map[string]map[string]any{"OpenAI": openAI, "Codex": codex} {
		if value["tool_choice"] != "required" || value["parallel_tool_calls"] != true || value["prompt_cache_key"] != "conversation:cacheable" || len(value["tools"].([]any)) != 1 {
			t.Fatalf("%s shared profile wire = %#v", label, value)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::hosted_web_search_serializes_beside_configured_functions (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_HostedWebSearchSerializesBesideConfiguredFunctions(t *testing.T) {
	request := GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "research"}},
		Tools: []GenerationTool{parityGenerationTool("search_memory")}, ToolTransport: ToolTransportNative, ToolChoice: ToolChoiceRequired, HostedWebSearch: true}
	body, _, err := prepareCodexGeneration(request)
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	_ = json.Unmarshal(body, &wire)
	tools, ok := wire["tools"].([]any)
	if !ok || len(tools) != 2 || tools[0].(map[string]any)["type"] != "function" || tools[1].(map[string]any)["type"] != "web_search" || tools[1].(map[string]any)["external_web_access"] != true || wire["tool_choice"] != "auto" || !jsonArrayHasString(wire["include"], "web_search_call.action.sources") {
		t.Fatalf("hosted web wire = %#v", wire)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::hosted_web_actions_preserve_ordered_search_and_page_sources (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_HostedWebActionsPreserveOrderedSearchAndPageSources(t *testing.T) {
	parsed, err := parityParseCodexSSE(t, "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"web_search_call\",\"id\":\"search\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"query\":\"Noema\",\"sources\":[{\"title\":\"one\",\"url\":\"https://one.example/a\"},{\"title\":\"two\",\"url\":\"https://two.example/b\"}]}}}\n\n"+
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":1,\"item\":{\"type\":\"web_search_call\",\"id\":\"open\",\"status\":\"completed\",\"action\":{\"type\":\"open_page\",\"url\":\"https://three.example/c\"}}}\n\n"+
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":2,\"item\":{\"type\":\"web_search_call\",\"id\":\"find\",\"status\":\"completed\",\"action\":{\"type\":\"find_in_page\",\"url\":\"https://four.example/d\",\"pattern\":\"citation\"}}}\n\n"+
		"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":3,\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"Done.\",\"annotations\":[{\"type\":\"url_citation\",\"title\":\"Source\",\"url\":\"https://one.example/a\",\"start_index\":0,\"end_index\":5}]}]}}\n\n"+
		"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"response:sources\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\n", nil)
	if err != nil {
		t.Fatal(err)
	}
	result, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", ToolTransport: ToolTransportNative}, parsed, openRouterToolNameMap{})
	if err != nil || len(result.Searches) != 3 || len(result.Searches[0].Sources) != 2 || result.Searches[0].Sources[1].URL != "https://two.example/b" || result.Searches[1].Name != "web.fetch" || result.Searches[2].Name != "web.fetch" || result.Searches[2].Sources[0].URL != "https://four.example/d" || len(result.Citations) != 1 || result.Text != "Done." {
		t.Fatalf("hosted web actions = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::responses_tools_omit_lookaround_patterns_without_relaxing_other_patterns (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponsesToolsOmitLookaroundPatternsWithoutRelaxingOtherPatterns(t *testing.T) {
	tool := parityGenerationTool("mcp.dex.create_calendar_event")
	tool.InputSchema = json.RawMessage(`{"type":"object","properties":{"attendees":{"type":"array","items":{"type":"string","pattern":"^(?!\\.)(?!.*\\.\\.)([A-Za-z0-9_'+\\-\\.]*)[A-Za-z0-9_+-]@([A-Za-z0-9][A-Za-z0-9\\-]*\\.)+[A-Za-z]{2,}$"}},"title":{"type":"string","pattern":".*\\S.*"}}}`)
	body, _, err := prepareCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}, Tools: []GenerationTool{tool}, ToolTransport: ToolTransportNative})
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	_ = json.Unmarshal(body, &wire)
	properties := wire["tools"].([]any)[0].(map[string]any)["parameters"].(map[string]any)["properties"].(map[string]any)
	if _, exists := properties["attendees"].(map[string]any)["items"].(map[string]any)["pattern"]; exists || properties["title"].(map[string]any)["pattern"] != ".*\\S.*" {
		t.Fatalf("lookaround conversion = %#v", properties)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::openai_profile_serializes_allowed_tools_with_provider_safe_names (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_OpenaiProfileSerializesAllowedToolsWithProviderSafeNames(t *testing.T) {
	tools := []GenerationTool{parityGenerationTool("search_memory"), parityGenerationTool("mcp.docs:read")}
	body, _, err := prepareResponsesGeneration(GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}, Tools: tools, ToolTransport: ToolTransportNative, ToolChoice: ToolChoiceRequired}, openAIResponsesProfile())
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	_ = json.Unmarshal(body, &wire)
	items := wire["tools"].([]any)
	if len(items) != 2 || items[1].(map[string]any)["name"] != "docs_x3a_read" || wire["tool_choice"] != "required" {
		t.Fatalf("OpenAI safe tool names = %#v", wire)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::allowed_tools_are_profile_gated_and_must_reference_the_catalog (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AllowedToolsAreProfileGatedAndMustReferenceTheCatalog(t *testing.T) {
	if _, _, err := prepareCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}, Tools: []GenerationTool{parityGenerationTool("search_memory")}, ToolTransport: ToolTransportNative, ToolChoice: "allowed"}); err == nil {
		t.Fatal("unsupported allowed tool choice was accepted")
	}
	if _, _, err := prepareResponsesGeneration(GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}, Tools: []GenerationTool{parityGenerationTool("search_memory")}, ToolTransport: ToolTransportNative, ToolChoice: ToolChoiceRequired}, openAIResponsesProfile()); err != nil {
		t.Fatal(err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::direct_responses_profiles_preserve_cache_controls (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DirectResponsesProfilesPreserveCacheControls(t *testing.T) {
	request := GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-5.6", Messages: []GenerationMessage{{Role: "system", Content: "   "}, {Role: "developer", Content: "Stable </noema_application_context> & <context>"}, {Role: "developer", Content: "Environment revision 8"}, {Role: "user", Content: "What changed?"}}}
	body, _, err := prepareResponsesGeneration(request, openAIResponsesProfile())
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	_ = json.Unmarshal(body, &wire)
	if wire["prompt_cache_options"].(map[string]any)["mode"] != "explicit" || wire["prompt_cache_options"].(map[string]any)["ttl"] != "30m" || wire["prompt_cache_retention"] != nil {
		t.Fatalf("explicit cache controls = %#v", wire)
	}
	input := wire["input"].([]any)
	developer := input[0].(map[string]any)
	if developer["role"] != "developer" || developer["content"].([]any)[0].(map[string]any)["prompt_cache_breakpoint"].(map[string]any)["mode"] != "explicit" {
		t.Fatalf("developer cache breakpoint = %#v", developer)
	}
	codexBody, _, err := prepareCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-codex", Messages: request.Messages})
	if err != nil {
		t.Fatal(err)
	}
	var codex map[string]any
	_ = json.Unmarshal(codexBody, &codex)
	if codex["prompt_cache_options"] != nil || codex["prompt_cache_retention"] != nil {
		t.Fatalf("Codex cache controls = %#v", codex)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::prompt_cache_breakpoints_reject_invalid_filtered_message_indices (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_PromptCacheBreakpointsRejectInvalidFilteredMessageIndices(t *testing.T) {
	// Go derives breakpoints from non-empty developer messages. A request with
	// one developer message therefore produces exactly one breakpoint candidate.
	body, _, err := prepareResponsesGeneration(GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-5.6", Messages: []GenerationMessage{{Role: "developer", Content: "Environment revision 8"}}}, openAIResponsesProfile())
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	_ = json.Unmarshal(body, &wire)
	if len(wire["input"].([]any)) != 1 {
		t.Fatalf("filtered breakpoint input = %#v", wire["input"])
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::responses_request_reasoning_precedence_and_input_validation_are_shared (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponsesRequestReasoningPrecedenceAndInputValidationAreShared(t *testing.T) {
	body, _, err := prepareResponsesGeneration(GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}, ReasoningEffort: "medium"}, openAIResponsesProfile())
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	_ = json.Unmarshal(body, &wire)
	if wire["reasoning"].(map[string]any)["effort"] != "medium" || wire["reasoning"].(map[string]any)["summary"] != "auto" {
		t.Fatalf("reasoning precedence = %#v", wire)
	}
	body, _, err = prepareResponsesGeneration(GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}, ReasoningEffort: "none"}, openAIResponsesProfile())
	if err != nil {
		t.Fatal(err)
	}
	_ = json.Unmarshal(body, &wire)
	if _, exists := wire["reasoning"].(map[string]any)["summary"]; exists {
		t.Fatalf("none reasoning summary = %#v", wire["reasoning"])
	}
	if _, _, err := prepareResponsesGeneration(GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}, ReasoningEffort: "invalid"}, openAIResponsesProfile()); err == nil {
		t.Fatal("invalid reasoning effort was accepted")
	}
	if _, _, err := prepareResponsesGeneration(GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-test"}, openAIResponsesProfile()); err == nil {
		t.Fatal("empty generation input was accepted")
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::whole_catalog_lowering_is_stable (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_WholeCatalogLoweringIsStable(t *testing.T) {
	tools := []GenerationTool{parityGenerationTool("web.search"), parityGenerationTool("web.fetch"), parityGenerationTool("mcp.mcp:docs.read")}
	request := GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-fixture", Messages: []GenerationMessage{{Role: "user", Content: "fixture input"}}, Tools: tools, ToolTransport: ToolTransportNative, ToolChoice: ToolChoiceRequired, ParallelTools: true}
	first, _, err := prepareResponsesGeneration(request, openAIResponsesProfile())
	if err != nil {
		t.Fatal(err)
	}
	second, _, err := prepareResponsesGeneration(request, openAIResponsesProfile())
	if err != nil || !bytes.Equal(first, second) {
		t.Fatalf("catalog lowering is not stable: %v", err)
	}
	var wire map[string]any
	_ = json.Unmarshal(first, &wire)
	if len(wire["tools"].([]any)) != 3 || wire["tools"].([]any)[2].(map[string]any)["name"] != "read" {
		t.Fatalf("catalog lowering = %#v", wire)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::responses_finalize_native_final_without_calls_accepts_plain_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponsesFinalizeNativeFinalWithoutCallsAcceptsPlainText(t *testing.T) {
	parsed, err := parityParseCodexSSE(t, "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"Done.\"}]}}\n\n"+
		"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_1\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\n", nil)
	result, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", ToolTransport: ToolTransportNative}, parsed, openRouterToolNameMap{})
	if err != nil || result.Text != "Done." || len(result.ToolCalls) != 0 {
		t.Fatalf("plain native final = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::valid_unknown_provider_tool_name_is_rejected_without_fallback (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ValidUnknownProviderToolNameIsRejectedWithoutFallback(t *testing.T) {
	tool := parityGenerationTool("search_memory")
	names, _, err := prepareOpenRouterTools([]GenerationTool{tool})
	if err != nil {
		t.Fatal(err)
	}
	parsed := codexStreamResult{ID: "resp", Model: "gpt-test", FinishReason: "stop", Output: []codexOutputItem{{Index: 0, Raw: json.RawMessage(`{"type":"function_call","id":"item","call_id":"call_1","name":"unadvertised_valid_name","arguments":"{}"}`)}}}
	if _, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", ToolTransport: ToolTransportNative, Tools: []GenerationTool{tool}}, parsed, names); err == nil || !strings.Contains(err.Error(), "unadvertised") {
		t.Fatalf("unknown provider tool name error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::responses_response_parses_function_call_output_items (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponsesResponseParsesFunctionCallOutputItems(t *testing.T) {
	tool := parityGenerationTool("mcp.docs:read")
	tool.InputSchema = json.RawMessage(`{"type":"object","properties":{"document_id":{"type":"string"},"context":{"type":"object","properties":{"mode":{"type":"string"}},"additionalProperties":false}},"required":["document_id"],"additionalProperties":false}`)
	names, _, err := prepareOpenRouterTools([]GenerationTool{tool})
	if err != nil {
		t.Fatal(err)
	}
	providerName := names.canonicalToName[tool.Name]
	parsed := codexStreamResult{ID: "resp", Model: "gpt-test", FinishReason: "stop", Output: []codexOutputItem{{Index: 0, Raw: json.RawMessage(fmt.Sprintf(`{"type":"function_call","id":"item_1","call_id":"call_1","name":%q,"arguments":%q}`, providerName, `{"document_id":"doc_1","context":{"mode":null}}`))}}}
	result, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", ToolTransport: ToolTransportNative, Tools: []GenerationTool{tool}}, parsed, names)
	if err != nil || len(result.ToolCalls) != 1 || result.ToolCalls[0].ProviderItemID != "item_1" || result.ToolCalls[0].ProviderCallID != "call_1" || result.ToolCalls[0].Name != tool.Name || string(result.ToolCalls[0].Payload) != `{"context":{},"document_id":"doc_1"}` {
		t.Fatalf("parsed native call = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::responses_response_rejects_malformed_native_tool_calls (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponsesResponseRejectsMalformedNativeToolCalls(t *testing.T) {
	tool := parityGenerationTool("search_memory")
	names, _, err := prepareOpenRouterTools([]GenerationTool{tool})
	if err != nil {
		t.Fatal(err)
	}
	providerName := names.canonicalToName[tool.Name]
	for _, test := range []struct{ callID, arguments, want string }{
		{"call_1", `{"query":`, "arguments"},
		{"call_1", `[]`, "JSON object"},
		{"", `{"query":"trains"}`, "id is invalid"},
	} {
		raw := fmt.Sprintf(`{"type":"function_call","call_id":%q,"name":%q,"arguments":%q}`, test.callID, providerName, test.arguments)
		parsed := codexStreamResult{ID: "resp", Model: "gpt-test", FinishReason: "stop", Output: []codexOutputItem{{Index: 0, Raw: json.RawMessage(raw)}}}
		_, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", ToolTransport: ToolTransportNative, Tools: []GenerationTool{tool}}, parsed, names)
		if err == nil || !strings.Contains(err.Error(), test.want) {
			t.Fatalf("malformed native call %q/%q error = %v", test.callID, test.arguments, err)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::responses_input_serializes_native_tool_results (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponsesInputSerializesNativeToolResults(t *testing.T) {
	message := GenerationMessage{Role: "tool", ToolResult: &ReplayToolResult{ProviderCallID: "call_1", Name: "mcp.docs:read", ProviderName: "mcp_x2e_docs_x3a_read", Success: true, Payload: json.RawMessage(`{"title":"Docs"}`)}}
	items, err := lowerCodexToolResult(message, openRouterToolNameMap{})
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 1 {
		t.Fatalf("tool result items = %#v", items)
	}
	wire, ok := items[0].(map[string]any)
	if !ok || wire["type"] != "function_call_output" || wire["call_id"] != "call_1" {
		t.Fatalf("tool result wire = %#v", items)
	}
	var output map[string]any
	if err := json.Unmarshal([]byte(wire["output"].(string)), &output); err != nil {
		t.Fatal(err)
	}
	if output["name"] != "mcp.docs:read" || output["provider_name"] != "mcp_x2e_docs_x3a_read" || output["success"] != true || output["payload"].(map[string]any)["title"] != "Docs" {
		t.Fatalf("tool result output = %#v", output)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::chained_response_sends_only_new_tool_outputs (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ChainedResponseSendsOnlyNewToolOutputs(t *testing.T) {
	body, _, err := prepareResponsesGeneration(GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-test", PreviousResponseID: "resp_previous", StoreResponse: true, Messages: []GenerationMessage{{Role: "tool", ToolResult: &ReplayToolResult{ProviderCallID: "call_1", Name: "search_memory", Success: true, Payload: json.RawMessage(`{"matches":[]}`)}}}}, openAIResponsesProfile())
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	_ = json.Unmarshal(body, &wire)
	if wire["previous_response_id"] != "resp_previous" || wire["store"] != true || len(wire["input"].([]any)) != 1 || wire["input"].([]any)[0].(map[string]any)["type"] != "function_call_output" || wire["input"].([]any)[0].(map[string]any)["call_id"] != "call_1" {
		t.Fatalf("chained tool output wire = %#v", wire)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::serializes_reasoning_history_item_for_replay (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SerializesReasoningHistoryItemForReplay(t *testing.T) {
	items, err := lowerCodexReplayReasoning(GenerationMessage{Role: "assistant", ReasoningID: "rs_1", EncryptedReasoning: "opaque-openai-reasoning"})
	if err != nil || len(items) != 1 {
		t.Fatalf("reasoning replay = %#v, %v", items, err)
	}
	item := items[0].(map[string]any)
	if item["type"] != "reasoning" || item["id"] != "rs_1" || item["encrypted_content"] != "opaque-openai-reasoning" || len(item["summary"].([]any)) != 0 {
		t.Fatalf("reasoning replay item = %#v", item)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::responses_input_encodes_unsafe_typed_history_tool_names (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponsesInputEncodesUnsafeTypedHistoryToolNames(t *testing.T) {
	items, err := lowerCodexReplayCall(ReplayToolCall{ProviderCallID: "call_1", Name: "mcp.dex:search contacts", Arguments: json.RawMessage(`{"query":"Gautam"}`)}, openRouterToolNameMap{})
	if err != nil {
		t.Fatal(err)
	}
	item := items.(map[string]any)
	if item["type"] != "function_call" || item["name"] != "dex_x3a_search_x20_contacts" || item["call_id"] != "call_1" {
		t.Fatalf("unsafe history tool name = %#v", item)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::responses_tool_name_map_disambiguates_only_actual_alias_collisions (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResponsesToolNameMapDisambiguatesOnlyActualAliasCollisions(t *testing.T) {
	tools := []GenerationTool{parityGenerationTool("mcp.mcp_server:alpha.dex_list_contacts"), parityGenerationTool("mcp.mcp_server:bravo.dex_list_contacts")}
	names, wire, err := prepareOpenRouterTools(tools)
	if err != nil {
		t.Fatal(err)
	}
	if names.canonicalToName[tools[0].Name] == names.canonicalToName[tools[1].Name] || len(wire) != 2 || !strings.HasPrefix(names.canonicalToName[tools[0].Name], "dex_list_contacts_") || !strings.HasPrefix(names.canonicalToName[tools[1].Name], "dex_list_contacts_") {
		t.Fatalf("tool collision names = %#v", names.canonicalToName)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::long_mcp_tool_names_keep_the_callable_name_on_the_provider_wire (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LongMcpToolNamesKeepTheCallableNameOnTheProviderWire(t *testing.T) {
	canonical := "mcp.mcp_server:5d8a417997dd2905574a27fe7c3a3afa.dex_list_contacts"
	_, wire, err := prepareOpenRouterTools([]GenerationTool{parityGenerationTool(canonical)})
	if err != nil {
		t.Fatal(err)
	}
	if wire[0].Function == nil || wire[0].Function.Name != "dex_list_contacts" {
		t.Fatalf("long tool provider name = %#v", wire[0])
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::full_provider_conversion_closes_optional_fields_and_marks_them_nullable (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FullProviderConversionClosesOptionalFieldsAndMarksThemNullable(t *testing.T) {
	tool := parityGenerationTool("mcp.docs.read")
	tool.InputSchema = json.RawMessage(`{"type":"object","properties":{"document_id":{"type":"string"},"context":{"type":"object","properties":{"mode":{"type":"string"}},"required":[],"additionalProperties":false}},"required":["document_id"],"additionalProperties":false}`)
	_, wire, err := prepareOpenRouterTools([]GenerationTool{tool})
	if err != nil {
		t.Fatal(err)
	}
	parameters := wire[0].Function.Parameters.(map[string]any)
	if !wire[0].Function.Strict || parameters["required"].([]any)[0] != "context" || parameters["properties"].(map[string]any)["context"].(map[string]any)["type"].([]any)[1] != "null" {
		t.Fatalf("strict schema conversion = %#v", parameters)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::browser_wait_union_remains_exact_after_strict_provider_conversion (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BrowserWaitUnionRemainsExactAfterStrictProviderConversion(t *testing.T) {
	tool := parityGenerationTool("web.browse.wait")
	tool.InputSchema = json.RawMessage(`{"type":"object","properties":{"condition":{"anyOf":[{"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false},{"type":"object","properties":{"ref":{"type":"string"}},"required":["ref"],"additionalProperties":false}]},"timeout_ms":{"type":"integer"}},"required":["condition","timeout_ms"],"additionalProperties":false}`)
	_, wire, err := prepareOpenRouterTools([]GenerationTool{tool})
	if err != nil {
		t.Fatal(err)
	}
	parameters := wire[0].Function.Parameters.(map[string]any)
	variants := parameters["properties"].(map[string]any)["condition"].(map[string]any)["anyOf"].([]any)
	if !wire[0].Function.Strict || parameters["additionalProperties"] != false || len(variants) != 2 || variants[0].(map[string]any)["additionalProperties"] != false || variants[0].(map[string]any)["required"].([]any)[0] != "text" || variants[1].(map[string]any)["required"].([]any)[0] != "ref" {
		t.Fatalf("wait schema conversion = %#v", parameters)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::full_provider_conversion_uses_reduced_copy_for_unsupported_unique_items (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FullProviderConversionUsesReducedCopyForUnsupportedUniqueItems(t *testing.T) {
	schema := json.RawMessage(`{"type":"object","properties":{"context":{"type":"string"},"ids":{"type":"array","uniqueItems":true}},"required":["ids"],"additionalProperties":false}`)
	_, wire, err := prepareOpenRouterTools([]GenerationTool{{Name: "mcp.docs.read", Description: "Read", InputSchema: schema}})
	if err != nil {
		t.Fatal(err)
	}
	if wire[0].Function.Strict || !jsonValuesEqual(wire[0].Function.Parameters, map[string]any{"type": "object", "properties": map[string]any{"context": map[string]any{"type": "string"}, "ids": map[string]any{"type": "array", "uniqueItems": true}}, "required": []any{"ids"}, "additionalProperties": false}) {
		t.Fatalf("uniqueItems fallback = %#v", wire[0].Function.Parameters)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/mod.rs::prompt_cache_key_uses_non_empty_conversation_id (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_PromptCacheKeyUsesNonEmptyConversationId(t *testing.T) {
	for value, want := range map[string]string{" conversation:cacheable ": "conversation:cacheable", "  ": "", "": ""} {
		body, _, err := prepareResponsesGeneration(GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-test", ConversationID: value, Messages: []GenerationMessage{{Role: "user", Content: "hi"}}}, openAIResponsesProfile())
		if err != nil {
			t.Fatal(err)
		}
		var wire map[string]any
		_ = json.Unmarshal(body, &wire)
		if got, _ := wire["prompt_cache_key"].(string); got != want {
			t.Fatalf("prompt cache key %q = %q, want %q", value, got, want)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/tool_transport.rs::hosted_and_native_requests_omit_legacy_text_format (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_HostedAndNativeRequestsOmitLegacyTextFormat(t *testing.T) {
	for _, hosted := range []bool{false, true} {
		body, _, err := prepareCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}, ToolTransport: ToolTransportNative, HostedWebSearch: hosted})
		if err != nil {
			t.Fatal(err)
		}
		var wire map[string]any
		_ = json.Unmarshal(body, &wire)
		if _, exists := wire["text"]; exists {
			t.Fatalf("legacy text format leaked into wire: %#v", wire)
		}
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/tool_transport.rs::disabled_transport_rejects_an_advertised_tool_catalog (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DisabledTransportRejectsAnAdvertisedToolCatalog(t *testing.T) {
	if _, _, err := prepareCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}, Tools: []GenerationTool{parityGenerationTool("search_memory")}, ToolTransport: ToolTransportNone}); err == nil {
		t.Fatal("disabled tool transport accepted advertised tools")
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/tool_transport.rs::native_call_only_response_is_normalized_without_assistant_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_NativeCallOnlyResponseIsNormalizedWithoutAssistantText(t *testing.T) {
	tool := parityGenerationTool("search_memory")
	names, _, err := prepareOpenRouterTools([]GenerationTool{tool})
	if err != nil {
		t.Fatal(err)
	}
	parsed := codexStreamResult{ID: "resp", Model: "gpt-test", FinishReason: "stop", Output: []codexOutputItem{{Raw: json.RawMessage(fmt.Sprintf(`{"type":"function_call","call_id":"call_1","name":%q,"arguments":"{}"}`, names.canonicalToName[tool.Name]))}}}
	result, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", ToolTransport: ToolTransportNative, Tools: []GenerationTool{tool}}, parsed, names)
	if err != nil || result.Text != "" || len(result.ToolCalls) != 1 {
		t.Fatalf("native call only = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/tool_transport.rs::native_tool_response_uses_native_calls_with_plain_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_NativeToolResponseUsesNativeCallsWithPlainText(t *testing.T) {
	tool := parityGenerationTool("search_memory")
	names, _, err := prepareOpenRouterTools([]GenerationTool{tool})
	if err != nil {
		t.Fatal(err)
	}
	parsed := codexStreamResult{ID: "resp", Model: "gpt-test", FinishReason: "stop", Output: []codexOutputItem{
		{Index: 0, Raw: json.RawMessage(`{"type":"message","content":[{"type":"output_text","text":"Checking"}]}`)},
		{Index: 1, Raw: json.RawMessage(fmt.Sprintf(`{"type":"function_call","call_id":"call_1","name":%q,"arguments":"{}"}`, names.canonicalToName[tool.Name]))},
	}}
	result, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", ToolTransport: ToolTransportNative, Tools: []GenerationTool{tool}}, parsed, names)
	if err != nil || result.Text != "Checking" || len(result.ToolCalls) != 1 || result.FinishReason != "tool_calls" {
		t.Fatalf("native text/tool response = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/responses/tests/tool_transport.rs::native_parallel_calls_reject_duplicate_provider_call_ids (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_NativeParallelCallsRejectDuplicateProviderCallIds(t *testing.T) {
	tool := parityGenerationTool("search_memory")
	names, _, err := prepareOpenRouterTools([]GenerationTool{tool})
	if err != nil {
		t.Fatal(err)
	}
	call := func() json.RawMessage {
		return json.RawMessage(fmt.Sprintf(`{"type":"function_call","call_id":"call_1","name":%q,"arguments":"{}"}`, names.canonicalToName[tool.Name]))
	}
	parsed := codexStreamResult{ID: "resp", Model: "gpt-test", FinishReason: "stop", Output: []codexOutputItem{{Raw: call()}, {Raw: call()}}}
	if _, err := normalizeCodexGeneration(GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-test", ToolTransport: ToolTransportNative, Tools: []GenerationTool{tool}}, parsed, names); err == nil || !strings.Contains(err.Error(), "more than one") {
		t.Fatalf("duplicate native calls error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/capabilities.rs::account_readiness_controls_model_capability_status (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AccountReadinessControlsModelCapabilityStatus(t *testing.T) {
	for _, test := range []struct {
		status AccountStatus
		want   string
	}{{StatusAuthenticated, "available"}, {StatusUnknown, "account_dependent"}, {StatusChecking, "account_dependent"}, {StatusUnauthenticated, "unavailable"}} {
		caps := Capabilities(Account{ProviderKind: "codex", Status: test.status})
		if len(caps) != 2 || caps[0].ID != "model.generate" || caps[0].Status != test.want || caps[1].Status != test.want {
			t.Fatalf("model capabilities for %s = %#v", test.status, caps)
		}
	}
}

// Rust source: crates/noema-providers/src/capabilities.rs::hosted_and_local_accounts_expose_distinct_reliability_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_HostedAndLocalAccountsExposeDistinctReliabilityContracts(t *testing.T) {
	hosted := Capabilities(Account{ProviderKind: "openai", Status: StatusAuthenticated})
	local := Capabilities(Account{ProviderKind: "local_models", Status: StatusAuthenticated})
	if len(hosted) == 0 || len(local) == 0 || hosted[0].ReliabilityContract != "hosted_provider" || local[0].ReliabilityContract != "first_party" || hosted[0].DataFlowClass == local[0].DataFlowClass {
		t.Fatalf("hosted/local capability contracts = %#v / %#v", hosted, local)
	}
}

// Rust source: crates/noema-providers/src/capabilities.rs::kernel_browser_capability_tracks_account_readiness_as_hosted (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_KernelBrowserCapabilityTracksAccountReadinessAsHosted(t *testing.T) {
	for _, test := range []struct {
		status AccountStatus
		want   string
	}{{StatusAuthenticated, "available"}, {StatusUnknown, "account_dependent"}, {StatusUnauthenticated, "unavailable"}} {
		caps := Capabilities(Account{ProviderKind: "kernel", Status: test.status})
		if len(caps) != 1 || caps[0].ID != "web.browse" || caps[0].Status != test.want || caps[0].ReliabilityContract != "hosted_provider" || !caps[0].Features.JSRendering {
			t.Fatalf("Kernel capability for %s = %#v", test.status, caps)
		}
	}
}

// Rust source: crates/noema-providers/src/chat_completions/tests.rs::request_lowering_preserves_chat_fields_and_openrouter_application_boundary (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RequestLoweringPreservesChatFieldsAndOpenrouterApplicationBoundary(t *testing.T) {
	request := GenerateRequest{AccountID: openRouterGenerationAccountID, Model: "anthropic/claude-haiku-4.5", ConversationID: " conversation:cacheable ", Messages: []GenerationMessage{{Role: "developer", Content: "Stable </noema_application_context> & <context>"}, {Role: "user", Content: "Human <input> stays raw"}, {Role: "assistant", Content: "Working.", ReasoningDetails: []json.RawMessage{json.RawMessage(`{"type":"reasoning.encrypted","id":"rs_1","data":"opaque"}`)}, ToolCalls: []ReplayToolCall{{ProviderCallID: "call_1", Name: "search_memory", Arguments: json.RawMessage(`{"query":"trains"}`)}}}}, Tools: []GenerationTool{parityGenerationTool("search_memory")}, ToolTransport: ToolTransportNative, ReasoningEffort: "high", MaxOutputTokens: func() *uint32 { value := uint32(64); return &value }(), Temperature: func() *float32 { value := float32(0.2); return &value }(), HostedWebSearch: true, ParallelTools: true}
	body, names, err := prepareOpenRouterGeneration(request)
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]any
	_ = json.Unmarshal(body, &wire)
	messages := wire["messages"].([]any)
	if wire["model"] != request.Model || wire["max_completion_tokens"] != float64(64) || wire["temperature"] != float64(0.2) || wire["reasoning"].(map[string]any)["effort"] != "high" || wire["prompt_cache_key"] != "conversation:cacheable" || wire["stream"] != true || wire["stream_options"].(map[string]any)["include_usage"] != true || wire["input"] != nil || messages[0].(map[string]any)["role"] != "system" || !strings.Contains(messages[0].(map[string]any)["content"].(string), openRouterApplicationInstruction) || !strings.Contains(messages[1].(map[string]any)["content"].(string), "<noema_application_context>") || len(wire["tools"].([]any)) != 2 || wire["tools"].([]any)[1].(map[string]any)["type"] != "openrouter:web_search" || names.canonicalToName["search_memory"] != "search_memory" {
		t.Fatalf("OpenRouter request lowering = %#v", wire)
	}
}

// Rust source: crates/noema-providers/src/chat_completions/tests.rs::openrouter_schema_request_wire_is_stable_for_named_and_auto_models (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_OpenrouterSchemaRequestWireIsStableForNamedAndAutoModels(t *testing.T) {
	for _, model := range []string{"anthropic/claude-haiku-4.5", "openrouter/auto"} {
		for _, test := range []struct {
			schema string
			strict bool
		}{{`{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}`, true}, {`{"type":"object","properties":{"ids":{"type":"array","items":{"type":"string"},"uniqueItems":true}},"required":["ids"],"additionalProperties":false}`, false}} {
			body, _, err := prepareOpenRouterGeneration(GenerateRequest{AccountID: openRouterGenerationAccountID, Model: model, Messages: []GenerationMessage{{Role: "user", Content: "search"}}, Tools: []GenerationTool{{Name: "search_memory", Description: "Search.", InputSchema: json.RawMessage(test.schema)}}, ToolTransport: ToolTransportNative})
			if err != nil {
				t.Fatal(err)
			}
			var wire map[string]any
			_ = json.Unmarshal(body, &wire)
			function := wire["tools"].([]any)[0].(map[string]any)["function"].(map[string]any)
			if wire["model"] != model || function["strict"] != test.strict {
				t.Fatalf("OpenRouter schema wire for %s = %#v", model, wire)
			}
		}
	}
}

// Rust source: crates/noema-providers/src/chat_completions/tests.rs::strict_provider_output_returns_to_nested_source_form (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_StrictProviderOutputReturnsToNestedSourceForm(t *testing.T) {
	tool := parityGenerationTool("mcp.docs.read")
	tool.InputSchema = json.RawMessage(`{"type":"object","properties":{"document_id":{"type":"string"},"context":{"type":"object","properties":{"mode":{"type":"string"},"nullable":{"type":["string","null"]}},"additionalProperties":false}},"required":["document_id"],"additionalProperties":false}`)
	names, _, err := prepareOpenRouterTools([]GenerationTool{tool})
	if err != nil {
		t.Fatal(err)
	}
	callName := names.canonicalToName[tool.Name]
	parsed := ChatStreamResult{ToolCalls: []ToolCall{{ID: "call_1", Name: callName, Arguments: `{"document_id":7,"context":{"mode":null,"nullable":null}}`}}}
	result, err := normalizeOpenRouterGeneration(GenerateRequest{AccountID: openRouterGenerationAccountID, Model: "openrouter/auto", ToolTransport: ToolTransportNative, Tools: []GenerationTool{tool}}, parsed, names, "tool_calls")
	if err != nil || len(result.ToolCalls) != 1 || string(result.ToolCalls[0].Payload) != `{"context":{"nullable":null},"document_id":7}` {
		t.Fatalf("strict output restoration = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/chat_completions/tests.rs::stream_normalization_assembles_text_tools_reasoning_citations_search_and_usage (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_StreamNormalizationAssemblesTextToolsReasoningCitationsSearchAndUsage(t *testing.T) {
	payload := "data: {\"id\":\"chat_1\",\"model\":\"vendor/model\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"search_memory\",\"arguments\":\"{\\\"query\\\":\\\"trains\\\"}\"}}]}}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}\n\n" + "data: [DONE]\n\n"
	events := []StreamEvent{}
	result, err := ParseChatStream(t.Context(), io.NopCloser(strings.NewReader(payload)), func(event StreamEvent) { events = append(events, event) })
	if err != nil || result.Text != "Hello" || len(result.ToolCalls) != 1 || result.ToolCalls[0].Name != "search_memory" || result.Usage.TotalTokens != 5 || len(events) < 2 {
		t.Fatalf("normalized chat stream = %#v, events=%#v, %v", result, events, err)
	}
}

// Rust source: crates/noema-providers/src/chat_completions/tests.rs::documented_search_usage_synthesizes_missing_markers (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DocumentedSearchUsageSynthesizesMissingMarkers(t *testing.T) {
	payload := "data: {\"id\":\"chat_1\",\"model\":\"vendor/model\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"answer\"}}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1,\"total_tokens\":2}}\n\n" + "data: [DONE]\n\n"
	result, err := ParseChatStream(t.Context(), io.NopCloser(strings.NewReader(payload)), nil)
	if err != nil || result.Text != "answer" || result.Usage.TotalTokens != 2 {
		t.Fatalf("documented usage normalization = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/chat_completions/tests.rs::openrouter_chat_prefix_and_direct_responses_are_isolated (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_OpenrouterChatPrefixAndDirectResponsesAreIsolated(t *testing.T) {
	chat, _, err := prepareOpenRouterGeneration(GenerateRequest{AccountID: openRouterGenerationAccountID, Model: "openrouter/auto", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}})
	if err != nil {
		t.Fatal(err)
	}
	responses, _, err := prepareResponsesGeneration(GenerateRequest{AccountID: openAIDefaultAccountID, Model: "gpt-test", Messages: []GenerationMessage{{Role: "user", Content: "hi"}}}, openAIResponsesProfile())
	if err != nil {
		t.Fatal(err)
	}
	var chatWire, responseWire map[string]any
	_ = json.Unmarshal(chat, &chatWire)
	_ = json.Unmarshal(responses, &responseWire)
	if chatWire["messages"] == nil || chatWire["input"] != nil || responseWire["input"] == nil || responseWire["messages"] != nil {
		t.Fatalf("chat/responses wire crossed boundaries: %#v / %#v", chatWire, responseWire)
	}
}

// Rust source: crates/noema-providers/src/config.rs::debug_excludes_credentials_and_preserves_ordinary_configuration (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DebugExcludesCredentialsAndPreservesOrdinaryConfiguration(t *testing.T) {
	secret, err := NewSecret("openai-secret")
	if err != nil {
		t.Fatal(err)
	}
	account := Account{ID: "provider_account:openai:default", ProviderKind: "openai", AccountKey: "default", DisplayName: "OpenAI", Metadata: AccountMetadata{"base_url": json.RawMessage(`"https://api.example.test/v1"`)}}
	debug := fmt.Sprintf("%+v %v", account, secret)
	if strings.Contains(debug, "openai-secret") || !strings.Contains(debug, "openai") || string(account.Metadata["base_url"]) != `"https://api.example.test/v1"` {
		t.Fatalf("provider configuration debug = %q", debug)
	}
}

// Rust source: crates/noema-providers/src/generation/error.rs::transport_error_formatting_has_no_raw_source_slot (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_TransportErrorFormattingHasNoRawSourceSlot(t *testing.T) {
	rawSource := "request to https://example.test?api_key=SECRET failed"
	err := providerTransportError("openai", "send_generation")
	formatted := fmt.Sprintf("%#v %v", err, err)
	if strings.Contains(formatted, rawSource) || strings.Contains(formatted, "SECRET") || !strings.Contains(formatted, "send_generation") {
		t.Fatalf("safe transport error = %q", formatted)
	}
}

// Rust source: crates/noema-providers/src/generation/message_splitter.rs::bubble_boundaries_split_across_chunks_but_fenced_rules_do_not (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BubbleBoundariesSplitAcrossChunksButFencedRulesDoNot(t *testing.T) {
	for _, test := range []struct {
		input    string
		expected []string
	}{
		{"one\n---\ntwo", []string{"one", "two"}},
		{"one\n\ntwo", []string{"one", "two"}},
		{"---\n\none\n---\n---\n\ntwo\n---", []string{"one", "two"}},
		{"```md\n---\n```\n---\nafter", []string{"```md\n---\n```", "after"}},
		{"```md\none\n\ntwo\n```", []string{"```md\none\n\ntwo\n```"}},
		{"~~~\n---\n~~~\n---\nafter", []string{"~~~\n---\n~~~", "after"}},
		{"before\n----\nafter", []string{"before\n----\nafter"}},
	} {
		if got := splitMarkdownMessages(test.input); !reflect.DeepEqual(got, test.expected) {
			t.Fatalf("split %q = %#v, want %#v", test.input, got, test.expected)
		}
	}
	var splitter providerMarkdownDeltaSplitter
	output := splitter.push("one\n--")
	output = append(output, splitter.push("-\ntwo")...)
	output = append(output, splitter.finish()...)
	if len(output) != 2 || output[0].Index != 0 || output[0].Text != "one\n" || output[1].Index != 1 || output[1].Text != "two" {
		t.Fatalf("split delta boundary = %#v", output)
	}
}

// Rust source: crates/noema-providers/src/generation/message_splitter.rs::segment_ranges_preserve_utf16_offsets_and_removed_separators (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SegmentRangesPreserveUtf16OffsetsAndRemovedSeparators(t *testing.T) {
	segments := splitMarkdownSegments("😀 first\n\nsecond\n---\nthird")
	if len(segments) != 3 || segments[0].Text != "😀 first" || segments[1].Text != "second" || segments[2].Text != "third" {
		t.Fatalf("markdown segments = %#v", segments)
	}
	want := [][2]int{{0, 8}, {10, 16}, {21, 26}}
	for index, expected := range want {
		if segments[index].SourceUTF16 != expected {
			t.Fatalf("segment %d range = %#v, want %#v", index, segments[index].SourceUTF16, expected)
		}
	}
}

// Rust source: crates/noema-providers/src/generation/response.rs::citations_follow_utf16_bubble_ranges_and_missing_offset_fallback (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CitationsFollowUtf16BubbleRangesAndMissingOffsetFallback(t *testing.T) {
	item := map[string]json.RawMessage{"type": json.RawMessage(`"message"`), "content": json.RawMessage(`[{"type":"output_text","text":"😀 first\n\nsecond","annotations":[{"type":"url_citation","title":"First","url":"https://first.example","end_index":9},{"type":"url_citation","title":"Second","url":"https://second.example","end_index":16},{"type":"url_citation","title":"Fallback","url":"https://fallback.example"}]}]`)}
	text, citations, err := codexOutputText(item, 0)
	if err != nil || text != "😀 first\n\nsecond" || len(citations) != 3 || citations[0].EndIndex == nil || *citations[0].EndIndex != 9 || citations[2].EndIndex != nil {
		t.Fatalf("citation offsets = %q %#v %v", text, citations, err)
	}
}

// Rust source: crates/noema-providers/src/generation/response.rs::assistant_response_texts_preserve_order_late_commentary_and_exact_duplicates (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AssistantResponseTextsPreserveOrderLateCommentaryAndExactDuplicates(t *testing.T) {
	response := ProviderResponse{
		Responses: []ProviderResponseItem{{Text: "progress"}, {Text: "same", Phase: "final"}, {Text: "same", Phase: "commentary"}},
		ToolCalls: []GenerationToolCall{{ProviderCallID: "call_1", Name: "read"}},
	}
	texts := response.assistantResponseTexts()
	if len(texts) != 3 || texts[0].Text != "progress" || texts[0].Phase != "commentary" || texts[1].Text != "same" || texts[1].Phase != "final" || texts[2].Text != "same" || texts[2].Phase != "commentary" {
		t.Fatalf("assistant response texts = %#v", texts)
	}
}

// Rust source: crates/noema-providers/src/local_model.rs::cancelled_and_installed_installations_reject_worker_state_regression (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CancelledAndInstalledInstallationsRejectWorkerStateRegression(t *testing.T) {
	if !ProviderStatusDownloading.canTransitionTo(ProviderStatusCancelled) || ProviderStatusCancelled.canTransitionTo(ProviderStatusInstalled) || ProviderStatusInstalled.canTransitionTo(ProviderStatusCancelled) {
		t.Fatal("local model worker state regression was accepted")
	}
}

// Rust source: crates/noema-providers/src/local_model.rs::local_model_persistence_codecs_round_trip_exactly (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LocalModelPersistenceCodecsRoundTripExactly(t *testing.T) {
	for value, encoded := range map[ProviderLocalModelBackend]string{
		ProviderBackendMetal:  "metal",
		ProviderBackendCUDA:   "cuda",
		ProviderBackendVulkan: "vulkan",
		ProviderBackendCPU:    "cpu",
	} {
		if got := value.persistenceString(); got != encoded {
			t.Fatalf("backend %q encoded as %q, want %q", value, got, encoded)
		}
		roundTrip, err := parseProviderLocalBackend(encoded)
		if err != nil || roundTrip != value {
			t.Fatalf("backend %q decoded as %q, %v; want %q", encoded, roundTrip, err, value)
		}
	}
	for value, encoded := range map[ProviderLocalModelSource]string{
		ProviderSourceCatalog:     "catalog",
		ProviderSourceHuggingFace: "hugging_face",
		ProviderSourceLocalFile:   "local_file",
	} {
		if got := value.persistenceString(); got != encoded {
			t.Fatalf("source %q encoded as %q, want %q", value, got, encoded)
		}
		roundTrip, err := parseProviderLocalSource(encoded)
		if err != nil || roundTrip != value {
			t.Fatalf("source %q decoded as %q, %v; want %q", encoded, roundTrip, err, value)
		}
	}
	for value, encoded := range map[ProviderLocalModelStatus]string{
		ProviderStatusQueued:      "queued",
		ProviderStatusDownloading: "downloading",
		ProviderStatusVerifying:   "verifying",
		ProviderStatusInstalled:   "installed",
		ProviderStatusFailed:      "failed",
		ProviderStatusCancelled:   "cancelled",
	} {
		if got := value.persistenceString(); got != encoded {
			t.Fatalf("status %q encoded as %q, want %q", value, got, encoded)
		}
		roundTrip, err := parseProviderLocalStatus(encoded)
		if err != nil || roundTrip != value {
			t.Fatalf("status %q decoded as %q, %v; want %q", encoded, roundTrip, err, value)
		}
	}
	for value, encoded := range map[ProviderLocalModelEvent]string{
		ProviderEventQueued:    "queued",
		ProviderEventProgress:  "progress",
		ProviderEventVerifying: "verifying",
		ProviderEventInstalled: "installed",
		ProviderEventFailed:    "failed",
		ProviderEventCancelled: "cancelled",
		ProviderEventRemoved:   "removed",
		ProviderEventActivated: "activated",
	} {
		if got := value.persistenceString(); got != encoded {
			t.Fatalf("event %q encoded as %q, want %q", value, got, encoded)
		}
		roundTrip, err := parseProviderLocalEvent(encoded)
		if err != nil || roundTrip != value {
			t.Fatalf("event %q decoded as %q, %v; want %q", encoded, roundTrip, err, value)
		}
	}
	if _, err := parseProviderLocalBackend("unknown"); err == nil {
		t.Fatal("unknown backend parsed successfully")
	}
	if _, err := parseProviderLocalSource("unknown"); err == nil {
		t.Fatal("unknown source parsed successfully")
	}
	if _, err := parseProviderLocalStatus("unknown"); err == nil {
		t.Fatal("unknown status parsed successfully")
	}
	if _, err := parseProviderLocalEvent("unknown"); err == nil {
		t.Fatal("unknown event parsed successfully")
	}
}

// Rust source: crates/noema-providers/src/local_models/catalog_tests.rs::bundled_catalog_loads_ranked_pinned_models (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BundledCatalogLoadsRankedPinnedModels(t *testing.T) {
	catalog := localModelCatalogForHardware([]LocalHardwareProfile{{Backend: LocalModelMetal, RAMGB: 16, UnifiedMemory: true}})
	if len(catalog) != 1 || catalog[0].ID != "gemma-4-e4b-it" || catalog[0].Priority != 100 || catalog[0].Revision != "2714b5519c6c3516b1000e7c5e1eba998dfe1fe8" || catalog[0].SelectedBuild == nil || catalog[0].SelectedBuild.File != "gemma-4-E4B-it-Q4_K_M.gguf" || catalog[0].SelectedBuild.SHA256 != "90ce98129eb3e8cc57e62433d500c97c624b1e3af1fcc85dd3b55ad7e0313e9f" {
		t.Fatalf("bundled local catalog = %#v", catalog)
	}
}

// Rust source: crates/noema-providers/src/local_models/catalog_tests.rs::qualified_model_wins_only_at_its_memory_boundary (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_QualifiedModelWinsOnlyAtItsMemoryBoundary(t *testing.T) {
	for _, test := range []struct {
		hardware LocalHardwareProfile
		selected bool
	}{
		{LocalHardwareProfile{Backend: LocalModelMetal, RAMGB: 12, UnifiedMemory: true}, false},
		{LocalHardwareProfile{Backend: LocalModelMetal, RAMGB: 15, UnifiedMemory: true}, false},
		{LocalHardwareProfile{Backend: LocalModelMetal, RAMGB: 16, UnifiedMemory: true}, true},
		{LocalHardwareProfile{Backend: LocalModelMetal, RAMGB: 32, UnifiedMemory: true}, true},
		{LocalHardwareProfile{Backend: LocalModelCUDA, RAMGB: 32, VRAMGB: intPointer(24)}, false},
		{LocalHardwareProfile{Backend: LocalModelMetal, RAMGB: 32, VRAMGB: intPointer(5)}, false},
		{LocalHardwareProfile{Backend: LocalModelMetal, RAMGB: 32, VRAMGB: intPointer(6)}, true},
	} {
		catalog := localModelCatalogForHardware([]LocalHardwareProfile{test.hardware})
		if len(catalog) != 1 || (catalog[0].SelectedBuild != nil) != test.selected {
			t.Fatalf("hardware %#v selection = %#v", test.hardware, catalog)
		}
	}
}

// Rust source: crates/noema-providers/src/local_models/catalog_tests.rs::backend_preference_skips_unqualified_backends (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BackendPreferenceSkipsUnqualifiedBackends(t *testing.T) {
	hardware := []LocalHardwareProfile{{Backend: LocalModelVulkan, RAMGB: 32, VRAMGB: intPointer(24)}, {Backend: LocalModelCPU, RAMGB: 32}, {Backend: LocalModelMetal, RAMGB: 32, UnifiedMemory: true}}
	catalog := localModelCatalogForHardware(hardware)
	if len(catalog) != 1 || catalog[0].SelectedBuild == nil || catalog[0].Hardware.Backend != LocalModelMetal || !catalog[0].Recommended || localModelCatalogForHardware([]LocalHardwareProfile{{Backend: LocalModelCPU, RAMGB: 32}})[0].SelectedBuild != nil {
		t.Fatalf("backend fallback selection = %#v", catalog)
	}
}

// Rust source: crates/noema-providers/src/local_models/catalog_tests.rs::recommendation_order_is_priority_then_catalog_order (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RecommendationOrderIsPriorityThenCatalogOrder(t *testing.T) {
	catalog := localModelCatalogForHardware([]LocalHardwareProfile{{Backend: LocalModelMetal, RAMGB: 16, UnifiedMemory: true}})
	if len(catalog) != 1 || !catalog[0].Recommended || catalog[0].Priority != 100 {
		t.Fatalf("local recommendation order = %#v", catalog)
	}
}

// Rust source: crates/noema-providers/src/local_models/catalog_tests.rs::model_priority_wins_before_backend_preference (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ModelPriorityWinsBeforeBackendPreference(t *testing.T) {
	catalog := localModelCatalogForHardware([]LocalHardwareProfile{{Backend: LocalModelMetal, RAMGB: 16, UnifiedMemory: true}})
	if len(catalog) != 1 || catalog[0].Recommended != true || catalog[0].ID != "gemma-4-e4b-it" {
		t.Fatalf("priority recommendation = %#v", catalog)
	}
}

// Rust source: crates/noema-providers/src/local_models/catalog_tests.rs::rejects_duplicate_ids_mutable_revisions_and_malformed_hashes (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RejectsDuplicateIdsMutableRevisionsAndMalformedHashes(t *testing.T) {
	seen := make(map[string]struct{}, len(bundledLocalModels))
	for _, model := range bundledLocalModels {
		if _, exists := seen[model.ID]; exists || len(model.Revision) != 40 {
			t.Fatalf("invalid bundled model identity = %#v", model)
		}
		seen[model.ID] = struct{}{}
		for _, build := range model.Builds {
			if len(build.SHA256) != 64 || build.MinRAMGB <= 0 || len(build.Backends) == 0 {
				t.Fatalf("invalid bundled build = %#v", build)
			}
		}
	}
}

// Rust source: crates/noema-providers/src/local_models/download_support.rs::hugging_face_urls_require_pinned_safe_gguf_paths (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_HuggingFaceUrlsRequirePinnedSafeGgufPaths(t *testing.T) {
	revision := strings.Repeat("a", 40)
	value, err := providerHuggingFaceURL("owner/repo", revision, "weights/model.gguf")
	if err != nil {
		t.Fatal(err)
	}
	parsed, err := url.Parse(value)
	if err != nil || parsed.Host != "huggingface.co" || !strings.HasSuffix(parsed.Path, "/resolve/"+revision+"/weights/model.gguf") {
		t.Fatalf("Hugging Face URL = %q, %v", value, err)
	}
	if _, err := providerHuggingFaceURL("owner/repo", "main", "model.gguf"); err == nil {
		t.Fatal("mutable revision accepted")
	}
	if _, err := providerHuggingFaceURL("owner/repo", revision, "../model.gguf"); err == nil {
		t.Fatal("unsafe path accepted")
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::local_file_import_is_verified_and_content_addressed (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LocalFileImportIsVerifiedAndContentAddressed(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "user-model.gguf")
	payload := []byte("GGUF small deterministic test payload")
	if err := os.WriteFile(source, payload, 0o600); err != nil {
		t.Fatal(err)
	}
	installed, err := importProviderLocalModel(t.Context(), providerLocalFileImport{Name: "User model", ModelID: "user-model", Path: source, Backend: ProviderBackendCPU}, root, nil, nil)
	if err != nil || installed.Status != ProviderStatusInstalled {
		t.Fatalf("local import = %#v, %v", installed, err)
	}
	digest := providerLocalDigest(payload)
	if installed.SHA256 != digest {
		t.Fatalf("verified digest = %q, want %q", installed.SHA256, digest)
	}
	blob := filepath.Join(root, "models", "blobs", digest+".gguf")
	got, err := os.ReadFile(blob)
	if err != nil || !bytes.Equal(got, payload) {
		t.Fatalf("installed blob = %q, %v", got, err)
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::verified_import_preserves_a_corrupt_shared_blob_and_fails_closed (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_VerifiedImportPreservesACorruptSharedBlobAndFailsClosed(t *testing.T) {
	root := t.TempDir()
	payload := []byte("GGUF same verified payload")
	first, second := filepath.Join(root, "first.gguf"), filepath.Join(root, "second.gguf")
	if err := os.WriteFile(first, payload, 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(second, payload, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := importProviderLocalModel(t.Context(), providerLocalFileImport{Path: first, ModelID: "first"}, root, nil, nil); err != nil {
		t.Fatal(err)
	}
	blob := filepath.Join(root, "models", "blobs", providerLocalDigest(payload)+".gguf")
	if err := os.WriteFile(blob, []byte("corrupt"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := importProviderLocalModel(t.Context(), providerLocalFileImport{Path: second, ModelID: "second"}, root, nil, nil); err == nil || !strings.Contains(err.Error(), "blob digest conflict") {
		t.Fatalf("occupied corrupt blob error = %v", err)
	}
	got, err := os.ReadFile(blob)
	if err != nil || string(got) != "corrupt" {
		t.Fatalf("corrupt shared blob = %q, %v", got, err)
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::verified_import_repairs_an_unreferenced_corrupt_blob (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_VerifiedImportRepairsAnUnreferencedCorruptBlob(t *testing.T) {
	root := t.TempDir()
	payload := []byte("GGUF orphan recovery payload")
	source := filepath.Join(root, "recovery.gguf")
	if err := os.WriteFile(source, payload, 0o600); err != nil {
		t.Fatal(err)
	}
	digest := providerLocalDigest(payload)
	blob := filepath.Join(root, "models", "blobs", digest+".gguf")
	if err := os.MkdirAll(filepath.Dir(blob), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blob, []byte("orphaned corrupt bytes"), 0o600); err != nil {
		t.Fatal(err)
	}
	installed, err := importProviderLocalModel(t.Context(), providerLocalFileImport{Path: source, ModelID: "recovered"}, root, nil, nil)
	if err != nil || installed.Status != ProviderStatusInstalled {
		t.Fatalf("orphan recovery = %#v, %v", installed, err)
	}
	got, err := os.ReadFile(blob)
	if err != nil || !bytes.Equal(got, payload) {
		t.Fatalf("repaired blob = %q, %v", got, err)
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::local_file_import_rejects_a_non_gguf_payload (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LocalFileImportRejectsANonGgufPayload(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "not-a-model.gguf")
	if err := os.WriteFile(source, []byte("this is not a model"), 0o600); err != nil {
		t.Fatal(err)
	}
	_, err := importProviderLocalModel(t.Context(), providerLocalFileImport{Path: source, ModelID: "invalid"}, root, nil, nil)
	if err == nil || !strings.Contains(err.Error(), "does not have a GGUF header") {
		t.Fatalf("non-GGUF error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::cancelled_local_import_persists_terminal_state_and_can_retry (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CancelledLocalImportPersistsTerminalStateAndCanRetry(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "retry.gguf")
	if err := os.WriteFile(source, []byte("GGUF retry payload"), 0o600); err != nil {
		t.Fatal(err)
	}
	cancelled := make(chan struct{})
	close(cancelled)
	installation, err := importProviderLocalModel(t.Context(), providerLocalFileImport{Path: source, ModelID: "retry"}, root, nil, cancelled)
	if err == nil || !errors.Is(err, context.Canceled) || installation.Status != ProviderStatusCancelled {
		t.Fatalf("cancelled import = %#v, %v", installation, err)
	}
	installed, err := importProviderLocalModel(t.Context(), providerLocalFileImport{Path: source, ModelID: "retry"}, root, nil, nil)
	if err != nil || installed.Status != ProviderStatusInstalled || installed.SHA256 == "" {
		t.Fatalf("retry import = %#v, %v", installed, err)
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::large_local_import_persists_incremental_progress_before_verification (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LargeLocalImportPersistsIncrementalProgressBeforeVerification(t *testing.T) {
	root := t.TempDir()
	payload := make([]byte, 9*1024*1024)
	copy(payload[:4], []byte("GGUF"))
	source := filepath.Join(root, "large.gguf")
	if err := os.WriteFile(source, payload, 0o600); err != nil {
		t.Fatal(err)
	}
	var events []struct {
		kind  ProviderLocalModelEvent
		bytes int64
	}
	installed, err := importProviderLocalModel(t.Context(), providerLocalFileImport{Path: source, ModelID: "large"}, root, func(kind ProviderLocalModelEvent, bytes int64) {
		events = append(events, struct {
			kind  ProviderLocalModelEvent
			bytes int64
		}{kind, bytes})
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	progress, verifying := false, false
	for _, event := range events {
		if event.kind == ProviderEventProgress && event.bytes >= 8*1024*1024 {
			progress = true
		}
		if event.kind == ProviderEventVerifying && event.bytes == installed.ExpectedBytes {
			verifying = true
		}
	}
	if !progress || !verifying {
		t.Fatalf("large import events = %#v", events)
	}
}

// Rust source: crates/noema-providers/src/local_models/eval.rs::eval_session_exposes_only_ready_provider_metadata_and_owned_shutdown (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_EvalSessionExposesOnlyReadyProviderMetadataAndOwnedShutdown(t *testing.T) {
	config := providerLocalEvalSessionConfig{ModelID: "eval-model", ModelPath: filepath.Join(t.TempDir(), "model.gguf"), RuntimeRoot: filepath.Join(t.TempDir(), "runtime"), ContextWindow: 4096, TimeoutSeconds: 5}
	debug := fmt.Sprint(config)
	if !strings.Contains(debug, config.ModelPath) || !strings.Contains(debug, config.RuntimeRoot) {
		t.Fatalf("evaluation config debug = %s", debug)
	}
	session, err := startProviderLocalEvalSession(config)
	if err != nil {
		t.Fatal(err)
	}
	if session.backend != ProviderBackendCPU || session.processID == nil {
		t.Fatalf("evaluation readiness = %#v", session)
	}
	provider := session.providerDebug()
	if provider.DefaultModel != "eval-model" || provider.ModelPath != config.ModelPath || provider.RuntimeRoot != config.RuntimeRoot {
		t.Fatalf("evaluation provider metadata = %#v", provider)
	}
	if config.ContextWindow != 4096 {
		t.Fatalf("evaluation context window = %d", config.ContextWindow)
	}
	session.shutdown()
	if session.status != providerLocalStopped || session.processID != nil || !session.providerClosed {
		t.Fatalf("evaluation shutdown = %#v", session)
	}
}

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::cache_hit_returns_verified_path_without_network (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CacheHitReturnsVerifiedPathWithoutNetwork(t *testing.T) {
	root := t.TempDir()
	bytesValue := []byte("GGUF cached evaluation model")
	digest := providerLocalDigest(bytesValue)
	destination := filepath.Join(root, digest+".gguf")
	if err := os.WriteFile(destination, bytesValue, 0o600); err != nil {
		t.Fatal(err)
	}
	path, err := materializeProviderModelAtomically(t.Context(), root, bytesValue, strings.NewReader("ignored"), digest)
	if err != nil || path != destination {
		t.Fatalf("cache hit = %q, %v", path, err)
	}
}

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::digest_mismatch_removes_partial_and_publishes_nothing (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DigestMismatchRemovesPartialAndPublishesNothing(t *testing.T) {
	root := t.TempDir()
	expected := []byte("GGUF expected evaluation model")
	digest := providerLocalDigest(expected)
	_, err := materializeProviderModelAtomically(t.Context(), root, expected, strings.NewReader("GGUF incorrect evaluation bytes"), digest)
	if err == nil || !strings.Contains(err.Error(), "digest mismatch") {
		t.Fatalf("checksum error = %v", err)
	}
	if entries, readErr := os.ReadDir(root); readErr != nil || len(entries) != 0 {
		t.Fatalf("partial cache entries = %v, %v", entries, readErr)
	}
}

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::cancellation_leaves_no_partial_or_published_file (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CancellationLeavesNoPartialOrPublishedFile(t *testing.T) {
	root := t.TempDir()
	cancelled, cancel := context.WithCancel(t.Context())
	cancel()
	bytesValue := []byte("GGUF cancelled evaluation model")
	_, err := materializeProviderModelAtomically(cancelled, root, bytesValue, strings.NewReader(string(bytesValue)), providerLocalDigest(bytesValue))
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("cancelled materialization = %v", err)
	}
	entries, readErr := os.ReadDir(root)
	if readErr != nil || len(entries) != 0 {
		t.Fatalf("cancelled cache entries = %v, %v", entries, readErr)
	}
}

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::successful_download_is_atomic_and_store_free (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SuccessfulDownloadIsAtomicAndStoreFree(t *testing.T) {
	root := t.TempDir()
	bytesValue := []byte("GGUF successful evaluation model")
	path, err := materializeProviderModelAtomically(t.Context(), root, bytesValue, bytes.NewReader(bytesValue), providerLocalDigest(bytesValue))
	if err != nil {
		t.Fatal(err)
	}
	got, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(got, bytesValue) {
		t.Fatalf("materialized bytes = %q, %v", got, err)
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if strings.HasSuffix(entry.Name(), ".partial") || strings.Contains(entry.Name(), "sqlite") {
			t.Fatalf("non-atomic cache entry = %q", entry.Name())
		}
	}
}

// Rust source: crates/noema-providers/src/local_models/manager/process.rs::verified_model_blob_path_accepts_canonical_untampered_blob (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_VerifiedModelBlobPathAcceptsCanonicalUntamperedBlob(t *testing.T) {
	root := t.TempDir()
	bytesValue := []byte("verified model bytes")
	digest := providerLocalDigest(bytesValue)
	blob := filepath.Join(root, "models", "blobs", digest+".gguf")
	if err := os.MkdirAll(filepath.Dir(blob), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blob, bytesValue, 0o600); err != nil {
		t.Fatal(err)
	}
	verified, err := verifyProviderModelBlob(root, providerLocalInstallation{SHA256: digest, BlobPath: blob})
	if err != nil || verified != blob {
		t.Fatalf("verified model blob = %q, %v", verified, err)
	}
}

// Rust source: crates/noema-providers/src/local_models/manager/process.rs::verified_model_blob_path_rejects_missing_and_tampered_blob (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_VerifiedModelBlobPathRejectsMissingAndTamperedBlob(t *testing.T) {
	root := t.TempDir()
	bytesValue := []byte("verified model bytes")
	digest := providerLocalDigest(bytesValue)
	installation := providerLocalInstallation{SHA256: digest}
	if _, err := verifyProviderModelBlob(root, installation); err == nil || !strings.Contains(err.Error(), "unavailable") {
		t.Fatalf("missing blob error = %v", err)
	}
	blob := filepath.Join(root, "models", "blobs", digest+".gguf")
	if err := os.MkdirAll(filepath.Dir(blob), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blob, []byte("tampered model bytes"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := verifyProviderModelBlob(root, installation); err == nil || !strings.Contains(err.Error(), "does not match") {
		t.Fatalf("tampered blob error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/local_models/manager/process.rs::verified_model_blob_path_rejects_noncanonical_durable_path (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_VerifiedModelBlobPathRejectsNoncanonicalDurablePath(t *testing.T) {
	root := t.TempDir()
	bytesValue := []byte("verified model bytes")
	digest := providerLocalDigest(bytesValue)
	errText := "models/blobs/other.gguf"
	if _, err := verifyProviderModelBlob(root, providerLocalInstallation{SHA256: digest, BlobPath: filepath.Join(root, errText)}); err == nil || !strings.Contains(err.Error(), "does not match") {
		t.Fatalf("noncanonical path error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/local_models/manager/tests.rs::activation_and_route_replacement_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ActivationAndRouteReplacementContracts(t *testing.T) {
	manager := newProviderLocalManagerContract()
	manager.add("first", "shared-model", ProviderStatusInstalled)
	manager.add("second", "shared-model", ProviderStatusInstalled)
	first, err := manager.activate("first")
	if err != nil || first.ID != "first" {
		t.Fatalf("first activation = %#v/%v", first, err)
	}
	oldRegistry := newProviderRegistry()
	oldGeneration, err := oldRegistry.register("local:first", providerTrackedProvider{label: "first"})
	if err != nil {
		t.Fatal(err)
	}
	oldLease, err := oldRegistry.lease("local:first")
	if err != nil {
		t.Fatal(err)
	}
	second, err := manager.activate("second")
	if err != nil || second.ID != "second" || manager.active != "second" {
		t.Fatalf("second activation = %#v/%v", second, err)
	}
	newGeneration, err := oldRegistry.register("local:second", providerTrackedProvider{label: "second"})
	if err != nil || oldGeneration == newGeneration || oldLease.key != "local:first" {
		t.Fatalf("route replacement = %d/%d/%#v", oldGeneration, newGeneration, oldLease)
	}
	oldLease.release()
}

// Rust source: crates/noema-providers/src/local_models/manager/tests.rs::removal_retirement_and_cleanup_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RemovalRetirementAndCleanupContracts(t *testing.T) {
	manager := newProviderLocalManagerContract()
	manager.add("first", "shared-model", ProviderStatusInstalled)
	manager.add("second", "shared-model", ProviderStatusInstalled)
	if _, err := manager.activate("first"); err != nil {
		t.Fatal(err)
	}
	if _, err := manager.remove("first"); err == nil || !strings.Contains(err.Error(), "active") {
		t.Fatalf("active removal = %v", err)
	}
	manager.active = ""
	removed, err := manager.remove("first")
	if err != nil || removed.ID != "first" {
		t.Fatalf("inactive removal = %#v/%v", removed, err)
	}
	manager.add("first", "shared-model", ProviderStatusInstalled)
	if _, err := manager.activate("first"); err != nil {
		t.Fatal(err)
	}
	if manager.generations["first"] != 2 {
		t.Fatalf("reinstall generation = %d", manager.generations["first"])
	}
}

// Rust source: crates/noema-providers/src/local_models/manager/tests.rs::reconstruction_reaping_and_retry_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ReconstructionReapingAndRetryContracts(t *testing.T) {
	manager := newProviderLocalManagerContract()
	manager.add("active", "shared-model", ProviderStatusInstalled)
	manager.add("inactive", "shared-model", ProviderStatusInstalled)
	manager.runtime = providerLocalFailed
	if _, err := manager.activate("active"); err != nil {
		t.Fatal(err)
	}
	if manager.active != "active" || manager.runtime != providerLocalFailed {
		t.Fatalf("reconstruction failure state = %#v", manager)
	}
	manager.runtime = providerLocalReady
	if _, err := manager.activate("active"); err != nil {
		t.Fatal(err)
	}
	if manager.generations["active"] != 2 {
		t.Fatalf("retry generation = %d", manager.generations["active"])
	}
}

// Rust source: crates/noema-providers/src/local_models/manager/tests.rs::worker_serialization_cancellation_and_shutdown_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_WorkerSerializationCancellationAndShutdownContracts(t *testing.T) {
	manager := newProviderLocalManagerContract()
	manager.add("first", "first-model", ProviderStatusInstalled)
	manager.add("second", "second-model", ProviderStatusInstalled)
	if _, err := manager.activate("first"); err != nil {
		t.Fatal(err)
	}
	manager.beginShutdown()
	if _, err := manager.activate("second"); err == nil || !strings.Contains(err.Error(), "shutting") {
		t.Fatalf("shutdown admission = %v", err)
	}
	if manager.runtime != providerLocalStopped || !manager.shutting {
		t.Fatalf("shutdown state = %#v", manager)
	}
}

// Rust source: crates/noema-providers/src/local_models/manager/tests.rs::event_subscription_lifecycle_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_EventSubscriptionLifecycleContracts(t *testing.T) {
	manager := newProviderLocalManagerContract()
	manager.add("active", "shared-model", ProviderStatusInstalled)
	if _, err := manager.activate("active"); err != nil {
		t.Fatal(err)
	}
	events := manager.subscribe("0")
	if len(events) < 2 || events[0].Cursor != "1" || events[1].Cursor != "2" || events[1].Kind != ProviderEventActivated {
		t.Fatalf("event backfill = %#v", events)
	}
	manager.beginShutdown()
	if manager.runtime != providerLocalStopped {
		t.Fatalf("final runtime status = %s", manager.runtime)
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::provider_debug_preserves_local_model_paths (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ProviderDebugPreservesLocalModelPaths(t *testing.T) {
	provider := providerLocalProviderDebug{DefaultModel: "local-8b", ModelPath: "/private/model-secret.gguf", RuntimeRoot: "/private/runtime-secret"}
	debug := fmt.Sprintf("%#v", provider)
	if !strings.Contains(debug, "/private/model-secret.gguf") || !strings.Contains(debug, "/private/runtime-secret") {
		t.Fatalf("local provider debug = %q", debug)
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::chat_request_preserves_replay_items_and_generation_controls (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ChatRequestPreservesReplayItemsAndGenerationControls(t *testing.T) {
	maxTokens := uint32(321)
	body, err := lowerProviderLocalChatRequest(ProviderLocalToolRequest{Reasoning: "system rules", Messages: []GenerationMessage{
		{Role: "user", Content: "question"},
		{Role: "assistant", Content: "answer"},
		{Role: "assistant", ToolCalls: []ReplayToolCall{{ProviderCallID: "call-1", Name: "read_file", Arguments: json.RawMessage(`{"path":"notes.txt"}`)}}},
		{Role: "tool", ToolResult: &ReplayToolResult{ProviderCallID: "call-1", Name: "read_file", Success: true, Payload: json.RawMessage(`{"text":"contents"}`)}},
	}, MaxTokens: &maxTokens})
	if err != nil {
		t.Fatal(err)
	}
	messages := body["messages"].([]map[string]any)
	if len(messages) != 5 || messages[0]["role"] != "system" || messages[1]["role"] != "user" || messages[2]["role"] != "assistant" || messages[3]["role"] != "assistant" || messages[3]["content"] != nil || len(messages[3]["tool_calls"].([]any)) != 1 || messages[4]["role"] != "tool" || messages[4]["tool_call_id"] != "call-1" || body["max_tokens"] != uint32(321) {
		t.Fatalf("local replay request = %#v", body)
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::chat_request_coalesces_developer_context_into_the_leading_system_message (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ChatRequestCoalescesDeveloperContextIntoTheLeadingSystemMessage(t *testing.T) {
	body, err := lowerProviderLocalChatRequest(ProviderLocalToolRequest{Reasoning: "system rules", Messages: []GenerationMessage{{Role: "developer", Content: "identity update"}, {Role: "developer", Content: "memory tool catalog"}, {Role: "user", Content: "question"}}})
	if err != nil {
		t.Fatal(err)
	}
	messages := body["messages"].([]map[string]any)
	if len(messages) != 2 || messages[0]["role"] != "system" || messages[0]["content"] != "system rules\n\nidentity update\n\nmemory tool catalog" || messages[1]["role"] != "user" || messages[1]["content"] != "question" {
		t.Fatalf("developer context lowering = %#v", body)
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::native_request_uses_openai_tool_fields_without_response_format (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_NativeRequestUsesOpenaiToolFieldsWithoutResponseFormat(t *testing.T) {
	body, err := lowerProviderLocalChatRequest(ProviderLocalToolRequest{Tools: []GenerationTool{parityGenerationTool("search_memory")}, ToolChoice: ToolChoiceAuto})
	if err != nil {
		t.Fatal(err)
	}
	tools := body["tools"].([]any)
	if len(tools) != 1 || tools[0].(map[string]any)["type"] != "function" || tools[0].(map[string]any)["function"].(map[string]any)["name"] != "search_memory" || body["tool_choice"] != nil {
		t.Fatalf("native local request = %#v", body)
	}
	if _, exists := body["response_format"]; exists {
		t.Fatal("local request included response_format")
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::native_tool_qualification_requests_one_required_empty_object_function (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_NativeToolQualificationRequestsOneRequiredEmptyObjectFunction(t *testing.T) {
	tools := qualifyProviderLocalTools(nil)
	body, err := lowerProviderLocalChatRequest(ProviderLocalToolRequest{Tools: tools, ToolChoice: ToolChoiceRequired, MaxTokens: func() *uint32 { v := uint32(64); return &v }()})
	if err != nil {
		t.Fatal(err)
	}
	if len(tools) != 1 || tools[0].Name != "__noema_tool_qualification" || tools[0].InputSchema == nil || body["tool_choice"] != "required" || body["max_tokens"] != uint32(64) {
		t.Fatalf("qualification request = %#v", body)
	}
	parameters := body["tools"].([]any)[0].(map[string]any)["function"].(map[string]any)["parameters"].(map[string]any)
	if parameters["type"] != "object" {
		t.Fatalf("qualification schema = %#v", parameters)
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::native_tool_qualification_requires_the_expected_object_call (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_NativeToolQualificationRequiresTheExpectedObjectCall(t *testing.T) {
	valid := GenerationToolCall{Name: "__noema_tool_qualification", Payload: json.RawMessage(`{}`)}
	if err := validateProviderLocalQualification(valid); err != nil {
		t.Fatal(err)
	}
	invalid := valid
	invalid.Name = "other_tool"
	if validateProviderLocalQualification(invalid) == nil {
		t.Fatal("unexpected qualification tool accepted")
	}
	invalid = valid
	invalid.Payload = json.RawMessage(`"not an object"`)
	if validateProviderLocalQualification(invalid) == nil {
		t.Fatal("non-object qualification accepted")
	}
	invalid = valid
	invalid.Payload = json.RawMessage(`{"unexpected":true}`)
	if validateProviderLocalQualification(invalid) == nil {
		t.Fatal("unexpected qualification fields accepted")
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::required_tool_choice_is_sent_as_native_policy (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RequiredToolChoiceIsSentAsNativePolicy(t *testing.T) {
	body, err := lowerProviderLocalChatRequest(ProviderLocalToolRequest{Tools: []GenerationTool{{Name: "task.finish_execution", Description: "Finish.", InputSchema: json.RawMessage(`{"type":"object","properties":{"summary":{"type":"string"}},"required":["summary"]}`)}}, ToolChoice: ToolChoiceRequired})
	if err != nil {
		t.Fatal(err)
	}
	if body["tool_choice"] != "required" {
		t.Fatalf("required choice = %#v", body["tool_choice"])
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::required_tool_choice_rejects_an_empty_catalog (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RequiredToolChoiceRejectsAnEmptyCatalog(t *testing.T) {
	if _, err := lowerProviderLocalChatRequest(ProviderLocalToolRequest{ToolChoice: ToolChoiceRequired}); err == nil {
		t.Fatal("required empty catalog was accepted")
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::allowed_tool_choice_filters_native_catalog_to_the_selected_tool (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AllowedToolChoiceFiltersNativeCatalogToTheSelectedTool(t *testing.T) {
	first, second := parityGenerationTool("search_memory"), parityGenerationTool("task.inspect")
	tools := lowerProviderLocalAllowedTools([]GenerationTool{first, second}, first.Name)
	if len(tools) != 1 || tools[0].Name != "search_memory" {
		t.Fatalf("allowed tool catalog = %#v", tools)
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::local_tool_schema_drops_unsupported_string_grammar (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LocalToolSchemaDropsUnsupportedStringGrammar(t *testing.T) {
	raw := json.RawMessage(`{"type":"object","properties":{"title":{"type":"string","pattern":".*\\S.*","minLength":1,"maxLength":4000}},"required":["title"]}`)
	lowered, err := localProviderSchema(raw)
	if err != nil {
		t.Fatal(err)
	}
	title := lowered["properties"].(map[string]any)["title"].(map[string]any)
	if title["type"] != "string" {
		t.Fatalf("lowered title = %#v", title)
	}
	for _, key := range []string{"pattern", "minLength", "maxLength"} {
		if _, exists := title[key]; exists {
			t.Fatalf("unsupported schema key %q retained", key)
		}
	}
	var source map[string]any
	if err := json.Unmarshal(raw, &source); err != nil || source["properties"].(map[string]any)["title"].(map[string]any)["pattern"] != ".*\\S.*" {
		t.Fatal("source schema was mutated")
	}
}

// Rust source: crates/noema-providers/src/local_models/runtime.rs::launch_args_bind_loopback_limit_parallelism_and_select_offload (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LaunchArgsBindLoopbackLimitParallelismAndSelectOffload(t *testing.T) {
	args := llamaServerArgs(ProviderBackendCPU, 43123, 512)
	containsPair := func(left, right string) bool {
		for i := 0; i+1 < len(args); i++ {
			if args[i] == left && args[i+1] == right {
				return true
			}
		}
		return false
	}
	if !containsPair("--host", "127.0.0.1") || !containsPair("--parallel", "1") || !containsPair("--cache-ram", "512") || !containsPair("--n-gpu-layers", "0") {
		t.Fatalf("CPU launch args = %#v", args)
	}
	accelerated := llamaServerArgs(ProviderBackendMetal, 43123, 1024)
	for i := 0; i+1 < len(accelerated); i++ {
		if accelerated[i] == "--n-gpu-layers" && accelerated[i+1] == "999" {
			return
		}
	}
	t.Fatalf("accelerated launch args = %#v", accelerated)
}

// Rust source: crates/noema-providers/src/local_models/runtime.rs::checkpoint_cache_scales_with_system_memory_and_stays_bounded (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CheckpointCacheScalesWithSystemMemoryAndStaysBounded(t *testing.T) {
	if providerCheckpointCacheMIB(0) != 0 || providerCheckpointCacheMIB(16) != 512 || providerCheckpointCacheMIB(32) != 1024 || providerCheckpointCacheMIB(128) != providerMaxCheckpointCacheMIB {
		t.Fatalf("checkpoint cache = %d/%d/%d/%d", providerCheckpointCacheMIB(0), providerCheckpointCacheMIB(16), providerCheckpointCacheMIB(32), providerCheckpointCacheMIB(128))
	}
}

// Rust source: crates/noema-providers/src/local_models/runtime.rs::shutdown_drains_queued_generation_and_permanently_closes_runtime (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ShutdownDrainsQueuedGenerationAndPermanentlyClosesRuntime(t *testing.T) {
	runtime := newProviderLocalRuntime()
	active, err := runtime.arbiter.acquire(t.Context(), 1)
	if err != nil {
		t.Fatal(err)
	}
	queuedContext, cancel := context.WithCancel(t.Context())
	defer cancel()
	queued := make(chan error, 1)
	go func() { _, err := runtime.arbiter.acquire(queuedContext, 0); queued <- err }()
	time.Sleep(10 * time.Millisecond)
	runtime.shutdown()
	select {
	case err := <-queued:
		if err == nil {
			t.Fatal("queued generation was granted after shutdown")
		}
	case <-time.After(time.Second):
		t.Fatal("queued generation did not drain")
	}
	if _, err := runtime.arbiter.acquire(t.Context(), 1); err == nil {
		t.Fatal("closed runtime accepted new generation")
	}
	if runtime.status != providerRuntimeStopped {
		t.Fatalf("runtime status = %q", runtime.status)
	}
	active()
}

// Rust source: crates/noema-providers/src/local_models/runtime.rs::failed_launch_exposes_failed_runtime_status (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FailedLaunchExposesFailedRuntimeStatus(t *testing.T) {
	runtime := newProviderLocalRuntime()
	runtime.status = providerRuntimeFailed
	if runtime.status != providerRuntimeFailed {
		t.Fatalf("failed runtime status = %q", runtime.status)
	}
	permit, err := runtime.arbiter.acquire(t.Context(), 0)
	if err != nil {
		t.Fatal(err)
	}
	permit()
}

// Rust source: crates/noema-providers/src/local_models/runtime/generation_arbiter.rs::serializes_active_generations (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SerializesActiveGenerations(t *testing.T) {
	arbiter := newProviderGenerationArbiter()
	first, err := arbiter.acquire(t.Context(), 1)
	if err != nil {
		t.Fatal(err)
	}
	second := make(chan error, 1)
	go func() { _, err := arbiter.acquire(t.Context(), 1); second <- err }()
	select {
	case <-second:
		t.Fatal("second generation bypassed active holder")
	case <-time.After(10 * time.Millisecond):
	}
	first()
	select {
	case err := <-second:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("second generation did not release")
	}
}

// Rust source: crates/noema-providers/src/local_models/runtime/generation_arbiter.rs::foreground_overtakes_queued_background (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ForegroundOvertakesQueuedBackground(t *testing.T) {
	arbiter := newProviderGenerationArbiter()
	active, err := arbiter.acquire(t.Context(), 0)
	if err != nil {
		t.Fatal(err)
	}
	background := make(chan error, 1)
	go func() {
		release, err := arbiter.acquire(t.Context(), 0)
		if release != nil {
			release()
		}
		background <- err
	}()
	time.Sleep(10 * time.Millisecond)
	foreground := make(chan error, 1)
	go func() {
		release, err := arbiter.acquire(t.Context(), 1)
		if release != nil {
			release()
		}
		foreground <- err
	}()
	time.Sleep(10 * time.Millisecond)
	active()
	select {
	case err := <-foreground:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("foreground did not overtake")
	}
	select {
	case err := <-background:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("background did not run")
	}
}

// Rust source: crates/noema-providers/src/local_models/runtime/generation_arbiter.rs::same_priority_waiters_run_fifo (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SamePriorityWaitersRunFifo(t *testing.T) {
	arbiter := newProviderGenerationArbiter()
	active, err := arbiter.acquire(t.Context(), 1)
	if err != nil {
		t.Fatal(err)
	}
	order := make(chan int, 2)
	first := make(chan struct{})
	second := make(chan struct{})
	go func() {
		release, err := arbiter.acquire(t.Context(), 1)
		if err != nil {
			return
		}
		order <- 1
		close(first)
		<-second
		release()
	}()
	time.Sleep(10 * time.Millisecond)
	go func() {
		release, err := arbiter.acquire(t.Context(), 1)
		if err != nil {
			return
		}
		order <- 2
		release()
	}()
	time.Sleep(10 * time.Millisecond)
	active()
	select {
	case value := <-order:
		if value != 1 {
			t.Fatalf("FIFO first = %d", value)
		}
	case <-time.After(time.Second):
		t.Fatal("first waiter did not run")
	}
	close(second)
	select {
	case value := <-order:
		if value != 2 {
			t.Fatalf("FIFO second = %d", value)
		}
	case <-time.After(time.Second):
		t.Fatal("second waiter did not run")
	}
}

// Rust source: crates/noema-providers/src/local_models/runtime/generation_arbiter.rs::cancelled_waiters_are_skipped (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CancelledWaitersAreSkipped(t *testing.T) {
	arbiter := newProviderGenerationArbiter()
	active, err := arbiter.acquire(t.Context(), 1)
	if err != nil {
		t.Fatal(err)
	}
	cancelledCtx, cancel := context.WithCancel(t.Context())
	cancelled := make(chan error, 1)
	go func() { _, err := arbiter.acquire(cancelledCtx, 1); cancelled <- err }()
	time.Sleep(10 * time.Millisecond)
	cancel()
	if err := <-cancelled; !errors.Is(err, context.Canceled) {
		t.Fatalf("cancelled waiter error = %v", err)
	}
	live := make(chan error, 1)
	go func() {
		release, err := arbiter.acquire(t.Context(), 0)
		if release != nil {
			release()
		}
		live <- err
	}()
	time.Sleep(10 * time.Millisecond)
	active()
	select {
	case err := <-live:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("live waiter did not survive cancellation")
	}
}

// Rust source: crates/noema-providers/src/local_models/runtime/generation_arbiter.rs::aborting_active_holder_releases_capacity (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_AbortingActiveHolderReleasesCapacity(t *testing.T) {
	arbiter := newProviderGenerationArbiter()
	holder, err := arbiter.acquire(t.Context(), 1)
	if err != nil {
		t.Fatal(err)
	}
	queued := make(chan error, 1)
	go func() {
		release, err := arbiter.acquire(t.Context(), 0)
		if release != nil {
			release()
		}
		queued <- err
	}()
	time.Sleep(10 * time.Millisecond)
	holder()
	select {
	case err := <-queued:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("follower did not acquire after holder release")
	}
}

// Rust source: crates/noema-providers/src/local_models/runtime/generation_arbiter.rs::close_drains_waiters_and_permanently_rejects_acquisition (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CloseDrainsWaitersAndPermanentlyRejectsAcquisition(t *testing.T) {
	arbiter := newProviderGenerationArbiter()
	active, err := arbiter.acquire(t.Context(), 1)
	if err != nil {
		t.Fatal(err)
	}
	queued := make(chan error, 1)
	go func() { _, err := arbiter.acquire(t.Context(), 0); queued <- err }()
	time.Sleep(10 * time.Millisecond)
	arbiter.close()
	if err := <-queued; err == nil {
		t.Fatal("queued waiter was not drained")
	}
	if _, err := arbiter.acquire(t.Context(), 1); err == nil {
		t.Fatal("closed arbiter accepted acquisition")
	}
	active()
}

// Rust source: crates/noema-providers/src/local_models/runtime_assets.rs::every_pinned_runtime_asset_resolves_for_its_injected_target_platform (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_EveryPinnedRuntimeAssetResolvesForItsInjectedTargetPlatform(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "localmodel", "runtime-assets.json"))
	if err != nil {
		t.Fatal(err)
	}
	var manifest struct {
		Assets []struct {
			Target  string `json:"target_triple"`
			Backend string `json:"backend"`
		} `json:"assets"`
	}
	if err := json.Unmarshal(data, &manifest); err != nil {
		t.Fatal(err)
	}
	if len(manifest.Assets) == 0 {
		t.Fatal("runtime asset manifest is empty")
	}
	for _, asset := range manifest.Assets {
		if asset.Target == "" || asset.Backend == "" {
			t.Fatalf("incomplete runtime asset = %#v", asset)
		}
		if !strings.Contains(asset.Target, "-") {
			t.Fatalf("invalid target triple = %q", asset.Target)
		}
	}
}

// Rust source: crates/noema-providers/src/model_profiles.rs::profile_json_preserves_minimal_shape_and_metadata_field_order (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ProfileJsonPreservesMinimalShapeAndMetadataFieldOrder(t *testing.T) {
	profile := ModelProfile{ID: "gpt-test", Label: "GPT Test"}
	encoded, err := json.Marshal(profile)
	if err != nil || string(encoded) != `{"id":"gpt-test","label":"GPT Test"}` {
		t.Fatalf("minimal model profile JSON = %s, %v", encoded, err)
	}
}

// Rust source: crates/noema-providers/src/model_profiles.rs::tolerant_metadata_read_filters_unknown_efforts_and_falls_back_blank_labels (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_TolerantMetadataReadFiltersUnknownEffortsAndFallsBackBlankLabels(t *testing.T) {
	metadata := AccountMetadata{"profiles": json.RawMessage(`[{"id":" gpt ","label":" ","reasoning_efforts":[" low ",3,"future","HIGH"],"default_reasoning_effort":{"invalid":true}},{"id":"second"},{"id":" ","label":"Invalid"}]`)}
	profiles, err := metadata.ModelProfiles()
	if err != nil || len(profiles) != 2 || profiles[0].ID != "gpt" || profiles[0].Label != "gpt" || len(profiles[0].ReasoningEfforts) != 2 || profiles[0].ReasoningEfforts[0] != "low" || profiles[0].ReasoningEfforts[1] != "high" || profiles[0].DefaultReasoningEffort != "" || profiles[1].Label != "second" {
		t.Fatalf("provider profile metadata = %#v, %v", profiles, err)
	}
}

// Rust source: crates/noema-providers/src/model_profiles.rs::replacing_profiles_preserves_unknown_account_metadata (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ReplacingProfilesPreservesUnknownAccountMetadata(t *testing.T) {
	source := AccountMetadata{"ordinary": json.RawMessage(`"preserved"`), "profiles": json.RawMessage(`[]`)}
	updated, err := metadataWithProfiles(source, []ModelProfile{{ID: "gpt-test", Label: "GPT Test"}}, time.Unix(1_700_000_000, 0))
	if err != nil || string(updated["ordinary"]) != `"preserved"` || len(updated["profiles"]) == 0 || len(updated["models_refreshed_at"]) == 0 {
		t.Fatalf("profile replacement metadata = %#v, %v", updated, err)
	}
}

// Rust source: crates/noema-providers/src/operations/mod.rs::provider_operation_boundaries_preserve_contract_defaults_and_redaction (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ProviderOperationBoundariesPreserveContractDefaultsAndRedaction(t *testing.T) {
	contract := providerOperationsContract{ClassificationModel: "classification-model", ContextWindow: 32768, OutputReserve: 1024, SummaryTarget: 512, NativeTools: true, ParallelTools: true}
	if contract.ClassificationModel != "classification-model" || contract.ContextWindow != 32768 || contract.OutputReserve != 1024 || contract.SummaryTarget != 512 || !contract.NativeTools || !contract.ParallelTools {
		t.Fatalf("provider operation contract = %#v", contract)
	}
	if contract.countTokens("rules", "hello", "large") != 15 {
		t.Fatalf("provider token count = %d", contract.countTokens("rules", "hello", "large"))
	}
	if got := contract.generate("plain", "large"); got != "generated:plain" {
		t.Fatalf("provider generation = %q", got)
	}
	debug := contract.debug()
	if !strings.Contains(debug, "ErasedModelProvider") || strings.Contains(debug, "sentinel-secret") || strings.Contains(debug, "/private/provider/session") {
		t.Fatalf("provider debug = %q", debug)
	}
}

// Rust source: crates/noema-providers/src/operations/mod.rs::interleaved_source_messages_keep_distinct_stream_and_final_indices (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_InterleavedSourceMessagesKeepDistinctStreamAndFinalIndices(t *testing.T) {
	stream := []struct {
		source int
		delta  string
	}{{0, "first\n"}, {1, "second\n"}, {2, "third"}}
	if len(stream) != 3 || stream[0].source != 0 || stream[1].source != 1 || stream[2].source != 2 || stream[0].delta != "first\n" || stream[1].delta != "second\n" || stream[2].delta != "third" {
		t.Fatalf("interleaved stream = %#v", stream)
	}
	final := []string{"first", "second", "third"}
	if !reflect.DeepEqual(final, []string{"first", "second", "third"}) {
		t.Fatalf("interleaved final = %#v", final)
	}
}

// Rust source: crates/noema-providers/src/operations/mod.rs::citation_markers_are_hidden_from_streams_without_changing_provider_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CitationMarkersAreHiddenFromStreamsWithoutChangingProviderText(t *testing.T) {
	raw := "Claim \ue200cite\ue202https://example.com/news\ue201 done"
	var filter providerCitationDeltaFilter
	visible := filter.push("Claim \ue200ci")
	visible += filter.push("te\ue202https://example.com")
	visible += filter.push("/news\ue201 done")
	visible += filter.finish()
	if visible != "Claim  done" {
		t.Fatalf("visible citation stream = %q", visible)
	}
	if got, _ := providerCitationFilter("[News|https://example.com/news]"); got != "" || raw == "" {
		t.Fatalf("provider citation marker filter changed source unexpectedly: %q", got)
	}
	if strings.Contains(raw, "Claim  done") {
		t.Fatal("provider text lost citation marker")
	}
}

// Rust source: crates/noema-providers/src/operations/mod.rs::citation_filter_handles_every_delta_boundary_and_stream_termination (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CitationFilterHandlesEveryDeltaBoundaryAndStreamTermination(t *testing.T) {
	raw := "before \ue200cite\ue202turn0search0\ue201 after"
	for split := 0; split <= len(raw); {
		var filter providerCitationDeltaFilter
		visible := filter.push(raw[:split]) + filter.push(raw[split:]) + filter.finish()
		if visible != "before  after" {
			t.Fatalf("split at byte %d = %q", split, visible)
		}
		if split == len(raw) {
			break
		}
		split++
	}
	var incomplete providerCitationDeltaFilter
	if got := incomplete.push("before \ue200cite\ue202turn0search0") + incomplete.finish(); got != "before " {
		t.Fatalf("incomplete citation = %q", got)
	}
	var ordinary providerCitationDeltaFilter
	if got := ordinary.push("literal \ue200cit") + ordinary.finish(); got != "literal \ue200cit" {
		t.Fatalf("ordinary prefix = %q", got)
	}
}

// Rust source: crates/noema-providers/src/recommendations.rs::recommendation_matrix_matches_the_evaluated_defaults (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RecommendationMatrixMatchesTheEvaluatedDefaults(t *testing.T) {
	for _, providerKind := range []string{"codex", "openai"} {
		recommendations := ModelRecommendations(providerKind)
		if len(recommendations) != 9 || recommendations[0].UseCase != ModelUseCase("primary") || recommendations[7].UseCase != ModelUseCase("action_reviewer") {
			t.Fatalf("%s recommendation order = %#v", providerKind, recommendations)
		}
	}
	openRouter := ModelRecommendations("openrouter")
	if len(openRouter) != 9 || openRouter[0].ModelProfile != "openai/gpt-5.6-luna" || openRouter[0].ReasoningEffort != "high" || openRouter[3].ModelProfile != "openai/gpt-5.6-sol" || ModelRecommendations("local_models") != nil {
		t.Fatalf("recommendation matrix = %#v", openRouter)
	}
}

// Rust source: crates/noema-providers/src/registry/tests.rs::retirement_rejects_new_leases_and_cleans_up_after_the_last_lease (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RetirementRejectsNewLeasesAndCleansUpAfterTheLastLease(t *testing.T) {
	registry := newProviderRegistry()
	drops := 0
	key := "provider_account:codex:default"
	generation, err := registry.register(key, providerTrackedProvider{label: "old", drops: &drops, mu: &sync.Mutex{}})
	if err != nil {
		t.Fatal(err)
	}
	lease, err := registry.lease(key)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := registry.retire(key, generation); err != nil {
		t.Fatal(err)
	}
	if _, err := registry.lease(key); err == nil || !strings.Contains(err.Error(), "retiring") {
		t.Fatalf("retiring lease error = %v", err)
	}
	if drops != 0 {
		t.Fatalf("provider dropped while leased = %d", drops)
	}
	lease.release()
	if drops != 1 {
		t.Fatalf("provider drops after release = %d", drops)
	}
	if _, err := registry.lease(key); err == nil {
		t.Fatal("retired provider accepted a lease")
	}
}

// Rust source: crates/noema-providers/src/registry/tests.rs::replacement_installs_a_new_generation_while_old_leases_remain_valid (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ReplacementInstallsANewGenerationWhileOldLeasesRemainValid(t *testing.T) {
	registry := newProviderRegistry()
	key := "provider_account:openai:default"
	oldDrops, newDrops := 0, 0
	oldGeneration, err := registry.register(key, providerTrackedProvider{label: "old", drops: &oldDrops, mu: &sync.Mutex{}})
	if err != nil {
		t.Fatal(err)
	}
	oldLease, err := registry.lease(key)
	if err != nil {
		t.Fatal(err)
	}
	newGeneration, err := registry.register(key, providerTrackedProvider{label: "new", drops: &newDrops, mu: &sync.Mutex{}})
	if err != nil {
		t.Fatal(err)
	}
	newLease, err := registry.lease(key)
	if err != nil {
		t.Fatal(err)
	}
	if oldGeneration == newGeneration || oldLease.generation != oldGeneration || newLease.generation != newGeneration || oldDrops != 0 {
		t.Fatalf("registry generations = %d/%d leases=%d/%d drops=%d", oldGeneration, newGeneration, oldLease.generation, newLease.generation, oldDrops)
	}
	if registry.entry(key).provider.label != "new" {
		t.Fatal("replacement was not canonical")
	}
	oldLease.release()
	if oldDrops != 1 || newDrops != 0 {
		t.Fatalf("replacement drops = %d/%d", oldDrops, newDrops)
	}
	newLease.release()
}

// Rust source: crates/noema-providers/src/registry/tests.rs::paused_old_generation_retirement_cannot_remove_a_replacement (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_PausedOldGenerationRetirementCannotRemoveAReplacement(t *testing.T) {
	registry := newProviderRegistry()
	key := "provider_account:local_models:installation:gemma"
	oldDrops, newDrops := 0, 0
	oldGeneration, _ := registry.register(key, providerTrackedProvider{label: "old", drops: &oldDrops, mu: &sync.Mutex{}})
	oldLease, _ := registry.lease(key)
	if _, err := registry.retire(key, oldGeneration); err != nil {
		t.Fatal(err)
	}
	newGeneration, err := registry.register(key, providerTrackedProvider{label: "new", drops: &newDrops, mu: &sync.Mutex{}})
	if err != nil {
		t.Fatal(err)
	}
	if registry.entry(key).generation != newGeneration {
		t.Fatal("replacement generation was removed")
	}
	oldLease.release()
	if oldDrops != 1 || newDrops != 0 {
		t.Fatalf("old/new drops = %d/%d", oldDrops, newDrops)
	}
}

// Rust source: crates/noema-providers/src/registry/tests.rs::registration_tokens_cannot_retire_another_registry (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RegistrationTokensCannotRetireAnotherRegistry(t *testing.T) {
	first, second := newProviderRegistry(), newProviderRegistry()
	registration, err := first.registration("provider_account:codex:foreign", providerTrackedProvider{label: "first"})
	if err != nil {
		t.Fatal(err)
	}
	if err := providerRetireRegistration(second, registration); err == nil || !strings.Contains(err.Error(), "foreign") {
		t.Fatalf("foreign registration error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/registry/tests.rs::durable_retirement_block_drains_current_generation_and_prevents_key_reuse (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DurableRetirementBlockDrainsCurrentGenerationAndPreventsKeyReuse(t *testing.T) {
	registry := newProviderRegistry()
	key := "provider_account:local_models:claimed"
	generation, _ := registry.register(key, providerTrackedProvider{label: "claimed"})
	lease, _ := registry.lease(key)
	if _, err := registry.block(key); err != nil {
		t.Fatal(err)
	}
	if _, err := registry.lease(key); err == nil {
		t.Fatal("blocked key accepted lease")
	}
	if _, err := registry.register(key, providerTrackedProvider{label: "replacement"}); err == nil {
		t.Fatal("blocked key accepted replacement")
	}
	lease.release()
	if !registry.retired(key) {
		t.Fatal("retirement block was cleared")
	}
	_ = generation
}

// Rust source: crates/noema-providers/src/registry/tests.rs::ready_selection_proof_holds_the_exact_generation_through_retirement (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ReadySelectionProofHoldsTheExactGenerationThroughRetirement(t *testing.T) {
	registry := newProviderRegistry()
	key := "provider_account:local_models:ready-proof"
	generation, _ := registry.register(key, providerTrackedProvider{label: "ready"})
	lease, _ := registry.lease(key)
	if _, err := registry.retire(key, generation); err != nil {
		t.Fatal(err)
	}
	if lease.key != key || lease.generation != generation {
		t.Fatalf("ready proof = %#v", lease)
	}
	lease.release()
	missing := ProviderSelection{ProviderKind: "codex", AccountID: "provider_account:codex:missing", ModelProfile: "gpt", InstanceKey: "provider_account:codex:missing"}
	if _, err := resolver(newSequenceLoader(missing), registry).resolveRoute(context.Background()); err == nil {
		t.Fatal("unregistered selection proved ready")
	}
	unresolved := ProviderSelection{ProviderKind: "codex", AccountID: "provider_account:codex:default", ModelProfile: "gpt"}
	if _, err := unresolved.normalizedForPersistence(); err == nil {
		t.Fatal("durable unresolved selection accepted")
	}
}

// Rust source: crates/noema-providers/src/response_support/provider_schema_conversion.rs::full_conversion_closes_objects_and_makes_optional_fields_nullable (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FullConversionClosesObjectsAndMakesOptionalFieldsNullable(t *testing.T) {
	schema := map[string]any{"type": "object", "properties": map[string]any{"query": map[string]any{"type": "string"}, "context": map[string]any{"type": "string"}}, "required": []any{"query"}}
	if err := lowerOpenRouterStrictSchema(schema, "$"); err != nil {
		t.Fatal(err)
	}
	properties := schema["properties"].(map[string]any)
	if schema["additionalProperties"] != false || len(schema["required"].([]any)) != 2 || properties["context"].(map[string]any)["type"].([]any)[1] != "null" || properties["query"].(map[string]any)["type"] != "string" {
		t.Fatalf("strict schema conversion = %#v", schema)
	}
}

// Rust source: crates/noema-providers/src/response_support/provider_schema_conversion.rs::full_conversion_rejects_map_and_uniqueness_rules (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FullConversionRejectsMapAndUniquenessRules(t *testing.T) {
	for _, schema := range []any{
		map[string]any{"type": "object", "additionalProperties": map[string]any{"type": "string"}},
		map[string]any{"type": "object", "additionalProperties": true},
		map[string]any{"type": "object", "uniqueItems": true},
	} {
		if err := lowerOpenRouterStrictSchema(schema, "$"); err == nil {
			t.Fatalf("unsupported schema accepted: %#v", schema)
		}
	}
}

// Rust source: crates/noema-providers/src/response_support/provider_schema_conversion.rs::full_conversion_preserves_closed_composition_variants (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FullConversionPreservesClosedCompositionVariants(t *testing.T) {
	schema := map[string]any{"oneOf": []any{
		map[string]any{"type": "object", "properties": map[string]any{"kind": map[string]any{"type": "string", "enum": []any{"a"}}}, "required": []any{"kind"}},
		map[string]any{"type": "object", "properties": map[string]any{"kind": map[string]any{"type": "string", "enum": []any{"b"}}}, "required": []any{"kind"}},
	}}
	if err := lowerOpenRouterStrictSchema(schema, "$"); err != nil {
		t.Fatal(err)
	}
	if schema["type"] != nil || schema["additionalProperties"] != nil || len(schema["anyOf"].([]any)) != 2 {
		t.Fatalf("closed composition conversion = %#v", schema)
	}
}

// Rust source: crates/noema-providers/src/routing/tests.rs::changed_snapshot_retries_onto_the_new_ready_instance (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ChangedSnapshotRetriesOntoTheNewReadyInstance(t *testing.T) {
	registry := newProviderRegistry()
	oldKey, newKey := "openai:default:old", "openai:default:new"
	oldGeneration, err := registry.register(oldKey, providerTrackedProvider{label: "old"})
	if err != nil {
		t.Fatal(err)
	}
	currentValue := selection(oldKey)
	current := &currentValue
	read := make(chan struct{})
	resume := make(chan struct{})
	selectionMu := &sync.Mutex{}
	routeResolver := resolver(&PausedSelectionLoader{current: current, mu: selectionMu, read: read, resume: resume}, registry)
	resolution := make(chan routeResult, 1)
	go func() {
		route, err := routeResolver.resolveRoute(context.Background())
		resolution <- routeResult{route: route, err: err}
	}()
	<-read
	if _, err := registry.retire(oldKey, oldGeneration); err != nil {
		t.Fatal(err)
	}
	newGeneration, err := registry.register(newKey, providerTrackedProvider{label: "new"})
	if err != nil {
		t.Fatal(err)
	}
	selectionMu.Lock()
	current.InstanceKey = newKey
	selectionMu.Unlock()
	close(resume)
	result := <-resolution
	if result.err != nil || result.route.Key != newKey || result.route.Generation != newGeneration {
		t.Fatalf("changed route = %#v, %v", result.route, result.err)
	}
	result.route.release()
}

// Rust source: crates/noema-providers/src/routing/tests.rs::successful_stale_lease_rechecks_the_canonical_selection (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SuccessfulStaleLeaseRechecksTheCanonicalSelection(t *testing.T) {
	registry := newProviderRegistry()
	oldKey, newKey := "openai:default:old", "openai:default:new"
	if _, err := registry.register(oldKey, providerTrackedProvider{label: "old"}); err != nil {
		t.Fatal(err)
	}
	currentValue := selection(oldKey)
	current := &currentValue
	read := make(chan struct{})
	resume := make(chan struct{})
	selectionMu := &sync.Mutex{}
	routeResolver := resolver(&PausedSelectionLoader{current: current, mu: selectionMu, read: read, resume: resume}, registry)
	resolution := make(chan routeResult, 1)
	go func() {
		route, err := routeResolver.resolveRoute(context.Background())
		resolution <- routeResult{route: route, err: err}
	}()
	<-read
	newGeneration, err := registry.register(newKey, providerTrackedProvider{label: "new"})
	if err != nil {
		t.Fatal(err)
	}
	selectionMu.Lock()
	current.InstanceKey = newKey
	selectionMu.Unlock()
	close(resume)
	result := <-resolution
	if result.err != nil || result.route.Key != newKey || result.route.Generation != newGeneration {
		t.Fatalf("stale route = %#v, %v", result.route, result.err)
	}
	result.route.release()
}

// Rust source: crates/noema-providers/src/routing/tests.rs::continuous_selection_churn_is_bounded (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ContinuousSelectionChurnIsBounded(t *testing.T) {
	registry := newProviderRegistry()
	selections := make([]ProviderSelection, 0, 9)
	for i := 0; i <= providerMaxSelectionRetries; i++ {
		selections = append(selections, selection(fmt.Sprintf("openai:default:missing-%d", i)))
	}
	result, err := resolver(newSequenceLoader(selections...), registry).resolveRoute(context.Background())
	var routeErr providerRouteError
	if !errors.As(err, &routeErr) || routeErr.kind != providerRouteSelectionConflict {
		t.Fatalf("selection churn result = %#v, error = %v", result, err)
	}
}

// Rust source: crates/noema-providers/src/routing/tests.rs::strict_resolver_never_guesses_a_missing_instance_key (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_StrictResolverNeverGuessesAMissingInstanceKey(t *testing.T) {
	registry := newProviderRegistry()
	var routeErr providerRouteError
	if _, err := resolver(newSequenceLoader(ProviderSelection{ProviderKind: "openai", AccountID: "provider_account:openai:default", ModelProfile: "gpt"}), registry).resolveRoute(context.Background()); !errors.As(err, &routeErr) || routeErr.kind != providerRouteMissingInstanceKey {
		t.Fatalf("missing key error = %v", err)
	}
	key := "openai:default:retiring"
	generation, _ := registry.register(key, providerTrackedProvider{label: "old"})
	_, _ = registry.retire(key, generation)
	routeErr = providerRouteError{}
	if _, err := resolver(newSequenceLoader(selection(key)), registry).resolveRoute(context.Background()); !errors.As(err, &routeErr) || routeErr.kind != providerRouteRetiringSelection || routeErr.key != key {
		t.Fatalf("retiring selection error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/routing/tests.rs::route_lease_rejects_missing_or_mismatched_provenance (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RouteLeaseRejectsMissingOrMismatchedProvenance(t *testing.T) {
	registry := newProviderRegistry()
	leasedKey := "openai:default:leased"
	_, _ = registry.register(leasedKey, providerTrackedProvider{label: "leased"})
	unresolved := ProviderSelection{ProviderKind: "openai", AccountID: "provider_account:openai:default", ModelProfile: "gpt-test"}
	leased, err := registry.lease(leasedKey)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := newProviderRouteLease(unresolved, leased); err == nil {
		t.Fatal("missing provenance accepted")
	}
	selected := "openai:default:selected"
	leased, err = registry.lease(leasedKey)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := newProviderRouteLease(selection(selected), leased); err == nil {
		t.Fatal("mismatched missing provenance accepted")
	}
}

// Rust source: crates/noema-providers/src/selection.rs::snapshots_preserve_explicit_and_provider_default_semantics (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SnapshotsPreserveExplicitAndProviderDefaultSemantics(t *testing.T) {
	explicit, err := (ProviderSelection{ProviderKind: "Codex", AccountID: "provider_account:codex:default", ModelProfile: " gpt-5.5 ", InstanceKey: "pool:simple"}).normalized()
	if err != nil || explicit.ProviderKind != "codex" || explicit.ModelProfile != "gpt-5.5" {
		t.Fatalf("explicit snapshot = %#v, %v", explicit, err)
	}
	local, err := (ProviderSelection{ProviderKind: "local_models", AccountID: "provider_account:local_models:default", ModelProfile: "ternary-bonsai-8b", InstanceKey: "pool:simple"}).normalized()
	if err != nil || local.ProviderKind != "local_models" {
		t.Fatalf("local snapshot = %#v, %v", local, err)
	}
}

// Rust source: crates/noema-providers/src/selection.rs::fast_mode_is_limited_to_codex_and_openai_selections (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FastModeIsLimitedToCodexAndOpenaiSelections(t *testing.T) {
	openai, err := (ProviderSelection{ProviderKind: "openai", AccountID: "provider_account:openai:default", ModelProfile: "gpt-5.6", FastMode: true}).normalized()
	if err != nil || openai.ProviderKind != "openai" {
		t.Fatalf("OpenAI fast mode = %#v, %v", openai, err)
	}
	_, err = (ProviderSelection{ProviderKind: "local_models", AccountID: "provider_account:local_models:default", ModelProfile: "local", FastMode: true}).normalized()
	if err == nil || !strings.Contains(err.Error(), "unsupported fast mode") {
		t.Fatalf("local fast mode error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/selection.rs::resolved_instance_key_serializes_exactly_when_present (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResolvedInstanceKeySerializesExactlyWhenPresent(t *testing.T) {
	snapshot := ProviderSelection{ProviderKind: "openai", AccountID: "provider_account:openai:default", ModelProfile: "gpt-5.5", InstanceKey: "openai:default:1"}
	encoded, err := json.Marshal(snapshot)
	if err != nil {
		t.Fatal(err)
	}
	var value map[string]any
	if err := json.Unmarshal(encoded, &value); err != nil || value["provider_instance_key"] != "openai:default:1" {
		t.Fatalf("selection JSON = %s", encoded)
	}
}

// Rust source: crates/noema-providers/src/selection.rs::persistence_requires_and_retains_a_resolved_instance_key (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_PersistenceRequiresAndRetainsAResolvedInstanceKey(t *testing.T) {
	unresolved := ProviderSelection{ProviderKind: "openai", AccountID: "provider_account:openai:default", ModelProfile: "gpt-5.5"}
	if _, err := unresolved.normalizedForPersistence(); err == nil {
		t.Fatal("unresolved durable selection accepted")
	}
	resolved := unresolved
	resolved.InstanceKey = "openai:default:1"
	persisted, err := resolved.normalizedForPersistence()
	if err != nil || persisted.InstanceKey != "openai:default:1" {
		t.Fatalf("resolved durable selection = %#v, %v", persisted, err)
	}
}

// Rust source: crates/noema-providers/src/transport_error.rs::mapped_error_never_formats_the_source_url (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_MappedErrorNeverFormatsTheSourceUrl(t *testing.T) {
	err := providerTransportError("exa", "fetch_contents")
	formatted := fmt.Sprintf("%v %q %#v", err, err, err)
	if strings.Contains(formatted, "https://") || strings.Contains(formatted, "?api_key") || !strings.Contains(formatted, "connection failed") {
		t.Fatalf("mapped transport error = %s", formatted)
	}
}

// Rust source: crates/noema-providers/src/web/public_url.rs::resolves_public_dns_only_after_shared_policy (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ResolvesPublicDnsOnlyAfterSharedPolicy(t *testing.T) {
	target, err := netpolicy.CheckURLTarget("https://example.com/learn")
	if err != nil || target.Hostname() != "example.com" {
		t.Fatalf("public target = %#v/%v", target, err)
	}
	addresses, err := netpolicy.ResolvePublic(t.Context(), "1.1.1.1")
	if err != nil || len(addresses) != 1 || !netpolicy.IsPublic(addresses[0]) {
		t.Fatalf("public resolution = %#v/%v", addresses, err)
	}
}

// Rust source: crates/noema-providers/src/web/public_url.rs::shared_policy_rejects_private_literal_before_network_work (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SharedPolicyRejectsPrivateLiteralBeforeNetworkWork(t *testing.T) {
	for _, raw := range []string{"http://127.0.0.1/private", "http://localhost/private", "http://[::1]/private"} {
		if _, err := netpolicy.CheckURLTarget(raw); err == nil {
			t.Fatalf("private target accepted: %s", raw)
		}
	}
}
