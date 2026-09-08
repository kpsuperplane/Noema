package provider

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"net/http"
	"net/url"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"
)

const (
	codexOAuthIssuer        = "https://auth.openai.com"
	codexOAuthClientID      = "app_EMoamEEZ73f0CkXaXp7hrann"
	codexOAuthTokenURL      = "https://auth.openai.com/oauth/token"
	codexOAuthResponseLimit = 1 << 20
	codexPollFloor          = 3 * time.Second
	codexAttemptTTL         = 5 * time.Minute
	codexModelsBaseURL      = "https://chatgpt.com/backend-api/codex"
	codexVersionURL         = "https://registry.npmjs.org/@openai%2fcodex/latest"
)

// CodexTokens contains protected OAuth authority.
type CodexTokens struct {
	accessToken  string
	refreshToken string
	lastRefresh  uint64
}

// CodexDeviceAuthRequest is the safe configuration projection used when a
// device-auth operation crosses a product boundary. It contains no live
// device code or token material.
type CodexDeviceAuthRequest struct {
	ProviderAccountID string
	AccountHome       string
	Issuer            string
	ClientID          string
	TokenURL          string
	TimeoutSeconds    uint64
	AttemptTimeout    time.Duration
}

func (request CodexDeviceAuthRequest) String() string {
	return fmt.Sprintf("provider.CodexDeviceAuthRequest{provider_account_id:%q,account_home:%q,issuer:%q,client_id:%q,token_url:%q,timeout_seconds:%d,attempt_timeout:%s}",
		request.ProviderAccountID, request.AccountHome,
		sanitizeOAuthDebugURL(request.Issuer), request.ClientID,
		sanitizeOAuthDebugURL(request.TokenURL), request.TimeoutSeconds, request.AttemptTimeout)
}

func (request CodexDeviceAuthRequest) GoString() string { return request.String() }

func sanitizeOAuthDebugURL(raw string) string {
	parsed, err := url.Parse(raw)
	if err != nil {
		return raw
	}
	parsed.User = nil
	query := parsed.Query()
	for key := range query {
		switch strings.ToLower(key) {
		case "access_token", "api_key", "client_secret", "code", "code_verifier", "device_code", "id_token", "password", "refresh_token", "secret", "token", "user_code":
			query.Del(key)
		}
	}
	parsed.RawQuery = query.Encode()
	return parsed.String()
}

// Use supplies Codex tokens only to one explicit secure binding.
func (t CodexTokens) Use(binding func(string, string, uint64) error) error {
	if binding == nil || !t.valid() {
		return errors.New("Codex tokens are unavailable")
	}
	return binding(t.accessToken, t.refreshToken, t.lastRefresh)
}

// GoString prevents diagnostic formatting from exposing Codex tokens.
func (CodexTokens) GoString() string { return "provider.CodexTokens{[REDACTED]}" }

// String prevents diagnostic formatting from exposing Codex tokens.
func (CodexTokens) String() string { return "[REDACTED]" }

func (t CodexTokens) valid() bool {
	return strings.TrimSpace(t.accessToken) != "" && strings.TrimSpace(t.refreshToken) != ""
}

type codexTokenFile struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	LastRefresh  uint64 `json:"last_refresh"`
}

// String prevents a decoded token file from exposing OAuth credentials in
// diagnostics while retaining its non-secret refresh timestamp.
func (f codexTokenFile) String() string {
	return fmt.Sprintf("provider.codexTokenFile{access_token:[REDACTED],refresh_token:[REDACTED],last_refresh:%d}", f.LastRefresh)
}

// GoString prevents %#v diagnostics from exposing OAuth credentials.
func (f codexTokenFile) GoString() string { return f.String() }

func (f codexTokenFile) tokens() CodexTokens {
	return CodexTokens{
		accessToken: strings.TrimSpace(f.AccessToken), refreshToken: strings.TrimSpace(f.RefreshToken),
		lastRefresh: f.LastRefresh,
	}
}

type codexAttempt struct {
	view             AuthAttempt
	auth             CodexDeviceAuthRequest
	expectedRevision uint64
	cancel           context.CancelFunc
	subscribers      map[chan AuthAttempt]struct{}
	claimed          bool
}

