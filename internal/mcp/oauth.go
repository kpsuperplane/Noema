package mcp

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/url"
	"strings"
	"time"

	mcpauth "github.com/modelcontextprotocol/go-sdk/auth"
	"github.com/modelcontextprotocol/go-sdk/oauthex"
	"golang.org/x/oauth2"

	"github.com/kpsuperplane/noema/internal/store"
)

const oauthAttemptTTL = 10 * time.Minute

type oauthAttempt struct {
	id, owner, serverID string
	input               SetupInput
	definition          store.MCPDefinition
	callback            *url.URL
	authorizationURL    string
	code                chan *mcpauth.AuthorizationResult
	done                chan struct{}
	result              SetupResult
	errorCode           string
}

// OAuthAttempt is one safe client-facing attempt view.
type OAuthAttempt struct {
	ID, Status, AuthorizationURL, Error string
	Result                              *SetupResult
}

// StartOAuthCreate starts browser OAuth for one pending HTTP setup.
func (s *Service) StartOAuthCreate(ctx context.Context, owner string, input SetupInput, redirect string) (OAuthAttempt, error) {
	if err := validateSetup(input); err != nil {
		return OAuthAttempt{}, err
	}
	definition, err := definitionFromSetup(input)
	if err != nil {
		return OAuthAttempt{}, err
	}
	return s.startOAuth(ctx, owner, input, definition, "", redirect)
}

// StartOAuthReauthentication starts browser OAuth for one current HTTP server.
func (s *Service) StartOAuthReauthentication(ctx context.Context, owner, serverID, redirect string) (OAuthAttempt, error) {
	server, err := s.database.MCPServer(ctx, serverID)
	if err != nil {
		return OAuthAttempt{}, err
	}
	if server.TransportKind != "streamable_http" {
		return OAuthAttempt{}, errors.New("browser OAuth requires Streamable HTTP")
	}
	definition := store.MCPDefinition{ID: server.DefinitionID, Revision: server.DefinitionRevision,
		DisplayName: server.DisplayName, TransportKind: server.TransportKind, SafeConfig: server.SafeConfig}
	input := SetupInput{DisplayName: server.DisplayName, TransportKind: server.TransportKind,
		DefinitionID: server.DefinitionID, DefinitionRevision: server.DefinitionRevision}
	_ = jsonUnmarshalSafe(server.SafeConfig, &input)
	return s.startOAuth(ctx, owner, input, definition, serverID, redirect)
}

func (s *Service) startOAuth(ctx context.Context, owner string, input SetupInput,
	definition store.MCPDefinition, serverID, redirect string) (OAuthAttempt, error) {
	if owner != "human:local" || input.TransportKind != "streamable_http" {
		return OAuthAttempt{}, errors.New("MCP OAuth setup is invalid")
	}
	callback, err := validateOAuthRedirect(redirect)
	if err != nil {
		return OAuthAttempt{}, err
	}
	id, err := newPrefixedID("mcp_oauth:")
	if err != nil {
		return OAuthAttempt{}, err
	}
	query := callback.Query()
	query.Set("attemptId", id)
	callback.RawQuery = query.Encode()
	secretRevision, err := randomHex()
	if err != nil {
		return OAuthAttempt{}, err
	}
	input.Secrets.Revision = secretRevision
	if err := s.secrets.writeAttempt(id, input.Secrets); err != nil {
		return OAuthAttempt{}, errors.New("MCP OAuth credentials could not be staged")
	}
	now := time.Now()
	if err := s.database.CreateMCPOAuthAttempt(ctx, store.MCPOAuthAttempt{ID: id, OwnerHumanID: owner,
		ServerID: serverID, ExpiresAt: now.Add(oauthAttemptTTL), CreatedAt: now, UpdatedAt: now}); err != nil {
		_ = s.secrets.removeAttempt(id)
		return OAuthAttempt{}, err
	}
	attempt := &oauthAttempt{id: id, owner: owner, serverID: serverID, input: input,
		definition: definition, callback: callback, code: make(chan *mcpauth.AuthorizationResult, 1), done: make(chan struct{})}
	s.mu.Lock()
	if len(s.attempts) >= 64 {
		s.mu.Unlock()
		_ = s.failOAuth(id, "capacity")
		return OAuthAttempt{}, errors.New("too many MCP OAuth attempts")
	}
	s.attempts[id] = attempt
	s.mu.Unlock()
	started := make(chan string, 1)
	failed := make(chan error, 1)
	handler, err := s.oauthHandler(attempt, started)
	if err != nil {
		_ = s.failOAuth(id, "invalid_client")
		return OAuthAttempt{}, err
	}
	go s.runOAuth(attempt, handler, failed)
	select {
	case authorizationURL := <-started:
		parsed, parseErr := validateOAuthRemoteURL(authorizationURL)
		if parseErr != nil {
			_ = s.failOAuth(id, "invalid_authorization_url")
			return OAuthAttempt{}, parseErr
		}
		s.mu.Lock()
		attempt.authorizationURL = parsed.String()
		s.mu.Unlock()
		return OAuthAttempt{ID: id, Status: "waiting_for_user", AuthorizationURL: parsed.String()}, nil
	case <-ctx.Done():
		_ = s.failOAuth(id, "cancelled")
		return OAuthAttempt{}, ctx.Err()
	case <-time.After(discoveryTimeout):
		_ = s.failOAuth(id, "start_timeout")
		return OAuthAttempt{}, errors.New("MCP OAuth setup timed out")
	case err := <-failed:
		_ = s.failOAuth(id, "authorization_unavailable")
		return OAuthAttempt{}, err
	}
}

