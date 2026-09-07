package provider

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"sort"
	"strings"
	"sync"
	"time"
)

const (
	openRouterAPIBase          = "https://openrouter.ai/api/v1"
	openRouterAuthorizationURL = "https://openrouter.ai/auth"
	openRouterResponseLimit    = 4 << 20
	minimumContextTokens       = 32_768
	defaultAuthAttemptTTL      = 5 * time.Minute
)

var (
	// ErrProviderUnavailable means a provider did not return a usable response.
	ErrProviderUnavailable = errors.New("provider unavailable")
	// ErrAuthenticationRejected means OpenRouter rejected a credential or code.
	ErrAuthenticationRejected = errors.New("provider authentication rejected")
	// ErrProviderRateLimited means a provider refused work because of a rate limit.
	ErrProviderRateLimited = errors.New("provider rate limited")
	// ErrProviderRequestRejected means a provider rejected the request or account state.
	ErrProviderRequestRejected = errors.New("provider request rejected")
	// ErrProviderPaymentRequired means a provider account cannot fund the request.
	ErrProviderPaymentRequired = errors.New("provider payment required")
	// ErrAuthAttemptNotCurrent means an attempt cannot change current credentials.
	ErrAuthAttemptNotCurrent = errors.New("provider authentication attempt is not current")
)

// ModelProfile is safe model metadata returned by a hosted provider.
type ModelProfile struct {
	ID                     string   `json:"id"`
	Label                  string   `json:"label"`
	ReasoningEfforts       []string `json:"reasoning_efforts,omitempty"`
	DefaultReasoningEffort string   `json:"default_reasoning_effort,omitempty"`
	ContextWindowTokens    *uint32  `json:"context_window_tokens,omitempty"`
}

// AuthAttemptStatus is the current state of one short-lived provider login.
type AuthAttemptStatus string

const (
	AuthAttemptWaiting   AuthAttemptStatus = "waiting_for_user"
	AuthAttemptCompleted AuthAttemptStatus = "completed"
	AuthAttemptFailed    AuthAttemptStatus = "failed"
	AuthAttemptExpired   AuthAttemptStatus = "expired"
	AuthAttemptCancelled AuthAttemptStatus = "cancelled"
)

// AuthAttempt contains only provider authentication state safe for clients.
type AuthAttempt struct {
	ID                string
	ProviderKind      string
	ProviderAccountID string
	Method            AuthMethod
	Status            AuthAttemptStatus
	VerificationURL   string
	UserCode          string
	Instructions      string
	ErrorCode         string
	ErrorMessage      string
}

type openRouterAttempt struct {
	view             AuthAttempt
	verifier         string
	expectedRevision uint64
	claimed          bool
	timer            *time.Timer
	subscribers      map[chan AuthAttempt]struct{}
}

// OpenRouterService owns OpenRouter validation and short-lived PKCE state.
type OpenRouterService struct {
	accounts      *AccountService
	client        *http.Client
	apiBase       string
	authorizeURL  string
	callbackBase  *url.URL
	attemptTTL    time.Duration
	now           func() time.Time
	mu            sync.Mutex
	attempts      map[string]*openRouterAttempt
	latestAttempt string
}

// NewOpenRouterService creates the production OpenRouter onboarding service.
func NewOpenRouterService(accounts *AccountService, callbackBase string) (*OpenRouterService, error) {
	return newOpenRouterService(
		accounts,
		callbackBase,
		openRouterAPIBase,
		openRouterAuthorizationURL,
		&http.Client{Timeout: 20 * time.Second},
		defaultAuthAttemptTTL,
	)
}

func newOpenRouterService(
	accounts *AccountService,
	callbackBase string,
	apiBase string,
	authorizeURL string,
	client *http.Client,
	attemptTTL time.Duration,
) (*OpenRouterService, error) {
	if accounts == nil || client == nil || attemptTTL <= 0 {
		return nil, errors.New("OpenRouter onboarding dependencies are unavailable")
	}
	callback, err := exactHTTPURL(callbackBase)
	if err != nil || callback.RawQuery != "" || callback.Fragment != "" {
		return nil, errors.New("OpenRouter callback URL is invalid")
	}
	api, err := exactHTTPURL(strings.TrimRight(apiBase, "/"))
	if err != nil || api.RawQuery != "" || api.Fragment != "" {
		return nil, errors.New("OpenRouter API URL is invalid")
	}
	authorize, err := exactHTTPURL(authorizeURL)
	if err != nil || authorize.RawQuery != "" || authorize.Fragment != "" {
		return nil, errors.New("OpenRouter authorization URL is invalid")
	}
	return &OpenRouterService{
		accounts: accounts, client: client, apiBase: api.String(), authorizeURL: authorize.String(),
		callbackBase: callback, attemptTTL: attemptTTL, now: time.Now,
		attempts: make(map[string]*openRouterAttempt),
	}, nil
}

