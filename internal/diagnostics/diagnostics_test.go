package diagnostics_test

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/diagnostics"
)

func TestWriterPreservesOrdinaryTechnicalValues(t *testing.T) {
	path := filepath.Join(t.TempDir(), "errors.log")
	writer, err := diagnostics.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	err = writer.Write("provider.request_failed",
		diagnostics.Text("task_id", "tsk_01JZY8EXACTopaque"),
		diagnostics.Text("model", "anthropic/claude-sonnet-4.5"),
		diagnostics.Text("url", "https://example.test/v1?authorization_status=ready&port=8080"))
	if err != nil {
		t.Fatal(err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}
	contents, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var got struct {
		Event  string            `json:"event"`
		Fields map[string]string `json:"fields"`
	}
	if bytes.Count(contents, []byte{'\n'}) != 1 || json.Unmarshal(contents, &got) != nil ||
		got.Event != "provider.request_failed" || got.Fields["task_id"] != "tsk_01JZY8EXACTopaque" ||
		got.Fields["model"] != "anthropic/claude-sonnet-4.5" || !strings.Contains(got.Fields["url"], "authorization_status=ready") {
		t.Fatalf("record = %q", contents)
	}
	if runtime.GOOS != "windows" {
		info, err := os.Stat(path)
		if err != nil || info.Mode().Perm() != 0o600 {
			t.Fatalf("mode = %v, %v", info, err)
		}
	}
}

func TestWriterBoundsCompleteRecords(t *testing.T) {
	if _, err := diagnostics.Open(filepath.Join(t.TempDir(), "missing", "errors.log")); err == nil {
		t.Fatal("Open succeeded without its parent directory")
	}
	var unavailable *diagnostics.Writer
	if err := unavailable.Write("runtime.failed"); err != nil {
		t.Fatalf("unavailable writer was not best-effort: %v", err)
	}
	path := filepath.Join(t.TempDir(), "errors.log")
	writer, err := diagnostics.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := writer.Write("runtime.failed", diagnostics.Text("detail", strings.Repeat("é", 4096))); err != nil {
		t.Fatal(err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}
	contents, err := os.ReadFile(path)
	if err != nil || len(contents) > diagnostics.MaxRecordBytes || contents[len(contents)-1] != '\n' || !json.Valid(contents) {
		t.Fatalf("record size = %d, error = %v", len(contents), err)
	}
}