func (s *Service) oauthHandler(attempt *oauthAttempt, started chan<- string) (*mcpauth.AuthorizationCodeHandler, error) {
	config := &mcpauth.AuthorizationCodeHandlerConfig{
		RedirectURL: attempt.callback.String(), RequestRefreshToken: true, Client: oauthHTTPClient(),
		AuthorizationCodeFetcher: func(ctx context.Context, args *mcpauth.AuthorizationArgs) (*mcpauth.AuthorizationResult, error) {
			select {
			case started <- args.URL:
			default:
			}
			select {
			case result := <-attempt.code:
				return result, nil
			case <-ctx.Done():
				return nil, ctx.Err()
			}
		},
		NewTokenSource: func(ctx context.Context, cfg *oauth2.Config, token *oauth2.Token) (oauth2.TokenSource, error) {
			credentials := &OAuthCredentials{AccessToken: token.AccessToken, RefreshToken: token.RefreshToken,
				TokenType: token.TokenType, Expiry: token.Expiry, ClientID: cfg.ClientID,
				ClientSecret: cfg.ClientSecret, TokenURL: cfg.Endpoint.TokenURL, Scopes: append([]string(nil), cfg.Scopes...)}
			attempt.input.Secrets.OAuth = credentials
			return cfg.TokenSource(ctx, token), nil
		},
	}
	if client := attempt.input.Secrets.Client; client != nil {
		credentials := &oauthex.ClientCredentials{ClientID: client.ClientID}
		if client.ClientSecret != "" {
			credentials.ClientSecretAuth = &oauthex.ClientSecretAuth{ClientSecret: client.ClientSecret}
		}
		config.PreregisteredClient = credentials
	} else {
		config.DynamicClientRegistrationConfig = &mcpauth.DynamicClientRegistrationConfig{Metadata: &oauthex.ClientRegistrationMetadata{
			RedirectURIs: []string{attempt.callback.String()}, GrantTypes: []string{"authorization_code", "refresh_token"},
			ResponseTypes: []string{"code"}, ClientName: "Noema"}}
	}
	return mcpauth.NewAuthorizationCodeHandler(config)
}

