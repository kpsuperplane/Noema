package provider

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"
)

func TestCodexDeviceAuthPreservesWirePollingAndProtectedTokens(t *testing.T) {
	if delay := (codexDeviceCode{Interval: 0, HasInterval: true}).pollInterval(codexPollFloor); delay != 3*time.Second {
		t.Fatalf("production polling floor = %s", delay)
	}
	var mu sync.Mutex
	var requests []codexRecordedRequest
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, _ := io.ReadAll(r.Body)
		mu.Lock()
		requests = append(requests, codexRecordedRequest{
			path: r.URL.Path, contentType: r.Header.Get("Content-Type"), body: string(body), at: time.Now(),
		})
		requestNumber := len(requests)
		mu.Unlock()
		w.Header().Set("Content-Type", "application/json")
		switch requestNumber {
		case 1:
			_, _ = w.Write([]byte(`{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":"0"}`))
		case 2:
			http.Error(w, `{"error":"authorization_pending"}`, http.StatusForbidden)
		case 3:
			http.Error(w, `{"error":"authorization_pending"}`, http.StatusNotFound)
		case 4:
			_, _ = w.Write([]byte(`{"authorization_code":"authorization-secret","code_verifier":"verifier-secret"}`))
		case 5:
			_, _ = w.Write([]byte(`{"access_token":"access-secret","refresh_token":"refresh-secret"}`))
		default:
			http.Error(w, `{}`, http.StatusInternalServerError)
		}
	}))
	t.Cleanup(remote.Close)
	service, accounts, tokenPath := codexTestService(t, remote.URL, 20*time.Millisecond, time.Second)
	production, err := NewCodexService(accounts)
	if err != nil {
		t.Fatal(err)
	}
	if production.issuer != "https://auth.openai.com" ||
		production.tokenURL != "https://auth.openai.com/oauth/token" {
		t.Fatalf("production Codex OAuth endpoints = %q, %q", production.issuer, production.tokenURL)
	}

	attempt, err := service.StartAuth(
		context.Background(), "codex", "provider_account:codex:default", AuthOAuthDeviceCode,
	)
	if err != nil {
		t.Fatal(err)
	}
	if attempt.Status != AuthAttemptWaiting || attempt.VerificationURL != remote.URL+"/codex/device" ||
		attempt.UserCode != "ABCD-EFGH" {
		t.Fatalf("starting attempt = %#v", attempt)
	}
	events, err := service.Subscribe(context.Background(), attempt.ID)
	if err != nil || (<-events).Status != AuthAttemptWaiting {
		t.Fatalf("subscribe to attempt: %v", err)
	}
	terminal := waitCodexAttempt(t, events)
	if terminal.Status != AuthAttemptCompleted || terminal.ErrorCode != "" {
		t.Fatalf("terminal attempt = %#v", terminal)
	}

	mu.Lock()
	got := append([]codexRecordedRequest(nil), requests...)
	mu.Unlock()
	if len(got) != 5 {
		t.Fatalf("request count = %d, want 5", len(got))
	}
	wantPaths := []string{
		"/api/accounts/deviceauth/usercode", "/api/accounts/deviceauth/token",
		"/api/accounts/deviceauth/token", "/api/accounts/deviceauth/token", "/oauth/token",
	}
	for index, path := range wantPaths {
		if got[index].path != path {
			t.Fatalf("request %d path = %q, want %q", index, got[index].path, path)
		}
	}
	var initial map[string]string
	if err := json.Unmarshal([]byte(got[0].body), &initial); err != nil || initial["client_id"] != codexOAuthClientID {
		t.Fatal("device-code request did not use the Codex client identifier")
	}
	for _, index := range []int{1, 2, 3} {
		var poll map[string]string
		if err := json.Unmarshal([]byte(got[index].body), &poll); err != nil ||
			poll["device_auth_id"] != "device-secret" || poll["user_code"] != "ABCD-EFGH" {
			t.Fatalf("poll request %d is invalid", index)
		}
		if got[index].at.Sub(got[index-1].at) < 15*time.Millisecond {
			t.Fatalf("poll request %d did not wait for the configured floor", index)
		}
	}
	form, err := url.ParseQuery(got[4].body)
	if err != nil || form.Get("grant_type") != "authorization_code" ||
		form.Get("client_id") != codexOAuthClientID || form.Get("redirect_uri") != remote.URL+"/deviceauth/callback" ||
		form.Get("code") != "authorization-secret" || form.Get("code_verifier") != "verifier-secret" {
		t.Fatal("token exchange form is invalid")
	}
	if got[4].contentType != "application/x-www-form-urlencoded" {
		t.Fatalf("token exchange content type = %q", got[4].contentType)
	}
	stored, err := accounts.LoadCodexTokens(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if err := stored.Use(func(access string, refresh string, lastRefresh uint64) error {
		if access != "access-secret" || refresh != "refresh-secret" || lastRefresh == 0 {
			t.Fatal("stored Codex token fields are invalid")
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	if runtime.GOOS != "windows" {
		info, err := os.Stat(tokenPath)
		if err != nil || info.Mode().Perm() != 0o600 {
			t.Fatalf("Codex token mode = %v, %v", info, err)
		}
	}
}

func TestCodexDeviceAuthCancellationStopsBeforeFirstPoll(t *testing.T) {
	requests := make(chan string, 4)
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests <- r.URL.Path
		_, _ = w.Write([]byte(`{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":0}`))
	}))
	t.Cleanup(remote.Close)
	service, _, tokenPath := codexTestService(t, remote.URL, 100*time.Millisecond, time.Second)
	attempt, err := service.StartAuth(
		context.Background(), "codex", "provider_account:codex:default", AuthOAuthDeviceCode,
	)
	if err != nil {
		t.Fatal(err)
	}
	if path := <-requests; path != "/api/accounts/deviceauth/usercode" {
		t.Fatalf("initial path = %q", path)
	}
	cancelled, ok := service.Cancel(attempt.ID)
	if !ok || cancelled.Status != AuthAttemptCancelled || cancelled.ErrorCode != "" {
		t.Fatalf("cancelled attempt = %#v", cancelled)
	}
	select {
	case path := <-requests:
		t.Fatalf("cancelled attempt sent request to %q", path)
	case <-time.After(150 * time.Millisecond):
	}
	if _, err := os.Stat(tokenPath); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("cancelled attempt token file error = %v", err)
	}
}

