package mcp

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
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
	database, err := store.Open(t.Context(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	service, err := NewService(paths, database, false)
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

func boolTestPointer(value bool) *bool { return &value }