// CodexService owns Codex device authorization and its short-lived state.
type CodexService struct {
	accounts      *AccountService
	client        *http.Client
	issuer        string
	tokenURL      string
	modelsBaseURL string
	versionURL    string
	attemptTTL    time.Duration
	pollFloor     time.Duration
	now           func() time.Time
	mu            sync.Mutex
	attempts      map[string]*codexAttempt
	latestAttempt string
	closed        bool
}

// NewCodexService creates the production Codex device authorization service.
func NewCodexService(accounts *AccountService) (*CodexService, error) {
	return newCodexService(
		accounts, codexOAuthIssuer, codexOAuthTokenURL,
		&http.Client{Timeout: 20 * time.Second}, codexAttemptTTL, codexPollFloor,
	)
}

// DeviceAuthRequest returns the non-secret configuration used by StartAuth.
// The returned value is safe to include in diagnostics and test reports.
func (s *CodexService) DeviceAuthRequest(accountHome string) CodexDeviceAuthRequest {
	return CodexDeviceAuthRequest{
		ProviderAccountID: "provider_account:codex:default",
		AccountHome:       accountHome,
		Issuer:            s.issuer,
		ClientID:          codexOAuthClientID,
		TokenURL:          s.tokenURL,
		TimeoutSeconds:    uint64(s.client.Timeout / time.Second),
		AttemptTimeout:    s.attemptTTL,
	}
}

func newCodexService(
	accounts *AccountService,
	issuer string,
	tokenURL string,
	client *http.Client,
	attemptTTL time.Duration,
	pollFloor time.Duration,
) (*CodexService, error) {
	if accounts == nil || client == nil || attemptTTL <= 0 || pollFloor <= 0 {
		return nil, errors.New("Codex authorization dependencies are unavailable")
	}
	issuerURL, err := exactHTTPURL(strings.TrimRight(issuer, "/"))
	if err != nil || issuerURL.RawQuery != "" || issuerURL.Fragment != "" {
		return nil, errors.New("Codex authorization issuer is invalid")
	}
	token, err := exactHTTPURL(tokenURL)
	if err != nil || token.RawQuery != "" || token.Fragment != "" {
		return nil, errors.New("Codex token URL is invalid")
	}
	return &CodexService{
		accounts: accounts, client: client, issuer: issuerURL.String(), tokenURL: token.String(),
		modelsBaseURL: codexModelsBaseURL, versionURL: codexVersionURL,
		attemptTTL: attemptTTL, pollFloor: pollFloor, now: time.Now,
		attempts: make(map[string]*codexAttempt),
	}, nil
}