func TestCodexDeviceAuthRejectsStalePublication(t *testing.T) {
	tokenRequest := make(chan struct{})
	releaseToken := make(chan struct{})
	var count int
	var mu sync.Mutex
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		mu.Lock()
		count++
		requestNumber := count
		mu.Unlock()
		switch requestNumber {
		case 1:
			_, _ = w.Write([]byte(`{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":0}`))
		case 2:
			_, _ = w.Write([]byte(`{"authorization_code":"authorization-secret","code_verifier":"verifier-secret"}`))
		case 3:
			close(tokenRequest)
			<-releaseToken
			_, _ = w.Write([]byte(`{"access_token":"stale-access","refresh_token":"stale-refresh"}`))
		}
	}))
	t.Cleanup(remote.Close)
	service, accounts, tokenPath := codexTestService(t, remote.URL, time.Millisecond, time.Second)
	attempt, err := service.StartAuth(
		context.Background(), "codex", "provider_account:codex:default", AuthOAuthDeviceCode,
	)
	if err != nil {
		t.Fatal(err)
	}
	events, _ := service.Subscribe(context.Background(), attempt.ID)
	<-events
	<-tokenRequest
	persistence := accounts.persistence.(*memoryAccountPersistence)
	persistence.mu.Lock()
	account := persistence.accounts[attempt.ProviderAccountID]
	account.Metadata = credentialMetadata(1, false)
	persistence.accounts[attempt.ProviderAccountID] = account
	persistence.mu.Unlock()
	close(releaseToken)

	terminal := waitCodexAttempt(t, events)
	if terminal.Status != AuthAttemptFailed || terminal.ErrorCode != "provider_auth_publication_failed" ||
		!strings.Contains(terminal.ErrorMessage, "changed") {
		t.Fatalf("stale terminal attempt = %#v", terminal)
	}
	if _, err := os.Stat(tokenPath); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("stale attempt token file error = %v", err)
	}
}

func TestCodexDeviceAuthUsesBoundedSafeFailureMessages(t *testing.T) {
	remoteSecret := "remote-secret-diagnostic"
	var count int
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		count++
		if count == 1 {
			_, _ = w.Write([]byte(`{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":0}`))
			return
		}
		http.Error(w, remoteSecret, http.StatusInternalServerError)
	}))
	t.Cleanup(remote.Close)
	service, _, _ := codexTestService(t, remote.URL, time.Millisecond, time.Second)
	attempt, err := service.StartAuth(
		context.Background(), "codex", "provider_account:codex:default", AuthOAuthDeviceCode,
	)
	if err != nil {
		t.Fatal(err)
	}
	events, _ := service.Subscribe(context.Background(), attempt.ID)
	<-events
	terminal := waitCodexAttempt(t, events)
	if terminal.Status != AuthAttemptFailed || terminal.ErrorMessage != "Codex login request failed" ||
		strings.Contains(terminal.ErrorMessage, remoteSecret) {
		t.Fatalf("unsafe terminal attempt = %#v", terminal)
	}

	oversized := strings.NewReader(strings.Repeat("x", codexOAuthResponseLimit+1))
	if _, err := readCodexResponse(oversized); err == nil {
		t.Fatal("oversized Codex response was accepted")
	}
}

type codexRecordedRequest struct {
	path        string
	contentType string
	body        string
	at          time.Time
}

func codexTestService(
	t *testing.T,
	baseURL string,
	pollFloor time.Duration,
	ttl time.Duration,
) (*CodexService, *AccountService, string) {
	t.Helper()
	now := time.Now().UTC()
	persistence := &memoryAccountPersistence{accounts: make(map[string]Account)}
	account := BuiltinAccounts(now)[0]
	persistence.accounts[account.ID] = account
	root := filepath.Join(t.TempDir(), "home")
	accounts, err := NewAccountService(root, persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newCodexService(
		accounts, baseURL, baseURL+"/oauth/token", http.DefaultClient, ttl, pollFloor,
	)
	if err != nil {
		t.Fatal(err)
	}
	return service, accounts, filepath.Join(root, "providers", "codex", "default", "codex_tokens.json")
}

func waitCodexAttempt(t *testing.T, events <-chan AuthAttempt) AuthAttempt {
	t.Helper()
	select {
	case event, open := <-events:
		if !open {
			t.Fatal("Codex attempt stream closed without a terminal event")
		}
		return event
	case <-time.After(2 * time.Second):
		t.Fatal("Codex attempt did not reach a terminal state")
		return AuthAttempt{}
	}
}
