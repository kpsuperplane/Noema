package provider

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"
)

func TestOpenRouterAPIKeyVerificationPublishesProfilesAndSecret(t *testing.T) {
	remote, requests := openRouterFixture(t, "published-key")
	service, accounts := openRouterTestService(t, remote.URL, time.Minute)
	secret, _ := NewSecret("published-key")
	account, err := service.CreateAPIKeyAccount(context.Background(), secret)
	if err != nil {
		t.Fatalf("create OpenRouter account: %v", err)
	}
	if account.Status != StatusAuthenticated || account.Metadata.CredentialRevision() != 1 {
		t.Fatalf("published account = %#v", account)
	}
	var profiles []ModelProfile
	if err := json.Unmarshal(account.Metadata["profiles"], &profiles); err != nil ||
		len(profiles) != 3 || profiles[0].ID != "openrouter/auto" || profiles[1].ID != "vendor/model" ||
		profiles[1].DefaultReasoningEffort != "xhigh" {
		t.Fatalf("published profiles = %#v, %v", profiles, err)
	}
	if profiles[2].ID != "vendor/optional-malformed" || profiles[2].Label != "vendor/optional-malformed" {
		t.Fatalf("malformed optional metadata removed a compatible model: %#v", profiles)
	}
	loaded, err := accounts.LoadSecret(context.Background(), account.ID)
	if err != nil {
		t.Fatal(err)
	}
	if err := loaded.Use(func(value string) error {
		if value != "published-key" {
			t.Fatalf("stored key = %q", value)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	for _, request := range <-requests {
		if request.path != "/auth/keys" && request.authorization != "Bearer published-key" {
			t.Fatalf("request %s authorization = %q", request.path, request.authorization)
		}
	}

	rejectedRemote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		http.Error(w, "remote-secret-diagnostic", http.StatusUnauthorized)
	}))
	t.Cleanup(rejectedRemote.Close)
	rejectedService, rejectedAccounts := openRouterTestService(t, rejectedRemote.URL, time.Minute)
	rejectedSecret, _ := NewSecret("rejected-key")
	_, err = rejectedService.CreateAPIKeyAccount(context.Background(), rejectedSecret)
	if !errors.Is(err, ErrAuthenticationRejected) || strings.Contains(err.Error(), "remote-secret-diagnostic") {
		t.Fatalf("rejected credential error = %v", err)
	}
	if _, err := rejectedAccounts.LoadAccount(context.Background(), "provider_account:openrouter:default"); !errors.Is(err, ErrAccountNotFound) {
		t.Fatalf("rejected account was published: %v", err)
	}
}

