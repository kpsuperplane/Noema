package mcp

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

func TestOAuthAttemptSurvivesStartingRequestCancellation(t *testing.T) {
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "oauth-fixture", Version: "1"}, nil)
	description := strings.Repeat("Documented tool behavior. ", 700)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "read", Description: description, InputSchema: map[string]any{"type": "object"}},
		func(context.Context, *mcpsdk.CallToolRequest, struct{}) (*mcpsdk.CallToolResult, map[string]any, error) {
			return nil, map[string]any{"ok": true}, nil
		})
	stream := mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil)
	var origin string
	var requests []string
	mux := http.NewServeMux()
	mux.HandleFunc("/mcp", func(w http.ResponseWriter, r *http.Request) {
		requests = append(requests, r.Method+" "+r.URL.Path)
		if r.Header.Get("Authorization") == "" {
			w.Header().Set("WWW-Authenticate", `Bearer resource_metadata="`+origin+`/.well-known/oauth-protected-resource", scope="read"`)
			w.WriteHeader(http.StatusUnauthorized)
			return
		}
		stream.ServeHTTP(w, r)
	})
	mux.HandleFunc("/.well-known/oauth-protected-resource", func(w http.ResponseWriter, _ *http.Request) {
		requests = append(requests, "GET /.well-known/oauth-protected-resource")
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(map[string]any{"resource": origin + "/mcp", "authorization_servers": []string{origin}, "scopes_supported": []string{"read"}})
	})
	mux.HandleFunc("/.well-known/oauth-authorization-server", func(w http.ResponseWriter, _ *http.Request) {
		requests = append(requests, "GET /.well-known/oauth-authorization-server")
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(map[string]any{
			"issuer": origin, "authorization_endpoint": origin + "/authorize", "token_endpoint": origin + "/token",
			"jwks_uri":              origin + "/jwks",
			"registration_endpoint": origin + "/register", "scopes_supported": []string{"read"},
			"response_types_supported": []string{"code"}, "code_challenge_methods_supported": []string{"S256"},
			"token_endpoint_auth_methods_supported": []string{"client_secret_post"},
		})
	})
	mux.HandleFunc("/register", func(w http.ResponseWriter, _ *http.Request) {
		requests = append(requests, "POST /register")
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(map[string]any{"client_id": "client", "client_secret": "secret", "token_endpoint_auth_method": "client_secret_post"})
	})
	mux.HandleFunc("/token", func(w http.ResponseWriter, r *http.Request) {
		requests = append(requests, "POST /token")
		if r.ParseForm() != nil || r.Form.Get("code") == "" || r.Form.Get("code_verifier") == "" {
			w.WriteHeader(http.StatusBadRequest)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(map[string]any{"access_token": "access", "refresh_token": "refresh", "token_type": "Bearer", "expires_in": 3600, "scope": "read"})
	})
	httpServer := httptest.NewServer(mux)
	t.Cleanup(httpServer.Close)
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
	t.Cleanup(func() { _ = database.Close() })
	service, err := NewService(paths, database, false, nil, "http://127.0.0.1:9321/mcp/oauth/callback")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)

	callerContext, cancelCaller := context.WithCancel(context.Background())
	attempt, err := service.StartOAuthCreate(callerContext, "human:local", SetupInput{
		DisplayName: "OAuth fixture", TransportKind: "streamable_http", URL: origin + "/mcp",
	}, "http://127.0.0.1:9321/mcp/oauth/callback")
	if err != nil || attempt.ID == "" || attempt.AuthorizationURL == "" {
		t.Fatalf("OAuth start = %#v, %v requests=%v", attempt, err, requests)
	}
	cancelCaller()
	time.Sleep(50 * time.Millisecond)
	view, err := service.Attempt(t.Context(), attempt.ID, "human:local")
	if err != nil || view.Status != "waiting_for_user" {
		t.Fatalf("attempt after starting request cancellation = %#v, %v", view, err)
	}

	authorizationURL, err := url.Parse(attempt.AuthorizationURL)
	if err != nil {
		t.Fatal(err)
	}
	query := authorizationURL.Query()
	callback := "http://127.0.0.1:9321/mcp/oauth/callback?attemptId=" + url.QueryEscape(attempt.ID) +
		"&code=test-code&state=" + url.QueryEscape(query.Get("state"))
	completionContext, cancelCompletion := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancelCompletion()
	if err := service.CompleteOAuth(completionContext, attempt.ID, callback); err != nil {
		t.Fatalf("OAuth completion = %v", err)
	}
	completed, err := service.Attempt(t.Context(), attempt.ID, "human:local")
	if err != nil || completed.Status != "completed" || completed.Result == nil || completed.Result.Server == nil {
		t.Fatalf("completed OAuth attempt = %#v, %v", completed, err)
	}
	tools, err := database.MCPTools(t.Context(), completed.Result.Server.ID)
	if err != nil || len(tools) != 1 || tools[0].Description != description {
		t.Fatalf("long tool description was not preserved: count=%d error=%v", len(tools), err)
	}
	scopes, err := service.GrantedScopes(t.Context(), completed.Result.Server.ID)
	if err != nil || len(scopes) != 1 || scopes[0] != "read" {
		t.Fatalf("public grant scopes = %v, %v", scopes, err)
	}

}
