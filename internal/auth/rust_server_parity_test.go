package auth

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"image"
	"image/png"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/coder/websocket"
	noemaadapter "github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/home"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/store"
	webserver "github.com/kpsuperplane/noema/internal/web"
)

// Rust source: crates/noema-server/src/web/authority.rs::bind_ip_requires_an_explicit_numeric_address.
func TestRustServer_bind_ip_requires_an_explicit_numeric_address(t *testing.T) {
	for _, host := range []string{"localhost", "127.0.0.1", "0.0.0.0"} {
		t.Run(host, func(t *testing.T) {
			paths, err := home.FromRoot(t.TempDir())
			if err != nil {
				t.Fatal(err)
			}
			if err := home.AtomicWritePrivate(paths.Config(), []byte("web:\n  host: "+host+"\n")); err != nil {
				t.Fatal(err)
			}
			_, _, err = LoadConfig(paths, "")
			if host == "localhost" {
				if err == nil {
					t.Fatal("hostname bind address was accepted")
				}
				return
			}
			if err != nil {
				t.Fatalf("numeric bind address rejected: %v", err)
			}
		})
	}
}

// Rust source: crates/noema-server/src/web/authority.rs::public_origin_is_exact_https_domain_or_localhost.
func TestRustServer_public_origin_is_exact_https_domain_or_localhost(t *testing.T) {
	deployed, err := canonicalConfig("https://login.noema.example:8443", "noema.example", false)
	if err != nil {
		t.Fatal(err)
	}
	if deployed.Authority != "login.noema.example:8443" ||
		deployed.Origin != "https://login.noema.example:8443" || deployed.RPID != "noema.example" || !deployed.Secure {
		t.Fatalf("deployed authority = %#v", deployed)
	}
	local, err := canonicalConfig("http://localhost:3737", "localhost", false)
	if err != nil {
		t.Fatal(err)
	}
	if local.Secure {
		t.Fatal("localhost was marked secure")
	}
	for _, invalid := range []string{
		"http://noema.example",
		"https://127.0.0.1",
		"https://noema.example/path",
		"https://user@noema.example",
	} {
		if _, err := canonicalConfig(invalid, "noema.example", false); err == nil {
			t.Fatalf("invalid public origin accepted: %s", invalid)
		}
	}
	if _, err := canonicalConfig("https://noema.example", "https://noema.example", false); err == nil {
		t.Fatal("origin was accepted as a passkey RP ID")
	}
}

// Rust source: crates/noema-server/src/web/authority.rs::only_oauth_approval_allows_an_opaque_origin.
func TestRustServer_only_oauth_approval_allows_an_opaque_origin(t *testing.T) {
	server, _, _ := newAuthTest(t, true)
	server.config.Authority = "noema.example"
	server.config.Origin = "https://noema.example"
	handler := server.authorityOnly(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusNoContent)
	}))
	for _, test := range []struct {
		path   string
		origin string
		status int
	}{
		{path: "/oauth/authorize", status: http.StatusNoContent},
		{path: "/oauth/authorize", origin: server.config.Origin, status: http.StatusNoContent},
		{path: "/oauth/authorize", origin: "null", status: http.StatusNoContent},
		{path: "/oauth/authorize", origin: "https://attacker.example", status: http.StatusForbidden},
		{path: "/auth/logout", status: http.StatusForbidden},
		{path: "/auth/logout", origin: "null", status: http.StatusForbidden},
	} {
		request := httptest.NewRequest(http.MethodPost, "https://noema.example"+test.path, nil)
		request.Host = server.config.Authority
		if test.origin != "" {
			request.Header.Set("Origin", test.origin)
		}
		response := serve(handler, request)
		if response.Code != test.status {
			t.Fatalf("%s origin %q response = %d, want %d", test.path, test.origin, response.Code, test.status)
		}
	}
}

// Rust source: crates/noema-server/src/web/mod.rs::websocket_capacity_rejects_excess_work.
func TestRustServer_websocket_capacity_rejects_excess_work(t *testing.T) {
	server, _, _ := newAuthTest(t, false)
	token, _, _ := seedPasskey(t, server, 30)
	server.wsSlots = make(chan struct{}, 1)
	started := make(chan struct{})
	released := make(chan struct{}, 2)
	var accepted atomic.Int32
	var startedOnce sync.Once
	application := http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		accepted.Add(1)
		connection, err := websocket.Accept(w, request, &websocket.AcceptOptions{Subprotocols: []string{"graphql-transport-ws"}})
		if err != nil {
			t.Errorf("WebSocket handshake: %v", err)
			return
		}
		defer connection.CloseNow()
		startedOnce.Do(func() { close(started) })
		_, _, _ = connection.Read(context.Background())
	})
	network := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		before := accepted.Load()
		server.Handler(application).ServeHTTP(w, request)
		if accepted.Load() > before {
			released <- struct{}{}
		}
	}))
	t.Cleanup(network.Close)
	server.config.Authority = strings.TrimPrefix(network.URL, "http://")
	server.config.Origin = network.URL
	cookie := server.sessions.cookieName + "=" + token
	first, response, err := websocket.Dial(context.Background(), "ws"+strings.TrimPrefix(network.URL, "http")+"/graphql/ws", &websocket.DialOptions{
		Subprotocols: []string{"graphql-transport-ws"}, HTTPHeader: http.Header{"Cookie": []string{cookie}, "Origin": []string{server.config.Origin}},
	})
	if err != nil {
		t.Fatalf("first WebSocket handshake: %v (%v)", err, response)
	}
	t.Cleanup(func() { _ = first.CloseNow() })
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("first WebSocket did not start")
	}
	_, response, err = websocket.Dial(context.Background(), "ws"+strings.TrimPrefix(network.URL, "http")+"/graphql/ws", &websocket.DialOptions{
		Subprotocols: []string{"graphql-transport-ws"}, HTTPHeader: http.Header{"Cookie": []string{cookie}, "Origin": []string{server.config.Origin}},
	})
	if err == nil || response == nil || response.StatusCode != http.StatusServiceUnavailable {
		t.Fatalf("full WebSocket capacity = %v (%v)", response, err)
	}
	_ = first.CloseNow()
	select {
	case <-released:
	case <-time.After(time.Second):
		t.Fatal("released WebSocket did not finish")
	}
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	third, response, err := websocket.Dial(ctx, "ws"+strings.TrimPrefix(network.URL, "http")+"/graphql/ws", &websocket.DialOptions{
		Subprotocols: []string{"graphql-transport-ws"}, HTTPHeader: http.Header{"Cookie": []string{cookie}, "Origin": []string{server.config.Origin}},
	})
	if err != nil {
		t.Fatalf("released WebSocket capacity = %v (%v)", response, err)
	}
	_ = third.CloseNow()
}

