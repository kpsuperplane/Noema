package auth

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
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
	status := serve(handler, authRequest(http.MethodGet, "/auth/status", nil))
	if status.Code != http.StatusOK || !bytes.Contains(status.Body.Bytes(), []byte(`"setup_ready"`)) {
		t.Fatalf("setup status = %d %s", status.Code, status.Body.String())
	}

	devServer, _, _ := newAuthTest(t, true)
	devHandler := devServer.Handler(application)
	if response := serve(devHandler, authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{}`))); response.Code != http.StatusNoContent {
		t.Fatalf("development GraphQL = %d", response.Code)
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
	application := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
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