// CreateAPIKeyAccount verifies and publishes the default OpenRouter account.
func (s *OpenRouterService) CreateAPIKeyAccount(
	ctx context.Context,
	secret Secret,
) (Account, error) {
	revision, err := s.startingRevision(ctx, true)
	if err != nil {
		return Account{}, err
	}
	profiles, err := s.verifyAPIKey(ctx, secret)
	if err != nil {
		return Account{}, err
	}
	return s.accounts.PublishVerifiedSecret(
		ctx, "provider_account:openrouter:default", revision, AuthSecretInput,
		secret, profiles, s.now(),
	)
}

// StartAuth starts one OpenRouter S256 PKCE attempt.
func (s *OpenRouterService) StartAuth(
	ctx context.Context,
	providerKind string,
	_ string,
	method AuthMethod,
) (AuthAttempt, error) {
	if providerKind != "openrouter" {
		return AuthAttempt{}, ErrUnsupportedProvider
	}
	accountID := "provider_account:openrouter:default"
	if method != AuthOAuthPKCE {
		return AuthAttempt{}, ErrAuthMethodMismatch
	}
	revision, err := s.startingRevision(ctx, false)
	if errors.Is(err, ErrAccountNotFound) {
		revision, err = 0, nil
	}
	if err != nil {
		return AuthAttempt{}, err
	}
	attemptID, err := randomBase64URL(24)
	if err != nil {
		return AuthAttempt{}, ErrProviderUnavailable
	}
	verifier, err := randomBase64URL(32)
	if err != nil {
		return AuthAttempt{}, ErrProviderUnavailable
	}
	callback := *s.callbackBase
	callback.Path = strings.TrimRight(callback.Path, "/") + "/" + attemptID
	challenge := sha256.Sum256([]byte(verifier))
	authorization, _ := url.Parse(s.authorizeURL)
	query := authorization.Query()
	query.Set("callback_url", callback.String())
	query.Set("code_challenge", base64.RawURLEncoding.EncodeToString(challenge[:]))
	query.Set("code_challenge_method", "S256")
	authorization.RawQuery = query.Encode()
	view := AuthAttempt{
		ID: attemptID, ProviderKind: "openrouter", ProviderAccountID: accountID,
		Method: AuthOAuthPKCE, Status: AuthAttemptWaiting,
		VerificationURL: authorization.String(),
		Instructions:    "Complete the connection in your browser.",
	}

	s.mu.Lock()
	if prior := s.attempts[s.latestAttempt]; prior != nil && !terminalAttempt(prior.view.Status) && !prior.claimed {
		prior.view.Status = AuthAttemptCancelled
		prior.verifier = ""
		if prior.timer != nil {
			prior.timer.Stop()
		}
		s.publishLocked(prior)
	}
	attempt := &openRouterAttempt{
		view: view, verifier: verifier, expectedRevision: revision,
		subscribers: make(map[chan AuthAttempt]struct{}),
	}
	s.attempts[attemptID] = attempt
	s.latestAttempt = attemptID
	attempt.timer = time.AfterFunc(s.attemptTTL, func() { s.expire(attemptID) })
	s.mu.Unlock()
	return view, nil
}

// Attempt returns one safe authentication attempt view.
func (s *OpenRouterService) Attempt(attemptID string) (AuthAttempt, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	attempt := s.attempts[attemptID]
	if attempt == nil {
		return AuthAttempt{}, false
	}
	return attempt.view, true
}

// Cancel stops one authentication attempt when it is still current.
func (s *OpenRouterService) Cancel(attemptID string) (AuthAttempt, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	attempt := s.attempts[attemptID]
	if attempt == nil {
		return AuthAttempt{}, false
	}
	if !terminalAttempt(attempt.view.Status) && !attempt.claimed {
		attempt.view.Status = AuthAttemptCancelled
		attempt.verifier = ""
		if attempt.timer != nil {
			attempt.timer.Stop()
		}
		s.publishLocked(attempt)
	}
	return attempt.view, true
}

