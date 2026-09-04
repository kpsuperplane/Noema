package diagnostics_test

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/diagnostics"
)

func TestWriterPreservesOrdinaryTechnicalValues(t *testing.T) {
	t.Parallel()

	want := map[string]string{
		"task_id":   "tsk_01JZY8EXACTopaque",
		"path":      `C:\Users\sam\Noema\model.gguf`,
		"url":       "https://example.test/v1?authorization_status=ready&port=8080",
		"model":     "anthropic/claude-sonnet-4.5",
		"client_id": "client_public_0189Z",
	}
	values := map[string]diagnostics.Value{
		"task_id":   diagnostics.Identifier(want["task_id"]),
		"path":      diagnostics.Path(want["path"]),
		"url":       diagnostics.URL(want["url"]),
		"model":     diagnostics.ModelName(want["model"]),
		"client_id": diagnostics.ClientID(want["client_id"]),
	}
	fields := make([]diagnostics.Field, 0, len(values))
	for name, value := range values {
		field, err := diagnostics.NewField(name, value)
		if err != nil {
			t.Fatalf("NewField(%q): %v", name, err)
		}
		fields = append(fields, field)
	}

	record, err := diagnostics.NewRecord(
		time.Date(2026, time.September, 4, 12, 30, 0, 123, time.FixedZone("test", 3600)),
		"provider.request.finished",
		fields...,
	)
	if err != nil {
		t.Fatalf("NewRecord: %v", err)
	}

	path := filepath.Join(t.TempDir(), "diagnostics.jsonl")
	writer, err := diagnostics.Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	if err := writer.Write(record); err != nil {
		t.Fatalf("Write: %v", err)
	}
	if err := writer.Close(); err != nil {
		t.Fatalf("Close: %v", err)
	}

	contents, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	if bytes.Count(contents, []byte{'\n'}) != 1 || contents[len(contents)-1] != '\n' {
		t.Fatalf("result is not one complete JSONL record: %q", contents)
	}

	var got struct {
		Time   string            `json:"time"`
		Event  string            `json:"event"`
		Fields map[string]string `json:"fields"`
	}
	if err := json.Unmarshal(contents, &got); err != nil {
		t.Fatalf("decode result: %v", err)
	}
	if got.Time != "2026-09-04T11:30:00.000000123Z" {
		t.Fatalf("time = %q", got.Time)
	}
	if got.Event != "provider.request.finished" {
		t.Fatalf("event = %q", got.Event)
	}
	for name, value := range want {
		if got.Fields[name] != value {
			t.Errorf("fields[%q] = %q, want %q", name, got.Fields[name], value)
		}
	}
}

func TestOversizedRecordDoesNotWritePartialLine(t *testing.T) {
	t.Parallel()

	first, err := diagnostics.NewField("source_path", diagnostics.Path(strings.Repeat("a", 2500)))
	if err != nil {
		t.Fatalf("NewField: %v", err)
	}
	second, err := diagnostics.NewField("target_path", diagnostics.Path(strings.Repeat("b", 2500)))
	if err != nil {
		t.Fatalf("NewField: %v", err)
	}
	record, err := diagnostics.NewRecord(time.Now(), "filesystem.open", first, second)
	if err != nil {
		t.Fatalf("NewRecord: %v", err)
	}

	path := filepath.Join(t.TempDir(), "diagnostics.jsonl")
	writer, err := diagnostics.Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	err = writer.Write(record)
	if err == nil || !strings.Contains(err.Error(), "record exceeds") {
		t.Fatalf("Write error = %v", err)
	}
	if err := writer.Close(); err != nil {
		t.Fatalf("Close: %v", err)
	}

	contents, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	if len(contents) != 0 {
		t.Fatalf("file contains %d bytes after rejected write", len(contents))
	}

	if err := os.Truncate(path, diagnostics.MaxFileBytes); err != nil {
		t.Fatalf("fill diagnostic file: %v", err)
	}
	writer, err = diagnostics.Open(path)
	if err != nil {
		t.Fatalf("reopen full file: %v", err)
	}
	defer writer.Close()
	small, err := diagnostics.NewRecord(time.Now(), "server.started")
	if err != nil {
		t.Fatalf("create small record: %v", err)
	}
	if err := writer.Write(small); err == nil || !strings.Contains(err.Error(), "file exceeds") {
		t.Fatalf("full file error = %v", err)
	}
}

func TestOpenCreatesPrivateJSONLFile(t *testing.T) {
	t.Parallel()

	path := filepath.Join(t.TempDir(), "diagnostics.jsonl")
	writer, err := diagnostics.Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	if err := writer.Close(); err != nil {
		t.Fatalf("Close: %v", err)
	}

	if runtime.GOOS == "windows" {
		return
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("Stat: %v", err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("permissions = %#o, want 0600", got)
	}
}

func TestRecordCannotBypassBoundedWriter(t *testing.T) {
	t.Parallel()

	record, err := diagnostics.NewRecord(time.Now(), "server.started")
	if err != nil {
		t.Fatalf("NewRecord: %v", err)
	}
	_, err = json.Marshal(record)
	if err == nil || !strings.Contains(err.Error(), "bounded writer") {
		t.Fatalf("Marshal error = %v", err)
	}

	_, err = json.Marshal(diagnostics.Identifier("ordinary-id"))
	if err == nil || !strings.Contains(err.Error(), "bounded writer") {
		t.Fatalf("value serialization error = %v", err)
	}

	type credential struct{ secret string }
	if _, accepted := any(credential{secret: "credential-sentinel"}).(diagnostics.Value); accepted {
		t.Fatal("credential type satisfies diagnostics.Value")
	}
}
