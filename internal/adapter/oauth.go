package adapter

import (
	"context"
	"crypto/rand"
	"crypto/subtle"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"net"
	"net/http"
	"net/url"
	"sort"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/netpolicy"
	"github.com/kpsuperplane/noema/internal/script"
	"golang.org/x/oauth2"
)

const (
	oauthAttemptTTL = 10 * time.Minute
	oauthEventLimit = 64
)

var errOAuthRejected = errors.New("adapter OAuth grant was rejected")

type OAuthAttempt struct {
	AttemptID, AuthorizationURL string
	ExpiresAt                   int64
}
type OAuthAttemptEvent struct {
	AttemptID, SemanticDigest, GrantID, Status string
	GrantRevision                              int
}
type oauthAttempt struct {
	ID, state, verifier, redirect string
	expires                       time.Time
	ApplicationID                 string
	ApplicationRevision           int
	GrantID                       string
	GrantRevision                 int
	SemanticDigest, ProfileDigest string
	Operations                    []string
	Additional                    []OAuthServiceSelection
	Scopes                        []string
}
type OAuthServiceSelection struct {
	SemanticDigest string
	OperationIDs   []string
}
type OAuthStart struct {
	ApplicationID               string
	ExpectedApplicationRevision int
	GrantID                     string
	ExpectedGrantRevision       int
	SemanticDigest              string
	OperationIDs                []string
	Additional                  []OAuthServiceSelection
}

func (s *Service) SetOAuthCallback(raw string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	parsed, err := url.Parse(raw)
	if err != nil || parsed.Scheme != "http" && parsed.Scheme != "https" || parsed.Host == "" || parsed.User != nil || parsed.RawQuery != "" || parsed.Fragment != "" || parsed.Path != "/adapter/oauth/callback" {
		return errors.New("adapter OAuth callback is invalid")
	}
	if oauthCallbackMode(parsed) == "" {
		return errors.New("adapter OAuth callback is invalid")
	}
	s.oauthCallback = parsed.String()
	return nil
}
func (s *Service) OAuthCallbackMode() string {
	s.mu.Lock()
	defer s.mu.Unlock()
	parsed, _ := url.Parse(s.oauthCallback)
	return oauthCallbackMode(parsed)
}
func oauthCallbackMode(value *url.URL) string {
	if value == nil {
		return ""
	}
	host := strings.ToLower(value.Hostname())
	ip := net.ParseIP(host)
	if value.Scheme == "http" && (host == "localhost" || ip != nil && ip.IsLoopback()) {
		return "loopback"
	}
	if value.Scheme == "https" {
		return "hosted"
	}
	return ""
}
func (s *Service) SetOAuthCompletionHandler(handler func(OAuthAttemptEvent)) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.oauthCompleted = handler
}
func (s *Service) OAuthCallbackHandler() http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" || len(r.URL.RequestURI()) > 8<<10 {
			http.Error(w, "OAuth callback is invalid", http.StatusBadRequest)
			return
		}
		_, err := s.CompleteOAuth(r.Context(), s.oauthCallback+"?"+r.URL.RawQuery)
		if err != nil {
			http.Error(w, "Noema could not finish this connection. Return to Noema.", http.StatusBadRequest)
			return
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		_, _ = io.WriteString(w, "<!doctype html><title>Noema</title><p>Connection complete. You can return to Noema.</p>")
	})
}

func parseGoogleClient(raw []byte, callback string) (string, string, string, error) {
	if len(raw) == 0 || len(raw) > 128<<10 {
		return "", "", "", errors.New("adapter OAuth client document is invalid")
	}
	// Decode each shape explicitly because Go cannot give two fields one tag.
	var root map[string]json.RawMessage
	if decodeExactJSON(raw, &root) != nil || len(root) != 1 {
		return "", "", "", errors.New("adapter OAuth client document is invalid")
	}
	mode := ""
	var client struct {
		ClientID     string   `json:"client_id"`
		ClientSecret string   `json:"client_secret"`
		RedirectURIs []string `json:"redirect_uris"`
	}
	if value := root["installed"]; value != nil {
		mode = "loopback"
		if json.Unmarshal(value, &client) != nil {
			return "", "", "", errors.New("adapter OAuth client document is invalid")
		}
	}
	if value := root["web"]; value != nil {
		mode = "hosted"
		if json.Unmarshal(value, &client) != nil {
			return "", "", "", errors.New("adapter OAuth client document is invalid")
		}
	}
	if client.ClientID == "" || len(client.ClientID) > 16<<10 || client.ClientSecret == "" || len(client.ClientSecret) > 16<<10 {
		return "", "", "", errors.New("adapter OAuth client document is invalid")
	}
	if mode == "hosted" {
		found := false
		for _, candidate := range client.RedirectURIs {
			found = found || candidate == callback
		}
		if !found {
			return "", "", "", errors.New("adapter OAuth client redirect URI does not match")
		}
	}
	return mode, client.ClientID, client.ClientSecret, nil
}

