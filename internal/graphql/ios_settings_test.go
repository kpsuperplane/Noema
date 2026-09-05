package graphql

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"regexp"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestIOSSettingsSnapshotResponse(t *testing.T) {
	source, err := os.ReadFile("../../apps/ios/Noema/Generated/Sources/Operations/Queries/SettingsSnapshotQuery.graphql.swift")
	if err != nil {
		t.Fatal(err)
	}
	query := regexp.MustCompile(`#"(query SettingsSnapshot .*?)"#`).FindSubmatch(source)
	if len(query) != 2 {
		t.Fatal("generated SettingsSnapshot query is missing")
	}
	resolver := openProviderTestResolver(t)
	paths, err := home.FromRoot(resolver.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	resolver.MCP, err = mcp.NewService(paths, resolver.Store, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(resolver.MCP.Close)
	resolver.Adapters, err = adapter.NewService(resolver.home, resolver.Store)
	if err != nil {
		t.Fatal(err)
	}
	ctx, now := context.Background(), time.Now()
	oldSession, newSession := sha256.Sum256([]byte("settings-old")), sha256.Sum256([]byte("settings-new"))
	if err := resolver.Store.CreateAnonymousSession(ctx, oldSession, now); err != nil {
		t.Fatal(err)
	}
	if err := resolver.Store.RegisterPasskey(ctx, store.HumanPasskey{CredentialID: "settings-passkey", CredentialJSON: `{}`},
		store.RegistrationInitial, oldSession, newSession, now); err != nil {
		t.Fatal(err)
	}
	config, recovery, err := auth.LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	resolver.Auth, err = auth.New(paths, resolver.Store, config, recovery)
	if err != nil {
		t.Fatal(err)
	}
	handler := resolver.Auth.Handler(NewHandler(resolver))
	access := seedNotificationNativeClient(t, resolver.Store)
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, nativeGraphQLRequest(t, access, string(query[1])))
	var payload graphQLResponse
	if err := json.Unmarshal(response.Body.Bytes(), &payload); err != nil {
		t.Fatal(err)
	}
	if response.Code != http.StatusOK || len(payload.Errors) != 0 || payload.Data == nil {
		t.Fatalf("native SettingsSnapshot = %d %s", response.Code, response.Body.String())
	}
	if _, err := resolver.requireMCP(ctx); err == nil {
		t.Fatal("MCP accepted an unauthenticated context")
	}
	if _, err := resolver.requireAdapters(ctx); err == nil {
		t.Fatal("adapters accepted an unauthenticated context")
	}
	response = httptest.NewRecorder()
	handler.ServeHTTP(response, nativeGraphQLRequest(t, "invalid", string(query[1])))
	if response.Code != http.StatusUnauthorized {
		t.Fatalf("invalid native credential status = %d", response.Code)
	}
}
