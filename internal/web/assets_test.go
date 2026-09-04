package web

import (
	"net/http"
	"net/http/httptest"
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