func (s *Service) ImportOAuthApplication(profileDigest string, projectLabel *string, document []byte) (OAuthApplication, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.oauthCallback == "" {
		return OAuthApplication{}, errors.New("adapter OAuth callback is unavailable")
	}
	profile := googleOAuthProfile()
	if profileDigest != profile.ProfileDigest {
		return OAuthApplication{}, errors.New("adapter OAuth profile is unavailable")
	}
	if projectLabel != nil && !boundedText(*projectLabel, 256, false) {
		return OAuthApplication{}, errors.New("adapter OAuth project label is invalid")
	}
	mode, clientID, secret, err := parseGoogleClient(document, s.oauthCallback)
	if err != nil {
		return OAuthApplication{}, err
	}
	callback, _ := url.Parse(s.oauthCallback)
	if mode != oauthCallbackMode(callback) {
		return OAuthApplication{}, errors.New("adapter OAuth client callback mode does not match")
	}
	snapshot, err := s.files.oauthSnapshot()
	if err != nil {
		return OAuthApplication{}, err
	}
	for _, existing := range snapshot.Applications {
		if existing.ProfileDigest == profileDigest && existing.CallbackMode == mode && existing.ClientID == clientID {
			_, credential, e := s.files.loadOAuthApplication(existing.ApplicationID)
			if e != nil {
				return OAuthApplication{}, e
			}
			if subtle.ConstantTimeCompare([]byte(credential.ClientSecret), []byte(secret)) == 1 {
				return existing, nil
			}
			return OAuthApplication{}, errors.New("adapter OAuth client conflicts")
		}
	}
	generation := randomHex()
	application := OAuthApplication{SchemaVersion: 1, ApplicationID: randomHex(), ProfileDigest: profileDigest, CallbackMode: mode, ClientID: clientID, ProjectLabel: projectLabel, CredentialGeneration: generation, Revision: 1, Status: "active"}
	return s.files.installOAuthApplication(application, oauthApplicationCredential{SchemaVersion: 1, GenerationID: generation, ClientSecret: secret})
}
func (s *Service) ReplaceOAuthApplication(id string, revision int, document []byte) (OAuthApplication, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	current, _, err := s.files.loadOAuthApplication(id)
	if err != nil || current.Revision != revision {
		return OAuthApplication{}, errors.New("adapter OAuth application changed")
	}
	mode, client, secret, err := parseGoogleClient(document, s.oauthCallback)
	if err != nil || mode != current.CallbackMode || client != current.ClientID {
		return OAuthApplication{}, errors.New("adapter OAuth application document is invalid")
	}
	generation := randomHex()
	current.CredentialGeneration = generation
	current.Revision++
	err = s.files.replaceOAuthObject("adapters/oauth-applications", id, "application.json", current, "credentials", generation, oauthApplicationCredential{SchemaVersion: 1, GenerationID: generation, ClientSecret: secret})
	return current, err
}
func (s *Service) DeleteOAuthApplication(id string, revision int) (bool, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	app, _, err := s.files.loadOAuthApplication(id)
	if err != nil {
		return false, nil
	}
	if app.Revision != revision {
		return false, errors.New("adapter OAuth application changed")
	}
	snapshot, err := s.files.oauthSnapshot()
	if err != nil {
		return false, err
	}
	for _, grant := range snapshot.Grants {
		if grant.ApplicationID == id {
			return false, errors.New("adapter OAuth application is in use")
		}
	}
	if err = s.files.quarantine("oauth-applications", id); err != nil {
		return false, err
	}
	return true, nil
}

func (d Definition) ScopeTarget(operationIDs, granted []string) ([]string, bool) {
	if len(operationIDs) == 0 {
		return nil, false
	}
	grant := map[string]bool{}
	for _, v := range granted {
		grant[v] = true
	}
	result := append([]string(nil), granted...)
	seen := map[string]bool{}
	for _, id := range operationIDs {
		if seen[id] {
			return nil, false
		}
		seen[id] = true
		var operation *CompiledOperation
		for i := range d.Operations {
			if d.Operations[i].OperationID == id {
				operation = &d.Operations[i]
				break
			}
		}
		if operation == nil {
			return nil, false
		}
		best := []string(nil)
		for _, alternative := range operation.Authorization.AcceptedScopeSets {
			candidate := []string{}
			for _, scope := range alternative {
				if !grant[scope] {
					candidate = append(candidate, scope)
				}
			}
			if best == nil || len(candidate) < len(best) || len(candidate) == len(best) && strings.Join(candidate, "\x00") < strings.Join(best, "\x00") {
				best = candidate
			}
		}
		result = append(result, best...)
	}
	sort.Strings(result)
	result = uniqueStrings(result)
	total := 0
	for _, v := range result {
		total += len(v)
	}
	return result, total <= 4096
}
func uniqueStrings(values []string) []string {
	if len(values) == 0 {
		return values
	}
	out := values[:1]
	for _, v := range values[1:] {
		if v != out[len(out)-1] {
			out = append(out, v)
		}
	}
	return out
}
func operationScopesSatisfied(operation CompiledOperation, scopes []string) bool {
	for _, accepted := range operation.Authorization.AcceptedScopeSets {
		if scopeSubset(scopes, accepted) {
			return true
		}
	}
	return false
}

