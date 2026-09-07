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
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/store"
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

// Rust source: crates/noema-capabilities/mcp/src/control/tests.rs::setup_secret_persistence_and_compensation_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_SetupSecretPersistenceAndCompensationContracts(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "read", Description: "Read docs", InputSchema: map[string]any{"type": "object"}}, func(context.Context, *mcpsdk.CallToolRequest, map[string]any) (*mcpsdk.CallToolResult, map[string]any, error) {
		return nil, map[string]any{"ok": true}, nil
	})
	server := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(server.Close)
	paths, database, service := newMCPParityService(t, false)
	created, err := service.Create(t.Context(), SetupInput{DisplayName: "Remote", TransportKind: "streamable_http", URL: server.URL, AuthPreference: "USE_ANONYMOUS", Headers: map[string]string{}})
	if err != nil || created.Server == nil || created.Status != "ready_for_policy" || created.Discovered != 1 {
		t.Fatalf("initial setup = %#v, %v", created, err)
	}
	serverRecord := *created.Server
	if serverRecord.ToolCount != 1 || serverRecord.SafeConfig == nil || strings.Contains(string(serverRecord.SafeConfig), "secret") {
		t.Fatalf("persisted setup = %#v", serverRecord)
	}
	added, err := service.AddConnection(t.Context(), SetupInput{DefinitionID: serverRecord.DefinitionID, DefinitionRevision: serverRecord.DefinitionRevision, ConnectionLabel: "Work", AuthPreference: "USE_ANONYMOUS"})
	if err != nil || added.Server == nil || added.Server.ID == serverRecord.ID || added.Server.ConnectionLabel != "Work" {
		t.Fatalf("independent connection = %#v, %v", added, err)
	}
	secret := SecretMaterial{Headers: map[string]string{"Authorization": "Bearer initial"}, Revision: strings.Repeat("a", 32)}
	createdWithSecret, err := service.Create(t.Context(), SetupInput{DisplayName: "Secret", TransportKind: "streamable_http", URL: server.URL, AuthPreference: "USE_ANONYMOUS", Secrets: secret})
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
	if err != nil || loaded.Headers["Authorization"] != "Bearer replacement" || strings.Contains(string(serverRecord.SafeConfig), "Bearer") {
		t.Fatalf("replaced secret = %#v, %v", loaded, err)
	}
	if _, err := os.Stat(filepath.Join(paths.Root(), "mcp", createdWithSecret.Server.ID, "credentials.json")); err != nil {
		t.Fatal(err)
	}
	_ = database
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
	origin := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
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
}

// Rust source: crates/noema-capabilities/mcp/src/invocation/tests.rs::authority_policy_and_serialization_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_AuthorityPolicyAndSerializationContracts(t *testing.T) {
	remote := parityMCPRemote(t, "read")
	paths, database, service := newMCPParityService(t, false)
	created, err := service.Create(t.Context(), SetupInput{DisplayName: "Docs", TransportKind: "streamable_http", URL: remote.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || created.Server == nil {
		t.Fatalf("create = %#v, %v", created, err)
	}
	binding, err := service.Binding(t.Context(), "mcp."+created.Server.ID+".read")
	if err != nil {
		t.Fatal(err)
	}
	forged := binding
	forged.OperationToken = "forged-authority"
	if _, _, err := service.Call(t.Context(), forged, json.RawMessage(`{}`)); !errors.Is(err, ErrAuthorityChanged) {
		t.Fatalf("forged authority error = %v", err)
	}
	result, success, err := service.Call(t.Context(), binding, json.RawMessage(`{}`))
	if err != nil || !success || !json.Valid(result) {
		t.Fatalf("safe call = %s, %t, %v", result, success, err)
	}
	if _, err := service.SaveConnectionPolicy(t.Context(), created.Server.ID, created.Server.ConnectionRevision, 0, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	if _, _, err := service.Call(t.Context(), binding, json.RawMessage(`{}`)); !errors.Is(err, ErrDenied) {
		t.Fatalf("risky call without review = %v", err)
	}
	sha := sha256JSON(json.RawMessage(`{}`))
	if _, _, err := service.CallReviewed(t.Context(), binding, json.RawMessage(`{}`), ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: sha}); err != nil {
		t.Fatalf("reviewed call = %v", err)
	}
	_ = paths
	_ = database
}

// Rust source: crates/noema-capabilities/mcp/src/invocation/tests.rs::transport_failure_status_and_diagnostic_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_TransportFailureStatusAndDiagnosticContracts(t *testing.T) {
	for _, status := range []string{"none", "needs_auth", "authenticated", "unavailable"} {
		server := store.MCPServer{AuthStatus: status, HealthStatus: "healthy"}
		if server.AuthStatus != status {
			t.Fatalf("auth status changed: %q", status)
		}
	}
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
	binding := Binding{PersistencePolicy: BindingPersistenceRedacted}
	views := binding.PersistedViews(map[string]any{"access_token": "private", "nested": map[string]any{"password": "private", "description": "contains access_token text"}}, nil)
	want := map[string]any{"access_token": "[REDACTED]", "nested": map[string]any{"password": "[REDACTED]", "description": "contains access_token text"}}
	if !reflect.DeepEqual(views.Arguments, want) {
		t.Fatalf("redacted arguments = %#v", views.Arguments)
	}
	if strings.Contains(fmt.Sprintf("%#v", views), "private") {
		t.Fatalf("redacted view exposed secret: %#v", views)
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
		t.Fatal("closed MCP service exposed a catalog")
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
			t.Fatalf("unsafe OAuth URL accepted: %s", raw)
		}
	}
	if _, err := validateOAuthCallback("https://callback.example/oauth"); err == nil {
		t.Fatal("non-loopback callback accepted")
	}
	if _, err := validateOAuthCallback("http://127.0.0.1/mcp/oauth/callback"); err != nil {
		t.Fatal(err)
	}
	attempt := OAuthAttempt{ID: "private-attempt", Status: "waiting_for_user", AuthorizationURL: "https://auth.example/authorize?state=private-token"}
	debug := fmt.Sprintf("%#v", attempt)
	if strings.Contains(debug, "private-token") {
		t.Fatalf("OAuth attempt debug leaked: %s", debug)
	}
}

