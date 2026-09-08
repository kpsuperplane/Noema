package mcp

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/diagnostics"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

// Rust source: crates/noema-capabilities/mcp/src/control/tests.rs::setup_secret_persistence_and_compensation_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_SetupSecretPersistenceAndCompensationContracts(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "read", Description: "Read docs", InputSchema: map[string]any{"type": "object"}}, func(context.Context, *mcpsdk.CallToolRequest, map[string]any) (*mcpsdk.CallToolResult, map[string]any, error) {
		return nil, map[string]any{"ok": true}, nil
	})
	streamable := mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil)
	var failureMode atomic.Int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if failureMode.Load() == 1 {
			http.Error(w, "credentials rejected", http.StatusUnauthorized)
			return
		}
		streamable.ServeHTTP(w, request)
	}))
	t.Cleanup(server.Close)
	paths, _, service := newMCPParityService(t, false)
	created, err := service.Create(t.Context(), SetupInput{DisplayName: "Remote", TransportKind: "streamable_http", URL: server.URL, AuthPreference: "USE_ANONYMOUS", Headers: map[string]string{}})
	if err != nil || created.Server == nil || created.Status != "ready_for_policy" || created.Discovered != 1 {
		t.Fatalf("initial setup = %#v, %v", created, err)
	}
	serverRecord := *created.Server
	if serverRecord.ToolCount != 1 || serverRecord.SafeConfig == nil || strings.Contains(string(serverRecord.SafeConfig), "Bearer") {
		t.Fatalf("persisted setup = %#v", serverRecord)
	}
	added, err := service.AddConnection(t.Context(), SetupInput{
		DefinitionID: serverRecord.DefinitionID, DefinitionRevision: serverRecord.DefinitionRevision,
		ConnectionLabel: "Work", AuthPreference: "USE_ANONYMOUS",
		Secrets: SecretMaterial{Headers: map[string]string{"Authorization": "Bearer work-secret"}},
	})
	if err != nil || added.Server == nil || added.Server.ID == serverRecord.ID || added.Server.ConnectionLabel != "Work" {
		t.Fatalf("independent connection = %#v, %v", added, err)
	}
	workSecret, err := service.secrets.loadConnection(added.Server.ID)
	if err != nil || workSecret.Headers["Authorization"] != "Bearer work-secret" {
		t.Fatalf("independent connection credentials = %#v, %v", workSecret, err)
	}
	secret := SecretMaterial{Headers: map[string]string{"Authorization": "Bearer initial"}, Environment: map[string]string{"TOKEN": "initial-token"}, Revision: strings.Repeat("a", 32)}
	createdWithSecret, err := service.Create(t.Context(), SetupInput{DisplayName: "Secret", TransportKind: "streamable_http", URL: server.URL, Headers: map[string]string{"X-Team": "infra"}, AuthPreference: "USE_ANONYMOUS", Secrets: secret})
	if err != nil || createdWithSecret.Server == nil {
		t.Fatalf("secret setup = %#v, %v", createdWithSecret, err)
	}
	loaded, err := service.secrets.loadConnection(createdWithSecret.Server.ID)
	if err != nil || loaded.Headers["Authorization"] != "Bearer initial" {
		t.Fatalf("stored secret = %#v, %v", loaded, err)
	}
	oldRevision := createdWithSecret.Server.ConnectionRevision
	continued, err := service.Continue(t.Context(), createdWithSecret.Server.ID, SecretMaterial{Headers: map[string]string{"Authorization": "Bearer replacement"}})
	if err != nil || continued.Server == nil || continued.Server.ConnectionRevision != oldRevision {
		t.Fatalf("continued setup = %#v, %v", continued, err)
	}
	loaded, err = service.secrets.loadConnection(createdWithSecret.Server.ID)
	if err != nil || loaded.Headers["Authorization"] != "Bearer replacement" || strings.Contains(string(continued.Server.SafeConfig), "Bearer replacement") || strings.Contains(string(continued.Server.SafeConfig), "Bearer initial") {
		t.Fatalf("replaced secret = %#v, %v", loaded, err)
	}
	if _, err := os.Stat(filepath.Join(paths.Root(), "mcp", createdWithSecret.Server.ID, "credentials.json")); err != nil {
		t.Fatal(err)
	}

	// Additional regression: rejected discovery must preserve active credentials.
	failureMode.Store(1)
	rejected, err := service.Continue(t.Context(), createdWithSecret.Server.ID, SecretMaterial{Headers: map[string]string{"Authorization": "Bearer rejected"}, Environment: map[string]string{"TOKEN": "rejected-token"}})
	if err != nil || rejected.Status != "needs_auth" || rejected.Server == nil {
		t.Errorf("rejected replacement = %#v, %v", rejected, err)
	}
	restored, err := service.secrets.loadConnection(createdWithSecret.Server.ID)
	if err != nil || restored.Headers["Authorization"] != "Bearer replacement" {
		t.Errorf("failed replacement changed active credentials: replacement=%t rejected=%t, err=%v", restored.Headers["Authorization"] == "Bearer replacement", restored.Headers["Authorization"] == "Bearer rejected", err)
	}
	if restored.Environment["TOKEN"] != "initial-token" || restored.Revision != loaded.Revision {
		t.Error("failed replacement changed the active environment credential or its revision")
	}
	fenced, err := service.Server(t.Context(), createdWithSecret.Server.ID)
	if err != nil || fenced.AuthStatus != "needs_auth" || fenced.HealthStatus != "unavailable" {
		t.Errorf("failed replacement status = %#v, %v", fenced, err)
	}
	var safeConfig struct {
		URL     string            `json:"url"`
		Headers map[string]string `json:"headers"`
	}
	if err := json.Unmarshal(fenced.SafeConfig, &safeConfig); err != nil || safeConfig.URL != server.URL || safeConfig.Headers["X-Team"] != "infra" {
		t.Error("failed replacement changed ordinary connection settings")
	}
}