// Subscribe emits the current view, future changes, and then closes after a terminal state.
func (s *OpenRouterService) Subscribe(ctx context.Context, attemptID string) (<-chan AuthAttempt, error) {
	s.mu.Lock()
	attempt := s.attempts[attemptID]
	if attempt == nil {
		s.mu.Unlock()
		return nil, ErrAuthAttemptNotCurrent
	}
	events := make(chan AuthAttempt, 4)
	events <- attempt.view
	if terminalAttempt(attempt.view.Status) {
		close(events)
		s.mu.Unlock()
		return events, nil
	}
	attempt.subscribers[events] = struct{}{}
	s.mu.Unlock()
	go func() {
		<-ctx.Done()
		s.mu.Lock()
		if current := s.attempts[attemptID]; current != nil {
			if _, exists := current.subscribers[events]; exists {
				delete(current.subscribers, events)
				close(events)
			}
		}
		s.mu.Unlock()
	}()
	return events, nil
}

// CompleteCallback exchanges one code and publishes a credential exactly once.
func (s *OpenRouterService) CompleteCallback(
	ctx context.Context,
	attemptID string,
	code string,
) (AuthAttempt, error) {
	if !validAttemptID(attemptID) || code == "" || len(code) > 16*1024 {
		return AuthAttempt{}, ErrAuthAttemptNotCurrent
	}
	s.mu.Lock()
	attempt := s.attempts[attemptID]
	if attempt == nil || s.latestAttempt != attemptID || attempt.view.Status != AuthAttemptWaiting ||
		attempt.claimed || attempt.verifier == "" {
		s.mu.Unlock()
		return AuthAttempt{}, ErrAuthAttemptNotCurrent
	}
	attempt.claimed = true
	if attempt.timer != nil {
		attempt.timer.Stop()
	}
	verifier := attempt.verifier
	attempt.verifier = ""
	expectedRevision := attempt.expectedRevision
	s.mu.Unlock()

	secret, profiles, err := s.exchangeCode(ctx, code, verifier)
	if err == nil {
		_, err = s.accounts.PublishVerifiedSecret(
			ctx, "provider_account:openrouter:default", expectedRevision,
			AuthOAuthPKCE, secret, profiles, s.now(),
		)
	}
	status, errorCode, errorMessage := AuthAttemptCompleted, "", ""
	if err != nil {
		status = AuthAttemptFailed
		errorCode = "provider_auth_publication_failed"
		switch {
		case errors.Is(err, ErrAccountConflict):
			errorMessage = "Provider account credentials changed during authentication"
		case errors.Is(err, ErrCompensationFailed):
			errorMessage = "Provider credential recovery failed"
		default:
			errorMessage = "Provider credentials could not be saved"
		}
	}
	s.mu.Lock()
	attempt = s.attempts[attemptID]
	if attempt == nil || !attempt.claimed || terminalAttempt(attempt.view.Status) {
		s.mu.Unlock()
		return AuthAttempt{}, ErrAuthAttemptNotCurrent
	}
	attempt.view.Status = status
	attempt.view.ErrorCode = errorCode
	attempt.view.ErrorMessage = errorMessage
	attempt.claimed = false
	view := attempt.view
	s.publishLocked(attempt)
	s.mu.Unlock()
	return view, nil
}

func (s *OpenRouterService) startingRevision(ctx context.Context, forCreate bool) (uint64, error) {
	account, err := s.accounts.persistence.ProviderAccount(ctx, "provider_account:openrouter:default")
	if errors.Is(err, ErrAccountNotFound) {
		if forCreate {
			return 0, nil
		}
		return 0, ErrAccountNotFound
	}
	if err != nil {
		return 0, err
	}
	if account.ProviderKind != "openrouter" || account.AccountKey != "default" || !account.IsActive {
		return 0, ErrAccountConflict
	}
	if forCreate && account.Metadata.CredentialRevision() > 0 {
		return 0, ErrAccountConflict
	}
	return account.Metadata.CredentialRevision(), nil
}

func (s *OpenRouterService) verifyAPIKey(ctx context.Context, secret Secret) ([]ModelProfile, error) {
	var profiles []ModelProfile
	err := secret.Use(func(key string) error {
		var ignored json.RawMessage
		if err := s.getJSON(ctx, s.apiBase+"/key", key, &ignored); err != nil {
			return err
		}
		var response struct {
			Data []json.RawMessage `json:"data"`
		}
		if err := s.getJSON(ctx, s.apiBase+"/models/user", key, &response); err != nil {
			return err
		}
		profiles = profilesFromModels(response.Data)
		if len(profiles) == 0 {
			return ErrProviderUnavailable
		}
		return nil
	})
	return profiles, err
}

