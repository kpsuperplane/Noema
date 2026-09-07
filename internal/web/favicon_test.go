package web

import (
	"bytes"
	"encoding/base64"
	"encoding/binary"
	"image"
	"image/color"
	"image/gif"
	"image/jpeg"
	"image/png"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/fyne-io/image/ico"
)

func TestFaviconRouteServesBoundedPNGWithPrivateETag(t *testing.T) {
	source := image.NewNRGBA(image.Rect(0, 0, 64, 16))
	for index := range source.Pix {
		source.Pix[index] = 0x80
	}
	var encoded bytes.Buffer
	if err := png.Encode(&encoded, source); err != nil {
		t.Fatal(err)
	}
	body, err := normalizeFaviconImage(encoded.Bytes())
	if err != nil {
		t.Fatal(err)
	}
	handler := NewFaviconHandler(t.TempDir())
	handler.write("example.com", body)
	mux := http.NewServeMux()
	mux.Handle("GET /favicons/{hostname}", handler)

	response := httptest.NewRecorder()
	mux.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/favicons/example.com", nil))
	if response.Code != http.StatusOK || response.Header().Get("Content-Type") != "image/png" ||
		response.Header().Get("Cache-Control") != "private, max-age=86400" {
		t.Fatalf("favicon response = %d %#v", response.Code, response.Header())
	}
	config, format, err := image.DecodeConfig(bytes.NewReader(response.Body.Bytes()))
	if err != nil || format != "png" || config.Width != 32 || config.Height != 32 || response.Body.Len() > faviconSourceLimit {
		t.Fatalf("favicon image = %#v %q %d bytes, %v", config, format, response.Body.Len(), err)
	}
	etag := response.Header().Get("ETag")
	if len(etag) != 66 || !strings.HasPrefix(etag, `"`) || !strings.HasSuffix(etag, `"`) {
		t.Fatalf("favicon ETag = %q", etag)
	}
	request := httptest.NewRequest(http.MethodGet, "/favicons/example.com", nil)
	request.Header.Set("If-None-Match", etag)
	response = httptest.NewRecorder()
	mux.ServeHTTP(response, request)
	if response.Code != http.StatusNotModified || response.Body.Len() != 0 {
		t.Fatalf("conditional favicon = %d, %d bytes", response.Code, response.Body.Len())
	}
	response = httptest.NewRecorder()
	mux.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/favicons/127.0.0.1", nil))
	if response.Code != http.StatusBadRequest {
		t.Fatalf("IP favicon status = %d", response.Code)
	}
}

func TestFaviconNormalizationSupportsCurrentFormatsAndBounds(t *testing.T) {
	source := image.NewNRGBA(image.Rect(0, 0, 8, 4))
	for y := range 4 {
		for x := range 8 {
			source.Set(x, y, color.NRGBA{R: 80, G: 120, B: 200, A: 255})
		}
	}
	encoders := map[string]func(*bytes.Buffer) error{
		"gif":  func(buffer *bytes.Buffer) error { return gif.Encode(buffer, source, nil) },
		"ico":  func(buffer *bytes.Buffer) error { return ico.Encode(buffer, source) },
		"jpeg": func(buffer *bytes.Buffer) error { return jpeg.Encode(buffer, source, nil) },
		"png":  func(buffer *bytes.Buffer) error { return png.Encode(buffer, source) },
	}
	for format, encode := range encoders {
		var input bytes.Buffer
		if err := encode(&input); err != nil {
			t.Fatal(err)
		}
		if output, err := normalizeFaviconImage(input.Bytes()); err != nil || len(output) == 0 {
			t.Fatalf("normalize %s = %d bytes, %v", format, len(output), err)
		}
	}
	webp, _ := base64.StdEncoding.DecodeString("UklGRrIBAABXRUJQVlA4TKUBAAAvSsAYAA8w//M///MfeJAkbXvaSG7m8Q3GfYSBJekwQztm/IcZlgwnmWImn2BK7aFmBtnVir6q//8VOkFE/xm4baTIu8c48ArEo6+B3zFKYln3pqClSCKX0begFTAXFOLXHSyF8cCNcZEG4OywuA4KVVfJCiArU7GAgJI8+lJP/OKMT/fBAjevg1cYB7YVkFuWga2lyPi5I0HFy5YTpWIHg0RZpkniRVW9odHAKOwosWuOGdxIyn2OvaCDvhg/we6TwadPBPbqBV58MsLmMJ8yZnOWk8SRz4N+QoyPL+MnamzMvcE1rHNEr91F9GKZPVUcS9w7PhhH36suB9qPeYb/oLk6cuTiJ0wOK3m5h1cKjW6EVZCYMK7dxcKCBdgP9HkKr9gkAO2P8GKZGWVdIAatQa+1IDpt6qyorVwdy01xdW8Jkfk6xjEXmVQQ+HQdFr6OKhIN34dXWq0+0qr6EJSCeeVLH9+gvGTLyqM65PQ44ihzlTXxQKjKbAvshXgir7Lil9w4L2bvMycmjQcqXaMCO6BlY28i+FOLzbfI1vEqxAhotocAAA==")
	if output, err := normalizeFaviconImage(webp); err != nil || len(output) == 0 {
		t.Fatalf("normalize webp = %d bytes, %v", len(output), err)
	}
	malformedICO := make([]byte, 22)
	copy(malformedICO, []byte{0, 0, 1, 0, 1, 0})
	binary.LittleEndian.PutUint32(malformedICO[14:18], ^uint32(0))
	binary.LittleEndian.PutUint32(malformedICO[18:22], 22)
	if _, err := normalizeFaviconImage(malformedICO); err == nil {
		t.Fatal("oversized ICO entry was accepted")
	}
	var oversized bytes.Buffer
	if err := png.Encode(&oversized, image.NewNRGBA(image.Rect(0, 0, 1025, 1))); err != nil {
		t.Fatal(err)
	}
	if _, err := normalizeFaviconImage(oversized.Bytes()); err == nil {
		t.Fatal("oversized image dimensions were accepted")
	}
	base := httptest.NewRequest(http.MethodGet, "https://example.com/page", nil).URL
	if icon := declaredFavicon([]byte(`<link rel="apple-touch-icon" href="apple.png"><link rel="icon" sizes="any" href="site.svg"><link rel="icon" sizes="32x32" href="site.png">`), base); icon.String() != "https://example.com/site.png" {
		t.Fatalf("declared favicon = %v", icon)
	}
}