// StartAuth starts one Codex device authorization attempt.
func (s *CodexService) StartAuth(
	ctx context.Context,
	providerKind string,
	accountID string,
	method AuthMethod,
) (AuthAttempt, error) {
	if providerKind != "codex" {
		return AuthAttempt{}, ErrUnsupportedProvider
	}
	if accountID != "provider_account:codex:default" || method != AuthOAuthDeviceCode {
		return AuthAttempt{}, ErrAuthMethodMismatch
	}
	s.mu.Lock()
	closed := s.closed
	s.mu.Unlock()
	if closed {
		return AuthAttempt{}, ErrProviderUnavailable
	}
	gate := s.accounts.gate(accountID)
	gate.Lock()
	account, err := s.accounts.LoadAccount(ctx, accountID)
	if errors.Is(err, ErrAccountNotFound) {
		account, err = s.accounts.persistence.CreateProviderAccount(ctx,
			builtinAccount("codex", "Codex", AuthOAuthDeviceCode, time.Now().UTC()))
	}
	gate.Unlock()
	if err != nil {
		return AuthAttempt{}, err
	}
	if account.ProviderKind != "codex" || account.AccountKey != "default" || !account.IsActive ||
		account.AuthMethod != AuthOAuthDeviceCode {
		return AuthAttempt{}, ErrAccountConflict
	}
	tokenPath, pathErr := s.accounts.codexTokenPath(account)
	if pathErr != nil {
		return AuthAttempt{}, pathErr
	}
	authRequest := s.DeviceAuthRequest(filepath.Dir(tokenPath))
	device, err := s.requestDeviceCode(ctx, authRequest)
	if err != nil {
		return AuthAttempt{}, codexAvailabilityError(err)
	}
	attemptID, err := randomBase64URL(24)
	if err != nil {
		return AuthAttempt{}, ErrProviderUnavailable
	}
	view := AuthAttempt{
		ID: attemptID, ProviderKind: "codex", ProviderAccountID: accountID,
		Method: AuthOAuthDeviceCode, Status: AuthAttemptWaiting,
		VerificationURL: s.issuer + "/codex/device", UserCode: device.UserCode,
		Instructions: "Complete the login in your browser.",
	}
	pollContext, cancel := context.WithTimeout(context.Background(), s.attemptTTL)
	attempt := &codexAttempt{
		view: view, auth: authRequest, expectedRevision: account.Metadata.CredentialRevision(), cancel: cancel,
		subscribers: make(map[chan AuthAttempt]struct{}),
	}

	s.mu.Lock()
	if s.closed {
		s.mu.Unlock()
		cancel()
		return AuthAttempt{}, ErrProviderUnavailable
	}
	if prior := s.attempts[s.latestAttempt]; prior != nil && !terminalAttempt(prior.view.Status) && !prior.claimed {
		prior.cancel()
		prior.view.Status = AuthAttemptCancelled
		s.publishLocked(prior)
	}
	s.attempts[attemptID] = attempt
	s.latestAttempt = attemptID
	s.mu.Unlock()

	go s.runAttempt(pollContext, attempt, device)
	return view, nil
}

// Close cancels all active Codex authorization work.
func (s *CodexService) Close() {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return
	}
	s.closed = true
	for _, attempt := range s.attempts {
		if terminalAttempt(attempt.view.Status) || attempt.claimed {
			continue
		}
		attempt.view.Status = AuthAttemptCancelled
		s.publishLocked(attempt)
	}
}

// Attempt returns one safe Codex authorization view.
func (s *CodexService) Attempt(attemptID string) (AuthAttempt, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	attempt := s.attempts[attemptID]
	if attempt == nil {
		return AuthAttempt{}, false
	}
	return attempt.view, true
}

// Cancel stops one waiting Codex authorization attempt.
func (s *CodexService) Cancel(attemptID string) (AuthAttempt, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	attempt := s.attempts[attemptID]
	if attempt == nil {
		return AuthAttempt{}, false
	}
	if !terminalAttempt(attempt.view.Status) && !attempt.claimed {
		attempt.cancel()
		attempt.view.Status = AuthAttemptCancelled
		s.publishLocked(attempt)
	}
	return attempt.view, true
}

// claimAttemptCompletion reserves the terminal transition for the completion
// worker. Cancellation and shutdown leave a claimed attempt in its current
// state until the worker publishes its result.
func (s *CodexService) claimAttemptCompletion(attemptID string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	attempt := s.attempts[attemptID]
	if attempt == nil || terminalAttempt(attempt.view.Status) || attempt.claimed {
		return false
	}
	attempt.claimed = true
	return true
}

func (s *CodexService) finishClaimedAttempt(attemptID string, status AuthAttemptStatus, code, message string) (AuthAttempt, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	attempt := s.attempts[attemptID]
	if attempt == nil || !attempt.claimed || terminalAttempt(attempt.view.Status) {
		return AuthAttempt{}, false
	}
	attempt.view.Status = status
	attempt.view.ErrorCode = code
	attempt.view.ErrorMessage = message
	s.publishLocked(attempt)
	return attempt.view, true
}

