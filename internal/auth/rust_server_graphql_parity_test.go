package auth_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/coder/websocket"
	webauthnlib "github.com/go-webauthn/webauthn/webauthn"
	noemaartifact "github.com/kpsuperplane/noema/internal/artifact"
	noemaauth "github.com/kpsuperplane/noema/internal/auth"
	noemagraphql "github.com/kpsuperplane/noema/internal/graphql"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	webserver "github.com/kpsuperplane/noema/internal/web"
)

const (
	parityAuthority = "localhost:3737"
	parityOrigin    = "http://localhost:3737"
	parityClient    = "noema-desktop:abcdefghijklmnop"
	parityRedirect  = "http://127.0.0.1:49152/oauth/callback"
	parityVerifier  = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
	parityChallenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
	parityRecovery  = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
)

// Rust source: crates/noema-server/src/web/router/tests.rs::development_auth_bypass_allows_graphql_without_bootstrap.
func TestRustServer_development_auth_bypass_allows_graphql_without_bootstrap(t *testing.T) {
	server, _, paths := newExternalAuthTest(t, true)
	application := externalGraphQLApplication(t, server, paths, false)
	handler := server.Handler(application)
	graphql := externalRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	response := externalServe(handler, graphql)
	var payload map[string]any
	if err := json.Unmarshal(response.Body.Bytes(), &payload); err != nil || response.Code != http.StatusOK ||
		!reflect.DeepEqual(payload, map[string]any{"data": map[string]any{"__typename": "QueryRoot"}}) {
		t.Fatalf("development GraphQL = %d %q (%v)", response.Code, response.Body.String(), err)
	}
	recovery := externalRequest(http.MethodPost, "/auth/recovery", bytes.NewBufferString(fmt.Sprintf(`{"code":%q}`, parityRecovery)))
	if response := externalServe(handler, recovery); response.Code != http.StatusNotFound {
		t.Fatalf("development recovery = %d", response.Code)
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::passkey_management_requires_auth_and_revokes_affected_sessions.
func TestRustServer_passkey_management_requires_auth_and_revokes_affected_sessions(t *testing.T) {
	server, database, paths := newExternalAuthTest(t, false)
	second := store.HumanPasskey{CredentialID: "second-passkey", CredentialJSON: `{"test":true}`}
	oldDigest, newDigest := externalDigest(), externalDigest()
	if err := database.CreateAnonymousSession(context.Background(), oldDigest, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := database.RegisterPasskey(context.Background(), second, store.RegistrationInitial, oldDigest, newDigest, time.Now()); err != nil {
		t.Fatal(err)
	}
	handler := server.TestHandler(http.NotFoundHandler())
	unauthenticatedRemove := externalRequest(http.MethodPost, "/auth/passkey/remove", bytes.NewBufferString(fmt.Sprintf(`{"credentialId":%q}`, second.CredentialID)))
	if response := externalServe(handler, unauthenticatedRemove); response.Code != http.StatusForbidden {
		t.Fatalf("unauthenticated passkey removal = %d", response.Code)
	}
	firstCookie := authenticateExternal(t, handler, server)
	list := externalRequest(http.MethodGet, "/auth/passkeys", nil)
	list.AddCookie(firstCookie)
	listResponse := externalServe(handler, list)
	if listResponse.Code != http.StatusOK || strings.TrimSpace(listResponse.Body.String()) != `[{"credentialId":"`+second.CredentialID+`","current":false},{"credentialId":"test-passkey","current":true}]` {
		t.Fatalf("passkey list = %d %s", listResponse.Code, listResponse.Body.String())
	}
	started, returned := make(chan struct{}), make(chan struct{})
	graphqlApplication := externalGraphQLApplication(t, server, paths, false)
	var startedOnce sync.Once
	application := http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if _, ok := noemaauth.BrowserSessionHash(request.Context()); !ok {
			t.Error("authenticated request lost browser binding")
		}
		if isExternalWebSocket(request) {
			startedOnce.Do(func() { close(started) })
			go func() {
				<-request.Context().Done()
				close(returned)
			}()
		}
		graphqlApplication.ServeHTTP(w, request)
	})
	public := server.Handler(application)
	connection, stop := dialExternalWebSocket(t, public, firstCookie.Value, "")
	defer stop()
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("affected passkey session WebSocket did not start")
	}
	if err := connection.Write(context.Background(), websocket.MessageText, []byte(`{"type":"connection_init"}`)); err != nil {
		t.Fatal(err)
	}
	if _, payload, err := connection.Read(context.Background()); err != nil || string(payload) != `{"type":"connection_ack"}` {
		t.Fatalf("passkey WebSocket acknowledgement = %q, %v", payload, err)
	}
	remove := externalRequest(http.MethodPost, "/auth/passkey/remove", bytes.NewBufferString(fmt.Sprintf(`{"credentialId":%q}`, "test-passkey")))
	remove.AddCookie(firstCookie)
	if response := externalServe(public, remove); response.Code != http.StatusNoContent {
		t.Fatalf("remove first passkey = %d %s", response.Code, response.Body.String())
	}
	select {
	case <-returned:
	case <-time.After(time.Second):
		t.Fatal("removed passkey did not revoke its WebSocket session")
	}
	unauthenticated := externalRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	unauthenticated.AddCookie(firstCookie)
	if response := externalServe(public, unauthenticated); response.Code != http.StatusUnauthorized {
		t.Fatalf("unauthenticated GraphQL = %d", response.Code)
	}
	firstFresh := authenticateExternal(t, handler, server)
	secondFresh := authenticateExternal(t, handler, server)
	authenticated := externalRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	authenticated.AddCookie(firstFresh)
	if response := externalServe(public, authenticated); response.Code != http.StatusOK {
		t.Fatalf("authenticated GraphQL = %d", response.Code)
	}
	logoutAll := externalRequest(http.MethodPost, "/auth/logout/all", nil)
	logoutAll.AddCookie(firstFresh)
	if response := externalServe(public, logoutAll); response.Code != http.StatusNoContent {
		t.Fatalf("logout all = %d", response.Code)
	}
	authenticated = externalRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	authenticated.AddCookie(secondFresh)
	if response := externalServe(public, authenticated); response.Code != http.StatusUnauthorized {
		t.Fatalf("logout-all session = %d", response.Code)
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::router_serves_schema_graphiql_and_spa_fallback.
func TestRustServer_router_serves_schema_graphiql_and_spa_fallback(t *testing.T) {
	directory := t.TempDir()
	for name, body := range map[string]string{
		"graphiql.html": "<title>Noema GraphiQL</title><script src=\"/assets/app.js\"></script>",
		"index.html":    "<html>app</html>",
	} {
		if err := os.WriteFile(filepath.Join(directory, name), []byte(body), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	t.Setenv("NOEMA_DEV_ASSET_DIR", directory)
	server, _, paths := newExternalAuthTest(t, true)
	disabled := externalGraphQLApplication(t, server, paths, false)
	disabledHandler := server.Handler(disabled)
	if response := externalParityRequest(disabledHandler, http.MethodGet, "/graphql"); response.Code != http.StatusNotFound {
		t.Fatalf("disabled GraphiQL = %d", response.Code)
	}
	enabled := externalGraphQLApplication(t, server, paths, true)
	handler := server.Handler(enabled)
	request := externalRequest(http.MethodGet, "/graphql", nil)
	response := externalServe(handler, request)
	if response.Code != http.StatusOK || response.Header().Get("Content-Type") != "text/html; charset=utf-8" ||
		response.Header().Get("Content-Security-Policy") != "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'" ||
		!strings.Contains(response.Body.String(), "Noema GraphiQL") || !strings.Contains(response.Body.String(), "/assets/") || strings.Contains(response.Body.String(), "https://") {
		t.Fatalf("GraphiQL response = %d %q", response.Code, response.Body.String())
	}
	if response := externalServe(handler, externalRequest(http.MethodGet, "/assets/graphiql.html", nil)); response.Code != http.StatusNotFound {
		t.Fatalf("GraphiQL asset route = %d", response.Code)
	}
	schema := externalServe(handler, externalRequest(http.MethodGet, "/graphql/schema.graphql", nil))
	if schema.Code != http.StatusOK || schema.Header().Get("Content-Type") != "text/plain; charset=utf-8" ||
		schema.Header().Get("Cache-Control") != "no-store" || schema.Body.Len() == 0 || !strings.Contains(schema.Body.String(), "type QueryRoot") {
		t.Fatalf("schema response = %d %q %#v", schema.Code, schema.Body.String(), schema.Header())
	}
	spa := externalServe(handler, externalRequest(http.MethodGet, "/memory/thread", nil))
	if spa.Code != http.StatusOK || spa.Header().Get("Content-Type") != "text/html; charset=utf-8" || spa.Body.Len() == 0 || spa.Body.String() != "<html>app</html>" {
		t.Fatalf("SPA response = %d %q", spa.Code, spa.Body.String())
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::authenticated_http_and_websocket_ignore_client_identity_metadata.
func TestRustServer_authenticated_http_and_websocket_ignore_client_identity_metadata(t *testing.T) {
	server, _, paths := newExternalAuthTest(t, false)
	public := server.TestHandler(http.NotFoundHandler())
	cookie := authenticateExternal(t, public, server)
	var browserBinding atomic.Bool
	started, returned := make(chan struct{}), make(chan struct{})
	var startedOnce sync.Once
	var returnedOnce sync.Once
	graphqlApplication := externalGraphQLApplication(t, server, paths, false)
	application := http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if noemaauth.ClientID(request.Context()) != "" {
			t.Errorf("browser request received native client identity %q", noemaauth.ClientID(request.Context()))
		}
		if _, ok := noemaauth.BrowserSessionHash(request.Context()); ok {
			browserBinding.Store(true)
		}
		if isExternalWebSocket(request) {
			startedOnce.Do(func() { close(started) })
			go func() {
				<-request.Context().Done()
				returnedOnce.Do(func() { close(returned) })
			}()
		}
		graphqlApplication.ServeHTTP(w, request)
	})
	handler := server.Handler(application)
	graphqlRequest := externalRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ task(taskId: \"task:transport\") { taskId } }","extensions":{"principal":{"subjectId":"attacker"}}}`))
	graphqlRequest.AddCookie(cookie)
	if response := externalServe(handler, graphqlRequest); response.Code != http.StatusOK || response.Header().Get("Cache-Control") != "no-store" ||
		response.Body.String() != `{"errors":[{"message":"Noema store is unavailable","path":["task"],"locations":[{"line":1,"column":3}]}],"data":null}` {
		t.Fatalf("authenticated HTTP = %d %#v %q", response.Code, response.Header(), response.Body.String())
	}
	connection, stop := dialExternalWebSocket(t, handler, cookie.Value, "")
	defer stop()
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("browser WebSocket did not start")
	}
	if err := connection.Write(context.Background(), websocket.MessageText, []byte(`{"type":"connection_init","payload":{"principal":{"subjectId":"attacker"}}}`)); err != nil {
		t.Fatal(err)
	}
	if _, payload, err := connection.Read(context.Background()); err != nil || string(payload) != `{"type":"connection_ack"}` {
		t.Fatalf("browser connection ack = %q, %v", payload, err)
	}
	if err := connection.Write(context.Background(), websocket.MessageText, []byte(`{"id":"principal","type":"subscribe","payload":{"query":"subscription { tasksEvents(workspaceId: \"workspace:personal\") { cursor } }","extensions":{"principal":{"subjectId":"attacker"}}}}`)); err != nil {
		t.Fatal(err)
	}
	if _, payload, err := connection.Read(context.Background()); err != nil ||
		string(payload) != `{"payload":{"errors":[{"message":"Noema store is unavailable","path":["tasksEvents"],"locations":[{"line":1,"column":16}]}],"data":null},"id":"principal","type":"next"}` {
		t.Fatalf("browser subscription result = %q, %v", payload, err)
	}
	if _, payload, err := connection.Read(context.Background()); err != nil || string(payload) != `{"id":"principal","type":"complete"}` {
		t.Fatalf("browser subscription completion = %q, %v", payload, err)
	}
	logout := externalRequest(http.MethodPost, "/auth/logout", nil)
	logout.AddCookie(cookie)
	if response := externalServe(handler, logout); response.Code != http.StatusNoContent {
		t.Fatalf("browser logout = %d", response.Code)
	}
	select {
	case <-returned:
	case <-time.After(time.Second):
		t.Fatal("browser session revocation did not close WebSocket")
	}
	assertExternalWebSocketClosed(t, connection)
	if !browserBinding.Load() {
		t.Fatal("browser application request lost its session binding")
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::client_bearer_authorizes_http_and_ws_without_browser_origin_and_revocation_closes_ws.
func TestRustServer_client_bearer_authorizes_http_and_ws_without_browser_origin_and_revocation_closes_ws(t *testing.T) {
	server, database, paths := newExternalAuthTest(t, false)
	setupAccess := seedNativeAccess(t, database, "setup")
	setupRequest := externalRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	setupRequest.Header.Del("Origin")
	setupRequest.Header.Set("Authorization", "Bearer "+setupAccess)
	if response := externalServe(server.Handler(http.NotFoundHandler()), setupRequest); response.Code != http.StatusForbidden {
		t.Fatalf("setup-barrier bearer request = %d %s", response.Code, response.Body.String())
	}
	public := server.TestHandler(http.NotFoundHandler())
	_ = authenticateExternal(t, public, server)
	access := seedNativeAccess(t, database, "active")
	faviconHandler := webserver.NewFaviconHandler(t.TempDir())
	favicon := []byte("png")
	faviconHandler.Seed("example.com", favicon)
	started, returned := make(chan struct{}), make(chan struct{})
	var startedOnce sync.Once
	var returnedOnce sync.Once
	graphqlApplication := externalGraphQLApplication(t, server, paths, false)
	application := http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if noemaauth.ClientID(request.Context()) != parityClient {
			t.Errorf("native client context = %q", noemaauth.ClientID(request.Context()))
		}
		if isExternalWebSocket(request) {
			startedOnce.Do(func() { close(started) })
			go func() {
				<-request.Context().Done()
				returnedOnce.Do(func() { close(returned) })
			}()
		}
		graphqlApplication.ServeHTTP(w, request)
	})
	mux := http.NewServeMux()
	mux.Handle("GET /favicons/{hostname}", faviconHandler)
	mux.Handle("/", application)
	artifactPaths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	artifactRoot, err := artifactPaths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = artifactRoot.Close() })
	artifactDatabase, err := store.Open(context.Background(), artifactPaths.Database())
	if err != nil {
		t.Fatal(err)
	}
	artifacts, err := noemaartifact.New(artifactRoot, artifactDatabase, nil)
	if err != nil {
		_ = artifactDatabase.Close()
		t.Fatal(err)
	}
	// Rust's GraphQL fixture has no filesystem Artifact service. Mount the
	// production Go route, then close its store to preserve that unavailable
	// service result after bearer authentication reaches the route.
	if err := artifactDatabase.Close(); err != nil {
		t.Fatal(err)
	}
	mux.Handle("/artifacts/versions/", artifacts.Handler())
	handler := server.Handler(mux)
	graphqlRequest := externalRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ task(taskId: \"task:transport\") { taskId } }"}`))
	graphqlRequest.Header.Del("Origin")
	graphqlRequest.Header.Set("Authorization", "Bearer "+access)
	response := externalServe(handler, graphqlRequest)
	if response.Code != http.StatusOK || response.Header().Get("Cache-Control") != "no-store" ||
		response.Body.String() != `{"errors":[{"message":"Noema store is unavailable","path":["task"],"locations":[{"line":1,"column":3}]}],"data":null}` {
		t.Fatalf("native GraphQL = %d %#v %q", response.Code, response.Header(), response.Body.String())
	}
	invalid := externalRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	invalid.Header.Del("Origin")
	invalid.Header.Set("Authorization", "Bearer invalid")
	if response := externalServe(handler, invalid); response.Code != http.StatusUnauthorized {
		t.Fatalf("invalid bearer = %d", response.Code)
	}
	faviconRequest := externalRequest(http.MethodGet, "/favicons/example.com", nil)
	faviconRequest.Header.Set("Authorization", "Bearer "+access)
	if response := externalServe(handler, faviconRequest); response.Code != http.StatusOK || !bytes.Equal(response.Body.Bytes(), favicon) {
		t.Fatalf("native favicon = %d %q", response.Code, response.Body.String())
	}
	browser := externalRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	browser.Header.Del("Origin")
	if response := externalServe(handler, browser); response.Code != http.StatusForbidden {
		t.Fatalf("browser request without Origin = %d", response.Code)
	}
	artifactRequest := externalRequest(http.MethodGet, "/artifacts/versions/missing/download", nil)
	artifactRequest.Header.Set("Authorization", "Bearer "+access)
	if response := externalServe(handler, artifactRequest); response.Code != http.StatusInternalServerError {
		t.Fatalf("native artifact = %d %s", response.Code, response.Body.String())
	}
	connection, stop := dialExternalWebSocket(t, handler, "", "Bearer "+access)
	defer stop()
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("native WebSocket did not start")
	}
	if err := connection.Write(context.Background(), websocket.MessageText, []byte(`{"type":"connection_init"}`)); err != nil {
		t.Fatal(err)
	}
	if _, payload, err := connection.Read(context.Background()); err != nil || string(payload) != `{"type":"connection_ack"}` {
		t.Fatalf("native connection ack = %q, %v", payload, err)
	}
	if err := connection.Write(context.Background(), websocket.MessageText, []byte(`{"id":"native","type":"subscribe","payload":{"query":"subscription { tasksEvents(workspaceId: \"workspace:personal\") { cursor } }"}}`)); err != nil {
		t.Fatal(err)
	}
	if _, payload, err := connection.Read(context.Background()); err != nil || !bytes.Contains(payload, []byte(`"tasksEvents"`)) {
		t.Fatalf("native subscription result = %q, %v", payload, err)
	}
	if _, payload, err := connection.Read(context.Background()); err != nil || string(payload) != `{"id":"native","type":"complete"}` {
		t.Fatalf("native subscription completion = %q, %v", payload, err)
	}
	expiringAccess := base64.RawURLEncoding.EncodeToString(bytes.Repeat([]byte{16}, 32))
	expiringNow := time.Now().Unix()
	var expiringRefresh [32]byte
	for index := range expiringRefresh {
		expiringRefresh[index] = 18
	}
	if err := database.InsertNativeOAuthFamily(context.Background(), store.NewNativeOAuthFamily{
		FamilyID:          "1123456789abcdef0123456789abcdef",
		ClientID:          parityClient,
		AccessHash:        sha256.Sum256([]byte(expiringAccess)),
		RefreshHash:       expiringRefresh,
		IssuedAt:          expiringNow,
		AccessExpiresAt:   expiringNow + 3,
		IdleExpiresAt:     expiringNow + 30*24*60*60,
		AbsoluteExpiresAt: expiringNow + 180*24*60*60,
	}); err != nil {
		t.Fatal(err)
	}
	expiringConnection, expiringStop := dialExternalWebSocket(t, handler, "", "Bearer "+expiringAccess)
	defer expiringStop()
	if err := expiringConnection.Write(context.Background(), websocket.MessageText, []byte(`{"type":"connection_init"}`)); err != nil {
		t.Fatal(err)
	}
	if _, payload, err := expiringConnection.Read(context.Background()); err != nil || string(payload) != `{"type":"connection_ack"}` {
		t.Fatalf("expiring connection ack = %q, %v", payload, err)
	}
	expiryContext, cancelExpiry := context.WithTimeout(context.Background(), 5*time.Second)
	_, _, expiryErr := expiringConnection.Read(expiryContext)
	cancelExpiry()
	if expiryErr == nil || errors.Is(expiryErr, context.DeadlineExceeded) {
		t.Fatalf("access expiry close = %v", expiryErr)
	}
	if _, err := server.RevokeClient(context.Background(), parityClient); err != nil {
		t.Fatal(err)
	}
	select {
	case <-returned:
	case <-time.After(time.Second):
		t.Fatal("client revocation did not close WebSocket")
	}
	assertExternalWebSocketClosed(t, connection)
}

func assertExternalWebSocketClosed(t *testing.T, connection *websocket.Conn) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	if _, _, err := connection.Read(ctx); err == nil || errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("WebSocket remained open: %v", err)
	}
}