// Rust source: crates/noema-server/src/web/native_oauth.rs::authorization_requires_s256_pkce_and_client_state.
func TestRustServer_authorization_requires_s256_pkce_and_client_state(t *testing.T) {
	valid := nativeAuthorizationQuery(testDesktopClient, testDesktopRedirect, "s")
	if _, err := parseAuthorizationRequest(valid); err != nil {
		t.Fatal(err)
	}
	for _, missing := range []string{"code_challenge", "code_challenge_method", "state"} {
		values, err := url.ParseQuery(valid)
		if err != nil {
			t.Fatal(err)
		}
		values.Del(missing)
		if _, err := parseAuthorizationRequest(values.Encode()); err == nil {
			t.Fatalf("request without %s was accepted", missing)
		}
	}
	values, err := url.ParseQuery(valid)
	if err != nil {
		t.Fatal(err)
	}
	values.Set("code_challenge_method", "plain")
	if _, err := parseAuthorizationRequest(values.Encode()); err == nil {
		t.Fatal("plain PKCE was accepted")
	}
}

// Rust source: crates/noema-server/src/web/native_oauth.rs::native_redirects_are_platform_bound.
func TestRustServer_native_redirects_are_platform_bound(t *testing.T) {
	if !validClientAndRedirect(testDesktopClient, testDesktopRedirect) {
		t.Fatal("desktop redirect was rejected")
	}
	for _, redirect := range []string{
		"http://localhost:49152/oauth/callback",
		"http://127.0.0.1:49152/other",
	} {
		if validClientAndRedirect(testDesktopClient, redirect) {
			t.Fatalf("desktop redirect accepted: %s", redirect)
		}
	}
	iosClient := "noema-ios:abcdefghijklmnop"
	if !validClientAndRedirect(iosClient, "noema://oauth/callback") {
		t.Fatal("iOS redirect was rejected")
	}
	if validClientAndRedirect(iosClient, "https://attacker.example/callback") {
		t.Fatal("attacker iOS redirect was accepted")
	}
}

// Rust source: crates/noema-server/src/web/native_oauth.rs::request_bound_retry_survives_delay_and_is_consumed_by_its_successor.
func TestRustServer_request_bound_retry_survives_delay_and_is_consumed_by_its_successor(t *testing.T) {
	path := filepath.Join(t.TempDir(), "native-oauth-retries.json")
	oldRefresh := sha256.Sum256([]byte("old-refresh"))
	store := nativeRetryStore{path: path}
	if err := store.save(oldRefresh, strings.Repeat("a", 32), nativeRetryEntry{
		IssuedAt: 100, AccessToken: "access", RefreshToken: "successor-refresh", RetainUntil: 10_000,
	}, 100); err != nil {
		t.Fatal(err)
	}
	reopened := nativeRetryStore{path: path}
	if entry, err := reopened.load(oldRefresh, strings.Repeat("a", 32), 9_000); err != nil || entry == nil {
		t.Fatalf("matching retry = %#v, %v", entry, err)
	}
	if entry, err := reopened.load(oldRefresh, strings.Repeat("b", 32), 9_000); err != nil || entry != nil {
		t.Fatalf("mismatched retry = %#v, %v", entry, err)
	}
	successor := sha256.Sum256([]byte("successor-refresh"))
	if err := reopened.save(successor, strings.Repeat("c", 32), nativeRetryEntry{
		IssuedAt: 9_000, AccessToken: "next-access", RefreshToken: "next-refresh", RetainUntil: 10_000,
	}, 9_000); err != nil {
		t.Fatal(err)
	}
	if entry, err := reopened.load(oldRefresh, strings.Repeat("a", 32), 9_000); err != nil || entry != nil {
		t.Fatalf("consumed retry = %#v, %v", entry, err)
	}
}

// Rust source: crates/noema-server/src/web/native_oauth.rs::code_exchange_refresh_rotation_and_replay_are_end_to_end.
func TestRustServer_code_exchange_refresh_rotation_and_replay_are_end_to_end(t *testing.T) {
	server, taskStore, _ := newAuthTest(t, false)
	_, _, _ = seedPasskey(t, server, 34)
	tokens := exchangeNativeTokens(t, server, taskStore, "rust-parity")
	if tokens.AccessToken == "" || tokens.RefreshToken == "" {
		t.Fatal("code exchange did not issue both tokens")
	}
	requestID := strings.Repeat("a", 32)
	values := url.Values{
		"grant_type": {"refresh_token"}, "refresh_token": {tokens.RefreshToken}, "refresh_request_id": {requestID},
	}
	first := decodeNativeTokens(t, serve(server.Handler(http.NotFoundHandler()), oauthFormRequest("/oauth/token", values)))
	accessHash := sha256.Sum256([]byte(tokens.AccessToken))
	access, exists, err := taskStore.ActiveNativeOAuthAccess(context.Background(), accessHash, time.Now().Unix())
	if err != nil || !exists || access.ClientID != testDesktopClient {
		t.Fatalf("issued access validation = %#v, exists=%v, err=%v", access, exists, err)
	}
	replay := decodeNativeTokens(t, serve(server.Handler(http.NotFoundHandler()), oauthFormRequest("/oauth/token", values)))
	if first.AccessToken != replay.AccessToken || first.RefreshToken != replay.RefreshToken {
		t.Fatal("request-bound replay did not return the rotated successor")
	}
	rotated := url.Values{
		"grant_type": {"refresh_token"}, "refresh_token": {first.RefreshToken}, "refresh_request_id": {strings.Repeat("b", 32)},
	}
	if response := serve(server.Handler(http.NotFoundHandler()), oauthFormRequest("/oauth/token", rotated)); response.Code != http.StatusOK {
		t.Fatalf("successor refresh = %d %s", response.Code, response.Body.String())
	}
	invalid := serve(server.Handler(http.NotFoundHandler()), oauthFormRequest("/oauth/token", values))
	if invalid.Code != http.StatusBadRequest || !strings.Contains(invalid.Body.String(), "invalid_grant") {
		t.Fatalf("replayed refresh after successor = %d %s", invalid.Code, invalid.Body.String())
	}
	if _, exists, err := taskStore.ActiveNativeOAuthAccess(context.Background(), sha256.Sum256([]byte(tokens.AccessToken)), time.Now().Unix()); err != nil || exists {
		t.Fatalf("rotated access remained active: exists=%v, err=%v", exists, err)
	}
}

