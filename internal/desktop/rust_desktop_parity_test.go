package desktop

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net"
	"os"
	"reflect"
	"strings"
	"testing"
)

type testCredential struct {
	value     string
	getErr    error
	setErr    error
	deleteErr error
}

func (c *testCredential) Get() (string, error) {
	if c.getErr != nil {
		return "", c.getErr
	}
	if c.value == "" {
		return "", errors.New("credential is unavailable")
	}
	return c.value, nil
}

func (c *testCredential) Set(value string) error {
	if c.setErr != nil {
		return c.setErr
	}
	c.value = value
	return nil
}

func (c *testCredential) Delete() error {
	if c.deleteErr != nil {
		return c.deleteErr
	}
	c.value = ""
	return nil
}

// Rust source: crates/noema-desktop/src/desktop_profile.rs::stored_selection_excludes_the_bearer_and_requires_secure_storage.
func TestRustDesktop_StoredSelectionExcludesTheBearerAndRequiresSecureStorage(t *testing.T) {
	root := t.TempDir()
	credential := &testCredential{}
	store := NewProfileStore(root+"/desktop.json", credential)
	bearer := "memory-access-token"
	profile := RemoteProfile{
		Metadata:     RemoteMetadata{Origin: "https://noema.example", ClientID: "client-one"},
		RefreshToken: "private-refresh-token",
		AccessToken:  &bearer,
	}
	if err := store.SaveRemote(profile); err != nil {
		t.Fatalf("save profile: %v", err)
	}
	stored, err := io.ReadAll(mustOpen(t, root+"/desktop.json"))
	if err != nil {
		t.Fatalf("settings: %v", err)
	}
	if strings.Contains(string(stored), "https://noema.example") {
		t.Fatal("settings contained the origin")
	}
	if strings.Contains(string(stored), "client-one") {
		t.Fatal("settings contained the client id")
	}
	if strings.Contains(string(stored), "private-refresh-token") {
		t.Fatal("settings contained the refresh token")
	}
	protected, err := credential.Get()
	if err != nil {
		t.Fatalf("protected profile: %v", err)
	}
	if !strings.Contains(protected, "private-refresh-token") {
		t.Fatal("protected profile did not contain the refresh token")
	}
	if strings.Contains(protected, "memory-access-token") {
		t.Fatal("protected profile contained the access token")
	}

	unavailablePath := root + "/unavailable.json"
	unavailable := NewProfileStore(unavailablePath, &testCredential{setErr: errors.New("credential store unavailable")})
	if err := unavailable.SaveRemote(profile); err == nil {
		t.Fatal("unavailable credential store accepted the profile")
	}
	if _, err := os.Stat(unavailablePath); err == nil {
		t.Fatal("unavailable profile wrote a settings file")
	}

	if err := credential.Delete(); err != nil {
		t.Fatalf("remove credential: %v", err)
	}
	if selection := store.Load(); selection.Kind != SelectionRecovery {
		t.Fatalf("selection after credential removal = %#v", selection)
	}
}

// Rust source: crates/noema-desktop/src/desktop_state.rs::finished_subscription_cleanup_preserves_newer_replacement.
func TestRustDesktop_FinishedSubscriptionCleanupPreservesNewerReplacement(t *testing.T) {
	subscriptions := NewSubscriptionTasks()
	oldContext, oldCancel := context.WithCancel(context.Background())
	oldGeneration := subscriptions.Insert("sub_1", oldCancel)
	_, newCancel := context.WithCancel(context.Background())
	newGeneration := subscriptions.Insert("sub_1", newCancel)
	select {
	case <-oldContext.Done():
	default:
		t.Fatal("replacing a subscription did not cancel the old task")
	}
	subscriptions.RemoveFinished("sub_1", oldGeneration)
	if generation, ok := subscriptions.Generation("sub_1"); !ok || generation != newGeneration {
		t.Fatalf("subscription generation = %d, %v; want %d", generation, ok, newGeneration)
	}
	subscriptions.RemoveFinished("sub_1", newGeneration)
	if got := subscriptions.Len(); got != 0 {
		t.Fatalf("subscription entries = %d", got)
	}
}

