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
			path: r.URL.RequestURI(), contentType: r.Header.Get("Content-Type"), body: string(body),
			authorized: r.Header.Get("Authorization") == "Bearer access-secret", at: time.Now(),
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
		case 6:
			_, _ = w.Write([]byte(`{"version":"0.144.1"}`))
		case 7:
			_, _ = w.Write([]byte(`{"models":[{"slug":"gpt-5.6-terra","display_name":"GPT-5.6 Terra","visibility":"list","default_reasoning_level":"medium","supported_reasoning_levels":["low","medium","high","xhigh"]},{"slug":"hidden","visibility":"hide"}]}`))
		default:
			http.Error(w, `{}`, http.StatusInternalServerError)
		}
	}))
	t.Cleanup(remote.Close)
	service, accounts, tokenPath := codexTestService(t, remote.URL, 20*time.Millisecond, time.Second)
	persistence := accounts.persistence.(*memoryAccountPersistence)
	delete(persistence.accounts, "provider_account:codex:default")
	production, err := NewCodexService(accounts)
	if err != nil {
		t.Fatal(err)
	}
	if production.issuer != "https://auth.openai.com" ||
		production.tokenURL != "https://auth.openai.com/oauth/token" ||
		production.modelsBaseURL != "https://chatgpt.com/backend-api/codex" {
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
	if len(got) != 7 {
		t.Fatalf("request count = %d, want 7", len(got))
	}
	wantPaths := []string{
		"/api/accounts/deviceauth/usercode", "/api/accounts/deviceauth/token",
		"/api/accounts/deviceauth/token", "/api/accounts/deviceauth/token", "/oauth/token",
		"/latest", "/models?client_version=0.144.1",
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
	if !got[6].authorized {
		t.Fatal("model catalog request did not use the transient access token")
	}
	account, err := accounts.LoadAccount(context.Background(), "provider_account:codex:default")
	if err != nil {
		t.Fatal(err)
	}
	profiles, err := account.Metadata.ModelProfiles()
	if err != nil || len(profiles) != 1 || profiles[0].ID != "gpt-5.6-terra" ||
		profiles[0].DefaultReasoningEffort != "medium" || len(profiles[0].ReasoningEfforts) != 4 {
		t.Fatalf("stored Codex profiles = %#v, %v", profiles, err)
	}
	var clientVersion string
	if json.Unmarshal(account.Metadata["models_client_version"], &clientVersion) != nil || clientVersion != "0.144.1" {
		t.Fatalf("stored Codex client version = %q", clientVersion)
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

func TestCodexDeviceAuthCloseCancelsDetachedPolling(t *testing.T) {
	requests := make(chan string, 4)
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests <- r.URL.Path
		_, _ = w.Write([]byte(`{"device_auth_id":"device-secret","user_code":"ABCD-EFGH","interval":0}`))
	}))
	t.Cleanup(remote.Close)
	service, _, _ := codexTestService(t, remote.URL, 100*time.Millisecond, time.Second)
	attempt, err := service.StartAuth(
		context.Background(), "codex", "provider_account:codex:default", AuthOAuthDeviceCode,
	)
	if err != nil {
		t.Fatal(err)
	}
	if path := <-requests; path != "/api/accounts/deviceauth/usercode" {
		t.Fatalf("initial path = %q", path)
	}
	events, err := service.Subscribe(context.Background(), attempt.ID)
	if err != nil || (<-events).Status != AuthAttemptWaiting {
		t.Fatalf("subscribe before close: %v", err)
	}
	service.Close()
	service.Close()
	if terminal, open := <-events; !open || terminal.Status != AuthAttemptCancelled {
		t.Fatalf("close event = %#v, open %v", terminal, open)
	}
	if _, err := service.StartAuth(
		context.Background(), "codex", "provider_account:codex:default", AuthOAuthDeviceCode,
	); !errors.Is(err, ErrProviderUnavailable) {
		t.Fatalf("start after close error = %v", err)
	}
	select {
	case path := <-requests:
		t.Fatalf("closed service sent request to %q", path)
	case <-time.After(150 * time.Millisecond):
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
		case 4:
			_, _ = w.Write([]byte(`{"version":"0.144.1"}`))
		case 5:
			_, _ = w.Write([]byte(`{"models":[{"slug":"gpt-live","visibility":"list"}]}`))
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
	authorized  bool
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
	service.modelsBaseURL = baseURL
	service.versionURL = baseURL + "/latest"
	return service, accounts, filepath.Join(root, "providers", "codex", "default", "codex_tokens.json")
}

func TestCodexCatalogParsesVisibleModelsAndValidatesVersions(t *testing.T) {
	profiles, err := codexProfiles([]byte(`{"models":[
		{"slug":"gpt-live","display_name":"GPT Live","visibility":"list","default_reasoning_effort":"max"},
		{"slug":"hidden","visibility":"hide"},
		{"slug":"gpt-live","visibility":"list"},
		{"slug":"","visibility":"list"}
	]}`))
	if err != nil || len(profiles) != 1 || profiles[0].ID != "gpt-live" ||
		profiles[0].DefaultReasoningEffort != "xhigh" || len(profiles[0].ReasoningEfforts) != 4 {
		t.Fatalf("Codex profiles = %#v, %v", profiles, err)
	}
	cases := map[string]bool{
		"0.144.1": true, "0.144.1-alpha.1": true, "latest": false,
		"0.144": false, "0.144.1+build": false, "0.144.1-": false,
	}
	for value, want := range cases {
		if got := validCodexClientVersion(value); got != want {
			t.Fatalf("validCodexClientVersion(%q) = %v, want %v", value, got, want)
		}
	}
}

func TestCodexCatalogUsesSafeClientVersionFallback(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/latest":
			http.Error(w, "unavailable", http.StatusServiceUnavailable)
		case "/models":
			if request.URL.Query().Get("client_version") != codexFallbackClientVersion ||
				request.Header.Get("Authorization") != "Bearer access" {
				t.Errorf("fallback catalog request was invalid: %s", request.URL.String())
			}
			_, _ = w.Write([]byte(`{"models":[{"slug":"gpt-fallback","visibility":"list"}]}`))
		default:
			http.NotFound(w, request)
		}
	}))
	t.Cleanup(remote.Close)
	service, _, _ := codexTestService(t, remote.URL, time.Millisecond, time.Second)
	catalog, err := service.fetchModelCatalog(context.Background(), CodexTokens{
		accessToken: "access", refreshToken: "refresh", lastRefresh: 1,
	})
	if err != nil || catalog.ClientVersion != codexFallbackClientVersion ||
		catalog.VersionFetchedAt != nil || len(catalog.Profiles) != 1 {
		t.Fatalf("fallback catalog = %#v, %v", catalog, err)
	}
	metadata, err := codexMetadataWithCatalog(AccountMetadata{
		"models_client_version_refreshed_at": json.RawMessage(`123`),
	}, catalog, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, exists := metadata["models_client_version_refreshed_at"]; exists {
		t.Fatal("fallback catalog retained a timestamp from a different client version")
	}
}

