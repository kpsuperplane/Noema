package provider

import (
	"bytes"
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
)

// TestRustProviderSecretContracts ports the Rust credential validation and
// diagnostic assertions to the Go secret wrappers used by each provider.
func TestRustProviderSecretContracts(t *testing.T) {
	for _, value := range []string{"", " ", "\n\t", " \n "} {
		if _, err := NewSecret(value); err == nil {
			t.Fatalf("blank provider secret %q was accepted", value)
		}
	}

	secret, err := NewSecret("credential-secret")
	if err != nil {
		t.Fatal(err)
	}
	formatted := fmt.Sprintf("%v %s %q %+v %#v", secret, secret, secret, secret, secret)
	if strings.Contains(formatted, "credential-secret") || !strings.Contains(formatted, "[REDACTED]") {
		t.Fatalf("secret formatting = %q", formatted)
	}
	if err := secret.Use(func(value string) error {
		if value != "credential-secret" {
			t.Fatalf("authorized secret value = %q", value)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	if err := secret.Use(nil); err == nil {
		t.Fatal("nil secret binding was accepted")
	}

	tokens := CodexTokens{accessToken: "access-secret", refreshToken: "refresh-secret", lastRefresh: 7}
	tokenFormatting := fmt.Sprintf("%v %s %q %+v %#v", tokens, tokens, tokens, tokens, tokens)
	if strings.Contains(tokenFormatting, "access-secret") || strings.Contains(tokenFormatting, "refresh-secret") ||
		!strings.Contains(tokenFormatting, "[REDACTED]") {
		t.Fatalf("Codex token formatting = %q", tokenFormatting)
	}
	if err := tokens.Use(func(access, refresh string, refreshed uint64) error {
		if access != "access-secret" || refresh != "refresh-secret" || refreshed != 7 {
			t.Fatalf("authorized Codex tokens = %q, %q, %d", access, refresh, refreshed)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
}

// TestRustAuthAttemptDiagnostics ports the Rust safe-view assertion while
// checking that ordinary account and authentication fields remain visible.
func TestRustAuthAttemptDiagnostics(t *testing.T) {
	attempt := AuthAttempt{
		ID:                "attempt-1",
		ProviderKind:      "codex",
		ProviderAccountID: "provider_account:codex:default",
		Method:            AuthOAuthDeviceCode,
		Status:            AuthAttemptWaiting,
		VerificationURL:   "https://example.test/verify",
		UserCode:          "SECRET-CODE",
	}
	formatted := fmt.Sprintf("%v %s %q %+v %#v", attempt, attempt, attempt, attempt, attempt)
	if strings.Contains(formatted, "SECRET-CODE") || !strings.Contains(formatted, "[REDACTED]") {
		t.Fatalf("auth attempt formatting = %q", formatted)
	}
	for _, ordinary := range []string{
		"attempt-1", "codex", "provider_account:codex:default", "oauth_device_code",
		"waiting_for_user", "https://example.test/verify",
	} {
		if !strings.Contains(formatted, ordinary) {
			t.Fatalf("auth attempt formatting omitted ordinary value %q: %s", ordinary, formatted)
		}
	}
}

// TestRustProviderFileSnapshots ports exact-byte restoration and redacted
// diagnostic assertions for failed credential mutations.
func TestRustProviderFileSnapshots(t *testing.T) {
	path := filepath.Join(t.TempDir(), "providers", "exa", "team", "api_key.json")
	original := []byte("{malformed prior secret}\x00\xff")
	if err := home.AtomicWritePrivate(path, original); err != nil {
		t.Fatal(err)
	}
	snapshot, err := snapshotPrivateFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !snapshot.exists || !bytes.Equal(snapshot.data, original) {
		t.Fatalf("snapshot = %#v", snapshot)
	}
	if formatted := fmt.Sprintf("%v %+v %#v", snapshot, snapshot, snapshot); strings.Contains(formatted, "malformed prior secret") ||
		!strings.Contains(formatted, "exists:true") || !strings.Contains(formatted, "[REDACTED]") {
		t.Fatalf("snapshot formatting = %q", formatted)
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

	missingPath := filepath.Join(t.TempDir(), "providers", "exa", "team", "api_key.json")
	missing, err := snapshotPrivateFile(missingPath)
	if err != nil {
		t.Fatal(err)
	}
	if missing.exists {
		t.Fatal("missing file snapshot was marked present")
	}
	if err := home.AtomicWritePrivate(missingPath, []byte("replacement")); err != nil {
		t.Fatal(err)
	}
	if err := restorePrivateFile(missingPath, missing); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(missingPath); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("restored missing path error = %v", err)
	}
}

// TestRustCodexOAuthContracts ports the Rust token-store, interval, refresh,
// and workspace assertions to the integrated Go Codex service.
func TestRustCodexOAuthContracts(t *testing.T) {
	for _, raw := range []string{`"5"`, `5`} {
		value, present, err := parseCodexInterval([]byte(raw))
		if err != nil || !present || value != 5 {
			t.Fatalf("Codex interval %s = %d, %t, %v", raw, value, present, err)
		}
	}
	for _, raw := range []string{`"not-a-number"`, `-1`, `1.5`} {
		if _, _, err := parseCodexInterval([]byte(raw)); err == nil {
			t.Fatalf("invalid Codex interval %s was accepted", raw)
		}
	}

	now := time.Unix(1_700_000_000, 0).UTC()
	header := base64.RawURLEncoding.EncodeToString([]byte(`{"alg":"none"}`))
	claims := base64.RawURLEncoding.EncodeToString([]byte(`{"exp":` + strconv.FormatInt(now.Unix()-1, 10) + `}`))
	if !codexTokenNeedsRefresh(""+header+"."+claims+".signature", now) {
		t.Fatal("expired JWT was not refreshed")
	}
	if codexTokenNeedsRefresh("opaque-token", now) {
		t.Fatal("opaque Codex token required JWT refresh")
	}

	workspaceClaims := base64.RawURLEncoding.EncodeToString([]byte(`{"https://api.openai.com/auth":{"chatgpt_account_id":"workspace-test"}}`))
	workspace, err := codexChatGPTAccountID("header." + workspaceClaims + ".signature")
	if err != nil || workspace != "workspace-test" {
		t.Fatalf("Codex workspace = %q, %v", workspace, err)
	}

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

	_, accounts, tokenPath := codexTestService(t, "https://codex.invalid", time.Millisecond, time.Second)
	if err := os.MkdirAll(filepath.Dir(tokenPath), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(filepath.Dir(tokenPath), "auth.json"), []byte(`{"tokens":{"access_token":"local","refresh_token":"local-refresh"}}`), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := accounts.LoadCodexTokens(context.Background()); err == nil {
		t.Fatal("CLI auth.json was treated as a Noema token store")
	}
}

func TestRustCodexRefreshDoesNotRecreateDeletedAccount(t *testing.T) {
	_, accounts, tokenPath := codexTestService(t, "https://codex.invalid", time.Millisecond, time.Second)
	now := time.Now().UTC()
	if _, err := accounts.publishCodexTokens(context.Background(), 0, CodexTokens{
		accessToken: "old-access", refreshToken: "old-refresh", lastRefresh: 1,
	}, codexModelCatalog{
		Profiles:      []ModelProfile{{ID: "gpt-test", Label: "GPT Test"}},
		ClientVersion: "0.144.1",
	}, now); err != nil {
		t.Fatal(err)
	}
	stored, err := accounts.LoadCodexTokens(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	var digest codexAccessTokenDigest
	if err := stored.Use(func(access string, _ string, _ uint64) error {
		digest = codexAccessTokenDigestFor(access)
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	persistence := accounts.persistence.(*memoryAccountPersistence)
	_, err = accounts.RefreshCodexTokens(context.Background(), &digest, now, func(context.Context, CodexTokens) (CodexTokens, error) {
		persistence.mu.Lock()
		delete(persistence.accounts, codexGenerationAccountID)
		persistence.mu.Unlock()
		if err := os.RemoveAll(filepath.Dir(tokenPath)); err != nil {
			return CodexTokens{}, err
		}
		return CodexTokens{accessToken: "new-access", refreshToken: "new-refresh"}, nil
	})
	if !errors.Is(err, ErrAccountNotFound) {
		t.Fatalf("deleted-account refresh error = %v", err)
	}
	if _, statErr := os.Stat(tokenPath); !errors.Is(statErr, os.ErrNotExist) {
		t.Fatalf("deleted-account token path = %v", statErr)
	}
	if _, loadErr := accounts.LoadAccount(context.Background(), codexGenerationAccountID); !errors.Is(loadErr, ErrAccountNotFound) {
		t.Fatalf("deleted-account metadata = %v", loadErr)
	}
}