// Subscribe emits the current view and closes after a terminal state.
func (s *CodexService) Subscribe(ctx context.Context, attemptID string) (<-chan AuthAttempt, error) {
	s.mu.Lock()
	attempt := s.attempts[attemptID]
	if attempt == nil {
		s.mu.Unlock()
		return nil, ErrAuthAttemptNotCurrent
	}
	events := make(chan AuthAttempt, 2)
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

func (s *CodexService) runAttempt(ctx context.Context, attempt *codexAttempt, device codexDeviceCode) {
	timer := time.NewTimer(device.pollInterval(s.pollFloor))
	defer timer.Stop()
	for {
		select {
		case <-ctx.Done():
			if errors.Is(ctx.Err(), context.DeadlineExceeded) {
				s.finishExpired(attempt)
			}
			return
		case <-timer.C:
		}

		authorization, pending, err := s.pollDeviceAuthorization(ctx, device, attempt.auth)
		if err != nil {
			if errors.Is(ctx.Err(), context.Canceled) {
				return
			}
			if errors.Is(ctx.Err(), context.DeadlineExceeded) {
				s.finishExpired(attempt)
				return
			}
			s.finishFailed(attempt, "codex_device_auth_failed", safeCodexError(err))
			return
		}
		if pending {
			timer.Reset(device.pollInterval(s.pollFloor))
			continue
		}
		if strings.TrimSpace(authorization.AuthorizationCode) == "" ||
			strings.TrimSpace(authorization.CodeVerifier) == "" {
			s.finishFailed(attempt, "codex_device_auth_failed", "Codex authorization response was incomplete")
			return
		}
		tokens, err := s.exchangeAuthorizationCode(ctx, authorization, attempt.auth)
		if err != nil {
			if errors.Is(ctx.Err(), context.Canceled) {
				return
			}
			if errors.Is(ctx.Err(), context.DeadlineExceeded) {
				s.finishExpired(attempt)
				return
			}
			s.finishFailed(attempt, "codex_device_auth_failed", safeCodexError(err))
			return
		}
		s.publishTokens(ctx, attempt, tokens)
		return
	}
}

func (s *CodexService) publishTokens(ctx context.Context, attempt *codexAttempt, tokens CodexTokens) {
	s.mu.Lock()
	current := s.attempts[attempt.view.ID]
	if current != attempt || s.latestAttempt != attempt.view.ID || terminalAttempt(attempt.view.Status) {
		s.mu.Unlock()
		return
	}
	if err := ctx.Err(); err != nil {
		if errors.Is(err, context.DeadlineExceeded) {
			s.setExpiredLocked(attempt)
		}
		s.mu.Unlock()
		return
	}
	s.mu.Unlock()

	catalog, err := s.fetchModelCatalog(ctx, tokens)
	s.mu.Lock()
	defer s.mu.Unlock()
	current = s.attempts[attempt.view.ID]
	if current != attempt || s.latestAttempt != attempt.view.ID || terminalAttempt(attempt.view.Status) {
		return
	}
	if errors.Is(ctx.Err(), context.DeadlineExceeded) {
		s.setExpiredLocked(attempt)
		return
	}
	if err != nil {
		attempt.view.Status = AuthAttemptFailed
		attempt.view.ErrorCode = "provider_model_catalog_failed"
		attempt.view.ErrorMessage = safeCodexCatalogError(err)
		s.publishLocked(attempt)
		return
	}
	_, err = s.accounts.publishCodexTokens(
		ctx, attempt.expectedRevision, tokens, catalog, s.now(),
	)
	if err == nil {
		attempt.view.Status = AuthAttemptCompleted
		s.publishLocked(attempt)
		return
	}
	if !errors.Is(err, ErrCompensationFailed) && errors.Is(ctx.Err(), context.DeadlineExceeded) {
		s.setExpiredLocked(attempt)
		return
	}
	attempt.view.Status = AuthAttemptFailed
	attempt.view.ErrorCode = "provider_auth_publication_failed"
	switch {
	case errors.Is(err, ErrAccountConflict):
		attempt.view.ErrorMessage = "Provider account credentials changed during authentication"
	case errors.Is(err, ErrCompensationFailed):
		attempt.view.ErrorMessage = "Provider credential recovery failed"
	default:
		attempt.view.ErrorMessage = "Provider credentials could not be saved"
	}
	s.publishLocked(attempt)
}

func safeCodexCatalogError(err error) string {
	var remote codexRemoteError
	if !errors.As(err, &remote) {
		return "Codex model catalog is unavailable"
	}
	switch remote.kind {
	case codexNetwork:
		return "Codex model catalog network request failed"
	case codexMalformed:
		return "Codex returned an invalid model catalog"
	case codexRejected:
		return "Codex model access was rejected"
	default:
		return "Codex model catalog is unavailable"
	}
}

func (s *CodexService) finishExpired(attempt *codexAttempt) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if current := s.attempts[attempt.view.ID]; current == attempt && !terminalAttempt(attempt.view.Status) {
		s.setExpiredLocked(attempt)
	}
}

