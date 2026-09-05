package adapter

import (
	"encoding/json"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/store"
)

func oauthManifest() Manifest {
	value := testManifest()
	value.Authentication = Authentication{Kind: "oauth2_authorization_code_pkce", ProfileDigest: googleOAuthProfile().ProfileDigest}
	value.Operations[0].Authorization = Authorization{Kind: "oauth_scopes", AcceptedScopeSets: [][]string{{"scope.read"}, {"scope.alt", "scope.write"}}}
	return value
}

func newOAuthService(t *testing.T, callback string) (*Service, string) {
	t.Helper()
	directory := t.TempDir()
	root, err := os.OpenRoot(directory)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	database, err := store.Open(t.Context(), filepath.Join(directory, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	service, err := NewService(root, database)
	if err != nil {
		t.Fatal(err)
	}
	if err = service.SetOAuthCallback(callback); err != nil {
		t.Fatal(err)
	}
	return service, directory
}

func TestOAuthManifestScopesAndProfileDigest(t *testing.T) {
	profile := googleOAuthProfile()
	if !validDigest(profile.ProfileDigest) {
		t.Fatalf("profile digest = %q", profile.ProfileDigest)
	}
	definition, err := Compile(oauthManifest())
	if err != nil {
		t.Fatal(err)
	}
	target, ok := definition.ScopeTarget([]string{"lookup"}, []string{"scope.old"})
	if !ok || strings.Join(target, ",") != "scope.old,scope.read" {
		t.Fatalf("scope target = %#v, %t", target, ok)
	}
	bad := oauthManifest()
	bad.Operations[0].Authorization.AcceptedScopeSets = [][]string{{"scope.read"}, {"scope.read", "scope.write"}}
	if _, err = Compile(bad); err == nil {
		t.Fatal("subset-related scope alternatives compiled")
	}
	bad = oauthManifest()
	bad.Authentication.ProfileDigest = "wrong"
	if _, err = Compile(bad); err == nil {
		t.Fatal("invalid profile digest compiled")
	}
}

func TestOAuthApplicationFilesAreIdempotentAndProtected(t *testing.T) {
	callback := "https://noema.example/adapter/oauth/callback"
	service, directory := newOAuthService(t, callback)
	document := []byte(`{"web":{"client_id":"ordinary-client","client_secret":"secret-marker","redirect_uris":["https://noema.example/adapter/oauth/callback"],"auth_uri":"https://accounts.google.com/o/oauth2/auth"}}`)
	profile := googleOAuthProfile()
	application, err := service.ImportOAuthApplication(profile.ProfileDigest, nil, document)
	if err != nil {
		t.Fatal(err)
	}
	same, err := service.ImportOAuthApplication(profile.ProfileDigest, nil, document)
	if err != nil || same.ApplicationID != application.ApplicationID {
		t.Fatalf("idempotent import = %#v, %v", same, err)
	}
	descriptor, err := os.ReadFile(filepath.Join(directory, "adapters", "oauth-applications", application.ApplicationID, "application.json"))
	if err != nil || strings.Contains(string(descriptor), "secret-marker") {
		t.Fatalf("public descriptor contains secret: %v", err)
	}
	secretPath := filepath.Join(directory, "adapters", "oauth-applications", application.ApplicationID, "credentials", application.CredentialGeneration+".json")
	info, err := os.Stat(secretPath)
	if err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("credential mode = %v, %v", info, err)
	}
	replacement := []byte(strings.ReplaceAll(string(document), "secret-marker", "replacement-marker"))
	application, err = service.ReplaceOAuthApplication(application.ApplicationID, application.Revision, replacement)
	if err != nil {
		t.Fatal(err)
	}
	entries, err := os.ReadDir(filepath.Dir(secretPath))
	if err != nil || len(entries) != 1 || entries[0].Name() != application.CredentialGeneration+".json" {
		t.Fatalf("credential generations = %#v, %v", entries, err)
	}
	if _, err = service.ImportOAuthApplication(profile.ProfileDigest, nil, document); err == nil {
		t.Fatal("conflicting client secret was accepted")
	}
}

func TestOAuthPKCEAttemptIsBoundedAndConsumesCallbackState(t *testing.T) {
	callback := "http://127.0.0.1:3737/adapter/oauth/callback"
	service, _ := newOAuthService(t, callback)
	profile := googleOAuthProfile()
	document := []byte(`{"installed":{"client_id":"desktop-client","client_secret":"desktop-secret","redirect_uris":["http://localhost"]}}`)
	application, err := service.ImportOAuthApplication(profile.ProfileDigest, nil, document)
	if err != nil {
		t.Fatal(err)
	}
	manifest := oauthManifest()
	manifest.Reviewed = true
	definition, err := service.files.installDefinition(manifest, "https://example.com/docs", nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	attempt, err := service.StartOAuth(OAuthStart{ApplicationID: application.ApplicationID, ExpectedApplicationRevision: 1, SemanticDigest: definition.SemanticDigest, OperationIDs: []string{"lookup"}})
	if err != nil {
		t.Fatal(err)
	}
	handoff, err := url.Parse(attempt.AuthorizationURL)
	if err != nil {
		t.Fatal(err)
	}
	query := handoff.Query()
	if query.Get("code_challenge_method") != "S256" || query.Get("prompt") != "select_account" || query.Get("access_type") != "offline" || query.Get("state") == "" {
		t.Fatalf("authorization query = %v", query)
	}
	if remaining := time.Until(time.Unix(attempt.ExpiresAt, 0)); remaining < 9*time.Minute || remaining > oauthAttemptTTL {
		t.Fatalf("attempt lifetime = %s", remaining)
	}
	events, err := service.SubscribeOAuth(t.Context(), attempt.AttemptID)
	if err != nil || (<-events).Status != "authorizing" {
		t.Fatalf("initial attempt event = %v", err)
	}
	service.mu.Lock()
	service.setOAuthEvent(OAuthAttemptEvent{AttemptID: attempt.AttemptID, Status: "denied"})
	service.mu.Unlock()
	if terminal, open := <-events; !open || terminal.Status != "denied" {
		t.Fatalf("terminal attempt event = %#v, %t", terminal, open)
	}
	if _, _, _, err = parseOAuthCallback(callback+"?state=a&state=b&code=c", callback); err == nil {
		t.Fatal("duplicate callback parameter was accepted")
	}
	state := query.Get("state")
	service.mu.Lock()
	delete(service.oauthAttempts, state)
	service.mu.Unlock()
	if _, err = service.CompleteOAuth(t.Context(), callback+"?state="+url.QueryEscape(state)+"&code=ordinary-code"); err == nil {
		t.Fatal("consumed attempt was accepted")
	}
	terminalEvents := make(chan OAuthAttemptEvent, 1)
	service.SetOAuthCompletionHandler(func(event OAuthAttemptEvent) { terminalEvents <- event })
	service.mu.Lock()
	service.oauthAttempts["expired-state"] = &oauthAttempt{ID: "expired-attempt", state: "expired-state", expires: time.Now().Add(-time.Second)}
	service.expireOAuthAttempts(time.Now())
	service.mu.Unlock()
	if terminal := <-terminalEvents; terminal.AttemptID != "expired-attempt" || terminal.Status != "expired" {
		t.Fatalf("expired event = %#v", terminal)
	}
	service.SetOAuthCompletionHandler(nil)
	service.mu.Lock()
	for index := 0; index <= oauthEventLimit; index++ {
		service.setOAuthEvent(OAuthAttemptEvent{AttemptID: randomHex(), Status: "failed"})
	}
	if len(service.oauthEvents) != oauthEventLimit {
		t.Fatalf("terminal event count = %d", len(service.oauthEvents))
	}
	service.mu.Unlock()
}

func TestOAuthTokenResponseAndGrantLifecycle(t *testing.T) {
	token, err := parseOAuthToken(200, []byte(`{"access_token":"access-secret","refresh_token":"refresh-secret","token_type":"Bearer","expires_in":3600}`), []string{"scope.read"}, false, 100)
	if err != nil || token.ExpiresAt != 3700 || strings.Join(token.Scopes, ",") != "scope.read" {
		t.Fatalf("token = %#v, %v", token, err)
	}
	if _, err = parseOAuthToken(400, []byte(`{"error":"invalid_grant"}`), nil, false, 100); err != errOAuthRejected {
		t.Fatalf("invalid grant = %v", err)
	}
	for _, invalid := range []string{
		`{"access_token":"one","access_token":"two","token_type":"Bearer"}`,
		`{"access_token":"access","token_type":"Bearer","scope":"scope.read scope.read"}`,
		`{"access_token":"access","token_type":"Bearer","expires_in":0}`,
		`{"access_token":"access","token_type":"MAC"}`,
	} {
		if _, err = parseOAuthToken(200, []byte(invalid), []string{"scope.read"}, false, 100); err == nil {
			t.Fatalf("invalid token response was accepted: %s", invalid)
		}
	}
	if _, err = parseOAuthToken(200, []byte(`{"access_token":"access","token_type":"Bearer","scope":"scope.other"}`), []string{"scope.read"}, true, 100); err == nil {
		t.Fatal("refresh scope expansion was accepted")
	}
	service, directory := newOAuthService(t, "http://127.0.0.1:3737/adapter/oauth/callback")
	profile := googleOAuthProfile()
	application, err := service.ImportOAuthApplication(profile.ProfileDigest, nil, []byte(`{"installed":{"client_id":"desktop-client","client_secret":"desktop-secret"}}`))
	if err != nil {
		t.Fatal(err)
	}
	manifest := oauthManifest()
	manifest.Reviewed = true
	definition, err := service.files.installDefinition(manifest, "https://example.com/docs", nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	generation := randomHex()
	grant := OAuthGrant{SchemaVersion: 1, GrantID: randomHex(), ApplicationID: application.ApplicationID, Audience: "google-apis", DesiredScopes: []string{"scope.read"}, GrantedScopes: []string{"scope.read"}, AuthorityRevision: 2, TokenGeneration: &generation, TokenRevision: 2, Status: "active"}
	secret := oauthGrantToken{SchemaVersion: 1, GenerationID: generation, AccessToken: "access-secret", RefreshToken: "refresh-secret", ExpiresAt: time.Now().Add(time.Hour).Unix()}
	if err = service.files.installOAuthOwned("adapters/oauth-grants", grant.GrantID, "grant.json", grant, "tokens", generation, secret); err != nil {
		t.Fatal(err)
	}
	_, connection, err := service.AttachOAuthConnection(t.Context(), definition.SemanticDigest, grant.GrantID, grant.AuthorityRevision, "")
	if err != nil {
		t.Fatal(err)
	}
	_, reused, err := service.AttachOAuthConnection(t.Context(), definition.SemanticDigest, grant.GrantID, grant.AuthorityRevision, "")
	if err != nil || reused.ConnectionID != connection.ConnectionID {
		t.Fatalf("grant reuse = %#v, %v", reused, err)
	}
	if _, _, err = service.AttachOAuthConnection(t.Context(), definition.SemanticDigest, grant.GrantID, grant.AuthorityRevision, connection.ConnectionID); err == nil {
		t.Fatal("active connection was replaced")
	}
	if _, err = service.SaveConnectionPolicy(t.Context(), connection.ConnectionID, "1", 1, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	bindings, err := service.Bindings()
	if err != nil || len(bindings) != 1 || bindings[0].GrantID != grant.GrantID || bindings[0].CredentialRevision != 2 {
		t.Fatalf("bindings = %#v, %v", bindings, err)
	}
	grant, err = service.DisconnectOAuthGrant(t.Context(), grant.GrantID, 2)
	if err != nil || grant.Status != "revoked" || grant.AuthorityRevision != 3 || grant.TokenRevision != 3 {
		t.Fatalf("disconnect = %#v, %v", grant, err)
	}
	tokenDirectory := filepath.Join(directory, "adapters", "oauth-grants", grant.GrantID, "tokens")
	if entries, readErr := os.ReadDir(tokenDirectory); readErr != nil || len(entries) != 0 {
		t.Fatalf("disconnected token files = %#v, %v", entries, readErr)
	}
	bindings, err = service.Bindings()
	if err != nil || len(bindings) != 0 {
		t.Fatalf("bindings after disconnect = %#v, %v", bindings, err)
	}
	raw, _ := json.Marshal(grant)
	if strings.Contains(string(raw), "access-secret") || strings.Contains(string(raw), "refresh-secret") {
		t.Fatal("grant descriptor contains token material")
	}
	failureGeneration := randomHex()
	failureGrant := OAuthGrant{SchemaVersion: 1, GrantID: randomHex(), ApplicationID: application.ApplicationID, Audience: "google-apis", DesiredScopes: []string{"scope.read"}, GrantedScopes: []string{"scope.read"}, AuthorityRevision: 4, TokenGeneration: &failureGeneration, TokenRevision: 7, Status: "active"}
	if err = service.files.installOAuthOwned("adapters/oauth-grants", failureGrant.GrantID, "grant.json", failureGrant, "tokens", failureGeneration, oauthGrantToken{SchemaVersion: 1, GenerationID: failureGeneration, AccessToken: "second-access", RefreshToken: "second-refresh"}); err != nil {
		t.Fatal(err)
	}
	unreviewed := oauthManifest()
	unreviewed.DefinitionRevision = "unreviewed"
	unreviewedDefinition, err := service.files.installDefinition(unreviewed, "https://example.com/unreviewed", nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err = service.AttachOAuthConnection(t.Context(), unreviewedDefinition.SemanticDigest, failureGrant.GrantID, 4, ""); err == nil {
		t.Fatal("unreviewed definition was attached")
	}
	if err = service.requireOAuthAuthentication(t.Context(), &failureGrant); err != nil {
		t.Fatal(err)
	}
	failureGrant, _, err = service.files.loadOAuthGrant(failureGrant.GrantID)
	if err != nil || failureGrant.Status != "authentication_required" || failureGrant.AuthorityRevision != 5 || failureGrant.TokenRevision != 8 || failureGrant.TokenGeneration != nil {
		t.Fatalf("authentication failure grant = %#v, %v", failureGrant, err)
	}
	failureTokenDirectory := filepath.Join(directory, "adapters", "oauth-grants", failureGrant.GrantID, "tokens")
	if entries, readErr := os.ReadDir(failureTokenDirectory); readErr != nil || len(entries) != 0 {
		t.Fatalf("failed token files = %#v, %v", entries, readErr)
	}
	corruptID := randomHex()
	corruptDirectory := filepath.Join(directory, "adapters", "oauth-grants", corruptID)
	if err = os.Mkdir(corruptDirectory, 0o700); err != nil {
		t.Fatal(err)
	}
	if err = os.WriteFile(filepath.Join(corruptDirectory, "grant.json"), []byte(`{"status":"partial"}`), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err = service.OAuthSnapshot(); err == nil {
		t.Fatal("invalid OAuth authority was omitted from the snapshot")
	}
}
