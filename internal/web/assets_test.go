package web

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"testing/fstest"
)

func TestAssetResponsesUseRouteSpecificHeaders(t *testing.T) {
	handler := testAssetHandler()
	for _, test := range []struct {
		path          string
		body          string
		contentType   string
		cacheControl  string
		workerAllowed string
	}{
		{"/assets/app-a1B2.js", "application", "application/javascript; charset=utf-8", "public, max-age=31536000, immutable", ""},
		{"/assets/styles-a1B2.css", "styles", "text/css; charset=utf-8", "public, max-age=31536000, immutable", ""},
		{"/assets/font-a1B2.woff", "woff", "font/woff", "public, max-age=31536000, immutable", ""},
		{"/assets/font-a1B2.woff2", "woff2", "font/woff2", "public, max-age=31536000, immutable", ""},
		{"/assets/manifest.webmanifest", "manifest", "application/manifest+json; charset=utf-8", "no-cache", ""},
		{"/assets/sw.js", "worker", "application/javascript; charset=utf-8", "no-cache", "/"},
		{"/favicon.ico", "icon", "image/x-icon", "no-cache", ""},
	} {
		t.Run(test.path, func(t *testing.T) {
			response := requestAsset(handler, http.MethodGet, test.path)
			if response.Code != http.StatusOK || response.Body.String() != test.body {
				t.Fatalf("response = %d %q", response.Code, response.Body.String())
			}
			if got := response.Header().Get("Content-Type"); got != test.contentType {
				t.Fatalf("Content-Type = %q, want %q", got, test.contentType)
			}
			if got := response.Header().Get("Cache-Control"); got != test.cacheControl {
				t.Fatalf("Cache-Control = %q, want %q", got, test.cacheControl)
			}
			if got := response.Header().Get("Service-Worker-Allowed"); got != test.workerAllowed {
				t.Fatalf("Service-Worker-Allowed = %q, want %q", got, test.workerAllowed)
			}
		})
	}
}

func TestGraphQLSupportServesOnlyEnabledProtectedResources(t *testing.T) {
	directory := t.TempDir()
	if err := os.WriteFile(filepath.Join(directory, "graphiql.html"), []byte("local GraphiQL"), 0o600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("NOEMA_DEV_ASSET_DIR", directory)
	var graphQLCalls int
	graphql := http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		graphQLCalls++
		w.WriteHeader(http.StatusNoContent)
	})
	disabled := NewGraphQLHandler(graphql, []byte("type Query { boot: String! }"), false)
	if response := requestAsset(disabled, http.MethodGet, "/graphql"); response.Code != http.StatusNotFound {
		t.Fatalf("disabled GraphiQL status = %d", response.Code)
	}
	handler := NewGraphQLHandler(graphql, []byte("type Query { boot: String! }"), true)
	response := requestAsset(handler, http.MethodGet, "/graphql")
	if response.Code != http.StatusOK || response.Body.String() != "local GraphiQL" ||
		!strings.Contains(response.Header().Get("Content-Security-Policy"), "script-src 'self'") {
		t.Fatalf("GraphiQL response = %d %q %#v", response.Code, response.Body.String(), response.Header())
	}
	schema := requestAsset(handler, http.MethodGet, "/graphql/schema.graphql")
	if schema.Code != http.StatusOK || schema.Body.String() != "type Query { boot: String! }" ||
		schema.Header().Get("Cache-Control") != "no-store" {
		t.Fatalf("schema response = %d %q %#v", schema.Code, schema.Body.String(), schema.Header())
	}
	if response := requestAsset(handler, http.MethodPost, "/graphql"); response.Code != http.StatusNoContent || graphQLCalls != 1 {
		t.Fatalf("GraphQL pass-through = %d with %d calls", response.Code, graphQLCalls)
	}
}

func TestSPAFallbackPreservesApplicationAndServerRouteBoundary(t *testing.T) {
	handler := testAssetHandler()
	for _, path := range []string{"/", "/memory", "/memory/thread", "/tasks/123?panel=activity"} {
		response := requestAsset(handler, http.MethodGet, path)
		if response.Code != http.StatusOK || response.Body.String() != "application shell" {
			t.Errorf("%s response = %d %q", path, response.Code, response.Body.String())
		}
	}
	for _, path := range []string{
		"/assets", "/api/value", "/graphql", "/graphql/schema.graphql", "/auth/status",
		"/oauth/authorize", "/__noema/health", "/mcp/oauth/callback",
		"/provider/oauth/callback/attempt", "/adapter/oauth/callback",
		"/artifacts/versions/one/download", "/memory/export.json",
	} {
		response := requestAsset(handler, http.MethodGet, path)
		if response.Code != http.StatusNotFound {
			t.Errorf("%s status = %d, want 404", path, response.Code)
		}
	}
}

func TestAssetPathsStayFlatAndKnown(t *testing.T) {
	handler := testAssetHandler()
	for _, path := range []string{
		"/assets/graphiql.html", "/assets/nested/app.js", "/assets/../app.js",
		`/assets/nested\app.js`, "/assets/data.bin", "/assets/missing.js",
	} {
		response := requestAsset(handler, http.MethodGet, path)
		if response.Code != http.StatusNotFound {
			t.Errorf("%s status = %d, want 404", path, response.Code)
		}
	}
}

func TestAssetFallbackAcceptsGETOnly(t *testing.T) {
	handler := testAssetHandler()
	for _, method := range []string{http.MethodHead, http.MethodPost, http.MethodPut} {
		response := requestAsset(handler, method, "/assets/app-a1B2.js")
		if response.Code != http.StatusNotFound {
			t.Errorf("%s status = %d, want 404", method, response.Code)
		}
	}
}

func testAssetHandler() http.Handler {
	return assetHandler{assets: fstest.MapFS{
		"index.html":           &fstest.MapFile{Data: []byte("application shell")},
		"app-a1B2.js":          &fstest.MapFile{Data: []byte("application")},
		"styles-a1B2.css":      &fstest.MapFile{Data: []byte("styles")},
		"font-a1B2.woff":       &fstest.MapFile{Data: []byte("woff")},
		"font-a1B2.woff2":      &fstest.MapFile{Data: []byte("woff2")},
		"manifest.webmanifest": &fstest.MapFile{Data: []byte("manifest")},
		"sw.js":                &fstest.MapFile{Data: []byte("worker")},
		"favicon.ico":          &fstest.MapFile{Data: []byte("icon")},
	}}
}

func requestAsset(handler http.Handler, method string, target string) *httptest.ResponseRecorder {
	request := httptest.NewRequest(method, target, nil)
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)
	return response
}