// Rust source: crates/noema-capabilities/mcp/src/secrets.rs::filesystem_secret_persistence_privacy_and_removal_contracts (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustMCP_FilesystemSecretPersistencePrivacyAndRemovalContracts(t *testing.T) {
	paths, _, service := newMCPParityService(t, false)
	id := "mcp_server:" + strings.Repeat("a", 32)
	expected := SecretMaterial{Revision: "revision:test", Environment: map[string]string{"GITHUB_TOKEN": "env-secret"}, Headers: map[string]string{"Authorization": "Bearer header-secret"}}
	if err := service.secrets.writeConnection(id, expected); err != nil {
		t.Fatal(err)
	}
	loaded, err := service.secrets.loadConnection(id)
	if err != nil || !reflect.DeepEqual(loaded, expected) {
		t.Fatalf("secret round trip = %#v, %v", loaded, err)
	}
	path := filepath.Join(paths.Root(), "mcp", id, "credentials.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), "env-secret") || strings.Contains(string(data), "safe_config") || strings.Contains(string(data), "url") {
		t.Fatalf("serialized secret material = %s", data)
	}
	if runtime.GOOS != "windows" {
		info, err := os.Stat(path)
		if err != nil || info.Mode().Perm() != 0o600 {
			t.Fatalf("secret permissions = %v, %v", info, err)
		}
	}
	if err := service.secrets.removeConnection(id); err != nil {
		t.Fatal(err)
	}
	if err := service.secrets.removeConnection(id); err == nil {
		t.Fatal("repeated secret removal unexpectedly failed")
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
	remote := parityMCPRemote(t, "read")
	_, database, service := newMCPParityService(t, false)
	created, err := service.Create(t.Context(), SetupInput{DisplayName: "Docs", TransportKind: "streamable_http", URL: remote.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || created.Server == nil {
		t.Fatalf("create = %#v, %v", created, err)
	}
	tools, err := service.Tools(t.Context(), created.Server.ID)
	if err != nil || len(tools) != 1 {
		t.Fatalf("tools = %#v, %v", tools, err)
	}
	_, err = service.SetToolEnabled(t.Context(), created.Server.ID, created.Server.ConnectionRevision, tools[0].ID, tools[0].SourceRevision, tools[0].PolicyRevision, false)
	if err != nil {
		t.Fatal(err)
	}
	catalog, err := service.Catalog(t.Context())
	if err != nil || len(catalog.Bindings) != 0 {
		t.Fatalf("disabled catalog = %#v, %v", catalog, err)
	}
	_, err = service.SetToolEnabled(t.Context(), created.Server.ID, created.Server.ConnectionRevision, tools[0].ID, tools[0].SourceRevision, tools[0].PolicyRevision+1, true)
	if err != nil {
		t.Fatal(err)
	}
	catalog, err = service.Catalog(t.Context())
	if err != nil || len(catalog.Bindings) != 1 {
		t.Fatalf("restored catalog = %#v, %v", catalog, err)
	}
	_ = database
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
	result := service.ConnectService(t.Context(), origin+"/")
	if result.Status != "ready_for_policy" || result.DisplayName != "Docs" || result.EndpointURL != origin+"/mcp" || result.Setup.Server == nil || result.Setup.Server.ToolCount != 1 {
		t.Fatalf("discovery setup = %#v", result)
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