func TestOpenRouterPKCECallbackPublishesOnceAndEmitsSafeState(t *testing.T) {
	remote, _ := openRouterFixture(t, "oauth-key")
	service, accounts := openRouterTestService(t, remote.URL, time.Minute)
	attempt, err := service.StartAuth(
		context.Background(), "openrouter", "provider_account:openrouter:default", AuthOAuthPKCE,
	)
	if err != nil {
		t.Fatal(err)
	}
	authorization, err := url.Parse(attempt.VerificationURL)
	if err != nil {
		t.Fatal(err)
	}
	query := authorization.Query()
	if query.Get("code_challenge_method") != "S256" || query.Get("code_challenge") == "" ||
		strings.Contains(attempt.VerificationURL, "oauth-key") ||
		!strings.HasSuffix(query.Get("callback_url"), "/"+attempt.ID) {
		t.Fatalf("unsafe PKCE start = %#v, %q", attempt, authorization.RawQuery)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	events, err := service.Subscribe(ctx, attempt.ID)
	if err != nil || (<-events).Status != AuthAttemptWaiting {
		t.Fatalf("initial event is unavailable: %v", err)
	}
	completed, err := service.CompleteCallback(context.Background(), attempt.ID, "one-use-code")
	if err != nil || completed.Status != AuthAttemptCompleted {
		t.Fatalf("callback result = %#v, %v", completed, err)
	}
	if event, open := <-events; !open || event.Status != AuthAttemptCompleted {
		t.Fatalf("terminal event = %#v, open %v", event, open)
	}
	if _, open := <-events; open {
		t.Fatal("terminal event stream stayed open")
	}
	if _, err := service.CompleteCallback(context.Background(), attempt.ID, "second-code"); !errors.Is(err, ErrAuthAttemptNotCurrent) {
		t.Fatalf("callback replay error = %v", err)
	}
	account, err := accounts.LoadAccount(context.Background(), "provider_account:openrouter:default")
	if err != nil || account.AuthMethod != AuthOAuthPKCE || account.Metadata.CredentialRevision() != 1 {
		t.Fatalf("OAuth account = %#v, %v", account, err)
	}
}

func TestOpenRouterStaleCallbackCannotReplaceNewerCredential(t *testing.T) {
	remote, _ := openRouterFixture(t, "stale-oauth-key")
	service, accounts := openRouterTestService(t, remote.URL, time.Minute)
	attempt, err := service.StartAuth(
		context.Background(), "openrouter", "provider_account:openrouter:default", AuthOAuthPKCE,
	)
	if err != nil {
		t.Fatal(err)
	}
	current, _ := NewSecret("current-key")
	if _, err := accounts.CreateSecretAccount(context.Background(), "openrouter", "", current, time.Now()); err != nil {
		t.Fatal(err)
	}
	completed, err := service.CompleteCallback(context.Background(), attempt.ID, "stale-code")
	if err != nil || completed.Status != AuthAttemptFailed ||
		completed.ErrorMessage != "Provider account credentials changed during authentication" {
		t.Fatalf("stale callback = %#v, %v", completed, err)
	}
	loaded, err := accounts.LoadSecret(context.Background(), "provider_account:openrouter:default")
	if err != nil {
		t.Fatal(err)
	}
	_ = loaded.Use(func(value string) error {
		if value != "current-key" {
			t.Fatalf("stale callback replaced current key with %q", value)
		}
		return nil
	})
}

func TestOpenRouterAttemptsSupersedeCancelAndExpire(t *testing.T) {
	remote, _ := openRouterFixture(t, "unused")
	service, _ := openRouterTestService(t, remote.URL, 20*time.Millisecond)
	first, err := service.StartAuth(
		context.Background(), "openrouter", "provider_account:openrouter:default", AuthOAuthPKCE,
	)
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	firstEvents, _ := service.Subscribe(ctx, first.ID)
	<-firstEvents
	second, err := service.StartAuth(
		context.Background(), "openrouter", "provider_account:openrouter:default", AuthOAuthPKCE,
	)
	if err != nil {
		t.Fatal(err)
	}
	if superseded := <-firstEvents; superseded.Status != AuthAttemptCancelled || superseded.ErrorCode != "" {
		t.Fatalf("superseded event = %#v", superseded)
	}
	secondEvents, _ := service.Subscribe(ctx, second.ID)
	<-secondEvents
	select {
	case expired := <-secondEvents:
		if expired.Status != AuthAttemptExpired || expired.ErrorCode != "provider_auth_expired" {
			t.Fatalf("expired event = %#v", expired)
		}
	case <-time.After(time.Second):
		t.Fatal("OpenRouter attempt did not expire")
	}
	deadline := time.Now().Add(time.Second)
	for {
		if _, exists := service.Attempt(second.ID); !exists {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("terminal OpenRouter attempt was not retired")
		}
		time.Sleep(time.Millisecond)
	}
}

func TestOpenRouterCallbackHandlerRejectsInvalidRequestsWithoutEcho(t *testing.T) {
	remote, _ := openRouterFixture(t, "handler-key")
	service, _ := openRouterTestService(t, remote.URL, time.Minute)
	attempt, err := service.StartAuth(
		context.Background(), "openrouter", "provider_account:openrouter:default", AuthOAuthPKCE,
	)
	if err != nil {
		t.Fatal(err)
	}
	handler := service.CallbackHandler()
	oversized := httptest.NewRecorder()
	handler.ServeHTTP(oversized, httptest.NewRequest(
		http.MethodGet,
		"/provider/oauth/callback/"+attempt.ID+"?padding="+strings.Repeat("x", 9000),
		nil,
	))
	if oversized.Code != http.StatusBadRequest {
		t.Fatalf("oversized callback status = %d", oversized.Code)
	}
	invalid := httptest.NewRecorder()
	handler.ServeHTTP(invalid, httptest.NewRequest(
		http.MethodGet, "/provider/oauth/callback/"+attempt.ID, nil,
	))
	if invalid.Code != http.StatusBadRequest || strings.Contains(invalid.Body.String(), "secret-value") {
		t.Fatalf("invalid callback = %d %q", invalid.Code, invalid.Body.String())
	}
	valid := httptest.NewRecorder()
	handler.ServeHTTP(valid, httptest.NewRequest(
		http.MethodGet, "/provider/oauth/callback/"+attempt.ID+"?code=one-use&code=ignored-secret", nil,
	))
	if valid.Code != http.StatusOK || !strings.Contains(valid.Body.String(), "Authentication completed.") ||
		strings.Contains(valid.Body.String(), "one-use") || strings.Contains(valid.Body.String(), "ignored-secret") {
		t.Fatalf("valid callback = %d %q", valid.Code, valid.Body.String())
	}
}

type recordedOpenRouterRequest struct {
	path          string
	authorization string
}

func openRouterFixture(t *testing.T, key string) (*httptest.Server, <-chan []recordedOpenRouterRequest) {
	t.Helper()
	completed := make(chan []recordedOpenRouterRequest, 4)
	var mu sync.Mutex
	requests := make([]recordedOpenRouterRequest, 0, 3)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		requests = append(requests, recordedOpenRouterRequest{r.URL.Path, r.Header.Get("Authorization")})
		count := len(requests)
		copyRequests := append([]recordedOpenRouterRequest(nil), requests...)
		mu.Unlock()
		w.Header().Set("Content-Type", "application/json")
		switch r.URL.Path {
		case "/auth/keys":
			body, _ := io.ReadAll(r.Body)
			if r.Method != http.MethodPost || !strings.Contains(string(body), `"code_challenge_method":"S256"`) {
				http.Error(w, `{}`, http.StatusBadRequest)
				return
			}
			_, _ = w.Write([]byte(`{"key":"` + key + `"}`))
		case "/key":
			_, _ = w.Write([]byte(`{"data":{"label":"test"}}`))
		case "/models/user":
			_, _ = w.Write([]byte(`{"data":[{"id":"vendor/model","name":"Vendor Model","context_length":65536,"architecture":{"output_modalities":["text"]},"supported_parameters":["tools","tool_choice"],"reasoning":{"supported_efforts":["low","max"],"default_effort":"max"}},{"id":"vendor/optional-malformed","name":4,"context_length":65536,"architecture":{"output_modalities":["text",4]},"supported_parameters":["tools",4,"tool_choice"],"reasoning":"unknown"},{"id":"short","context_length":8192,"architecture":{"output_modalities":["text"]},"supported_parameters":["tools","tool_choice"]}]}`))
		default:
			http.NotFound(w, r)
		}
		if count == 3 || count == 2 && requests[0].path != "/auth/keys" {
			select {
			case completed <- copyRequests:
			default:
			}
		}
	}))
	t.Cleanup(server.Close)
	return server, completed
}