func (s *Service) runOAuth(attempt *oauthAttempt, handler *mcpauth.AuthorizationCodeHandler, failed chan<- error) {
	ctx, cancel := context.WithTimeout(context.Background(), oauthAttemptTTL)
	defer cancel()
	defer close(attempt.done)
	discovery, err := Discover(ctx, Config{TransportKind: attempt.input.TransportKind,
		SafeConfig: attempt.definition.SafeConfig, Secrets: attempt.input.Secrets, OAuthHandler: handler})
	if err != nil {
		select {
		case failed <- errors.New("MCP OAuth authorization is unavailable"):
		default:
		}
		_ = s.failOAuth(attempt.id, "authorization_rejected")
		return
	}
	if attempt.input.Secrets.OAuth == nil {
		_ = s.failOAuth(attempt.id, "missing_token")
		return
	}
	secretRevision, err := randomHex()
	if err != nil {
		_ = s.failOAuth(attempt.id, "credential_persistence")
		return
	}
	attempt.input.Secrets.Revision = secretRevision
	var result SetupResult
	if attempt.serverID == "" {
		result, err = s.publishDiscovery(ctx, attempt.input, attempt.definition, false, discovery)
	} else {
		result, err = s.publishOAuthReauthentication(ctx, attempt, discovery)
	}
	if err != nil {
		_ = s.failOAuth(attempt.id, "discovery_failed")
		return
	}
	if err := s.database.FinishMCPOAuthAttempt(ctx, attempt.id, "completed", "", result.Server.ID, time.Now()); err != nil {
		_ = s.failOAuth(attempt.id, "credential_persistence")
		return
	}
	s.mu.Lock()
	attempt.result = result
	attempt.authorizationURL = ""
	completion := s.oauthComplete
	delete(s.attempts, attempt.id)
	s.mu.Unlock()
	_ = s.secrets.removeAttempt(attempt.id)
	if completion != nil {
		completion(attempt.id)
	}
}

func (s *Service) publishDiscovery(ctx context.Context, input SetupInput, definition store.MCPDefinition,
	reuse bool, discovery Discovery) (SetupResult, error) {
	serverID, err := newPrefixedID("mcp_server:")
	if err != nil {
		return SetupResult{}, err
	}
	connectionRevision, err := newPrefixedID("mcp_connection_revision:")
	if err != nil {
		return SetupResult{}, err
	}
	if err := s.secrets.writeConnection(serverID, input.Secrets); err != nil {
		return SetupResult{}, errors.New("MCP credentials could not be stored")
	}
	tools, err := storedTools(serverID, discovery.Tools)
	if err != nil {
		_ = s.secrets.removeConnection(serverID)
		return SetupResult{}, err
	}
	server, err := s.database.CommitMCPConnection(ctx, store.NewMCPConnection{Definition: definition,
		ReuseDefinition: reuse, ServerID: serverID, ConnectionLabel: input.ConnectionLabel,
		ConnectionRevision: connectionRevision, SecretRevision: input.Secrets.Revision,
		ServiceDescription: discovery.ServiceDescription, AuthStatus: "authenticated", Tools: tools}, time.Now())
	if err != nil {
		_ = s.secrets.removeConnection(serverID)
		return SetupResult{}, err
	}
	return SetupResult{Server: &server, Status: "ready_for_policy", Discovered: len(tools)}, nil
}

func (s *Service) publishOAuthReauthentication(ctx context.Context, attempt *oauthAttempt, discovery Discovery) (SetupResult, error) {
	s.credentialMu.Lock()
	defer s.credentialMu.Unlock()
	server, err := s.database.MCPServer(ctx, attempt.serverID)
	if err != nil {
		return SetupResult{}, err
	}
	current := SecretMaterial{}
	if server.SecretRevision != "" {
		current, err = s.secrets.loadConnection(server.ID)
		if err != nil {
			return SetupResult{}, err
		}
	}
	replacement, err := s.secrets.loadAttempt(attempt.id)
	if err != nil {
		return SetupResult{}, err
	}
	replacement.OAuth = attempt.input.Secrets.OAuth
	replacement.Revision = attempt.input.Secrets.Revision
	merged := mergeSecrets(current, replacement)
	if err := s.secrets.writeConnection(server.ID, merged); err != nil {
		return SetupResult{}, err
	}
	tools, err := storedTools(server.ID, discovery.Tools)
	if err != nil {
		_ = restoreConnectionSecrets(s.secrets, server.ID, current)
		return SetupResult{}, err
	}
	server, err = s.database.ReconcileMCPConnection(ctx, server.ID, server.ConnectionRevision, merged.Revision,
		"authenticated", tools, time.Now())
	if err != nil {
		_ = restoreConnectionSecrets(s.secrets, server.ID, current)
		return SetupResult{}, err
	}
	return SetupResult{Server: &server, Status: "ready_for_policy", Discovered: len(tools)}, nil
}