func TestCodexCatalogPreservesModelContextWindow(t *testing.T) {
	for _, test := range []struct {
		field string
		want  uint32
	}{
		{`,"context_window":272000,"max_context_window":872000`, 272000},
		{`,"context_window":64000`, 64000},
		{"", 128000},
		{`,"context_window":0`, 128000},
		{`,"context_window":-1`, 128000},
		{`,"context_window":"272000"`, 128000},
	} {
		t.Run(test.field, func(t *testing.T) {
			profile, visible := codexProfile(json.RawMessage(`{"slug":"test","visibility":"list"` + test.field + `}`))
			if !visible || profile.ContextWindowTokens == nil || *profile.ContextWindowTokens != test.want {
				t.Fatalf("context window = %v, want %d", profile.ContextWindowTokens, test.want)
			}
		})
	}
}

func TestCodexCatalogPublicationRestoresPriorTokens(t *testing.T) {
	_, accounts, _ := codexTestService(t, "https://codex.invalid", time.Millisecond, time.Second)
	now := time.Now().UTC()
	firstCatalog := codexModelCatalog{
		Profiles:      []ModelProfile{{ID: "gpt-first", Label: "GPT First"}},
		ClientVersion: "0.144.1",
	}
	if _, err := accounts.publishCodexTokens(context.Background(), 0, CodexTokens{
		accessToken: "first-access", refreshToken: "first-refresh", lastRefresh: 1,
	}, firstCatalog, now); err != nil {
		t.Fatal(err)
	}
	memory := accounts.persistence.(*memoryAccountPersistence)
	failingAccounts, err := NewAccountService(accounts.root, &failingCodexPersistence{memory})
	if err != nil {
		t.Fatal(err)
	}
	_, err = failingAccounts.publishCodexTokens(context.Background(), 1, CodexTokens{
		accessToken: "new-access", refreshToken: "new-refresh", lastRefresh: 2,
	}, codexModelCatalog{
		Profiles: []ModelProfile{{ID: "gpt-new", Label: "GPT New"}}, ClientVersion: "0.144.2",
	}, now.Add(time.Second))
	if err == nil {
		t.Fatal("failed Codex metadata publication succeeded")
	}
	stored, err := accounts.LoadCodexTokens(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if err := stored.Use(func(access string, refresh string, _ uint64) error {
		if access != "first-access" || refresh != "first-refresh" {
			t.Fatal("Codex token rollback did not restore the prior tokens")
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	account, err := accounts.LoadAccount(context.Background(), "provider_account:codex:default")
	if err != nil {
		t.Fatal(err)
	}
	profiles, err := account.Metadata.ModelProfiles()
	if err != nil || len(profiles) != 1 || profiles[0].ID != "gpt-first" {
		t.Fatalf("profiles after rollback = %#v, %v", profiles, err)
	}
}

func TestCodexCatalogFailureDoesNotPublishTokens(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/api/accounts/deviceauth/usercode":
			_, _ = w.Write([]byte(`{"device_auth_id":"device","user_code":"CODE","interval":0}`))
		case "/api/accounts/deviceauth/token":
			_, _ = w.Write([]byte(`{"authorization_code":"authorization","code_verifier":"verifier"}`))
		case "/oauth/token":
			_, _ = w.Write([]byte(`{"access_token":"access","refresh_token":"refresh"}`))
		case "/latest":
			_, _ = w.Write([]byte(`{"version":"0.144.1"}`))
		case "/models":
			_, _ = w.Write([]byte(`{"models":[]}`))
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
	terminal := waitCodexAttempt(t, events)
	if terminal.Status != AuthAttemptFailed || terminal.ErrorCode != "provider_model_catalog_failed" ||
		terminal.ErrorMessage != "Codex returned an invalid model catalog" {
		t.Fatalf("catalog failure terminal = %#v", terminal)
	}
	if _, err := os.Stat(tokenPath); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("catalog failure token file = %v", err)
	}
	account, err := accounts.LoadAccount(context.Background(), attempt.ProviderAccountID)
	if err != nil || account.Status == StatusAuthenticated || account.Metadata.CredentialRevision() != 0 {
		t.Fatalf("account after catalog failure = %#v, %v", account, err)
	}
}

func TestCodexCatalogExpiryCannotPublishReturnedModels(t *testing.T) {
	modelsStarted := make(chan struct{})
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/api/accounts/deviceauth/usercode":
			_, _ = w.Write([]byte(`{"device_auth_id":"device","user_code":"CODE","interval":0}`))
		case "/api/accounts/deviceauth/token":
			_, _ = w.Write([]byte(`{"authorization_code":"authorization","code_verifier":"verifier"}`))
		case "/oauth/token":
			_, _ = w.Write([]byte(`{"access_token":"access","refresh_token":"refresh"}`))
		case "/latest":
			_, _ = w.Write([]byte(`{"version":"0.144.1"}`))
		case "/models":
			close(modelsStarted)
			<-request.Context().Done()
		}
	}))
	t.Cleanup(remote.Close)
	service, _, tokenPath := codexTestService(t, remote.URL, time.Millisecond, 30*time.Millisecond)
	attempt, err := service.StartAuth(
		context.Background(), "codex", "provider_account:codex:default", AuthOAuthDeviceCode,
	)
	if err != nil {
		t.Fatal(err)
	}
	events, _ := service.Subscribe(context.Background(), attempt.ID)
	<-events
	<-modelsStarted
	terminal := waitCodexAttempt(t, events)
	if terminal.Status != AuthAttemptExpired || terminal.ErrorCode != "provider_auth_expired" {
		t.Fatalf("catalog expiry terminal = %#v", terminal)
	}
	if _, err := os.Stat(tokenPath); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("expired catalog token file = %v", err)
	}
}