func (s *CodexService) setExpiredLocked(attempt *codexAttempt) {
	attempt.view.Status = AuthAttemptExpired
	attempt.view.ErrorCode = "provider_auth_expired"
	attempt.view.ErrorMessage = "Provider authentication expired"
	s.publishLocked(attempt)
}

func (s *CodexService) finishFailed(attempt *codexAttempt, code string, message string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if current := s.attempts[attempt.view.ID]; current != attempt || terminalAttempt(attempt.view.Status) {
		return
	}
	attempt.view.Status = AuthAttemptFailed
	attempt.view.ErrorCode = code
	attempt.view.ErrorMessage = message
	s.publishLocked(attempt)
}

func (s *CodexService) publishLocked(attempt *codexAttempt) {
	terminal := terminalAttempt(attempt.view.Status)
	for subscriber := range attempt.subscribers {
		subscriber <- attempt.view
		if terminal {
			delete(attempt.subscribers, subscriber)
			close(subscriber)
		}
	}
	if terminal {
		attempt.cancel()
		attemptID := attempt.view.ID
		time.AfterFunc(s.attemptTTL, func() { s.retire(attemptID) })
	}
}

func (s *CodexService) retire(attemptID string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if attempt := s.attempts[attemptID]; attempt != nil && terminalAttempt(attempt.view.Status) {
		delete(s.attempts, attemptID)
		if s.latestAttempt == attemptID {
			s.latestAttempt = ""
		}
	}
}

type codexDeviceCode struct {
	DeviceAuthID string
	UserCode     string
	Interval     uint64
	HasInterval  bool
}

// String prevents a device authorization response from exposing its secrets
// while retaining the ordinary polling configuration.
func (d codexDeviceCode) String() string {
	return fmt.Sprintf("provider.codexDeviceCode{device_auth_id:[REDACTED],user_code:[REDACTED],interval:%d,has_interval:%t}", d.Interval, d.HasInterval)
}

// GoString prevents %#v diagnostics from exposing device authorization data.
func (d codexDeviceCode) GoString() string { return d.String() }

func (d codexDeviceCode) pollInterval(floor time.Duration) time.Duration {
	requested := time.Duration(d.Interval) * time.Second
	if !d.HasInterval || requested < floor {
		return floor
	}
	return requested
}

type codexAuthorization struct {
	AuthorizationCode string `json:"authorization_code"`
	CodeVerifier      string `json:"code_verifier"`
}

// String prevents a token exchange response from exposing its secrets.
func (a codexAuthorization) String() string {
	return "provider.codexAuthorization{authorization_code:[REDACTED],code_verifier:[REDACTED]}"
}

// GoString prevents %#v diagnostics from exposing token exchange data.
func (a codexAuthorization) GoString() string { return a.String() }

func (s *CodexService) requestDeviceCode(ctx context.Context, auth CodexDeviceAuthRequest) (codexDeviceCode, error) {
	var response struct {
		DeviceAuthID string          `json:"device_auth_id"`
		UserCode     string          `json:"user_code"`
		Interval     json.RawMessage `json:"interval"`
	}
	if err := s.postJSON(ctx, auth.Issuer+"/api/accounts/deviceauth/usercode", map[string]string{
		"client_id": auth.ClientID,
	}, &response, false); err != nil {
		return codexDeviceCode{}, err
	}
	interval, hasInterval, err := parseCodexInterval(response.Interval)
	if err != nil || strings.TrimSpace(response.DeviceAuthID) == "" || strings.TrimSpace(response.UserCode) == "" {
		return codexDeviceCode{}, codexRemoteError{kind: codexMalformed}
	}
	return codexDeviceCode{
		DeviceAuthID: response.DeviceAuthID, UserCode: response.UserCode,
		Interval: interval, HasInterval: hasInterval,
	}, nil
}