// CompleteOAuth validates one callback and waits for its durable result.
func (s *Service) CompleteOAuth(ctx context.Context, attemptID, rawCallback string) error {
	s.mu.Lock()
	attempt := s.attempts[attemptID]
	s.mu.Unlock()
	if attempt == nil {
		return errors.New("MCP OAuth attempt was not found")
	}
	callback, err := url.Parse(rawCallback)
	if err != nil || callback.Scheme != attempt.callback.Scheme || callback.Host != attempt.callback.Host ||
		callback.Path != attempt.callback.Path || callback.Query().Get("attemptId") != attemptID {
		return errors.New("MCP OAuth callback is invalid")
	}
	code, states := callback.Query()["code"], callback.Query()["state"]
	if len(code) != 1 || code[0] == "" || len(code[0]) > 4096 || len(states) != 1 || states[0] == "" {
		return errors.New("MCP OAuth callback is invalid")
	}
	result := &mcpauth.AuthorizationResult{Code: code[0], State: states[0], Iss: callback.Query().Get("iss")}
	select {
	case attempt.code <- result:
	case <-ctx.Done():
		return ctx.Err()
	}
	select {
	case <-attempt.done:
	case <-ctx.Done():
		return ctx.Err()
	case <-time.After(discoveryTimeout):
		return errors.New("MCP OAuth completion timed out")
	}
	view, err := s.Attempt(ctx, attemptID, attempt.owner)
	if err != nil || view.Status != "completed" {
		return errors.New("Noema could not complete MCP OAuth authorization")
	}
	return nil
}

// Attempt returns one owner-bound safe OAuth attempt.
func (s *Service) Attempt(ctx context.Context, id, owner string) (OAuthAttempt, error) {
	stored, err := s.database.MCPOAuthAttempt(ctx, id, owner, time.Now())
	if err != nil {
		return OAuthAttempt{}, err
	}
	view := OAuthAttempt{ID: id, Status: stored.Status}
	s.mu.Lock()
	if attempt := s.attempts[id]; attempt != nil {
		view.AuthorizationURL = attempt.authorizationURL
		if attempt.result.Server != nil {
			result := attempt.result
			view.Result = &result
		}
	}
	s.mu.Unlock()
	if stored.Status == "failed" {
		view.Error = oauthFailureMessage(stored.FailureCode)
	}
	if view.Result == nil && stored.ResultServerID != "" {
		server, loadErr := s.database.MCPServer(ctx, stored.ResultServerID)
		if loadErr == nil {
			view.Result = &SetupResult{Server: &server, Status: "ready_for_policy", Discovered: server.ToolCount}
		}
	}
	return view, nil
}

func (s *Service) failOAuth(id, code string) error {
	s.mu.Lock()
	if attempt := s.attempts[id]; attempt != nil {
		attempt.authorizationURL = ""
		attempt.errorCode = code
	}
	delete(s.attempts, id)
	s.mu.Unlock()
	_ = s.secrets.removeAttempt(id)
	return s.database.FinishMCPOAuthAttempt(context.Background(), id, "failed", code, "", time.Now())
}

// CancelOAuth removes one attempt that could not be bound to its caller.
func (s *Service) CancelOAuth(id string) { _ = s.failOAuth(id, "cancelled") }

// CallbackHandler completes the exact callback without logging query values.
func (s *Service) CallbackHandler() http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.Header().Set("X-Content-Type-Options", "nosniff")
		if r.Method != http.MethodGet || r.URL.Path != "/mcp/oauth/callback" || len(r.URL.RawQuery) > 16384 {
			http.Error(w, "Invalid MCP OAuth callback.", http.StatusBadRequest)
			return
		}
		id := r.URL.Query().Get("attemptId")
		if err := s.CompleteOAuth(r.Context(), id, absoluteCallback(r)); err != nil {
			http.Error(w, "Noema could not complete this MCP connection.", http.StatusBadRequest)
			return
		}
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("<!doctype html><title>Noema MCP OAuth</title><p>Authentication completed.</p><p><a href=\"/\">Return to Noema</a></p>"))
	})
}