type externalAuthFixture struct {
	server   *noemaauth.Server
	database *store.Store
	paths    home.Paths
}

func newExternalAuthTest(t *testing.T, devNoAuth bool) (*noemaauth.Server, *store.Store, home.Paths) {
	t.Helper()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	config, recovery, err := noemaauth.LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	config.DevNoAuth = devNoAuth
	database, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	server, err := noemaauth.New(paths, database, config, recovery)
	if err != nil {
		t.Fatal(err)
	}
	return server, database, paths
}

func authenticateExternal(t *testing.T, handler http.Handler, server *noemaauth.Server) *http.Cookie {
	t.Helper()
	response := externalServe(handler, externalRequest(http.MethodPost, "/__test/authenticate", nil))
	if response.Code != http.StatusNoContent {
		t.Fatalf("test authentication = %d %s", response.Code, response.Body.String())
	}
	for _, cookie := range response.Result().Cookies() {
		if cookie.Name == "noema.sid" || cookie.Name == "__Host-noema.sid" {
			return cookie
		}
	}
	t.Fatal("test authentication did not set a session cookie")
	return nil
}

func externalCredential(value byte) store.HumanPasskey {
	encoded, _ := json.Marshal(&webauthnlib.Credential{ID: []byte{value}})
	return store.HumanPasskey{CredentialID: base64.RawURLEncoding.EncodeToString([]byte{value}), CredentialJSON: string(encoded)}
}

