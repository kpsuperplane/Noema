package mcp

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

func TestHTTPDiscoveryCallAndExactSourceFence(t *testing.T) {
	server := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "calendar", Version: "1"}, nil)
	add := func(description string) {
		mcpsdk.AddTool(server, &mcpsdk.Tool{Name: "lookup", Description: description,
			Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: true, IdempotentHint: true,
				DestructiveHint: boolTestPointer(false), OpenWorldHint: boolTestPointer(true)}},
			func(_ context.Context, _ *mcpsdk.CallToolRequest, input echoInput) (*mcpsdk.CallToolResult, echoOutput, error) {
				return nil, echoOutput{Text: input.Text}, nil
			})
	}
	add("Find one event")
	httpServer := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return server }, nil))
	defer httpServer.Close()
	config := Config{TransportKind: "streamable_http", SafeConfig: json.RawMessage(`{"url":"` + httpServer.URL + `"}`)}
	discovery, err := Discover(context.Background(), config)
	if err != nil || len(discovery.Tools) != 1 {
		t.Fatalf("discovery = %#v, %v", discovery, err)
	}
	revision := discovery.Tools[0].SourceRevision
	result, success, err := CallExact(context.Background(), config, "lookup", revision, map[string]any{"text": "today"})
	if err != nil || !success || !json.Valid(result) {
		t.Fatalf("call = %s, %t, %v", result, success, err)
	}
	server.RemoveTools("lookup")
	add("Find one changed event")
	if _, _, err := CallExact(context.Background(), config, "lookup", revision, map[string]any{"text": "today"}); err == nil {
		t.Fatal("changed source revision was called")
	}

	unauthorized := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) { w.WriteHeader(http.StatusUnauthorized) }))
	defer unauthorized.Close()
	_, err = Discover(context.Background(), Config{TransportKind: "streamable_http", SafeConfig: json.RawMessage(`{"url":"` + unauthorized.URL + `"}`)})
	if !errors.Is(err, ErrAuthenticationRequired) {
		t.Fatalf("authentication error = %v", err)
	}

	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(paths.Database()), 0o700); err != nil {
		t.Fatal(err)
	}
	database, err := store.Open(t.Context(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	service, err := NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	var origin string
	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/mcp.json", func(w http.ResponseWriter, _ *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"title": "Official Calendar", "description": "Calendar tools",
			"transport": map[string]any{"type": "streamable-http", "endpoint": origin + "/mcp"}})
	})
	mux.Handle("/mcp", mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return server }, nil))
	cardServer := httptest.NewServer(mux)
	defer cardServer.Close()
	origin = cardServer.URL
	connected := service.ConnectService(t.Context(), origin+"/product?private=removed")
	if connected.Status != "ready_for_policy" || connected.Setup.Server == nil ||
		connected.ServiceURL != origin+"/" || connected.EndpointURL != origin+"/mcp" {
		t.Fatalf("server-card setup = %#v", connected)
	}
}