// Rust source: crates/noema-capabilities/mcp/src/http.rs::http_configuration_validation_redaction_and_redirect_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_HTTPConfigurationValidationRedactionAndRedirectContracts(t *testing.T) {
	secret := SecretMaterial{Headers: map[string]string{"Authorization": "Bearer secret", "X-Override": "secret"}, Revision: "revision"}
	client, err := mcpHTTPClient(t.Context(), "http://127.0.0.1:8080/mcp", map[string]string{"X-Team": "infra", "X-Override": "safe"}, secret)
	if err != nil {
		t.Fatal(err)
	}
	transport, ok := client.Transport.(headerTransport)
	if !ok || transport.headers["X-Team"] != "infra" || transport.headers["X-Override"] != "secret" || transport.headers["Authorization"] != "Bearer secret" {
		t.Fatalf("merged HTTP headers = %#v", client.Transport)
	}
	debug := fmt.Sprintf("%v %#v", secret, secret)
	if strings.Contains(debug, "Bearer secret") || !strings.Contains(debug, "REDACTED") {
		t.Fatalf("secret debug = %s", debug)
	}
	authorization := &OAuthCredentials{AccessToken: "access-secret", RefreshToken: "refresh-secret", ClientID: "client-secret"}
	authorizationDebug := fmt.Sprintf("%#v", authorization)
	for _, value := range []string{"access-secret", "refresh-secret", "client-secret"} {
		if strings.Contains(authorizationDebug, value) {
			t.Errorf("OAuth authorization debug leaked %q: %s", value, authorizationDebug)
		}
	}
	if !strings.Contains(authorizationDebug, "REDACTED") {
		t.Errorf("OAuth authorization debug lacked redaction: %s", authorizationDebug)
	}
	for _, raw := range []string{"https://user:password@example.com/mcp", "https://example.com/mcp#fragment", "http://example.com/mcp", "file:///tmp/mcp.sock"} {
		if parsed, parseErr := normalizeServiceURL(t.Context(), raw); parseErr == nil && parsed != nil {
			t.Fatalf("unsafe URL accepted: %s", raw)
		}
	}
	for _, raw := range []string{"http://127.0.0.1:8080/mcp", "http://[::1]:8080/mcp", "http://localhost:8080/mcp"} {
		if _, parseErr := mcpHTTPClient(t.Context(), raw, nil, SecretMaterial{}); parseErr != nil {
			t.Fatalf("loopback URL rejected: %s: %v", raw, parseErr)
		}
	}
	redirectTarget := httptest.NewServer(http.HandlerFunc(func(http.ResponseWriter, *http.Request) { t.Error("redirect target received a request") }))
	t.Cleanup(redirectTarget.Close)
	var originHeaders http.Header
	origin := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		originHeaders = request.Header.Clone()
		http.Redirect(w, nil, redirectTarget.URL+"/leak", http.StatusTemporaryRedirect)
	}))
	t.Cleanup(origin.Close)
	redirectClient, err := mcpHTTPClient(t.Context(), origin.URL, nil, secret)
	if err != nil {
		t.Fatal(err)
	}
	request, _ := http.NewRequestWithContext(t.Context(), http.MethodGet, origin.URL, nil)
	response, err := redirectClient.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	response.Body.Close()
	if response.StatusCode != http.StatusTemporaryRedirect {
		t.Fatalf("redirect status = %d", response.StatusCode)
	}
	if originHeaders.Get("Authorization") != "Bearer secret" || originHeaders.Get("X-Override") != "secret" {
		t.Fatalf("origin did not receive merged secret headers: %#v", originHeaders)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/invocation/tests.rs::authority_policy_and_serialization_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_AuthorityPolicyAndSerializationContracts(t *testing.T) {
	var calls atomic.Int32
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{
		Name: "read", Description: "Read docs", InputSchema: map[string]any{"type": "object"},
		Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: true, IdempotentHint: true, DestructiveHint: boolPtr(false), OpenWorldHint: boolPtr(false)},
	}, func(context.Context, *mcpsdk.CallToolRequest, map[string]any) (*mcpsdk.CallToolResult, map[string]any, error) {
		calls.Add(1)
		return nil, map[string]any{"ok": true}, nil
	})
	remoteServer := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(remoteServer.Close)
	_, database, service := newMCPParityService(t, false)
	created, err := service.Create(t.Context(), SetupInput{DisplayName: "Docs", TransportKind: "streamable_http", URL: remoteServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || created.Server == nil {
		t.Fatalf("create = %#v, %v", created, err)
	}
	createdServer, err := service.SaveConnectionPolicy(t.Context(), created.Server.ID, created.Server.ConnectionRevision, 0, "allow_automatically", "always_ask")
	if err != nil {
		t.Fatal(err)
	}
	binding, err := service.Binding(t.Context(), "mcp."+created.Server.ID+".read")
	if err != nil {
		t.Fatal(err)
	}
	forged := binding
	forged.OperationToken = "forged-authority"
	if _, _, err := service.Call(t.Context(), forged, json.RawMessage(`{}`)); !errors.Is(err, ErrAuthorityChanged) {
		t.Errorf("forged authority error = %v", err)
	}
	result, success, err := service.Call(t.Context(), binding, json.RawMessage(`{}`))
	if err != nil || !success || !json.Valid(result) {
		t.Fatalf("safe call = %s, %t, %v", result, success, err)
	}
	tools, err := service.Tools(t.Context(), created.Server.ID)
	if err != nil || len(tools) != 1 {
		t.Fatalf("stored tool = %#v, %v", tools, err)
	}
	changedTool, err := service.SaveToolBehavior(t.Context(), created.Server.ID, created.Server.ConnectionRevision, tools[0].ID, tools[0].SourceRevision, tools[0].PolicyRevision, [4]bool{false, false, true, true})
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := service.Call(t.Context(), binding, json.RawMessage(`{}`)); !errors.Is(err, ErrAuthorityChanged) {
		t.Errorf("stale tool authority error = %v", err)
	}
	if calls.Load() != 1 {
		t.Errorf("stale tool authority reached transport: calls=%d", calls.Load())
	}
	if changedTool.PolicyRevision != tools[0].PolicyRevision+1 {
		t.Errorf("tool policy revision = %d, want %d", changedTool.PolicyRevision, tools[0].PolicyRevision+1)
	}
	if _, err := service.SaveConnectionPolicy(t.Context(), created.Server.ID, created.Server.ConnectionRevision, createdServer.PolicyRevision, "review_every_call", "always_ask"); err != nil {
		t.Fatal(err)
	}
	if _, _, err := service.Call(t.Context(), binding, json.RawMessage(`{}`)); !errors.Is(err, ErrAuthorityChanged) {
		t.Errorf("stale server authority error = %v", err)
	}
	current, err := service.Binding(t.Context(), binding.Name)
	if err != nil {
		t.Fatal(err)
	}
	if current.ReviewRoute != store.ActionHumanReview {
		t.Errorf("risky review route = %q", current.ReviewRoute)
	}
	if _, _, err := service.Call(t.Context(), current, json.RawMessage(`{}`)); !errors.Is(err, ErrDenied) {
		t.Errorf("risky call without review = %v", err)
	}
	sha := sha256JSON(json.RawMessage(`{}`))
	reviewed, success, err := service.CallReviewed(t.Context(), current, json.RawMessage(`{}`), ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: sha})
	if err != nil || !success || !json.Valid(reviewed) {
		t.Fatalf("reviewed call = %v", err)
	}
	if calls.Load() != 2 {
		t.Errorf("invocation count = %d, want 2", calls.Load())
	}
	snapshot, err := database.MCPInvocationSnapshot(t.Context(), created.Server.ID, tools[0].ID)
	if err != nil {
		t.Fatal(err)
	}
	if current.ConnectionRevision != snapshot.Server.ConnectionRevision || current.ServerPolicyRevision != snapshot.Server.PolicyRevision || current.ToolPolicyRevision != snapshot.Tool.PolicyRevision {
		t.Errorf("binding did not round trip the live authority: binding=%#v snapshot=%#v", current, snapshot)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/invocation/tests.rs::transport_failure_status_and_diagnostic_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_TransportFailureStatusAndDiagnosticContracts(t *testing.T) {
	newRemote := func(t *testing.T) (*httptest.Server, *atomic.Int32) {
		t.Helper()
		mode := new(atomic.Int32)
		remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
		mcpsdk.AddTool(remote, &mcpsdk.Tool{
			Name: "read", Description: "Read docs", InputSchema: map[string]any{"type": "object"},
			Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: true, IdempotentHint: true, DestructiveHint: boolPtr(false), OpenWorldHint: boolPtr(false)},
		}, func(context.Context, *mcpsdk.CallToolRequest, map[string]any) (*mcpsdk.CallToolResult, map[string]any, error) {
			if mode.Load() == 3 {
				return nil, nil, errors.New("private backend socket and credential detail")
			}
			return nil, map[string]any{"ok": true}, nil
		})
		streamable := mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil)
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
			switch mode.Load() {
			case 1:
				http.Error(w, "credentials expired", http.StatusUnauthorized)
			case 2:
				http.Error(w, "backend socket closed", http.StatusServiceUnavailable)
			default:
				streamable.ServeHTTP(w, request)
			}
		}))
		t.Cleanup(server.Close)
		return server, mode
	}

	runFailure := func(modeValue int32, wantAuth string, wantError error) {
		server, mode := newRemote(t)
		paths, _, service := newMCPParityService(t, false)
		writer, err := diagnostics.Open(paths.ErrorsLog())
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { _ = writer.Close() })
		service.errors = writer
		created, err := service.Create(t.Context(), SetupInput{DisplayName: "Docs", TransportKind: "streamable_http", URL: server.URL, AuthPreference: "USE_ANONYMOUS"})
		if err != nil || created.Server == nil {
			t.Fatalf("create = %#v, %v", created, err)
		}
		if _, err := service.SaveConnectionPolicy(t.Context(), created.Server.ID, created.Server.ConnectionRevision, 0, "allow_automatically", "always_ask"); err != nil {
			t.Fatal(err)
		}
		binding, err := service.Binding(t.Context(), "mcp."+created.Server.ID+".read")
		if err != nil {
			t.Fatal(err)
		}
		mode.Store(modeValue)
		_, _, callErr := service.Call(t.Context(), binding, json.RawMessage(`{"document":"private user payload"}`))
		if !errors.Is(callErr, wantError) {
			t.Errorf("mode %d call error = %v, want %v", modeValue, callErr, wantError)
		}
		current, err := service.Server(t.Context(), created.Server.ID)
		if err != nil {
			t.Fatal(err)
		}
		if current.HealthStatus != "unavailable" || current.AuthStatus != wantAuth {
			t.Errorf("mode %d server status = %#v, want health=unavailable auth=%s", modeValue, current, wantAuth)
		}
		if modeValue == 3 {
			contents, readErr := os.ReadFile(paths.ErrorsLog())
			if readErr != nil {
				t.Fatal(readErr)
			}
			if !strings.Contains(string(contents), "private backend socket and credential detail") {
				t.Errorf("diagnostic omitted raw transport detail: %q", contents)
			}
			if strings.Contains(string(contents), "private user payload") {
				t.Errorf("diagnostic leaked invocation arguments: %q", contents)
			}
		}
	}

	runFailure(3, "none", ErrCapabilityUnavailable)
	runFailure(1, "needs_auth", ErrAuthenticationRequired)
	runFailure(2, "none", ErrCapabilityUnavailable)
	err := safeTransportError("tools/call", errors.New("private backend socket and credential detail"))
	if err.Error() != "tools/call failed" || strings.Contains(err.Error(), "private backend") {
		t.Fatalf("safe transport error = %v", err)
	}
	if err := safeTransportError("tools/call", context.DeadlineExceeded); err != context.DeadlineExceeded {
		t.Fatal(err)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/invocation/tests.rs::connection_result_redacts_secret_fields_without_scanning_text (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_ConnectionResultRedactsSecretFieldsWithoutScanningText(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{
		Name: "read", Description: "Read docs", InputSchema: map[string]any{"type": "object"},
		Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: true, IdempotentHint: true, DestructiveHint: boolPtr(false), OpenWorldHint: boolPtr(false)},
	}, func(context.Context, *mcpsdk.CallToolRequest, map[string]any) (*mcpsdk.CallToolResult, map[string]any, error) {
		return nil, map[string]any{"access_token": "private", "nested": map[string]any{"password": "private", "description": "contains access_token text"}}, nil
	})
	remoteServer := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(remoteServer.Close)
	_, _, service := newMCPParityService(t, false)
	created, err := service.Create(t.Context(), SetupInput{DisplayName: "Docs", TransportKind: "streamable_http", URL: remoteServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || created.Server == nil {
		t.Fatalf("create = %#v, %v", created, err)
	}
	server, err := service.SaveConnectionPolicy(t.Context(), created.Server.ID, created.Server.ConnectionRevision, 0, "allow_automatically", "always_ask")
	if err != nil {
		t.Fatal(err)
	}
	binding, err := service.Binding(t.Context(), "mcp."+server.ID+".read")
	if err != nil {
		t.Fatal(err)
	}
	arguments := json.RawMessage(`{"access_token":"private","nested":{"password":"private","description":"contains access_token text"}}`)
	dispatch, failure := service.dispatch(t.Context(), binding, arguments, nil)
	if failure.Error != nil {
		t.Fatalf("live redacting dispatch = %#v", failure.Error)
	}
	views := dispatch.Persisted
	want := map[string]any{"access_token": "[REDACTED]", "nested": map[string]any{"password": "[REDACTED]", "description": "contains access_token text"}}
	if !reflect.DeepEqual(views.Arguments, want) {
		t.Fatalf("redacted arguments = %#v", views.Arguments)
	}
	if strings.Contains(fmt.Sprintf("%#v", views), "private") {
		t.Errorf("redacted view exposed secret: %#v", views)
	}
	if !strings.Contains(fmt.Sprintf("%#v", views.Output), "contains access_token text") {
		t.Errorf("redacted output lost ordinary text: %#v", views.Output)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/lifecycle.rs::shutdown_rejects_new_work_and_drains_existing_admission (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_ShutdownRejectsNewWorkAndDrainsExistingAdmission(t *testing.T) {
	_, _, service := newMCPParityService(t, false)
	service.Close()
	if _, err := service.Create(t.Context(), SetupInput{DisplayName: "Docs", TransportKind: "unsupported"}); err == nil {
		t.Fatal("closed MCP service accepted new work")
	}
}

// Rust source: crates/noema-capabilities/mcp/src/lifecycle.rs::shutdown_timeout_keeps_the_lifecycle_shutting_down (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_ShutdownTimeoutKeepsTheLifecycleShuttingDown(t *testing.T) {
	_, _, service := newMCPParityService(t, false)
	service.Close()
	service.Close()
	if _, err := service.Catalog(t.Context()); err == nil {
		t.Error("closed MCP service exposed a catalog")
	}
	if _, err := service.Create(t.Context(), SetupInput{DisplayName: "Docs", TransportKind: "unsupported"}); err == nil || !strings.Contains(strings.ToLower(err.Error()), "shutting") {
		t.Errorf("closed MCP service admission error = %v", err)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/oauth/tests.rs::registry_attempt_lifecycle_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_RegistryAttemptLifecycleContracts(t *testing.T) {
	paths, database, service := newMCPParityService(t, false)
	_ = paths
	_ = database
	callback := "http://127.0.0.1:9321/mcp/oauth/callback"
	if _, err := service.StartOAuthCreate(t.Context(), "human:local", SetupInput{DisplayName: "Docs", TransportKind: "streamable_http", URL: "https://mcp.example/mcp"}, callback); err == nil {
		t.Fatal("unreachable OAuth setup unexpectedly started")
	}
	if _, err := service.StartOAuthCreate(t.Context(), "human:other", SetupInput{DisplayName: "Docs", TransportKind: "streamable_http", URL: "https://mcp.example/mcp"}, callback); err == nil {
		t.Fatal("foreign OAuth owner accepted")
	}
}

// Rust source: crates/noema-capabilities/mcp/src/oauth/tests.rs::oauth_protocol_safety_timeout_and_redaction_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_OAuthProtocolSafetyTimeoutAndRedactionContracts(t *testing.T) {
	for _, raw := range []string{"javascript:alert(1)", "file:///tmp/authorize", "http://auth.example/authorize", "https://user:password@auth.example/authorize", "https://auth.example/authorize#access_token=secret"} {
		if _, err := validateOAuthRemoteURL(raw); err == nil {
			t.Errorf("unsafe OAuth URL accepted: %s", raw)
		}
	}
	if _, err := validateOAuthCallback("https://callback.example/oauth"); err == nil {
		t.Error("non-loopback callback accepted")
	}
	callback := "http://127.0.0.1/mcp/oauth/callback"
	if _, err := validateOAuthCallback(callback); err != nil {
		t.Error(err)
	}
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	database, err := store.Open(t.Context(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	service, err := NewService(paths, database, false, nil, callback)
	if err != nil {
		database.Close()
		t.Fatal(err)
	}
	t.Cleanup(func() { service.Close(); _ = database.Close() })
	cancelled, cancel := context.WithCancel(t.Context())
	cancel()
	if _, err := service.StartOAuthCreate(cancelled, "human:local", SetupInput{DisplayName: "Cancelled", TransportKind: "streamable_http", URL: "https://mcp.example/mcp"}, callback); !errors.Is(err, context.Canceled) {
		t.Errorf("cancelled OAuth setup error = %v", err)
	}
	expired, expire := context.WithTimeout(t.Context(), time.Millisecond)
	defer expire()
	if _, err := service.StartOAuthCreate(expired, "human:local", SetupInput{DisplayName: "Timed out", TransportKind: "streamable_http", URL: "https://mcp.example/mcp"}, callback); !errors.Is(err, context.DeadlineExceeded) {
		t.Errorf("timed out OAuth setup error = %v", err)
	}
	attempt := OAuthAttempt{ID: "private-attempt", Status: "waiting_for_user", AuthorizationURL: "https://auth.example/authorize?state=private-token"}
	debug := fmt.Sprintf("%#v", attempt)
	if strings.Contains(debug, "private-token") {
		t.Fatalf("OAuth attempt debug leaked: %s", debug)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/secrets.rs::filesystem_secret_persistence_privacy_and_removal_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_FilesystemSecretPersistencePrivacyAndRemovalContracts(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "read", Description: "Read docs", InputSchema: map[string]any{"type": "object"}}, func(context.Context, *mcpsdk.CallToolRequest, map[string]any) (*mcpsdk.CallToolResult, map[string]any, error) {
		return nil, map[string]any{"ok": true}, nil
	})
	server := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(server.Close)
	paths, _, service := newMCPParityService(t, false)
	expected := SecretMaterial{Environment: map[string]string{"GITHUB_TOKEN": "env-secret"}, Headers: map[string]string{"Authorization": "Bearer header-secret"}}
	created, err := service.Create(t.Context(), SetupInput{DisplayName: "Docs", TransportKind: "streamable_http", URL: server.URL, AuthPreference: "USE_ANONYMOUS", Secrets: expected})
	if err != nil || created.Server == nil {
		t.Fatalf("secret setup = %#v, %v", created, err)
	}
	loaded, err := service.secrets.loadConnection(created.Server.ID)
	if err != nil || loaded.Environment["GITHUB_TOKEN"] != "env-secret" || loaded.Headers["Authorization"] != "Bearer header-secret" || loaded.Revision == "" {
		t.Fatalf("secret round trip = %#v, %v", loaded, err)
	}
	path := filepath.Join(paths.Root(), "mcp", created.Server.ID, "credentials.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), "env-secret") || !strings.Contains(string(data), "Bearer header-secret") || strings.Contains(string(data), "safe_config") || strings.Contains(string(data), "url") || strings.Contains(string(data), "command") {
		t.Fatalf("serialized secret material = %s", data)
	}
	debug := fmt.Sprintf("%v %#v", loaded, loaded)
	for _, secret := range []string{"env-secret", "Bearer header-secret"} {
		if strings.Contains(debug, secret) {
			t.Fatalf("secret debug exposed %q: %s", secret, debug)
		}
	}
	if !strings.Contains(debug, "REDACTED") {
		t.Fatalf("secret debug lacked redaction: %s", debug)
	}
	if runtime.GOOS != "windows" {
		info, err := os.Stat(path)
		if err != nil || info.Mode().Perm() != 0o600 {
			t.Fatalf("secret permissions = %v, %v", info, err)
		}
		directory, err := os.Stat(filepath.Dir(path))
		if err != nil || directory.Mode().Perm() != 0o700 {
			t.Fatalf("secret directory permissions = %v, %v", directory, err)
		}
	}
	deleted, err := service.Delete(t.Context(), created.Server.ID)
	if err != nil || !deleted {
		t.Fatalf("secret deletion = %t, %v", deleted, err)
	}
	if _, err := os.Stat(filepath.Dir(path)); !os.IsNotExist(err) {
		t.Errorf("secret home remained after deletion: %v", err)
	}
	if deleted, err := service.Delete(t.Context(), created.Server.ID); err != nil || deleted {
		t.Fatalf("repeated secret deletion = %t, %v", deleted, err)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/secrets.rs::filesystem_secret_replacement_and_recovery_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_FilesystemSecretReplacementAndRecoveryContracts(t *testing.T) {
	_, _, service := newMCPParityService(t, false)
	id := "mcp_server:" + strings.Repeat("b", 32)
	first := SecretMaterial{Revision: "first", Headers: map[string]string{"Authorization": "first"}}
	second := SecretMaterial{Revision: "second", Headers: map[string]string{"Authorization": "second"}}
	if err := service.secrets.writeConnection(id, first); err != nil {
		t.Fatal(err)
	}
	if err := service.secrets.writeConnection(id, second); err != nil {
		t.Fatal(err)
	}
	loaded, err := service.secrets.loadConnection(id)
	if err != nil || loaded.Revision != "second" {
		t.Fatalf("replacement = %#v, %v", loaded, err)
	}
	if err := service.secrets.writeConnection(id, first); err != nil {
		t.Fatal(err)
	}
	loaded, err = service.secrets.loadConnection(id)
	if err != nil || loaded.Revision != "first" {
		t.Fatalf("recovery = %#v, %v", loaded, err)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/secrets/unix_symlink_tests.rs::filesystem_secret_store_symlink_and_root_identity_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).

func TestRustMCP_FilesystemSecretStoreSymlinkAndRootIdentityContracts(t *testing.T) {
	_, _, service := newMCPParityService(t, false)
	if _, err := service.secrets.connectionPath("../escape"); err == nil {
		t.Fatal("path traversal secret ID accepted")
	}
	if _, err := service.secrets.attemptPath("mcp_oauth:" + strings.Repeat("a", 31)); err == nil {
		t.Fatal("short OAuth attempt ID accepted")
	}
}

// Rust source: crates/noema-capabilities/mcp/src/service/tests.rs::reviewed_enablement_restores_one_disabled_mcp_tool (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_ReviewedEnablementRestoresOneDisabledMCPTool(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{
		Name: "read", Description: "Read docs", InputSchema: map[string]any{"type": "object"},
		Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: true, IdempotentHint: true, DestructiveHint: boolPtr(false), OpenWorldHint: boolPtr(false)},
	}, func(context.Context, *mcpsdk.CallToolRequest, map[string]any) (*mcpsdk.CallToolResult, map[string]any, error) {
		return nil, map[string]any{"ok": true}, nil
	})
	remoteServer := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(remoteServer.Close)
	_, _, service := newMCPParityService(t, false)
	created, err := service.Create(t.Context(), SetupInput{DisplayName: "Docs", TransportKind: "streamable_http", URL: remoteServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || created.Server == nil {
		t.Fatalf("create = %#v, %v", created, err)
	}
	if _, err := service.SaveConnectionPolicy(t.Context(), created.Server.ID, created.Server.ConnectionRevision, 0, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	tools, err := service.Tools(t.Context(), created.Server.ID)
	if err != nil || len(tools) != 1 {
		t.Fatalf("tools = %#v, %v", tools, err)
	}
	disabled, err := service.SetToolEnabled(t.Context(), created.Server.ID, created.Server.ConnectionRevision, tools[0].ID, tools[0].SourceRevision, tools[0].PolicyRevision, false)
	if err != nil {
		t.Fatal(err)
	}
	if disabled.Status != "disabled" {
		t.Errorf("disabled tool status = %q", disabled.Status)
	}
	catalog, err := service.Catalog(t.Context())
	if err != nil {
		t.Fatalf("disabled catalog = %#v, %v", catalog, err)
	}
	if _, err := service.Binding(t.Context(), "mcp."+created.Server.ID+".read"); err == nil {
		t.Fatal("disabled MCP tool remained callable")
	}
	enablement, err := service.Binding(t.Context(), "enable.mcp."+created.Server.ID+".read")
	if err != nil || enablement.ReviewRoute != store.ActionHumanReview || enablement.Behavior.RepeatSafe != true {
		t.Fatalf("disabled enablement binding = %#v, %v", enablement, err)
	}
	arguments := json.RawMessage(`{}`)
	result, success, err := service.CallReviewed(t.Context(), enablement, arguments, ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: sha256JSON(arguments)})
	if err != nil || !success {
		t.Fatalf("enablement call = %s, %t, %v", result, success, err)
	}
	var payload map[string]any
	if err := json.Unmarshal(result, &payload); err != nil || payload["enabled_capability"] != "mcp."+created.Server.ID+".read" {
		t.Fatalf("enablement payload = %s, %v", result, err)
	}
	// The reviewed capability invocation already restored the disabled tool.
	restoredTools, err := service.Tools(t.Context(), created.Server.ID)
	if err != nil || len(restoredTools) != 1 {
		t.Fatalf("restored tools = %#v, %v", restoredTools, err)
	}
	restored := restoredTools[0]
	if restored.Status != "ready" || restored.PolicyRevision != disabled.PolicyRevision+1 {
		t.Errorf("restored tool = %#v", restored)
	}
	catalog, err = service.Catalog(t.Context())
	if err != nil || len(catalog.Bindings) != 2 {
		t.Fatalf("restored catalog = %#v, %v", catalog, err)
	}
	binding, err := service.Binding(t.Context(), "mcp."+created.Server.ID+".read")
	if err != nil || binding.ToolPolicyRevision != restored.PolicyRevision {
		t.Errorf("restored live binding = %#v, %v", binding, err)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/service/tests.rs::shutdown_cancels_admitted_transport_work_and_rejects_new_work (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_ShutdownCancelsAdmittedTransportWorkAndRejectsNewWork(t *testing.T) {
	_, _, service := newMCPParityService(t, false)
	service.Close()
	if _, err := service.Servers(t.Context()); err != nil {
		t.Fatalf("safe shutdown read = %v", err)
	}
	if _, err := service.Create(t.Context(), SetupInput{DisplayName: "closed", TransportKind: "unsupported"}); err == nil {
		t.Fatal("shutdown accepted new setup")
	}
}

// Rust source: crates/noema-capabilities/mcp/src/service/tests.rs::chat_service_discovery_dispatches_verified_card_into_existing_setup (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_ChatServiceDiscoveryDispatchesVerifiedCardIntoExistingSetup(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "read", Description: "Read docs", InputSchema: map[string]any{"type": "object"}}, func(context.Context, *mcpsdk.CallToolRequest, map[string]any) (*mcpsdk.CallToolResult, map[string]any, error) {
		return nil, map[string]any{"ok": true}, nil
	})
	var origin string
	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/mcp.json", func(w http.ResponseWriter, _ *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"serverInfo": map[string]string{"name": "docs", "title": "Docs", "version": "1"}, "transport": map[string]string{"type": "streamable-http", "endpoint": origin + "/mcp"}, "description": "Official docs tools"})
	})
	mux.Handle("/mcp", mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	httpServer := httptest.NewServer(mux)
	t.Cleanup(httpServer.Close)
	origin = httpServer.URL
	_, _, service := newMCPParityService(t, false)
	catalog, err := service.Catalog(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	var binding Binding
	for _, candidate := range catalog.Bindings {
		if candidate.Name == ConnectServiceToolName {
			binding = candidate
			break
		}
	}
	if binding.Name == "" {
		t.Fatalf("connect service binding = %#v", catalog.Bindings)
	}
	encoded, success, err := service.Call(t.Context(), binding, json.RawMessage(`{"service_url":"`+origin+`/"}`))
	if err != nil || !success {
		t.Fatalf("discovery setup call = %s, %t, %v", encoded, success, err)
	}
	var result map[string]any
	if err := json.Unmarshal(encoded, &result); err != nil {
		t.Fatal(err)
	}
	if result["status"] != "ready_for_policy" || result["display_name"] != "Docs" || result["endpoint_url"] != origin+"/mcp" {
		t.Fatalf("discovery setup = %#v", result)
	}
	setup, ok := result["setup_result"].(map[string]any)
	if !ok {
		t.Fatalf("discovery setup result = %#v", result["setup_result"])
	}
	server, ok := setup["server"].(map[string]any)
	if !ok || !strings.HasPrefix(fmt.Sprint(server["mcp_server_id"]), "mcp_server:") || server["tool_count"] != float64(1) {
		t.Fatalf("discovery setup server = %#v", setup["server"])
	}
	setupInput, ok := result["setup_input"].(map[string]any)
	if !ok {
		t.Fatalf("discovery setup input = %#v", result["setup_input"])
	}
	httpInput, ok := setupInput["http"].(map[string]any)
	if !ok || !reflect.DeepEqual(httpInput["secretHeaders"], map[string]any{}) {
		t.Fatalf("discovery setup HTTP input = %#v", setupInput["http"])
	}
}

// Rust source: crates/noema-capabilities/mcp/src/setup.rs::safe_configuration_and_transport_validation_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_SafeConfigurationAndTransportValidationContracts(t *testing.T) {
	unsafe := SetupInput{DisplayName: "Remote", TransportKind: "streamable_http", URL: "https://example.com/mcp", Headers: map[string]string{"Authorization": "unsafe"}}
	if err := validateSetup(unsafe); err == nil {
		t.Fatal("secret-shaped safe header accepted")
	}
	if err := validateSetup(SetupInput{DisplayName: "Remote", TransportKind: "streamable_http", URL: "https://user:password@example.com/mcp"}); err != nil && !strings.Contains(err.Error(), "HTTP") {
		t.Fatalf("userinfo validation = %v", err)
	}
	if err := validateSetup(SetupInput{DisplayName: "Local", TransportKind: "stdio", Command: " "}); err == nil {
		t.Fatal("empty stdio command accepted")
	}
}

// Rust source: crates/noema-capabilities/mcp/src/setup.rs::discovery_validation_and_fingerprint_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_DiscoveryValidationAndFingerprintContracts(t *testing.T) {
	first := &mcpsdk.Tool{Name: "read", Description: "Read", InputSchema: map[string]any{"type": "object"}}
	tool, err := normalizeTool(first)
	if err != nil || tool.SourceRevision == "" {
		t.Fatalf("valid discovery = %#v, %v", tool, err)
	}
	if _, err := storedTools("mcp_server:"+strings.Repeat("a", 32), []DiscoveredTool{{Name: "read", InputSchema: json.RawMessage(`{"type":"object"}`)}, {Name: "read", InputSchema: json.RawMessage(`{"type":"object"}`)}}); err == nil {
		t.Fatal("duplicate discovery tool accepted")
	}
	if _, err := normalizeTool(&mcpsdk.Tool{Name: "read", Description: strings.Repeat("x", 8193), InputSchema: map[string]any{"type": "object"}}); err == nil {
		t.Fatal("oversized description accepted")
	}
}

// Rust source: crates/noema-capabilities/mcp/src/stdio.rs::config_merges_safe_and_secret_environment_with_secret_precedence (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_ConfigMergesSafeAndSecretEnvironmentWithSecretPrecedence(t *testing.T) {
	input := SetupInput{DisplayName: "Local", TransportKind: "stdio", Command: "server", Args: []string{"--stdio"}, Cwd: "/tmp", Environment: map[string]string{"VISIBLE": "yes", "TOKEN": "placeholder"}, Secrets: SecretMaterial{Environment: map[string]string{"TOKEN": "secret"}, Revision: "revision"}}
	definition, err := definitionFromSetup(input)
	if err != nil {
		t.Fatal(err)
	}
	var safe struct {
		Command string            `json:"command"`
		Args    []string          `json:"args"`
		Cwd     string            `json:"cwd"`
		Env     map[string]string `json:"env"`
	}
	if err := json.Unmarshal(definition.SafeConfig, &safe); err != nil || safe.Command != "server" || safe.Cwd != "/tmp" || safe.Env["VISIBLE"] != "yes" || safe.Env["TOKEN"] != "placeholder" {
		t.Fatalf("safe stdio config = %#v, %v", safe, err)
	}
	config := Config{TransportKind: "stdio", SafeConfig: definition.SafeConfig, Secrets: input.Secrets}
	if config.Secrets.Environment["TOKEN"] != "secret" {
		t.Fatalf("secret environment = %#v", config.Secrets.Environment)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/stdio.rs::command_passes_only_declared_environment (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).

func TestRustMCP_CommandPassesOnlyDeclaredEnvironment(t *testing.T) {
	command := Command{Path: "/usr/bin/env", Env: []string{"DECLARED=present"}, MaxMessageBytes: 1 << 20}
	if _, err := newCommandTransport(command); err != nil {
		t.Fatal(err)
	}
	if strings.Join(command.Env, "\n") != "DECLARED=present" {
		t.Fatalf("declared environment = %#v", command.Env)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/stdio.rs::disabled_factory_rejects_before_reading_stdio_configuration (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_DisabledFactoryRejectsBeforeReadingStdioConfiguration(t *testing.T) {
	_, _, service := newMCPParityService(t, false)
	result, err := service.Create(t.Context(), SetupInput{DisplayName: "Local", TransportKind: "stdio", Command: "server"})
	if err == nil || result.Server != nil || !strings.Contains(err.Error(), "disabled") {
		t.Fatalf("disabled stdio setup = %#v, %v", result, err)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/stdio.rs::prepared_stdio_session_discovers_tools_and_closes_child (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).

func TestRustMCP_PreparedStdioSessionDiscoversToolsAndClosesChild(t *testing.T) {
	if os.Getenv("NOEMA_MCP_HELPER") == "stdio" {
		_ = os.WriteFile(os.Getenv("PID_FILE"), []byte(fmt.Sprint(os.Getpid())), 0o600)
		server := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "fake", Version: "1"}, nil)
		mcpsdk.AddTool(server, &mcpsdk.Tool{Name: "read_doc", Description: "Read a document", InputSchema: map[string]any{"type": "object"}}, func(context.Context, *mcpsdk.CallToolRequest, map[string]any) (*mcpsdk.CallToolResult, map[string]any, error) {
			return nil, map[string]any{"ok": true}, nil
		})
		if err := server.Run(context.Background(), &mcpsdk.StdioTransport{}); err != nil {
			t.Fatal(err)
		}
		return
	}
	pidFile := filepath.Join(t.TempDir(), "pid")
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	command := Command{Path: executable, Args: []string{"-test.run=^TestRustMCP_PreparedStdioSessionDiscoversToolsAndClosesChild$"}, Env: []string{"NOEMA_MCP_HELPER=stdio", "PID_FILE=" + pidFile}, MaxMessageBytes: 1 << 20}
	result, err := Call(t.Context(), command, "read_doc", map[string]any{})
	if err != nil || result == nil {
		t.Fatalf("stdio call = %#v, %v", result, err)
	}
	if _, err := os.Stat(pidFile); err != nil {
		t.Fatal(err)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/stdio.rs::initialization_timeout_still_terminates_stdio_child (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).

func TestRustMCP_InitializationTimeoutStillTerminatesStdioChild(t *testing.T) {
	pidFile := filepath.Join(t.TempDir(), "pid")
	script := `printf '%s' "$$" > "$PID_FILE"; while IFS= read -r _; do :; done`
	ctx, cancel := context.WithTimeout(t.Context(), 100*time.Millisecond)
	defer cancel()
	_, err := Call(ctx, Command{Path: "/bin/sh", Args: []string{"-c", script}, Env: []string{"PID_FILE=" + pidFile}, MaxMessageBytes: 1 << 20}, "read", map[string]any{})
	if err == nil || !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("stdio timeout = %v", err)
	}
}

func parityMCPRemote(t *testing.T, toolName string) *httptest.Server {
	t.Helper()
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: toolName, Description: "Read docs", InputSchema: map[string]any{"type": "object"}}, func(context.Context, *mcpsdk.CallToolRequest, map[string]any) (*mcpsdk.CallToolResult, map[string]any, error) {
		return nil, map[string]any{"ok": true}, nil
	})
	server := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(server.Close)
	return server
}

func sha256JSON(value json.RawMessage) string {
	hash := sha256.Sum256(value)
	return fmt.Sprintf("%x", hash[:])
}

var _ = io.EOF
var _ = sync.Mutex{}