func TestCodexPublicationDeadlineExpiresAndRestoresTokens(t *testing.T) {
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/api/accounts/deviceauth/usercode":
			_, _ = w.Write([]byte(`{"device_auth_id":"device","user_code":"CODE","interval":0}`))
		case "/api/accounts/deviceauth/token":
			_, _ = w.Write([]byte(`{"authorization_code":"authorization","code_verifier":"verifier"}`))
		case "/oauth/token":
			_, _ = w.Write([]byte(`{"access_token":"access","refresh_token":"refresh"}`))
		case "/latest":
			_, _ = w.Write([]byte(`{"version":"0.144.1"}`))
		case "/models":
			_, _ = w.Write([]byte(`{"models":[{"slug":"gpt-live","visibility":"list"}]}`))
		}
	}))
	t.Cleanup(remote.Close)
	service, accounts, tokenPath := codexTestService(t, remote.URL, time.Millisecond, 30*time.Millisecond)
	memory := accounts.persistence.(*memoryAccountPersistence)
	deadlineAccounts, err := NewAccountService(accounts.root, &deadlineCodexPersistence{memory})
	if err != nil {
		t.Fatal(err)
	}
	service.accounts = deadlineAccounts
	attempt, err := service.StartAuth(
		context.Background(), "codex", "provider_account:codex:default", AuthOAuthDeviceCode,
	)
	if err != nil {
		t.Fatal(err)
	}
	events, _ := service.Subscribe(context.Background(), attempt.ID)
	<-events
	terminal := waitCodexAttempt(t, events)
	if terminal.Status != AuthAttemptExpired || terminal.ErrorCode != "provider_auth_expired" {
		t.Fatalf("publication deadline terminal = %#v", terminal)
	}
	if _, err := os.Stat(tokenPath); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("publication deadline token file = %v", err)
	}
}

type failingCodexPersistence struct{ *memoryAccountPersistence }

func (*failingCodexPersistence) UpdateProviderCredential(
	context.Context, string, uint64, AuthMethod, bool, AccountMetadata, time.Time,
) (Account, error) {
	return Account{}, errors.New("injected Codex metadata failure")
}

type deadlineCodexPersistence struct{ *memoryAccountPersistence }

func (*deadlineCodexPersistence) UpdateProviderCredential(
	ctx context.Context, _ string, _ uint64, _ AuthMethod, _ bool, _ AccountMetadata, _ time.Time,
) (Account, error) {
	<-ctx.Done()
	return Account{}, ctx.Err()
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