func (s *CodexService) pollDeviceAuthorization(
	ctx context.Context,
	device codexDeviceCode,
	auth CodexDeviceAuthRequest,
) (codexAuthorization, bool, error) {
	var response codexAuthorization
	err := s.postJSON(ctx, auth.Issuer+"/api/accounts/deviceauth/token", map[string]string{
		"device_auth_id": device.DeviceAuthID, "user_code": device.UserCode,
	}, &response, true)
	if errors.Is(err, errCodexPending) {
		return codexAuthorization{}, true, nil
	}
	return response, false, err
}

func (s *CodexService) exchangeAuthorizationCode(
	ctx context.Context,
	authorization codexAuthorization,
	auth CodexDeviceAuthRequest,
) (CodexTokens, error) {
	values := url.Values{
		"grant_type":    {"authorization_code"},
		"code":          {authorization.AuthorizationCode},
		"redirect_uri":  {auth.Issuer + "/deviceauth/callback"},
		"client_id":     {auth.ClientID},
		"code_verifier": {authorization.CodeVerifier},
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, auth.TokenURL, strings.NewReader(values.Encode()))
	if err != nil {
		return CodexTokens{}, codexRemoteError{kind: codexUnavailable}
	}
	request.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	response, err := s.client.Do(request)
	if err != nil {
		return CodexTokens{}, codexNetworkError("exchange_token")
	}
	defer response.Body.Close()
	data, err := readCodexResponse(response.Body)
	if err != nil {
		return CodexTokens{}, codexRemoteError{kind: codexMalformed}
	}
	if response.StatusCode == http.StatusTooManyRequests {
		return CodexTokens{}, codexRemoteError{kind: codexRateLimited}
	}
	if response.StatusCode == http.StatusUnauthorized || response.StatusCode == http.StatusForbidden {
		return CodexTokens{}, codexRemoteError{kind: codexRejected}
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return CodexTokens{}, codexRemoteError{kind: codexUnavailable}
	}
	var file codexTokenFile
	if err := decodeCodexRemoteJSON(data, &file); err != nil {
		return CodexTokens{}, codexRemoteError{kind: codexMalformed}
	}
	file.LastRefresh = uint64(s.now().Unix())
	tokens := file.tokens()
	if !tokens.valid() {
		return CodexTokens{}, codexRemoteError{kind: codexMalformed}
	}
	return tokens, nil
}

// completeDeviceAuthorization returns protected tokens before account
// publication. The account service owns persistence for the normal flow.
func (s *CodexService) completeDeviceAuthorization(ctx context.Context, device codexDeviceCode) (CodexTokens, error) {
	auth := s.DeviceAuthRequest("")
	authorization, pending, err := s.pollDeviceAuthorization(ctx, device, auth)
	if err != nil {
		return CodexTokens{}, err
	}
	if pending {
		return CodexTokens{}, errCodexPending
	}
	if strings.TrimSpace(authorization.AuthorizationCode) == "" || strings.TrimSpace(authorization.CodeVerifier) == "" {
		return CodexTokens{}, codexRemoteError{kind: codexMalformed}
	}
	return s.exchangeAuthorizationCode(ctx, authorization, auth)
}

var errCodexPending = errors.New("Codex authorization is pending")

type codexRemoteKind uint8

const (
	codexUnavailable codexRemoteKind = iota
	codexNetwork
	codexMalformed
	codexRateLimited
	codexRejected
)

type codexRemoteError struct {
	kind  codexRemoteKind
	cause error
}

func (e codexRemoteError) Error() string { return "Codex authorization request failed" }

func (e codexRemoteError) Unwrap() error { return e.cause }

func codexNetworkError(operation string) codexRemoteError {
	return codexRemoteError{kind: codexNetwork, cause: providerTransportError("codex", operation)}
}