func currentReviewedDefinition(definitions []Definition, digest string) (Definition, bool) {
	for _, definition := range definitions {
		if definition.SemanticDigest == digest && definition.Manifest.Reviewed && !definition.Superseded {
			return definition, true
		}
	}
	return Definition{}, false
}

func (s *Service) StartOAuth(start OAuthStart) (OAuthAttempt, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.oauthCallback == "" {
		return OAuthAttempt{}, errors.New("adapter OAuth callback is unavailable")
	}
	if len(s.oauthAttempts) >= 64 {
		s.expireOAuthAttempts(time.Now())
		if len(s.oauthAttempts) >= 64 {
			return OAuthAttempt{}, errors.New("adapter OAuth setup is unavailable")
		}
	}
	application, _, err := s.files.loadOAuthApplication(start.ApplicationID)
	if err != nil || application.Revision != start.ExpectedApplicationRevision || application.Status != "active" {
		return OAuthAttempt{}, errors.New("adapter OAuth application changed")
	}
	callback, _ := url.Parse(s.oauthCallback)
	if application.CallbackMode != oauthCallbackMode(callback) {
		return OAuthAttempt{}, errors.New("adapter OAuth application callback changed")
	}
	definitions, err := s.files.definitions()
	definition, found := currentReviewedDefinition(definitions, start.SemanticDigest)
	if err != nil || !found || definition.Manifest.Authentication.ProfileDigest != application.ProfileDigest {
		return OAuthAttempt{}, errors.New("adapter OAuth definition changed")
	}
	var grant OAuthGrant
	if start.GrantID != "" {
		grant, _, err = s.files.loadOAuthGrant(start.GrantID)
		if err != nil || grant.ApplicationID != application.ApplicationID || grant.AuthorityRevision != start.ExpectedGrantRevision {
			return OAuthAttempt{}, errors.New("adapter OAuth grant changed")
		}
	}
	scopes, ok := definition.ScopeTarget(start.OperationIDs, grant.GrantedScopes)
	if !ok {
		return OAuthAttempt{}, errors.New("adapter OAuth operation selection is invalid")
	}
	selected := map[string]bool{start.SemanticDigest: true}
	for _, extra := range start.Additional {
		if selected[extra.SemanticDigest] {
			return OAuthAttempt{}, errors.New("adapter OAuth service selection is invalid")
		}
		selected[extra.SemanticDigest] = true
		other, found := currentReviewedDefinition(definitions, extra.SemanticDigest)
		if !found || other.Manifest.Authentication.ProfileDigest != application.ProfileDigest {
			return OAuthAttempt{}, errors.New("adapter OAuth service selection is invalid")
		}
		more, valid := other.ScopeTarget(extra.OperationIDs, grant.GrantedScopes)
		if !valid {
			return OAuthAttempt{}, errors.New("adapter OAuth service selection is invalid")
		}
		scopes = append(scopes, more...)
	}
	sort.Strings(scopes)
	scopes = uniqueStrings(scopes)
	total := 0
	for _, v := range scopes {
		total += len(v)
	}
	if total > 4096 {
		return OAuthAttempt{}, errors.New("adapter OAuth scopes are too large")
	}
	for _, attempt := range s.oauthAttempts {
		if start.GrantID != "" && attempt.GrantID == start.GrantID {
			delete(s.oauthAttempts, attempt.state)
			s.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: attempt.GrantID, GrantRevision: attempt.GrantRevision, Status: "superseded"})
		}
	}
	state := randomURL(32)
	verifier := randomURL(32)
	profile := googleOAuthProfile()
	config := oauth2.Config{ClientID: application.ClientID, Endpoint: oauth2.Endpoint{AuthURL: profile.AuthorizationEndpoint, TokenURL: profile.TokenEndpoint}, RedirectURL: s.oauthCallback, Scopes: scopes}
	options := []oauth2.AuthCodeOption{oauth2.S256ChallengeOption(verifier), oauth2.SetAuthURLParam("access_type", "offline"), oauth2.SetAuthURLParam("include_granted_scopes", "true")}
	if start.GrantID == "" {
		options = append(options, oauth2.SetAuthURLParam("prompt", "select_account"))
	}
	id := randomHex()
	expires := time.Now().Add(oauthAttemptTTL)
	attempt := &oauthAttempt{ID: id, state: state, verifier: verifier, redirect: s.oauthCallback, expires: expires, ApplicationID: application.ApplicationID, ApplicationRevision: application.Revision, GrantID: start.GrantID, GrantRevision: start.ExpectedGrantRevision, SemanticDigest: start.SemanticDigest, ProfileDigest: application.ProfileDigest, Operations: append([]string(nil), start.OperationIDs...), Additional: start.Additional, Scopes: scopes}
	s.oauthAttempts[state] = attempt
	s.setOAuthEvent(OAuthAttemptEvent{AttemptID: id, SemanticDigest: start.SemanticDigest, GrantID: start.GrantID, GrantRevision: start.ExpectedGrantRevision, Status: "authorizing"})
	time.AfterFunc(time.Until(expires), func() {
		s.mu.Lock()
		s.expireOAuthAttempts(time.Now())
		s.mu.Unlock()
	})
	return OAuthAttempt{AttemptID: id, AuthorizationURL: config.AuthCodeURL(state, options...), ExpiresAt: expires.Unix()}, nil
}
func randomURL(size int) string {
	raw := make([]byte, size)
	if _, err := rand.Read(raw); err != nil {
		panic(err)
	}
	return base64.RawURLEncoding.EncodeToString(raw)
}
func (s *Service) expireOAuthAttempts(now time.Time) {
	for state, a := range s.oauthAttempts {
		if !now.Before(a.expires) {
			delete(s.oauthAttempts, state)
			s.setOAuthEvent(OAuthAttemptEvent{AttemptID: a.ID, SemanticDigest: a.SemanticDigest, GrantID: a.GrantID, GrantRevision: a.GrantRevision, Status: "expired"})
		}
	}
}

