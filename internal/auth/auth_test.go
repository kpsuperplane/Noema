package auth

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"runtime"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	webauthnlib "github.com/go-webauthn/webauthn/webauthn"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestRecoveryCodeRotatesSeriallyAndPreservesConfig(t *testing.T) {
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(paths.Config(), []byte("mcp:\n  stdio_enabled: true\n")); err != nil {
		t.Fatal(err)
	}
	config, recovery, err := LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	if config.Authority != "localhost:3737" || config.Origin != "http://localhost:3737" {
		t.Fatalf("unexpected local authority: %#v", config)
	}
	initial := readRecoveryCode(t, paths)
	if matched, err := recovery.Attempt("wrong"); err != nil || matched {
		t.Fatalf("wrong recovery attempt = %v, %v", matched, err)
	}
	rotated := readRecoveryCode(t, paths)
	if rotated == initial {
		t.Fatal("wrong recovery attempt did not rotate the code")
	}
	if matched, err := recovery.Attempt(initial); err != nil || matched {
		t.Fatalf("old recovery code = %v, %v", matched, err)
	}
	current := readRecoveryCode(t, paths)
	results := make(chan bool, 2)
	errorsSeen := make(chan error, 2)
	var group sync.WaitGroup
	for range 2 {
		group.Add(1)
		go func() {
			defer group.Done()
			matched, err := recovery.Attempt(current)
			results <- matched
			errorsSeen <- err
		}()
	}
	group.Wait()
	close(results)
	close(errorsSeen)
	var matches int
	for matched := range results {
		if matched {
			matches++
		}
	}
	for err := range errorsSeen {
		if err != nil {
			t.Fatal(err)
		}
	}
	if matches != 1 {
		t.Fatalf("concurrent recovery matches = %d, want 1", matches)
	}
	document, err := readConfigDocument(paths.Config())
	if err != nil {
		t.Fatal(err)
	}
	mcp, err := childMap(document, "mcp")
	if err != nil || mcp["stdio_enabled"] != true {
		t.Fatalf("unrelated config changed: %#v, %v", document, err)
	}
	nullPaths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(nullPaths.Config(), []byte("null\n")); err != nil {
		t.Fatal(err)
	}
	if _, _, err := LoadConfig(nullPaths, "127.0.0.1:3737"); err == nil {
		t.Fatal("top-level YAML null did not fail closed")
	}
	configuredPaths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(configuredPaths.Config(), []byte("web:\n  host: 0.0.0.0\n  port: 4747\n  graphiql: true\n  local_graphql_socket: true\n")); err != nil {
		t.Fatal(err)
	}
	configured, _, err := LoadConfig(configuredPaths, "")
	if err != nil {
		t.Fatal(err)
	}
	if configured.ListenAddress != "0.0.0.0:4747" || !configured.GraphiQL || !configured.LocalGraphQLSocket {
		t.Fatalf("configured web server = %#v", configured)
	}
}