// Rust source: crates/noema-desktop/src/desktop_state.rs::local_mode_requires_revocation_or_explicit_forget.
func TestRustDesktop_LocalModeRequiresRevocationOrExplicitForget(t *testing.T) {
	if err := AllowLocalAfterRemote(nil); err != nil {
		t.Fatalf("successful revocation = %v", err)
	}
	if err := AllowLocalAfterRemote(RemoteUnauthorized); err != nil {
		t.Fatalf("unauthorized revocation = %v", err)
	}
	if err := AllowLocalAfterRemote(RemoteOffline); err == nil {
		t.Fatal("offline revocation was accepted")
	}
	if err := AllowLocalAfterRemote(RemoteInvalidResponse); err == nil {
		t.Fatal("invalid response revocation was accepted")
	}
}

// Rust source: crates/noema-desktop/src/external_url.rs::external_url_policy_accepts_web_and_rejects_other_schemes.
func TestRustDesktop_ExternalURLPolicyAcceptsWebAndRejectsOtherSchemes(t *testing.T) {
	if !IsExternalWebURL("https://example.com") {
		t.Fatal("HTTPS URL was rejected")
	}
	if !IsExternalWebURL("http://example.com") {
		t.Fatal("HTTP URL was rejected")
	}
	if IsExternalWebURL("file:///etc/passwd") {
		t.Fatal("file URL was accepted")
	}
	if IsExternalWebURL("not a url") {
		t.Fatal("invalid URL was accepted")
	}
}

// Rust source: crates/noema-desktop/src/graphql_ipc.rs::ipc_boundary_preserves_frontend_wire_key_and_main_window_authority.
func TestRustDesktop_IPCBoundaryPreservesFrontendWireKeyAndMainWindowAuthority(t *testing.T) {
	payload := SubscriptionEventPayload{
		SubscriptionID: "sub_1",
		Response:       json.RawMessage(`{"data":{"ok":true}}`),
	}
	serialized, err := json.Marshal(payload)
	if err != nil {
		t.Fatalf("serialize subscription event payload: %v", err)
	}
	var value map[string]any
	if err := json.Unmarshal(serialized, &value); err != nil {
		t.Fatalf("decode subscription event payload: %v", err)
	}
	if value["subscriptionId"] != "sub_1" {
		t.Fatalf("subscriptionId = %#v", value["subscriptionId"])
	}
	if _, exists := value["subscription_id"]; exists {
		t.Fatal("snake-case subscription key was serialized")
	}
	response, ok := value["response"].(map[string]any)
	if !ok || response["data"].(map[string]any)["ok"] != true {
		t.Fatalf("response = %#v", value["response"])
	}
	if err := RequireMainWindowLabel("main"); err != nil {
		t.Fatalf("main window was rejected: %v", err)
	}
	if err := RequireMainWindowLabel("secondary"); err == nil {
		t.Fatal("secondary window was accepted")
	}
	if err := RequireMainWindowLabel(""); err == nil {
		t.Fatal("empty window label was accepted")
	}
	changed, err := json.Marshal(ConnectionChangedPayload{State: "offline"})
	if err != nil {
		t.Fatalf("serialize connection change: %v", err)
	}
	var changedValue map[string]any
	if err := json.Unmarshal(changed, &changedValue); err != nil || !reflect.DeepEqual(changedValue, map[string]any{"state": "offline"}) {
		t.Fatalf("connection change = %s, %v", changed, err)
	}
}