func externalDigest() [32]byte {
	var value [32]byte
	_, _ = rand.Read(value[:])
	return value
}

func seedNativeAccess(t *testing.T, database *store.Store, suffix string) string {
	t.Helper()
	seed := sha256.Sum256([]byte(suffix))
	access := base64.RawURLEncoding.EncodeToString(seed[:])
	refreshSeed := sha256.Sum256([]byte(suffix + "-refresh"))
	refresh := base64.RawURLEncoding.EncodeToString(refreshSeed[:])
	now := time.Now().Unix()
	code := "code-" + suffix
	if err := database.InsertNativeOAuthCode(context.Background(), sha256.Sum256([]byte(code)), parityClient, "Noema Desktop", parityRedirect, parityChallenge, now, now+600); err != nil {
		t.Fatal(err)
	}
	family := fmt.Sprintf("%032x", sha256.Sum256([]byte(suffix)))[:32]
	if err := database.ExchangeNativeOAuthCode(context.Background(), sha256.Sum256([]byte(code)), parityClient, parityRedirect, parityVerifier, family, sha256.Sum256([]byte(access)), sha256.Sum256([]byte(refresh)), now); err != nil {
		t.Fatal(err)
	}
	return access
}

func externalGraphQLApplication(t *testing.T, server *noemaauth.Server, paths home.Paths, graphiQL bool) http.Handler {
	t.Helper()
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	resolver := noemagraphql.NewResolver(nil, root, server, nil, nil, nil, nil, nil, nil, nil)
	graphql := noemagraphql.NewHandler(resolver)
	route := webserver.NewGraphQLHandler(graphql, noemagraphql.Schema(), graphiQL)
	assets := webserver.NewAssetHandler()
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasPrefix(r.URL.Path, "/graphql") {
			route.ServeHTTP(w, r)
			return
		}
		assets.ServeHTTP(w, r)
	})
}