func parseOAuthCallback(raw, expected string) (string, string, string, error) {
	if len(raw) > 8<<10 || strings.Contains(raw, "#") {
		return "", "", "", errors.New("invalid")
	}
	parsed, err := url.Parse(raw)
	if err != nil {
		return "", "", "", err
	}
	base := *parsed
	base.RawQuery = ""
	base.ForceQuery = false
	base.Fragment = ""
	if base.String() != expected {
		return "", "", "", errors.New("invalid")
	}
	seen := map[string]bool{}
	values := map[string]string{}
	for _, pair := range strings.Split(parsed.RawQuery, "&") {
		parts := strings.SplitN(pair, "=", 2)
		name, e := url.QueryUnescape(parts[0])
		if e != nil || seen[name] {
			return "", "", "", errors.New("invalid")
		}
		seen[name] = true
		value := ""
		if len(parts) == 2 {
			value, e = url.QueryUnescape(parts[1])
			if e != nil {
				return "", "", "", errors.New("invalid")
			}
		}
		values[name] = value
	}
	if len(values["code"]) > 16<<10 {
		return "", "", "", errors.New("invalid")
	}
	return values["state"], values["code"], values["error"], nil
}

func (s *Service) CompleteOAuth(ctx context.Context, callback string) (OAuthAttemptEvent, error) {
	s.mu.Lock()
	state, code, providerError, err := parseOAuthCallback(callback, s.oauthCallback)
	attempt := s.oauthAttempts[state]
	if attempt != nil {
		delete(s.oauthAttempts, state)
	}
	if err != nil || attempt == nil || subtle.ConstantTimeCompare([]byte(state), []byte(attempt.state)) != 1 {
		s.mu.Unlock()
		return OAuthAttemptEvent{}, errors.New("adapter OAuth callback is invalid")
	}
	if time.Now().After(attempt.expires) {
		event := OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: attempt.GrantID, GrantRevision: attempt.GrantRevision, Status: "expired"}
		s.setOAuthEvent(event)
		s.mu.Unlock()
		return event, errors.New("adapter OAuth setup expired")
	}
	if providerError != "" || code == "" {
		event := OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: attempt.GrantID, GrantRevision: attempt.GrantRevision, Status: "denied"}
		s.setOAuthEvent(event)
		s.mu.Unlock()
		return event, errors.New("adapter OAuth authorization was denied")
	}
	application, credential, e := s.files.loadOAuthApplication(attempt.ApplicationID)
	definitions, de := s.files.definitions()
	definition, found := currentReviewedDefinition(definitions, attempt.SemanticDigest)
	var current OAuthGrant
	if attempt.GrantID != "" {
		current, _, err = s.files.loadOAuthGrant(attempt.GrantID)
	}
	if e != nil || de != nil || !found || err != nil || application.Revision != attempt.ApplicationRevision || definition.Manifest.Authentication.ProfileDigest != attempt.ProfileDigest || current.AuthorityRevision != attempt.GrantRevision {
		s.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: attempt.GrantID, GrantRevision: attempt.GrantRevision, Status: "superseded"})
		s.mu.Unlock()
		return OAuthAttemptEvent{}, errors.New("adapter OAuth setup was superseded")
	}
	currentScopes, valid := definition.ScopeTarget(attempt.Operations, current.GrantedScopes)
	for _, extra := range attempt.Additional {
		other, found := currentReviewedDefinition(definitions, extra.SemanticDigest)
		if !found || other.Manifest.Authentication.ProfileDigest != attempt.ProfileDigest {
			valid = false
			break
		}
		more, ok := other.ScopeTarget(extra.OperationIDs, current.GrantedScopes)
		valid = valid && ok
		currentScopes = append(currentScopes, more...)
	}
	sort.Strings(currentScopes)
	currentScopes = uniqueStrings(currentScopes)
	if !valid || strings.Join(currentScopes, "\x00") != strings.Join(attempt.Scopes, "\x00") {
		s.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: attempt.GrantID, GrantRevision: attempt.GrantRevision, Status: "superseded"})
		s.mu.Unlock()
		return OAuthAttemptEvent{}, errors.New("adapter OAuth setup was superseded")
	}
	s.mu.Unlock()
	token, err := exchangeOAuthToken(ctx, googleOAuthProfile(), application, credential, url.Values{"grant_type": {"authorization_code"}, "code": {code}, "redirect_uri": {attempt.redirect}, "code_verifier": {attempt.verifier}}, attempt.Scopes)
	if err != nil {
		s.mu.Lock()
		s.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: attempt.GrantID, GrantRevision: attempt.GrantRevision, Status: "failed"})
		s.mu.Unlock()
		return OAuthAttemptEvent{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	application, _, e = s.files.loadOAuthApplication(attempt.ApplicationID)
	if e != nil || application.Revision != attempt.ApplicationRevision {
		s.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: attempt.GrantID, GrantRevision: attempt.GrantRevision, Status: "superseded"})
		return OAuthAttemptEvent{}, errors.New("adapter OAuth setup was superseded")
	}
	if attempt.GrantID != "" {
		var old oauthGrantToken
		current, old, e = s.files.loadOAuthGrant(attempt.GrantID)
		if e != nil || current.AuthorityRevision != attempt.GrantRevision {
			s.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: attempt.GrantID, GrantRevision: attempt.GrantRevision, Status: "superseded"})
			return OAuthAttemptEvent{}, errors.New("adapter OAuth setup was superseded")
		}
		if token.RefreshToken == "" {
			token.RefreshToken = old.RefreshToken
		}
	}
	if attempt.GrantID == "" {
		current = OAuthGrant{SchemaVersion: 1, GrantID: randomHex(), ApplicationID: application.ApplicationID, Audience: "google-apis", DesiredScopes: append([]string(nil), attempt.Scopes...), AuthorityRevision: 1, TokenRevision: 1, Status: "authentication_required"}
		if err = s.files.installOAuthOwned("adapters/oauth-grants", current.GrantID, "grant.json", current, "", "", nil); err != nil {
			s.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: attempt.GrantID, GrantRevision: attempt.GrantRevision, Status: "failed"})
			return OAuthAttemptEvent{}, err
		}
	}
	generation := randomHex()
	current.DesiredScopes = append([]string(nil), attempt.Scopes...)
	current.GrantedScopes = token.Scopes
	current.AuthorityRevision++
	current.TokenRevision++
	current.TokenGeneration = &generation
	current.Status = "active"
	token.SchemaVersion = 1
	token.GenerationID = generation
	if err = s.files.replaceOAuthObject("adapters/oauth-grants", current.GrantID, "grant.json", current, "tokens", generation, token); err != nil {
		s.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: attempt.GrantID, GrantRevision: attempt.GrantRevision, Status: "failed"})
		return OAuthAttemptEvent{}, err
	}
	if err = s.setGrantConnections(current.GrantID, "active"); err != nil {
		s.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: current.GrantID, GrantRevision: current.AuthorityRevision, Status: "failed"})
		return OAuthAttemptEvent{}, err
	}
	if err = s.reconcile(ctx); err != nil {
		s.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: current.GrantID, GrantRevision: current.AuthorityRevision, Status: "failed"})
		return OAuthAttemptEvent{}, err
	}
	event := OAuthAttemptEvent{AttemptID: attempt.ID, SemanticDigest: attempt.SemanticDigest, GrantID: current.GrantID, GrantRevision: current.AuthorityRevision, Status: "completed"}
	s.setOAuthEvent(event)
	return event, nil
}