// Rust source: crates/noema-desktop/src/mcp_oauth_callback.rs::parses_callback_request_head.
func TestRustDesktop_ParsesCallbackRequestHead(t *testing.T) {
	request, err := ParseRequestHead("GET /mcp/oauth/callback?attemptId=mcp_oauth%3Aabc&code=123 HTTP/1.1\r\nHost: 127.0.0.1:4444\r\n\r\n")
	if err != nil {
		t.Fatalf("request: %v", err)
	}
	if request.Method != "GET" {
		t.Fatalf("method = %q", request.Method)
	}
	if request.Path() != "/mcp/oauth/callback" {
		t.Fatalf("path = %q", request.Path())
	}
	query, ok := request.Query()
	if !ok {
		t.Fatal("request query was absent")
	}
	if value, ok := QueryValue(query, "attemptId"); !ok || value != "mcp_oauth:abc" {
		t.Fatalf("attemptId = %q, %v", value, ok)
	}
	if got := request.CallbackURL("127.0.0.1:4444"); got != "http://127.0.0.1:4444/mcp/oauth/callback?attemptId=mcp_oauth%3Aabc&code=123" {
		t.Fatalf("callback URL = %q", got)
	}
	if request.Host == "127.0.0.1:5555" {
		t.Fatal("request host matched an unrelated authority")
	}

	adapter, err := ParseRequestHead("GET /adapter/oauth/callback?code=123&state=abc HTTP/1.1\r\nHost: 127.0.0.1:4444\r\n\r\n")
	if err != nil {
		t.Fatalf("adapter request: %v", err)
	}
	if adapter.Path() != "/adapter/oauth/callback" {
		t.Fatalf("adapter path = %q", adapter.Path())
	}
	if got := adapter.CallbackURL("127.0.0.1:4444"); got != "http://127.0.0.1:4444/adapter/oauth/callback?code=123&state=abc" {
		t.Fatalf("adapter callback URL = %q", got)
	}

	provider, err := ParseRequestHead("GET /provider/oauth/callback/abcdEFGH01234567ijklMNOP89012345?code=123 HTTP/1.1\r\nHost: 127.0.0.1:4444\r\n\r\n")
	if err != nil {
		t.Fatalf("provider request: %v", err)
	}
	if got := provider.CallbackURL("127.0.0.1:4444"); got != "http://127.0.0.1:4444/provider/oauth/callback/abcdEFGH01234567ijklMNOP89012345?code=123" {
		t.Fatalf("provider callback URL = %q", got)
	}
}

// Rust source: crates/noema-desktop/src/remote_graphql.rs::graphql_request_keeps_values_and_adds_bearer_authorization.
func TestRustDesktop_GraphQLRequestKeepsValuesAndAddsBearerAuthorization(t *testing.T) {
	remote, err := NewRemoteGraphQL("https://noema.example")
	if err != nil {
		t.Fatalf("remote: %v", err)
	}
	body := []byte(`{"query":"query Example($value: String!) { echo(value: $value) }","variables":{"value":"ordinary-value"}}`)
	request, err := remote.BuildGraphQLRequest(body, "memory-access")
	if err != nil {
		t.Fatalf("request: %v", err)
	}
	if request.URL.String() != "https://noema.example/graphql" {
		t.Fatalf("request URL = %q", request.URL)
	}
	if value := request.Header.Get("Authorization"); !strings.HasPrefix(value, "Bearer ") {
		t.Fatalf("authorization = %q", value)
	}
	savedBytes, err := io.ReadAll(request.Body)
	if err != nil {
		t.Fatalf("request body: %v", err)
	}
	var saved, expected any
	if json.Unmarshal(savedBytes, &saved) != nil || json.Unmarshal(body, &expected) != nil || !reflect.DeepEqual(saved, expected) {
		t.Fatalf("saved body = %s, want %s", savedBytes, body)
	}

	local, err := NewLocalGraphQL("http://127.0.0.1:4747", "desktop-access")
	if err != nil {
		t.Fatalf("local: %v", err)
	}
	request, err = local.BuildGraphQLRequest(body, "desktop-access")
	if err != nil {
		t.Fatalf("local request: %v", err)
	}
	if request.URL.String() != "http://127.0.0.1:4747/graphql" {
		t.Fatalf("local request URL = %q", request.URL)
	}
	if local.WebsocketURL() != "ws://127.0.0.1:4747/graphql/ws" {
		t.Fatalf("websocket URL = %q", local.WebsocketURL())
	}
	if _, err := NewLocalGraphQL("http://example.com:4747", "x"); err == nil {
		t.Fatal("non-loopback local origin was accepted")
	}
}

