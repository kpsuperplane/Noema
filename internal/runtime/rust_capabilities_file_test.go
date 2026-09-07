package runtime

import (
	"encoding/json"
	"testing"
)

// Rust source: crates/noema-capabilities/src/file.rs::parse_contract_bounds_input.
func TestRustCapabilities_parse_contract_bounds_input(t *testing.T) {
	request, err := parseFileArguments(json.RawMessage(`{"path":" data.csv ","max_chars":1000}`))
	if err != nil {
		t.Fatalf("parse arguments: %v", err)
	}
	if request.path != "data.csv" {
		t.Errorf("path = %q", request.path)
	}
	if request.maxChars != 1000 {
		t.Errorf("max chars = %d", request.maxChars)
	}
	if _, err := parseFileArguments(json.RawMessage(`{"path":"data.csv","extra":true}`)); err == nil {
		t.Error("extra file.parse field was accepted")
	}

	download, err := parseFileDownloadArguments(json.RawMessage(`{"url":"https://example.com/data.csv","path":"data.csv","parse":true}`))
	if err != nil {
		t.Fatalf("parse download arguments: %v", err)
	}
	if !download.Parse {
		t.Error("download parse flag = false")
	}
	if download.MaxChars != fileParseMaximumCharacters {
		t.Errorf("download max chars = %d, want %d", download.MaxChars, fileParseMaximumCharacters)
	}
}