func safeCodexError(err error) string {
	var remote codexRemoteError
	if !errors.As(err, &remote) {
		return "Codex login is currently unavailable"
	}
	switch remote.kind {
	case codexNetwork:
		return "Codex auth network request failed"
	case codexMalformed:
		return "Codex returned an invalid login response"
	case codexRateLimited:
		return "Codex login is temporarily rate-limited"
	case codexRejected:
		return "Codex rejected the login request"
	default:
		return "Codex login request failed"
	}
}

func codexAvailabilityError(err error) error {
	var remote codexRemoteError
	if !errors.As(err, &remote) {
		return fmt.Errorf("%w: Codex authorization request failed", ErrProviderUnavailable)
	}
	if remote.cause == nil {
		remote.cause = ErrProviderUnavailable
	}
	return remote
}

func (s *CodexService) postJSON(
	ctx context.Context,
	endpoint string,
	body any,
	destination any,
	pendingAllowed bool,
) error {
	encoded, err := json.Marshal(body)
	if err != nil {
		return codexRemoteError{kind: codexUnavailable}
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint, bytes.NewReader(encoded))
	if err != nil {
		return codexRemoteError{kind: codexUnavailable}
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := s.client.Do(request)
	if err != nil {
		return codexNetworkError("device_auth")
	}
	defer response.Body.Close()
	data, err := readCodexResponse(response.Body)
	if err != nil {
		return codexRemoteError{kind: codexMalformed}
	}
	if pendingAllowed && (response.StatusCode == http.StatusForbidden || response.StatusCode == http.StatusNotFound) {
		return errCodexPending
	}
	if response.StatusCode == http.StatusTooManyRequests {
		return codexRemoteError{kind: codexRateLimited}
	}
	if response.StatusCode == http.StatusUnauthorized || response.StatusCode == http.StatusForbidden {
		return codexRemoteError{kind: codexRejected}
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return codexRemoteError{kind: codexUnavailable}
	}
	if err := decodeCodexRemoteJSON(data, destination); err != nil {
		return codexRemoteError{kind: codexMalformed}
	}
	return nil
}

func readCodexResponse(reader io.Reader) ([]byte, error) {
	data, err := io.ReadAll(io.LimitReader(reader, codexOAuthResponseLimit+1))
	if err != nil || len(data) > codexOAuthResponseLimit {
		return nil, errors.New("Codex response exceeds the limit")
	}
	return data, nil
}

func decodeCodexRemoteJSON(data []byte, destination any) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := decoder.Decode(destination); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return errors.New("Codex response has trailing data")
	}
	return nil
}

func parseCodexInterval(raw json.RawMessage) (uint64, bool, error) {
	raw = bytes.TrimSpace(raw)
	if len(raw) == 0 || bytes.Equal(raw, []byte("null")) {
		return 0, false, nil
	}
	var number uint64
	if raw[0] == '"' {
		var value string
		if json.Unmarshal(raw, &value) != nil {
			return 0, false, errors.New("invalid Codex polling interval")
		}
		parsed, err := strconv.ParseUint(value, 10, 64)
		if err != nil {
			return 0, false, errors.New("invalid Codex polling interval")
		}
		number = parsed
	} else if json.Unmarshal(raw, &number) != nil {
		return 0, false, errors.New("invalid Codex polling interval")
	}
	if number > uint64(math.MaxInt64/int64(time.Second)) {
		return 0, false, errors.New("invalid Codex polling interval")
	}
	return number, true, nil
}

func (a AuthAttempt) GoString() string {
	copy := a
	if copy.UserCode != "" {
		copy.UserCode = "[REDACTED]"
	}
	return fmt.Sprintf("provider.AuthAttempt{%q, %q, %q, %q, %q, %q, %q}",
		copy.ID, copy.ProviderKind, copy.ProviderAccountID, copy.Method, copy.Status,
		copy.VerificationURL, copy.UserCode)
}

// String prevents diagnostic formatting from exposing device user codes.
func (a AuthAttempt) String() string { return a.GoString() }
