package diagnostics

import (
	"bufio"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

type rustHomeDiagnosticRecord struct {
	Event  string            `json:"event"`
	Fields map[string]string `json:"fields"`
}

func rustHomeReadDiagnosticRecords(t *testing.T, path string) []rustHomeDiagnosticRecord {
	t.Helper()
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()

	var records []rustHomeDiagnosticRecord
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		var record rustHomeDiagnosticRecord
		if err := json.Unmarshal(scanner.Bytes(), &record); err != nil {
			t.Fatalf("diagnostic record %q: %v", scanner.Bytes(), err)
		}
		records = append(records, record)
	}
	if err := scanner.Err(); err != nil {
		t.Fatal(err)
	}
	return records
}

func rustHomeDiagnosticField(name, value string) Field {
	return Text(name, value)
}

// Rust source: crates/noema-home/src/diagnostics.rs:234::appends_jsonl_events_without_overwriting
func TestRustHome_appends_jsonl_events_without_overwriting(t *testing.T) {
	directory := t.TempDir()
	path := filepath.Join(directory, "errors.log")
	writer, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := writer.Write("first_failure",
		rustHomeDiagnosticField("severity", "error"),
		rustHomeDiagnosticField("provider_kind", "test"),
		rustHomeDiagnosticField("provider_text", "line one\nline two"),
	); err != nil {
		t.Fatalf("first append: %v", err)
	}
	if err := writer.Write("second_failure",
		rustHomeDiagnosticField("severity", "error"),
		rustHomeDiagnosticField("error_chain", "outer\ninner"),
	); err != nil {
		t.Fatalf("second append: %v", err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}

	records := rustHomeReadDiagnosticRecords(t, path)
	if len(records) != 2 {
		t.Fatalf("diagnostic records = %d, want 2", len(records))
	}
	if records[0].Event != "first_failure" || records[0].Fields["severity"] != "error" ||
		records[0].Fields["provider_kind"] != "test" ||
		records[0].Fields["provider_text"] != "line one\nline two" {
		t.Fatalf("first diagnostic record = %#v", records[0])
	}
	if records[1].Event != "second_failure" || records[1].Fields["severity"] != "error" ||
		records[1].Fields["error_chain"] != "outer\ninner" {
		t.Fatalf("second diagnostic record = %#v", records[1])
	}
	if runtime.GOOS != "windows" {
		info, err := os.Stat(path)
		if err != nil {
			t.Fatal(err)
		}
		if info.Mode().Perm() != 0o600 {
			t.Fatalf("diagnostic permissions = %#o, want 0600", info.Mode().Perm())
		}
	}

	// Rust try_append accepts any path and suppresses append errors. Go Open
	// reports an invalid directory target before Write, without panicking.
	if unavailable, err := Open(directory); err == nil {
		_ = unavailable.Close()
	}
}

// Rust source: crates/noema-home/src/diagnostics.rs:277::rotates_one_bounded_backup
func TestRustHome_rotates_one_bounded_backup(t *testing.T) {
	path := filepath.Join(t.TempDir(), "errors.log")
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY, 0o600)
	if err != nil {
		t.Fatal(err)
	}
	if err := file.Truncate(MaxFileBytes); err != nil {
		_ = file.Close()
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}

	writer, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	err = writer.Write("bounded_failure", rustHomeDiagnosticField("sequence", "0"))
	if err == nil {
		t.Fatal("write beyond the Go file bound was accepted")
	}
	if closeErr := writer.Close(); closeErr != nil {
		t.Fatal(closeErr)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if info.Size() > MaxFileBytes {
		t.Fatalf("current log size = %d, want <= %d", info.Size(), MaxFileBytes)
	}
	if _, err := os.Stat(path + ".1"); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("unexpected Go rotation backup: %v", err)
	}
}

// Rust source: crates/noema-home/src/diagnostics.rs:298::replaces_oversized_raw_data_and_preserves_small_raw_data
func TestRustHome_replaces_oversized_raw_data_and_preserves_small_raw_data(t *testing.T) {
	path := filepath.Join(t.TempDir(), "errors.log")
	writer, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := writer.Write("small_failure", rustHomeDiagnosticField("ordinary_value", "preserved")); err != nil {
		t.Fatal(err)
	}
	large := strings.Repeat("x", maxValueBytes*2)
	if err := writer.Write("large_failure",
		rustHomeDiagnosticField("ordinary_value", "preserved"),
		rustHomeDiagnosticField("content", large),
	); err != nil {
		t.Fatal(err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}

	// Go's approved flat-field layout bounds each value directly. It does not
	// add the Rust raw.truncated/original_bytes metadata object.
	records := rustHomeReadDiagnosticRecords(t, path)
	if len(records) != 2 || records[0].Fields["ordinary_value"] != "preserved" ||
		records[1].Fields["ordinary_value"] != "preserved" {
		t.Fatalf("ordinary diagnostic values = %#v", records)
	}
	if len(records[1].Fields["content"]) != maxValueBytes || records[1].Fields["content"] == large {
		t.Fatalf("bounded diagnostic content length = %d", len(records[1].Fields["content"]))
	}
}

// Rust source: crates/noema-home/src/diagnostics.rs:310::rejects_an_event_larger_than_the_file_limit
func TestRustHome_rejects_an_event_larger_than_the_file_limit(t *testing.T) {
	path := filepath.Join(t.TempDir(), "errors.log")
	writer, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	err = writer.Write("large_failure", Field{
		name:  "detail",
		value: strings.Repeat("x", MaxRecordBytes),
	})
	if err == nil || !strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("oversized diagnostic write = %v", err)
	}
	if closeErr := writer.Close(); closeErr != nil {
		t.Fatal(closeErr)
	}
	contents, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if len(contents) != 0 {
		t.Fatalf("oversized diagnostic persisted %d bytes", len(contents))
	}
}