func externalRequest(method, path string, body *bytes.Buffer) *http.Request {
	var reader *bytes.Buffer
	if body == nil {
		reader = bytes.NewBuffer(nil)
	} else {
		reader = body
	}
	request := httptest.NewRequest(method, parityOrigin+path, reader)
	request.Host = parityAuthority
	if method == http.MethodPost || path == "/graphql/ws" {
		request.Header.Set("Origin", parityOrigin)
	}
	if body != nil {
		request.Header.Set("Content-Type", "application/json")
	}
	return request
}

func externalServe(handler http.Handler, request *http.Request) *httptest.ResponseRecorder {
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)
	return response
}

func externalParityRequest(handler http.Handler, method, path string) *httptest.ResponseRecorder {
	return externalServe(handler, externalRequest(method, path, nil))
}

func isExternalWebSocket(r *http.Request) bool {
	return r.Method == http.MethodGet && r.URL.Path == "/graphql/ws" && strings.EqualFold(r.Header.Get("Upgrade"), "websocket")
}

func dialExternalWebSocket(t *testing.T, handler http.Handler, cookie, bearer string) (*websocket.Conn, func()) {
	t.Helper()
	network := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		r.Host = parityAuthority
		handler.ServeHTTP(w, r)
	}))
	requestHeaders := make(http.Header)
	if cookie != "" {
		requestHeaders.Set("Origin", parityOrigin)
		requestHeaders.Set("Cookie", "noema.sid="+cookie)
	}
	if bearer != "" {
		requestHeaders.Set("Authorization", bearer)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	connection, response, err := websocket.Dial(ctx, "ws"+strings.TrimPrefix(network.URL, "http")+"/graphql/ws", &websocket.DialOptions{Subprotocols: []string{"graphql-transport-ws"}, HTTPHeader: requestHeaders})
	cancel()
	if err != nil {
		network.Close()
		if response != nil {
			t.Fatalf("WebSocket handshake: %v (%s)", err, response.Status)
		}
		t.Fatal(err)
	}
	return connection, func() {
		_ = connection.CloseNow()
		network.Close()
	}
}