func TestAuthorityAndSetupBarrierRejectBeforeGraphQLParsing(t *testing.T) {
	server, _, _ := newAuthTest(t, false)
	var applicationCalls atomic.Int32
	application := http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		applicationCalls.Add(1)
		w.WriteHeader(http.StatusNoContent)
	})
	handler := server.Handler(application)

	invalidHost := authRequest(http.MethodGet, "/auth/status", nil)
	invalidHost.Host = "attacker.example"
	if response := serve(handler, invalidHost); response.Code != http.StatusBadRequest {
		t.Fatalf("invalid Host status = %d", response.Code)
	}
	missingOrigin := authRequest(http.MethodPost, "/auth/passkey/register/start", nil)
	missingOrigin.Header.Del("Origin")
	if response := serve(handler, missingOrigin); response.Code != http.StatusForbidden {
		t.Fatalf("missing Origin status = %d", response.Code)
	}
	graphql := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString("not GraphQL"))
	if response := serve(handler, graphql); response.Code != http.StatusForbidden || applicationCalls.Load() != 0 {
		t.Fatalf("setup GraphQL = %d with %d application calls", response.Code, applicationCalls.Load())
	}
	nativeGraphQL := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString("not GraphQL"))
	nativeGraphQL.Header.Del("Origin")
	nativeGraphQL.Header.Set("Authorization", "Bearer invalid")
	if response := serve(handler, nativeGraphQL); response.Code != http.StatusForbidden ||
		!strings.Contains(response.Body.String(), "setup_required") {
		t.Fatalf("setup bearer barrier = %d %s", response.Code, response.Body.String())
	}
	status := serve(handler, authRequest(http.MethodGet, "/auth/status", nil))
	if status.Code != http.StatusOK || !bytes.Contains(status.Body.Bytes(), []byte(`"setup_ready"`)) {
		t.Fatalf("setup status = %d %s", status.Code, status.Body.String())
	}
	policy := status.Header().Get("Content-Security-Policy")
	if !strings.Contains(policy, "; font-src 'self' data:;") || !strings.Contains(policy, "; script-src 'self';") {
		t.Fatalf("embedded fonts need permission without extending script sources: %q", policy)
	}
	token, _, _ := seedPasskey(t, server, 9)
	artifactRequest := authRequest(http.MethodGet, "/artifacts/versions/abc/download", nil)
	if response := serve(handler, artifactRequest); response.Code != http.StatusUnauthorized {
		t.Fatalf("unauthenticated Artifact status = %d", response.Code)
	}
	artifactRequest = authRequest(http.MethodGet, "/artifacts/versions/abc/download", nil)
	artifactRequest.AddCookie(&http.Cookie{Name: server.sessions.cookieName, Value: token})
	if response := serve(handler, artifactRequest); response.Code != http.StatusNoContent {
		t.Fatalf("authenticated Artifact status = %d", response.Code)
	}
	faviconRequest := authRequest(http.MethodGet, "/favicons/example.com", nil)
	if response := serve(handler, faviconRequest); response.Code != http.StatusUnauthorized {
		t.Fatalf("unauthenticated favicon status = %d", response.Code)
	}
	faviconRequest.AddCookie(&http.Cookie{Name: server.sessions.cookieName, Value: token})
	if response := serve(handler, faviconRequest); response.Code != http.StatusNoContent {
		t.Fatalf("authenticated favicon status = %d", response.Code)
	}
	if response := serve(handler, authRequest(http.MethodGet, "/graphql/schema.graphql", nil)); response.Code != http.StatusUnauthorized {
		t.Fatalf("unauthenticated schema status = %d", response.Code)
	}

	devServer, _, _ := newAuthTest(t, true)
	devHandler := devServer.Handler(application)
	if response := serve(devHandler, authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{}`))); response.Code != http.StatusNoContent {
		t.Fatalf("development GraphQL = %d", response.Code)
	}
	var bodySize int
	bodyHandler := devServer.Handler(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		body, err := io.ReadAll(request.Body)
		if err != nil {
			http.Error(w, err.Error(), http.StatusRequestEntityTooLarge)
			return
		}
		bodySize = len(body)
		w.WriteHeader(http.StatusNoContent)
	}))
	largeDocumentRequest := strings.Repeat("x", 400*1024)
	if response := serve(bodyHandler, authRequest(http.MethodPost, "/graphql",
		bytes.NewBufferString(largeDocumentRequest))); response.Code != http.StatusNoContent || bodySize != len(largeDocumentRequest) {
		t.Fatalf("large GraphQL body = %d with %d bytes", response.Code, bodySize)
	}
	if response := serve(devHandler, recoveryRequest("unavailable", nil)); response.Code != http.StatusNotFound {
		t.Fatalf("development recovery = %d", response.Code)
	}
	devInvalidHost := authRequest(http.MethodGet, "/auth/status", nil)
	devInvalidHost.Host = "attacker.example"
	if response := serve(devHandler, devInvalidHost); response.Code != http.StatusBadRequest {
		t.Fatalf("development invalid Host = %d", response.Code)
	}
	applicationCount := applicationCalls.Load()
	wrongUpgrade := authRequest(http.MethodGet, "/graphql", nil)
	wrongUpgrade.Header.Set("Origin", "http://localhost:3737")
	wrongUpgrade.Header.Set("Upgrade", "websocket")
	if response := serve(devHandler, wrongUpgrade); response.Code != http.StatusNotFound {
		t.Fatalf("wrong WebSocket route = %d", response.Code)
	}
	postWebSocket := authRequest(http.MethodPost, "/graphql/ws", bytes.NewBufferString(`{}`))
	if response := serve(devHandler, postWebSocket); response.Code != http.StatusMethodNotAllowed {
		t.Fatalf("POST WebSocket route = %d", response.Code)
	}
	if applicationCalls.Load() != applicationCount {
		t.Fatal("invalid WebSocket routes reached GraphQL")
	}
	for range cap(devServer.httpSlots) {
		devServer.httpSlots <- struct{}{}
	}
	if response := serve(devHandler, authRequest(http.MethodGet, "/auth/status", nil)); response.Code != http.StatusServiceUnavailable {
		t.Fatalf("HTTP capacity status = %d", response.Code)
	}
	for range cap(devServer.httpSlots) {
		<-devServer.httpSlots
	}
}

func TestCeremoniesAreBoundedBoundAndSingleUse(t *testing.T) {
	server, _, _ := newAuthTest(t, false)
	security := server.passkeys
	first := pendingCeremony{kind: registrationCeremony, binding: testAuthDigest("one"), expiresAt: time.Now().Add(time.Minute)}
	firstID, err := security.insert(first)
	if err != nil {
		t.Fatal(err)
	}
	secondID, err := security.insert(first)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := security.take(firstID, first.binding, registrationCeremony); !errors.Is(err, errInvalidCeremony) {
		t.Fatalf("replaced ceremony = %v", err)
	}
	if _, err := security.take(secondID, testAuthDigest("wrong"), registrationCeremony); !errors.Is(err, errInvalidCeremony) {
		t.Fatalf("wrong binding = %v", err)
	}
	if _, err := security.take(secondID, first.binding, registrationCeremony); err != nil {
		t.Fatal(err)
	}
	if _, err := security.take(secondID, first.binding, registrationCeremony); !errors.Is(err, errInvalidCeremony) {
		t.Fatalf("reused ceremony = %v", err)
	}
	for index := range ceremonyCapacity {
		_, err := security.insert(pendingCeremony{
			kind:      registrationCeremony,
			binding:   testAuthDigest(fmt.Sprintf("binding-%d", index)),
			expiresAt: time.Now().Add(time.Minute),
		})
		if err != nil {
			t.Fatal(err)
		}
	}
	if _, err := security.insert(pendingCeremony{
		kind: registrationCeremony, binding: testAuthDigest("overflow"), expiresAt: time.Now().Add(time.Minute),
	}); !errors.Is(err, errCeremonyFull) {
		t.Fatalf("ceremony capacity = %v", err)
	}
}

func TestRegistrationStartUsesRequiredVerificationAndPrivateCookie(t *testing.T) {
	server, _, _ := newAuthTest(t, false)
	handler := server.Handler(http.NotFoundHandler())
	response := serve(handler, authRequest(http.MethodPost, "/auth/passkey/register/start", nil))
	if response.Code != http.StatusOK {
		t.Fatalf("registration start = %d %s", response.Code, response.Body.String())
	}
	var payload struct {
		CeremonyID string `json:"ceremonyId"`
		Options    struct {
			PublicKey struct {
				AuthenticatorSelection struct {
					UserVerification string `json:"userVerification"`
				} `json:"authenticatorSelection"`
			} `json:"publicKey"`
		} `json:"options"`
	}
	if err := json.Unmarshal(response.Body.Bytes(), &payload); err != nil {
		t.Fatal(err)
	}
	if payload.CeremonyID == "" || payload.Options.PublicKey.AuthenticatorSelection.UserVerification != "required" {
		t.Fatalf("registration payload = %#v", payload)
	}
	cookies := response.Result().Cookies()
	if len(cookies) != 1 {
		t.Fatalf("registration cookies = %#v", cookies)
	}
	cookie := cookies[0]
	if cookie.Name != "noema.sid" || !cookie.HttpOnly || cookie.SameSite != http.SameSiteStrictMode || cookie.Path != "/" || cookie.Secure {
		t.Fatalf("local cookie = %#v", cookie)
	}
	if _, ok := server.sessions.verifyCookie(cookie.Value); !ok {
		t.Fatal("issued browser cookie did not verify")
	}
}

func TestRecoverySetupIsIsolatedBoundedAndReplaced(t *testing.T) {
	server, _, paths := newAuthTest(t, false)
	_, _, _ = seedPasskey(t, server, 1)
	handler := server.Handler(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusNoContent)
	}))
	code := readRecoveryCode(t, paths)
	response := serve(handler, recoveryRequest(code, nil))
	if response.Code != http.StatusNoContent {
		t.Fatalf("recovery = %d %s", response.Code, response.Body.String())
	}
	setupCookie := lastSessionCookie(t, response, server.sessions.cookieName)
	graphql := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	graphql.AddCookie(setupCookie)
	if result := serve(handler, graphql); result.Code != http.StatusUnauthorized {
		t.Fatalf("setup session GraphQL = %d", result.Code)
	}
	for index := range 8 {
		request := authRequest(http.MethodPost, "/auth/passkey/register/start", nil)
		request.AddCookie(setupCookie)
		if result := serve(handler, request); result.Code != http.StatusOK {
			t.Fatalf("setup start %d = %d %s", index, result.Code, result.Body.String())
		}
	}
	ninth := authRequest(http.MethodPost, "/auth/passkey/register/start", nil)
	ninth.AddCookie(setupCookie)
	if result := serve(handler, ninth); result.Code != http.StatusForbidden {
		t.Fatalf("ninth setup start = %d", result.Code)
	}

	secondCode := readRecoveryCode(t, paths)
	second := serve(handler, recoveryRequest(secondCode, nil))
	if second.Code != http.StatusNoContent {
		t.Fatalf("second recovery = %d", second.Code)
	}
	oldStart := authRequest(http.MethodPost, "/auth/passkey/register/start", nil)
	oldStart.AddCookie(setupCookie)
	if result := serve(handler, oldStart); result.Code != http.StatusForbidden {
		t.Fatalf("replaced setup grant = %d", result.Code)
	}
}

func TestPasskeyManagementAndGraphQLRevocationUseStoredSessions(t *testing.T) {
	server, taskStore, paths := newAuthTest(t, false)
	firstToken, firstDigest, first := seedPasskey(t, server, 1)
	secondToken, secondDigest, err := server.sessions.newToken()
	if err != nil {
		t.Fatal(err)
	}
	second := authCredential(t, 2)
	if err := taskStore.RegisterPasskey(
		context.Background(),
		second,
		store.RegistrationCurrent,
		firstDigest,
		secondDigest,
		time.Now(),
	); err != nil {
		t.Fatal(err)
	}
	_ = firstToken
	cookie := &http.Cookie{Name: server.sessions.cookieName, Value: secondToken}

	handlerReturned := make(chan struct{})
	handlerStarted := make(chan struct{})
	var missingBrowserBinding atomic.Bool
	application := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if digest, ok := BrowserSessionHash(r.Context()); !ok || digest != secondDigest {
			missingBrowserBinding.Store(true)
		}
		if r.URL.Path == "/graphql/ws" {
			close(handlerStarted)
			<-r.Context().Done()
			close(handlerReturned)
			return
		}
		w.WriteHeader(http.StatusNoContent)
	})
	handler := server.Handler(application)
	list := authRequest(http.MethodGet, "/auth/passkeys", nil)
	list.AddCookie(cookie)
	listed := serve(handler, list)
	if listed.Code != http.StatusOK || !bytes.Contains(listed.Body.Bytes(), []byte(`"current":true`)) {
		t.Fatalf("passkey list = %d %s", listed.Code, listed.Body.String())
	}
	remove := authRequest(
		http.MethodPost,
		"/auth/passkey/remove",
		bytes.NewBufferString(fmt.Sprintf(`{"credentialId":%q}`, first.CredentialID)),
	)
	remove.AddCookie(cookie)
	if result := serve(handler, remove); result.Code != http.StatusNoContent {
		t.Fatalf("remove passkey = %d %s", result.Code, result.Body.String())
	}
	removeFinal := authRequest(
		http.MethodPost,
		"/auth/passkey/remove",
		bytes.NewBufferString(fmt.Sprintf(`{"credentialId":%q}`, second.CredentialID)),
	)
	removeFinal.AddCookie(cookie)
	if result := serve(handler, removeFinal); result.Code != http.StatusConflict {
		t.Fatalf("remove final passkey = %d", result.Code)
	}
	unauthorized := serve(handler, authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{}`)))
	if unauthorized.Code != http.StatusUnauthorized {
		t.Fatalf("unauthenticated GraphQL = %d", unauthorized.Code)
	}
	authorized := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{}`))
	authorized.AddCookie(cookie)
	if result := serve(handler, authorized); result.Code != http.StatusNoContent {
		t.Fatalf("authenticated GraphQL = %d", result.Code)
	}
	if missingBrowserBinding.Load() {
		t.Fatal("authenticated GraphQL did not receive its browser binding")
	}

	restartedSessions, err := newSessionSecurity(paths, taskStore, false)
	if err != nil {
		t.Fatal(err)
	}
	if digest, ok := restartedSessions.verifyCookie(secondToken); !ok || digest != secondDigest {
		t.Fatal("browser cookie did not survive session-security restart")
	}

	websocket := authRequest(http.MethodGet, "/graphql/ws", nil)
	websocket.Header.Set("Upgrade", "websocket")
	websocket.AddCookie(cookie)
	for range cap(server.wsSlots) {
		server.wsSlots <- struct{}{}
	}
	if result := serve(handler, websocket.Clone(websocket.Context())); result.Code != http.StatusServiceUnavailable {
		t.Fatalf("WebSocket capacity status = %d", result.Code)
	}
	for range cap(server.wsSlots) {
		<-server.wsSlots
	}
	go handler.ServeHTTP(httptest.NewRecorder(), websocket)
	select {
	case <-handlerStarted:
	case <-time.After(time.Second):
		t.Fatal("WebSocket handler did not start")
	}
	if err := taskStore.DeleteBrowserSession(context.Background(), secondDigest); err != nil {
		t.Fatal(err)
	}
	server.sessions.revoke(secondDigest)
	select {
	case <-handlerReturned:
	case <-time.After(time.Second):
		t.Fatal("session revocation did not close the WebSocket")
	}
}

func TestNativeAuthorizationRequestValidationMatchesCurrentClients(t *testing.T) {
	desktop := nativeAuthorizationQuery(testDesktopClient, testDesktopRedirect, "s")
	ios := nativeAuthorizationQuery("noema-ios:abcdefghijklmnop", "noema://oauth/callback", "i")
	for name, query := range map[string]string{"desktop": desktop, "iOS": ios} {
		if _, err := parseAuthorizationRequest(query); err != nil {
			t.Fatalf("%s request = %v", name, err)
		}
	}
	for name, query := range map[string]string{
		"duplicate": desktop + "&state=duplicate",
		"scope":     desktop + "&scope=other",
		"redirect":  nativeAuthorizationQuery(testDesktopClient, "http://example.com/oauth/callback", "r"),
		"pkce":      strings.Replace(desktop, testVerifierChallenge, "short", 1),
	} {
		if _, err := parseAuthorizationRequest(query); err == nil {
			t.Fatalf("%s request was accepted", name)
		}
	}
}

func TestNativeConsentResumesAfterPasskeyAndConsumesCSRFOnce(t *testing.T) {
	server, taskStore, _ := newAuthTest(t, false)
	_, _, credential := seedPasskey(t, server, 8)
	oldToken, oldDigest, err := server.sessions.newToken()
	if err != nil {
		t.Fatal(err)
	}
	if err := taskStore.CreateAnonymousSession(context.Background(), oldDigest, time.Now()); err != nil {
		t.Fatal(err)
	}
	handler := server.Handler(http.NotFoundHandler())
	query := nativeAuthorizationQuery(testDesktopClient, testDesktopRedirect, "c")
	request := authRequest(http.MethodGet, "/oauth/authorize?"+query, nil)
	request.AddCookie(&http.Cookie{Name: server.sessions.cookieName, Value: oldToken})
	if response := serve(handler, request); response.Code != http.StatusSeeOther ||
		response.Header().Get("Location") != "/?native_authorization=resume" {
		t.Fatalf("authorization resume = %d %q", response.Code, response.Header().Get("Location"))
	}
	newToken, newDigest, err := server.sessions.newToken()
	if err != nil {
		t.Fatal(err)
	}
	if err := taskStore.AuthenticatePasskey(
		context.Background(), credential.CredentialID, credential.CredentialJSON,
		credential.CredentialJSON, oldDigest, newDigest, time.Now(),
	); err != nil {
		t.Fatal(err)
	}
	cookie := &http.Cookie{Name: server.sessions.cookieName, Value: newToken}
	resume := authRequest(http.MethodGet, "/oauth/authorize", nil)
	resume.AddCookie(cookie)
	if response := serve(handler, resume); response.Code != http.StatusOK || !strings.Contains(response.Body.String(), "Connect Noema Desktop?</h1>") {
		t.Fatalf("resumed consent = %d %s", response.Code, response.Body.String())
	}
	_, csrf, found, err := taskStore.NativeOAuthBrowserRequest(context.Background(), newDigest)
	if err != nil || !found || csrf == nil {
		t.Fatalf("pending consent = %v, %v, %v", found, csrf, err)
	}
	wrong := oauthFormRequest("/oauth/authorize", url.Values{"csrf": {"wrong"}, "decision": {"approve"}})
	wrong.AddCookie(cookie)
	if response := serve(handler, wrong); response.Code != http.StatusBadRequest {
		t.Fatalf("wrong CSRF = %d", response.Code)
	}
	reused := oauthFormRequest("/oauth/authorize", url.Values{"csrf": {*csrf}, "decision": {"approve"}})
	reused.AddCookie(cookie)
	if response := serve(handler, reused); response.Code != http.StatusBadRequest {
		t.Fatalf("reused consent = %d", response.Code)
	}

	start := authRequest(http.MethodGet, "/oauth/authorize?"+query, nil)
	start.AddCookie(cookie)
	if response := serve(handler, start); response.Code != http.StatusOK {
		t.Fatalf("second consent = %d", response.Code)
	}
	_, csrf, _, _ = taskStore.NativeOAuthBrowserRequest(context.Background(), newDigest)
	deny := oauthFormRequest("/oauth/authorize", url.Values{"csrf": {*csrf}, "decision": {"deny"}})
	deny.AddCookie(cookie)
	deny.Header.Set("Origin", "null")
	denied := serve(handler, deny)
	if denied.Code != http.StatusFound || !strings.Contains(denied.Header().Get("Location"), "error=access_denied") {
		t.Fatalf("denied redirect = %d %q", denied.Code, denied.Header().Get("Location"))
	}
	start = authRequest(http.MethodGet, "/oauth/authorize?"+query, nil)
	start.AddCookie(cookie)
	if response := serve(handler, start); response.Code != http.StatusOK {
		t.Fatalf("third consent = %d", response.Code)
	}
	_, csrf, _, _ = taskStore.NativeOAuthBrowserRequest(context.Background(), newDigest)
	blocked := oauthFormRequest("/oauth/authorize", url.Values{"csrf": {*csrf}, "decision": {"approve"}})
	blocked.AddCookie(cookie)
	blocked.Header.Set("Origin", "http://attacker.example")
	if response := serve(handler, blocked); response.Code != http.StatusForbidden {
		t.Fatalf("foreign consent Origin = %d", response.Code)
	}
	approve := oauthFormRequest("/oauth/authorize", url.Values{"csrf": {*csrf}, "decision": {"approve"}})
	approve.AddCookie(cookie)
	approve.Header.Del("Origin")
	response := serve(handler, approve)
	location, err := url.Parse(response.Header().Get("Location"))
	if err != nil || response.Code != http.StatusFound || location.Query().Get("code") == "" || location.Query().Get("state") != strings.Repeat("c", 32) {
		t.Fatalf("approved redirect = %d %q, %v", response.Code, response.Header().Get("Location"), err)
	}
}

func TestNativeBearerAdmitsGraphQLAndClosesOnClientRevocation(t *testing.T) {
	server, taskStore, _ := newAuthTest(t, false)
	browserToken, _, _ := seedPasskey(t, server, 9)
	tokens := exchangeNativeTokens(t, server, taskStore, "bearer")
	started, returned := make(chan struct{}), make(chan struct{})
	application := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if ClientID(r.Context()) != testDesktopClient {
			t.Errorf("native client context = %q", ClientID(r.Context()))
		}
		if isWebSocket(r) {
			close(started)
			<-r.Context().Done()
			close(returned)
			return
		}
		w.WriteHeader(http.StatusNoContent)
	})
	handler := server.Handler(application)
	graphql := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{}`))
	graphql.Header.Del("Origin")
	graphql.Header.Set("Authorization", "Bearer "+tokens.AccessToken)
	if response := serve(handler, graphql); response.Code != http.StatusNoContent {
		t.Fatalf("native GraphQL = %d %s", response.Code, response.Body.String())
	}
	invalid := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{}`))
	invalid.Header.Del("Origin")
	invalid.Header.Set("Authorization", "Bearer invalid")
	invalid.AddCookie(&http.Cookie{Name: server.sessions.cookieName, Value: browserToken})
	if response := serve(handler, invalid); response.Code != http.StatusUnauthorized {
		t.Fatalf("invalid bearer fallback = %d", response.Code)
	}
	websocket := authRequest(http.MethodGet, "/graphql/ws", nil)
	websocket.Header.Del("Origin")
	websocket.Header.Set("Upgrade", "websocket")
	websocket.Header.Set("Authorization", "Bearer "+tokens.AccessToken)
	go handler.ServeHTTP(httptest.NewRecorder(), websocket)
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("native WebSocket did not start")
	}
	client, err := server.RevokeClient(context.Background(), testDesktopClient)
	if err != nil || client.RevokedAt == nil {
		t.Fatalf("client revocation = %#v, %v", client, err)
	}
	select {
	case <-returned:
	case <-time.After(time.Second):
		t.Fatal("client revocation did not close the native WebSocket")
	}
}

func TestNativeRefreshRetrySurvivesRestartAndMismatchRevokes(t *testing.T) {
	server, taskStore, paths := newAuthTest(t, false)
	_, _, _ = seedPasskey(t, server, 10)
	tokens := exchangeNativeTokens(t, server, taskStore, "refresh")
	handler := server.Handler(http.NotFoundHandler())
	requestID := strings.Repeat("a", 32)
	refreshValues := url.Values{
		"grant_type": {"refresh_token"}, "refresh_token": {tokens.RefreshToken},
		"refresh_request_id": {requestID},
	}
	start := make(chan struct{})
	responses := make(chan *httptest.ResponseRecorder, 2)
	var group sync.WaitGroup
	for range 2 {
		group.Add(1)
		go func() {
			defer group.Done()
			<-start
			responses <- serve(handler, oauthFormRequest("/oauth/token", refreshValues))
		}()
	}
	close(start)
	group.Wait()
	close(responses)
	var firstTokens nativeTokenResponse
	for response := range responses {
		current := decodeNativeTokens(t, response)
		if firstTokens.AccessToken == "" {
			firstTokens = current
		} else if firstTokens.AccessToken != current.AccessToken || firstTokens.RefreshToken != current.RefreshToken {
			t.Fatal("concurrent request-bound retries returned different successors")
		}
	}
	restarted, err := New(paths, taskStore, server.config, server.recovery)
	if err != nil {
		t.Fatal(err)
	}
	retry := serve(restarted.Handler(http.NotFoundHandler()), oauthFormRequest("/oauth/token", refreshValues))
	retryTokens := decodeNativeTokens(t, retry)
	if firstTokens.AccessToken != retryTokens.AccessToken || firstTokens.RefreshToken != retryTokens.RefreshToken {
		t.Fatal("request-bound retry did not return the saved successor")
	}
	mismatch := url.Values{
		"grant_type": {"refresh_token"}, "refresh_token": {tokens.RefreshToken},
		"refresh_request_id": {strings.Repeat("b", 32)},
	}
	response := serve(restarted.Handler(http.NotFoundHandler()), oauthFormRequest("/oauth/token", mismatch))
	if response.Code != http.StatusBadRequest || !strings.Contains(response.Body.String(), "invalid_grant") {
		t.Fatalf("mismatched retry = %d %s", response.Code, response.Body.String())
	}
	if _, exists, err := taskStore.ActiveNativeOAuthAccess(
		context.Background(), sha256.Sum256([]byte(firstTokens.AccessToken)), time.Now().Unix(),
	); err != nil || exists {
		t.Fatalf("replayed family access = %v, %v", exists, err)
	}
	entries, err := restarted.native.retries.read()
	if err != nil {
		t.Fatal(err)
	}
	if _, exists := entries[nativeRetryKey(sha256.Sum256([]byte(tokens.RefreshToken)), requestID)]; exists {
		t.Fatal("revoked family retained its request-bound retry")
	}
	expiredHash := sha256.Sum256([]byte("expired-retry"))
	expiredAt := time.Now().Unix() - 61
	expiredRequestID := strings.Repeat("c", 32)
	if err := restarted.native.retries.save(expiredHash, expiredRequestID, nativeRetryEntry{
		IssuedAt: expiredAt, RetainUntil: expiredAt, FamilyID: "expired-family",
	}, expiredAt); err != nil {
		t.Fatal(err)
	}
	entries, err = restarted.native.retries.read()
	if err != nil {
		t.Fatal(err)
	}
	if _, exists := entries[nativeRetryKey(expiredHash, expiredRequestID)]; !exists {
		t.Fatal("request-bound expiry fixture was not saved")
	}
	if err := restarted.native.retries.prune(time.Now().Unix()); err != nil {
		t.Fatal(err)
	}
	entries, err = restarted.native.retries.read()
	if err != nil {
		t.Fatal(err)
	}
	if _, exists := entries[nativeRetryKey(expiredHash, expiredRequestID)]; exists {
		t.Fatal("idle-expired request-bound retry was not pruned")
	}
}

func newAuthTest(t *testing.T, devNoAuth bool) (*Server, *store.Store, home.Paths) {
	t.Helper()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	config, recovery, err := LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	config.DevNoAuth = devNoAuth
	taskStore, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = taskStore.Close() })
	server, err := New(paths, taskStore, config, recovery)
	if err != nil {
		t.Fatal(err)
	}
	return server, taskStore, paths
}

func seedPasskey(t *testing.T, server *Server, value byte) (string, [32]byte, store.HumanPasskey) {
	t.Helper()
	_, oldDigest, err := server.sessions.newToken()
	if err != nil {
		t.Fatal(err)
	}
	if err := server.sessions.store.CreateAnonymousSession(context.Background(), oldDigest, time.Now()); err != nil {
		t.Fatal(err)
	}
	token, digest, err := server.sessions.newToken()
	if err != nil {
		t.Fatal(err)
	}
	credential := authCredential(t, value)
	if err := server.sessions.store.RegisterPasskey(
		context.Background(), credential, store.RegistrationInitial, oldDigest, digest, time.Now(),
	); err != nil {
		t.Fatal(err)
	}
	return token, digest, credential
}

func authCredential(t *testing.T, value byte) store.HumanPasskey {
	t.Helper()
	encoded, err := json.Marshal(&webauthnlib.Credential{ID: []byte{value}})
	if err != nil {
		t.Fatal(err)
	}
	return store.HumanPasskey{
		CredentialID:   base64.RawURLEncoding.EncodeToString([]byte{value}),
		CredentialJSON: string(encoded),
	}
}

func authRequest(method string, path string, body *bytes.Buffer) *http.Request {
	var request *http.Request
	if body == nil {
		request = httptest.NewRequest(method, "http://localhost:3737"+path, nil)
	} else {
		request = httptest.NewRequest(method, "http://localhost:3737"+path, body)
	}
	request.Host = "localhost:3737"
	if method == http.MethodPost || path == "/graphql/ws" {
		request.Header.Set("Origin", "http://localhost:3737")
	}
	if body != nil {
		request.Header.Set("Content-Type", "application/json")
	}
	return request
}

func recoveryRequest(code string, cookie *http.Cookie) *http.Request {
	request := authRequest(
		http.MethodPost,
		"/auth/recovery",
		bytes.NewBufferString(fmt.Sprintf(`{"code":%q}`, code)),
	)
	if cookie != nil {
		request.AddCookie(cookie)
	}
	return request
}

func serve(handler http.Handler, request *http.Request) *httptest.ResponseRecorder {
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)
	return response
}

func readRecoveryCode(t *testing.T, paths home.Paths) string {
	t.Helper()
	document, err := readConfigDocument(paths.Config())
	if err != nil {
		t.Fatal(err)
	}
	web, err := childMap(document, "web")
	if err != nil {
		t.Fatal(err)
	}
	code, _ := web["recovery_code"].(string)
	if !validRecoveryCode(code) {
		t.Fatal("stored recovery code is invalid")
	}
	return code
}

func lastSessionCookie(t *testing.T, response *httptest.ResponseRecorder, name string) *http.Cookie {
	t.Helper()
	cookies := response.Result().Cookies()
	for index := len(cookies) - 1; index >= 0; index-- {
		if cookies[index].Name == name {
			return cookies[index]
		}
	}
	t.Fatal("response did not contain a browser session cookie")
	return nil
}

func testAuthDigest(value string) [32]byte {
	return sha256.Sum256([]byte(value))
}

const (
	testDesktopClient     = "noema-desktop:abcdefghijklmnop"
	testDesktopRedirect   = "http://127.0.0.1:49152/oauth/callback"
	testVerifier          = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
	testVerifierChallenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
)

func nativeAuthorizationQuery(clientID string, redirect string, stateCharacter string) string {
	return url.Values{
		"client_id":             {clientID},
		"redirect_uri":          {redirect},
		"response_type":         {"code"},
		"state":                 {strings.Repeat(stateCharacter, 32)},
		"code_challenge":        {testVerifierChallenge},
		"code_challenge_method": {"S256"},
	}.Encode()
}

func oauthFormRequest(path string, values url.Values) *http.Request {
	request := authRequest(http.MethodPost, path, bytes.NewBufferString(values.Encode()))
	request.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	return request
}

func exchangeNativeTokens(
	t *testing.T,
	server *Server,
	taskStore *store.Store,
	suffix string,
) nativeTokenResponse {
	t.Helper()
	code := "code-" + suffix
	now := time.Now().Unix()
	if err := taskStore.InsertNativeOAuthCode(
		context.Background(), sha256.Sum256([]byte(code)), testDesktopClient,
		"Noema Desktop", testDesktopRedirect, testVerifierChallenge, now, now+600,
	); err != nil {
		t.Fatal(err)
	}
	request := oauthFormRequest("/oauth/token", url.Values{
		"grant_type":    {"authorization_code"},
		"client_id":     {testDesktopClient},
		"redirect_uri":  {testDesktopRedirect},
		"code":          {code},
		"code_verifier": {testVerifier},
	})
	return decodeNativeTokens(t, serve(server.Handler(http.NotFoundHandler()), request))
}

func decodeNativeTokens(t *testing.T, response *httptest.ResponseRecorder) nativeTokenResponse {
	t.Helper()
	if response.Code != http.StatusOK {
		t.Fatalf("native token response = %d %s", response.Code, response.Body.String())
	}
	var tokens nativeTokenResponse
	if err := json.Unmarshal(response.Body.Bytes(), &tokens); err != nil {
		t.Fatal(err)
	}
	if tokens.AccessToken == "" || tokens.RefreshToken == "" || tokens.ExpiresIn < 0 {
		t.Fatalf("native token payload = %#v", tokens)
	}
	return tokens
}

func TestLocalSocketDefaultsAndDisable(t *testing.T) {
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	for _, value := range []string{"", "false", "true"} {
		document := "web: {}\n"
		if value != "" {
			document = "web:\n  local_graphql_socket: " + value + "\n"
		}
		if err := home.AtomicWritePrivate(paths.Config(), []byte(document)); err != nil {
			t.Fatal(err)
		}
		config, _, err := LoadConfig(paths, "127.0.0.1:3737")
		if err != nil {
			t.Fatal(err)
		}
		want := value == "true" || value == "" && runtime.GOOS != "windows"
		if config.LocalGraphQLSocket != want || config.DevNoAuth {
			t.Fatalf("socket setting %q: socket=%v bypass=%v", value, config.LocalGraphQLSocket, config.DevNoAuth)
		}
	}
	t.Setenv("NOEMA_WEB__LOCAL_GRAPHQL_SOCKET", "false")
	config, _, err := LoadConfig(paths, "127.0.0.1:3737")
	if err != nil || config.LocalGraphQLSocket {
		t.Fatalf("environment disable: %v", err)
	}
}