func exchangeOAuthToken(ctx context.Context, profile OAuthProfile, application OAuthApplication, credential oauthApplicationCredential, form url.Values, expected []string) (oauthGrantToken, error) {
	form.Set("client_id", application.ClientID)
	form.Set("client_secret", credential.ClientSecret)
	checked, err := netpolicy.CheckURL(ctx, profile.TokenEndpoint)
	if err != nil {
		return oauthGrantToken{}, errors.New("adapter OAuth service is unavailable")
	}
	request, err := http.NewRequestWithContext(ctx, "POST", checked.URL.String(), strings.NewReader(form.Encode()))
	if err != nil {
		return oauthGrantToken{}, err
	}
	request.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	request.Header.Set("Accept-Encoding", "identity")
	client := netpolicy.PinnedClient(checked, 10*time.Second)
	client.Timeout = 30 * time.Second
	response, err := client.Do(request)
	if err != nil {
		return oauthGrantToken{}, errors.New("adapter OAuth service is unavailable")
	}
	defer response.Body.Close()
	headerBytes := 0
	for name, values := range response.Header {
		for _, value := range values {
			headerBytes += len(name) + len(value)
		}
	}
	if headerBytes > 64<<10 {
		return oauthGrantToken{}, errors.New("adapter OAuth response is invalid")
	}
	body, err := io.ReadAll(io.LimitReader(response.Body, (128<<10)+1))
	if err != nil || len(body) > 128<<10 {
		return oauthGrantToken{}, errors.New("adapter OAuth response is invalid")
	}
	return parseOAuthToken(response.StatusCode, body, expected, form.Get("grant_type") == "refresh_token", time.Now().Unix())
}

