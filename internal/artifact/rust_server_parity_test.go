package artifact

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

// Rust source: crates/noema-server/src/web/router/tests.rs::artifact_preview_adapter_allows_only_inert_browser_formats.
func TestRustServer_artifact_preview_adapter_allows_only_inert_browser_formats(t *testing.T) {
	response := httptest.NewRecorder()
	response.Header().Set("Content-Type", "image/png")
	response.Header().Set("Content-Disposition", contentDisposition("inline", "label.png"))
	response.Header().Set("Content-Security-Policy", "default-src 'none'; frame-ancestors 'self'; base-uri 'none'; form-action 'none'")
	response.WriteHeader(http.StatusOK)
	if response.Code != http.StatusOK || response.Header().Get("Content-Type") != "image/png" ||
		response.Header().Get("Content-Disposition") != `inline; filename="label.png"` ||
		response.Header().Get("Content-Security-Policy") != "default-src 'none'; frame-ancestors 'self'; base-uri 'none'; form-action 'none'" {
		t.Fatalf("PNG preview response = %d %#v", response.Code, response.Header())
	}
	for _, mediaType := range []string{"image/svg+xml", "text/html"} {
		if inlineMediaType(mediaType) {
			t.Fatalf("unsafe preview media type accepted: %s", mediaType)
		}
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::artifact_download_adapter_sanitizes_response_headers.
func TestRustServer_artifact_download_adapter_sanitizes_response_headers(t *testing.T) {
	mediaType := "text/markdown\r\nx-injected: yes"
	filename := "report\"\r\nx-injected: yes.md"
	response := httptest.NewRecorder()
	response.Header().Set("Content-Type", safeMediaType(mediaType))
	response.Header().Set("Content-Disposition", contentDisposition("attachment", filename))
	response.WriteHeader(http.StatusOK)
	if response.Code != http.StatusOK || response.Header().Get("Content-Type") != "application/octet-stream" {
		t.Fatalf("download content type = %q", response.Header().Get("Content-Type"))
	}
	if response.Header().Get("Content-Disposition") != `attachment; filename="report\"__x-injected: yes.md"` {
		t.Fatalf("download disposition = %q", response.Header().Get("Content-Disposition"))
	}
	if response.Header().Get("X-Injected") != "" {
		t.Fatal("response injected a header")
	}
}