func (s *OpenRouterService) exchangeCode(
	ctx context.Context,
	code string,
	verifier string,
) (Secret, []ModelProfile, error) {
	body, err := json.Marshal(map[string]string{
		"code": code, "code_verifier": verifier, "code_challenge_method": "S256",
	})
	if err != nil {
		return Secret{}, nil, ErrProviderUnavailable
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, s.apiBase+"/auth/keys", bytes.NewReader(body))
	if err != nil {
		return Secret{}, nil, ErrProviderUnavailable
	}
	request.Header.Set("Content-Type", "application/json")
	var response struct {
		Key string `json:"key"`
	}
	if err := s.doJSON(request, &response); err != nil {
		return Secret{}, nil, err
	}
	secret, err := NewSecret(response.Key)
	if err != nil {
		return Secret{}, nil, ErrProviderUnavailable
	}
	profiles, err := s.verifyAPIKey(ctx, secret)
	if err != nil {
		return Secret{}, nil, err
	}
	return secret, profiles, nil
}

func (s *OpenRouterService) getJSON(ctx context.Context, endpoint string, key string, destination any) error {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return ErrProviderUnavailable
	}
	request.Header.Set("Authorization", "Bearer "+key)
	return s.doJSON(request, destination)
}

func (s *OpenRouterService) doJSON(request *http.Request, destination any) error {
	response, err := s.client.Do(request)
	if err != nil {
		return providerTransportError("openrouter", "request")
	}
	defer response.Body.Close()
	if response.StatusCode == http.StatusUnauthorized || response.StatusCode == http.StatusForbidden {
		_, _ = io.Copy(io.Discard, io.LimitReader(response.Body, openRouterResponseLimit))
		return ErrAuthenticationRejected
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		_, _ = io.Copy(io.Discard, io.LimitReader(response.Body, openRouterResponseLimit))
		return ErrProviderUnavailable
	}
	decoder := json.NewDecoder(io.LimitReader(response.Body, openRouterResponseLimit+1))
	if err := decoder.Decode(destination); err != nil {
		return ErrProviderUnavailable
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return ErrProviderUnavailable
	}
	return nil
}

func profilesFromModels(values []json.RawMessage) []ModelProfile {
	profiles := make([]ModelProfile, 0, len(values)+1)
	for _, value := range values {
		var fields map[string]json.RawMessage
		if json.Unmarshal(value, &fields) != nil {
			continue
		}
		id, idOK := rawString(fields["id"])
		id = strings.TrimSpace(id)
		contextLength, contextOK := rawUint64(fields["context_length"])
		var architecture map[string]json.RawMessage
		architectureOK := json.Unmarshal(fields["architecture"], &architecture) == nil
		outputs, outputsOK := rawStrings(architecture["output_modalities"])
		parameters, parametersOK := rawStrings(fields["supported_parameters"])
		if !idOK || id == "" || !contextOK || contextLength < minimumContextTokens ||
			!architectureOK || !outputsOK || !parametersOK ||
			!containsString(outputs, "text") || !containsString(parameters, "tools") ||
			!containsString(parameters, "tool_choice") {
			continue
		}
		label, _ := rawString(fields["name"])
		label = strings.TrimSpace(label)
		if label == "" {
			label = id
		}
		var reasoning map[string]json.RawMessage
		_ = json.Unmarshal(fields["reasoning"], &reasoning)
		rawEfforts, _ := rawStrings(reasoning["supported_efforts"])
		efforts := make([]string, 0, len(rawEfforts))
		for _, effort := range rawEfforts {
			if normalized := normalizeReasoningEffort(effort); normalized != "" {
				efforts = append(efforts, normalized)
			}
		}
		defaultEffort, _ := rawString(reasoning["default_effort"])
		var contextTokens *uint32
		if converted := uint32(contextLength); uint64(converted) == contextLength {
			contextTokens = &converted
		}
		profiles = append(profiles, ModelProfile{
			ID: id, Label: label, ReasoningEfforts: efforts,
			DefaultReasoningEffort: normalizeReasoningEffort(defaultEffort),
			ContextWindowTokens:    contextTokens,
		})
	}
	if len(profiles) > 0 {
		auto := false
		for _, profile := range profiles {
			auto = auto || profile.ID == "openrouter/auto"
		}
		if !auto {
			profiles = append(profiles, ModelProfile{ID: "openrouter/auto", Label: "OpenRouter Auto"})
		}
	}
	sort.Slice(profiles, func(left, right int) bool {
		if (profiles[left].ID == "openrouter/auto") != (profiles[right].ID == "openrouter/auto") {
			return profiles[left].ID == "openrouter/auto"
		}
		leftLabel, rightLabel := strings.ToLower(profiles[left].Label), strings.ToLower(profiles[right].Label)
		if leftLabel != rightLabel {
			return leftLabel < rightLabel
		}
		return profiles[left].ID < profiles[right].ID
	})
	return profiles
}