func parseOAuthToken(status int, body []byte, expected []string, refresh bool, now int64) (oauthGrantToken, error) {
	if status < 200 || status >= 300 {
		var failure struct {
			Error string `json:"error"`
		}
		if json.Unmarshal(body, &failure) == nil && failure.Error != "" {
			return oauthGrantToken{}, errOAuthRejected
		}
		return oauthGrantToken{}, errors.New("adapter OAuth authorization failed")
	}
	var value struct {
		AccessToken  string `json:"access_token"`
		RefreshToken string `json:"refresh_token"`
		TokenType    string `json:"token_type"`
		ExpiresIn    *int64 `json:"expires_in"`
		Scope        string `json:"scope"`
	}
	decoded, err := script.DecodeJSON(body)
	normalized, marshalErr := script.MarshalJSON(decoded)
	if err != nil || marshalErr != nil || json.Unmarshal(normalized, &value) != nil || value.AccessToken == "" || len(value.AccessToken) > 16<<10 || len(value.RefreshToken) > 16<<10 || !strings.EqualFold(value.TokenType, "Bearer") || value.ExpiresIn != nil && *value.ExpiresIn <= 0 {
		return oauthGrantToken{}, errors.New("adapter OAuth response is invalid")
	}
	scopes := strings.Fields(value.Scope)
	if value.Scope == "" {
		scopes = append([]string(nil), expected...)
	}
	sort.Strings(scopes)
	if len(uniqueStrings(append([]string(nil), scopes...))) != len(scopes) {
		return oauthGrantToken{}, errors.New("adapter OAuth response is invalid")
	}
	total := 0
	for _, scope := range scopes {
		if !boundedText(scope, 256, false) {
			return oauthGrantToken{}, errors.New("adapter OAuth response is invalid")
		}
		total += len(scope)
	}
	if total > 4096 || refresh && !scopeSubset(expected, scopes) {
		return oauthGrantToken{}, errors.New("adapter OAuth response is invalid")
	}
	expiry := int64(0)
	if value.ExpiresIn != nil {
		expiry = now + *value.ExpiresIn
	}
	return oauthGrantToken{AccessToken: value.AccessToken, RefreshToken: value.RefreshToken, ExpiresAt: expiry, Scopes: scopes}, nil
}

