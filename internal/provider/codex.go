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
)

// CodexTokens contains protected OAuth authority.
type CodexTokens struct {
	accessToken  string
	refreshToken string
	lastRefresh  uint64
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

func (f codexTokenFile) tokens() CodexTokens {
	return CodexTokens{
		accessToken: strings.TrimSpace(f.AccessToken), refreshToken: strings.TrimSpace(f.RefreshToken),
		lastRefresh: f.LastRefresh,
	}
}

type codexAttempt struct {
	view             AuthAttempt
	expectedRevision uint64
	cancel           context.CancelFunc
	subscribers      map[chan AuthAttempt]struct{}
}

// CodexService owns Codex device authorization and its short-lived state.
type CodexService struct {
	accounts      *AccountService
	client        *http.Client
	issuer        string
	tokenURL      string
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
	account, err := s.accounts.LoadAccount(ctx, accountID)
	if err != nil {
		return AuthAttempt{}, err
	}
	if account.ProviderKind != "codex" || account.AccountKey != "default" || !account.IsActive ||
		account.AuthMethod != AuthOAuthDeviceCode {
		return AuthAttempt{}, ErrAccountConflict
	}
	device, err := s.requestDeviceCode(ctx)
	if err != nil {
		return AuthAttempt{}, ErrProviderUnavailable
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
		view: view, expectedRevision: account.Metadata.CredentialRevision(), cancel: cancel,
		subscribers: make(map[chan AuthAttempt]struct{}),
	}

	s.mu.Lock()
	if s.closed {
		s.mu.Unlock()
		cancel()
		return AuthAttempt{}, ErrProviderUnavailable
	}
	if prior := s.attempts[s.latestAttempt]; prior != nil && !terminalAttempt(prior.view.Status) {
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
		if terminalAttempt(attempt.view.Status) {
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
	if !terminalAttempt(attempt.view.Status) {
		attempt.cancel()
		attempt.view.Status = AuthAttemptCancelled
		s.publishLocked(attempt)
	}
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

		authorization, pending, err := s.pollDeviceAuthorization(ctx, device)
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
		tokens, err := s.exchangeAuthorizationCode(ctx, authorization)
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
	defer s.mu.Unlock()
	current := s.attempts[attempt.view.ID]
	if current != attempt || s.latestAttempt != attempt.view.ID || terminalAttempt(attempt.view.Status) {
		return
	}
	if err := ctx.Err(); err != nil {
		if errors.Is(err, context.DeadlineExceeded) {
			s.setExpiredLocked(attempt)
		}
		return
	}
	_, err := s.accounts.PublishCodexTokens(ctx, attempt.expectedRevision, tokens, s.now())
	if err == nil {
		attempt.view.Status = AuthAttemptCompleted
		s.publishLocked(attempt)
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

func (s *CodexService) requestDeviceCode(ctx context.Context) (codexDeviceCode, error) {
	var response struct {
		DeviceAuthID string          `json:"device_auth_id"`
		UserCode     string          `json:"user_code"`
		Interval     json.RawMessage `json:"interval"`
	}
	if err := s.postJSON(ctx, s.issuer+"/api/accounts/deviceauth/usercode", map[string]string{
		"client_id": codexOAuthClientID,
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
) (codexAuthorization, bool, error) {
	var response codexAuthorization
	err := s.postJSON(ctx, s.issuer+"/api/accounts/deviceauth/token", map[string]string{
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
) (CodexTokens, error) {
	values := url.Values{
		"grant_type":    {"authorization_code"},
		"code":          {authorization.AuthorizationCode},
		"redirect_uri":  {s.issuer + "/deviceauth/callback"},
		"client_id":     {codexOAuthClientID},
		"code_verifier": {authorization.CodeVerifier},
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, s.tokenURL, strings.NewReader(values.Encode()))
	if err != nil {
		return CodexTokens{}, codexRemoteError{kind: codexUnavailable}
	}
	request.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	response, err := s.client.Do(request)
	if err != nil {
		return CodexTokens{}, codexRemoteError{kind: codexNetwork}
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

var errCodexPending = errors.New("Codex authorization is pending")

type codexRemoteKind uint8

const (
	codexUnavailable codexRemoteKind = iota
	codexNetwork
	codexMalformed
	codexRateLimited
	codexRejected
)

type codexRemoteError struct{ kind codexRemoteKind }

func (e codexRemoteError) Error() string { return "Codex authorization request failed" }

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
		return codexRemoteError{kind: codexNetwork}
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