// Rust source: crates/noema-desktop/src/remote_graphql.rs::websocket_protocol_maps_next_error_complete_and_ping.
func TestRustDesktop_WebSocketProtocolMapsNextErrorCompleteAndPing(t *testing.T) {
	next := MapProtocolMessage(`{"type":"next","payload":{"data":{"ok":true}}}`)
	if next.Kind != ProtocolNext || next.Value["data"].(map[string]any)["ok"] != true {
		t.Fatalf("next message = %#v", next)
	}
	protocolError := MapProtocolMessage(`{"type":"error","payload":[{"message":"denied"}]}`)
	errorsValue, ok := protocolError.Value["errors"].([]any)
	if protocolError.Kind != ProtocolNext || !ok || errorsValue[0].(map[string]any)["message"] != "denied" {
		t.Fatalf("error message = %#v", protocolError)
	}
	if message := MapProtocolMessage(`{"type":"complete"}`); message.Kind != ProtocolComplete {
		t.Fatalf("complete message = %#v", message)
	}
	if message := MapProtocolMessage(`{"type":"ping"}`); message.Kind != ProtocolPing {
		t.Fatalf("ping message = %#v", message)
	}
}

// Rust source: crates/noema-desktop/src/remote_oauth.rs::callback_distinguishes_verified_denial_from_handoff_and_invalid_state.
func TestRustDesktop_CallbackDistinguishesVerifiedDenialFromHandoffAndInvalidState(t *testing.T) {
	for _, test := range []struct {
		query    string
		title    string
		accepted bool
	}{
		{query: "code=unit-code&state=unit-state", title: "Continue in Noema Desktop", accepted: true},
		{query: "error=access_denied&state=unit-state", title: "Connection declined", accepted: false},
		{query: "error=access_denied&state=wrong-state", title: "Sign-in did not finish", accepted: false},
	} {
		listener, err := net.Listen("tcp", "127.0.0.1:0")
		if err != nil {
			t.Fatalf("listener: %v", err)
		}
		address := listener.Addr().String()
		redirect := "http://" + address + "/oauth/callback"
		bodyCh := make(chan string, 1)
		go func() {
			connection, dialErr := net.Dial("tcp", address)
			if dialErr != nil {
				bodyCh <- ""
				return
			}
			defer connection.Close()
			_, _ = io.WriteString(connection, "GET /oauth/callback?"+test.query+" HTTP/1.1\r\nHost: "+address+"\r\n\r\n")
			body, _ := io.ReadAll(connection)
			bodyCh <- string(body)
		}()
		_, callbackErr := ReceiveCallback(listener, redirect, "https://noema.example", "unit-state")
		_ = listener.Close()
		body := <-bodyCh
		if (callbackErr == nil) != test.accepted {
			t.Fatalf("callback result error = %v, accepted = %v", callbackErr, test.accepted)
		}
		if !strings.Contains(body, test.title) {
			t.Fatalf("callback body lacks %q: %s", test.title, body)
		}
		if strings.Contains(body, "unit-code") {
			t.Fatal("callback body exposed the authorization code")
		}
		if strings.Contains(body, "Authorization complete") {
			t.Fatal("callback body exposed the authorization completion text")
		}
	}
}

// Rust source: crates/noema-desktop/src/remote_oauth.rs::connection_parser_accepts_https_and_server_links.
func TestRustDesktop_ConnectionParserAcceptsHTTPSAndServerLinks(t *testing.T) {
	direct, err := ParsePendingConnection("https://noema.example:8443")
	if err != nil || direct.Stage().Origin != "https://noema.example:8443" {
		t.Fatalf("direct origin = %#v, %v", direct.Stage().Origin, err)
	}
	link, err := ParsePendingConnection("noema://connect?origin=https%3A%2F%2Fnoema.example%3A8443")
	if err != nil || link.Stage().Origin != "https://noema.example:8443" {
		t.Fatalf("connection link origin = %#v, %v", link.Stage().Origin, err)
	}
}

// Rust source: crates/noema-desktop/src/remote_oauth.rs::connection_parser_rejects_unsafe_origins_and_extra_fields.
func TestRustDesktop_ConnectionParserRejectsUnsafeOriginsAndExtraFields(t *testing.T) {
	for _, invalid := range []string{
		"http://noema.example",
		"https://localhost",
		"https://127.0.0.1",
		"https://noema.example/path",
		"noema://connect?origin=https%3A%2F%2Fnoema.example&extra=value",
	} {
		if _, err := ParsePendingConnection(invalid); err == nil {
			t.Fatalf("accepted %s", invalid)
		}
	}
}

func mustOpen(t *testing.T, path string) io.ReadCloser {
	t.Helper()
	file, err := os.Open(path)
	if err != nil {
		t.Fatalf("open %s: %v", path, err)
	}
	return file
}