func validateOAuthRedirect(raw string) (*url.URL, error) {
	value, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || value.User != nil || value.Path != "/mcp/oauth/callback" || value.RawQuery != "" || value.Fragment != "" {
		return nil, errors.New("MCP redirect URI is invalid")
	}
	if value.Scheme == "https" && value.Hostname() != "" {
		return value, nil
	}
	if value.Scheme == "http" && loopbackHost(context.Background(), value.Hostname()) {
		return value, nil
	}
	return nil, errors.New("MCP redirect URI must use HTTPS or loopback HTTP")
}
func validateOAuthRemoteURL(raw string) (*url.URL, error) {
	value, err := url.Parse(raw)
	if err != nil || value.User != nil || value.Hostname() == "" {
		return nil, errors.New("MCP authorization URL is invalid")
	}
	if value.Scheme == "https" || value.Scheme == "http" && loopbackHost(context.Background(), value.Hostname()) {
		return value, nil
	}
	return nil, errors.New("MCP authorization URL is invalid")
}

type oauthRoundTripper struct{}

func (oauthRoundTripper) RoundTrip(request *http.Request) (*http.Response, error) {
	client, err := mcpHTTPClient(request.Context(), request.URL.String(), nil, SecretMaterial{})
	if err != nil {
		return nil, err
	}
	return client.Transport.RoundTrip(request)
}
func oauthHTTPClient() *http.Client {
	return &http.Client{Transport: oauthRoundTripper{}, Timeout: 30 * time.Second,
		CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
}

func (s *Service) refreshOAuth(ctx context.Context, server store.MCPServer, secrets SecretMaterial) (SecretMaterial, error) {
	s.credentialMu.Lock()
	defer s.credentialMu.Unlock()
	current, err := s.database.MCPServer(ctx, server.ID)
	if err != nil {
		return SecretMaterial{}, err
	}
	if current.SecretRevision != server.SecretRevision {
		secrets, err = s.secrets.loadConnection(server.ID)
		if err != nil || secrets.Revision != current.SecretRevision {
			return SecretMaterial{}, errors.New("MCP credentials are unavailable")
		}
		server = current
	}
	value := secrets.OAuth
	if value == nil || value.RefreshToken == "" || value.Expiry.IsZero() || time.Until(value.Expiry) > time.Minute {
		return secrets, nil
	}
	if _, err := validateOAuthRemoteURL(value.TokenURL); err != nil {
		return SecretMaterial{}, err
	}
	httpClient := oauthHTTPClient()
	ctx = context.WithValue(ctx, oauth2.HTTPClient, httpClient)
	config := &oauth2.Config{ClientID: value.ClientID, ClientSecret: value.ClientSecret,
		Endpoint: oauth2.Endpoint{TokenURL: value.TokenURL}, Scopes: value.Scopes}
	token, err := config.TokenSource(ctx, &oauth2.Token{AccessToken: value.AccessToken,
		RefreshToken: value.RefreshToken, TokenType: value.TokenType, Expiry: value.Expiry}).Token()
	if err != nil {
		return SecretMaterial{}, ErrAuthenticationRequired
	}
	if token.AccessToken == value.AccessToken && token.Expiry.Equal(value.Expiry) {
		return secrets, nil
	}
	updated := secrets
	copy := *value
	copy.AccessToken, copy.RefreshToken, copy.TokenType, copy.Expiry = token.AccessToken, token.RefreshToken, token.TokenType, token.Expiry
	updated.OAuth = &copy
	revision, err := randomHex()
	if err != nil {
		return SecretMaterial{}, err
	}
	updated.Revision = revision
	if err := s.secrets.writeConnection(server.ID, updated); err != nil {
		return SecretMaterial{}, err
	}
	if err := s.database.UpdateMCPSecretRevision(ctx, server.ID, server.ConnectionRevision,
		server.SecretRevision, revision, time.Now()); err != nil {
		_ = s.secrets.writeConnection(server.ID, secrets)
		return SecretMaterial{}, err
	}
	return updated, nil
}
func absoluteCallback(r *http.Request) string {
	scheme := "http"
	if r.TLS != nil {
		scheme = "https"
	}
	return scheme + "://" + r.Host + r.URL.RequestURI()
}
func oauthFailureMessage(string) string { return "Noema could not complete MCP OAuth authorization" }
func jsonUnmarshalSafe(data json.RawMessage, input *SetupInput) error {
	return json.Unmarshal(data, safeTarget(input))
}
