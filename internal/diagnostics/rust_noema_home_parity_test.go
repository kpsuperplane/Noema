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

func rustHomeReadDiagnosticRecords(t *testing.T, path string) []map[string]any {
	t.Helper()
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()

	var records []map[string]any
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		record := make(map[string]any)
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

// Rust source: crates/noema-home/src/diagnostics.rs:234::appends_jsonl_events_without_overwriting
func TestRustHome_appends_jsonl_events_without_overwriting(t *testing.T) {
	directory := t.TempDir()
	path := filepath.Join(directory, "errors.log")
	writer, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := writer.WriteEvent(Event{Category: "first_failure", Message: "first failure", Context: map[string]any{"provider_kind": "test"}, Raw: map[string]any{"provider_text": "line one\nline two"}}); err != nil {
		t.Fatalf("first append: %v", err)
	}
	if err := writer.WriteEvent(Event{Category: "second_failure", Message: "second failure", ErrorChain: []string{"outer", "inner"}}); err != nil {
		t.Fatalf("second append: %v", err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}

	records := rustHomeReadDiagnosticRecords(t, path)
	if len(records) != 2 {
		t.Fatalf("diagnostic records = %d, want 2", len(records))
	}
	if records[0]["category"] != "first_failure" || records[0]["severity"] != "error" {
		t.Fatalf("first diagnostic identity = %#v", records[0])
	}
	context, ok := records[0]["context"].(map[string]any)
	if !ok || context["provider_kind"] != "test" {
		t.Fatalf("first diagnostic context = %#v", records[0]["context"])
	}
	raw, ok := records[0]["raw"].(map[string]any)
	if !ok || raw["provider_text"] != "line one\nline two" {
		t.Fatalf("first diagnostic raw = %#v", records[0]["raw"])
	}
	if records[1]["category"] != "second_failure" {
		t.Fatalf("second diagnostic category = %#v", records[1]["category"])
	}
	errorChain, ok := records[1]["error_chain"].([]any)
	if !ok || len(errorChain) != 2 || errorChain[0] != "outer" || errorChain[1] != "inner" {
		t.Fatalf("second diagnostic error chain = %#v", records[1]["error_chain"])
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
	writer, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	for sequence := 0; sequence < 20; sequence++ {
		if err := writer.writeWithLimits(Event{Category: "bounded_failure", Message: strings.Repeat("x", 80), Context: map[string]any{"sequence": sequence}}, 512, 256); err != nil {
			t.Fatalf("bounded append %d: %v", sequence, err)
		}
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if info.Size() > 512 {
		t.Fatalf("current log size = %d, want <= 512", info.Size())
	}
	backup, err := os.Stat(path + ".1")
	if err != nil {
		t.Fatalf("rotated log: %v", err)
	}
	if backup.Size() > 512 {
		t.Fatalf("backup log size = %d, want <= 512", backup.Size())
	}
}

// Rust source: crates/noema-home/src/diagnostics.rs:298::replaces_oversized_raw_data_and_preserves_small_raw_data
func TestRustHome_replaces_oversized_raw_data_and_preserves_small_raw_data(t *testing.T) {
	path := filepath.Join(t.TempDir(), "errors.log")
	writer, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := writer.WriteEvent(Event{Category: "small_failure", Message: "small", Raw: map[string]any{"ordinary_value": "preserved"}}); err != nil {
		t.Fatal(err)
	}
	large := strings.Repeat("x", 64*1024)
	if err := writer.WriteEvent(Event{Category: "large_failure", Message: "large", Raw: map[string]any{"content": large}}); err != nil {
		t.Fatal(err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}

	records := rustHomeReadDiagnosticRecords(t, path)
	if len(records) != 2 {
		t.Fatalf("diagnostic records = %d, want 2", len(records))
	}
	smallRaw, ok := records[0]["raw"].(map[string]any)
	if !ok || smallRaw["ordinary_value"] != "preserved" {
		t.Fatalf("small raw diagnostic = %#v", records[0]["raw"])
	}
	largeRaw, ok := records[1]["raw"].(map[string]any)
	if !ok || largeRaw["truncated"] != true {
		t.Fatalf("large raw diagnostic = %#v", records[1]["raw"])
	}
	originalBytes, ok := largeRaw["original_bytes"].(float64)
	if !ok || originalBytes <= 64*1024 {
		t.Fatalf("large raw original bytes = %#v", largeRaw["original_bytes"])
	}
}

// Rust source: crates/noema-home/src/diagnostics.rs:310::rejects_an_event_larger_than_the_file_limit
func TestRustHome_rejects_an_event_larger_than_the_file_limit(t *testing.T) {
	path := filepath.Join(t.TempDir(), "errors.log")
	writer, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	err = writer.writeWithLimits(Event{Category: "large_failure", Message: strings.Repeat("x", 512)}, 512, 256)
	if err == nil || !strings.Contains(err.Error(), "maximum is 256") {
		t.Errorf("oversized diagnostic error = %v, want EventTooLarge maximum 256", err)
	}
	if closeErr := writer.Close(); closeErr != nil {
		t.Fatal(closeErr)
	}
	if _, err := os.Stat(path); !errors.Is(err, os.ErrNotExist) {
		t.Errorf("oversized diagnostic path = %v, want absent", err)
	}
}