// Rust source: crates/noema-server/src/web/passkey.rs::ceremony_is_session_bound_and_consumed_once.
func TestRustServer_ceremony_is_session_bound_and_consumed_once(t *testing.T) {
	server, _, _ := newAuthTest(t, false)
	handler := server.Handler(http.NotFoundHandler())
	id, expectedCookie := startRegistrationCeremony(t, handler, server, nil)
	_, differentCookie := startRegistrationCeremony(t, handler, server, nil)
	expectedRequest := authRequest(http.MethodGet, "/auth/status", nil)
	expectedRequest.AddCookie(expectedCookie)
	expected, exists, err := server.sessions.current(expectedRequest, false)
	if err != nil || !exists {
		t.Fatalf("expected ceremony session = %#v, exists=%v, err=%v", expected, exists, err)
	}
	differentRequest := authRequest(http.MethodGet, "/auth/status", nil)
	differentRequest.AddCookie(differentCookie)
	different, exists, err := server.sessions.current(differentRequest, false)
	if err != nil || !exists {
		t.Fatalf("different ceremony session = %#v, exists=%v, err=%v", different, exists, err)
	}
	if _, err := server.passkeys.take(id, different.digest, registrationCeremony); !errors.Is(err, errInvalidCeremony) {
		t.Fatalf("different browser error = %v", err)
	}
	if got, err := server.passkeys.take(id, expected.digest, registrationCeremony); err != nil || got.kind != registrationCeremony {
		t.Fatalf("bound browser take = %#v, %v", got, err)
	}
	if _, err := server.passkeys.take(id, expected.digest, registrationCeremony); !errors.Is(err, errInvalidCeremony) {
		t.Fatalf("reused ceremony error = %v", err)
	}
}

// Rust source: crates/noema-server/src/web/passkey.rs::full_ceremony_registry_rejects_new_work_without_evicting_active_work.
func TestRustServer_full_ceremony_registry_rejects_new_work_without_evicting_active_work(t *testing.T) {
	server, _, _ := newAuthTest(t, false)
	handler := server.Handler(http.NotFoundHandler())
	first := ""
	var firstCookie *http.Cookie
	for index := range ceremonyCapacity {
		id, cookie := startRegistrationCeremony(t, handler, server, nil)
		if index == 0 {
			first = id
			firstCookie = cookie
		}
	}
	extra := serve(handler, authRequest(http.MethodPost, "/auth/passkey/register/start", nil))
	if extra.Code != http.StatusServiceUnavailable || !strings.Contains(extra.Body.String(), "ceremony_unavailable") {
		t.Fatalf("full ceremony registry response = %d %s", extra.Code, extra.Body.String())
	}
	firstRequest := authRequest(http.MethodGet, "/auth/status", nil)
	firstRequest.AddCookie(firstCookie)
	browser, exists, err := server.sessions.current(firstRequest, false)
	if err != nil || !exists {
		t.Fatalf("first ceremony session = %#v, exists=%v, err=%v", browser, exists, err)
	}
	if _, err := server.passkeys.take(first, browser.digest, registrationCeremony); err != nil {
		t.Fatalf("active ceremony was evicted: %v", err)
	}
}

// Rust source: crates/noema-server/src/web/session.rs::browser_cookie_key_survives_session_security_reconstruction.
func TestRustServer_browser_cookie_key_survives_session_security_reconstruction(t *testing.T) {
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	databasePath := paths.Database()
	firstStore, err := store.Open(context.Background(), databasePath)
	if err != nil {
		t.Fatal(err)
	}
	first, err := newSessionSecurity(paths, firstStore, false)
	if err != nil {
		_ = firstStore.Close()
		t.Fatal(err)
	}
	expected := append([]byte(nil), first.key...)
	if err := firstStore.Close(); err != nil {
		t.Fatal(err)
	}
	restartedStore, err := store.Open(context.Background(), databasePath)
	if err != nil {
		t.Fatal(err)
	}
	defer restartedStore.Close()
	restarted, err := newSessionSecurity(paths, restartedStore, false)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(restarted.key, expected) {
		t.Fatal("browser cookie key changed after reconstruction")
	}
}

// Rust source: crates/noema-server/src/web/session_store/tests.rs::capacity_rejects_new_sessions_without_evicting_active_sessions.
func TestRustServer_capacity_rejects_new_sessions_without_evicting_active_sessions(t *testing.T) {
	_, taskStore, _ := newAuthTest(t, false)
	if err := taskStore.SetBrowserSessionCapacityForTesting(1); err != nil {
		t.Fatal(err)
	}
	now := time.Now().UTC()
	first := testAuthDigest("first-session")
	if err := taskStore.CreateAnonymousSession(context.Background(), first, now); err != nil {
		t.Fatal(err)
	}
	if err := taskStore.CreateAnonymousSession(context.Background(), testAuthDigest("overflow-session"), now); !errors.Is(err, store.ErrSessionFull) {
		t.Fatalf("overflow session error = %v", err)
	}
	if _, exists, err := taskStore.BrowserSession(context.Background(), first, now, false); err != nil || !exists {
		t.Fatalf("first session = %v, %v", exists, err)
	}
}

