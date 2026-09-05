package runtime

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"testing"
)

func TestFileDownloadArgumentsAndRootedDestination(t *testing.T) {
	request, err := parseFileDownloadArguments(json.RawMessage(`{"url":"https://example.net/report.pdf","path":"reports/a.pdf","parse":true,"max_chars":1000}`))
	if err != nil || request.Path != "reports/a.pdf" || !request.Parse || request.MaxChars != 1000 {
		t.Fatalf("valid request = %#v, %v", request, err)
	}
	request, err = parseFileDownloadArguments(json.RawMessage(`{"url":"https://alice@example.net/report.pdf","path":"report.pdf"}`))
	if err != nil || request.URL != "https://alice@example.net/report.pdf" {
		t.Fatalf("username URL = %#v, %v", request, err)
	}
	for _, raw := range []string{
		`{"url":"https://example.net/a","path":"../a"}`,
		`{"url":"https://example.net/a","path":"/absolute"}`,
		`{"url":"https://example.net/a","path":"a","unknown":true}`,
		`{"url":"https://example.net/a","path":"a","reason":" "}`,
		`{"url":"https://alice:secret@example.net/a","path":"a"}`,
		`{"url":"https://example.net/a","url":"https://alice:secret@example.net/a","path":"a"}`,
	} {
		parsed, parseErr := parseFileDownloadArguments(json.RawMessage(raw))
		if parseErr == nil {
			_, parseErr = normalizedFilePath(parsed.Path)
		}
		if parseErr == nil {
			t.Fatalf("invalid request accepted: %s", raw)
		}
	}

	rootPath := t.TempDir()
	extensionless := filepath.Join(rootPath, "download")
	if err := os.WriteFile(extensionless, []byte("plain response"), 0o600); err != nil {
		t.Fatal(err)
	}
	file, err := os.Open(extensionless)
	if err != nil {
		t.Fatal(err)
	}
	parsed := parseOpenFileWithMedia(t.Context(), file, "download", 1000, "text/plain; charset=utf-8")
	_ = file.Close()
	if parsed.Status != "converted" || parsed.Content == nil || *parsed.Content != "plain response" ||
		parsed.Format == nil || *parsed.Format != "txt" {
		t.Fatalf("extensionless parse = %#v", parsed)
	}

	root, err := os.OpenRoot(rootPath)
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(rootPath, "linked")); err != nil {
		t.Fatal(err)
	}
	if err := prepareDownloadParent(root, "linked/a.pdf"); err == nil {
		t.Fatal("symbolic-link parent was accepted")
	}
	if err := prepareDownloadParent(root, "new/nested/a.pdf"); err != nil {
		t.Fatal(err)
	}
	if info, err := os.Stat(filepath.Join(rootPath, "new", "nested")); err != nil || !info.IsDir() {
		t.Fatalf("rooted parent = %#v, %v", info, err)
	}
	for _, response := range []*http.Response{
		{StatusCode: http.StatusOK, Header: http.Header{"Content-Type": {"text/html; charset=utf-8"}}, Body: io.NopCloser(bytes.NewBufferString("<html>"))},
		{StatusCode: http.StatusOK, Header: http.Header{}, ContentLength: fileDownloadLimit + 1, Body: io.NopCloser(bytes.NewBufferString("large"))},
	} {
		if _, _, err := saveDownloadResponse(io.Discard, response); err == nil {
			t.Fatal("unsafe download response was accepted")
		}
	}
}