func (s *Service) OAuthAttempt(id string) (OAuthAttemptEvent, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.expireOAuthAttempts(time.Now())
	value, ok := s.oauthEvents[id]
	return value, ok
}
func (s *Service) setOAuthEvent(value OAuthAttemptEvent) {
	if _, exists := s.oauthEvents[value.AttemptID]; !exists && len(s.oauthEvents) >= oauthEventLimit {
		for id, event := range s.oauthEvents {
			if event.Status != "authorizing" {
				delete(s.oauthEvents, id)
				break
			}
		}
	}
	s.oauthEvents[value.AttemptID] = value
	terminal := value.Status != "authorizing"
	for subscriber := range s.oauthSubscribers[value.AttemptID] {
		select {
		case subscriber <- value:
		default:
		}
		if terminal {
			close(subscriber)
			delete(s.oauthSubscribers[value.AttemptID], subscriber)
		}
	}
	if terminal {
		delete(s.oauthSubscribers, value.AttemptID)
		if handler := s.oauthCompleted; handler != nil {
			go handler(value)
		}
	}
}
func (s *Service) SubscribeOAuth(ctx context.Context, id string) (<-chan OAuthAttemptEvent, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.expireOAuthAttempts(time.Now())
	current, ok := s.oauthEvents[id]
	if !ok {
		return nil, errors.New("adapter OAuth setup is unavailable")
	}
	channel := make(chan OAuthAttemptEvent, 2)
	channel <- current
	if current.Status != "authorizing" {
		close(channel)
		return channel, nil
	}
	if s.oauthSubscribers[id] == nil {
		s.oauthSubscribers[id] = make(map[chan OAuthAttemptEvent]struct{})
	}
	s.oauthSubscribers[id][channel] = struct{}{}
	go func() {
		<-ctx.Done()
		s.mu.Lock()
		if _, ok := s.oauthSubscribers[id][channel]; ok {
			delete(s.oauthSubscribers[id], channel)
			close(channel)
		}
		s.mu.Unlock()
	}()
	return channel, nil
}
func (s *Service) OAuthSnapshot() (OAuthSnapshot, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.files.oauthSnapshot()
}

func (s *Service) ConnectionIDsForGrant(grantID string) ([]string, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	values, err := s.files.connections()
	if err != nil {
		return nil, err
	}
	result := []string{}
	for _, value := range values {
		if value.Authentication.GrantID == grantID {
			result = append(result, value.ConnectionID)
		}
	}
	return result, nil
}

func (s *Service) AttachOAuthConnection(ctx context.Context, digest, grantID string, grantRevision int, replacement string) (Definition, Connection, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	definitions, err := s.files.definitions()
	var definition Definition
	for _, candidate := range definitions {
		if candidate.SemanticDigest == digest {
			definition = candidate
		}
	}
	family := map[string]bool{}
	if definition.Manifest.DefinitionID != "" {
		for _, candidate := range definitions {
			if candidate.Manifest.DefinitionID == definition.Manifest.DefinitionID {
				family[candidate.SemanticDigest] = true
			}
		}
	}
	grant, _, ge := s.files.loadOAuthGrant(grantID)
	application, _, ae := s.files.loadOAuthApplication(grant.ApplicationID)
	if err != nil || ge != nil || ae != nil || definition.SemanticDigest == "" || !definition.Manifest.Reviewed || definition.Superseded || grant.AuthorityRevision != grantRevision || grant.Status != "active" || definition.Manifest.Authentication.ProfileDigest != application.ProfileDigest {
		return Definition{}, Connection{}, errors.New("adapter OAuth connection is unavailable")
	}
	allowed := []string{}
	for _, op := range definition.Operations {
		for _, set := range op.Authorization.AcceptedScopeSets {
			if scopeSubset(grant.GrantedScopes, set) {
				allowed = append(allowed, op.OperationID)
				break
			}
		}
	}
	var connection Connection
	if replacement == "" {
		connections, loadErr := s.files.connections()
		if loadErr != nil {
			return Definition{}, Connection{}, loadErr
		}
		for _, candidate := range connections {
			if family[candidate.SemanticDigest] && candidate.Authentication.GrantID == grantID {
				if connection.ConnectionID != "" || candidate.SemanticDigest != digest {
					return Definition{}, Connection{}, errors.New("adapter OAuth connection conflicts")
				}
				connection = candidate
			}
		}
		if connection.ConnectionID != "" {
			return definition, connection, nil
		}
		id := randomHex()
		connection = Connection{SchemaVersion: 2, ConnectionID: id, ConnectionSlug: "personal-" + id[:8], SemanticDigest: digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: allowed, Overrides: map[string]OperationOverride{}, Authentication: ConnectionAuthentication{Kind: "oauth_grant", GrantID: grantID}}
		connection, err = s.files.installConnection(connection)
	} else {
		connection, err = s.files.loadConnection(replacement)
		if err != nil || connection.Status != "authentication_required" || connection.SemanticDigest != digest {
			return Definition{}, Connection{}, errors.New("adapter OAuth replacement changed")
		}
		connection.SemanticDigest = digest
		connection.Authentication = ConnectionAuthentication{Kind: "oauth_grant", GrantID: grantID}
		connection.Status = "active"
		connection.AllowedOperations = allowed
		connection.ConnectionRevision++
		connection, err = s.files.replaceConnection(connection)
	}
	if err == nil {
		err = s.reconcile(ctx)
	}
	return definition, connection, err
}
func (s *Service) DisconnectOAuthGrant(ctx context.Context, id string, revision int) (OAuthGrant, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	grant, _, err := s.files.loadOAuthGrant(id)
	if err != nil || grant.AuthorityRevision != revision {
		return OAuthGrant{}, errors.New("adapter OAuth grant changed")
	}
	grant.Status = "revoked"
	grant.AuthorityRevision++
	grant.TokenRevision++
	grant.TokenGeneration = nil
	err = s.files.replaceOAuthGrantWithoutToken(grant)
	if err == nil {
		err = s.setGrantConnections(id, "authentication_required")
	}
	if err == nil {
		err = s.reconcile(ctx)
	}
	return grant, err
}
func (s *Service) LabelOAuthGrant(id string, revision int, label *string) (OAuthGrant, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	grant, token, err := s.files.loadOAuthGrant(id)
	if err != nil || grant.AuthorityRevision != revision {
		return OAuthGrant{}, errors.New("adapter OAuth grant changed")
	}
	if label != nil {
		v := strings.TrimSpace(*label)
		if v == "" || len(v) > 256 {
			return OAuthGrant{}, errors.New("adapter OAuth label is invalid")
		}
		grant.AccountLabel = &v
	} else {
		grant.AccountLabel = nil
	}
	grant.AuthorityRevision++
	secretDir, secretID := "", ""
	var secret any
	if grant.TokenGeneration != nil {
		secretDir, secretID, secret = "tokens", *grant.TokenGeneration, token
	}
	err = s.files.replaceOAuthObject("adapters/oauth-grants", id, "grant.json", grant, secretDir, secretID, secret)
	return grant, err
}