func openRouterTestService(
	t *testing.T,
	apiBase string,
	ttl time.Duration,
) (*OpenRouterService, *AccountService) {
	t.Helper()
	persistence := &memoryAccountPersistence{accounts: make(map[string]Account)}
	accounts, err := NewAccountService(filepath.Join(t.TempDir(), "home"), persistence)
	if err != nil {
		t.Fatal(err)
	}
	service, err := newOpenRouterService(
		accounts, "http://localhost:3737/provider/oauth/callback", apiBase,
		apiBase+"/authorize", http.DefaultClient, ttl,
	)
	if err != nil {
		t.Fatal(err)
	}
	return service, accounts
}

type memoryAccountPersistence struct {
	mu       sync.Mutex
	accounts map[string]Account
}

func (m *memoryAccountPersistence) EnsureBuiltinProviderAccounts(context.Context, time.Time) error {
	return nil
}

func (m *memoryAccountPersistence) ProviderAccount(_ context.Context, id string) (Account, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	account, exists := m.accounts[id]
	if !exists {
		return Account{}, ErrAccountNotFound
	}
	return account, nil
}

func (m *memoryAccountPersistence) ActiveProviderAccounts(context.Context) ([]Account, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	result := make([]Account, 0, len(m.accounts))
	for _, account := range m.accounts {
		result = append(result, account)
	}
	return result, nil
}

func (m *memoryAccountPersistence) CreateProviderAccount(_ context.Context, account Account) (Account, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if _, exists := m.accounts[account.ID]; exists {
		return Account{}, ErrAccountConflict
	}
	m.accounts[account.ID] = account
	return account, nil
}

func (m *memoryAccountPersistence) UpdateProviderCredential(
	_ context.Context,
	id string,
	expected uint64,
	method AuthMethod,
	configured bool,
	metadata AccountMetadata,
	now time.Time,
) (Account, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	account, exists := m.accounts[id]
	if !exists {
		return Account{}, ErrAccountNotFound
	}
	if account.Metadata.CredentialRevision() != expected {
		return Account{}, ErrAccountConflict
	}
	metadata = cloneMetadata(metadata)
	revision, _ := json.Marshal(expected + 1)
	secretConfigured, _ := json.Marshal(configured)
	metadata["credentialRevision"] = revision
	metadata["secretConfigured"] = secretConfigured
	account.Metadata = metadata
	account.AuthMethod = method
	account.Status = StatusAuthenticated
	account.UpdatedAt = now
	account.LastCheckedAt = &now
	account.LastAuthenticatedAt = &now
	m.accounts[id] = account
	return account, nil
}

func (m *memoryAccountPersistence) DeleteProviderAccount(_ context.Context, id string) (bool, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if _, exists := m.accounts[id]; !exists {
		return false, nil
	}
	delete(m.accounts, id)
	return true, nil
}