func rawString(value json.RawMessage) (string, bool) {
	var result string
	if len(value) == 0 || json.Unmarshal(value, &result) != nil {
		return "", false
	}
	return result, true
}

func rawUint64(value json.RawMessage) (uint64, bool) {
	var result uint64
	if len(value) == 0 || json.Unmarshal(value, &result) != nil {
		return 0, false
	}
	return result, true
}

func rawStrings(value json.RawMessage) ([]string, bool) {
	var raw []json.RawMessage
	if len(value) == 0 || json.Unmarshal(value, &raw) != nil {
		return nil, false
	}
	result := make([]string, 0, len(raw))
	for _, item := range raw {
		if text, ok := rawString(item); ok {
			result = append(result, text)
		}
	}
	return result, true
}

func normalizeReasoningEffort(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "none", "minimal", "low", "medium", "high", "xhigh":
		return strings.ToLower(strings.TrimSpace(value))
	case "max":
		return "xhigh"
	default:
		return ""
	}
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func (s *OpenRouterService) expire(attemptID string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	attempt := s.attempts[attemptID]
	if attempt == nil || attempt.claimed || terminalAttempt(attempt.view.Status) {
		return
	}
	attempt.verifier = ""
	attempt.view.Status = AuthAttemptExpired
	attempt.view.ErrorCode = "provider_auth_expired"
	attempt.view.ErrorMessage = "Provider authentication expired"
	s.publishLocked(attempt)
}

func (s *OpenRouterService) publishLocked(attempt *openRouterAttempt) {
	terminal := terminalAttempt(attempt.view.Status)
	for subscriber := range attempt.subscribers {
		subscriber <- attempt.view
		if terminal {
			delete(attempt.subscribers, subscriber)
			close(subscriber)
		}
	}
	if terminal {
		attemptID := attempt.view.ID
		attempt.timer = time.AfterFunc(s.attemptTTL, func() { s.retire(attemptID) })
	}
}

func (s *OpenRouterService) retire(attemptID string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	attempt := s.attempts[attemptID]
	if attempt == nil || !terminalAttempt(attempt.view.Status) {
		return
	}
	delete(s.attempts, attemptID)
	if s.latestAttempt == attemptID {
		s.latestAttempt = ""
	}
}

func terminalAttempt(status AuthAttemptStatus) bool {
	return status == AuthAttemptCompleted || status == AuthAttemptFailed ||
		status == AuthAttemptExpired || status == AuthAttemptCancelled
}

func randomBase64URL(size int) (string, error) {
	data := make([]byte, size)
	if _, err := rand.Read(data); err != nil {
		return "", fmt.Errorf("read secure randomness: %w", err)
	}
	return base64.RawURLEncoding.EncodeToString(data), nil
}

func validAttemptID(value string) bool {
	if len(value) != 32 {
		return false
	}
	for _, character := range value {
		if !(character >= 'a' && character <= 'z') && !(character >= 'A' && character <= 'Z') &&
			!(character >= '0' && character <= '9') && character != '-' && character != '_' {
			return false
		}
	}
	return true
}

func exactHTTPURL(value string) (*url.URL, error) {
	parsed, err := url.Parse(value)
	if err != nil || parsed.Host == "" || parsed.User != nil ||
		(parsed.Scheme != "http" && parsed.Scheme != "https") {
		return nil, errors.New("URL must use HTTP or HTTPS with an authority")
	}
	return parsed, nil
}

func metadataWithProfiles(source AccountMetadata, profiles []ModelProfile, now time.Time) (AccountMetadata, error) {
	encoded, err := json.Marshal(profiles)
	if err != nil {
		return nil, errors.New("encode provider model profiles")
	}
	refreshed, err := json.Marshal(now.UTC().Unix())
	if err != nil {
		return nil, errors.New("encode provider model refresh time")
	}
	metadata := cloneMetadata(source)
	metadata["profiles"] = encoded
	metadata["models_refreshed_at"] = refreshed
	return metadata, nil
}