func (s *Service) oauthBearer(ctx context.Context, grantID string, force bool) (OAuthGrant, oauthGrantToken, error) {
	grant, token, err := s.files.loadOAuthGrant(grantID)
	if err != nil || grant.Status != "active" || grant.TokenGeneration == nil {
		return grant, token, ErrAuthenticationRequired
	}
	if !force && (token.ExpiresAt == 0 || time.Now().Unix() < token.ExpiresAt-60) {
		return grant, token, nil
	}
	if token.RefreshToken == "" {
		return grant, token, ErrAuthenticationRequired
	}
	application, credential, err := s.files.loadOAuthApplication(grant.ApplicationID)
	if err != nil {
		return grant, token, err
	}
	fresh, err := exchangeOAuthToken(ctx, googleOAuthProfile(), application, credential, url.Values{"grant_type": {"refresh_token"}, "refresh_token": {token.RefreshToken}}, grant.DesiredScopes)
	if err != nil {
		return grant, token, err
	}
	if fresh.RefreshToken == "" {
		fresh.RefreshToken = token.RefreshToken
	}
	generation := randomHex()
	fresh.SchemaVersion = 1
	fresh.GenerationID = generation
	grant.TokenGeneration = &generation
	grant.TokenRevision++
	if err = s.files.replaceOAuthObject("adapters/oauth-grants", grantID, "grant.json", grant, "tokens", generation, fresh); err != nil {
		return grant, token, err
	}
	return grant, fresh, nil
}
func (s *Service) requireOAuthAuthentication(ctx context.Context, grant *OAuthGrant) error {
	if grant.GrantID != "" {
		grant.Status = "authentication_required"
		grant.AuthorityRevision++
		grant.TokenRevision++
		grant.TokenGeneration = nil
		if err := s.files.replaceOAuthGrantWithoutToken(*grant); err != nil {
			return err
		}
	}
	if err := s.setGrantConnections(grant.GrantID, "authentication_required"); err != nil {
		return err
	}
	return s.reconcile(ctx)
}
func (s *Service) setGrantConnections(grantID, status string) error {
	connections, err := s.files.connections()
	if err != nil {
		return err
	}
	for _, value := range connections {
		if value.Authentication.GrantID == grantID && value.Status != status {
			value.Status = status
			value.ConnectionRevision++
			if _, err = s.files.replaceConnection(value); err != nil {
				return err
			}
		}
	}
	return nil
}
func bearerHeader(token string, request *encodedRequest) error {
	if token == "" || len(token) > 16<<10 || requestHeaderExists(request.headers, "authorization") {
		return errors.New("adapter OAuth bearer is invalid")
	}
	request.headers["Authorization"] = "Bearer " + token
	return nil
}