// Rust source: crates/noema-server/src/web/session_store/tests.rs::expiry_cleanup_removes_sessions_and_announces_revocation.
func TestRustServer_expiry_cleanup_removes_sessions_and_announces_revocation(t *testing.T) {
	server, taskStore, _ := newAuthTest(t, false)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	revocations := taskStore.SubscribeBrowserSessionRevocations(ctx)
	now := time.Now().UTC()
	digest := testAuthDigest("expired-session")
	if err := taskStore.CreateAnonymousSession(context.Background(), digest, now); err != nil {
		t.Fatal(err)
	}
	digests, err := taskStore.DeleteExpiredBrowserSessions(context.Background(), now.Add(5*time.Minute+time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if len(digests) != 1 || digests[0] != digest {
		t.Fatalf("expired digests = %x, want %x", digests, digest)
	}
	if _, exists, err := taskStore.BrowserSession(context.Background(), digest, now, false); err != nil || exists {
		t.Fatalf("expired session = %v, %v", exists, err)
	}
	revoked := make(chan struct{})
	server.sessions.registerConnection(digest, func() { close(revoked) })
	select {
	case event := <-revocations:
		if event != digest {
			t.Fatalf("expired session revocation = %x, want %x", event, digest)
		}
		server.sessions.revoke(event)
	case <-time.After(time.Second):
		t.Fatal("store did not announce expired session revocation")
	}
	select {
	case <-revoked:
	default:
		t.Fatal("expired session did not announce revocation")
	}
}

// Rust source: crates/noema-server/src/web/session_store/tests.rs::deletion_is_targeted_and_announces_revocation.
func TestRustServer_deletion_is_targeted_and_announces_revocation(t *testing.T) {
	server, taskStore, _ := newAuthTest(t, false)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	revocations := taskStore.SubscribeBrowserSessionRevocations(ctx)
	now := time.Now().UTC()
	first, second := testAuthDigest("first-session"), testAuthDigest("second-session")
	for _, digest := range [][32]byte{first, second} {
		if err := taskStore.CreateAnonymousSession(context.Background(), digest, now); err != nil {
			t.Fatal(err)
		}
	}
	if err := taskStore.DeleteBrowserSession(context.Background(), first); err != nil {
		t.Fatal(err)
	}
	if _, exists, err := taskStore.BrowserSession(context.Background(), first, now, false); err != nil || exists {
		t.Fatalf("deleted session = %v, %v", exists, err)
	}
	if _, exists, err := taskStore.BrowserSession(context.Background(), second, now, false); err != nil || !exists {
		t.Fatalf("unrelated session = %v, %v", exists, err)
	}
	firstRevoked, secondRevoked := make(chan struct{}), make(chan struct{})
	server.sessions.registerConnection(first, func() { close(firstRevoked) })
	server.sessions.registerConnection(second, func() { close(secondRevoked) })
	select {
	case event := <-revocations:
		if event != first {
			t.Fatalf("deleted session revocation = %x, want %x", event, first)
		}
		server.sessions.revoke(event)
	case <-time.After(time.Second):
		t.Fatal("store did not announce deleted session revocation")
	}
	select {
	case <-firstRevoked:
	default:
		t.Fatal("deleted session did not announce revocation")
	}
	select {
	case <-secondRevoked:
		t.Fatal("targeted deletion revoked the unrelated session")
	default:
	}
	server.sessions.revoke(second)
}

// Rust source: crates/noema-server/src/web/session_store/tests.rs::persistent_store_loads_the_same_session_after_reconstruction.
func TestRustServer_persistent_store_loads_the_same_session_after_reconstruction(t *testing.T) {
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	databasePath := paths.Database()
	firstStore, err := store.Open(context.Background(), databasePath)
	if err != nil {
		t.Fatal(err)
	}
	now := time.Now().UTC()
	oldDigest := testAuthDigest("persistent-old-session")
	digest := testAuthDigest("persistent-session")
	if err := firstStore.CreateAnonymousSession(context.Background(), oldDigest, now); err != nil {
		_ = firstStore.Close()
		t.Fatal(err)
	}
	credential := authCredential(t, 52)
	if err := firstStore.RegisterPasskey(context.Background(), credential, store.RegistrationInitial, oldDigest, digest, now); err != nil {
		_ = firstStore.Close()
		t.Fatal(err)
	}
	saved, exists, err := firstStore.BrowserSession(context.Background(), digest, now, false)
	if err != nil || !exists {
		_ = firstStore.Close()
		t.Fatalf("saved session = %#v, exists=%v, err=%v", saved, exists, err)
	}
	if err := firstStore.Close(); err != nil {
		t.Fatal(err)
	}
	restarted, err := store.Open(context.Background(), databasePath)
	if err != nil {
		t.Fatal(err)
	}
	defer restarted.Close()
	loaded, exists, err := restarted.BrowserSession(context.Background(), digest, now, false)
	if err != nil || !exists {
		t.Fatalf("reconstructed session = %#v, %v, %v", loaded, exists, err)
	}
	if loaded.State != saved.State || loaded.PasskeyID != saved.PasskeyID ||
		!loaded.RecentPasskeyAt.Equal(saved.RecentPasskeyAt) ||
		!loaded.CreatedAt.Equal(saved.CreatedAt) || !loaded.ExpiresAt.Equal(saved.ExpiresAt) {
		t.Fatalf("reconstructed record = %#v, saved=%#v", loaded, saved)
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::favicon_route_requires_authentication_and_serves_cached_images.
func TestRustServer_favicon_route_requires_authentication_and_serves_cached_images(t *testing.T) {
	server, _, _ := newAuthTest(t, false)
	token, _, _ := seedPasskey(t, server, 31)
	faviconHandler := webserver.NewFaviconHandler(t.TempDir())
	var faviconEncoded bytes.Buffer
	if err := png.Encode(&faviconEncoded, image.NewRGBA(image.Rect(0, 0, 32, 32))); err != nil {
		t.Fatal(err)
	}
	favicon := faviconEncoded.Bytes()
	faviconHandler.Seed("example.com", favicon)
	applicationMux := http.NewServeMux()
	applicationMux.Handle("GET /favicons/{hostname}", faviconHandler)
	applicationMux.Handle("/", http.NotFoundHandler())
	handler := server.Handler(applicationMux)
	if response := serve(handler, authRequest(http.MethodGet, "/favicons/example.com", nil)); response.Code != http.StatusUnauthorized {
		t.Fatalf("unauthenticated favicon = %d", response.Code)
	}
	authenticated := authRequest(http.MethodGet, "/favicons/example.com", nil)
	authenticated.AddCookie(&http.Cookie{Name: server.sessions.cookieName, Value: token})
	response := serve(handler, authenticated)
	if response.Code != http.StatusOK || response.Header().Get("Content-Type") != "image/png" ||
		response.Header().Get("Cache-Control") != "private, max-age=86400" || !bytes.Equal(response.Body.Bytes(), favicon) {
		t.Fatalf("authenticated favicon = %d %q %#v", response.Code, response.Body.String(), response.Header())
	}
	if _, format, err := image.Decode(bytes.NewReader(response.Body.Bytes())); err != nil || format != "png" {
		t.Fatalf("authenticated favicon body = %q, format=%q, err=%v", response.Body.Bytes(), format, err)
	}
	etag := response.Header().Get("ETag")
	if etag == "" {
		t.Fatal("authenticated favicon did not include an ETag")
	}
	conditional := authenticated.Clone(authenticated.Context())
	conditional.Header.Set("If-None-Match", etag)
	if response := serve(handler, conditional); response.Code != http.StatusNotModified || response.Body.Len() != 0 {
		t.Fatalf("conditional favicon = %d %d bytes", response.Code, response.Body.Len())
	}
	invalid := authRequest(http.MethodGet, "/favicons/127.0.0.1", nil)
	invalid.AddCookie(&http.Cookie{Name: server.sessions.cookieName, Value: token})
	if response := serve(handler, invalid); response.Code != http.StatusBadRequest {
		t.Fatalf("invalid favicon = %d", response.Code)
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::authority_session_and_removed_bootstrap_boundary.
func TestRustServer_authority_session_and_removed_bootstrap_boundary(t *testing.T) {
	for _, test := range []struct {
		method string
		path   string
	}{
		{method: http.MethodGet, path: "/"},
		{method: http.MethodGet, path: "/__noema/bootstrap/test-capability"},
		{method: http.MethodPost, path: "/graphql"},
		{method: http.MethodGet, path: "/mcp/oauth/callback"},
		{method: http.MethodGet, path: "/adapter/oauth/callback"},
	} {
		for _, host := range []string{"", "attacker.invalid:3737"} {
			server, _, _ := newAuthTest(t, false)
			request := httptest.NewRequest(test.method, "http://localhost:3737"+test.path, nil)
			request.Host = host
			if response := serve(server.Handler(http.NotFoundHandler()), request); response.Code != http.StatusBadRequest {
				t.Fatalf("invalid authority %s %s %q = %d", test.method, test.path, host, response.Code)
			}
		}
	}
	setupServer, _, _ := newAuthTest(t, false)
	setupHandler := setupServer.Handler(http.NotFoundHandler())
	graphql := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	if response := serve(setupHandler, graphql); response.Code != http.StatusForbidden {
		t.Fatalf("setup GraphQL = %d", response.Code)
	}
	missingOrigin := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	missingOrigin.Header.Del("Origin")
	if response := serve(setupHandler, missingOrigin); response.Code != http.StatusForbidden {
		t.Fatalf("missing GraphQL origin = %d", response.Code)
	}
	for _, test := range []struct {
		method string
		path   string
	}{
		{method: http.MethodPost, path: "/graphql"},
		{method: http.MethodGet, path: "/graphql/ws"},
		{method: http.MethodPost, path: "/auth/passkey/login/start"},
	} {
		for _, origin := range []string{"", "http://attacker.invalid:3737"} {
			server, _, _ := newAuthTest(t, false)
			request := httptest.NewRequest(test.method, "http://localhost:3737"+test.path, nil)
			request.Host = "localhost:3737"
			if origin != "" {
				request.Header.Set("Origin", origin)
			}
			if response := serve(server.Handler(http.NotFoundHandler()), request); response.Code != http.StatusForbidden {
				t.Fatalf("origin boundary %s %s %q = %d", test.method, test.path, origin, response.Code)
			}
		}
	}
	server, _, _ := newAuthTest(t, false)
	token, _, _ := seedPasskey(t, server, 36)
	handler := server.Handler(http.NotFoundHandler())
	invalid := authRequest(http.MethodGet, "/auth/status", nil)
	invalid.Host = "attacker.invalid:3737"
	if response := serve(handler, invalid); response.Code != http.StatusBadRequest {
		t.Fatalf("invalid authority = %d", response.Code)
	}
	for _, path := range []string{"/__noema/bootstrap/test-capability", "/__noema/bootstrap/wrong"} {
		request := authRequest(http.MethodGet, path, nil)
		request.AddCookie(&http.Cookie{Name: server.sessions.cookieName, Value: token})
		if response := serve(handler, request); response.Code != http.StatusNotFound || response.Body.String() != "404 page not found\n" {
			t.Fatalf("removed bootstrap %s = %d %q", path, response.Code, response.Body.String())
		}
	}
	securePaths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	secureConfig, recovery, err := LoadConfig(securePaths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	secureConfig, err = canonicalConfig("https://noema.example", "noema.example", false)
	if err != nil {
		t.Fatal(err)
	}
	secureStore, err := store.Open(context.Background(), securePaths.Database())
	if err != nil {
		t.Fatal(err)
	}
	defer secureStore.Close()
	secureServer, err := New(securePaths, secureStore, secureConfig, recovery)
	if err != nil {
		t.Fatal(err)
	}
	_, _, _ = seedPasskey(t, secureServer, 54)
	secureHandler := secureServer.TestHandler(http.NotFoundHandler())
	secureStart := httptest.NewRequest(http.MethodPost, "https://noema.example/__test/authenticate", nil)
	secureStart.Host = "noema.example"
	secureStart.Header.Set("Origin", "https://noema.example")
	secureResponse := serve(secureHandler, secureStart)
	if secureResponse.Code != http.StatusNoContent || !strings.HasPrefix(secureResponse.Header().Get("Set-Cookie"), "__Host-noema.sid=") {
		t.Fatalf("secure authentication cookie = %d %q", secureResponse.Code, secureResponse.Header().Get("Set-Cookie"))
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::recovery_authorizes_one_setup_session_without_authenticating_it.
func TestRustServer_recovery_authorizes_one_setup_session_without_authenticating_it(t *testing.T) {
	server, _, paths := newAuthTest(t, false)
	handler := server.Handler(http.NotFoundHandler())
	initial := serve(handler, authRequest(http.MethodPost, "/auth/passkey/register/start", nil))
	if initial.Code != http.StatusOK {
		t.Fatalf("initial registration = %d", initial.Code)
	}
	var initialPayload struct {
		CeremonyID string `json:"ceremonyId"`
	}
	if err := json.Unmarshal(initial.Body.Bytes(), &initialPayload); err != nil || initialPayload.CeremonyID == "" {
		t.Fatalf("initial registration body = %q, %v", initial.Body.String(), err)
	}
	recovery := serve(handler, recoveryRequest(readRecoveryCode(t, paths), nil))
	if recovery.Code != http.StatusNoContent {
		t.Fatalf("recovery = %d", recovery.Code)
	}
	setupCookie := lastSessionCookie(t, recovery, server.sessions.cookieName)
	graphql := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	graphql.AddCookie(setupCookie)
	if response := serve(handler, graphql); response.Code != http.StatusForbidden {
		t.Fatalf("setup GraphQL = %d", response.Code)
	}
	start := authRequest(http.MethodPost, "/auth/passkey/register/start", nil)
	start.AddCookie(setupCookie)
	response := serve(handler, start)
	if response.Code != http.StatusOK {
		t.Fatalf("setup registration = %d", response.Code)
	}
	var payload struct {
		CeremonyID string `json:"ceremonyId"`
		Options    struct {
			PublicKey struct {
				Challenge string `json:"challenge"`
			} `json:"publicKey"`
		} `json:"options"`
	}
	if err := json.Unmarshal(response.Body.Bytes(), &payload); err != nil || payload.CeremonyID == "" || payload.Options.PublicKey.Challenge == "" {
		t.Fatalf("setup registration body = %q, %v", response.Body.String(), err)
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::development_mode_keeps_canonical_host_and_origin_checks.
func TestRustServer_development_mode_keeps_canonical_host_and_origin_checks(t *testing.T) {
	server, _, _ := newAuthTest(t, true)
	handler := server.Handler(http.NotFoundHandler())
	invalidHost := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	invalidHost.Host = "192.0.2.10:3737"
	invalidHost.Header.Set("Origin", "http://192.0.2.10:3737")
	if response := serve(handler, invalidHost); response.Code != http.StatusBadRequest {
		t.Fatalf("development invalid authority = %d", response.Code)
	}
	logout := authRequest(http.MethodPost, "/auth/logout", nil)
	logout.Header.Del("Origin")
	if response := serve(handler, logout); response.Code != http.StatusForbidden {
		t.Fatalf("development logout without origin = %d", response.Code)
	}
	approval := authRequest(http.MethodPost, "/oauth/authorize", nil)
	approval.Header.Set("Authorization", "Bearer ignored")
	approval.Header.Del("Origin")
	approval.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	approval.Body = io.NopCloser(strings.NewReader("csrf=ignored&decision=approve"))
	if response := serve(handler, approval); response.Code != http.StatusBadRequest {
		t.Fatalf("development OAuth approval = %d", response.Code)
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::browser_authentication_precedes_graphql_parsing.
func TestRustServer_browser_authentication_precedes_graphql_parsing(t *testing.T) {
	server, _, _ := newAuthTest(t, false)
	_, _, _ = seedPasskey(t, server, 35)
	var applicationCalls atomic.Int32
	handler := server.Handler(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		applicationCalls.Add(1)
		w.WriteHeader(http.StatusNoContent)
	}))
	registration := authRequest(http.MethodPost, "/auth/passkey/register/start", nil)
	if response := serve(handler, registration); response.Code != http.StatusForbidden {
		t.Fatalf("unauthenticated registration = %d", response.Code)
	}
	malformed := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString("{"))
	if response := serve(handler, malformed); response.Code != http.StatusUnauthorized {
		t.Fatalf("unauthenticated malformed GraphQL = %d", response.Code)
	}
	if applicationCalls.Load() != 0 {
		t.Fatal("unauthenticated GraphQL reached the application")
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::native_authorization_resumes_after_recent_passkey_authentication.
func TestRustServer_native_authorization_resumes_after_recent_passkey_authentication(t *testing.T) {
	server, _, _ := newAuthTest(t, false)
	_, _, _ = seedPasskey(t, server, 37)
	handler := server.TestHandler(http.NotFoundHandler())
	query := nativeAuthorizationQuery(testDesktopClient, testDesktopRedirect, "s")
	request := authRequest(http.MethodGet, "/oauth/authorize?"+query, nil)
	response := serve(handler, request)
	if response.Code != http.StatusSeeOther || response.Header().Get("Location") != "/?native_authorization=resume" {
		t.Fatalf("authorization resume = %d %q", response.Code, response.Header().Get("Location"))
	}
	pendingCookie := lastSessionCookie(t, response, server.sessions.cookieName)
	authenticated := httptest.NewRequest(http.MethodPost, "http://localhost:3737/__test/authenticate", nil)
	authenticated.Host = server.config.Authority
	authenticated.Header.Set("Origin", server.config.Origin)
	authenticated.AddCookie(pendingCookie)
	authenticatedResponse := serve(handler, authenticated)
	if authenticatedResponse.Code != http.StatusNoContent {
		t.Fatalf("pending authentication = %d %s", authenticatedResponse.Code, authenticatedResponse.Body.String())
	}
	newCookie := lastSessionCookie(t, authenticatedResponse, server.sessions.cookieName)
	resume := authRequest(http.MethodGet, "/oauth/authorize", nil)
	resume.AddCookie(newCookie)
	response = serve(handler, resume)
	if response.Code != http.StatusOK ||
		!strings.Contains(response.Body.String(), "<title>Connect Noema Desktop · Noema</title>") ||
		!strings.Contains(response.Body.String(), ">Connect Noema Desktop?</h1>") {
		t.Fatalf("resumed consent = %d %s", response.Code, response.Body.String())
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::ios_authorization_page_alone_permits_native_form_navigation.
func TestRustServer_ios_authorization_page_alone_permits_native_form_navigation(t *testing.T) {
	server, _, _ := newAuthTest(t, false)
	_, _, _ = seedPasskey(t, server, 32)
	handler := server.TestHandler(http.NotFoundHandler())
	cookie := authenticateViaTestRoute(t, handler, server)
	query := nativeAuthorizationQuery("noema-ios:abcdefghijklmnop", "noema://oauth/callback", "s")
	request := authRequest(http.MethodGet, "/oauth/authorize?"+query, nil)
	request.AddCookie(cookie)
	response := serve(handler, request)
	iosPolicy := "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self' noema:"
	if response.Code != http.StatusOK || response.Header().Get("Content-Security-Policy") != iosPolicy {
		t.Fatalf("iOS authorization = %d %#v", response.Code, response.Header())
	}
	status := authRequest(http.MethodGet, "/auth/status", nil)
	status.AddCookie(cookie)
	response = serve(handler, status)
	defaultPolicy := "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; img-src 'self' data:; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'"
	if response.Header().Get("Content-Security-Policy") != defaultPolicy {
		t.Fatalf("default CSP = %q", response.Header().Get("Content-Security-Policy"))
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::development_auth_bypass_allows_graphql_without_bootstrap.
func TestRustServer_development_auth_bypass_allows_graphql_without_bootstrap(t *testing.T) {
	server, _, _ := newAuthTest(t, true)
	application := http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if request.URL.Path == "/graphql" {
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"data":{"__typename":"QueryRoot"}}`))
			return
		}
		http.NotFound(w, request)
	})
	handler := server.Handler(application)
	graphql := authRequest(http.MethodPost, "/graphql", bytes.NewBufferString(`{"query":"{ __typename }"}`))
	response := serve(handler, graphql)
	if response.Code != http.StatusOK || response.Body.String() != `{"data":{"__typename":"QueryRoot"}}` {
		t.Fatalf("development GraphQL = %d %q", response.Code, response.Body.String())
	}
	if response := serve(handler, recoveryRequest("unavailable", nil)); response.Code != http.StatusNotFound {
		t.Fatalf("development recovery = %d", response.Code)
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::recovery_keeps_origin_checks_and_rotates_rejected_candidates.
func TestRustServer_recovery_keeps_origin_checks_and_rotates_rejected_candidates(t *testing.T) {
	server, _, paths := newAuthTest(t, false)
	handler := server.Handler(http.NotFoundHandler())
	initial := readRecoveryCode(t, paths)
	missing := recoveryRequest(readRecoveryCode(t, paths), nil)
	missing.Header.Del("Origin")
	if response := serve(handler, missing); response.Code != http.StatusForbidden {
		t.Fatalf("missing recovery Origin = %d", response.Code)
	}
	if response := serve(handler, recoveryRequest("wrong", nil)); response.Code != http.StatusUnauthorized {
		t.Fatalf("wrong recovery = %d", response.Code)
	}
	if response := serve(handler, recoveryRequest(initial, nil)); response.Code != http.StatusUnauthorized {
		t.Fatalf("stale recovery = %d", response.Code)
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::pwa_asset_responses_use_release_safe_headers.
func TestRustServer_pwa_asset_responses_use_release_safe_headers(t *testing.T) {
	directory := t.TempDir()
	for _, name := range []string{"sw.js", "manifest.webmanifest", "pwa-192x192.png", "pwa-512x512.png", "apple-touch-icon.png"} {
		body := []byte("pwa asset")
		if strings.HasSuffix(name, ".png") {
			var encoded bytes.Buffer
			if err := png.Encode(&encoded, image.NewRGBA(image.Rect(0, 0, 1, 1))); err != nil {
				t.Fatal(err)
			}
			body = encoded.Bytes()
		}
		if err := os.WriteFile(filepath.Join(directory, name), body, 0o600); err != nil {
			t.Fatal(err)
		}
	}
	t.Setenv("NOEMA_DEV_ASSET_DIR", directory)
	handler := webserver.NewAssetHandler()
	for _, test := range []struct {
		name, contentType, worker string
	}{
		{"sw.js", "application/javascript; charset=utf-8", "/"},
		{"manifest.webmanifest", "application/manifest+json; charset=utf-8", ""},
		{"pwa-192x192.png", "image/png", ""},
		{"pwa-512x512.png", "image/png", ""},
		{"apple-touch-icon.png", "image/png", ""},
	} {
		response := parityRequest(handler, http.MethodGet, "/assets/"+test.name)
		if response.Code != http.StatusOK || response.Header().Get("Content-Type") != test.contentType ||
			response.Header().Get("Cache-Control") != "no-cache" || response.Header().Get("Service-Worker-Allowed") != test.worker {
			t.Fatalf("%s response = %d %#v", test.name, response.Code, response.Header())
		}
		if strings.HasSuffix(test.name, ".png") {
			decoded, format, err := image.Decode(bytes.NewReader(response.Body.Bytes()))
			if err != nil || decoded == nil || format != "png" || decoded.Bounds().Dx() != 1 || decoded.Bounds().Dy() != 1 {
				t.Fatalf("%s PNG = %v, %q, %v", test.name, decoded, format, err)
			}
		}
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::private_and_network_endpoints_remain_excluded_from_http_caches.
func TestRustServer_private_and_network_endpoints_remain_excluded_from_http_caches(t *testing.T) {
	server, _, _ := newAuthTest(t, true)
	handler := server.Handler(http.NotFoundHandler())
	for _, path := range []string{"/auth/status", "/auth/recovery", "/artifacts/versions/missing/download", "/artifacts/versions/missing/preview"} {
		response := serve(handler, authRequest(http.MethodGet, path, nil))
		if response.Header().Get("Cache-Control") != "no-store" || response.Header().Get("Service-Worker-Allowed") != "" {
			t.Fatalf("%s headers = %#v", path, response.Header())
		}
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::router_preserves_oauth_and_plain_text_not_found_responses.
func TestRustServer_router_preserves_oauth_and_plain_text_not_found_responses(t *testing.T) {
	server, taskStore, paths := newAuthTest(t, false)
	_, _, _ = seedPasskey(t, server, 53)
	server.config.Authority = "localhost:3737"
	server.config.Origin = "http://localhost:3737"
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	mcpService, err := noemamcp.NewService(paths, taskStore, false, nil, server.config.Origin+"/mcp/oauth/callback")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(mcpService.Close)
	adapterService, err := noemaadapter.NewService(root, taskStore)
	if err != nil {
		t.Fatal(err)
	}
	if err := adapterService.SetOAuthCallback(server.config.Origin + "/adapter/oauth/callback"); err != nil {
		t.Fatal(err)
	}
	artifacts, err := artifact.New(root, taskStore, nil)
	if err != nil {
		t.Fatal(err)
	}
	graphql := http.NotFoundHandler()
	applicationMux := http.NewServeMux()
	applicationMux.Handle("/graphql", graphql)
	applicationMux.Handle("/graphql/ws", graphql)
	applicationMux.Handle("/graphql/schema.graphql", graphql)
	applicationMux.Handle("GET /mcp/oauth/callback", mcpService.CallbackHandler())
	applicationMux.Handle("GET /adapter/oauth/callback", adapterService.OAuthCallbackHandler())
	applicationMux.Handle("/artifacts/versions/", artifacts.Handler())
	applicationMux.Handle("/", http.NotFoundHandler())
	application := applicationMux
	handler := server.Handler(application)
	if got := oauthCallbackURL(server.config.Authority, "/mcp/oauth/callback", "attemptId=1&host=attacker.invalid"); got != "http://localhost:3737/mcp/oauth/callback?attemptId=1&host=attacker.invalid" {
		t.Fatalf("OAuth callback URL = %q", got)
	}
	oauth := serve(handler, authRequest(http.MethodGet, "/mcp/oauth/callback", nil))
	if oauth.Code != http.StatusBadRequest || oauth.Header().Get("Content-Type") != "text/plain; charset=utf-8" ||
		oauth.Body.String() != "missing OAuth callback query" {
		t.Fatalf("OAuth callback response = %d %#v %q", oauth.Code, oauth.Header(), oauth.Body.String())
	}
	adapter := serve(handler, authRequest(http.MethodGet, "/adapter/oauth/callback?state=missing&code=hidden", nil))
	if adapter.Code != http.StatusBadRequest || adapter.Header().Get("Content-Type") != "text/html; charset=utf-8" ||
		adapter.Body.String() != "<!doctype html><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Noema OAuth</title><main><p>Noema no longer recognizes this connection attempt. Return to Noema and start again.</p><p><a href=\"/\">Return to Noema</a></p></main>" {
		t.Fatalf("adapter callback response = %d %#v %q", adapter.Code, adapter.Header(), adapter.Body.String())
	}
	oversized := serve(handler, authRequest(http.MethodGet, "/adapter/oauth/callback?state="+strings.Repeat("x", 8193), nil))
	if oversized.Code != http.StatusBadRequest || oversized.Header().Get("Content-Type") != "text/plain; charset=utf-8" ||
		oversized.Body.String() != "invalid OAuth callback query" {
		t.Fatalf("oversized callback response = %d %#v %q", oversized.Code, oversized.Header(), oversized.Body.String())
	}
	for _, test := range []struct {
		method string
		path   string
	}{
		{method: http.MethodHead, path: "/graphql"},
		{method: http.MethodPut, path: "/graphql/schema.graphql"},
		{method: http.MethodHead, path: "/graphql/ws"},
		{method: http.MethodPut, path: "/mcp/oauth/callback"},
		{method: http.MethodPut, path: "/adapter/oauth/callback"},
		{method: http.MethodHead, path: "/artifacts/versions/missing/download"},
		{method: http.MethodHead, path: "/artifacts/versions/missing/preview"},
		{method: http.MethodPut, path: "/memory"},
	} {
		response := serve(handler, authRequest(test.method, test.path, nil))
		if response.Code != http.StatusNotFound || response.Header().Get("Content-Type") != "text/plain; charset=utf-8" {
			t.Fatalf("%s %s = %d %#v", test.method, test.path, response.Code, response.Header())
		}
		if test.method != http.MethodHead && response.Body.String() != "404 page not found\n" {
			t.Fatalf("%s %s body = %q", test.method, test.path, response.Body.String())
		}
	}
	artifactRequest := authRequest(http.MethodGet, "/artifacts/versions/missing/download", nil)
	if response := serve(handler, artifactRequest); response.Code != http.StatusUnauthorized || response.Body.Len() != 0 {
		t.Fatalf("unauthorized artifact download = %d %q", response.Code, response.Body.String())
	}
}

func dialParityWebSocket(t *testing.T, server *Server, handler http.Handler, cookie, bearer string) (*websocket.Conn, func()) {
	t.Helper()
	network := httptest.NewServer(handler)
	authority := strings.TrimPrefix(network.URL, "http://")
	server.config.Authority = authority
	server.config.Origin = "http://" + authority
	requestHeaders := make(http.Header)
	if cookie != "" {
		requestHeaders.Set("Cookie", cookie)
		requestHeaders.Set("Origin", server.config.Origin)
	}
	if bearer != "" {
		requestHeaders.Set("Authorization", bearer)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	connection, response, err := websocket.Dial(ctx, "ws"+strings.TrimPrefix(network.URL, "http")+"/graphql/ws", &websocket.DialOptions{
		Subprotocols: []string{"graphql-transport-ws"}, HTTPHeader: requestHeaders,
	})
	cancel()
	if err != nil {
		network.Close()
		if response != nil {
			t.Fatalf("WebSocket handshake: %v (%s)", err, response.Status)
		}
		t.Fatal(err)
	}
	stop := func() {
		_ = connection.CloseNow()
		network.Close()
	}
	return connection, stop
}

func parityRequest(handler http.Handler, method string, target string) *httptest.ResponseRecorder {
	request := httptest.NewRequest(method, target, nil)
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)
	return response
}

// startRegistrationCeremony goes through the public registration-start route.
// The Rust test creates a real WebAuthn registration state before exercising
// the session-bound take operation; this keeps the same production boundary.
func startRegistrationCeremony(t *testing.T, handler http.Handler, server *Server, cookie *http.Cookie) (string, *http.Cookie) {
	t.Helper()
	request := authRequest(http.MethodPost, "/auth/passkey/register/start", nil)
	if cookie != nil {
		request.AddCookie(cookie)
	}
	response := serve(handler, request)
	if response.Code != http.StatusOK {
		t.Fatalf("registration start = %d %s", response.Code, response.Body.String())
	}
	var payload struct {
		CeremonyID string `json:"ceremonyId"`
	}
	if err := json.Unmarshal(response.Body.Bytes(), &payload); err != nil || payload.CeremonyID == "" {
		t.Fatalf("registration start body = %q, err=%v", response.Body.String(), err)
	}
	return payload.CeremonyID, lastSessionCookie(t, response, server.sessions.cookieName)
}

func authenticateViaTestRoute(t *testing.T, handler http.Handler, server *Server) *http.Cookie {
	t.Helper()
	request := authRequest(http.MethodPost, "/__test/authenticate", nil)
	response := serve(handler, request)
	if response.Code != http.StatusNoContent {
		t.Fatalf("test authentication = %d %s", response.Code, response.Body.String())
	}
	return lastSessionCookie(t, response, server.sessions.cookieName)
}

func oauthCallbackURL(authority, path, query string) string {
	return "http://" + authority + path + "?" + query
}