func TestToolErrorDoesNotFenceHealthyMCPConnection(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "notes", Version: "1"}, nil)
	remote.AddTool(&mcpsdk.Tool{
		Name: "search", Description: "Search notes", InputSchema: map[string]any{"type": "object"},
		Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: true, IdempotentHint: true,
			DestructiveHint: boolTestPointer(false), OpenWorldHint: boolTestPointer(true)},
	}, func(context.Context, *mcpsdk.CallToolRequest) (*mcpsdk.CallToolResult, error) {
		return &mcpsdk.CallToolResult{IsError: true, Content: []mcpsdk.Content{&mcpsdk.TextContent{Text: `{"code":"invalid_cursor"}`}}}, nil
	})
	httpServer := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(httpServer.Close)
	_, _, service := newMCPParityService(t, false)
	created, err := service.Create(t.Context(), SetupInput{DisplayName: "Notes", TransportKind: "streamable_http", URL: httpServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || created.Server == nil {
		t.Fatalf("create = %#v, %v", created, err)
	}
	if _, err := service.SaveConnectionPolicy(t.Context(), created.Server.ID, created.Server.ConnectionRevision, 0, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	binding, err := service.Binding(t.Context(), "mcp."+created.Server.ID+".search")
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := service.Call(t.Context(), binding, json.RawMessage(`{"start_cursor":"page-999"}`)); !errors.Is(err, ErrCapabilityUnavailable) {
		t.Fatalf("tool error = %v, want capability unavailable", err)
	}
	current, err := service.Server(t.Context(), created.Server.ID)
	if err != nil {
		t.Fatal(err)
	}
	if current.HealthStatus != "healthy" || current.AuthStatus != "none" {
		t.Fatalf("tool error fenced connection = %#v", current)
	}
}

func TestConnectServiceUsesExplicitPathCardFallback(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "notes", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "search"},
		func(context.Context, *mcpsdk.CallToolRequest, struct{}) (*mcpsdk.CallToolResult, map[string]any, error) {
			return nil, map[string]any{"ok": true}, nil
		})
	var origin string
	mux := http.NewServeMux()
	mux.HandleFunc("/product/.well-known/mcp.json", func(w http.ResponseWriter, _ *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"title": "Notes", "transport": map[string]any{"type": "streamable-http", "endpoint": origin + "/product/mcp"}})
	})
	mux.Handle("/product/mcp", mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	httpServer := httptest.NewServer(mux)
	defer httpServer.Close()
	origin = httpServer.URL
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(paths.Database()), 0o700); err != nil {
		t.Fatal(err)
	}
	database, err := store.Open(t.Context(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	service, err := NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer service.Close()
	connected := service.ConnectService(t.Context(), origin+"/product?private=removed")
	if connected.Status != "ready_for_policy" || connected.CardURL != origin+"/product/.well-known/mcp.json" || connected.EndpointURL != origin+"/product/mcp" || connected.Setup.Server == nil {
		t.Fatalf("path-card setup = %#v", connected)
	}
}

func TestConnectServiceFindsPathMountedOAuthMetadata(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "notes", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "search"},
		func(context.Context, *mcpsdk.CallToolRequest, struct{}) (*mcpsdk.CallToolResult, map[string]any, error) {
			return nil, map[string]any{"ok": true}, nil
		})
	var origin string
	mcpHandler := mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil)
	mux := http.NewServeMux()
	mux.HandleFunc("/product/.well-known/mcp.json", func(w http.ResponseWriter, _ *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"title": "Notes", "transport": map[string]any{"type": "streamable-http", "endpoint": origin + "/product/mcp"}})
	})
	mux.HandleFunc("/product/.well-known/oauth-protected-resource", func(w http.ResponseWriter, _ *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"resource": origin + "/product/mcp", "authorization_servers": []string{origin}})
	})
	mux.Handle("/product/mcp", mcpHandler)
	httpServer := httptest.NewServer(mux)
	defer httpServer.Close()
	origin = httpServer.URL
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(paths.Database()), 0o700); err != nil {
		t.Fatal(err)
	}
	database, err := store.Open(t.Context(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	service, err := NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer service.Close()
	connected := service.ConnectService(t.Context(), origin+"/product")
	if connected.Status != "authentication_available" || connected.Setup.Server != nil || !connected.Setup.OAuthSupported {
		t.Fatalf("path-mounted OAuth setup = %#v", connected)
	}
}

func boolTestPointer(value bool) *bool { return &value }

func TestHTTPResponseWireLimit(t *testing.T) {
	httpServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		chunk := []byte(strings.Repeat("x", 8192))
		for remaining := maxWireBody + 1; remaining > 0; remaining -= len(chunk) {
			if remaining < len(chunk) {
				chunk = chunk[:remaining]
			}
			_, _ = w.Write(chunk)
		}
	}))
	defer httpServer.Close()
	_, err := Discover(t.Context(), Config{TransportKind: "streamable_http",
		SafeConfig: json.RawMessage(`{"url":"` + httpServer.URL + `"}`)})
	if !errors.Is(err, ErrMessageTooLarge) {
		t.Fatalf("oversized MCP response error = %v", err)
	}
}

func TestSetupHonorsAuthenticationPreferenceAndExactCallback(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "calendar", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "lookup"},
		func(context.Context, *mcpsdk.CallToolRequest, struct{}) (*mcpsdk.CallToolResult, map[string]any, error) {
			return nil, map[string]any{"ok": true}, nil
		})
	mcpHandler := mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil)
	var origin string
	httpServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if request.URL.Path == "/.well-known/oauth-protected-resource" {
			_ = json.NewEncoder(w).Encode(map[string]any{"resource": origin, "authorization_server": origin})
			return
		}
		mcpHandler.ServeHTTP(w, request)
	}))
	defer httpServer.Close()
	origin = httpServer.URL
	paths, _ := home.FromRoot(t.TempDir())
	if err := os.MkdirAll(filepath.Dir(paths.Database()), 0o700); err != nil {
		t.Fatal(err)
	}
	database, err := store.Open(t.Context(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	callback := "http://127.0.0.1:9321/mcp/oauth/callback"
	service, err := NewService(paths, database, false, nil, callback)
	if err != nil {
		t.Fatal(err)
	}
	defer service.Close()
	prompted, err := service.Create(t.Context(), SetupInput{DisplayName: "Calendar", TransportKind: "streamable_http",
		URL: httpServer.URL, AuthPreference: "PROMPT_IF_AVAILABLE"})
	if err != nil || prompted.Status != "authentication_available" || prompted.Server != nil {
		t.Fatalf("prompted setup = %#v, %v", prompted, err)
	}
	anonymous, err := service.Create(t.Context(), SetupInput{DisplayName: "Calendar", TransportKind: "streamable_http",
		URL: httpServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || anonymous.Server == nil {
		t.Fatalf("anonymous setup = %#v, %v", anonymous, err)
	}
	_, err = service.StartOAuthCreate(t.Context(), "human:local", SetupInput{DisplayName: "Calendar",
		TransportKind: "streamable_http", URL: httpServer.URL}, "http://127.0.0.1:9322/mcp/oauth/callback")
	if err == nil || !strings.Contains(err.Error(), "does not match") {
		t.Fatalf("mismatched callback error = %v", err)
	}
}

func TestToolDescriptionLimit(t *testing.T) {
	source := &mcpsdk.Tool{Name: "read", Description: strings.Repeat("x", 64<<10), InputSchema: map[string]any{"type": "object"}}
	tool, err := normalizeTool(source)
	if err != nil || tool.Description != source.Description {
		t.Fatalf("description at limit was not preserved: %v", err)
	}
	source.Description += "x"
	if _, err := normalizeTool(source); err == nil {
		t.Fatal("oversized tool description accepted")
	}
}
