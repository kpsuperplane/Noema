package web

import (
	"bytes"
	"image"
	"image/png"
	"net/http"
	"testing"
	"testing/fstest"
)

// Rust source: crates/noema-server/src/web/assets.rs::assets_preserve_safe_paths_content_types_dynamic_chunks_and_spa_boundaries.
func TestRustServer_assets_preserve_safe_paths_content_types_dynamic_chunks_and_spa_boundaries(t *testing.T) {
	handler := assetHandler{assets: fstest.MapFS{
		"__noema_asset_test_chunk.js": {Data: []byte("export {};")},
	}}
	response := requestAsset(handler, http.MethodGet, "/assets/__noema_asset_test_chunk.js")
	if response.Code != http.StatusOK {
		t.Fatalf("dynamic chunk status = %d", response.Code)
	}
	if got := response.Header().Get("Content-Type"); got != "application/javascript; charset=utf-8" {
		t.Fatalf("dynamic chunk content type = %q", got)
	}
	if got := response.Header().Get("Cache-Control"); got != "public, max-age=31536000, immutable" {
		t.Fatalf("dynamic chunk cache control = %q", got)
	}
	if got := response.Header().Get("Service-Worker-Allowed"); got != "" {
		t.Fatalf("dynamic chunk service worker header = %q", got)
	}
	if !bytes.Equal(response.Body.Bytes(), []byte("export {};")) {
		t.Fatalf("dynamic chunk body = %q", response.Body.Bytes())
	}

	for path, want := range map[string]string{
		"/assets/app.js":         "app.js",
		"/assets/route-chunk.js": "route-chunk.js",
		"/assets/styles.css":     "styles.css",
		"/favicon.ico":           "favicon.ico",
	} {
		if got, ok := staticAssetName(path); !ok || got != want {
			t.Fatalf("staticAssetName(%q) = %q, %v", path, got, ok)
		}
	}
	for _, path := range []string{"/assets/nested/chunk.js", "/assets/../chunk.js", "/assets"} {
		if _, ok := staticAssetName(path); ok {
			t.Fatalf("staticAssetName(%q) accepted unsafe path", path)
		}
	}

	for name, want := range map[string]string{
		"route-chunk.js":       "application/javascript; charset=utf-8",
		"styles.css":           "text/css; charset=utf-8",
		"icon.svg":             "image/svg+xml; charset=utf-8",
		"manifest.webmanifest": "application/manifest+json; charset=utf-8",
		"icon.png":             "image/png",
		"favicon.ico":          "image/x-icon",
		"noema-font.ttf":       "font/ttf",
	} {
		if got, ok := assetContentType(name); !ok || got != want {
			t.Fatalf("assetContentType(%q) = %q, %v", name, got, ok)
		}
	}
	if _, ok := assetContentType("data.bin"); ok {
		t.Fatal("unknown asset content type was accepted")
	}
	for name, want := range map[string]string{
		"app-a1B2c3D4.js":         "public, max-age=31536000, immutable",
		"sw.js":                   "no-cache",
		"noema-font-a1B2c3D4.ttf": "public, max-age=31536000, immutable",
		"index.html":              "no-cache",
	} {
		if got := cacheControl(name); got != want {
			t.Fatalf("cacheControl(%q) = %q, want %q", name, got, want)
		}
	}
	if _, _, ok := assetRequest("/assets/data.bin"); ok {
		t.Fatal("unknown embedded asset was accepted")
	}
	if !IsSPAPath("/memory") {
		t.Fatal("memory route was not classified as SPA")
	}
	if IsSPAPath("/graphql/schema.graphql") {
		t.Fatal("GraphQL schema was classified as SPA")
	}
}

// Rust source: crates/noema-server/src/web/favicons.rs::hostname_normalization_keeps_exact_hosts_distinct.
func TestRustServer_hostname_normalization_keeps_exact_hosts_distinct(t *testing.T) {
	if got, err := normalizeFaviconHostname("EXAMPLE.com."); err != nil || got != "example.com" {
		t.Fatalf("normalized hostname = %q, %v", got, err)
	}
	if got, err := normalizeFaviconHostname("www.example.com"); err != nil || got != "www.example.com" {
		t.Fatalf("normalized www hostname = %q, %v", got, err)
	}
	handler := NewFaviconHandler().(*faviconHandler)
	handler.write("example.com", []byte("root"))
	handler.write("www.example.com", []byte("www"))
	if bytes.Equal(handler.read("example.com"), handler.read("www.example.com")) {
		t.Fatal("distinct favicon hosts shared a cache entry")
	}
}

// Rust source: crates/noema-server/src/web/favicons.rs::hostname_normalization_rejects_ip_and_authority_values.
func TestRustServer_hostname_normalization_rejects_ip_and_authority_values(t *testing.T) {
	for _, hostname := range []string{"127.0.0.1", "[::1]", "example.com:443", "user@example.com", ""} {
		if _, err := normalizeFaviconHostname(hostname); err == nil {
			t.Fatalf("normalizeFaviconHostname(%q) accepted invalid hostname", hostname)
		}
	}
}

// Rust source: crates/noema-server/src/web/favicons.rs::image_normalization_bounds_and_converts_raster_input.
func TestRustServer_image_normalization_bounds_and_converts_raster_input(t *testing.T) {
	input := image.NewRGBA(image.Rect(0, 0, 48, 24))
	var encoded bytes.Buffer
	if err := png.Encode(&encoded, input); err != nil {
		t.Fatal(err)
	}
	normalized, err := normalizeFaviconImage(encoded.Bytes())
	if err != nil {
		t.Fatal(err)
	}
	config, format, err := image.DecodeConfig(bytes.NewReader(normalized))
	if err != nil || format != "png" || config.Width != faviconSize || config.Height != faviconSize {
		t.Fatalf("normalized image = %#v, %q, %v", config, format, err)
	}
	if _, err := normalizeFaviconImage([]byte("<svg/>")); err != faviconMissing {
		t.Fatalf("SVG normalization error = %v", err)
	}
	var oversized bytes.Buffer
	if err := png.Encode(&oversized, image.NewRGBA(image.Rect(0, 0, 1025, 1))); err != nil {
		t.Fatal(err)
	}
	if _, err := normalizeFaviconImage(oversized.Bytes()); err != faviconMissing {
		t.Fatalf("oversized normalization error = %v", err)
	}
}

// Rust source: crates/noema-server/src/web/favicons.rs::cache_preserves_positive_and_negative_outcomes.
func TestRustServer_cache_preserves_positive_and_negative_outcomes(t *testing.T) {
	handler := NewFaviconHandler().(*faviconHandler)
	handler.write("example.com", []byte("png"))
	if got := handler.read("example.com"); !bytes.Equal(got, []byte("png")) {
		t.Fatalf("positive cache = %q", got)
	}
	t.Fatalf("unsupported port: Go favicon handler has no durable missing, transient, or stale cache outcomes")
}

// Rust source: crates/noema-server/src/web/favicons.rs::concurrent_requests_share_one_hostname_fetch_lock.
func TestRustServer_concurrent_requests_share_one_hostname_fetch_lock(t *testing.T) {
	t.Fatalf("unsupported port: Go favicon handler has no per-host fetch lock")
}

// Rust source: crates/noema-server/src/web/favicons.rs::cache_write_evicts_the_oldest_hostname.
func TestRustServer_cache_write_evicts_the_oldest_hostname(t *testing.T) {
	t.Fatalf("unsupported port: Go favicon cache has no insertion timestamps and evicts an arbitrary key")
}
