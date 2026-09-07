package webtool

import (
	"context"
	"encoding/json"
	"reflect"
	"strings"
	"testing"
)

// Rust source: crates/noema-capabilities/src/web/browse.rs::parsers_enforce_operation_specific_arguments.
func TestRustCapabilities_parsers_enforce_operation_specific_arguments(t *testing.T) {
	if _, _, _, err := parseBrowserArguments(context.Background(), BrowseOpenName, json.RawMessage(`{"url":" https://1.1.1.1 "}`)); err != nil {
		t.Errorf("open arguments: %v", err)
	}
	assertBrowserError := func(raw string, want string) {
		t.Helper()
		_, _, _, err := parseBrowserArguments(context.Background(), BrowseInteractName, json.RawMessage(raw))
		if err == nil {
			t.Errorf("interaction %s was accepted", raw)
			return
		}
		if err.Error() != want {
			t.Errorf("interaction %s error = %q, want %q", raw, err, want)
		}
	}
	assertBrowserError(`{"snapshot_revision":1,"ref":"e1","action":"fill"}`, "value is required for this interaction")
	assertBrowserError(`{"snapshot_revision":1,"ref":"e1","action":"upload_file"}`, "artifact_id and artifact_version_id are required for file upload")
	values, _, _, err := parseBrowserArguments(context.Background(), BrowseInteractName, json.RawMessage(`{"snapshot_revision":1,"ref":"e1","action":"upload_file","artifact_id":"artifact:1","artifact_version_id":"artifact_version:1"}`))
	if err != nil {
		t.Fatalf("upload arguments: %v", err)
	}
	if values["action"] != "upload_file" {
		t.Errorf("upload action = %#v", values["action"])
	}
	assertWaitError := func(raw string, want string) {
		t.Helper()
		_, _, _, err := parseBrowserArguments(context.Background(), BrowseWaitName, json.RawMessage(raw))
		if err == nil {
			t.Errorf("wait arguments %s were accepted", raw)
			return
		}
		if err.Error() != want {
			t.Errorf("wait arguments %s error = %q, want %q", raw, err, want)
		}
	}
	assertWaitError(`{"condition":{"text":"ready","ref":"e1"}}`, "arguments do not match the web browse schema")
	assertWaitError(`{"timeout_ms":5000}`, "arguments do not match the web browse schema")
	assertWaitError(`{"condition":{"text":"`+strings.Repeat("x", 501)+`"}}`, "wait text is too long")
	_, revision, _, err := parseBrowserArguments(context.Background(), BrowseSwitchName, json.RawMessage(`{"snapshot_revision":4,"url":" https://1.1.1.1/start "}`))
	if err != nil {
		t.Fatalf("provider switch: %v", err)
	}
	if revision != 4 {
		t.Errorf("provider switch revision = %d, want 4", revision)
	}
	_, revision, _, err = parseBrowserArguments(context.Background(), BrowseSwitchName, json.RawMessage(`{"url":"https://1.1.1.1/start"}`))
	if err != nil {
		t.Fatalf("provider switch without revision: %v", err)
	}
	if revision != 0 {
		t.Errorf("failed-open provider switch revision = %d, want 0", revision)
	}
	var openDescription string
	for _, tool := range BrowserTools {
		if tool.Name == BrowseOpenName {
			openDescription = tool.Description
			break
		}
	}
	if !strings.Contains(openDescription, "Use web search to find sources") {
		t.Error("browser open description does not recommend web search")
	}
	if !strings.Contains(openDescription, "when JavaScript rendering or page interaction is necessary") {
		t.Error("browser open description does not state when browser use is necessary")
	}
}

// Rust source: crates/noema-capabilities/src/web/fetch.rs::exact_tool_spec_snapshot_is_stable.
func TestRustCapabilities_exact_tool_spec_snapshot_is_stable(t *testing.T) {
	var toolName, description string
	var schema json.RawMessage
	for _, tool := range Tools {
		if tool.Name == FetchName {
			toolName, description, schema = tool.Name, tool.Description, tool.InputSchema
			break
		}
	}
	gotSchema := decodeRustCapabilityJSON(t, schema)
	got := map[string]any{"name": toolName, "description": description, "input_schema": gotSchema}
	want := decodeRustCapabilityJSON(t, `{"name":"web.fetch","description":"Fetch and read a public web page or UTF-8 text resource using Noema's configured web fetch provider. When following a search result or fetched-page link, pass its exact URL unchanged.","input_schema":{"type":"object","properties":{"url":{"type":"string","minLength":1,"maxLength":2048,"description":"The public http(s) URL of a web page or text resource to fetch and read."},"reason":{"type":"string","maxLength":500,"description":"Brief reason this page is useful for the current response."},"max_chars":{"type":"integer","minimum":1000,"maximum":20000,"description":"Maximum characters to return after extraction and optional summarization."}},"required":["url"],"additionalProperties":false}}`)
	if !reflect.DeepEqual(got, want) {
		t.Errorf("web.fetch spec = %#v, want %#v", got, want)
	}
}

// Rust source: crates/noema-capabilities/src/web/fetch.rs::parser_error_does_not_echo_secret_values.
func TestRustCapabilities_parser_error_does_not_echo_secret_values(t *testing.T) {
	_, err := parseFetch(json.RawMessage(`{"url":"https://1.1.1.1","max_chars":"secret-value-that-must-not-leak"}`))
	if err == nil {
		t.Fatal("wrong max_chars type was accepted")
	}
	if err.Error() != "arguments do not match the web.fetch schema" {
		t.Errorf("parser error = %q", err)
	}
	if strings.Contains(err.Error(), "secret-value") {
		t.Error("parser error echoed the secret value")
	}
}

// Rust source: crates/noema-capabilities/src/web/fetch.rs::parser_normalizes_nested_arguments_and_enforces_bounds.
func TestRustCapabilities_parser_normalizes_nested_arguments_and_enforces_bounds(t *testing.T) {
	parsed, err := parseFetch(json.RawMessage(`{"arguments":{"url":"  https://1.1.1.1/page  ","reason":"  source  ","max_chars":1}}`))
	if err != nil {
		t.Fatalf("nested fetch arguments: %v", err)
	}
	want := fetchRequest{URL: "https://1.1.1.1/page", Reason: "source", MaxChars: 1000}
	if !reflect.DeepEqual(parsed, want) {
		t.Errorf("nested fetch = %#v, want %#v", parsed, want)
	}
	for _, raw := range []string{
		`{"url":"  "}`,
		`{"url":"[REDACTED_SENSITIVE_URL]","__noema_rejected_sensitive_url":true}`,
	} {
		if _, err := parseFetch(json.RawMessage(raw)); err == nil {
			t.Errorf("invalid fetch arguments were accepted: %s", raw)
		}
	}
	defaulted, err := parseFetch(json.RawMessage(`{"url":"https://1.1.1.1"}`))
	if err != nil {
		t.Fatalf("default fetch arguments: %v", err)
	}
	if defaulted.MaxChars != 20_000 {
		t.Errorf("default max chars = %d", defaulted.MaxChars)
	}
	_, err = parseFetch(json.RawMessage(`{"arguments":{"url":"https://1.1.1.1"},"operation_token":"forged"}`))
	if err == nil || err.Error() != "nested arguments payload cannot include outer fields" {
		t.Errorf("outer authority error = %v", err)
	}
}

// Rust source: crates/noema-capabilities/src/web/search.rs::exact_tool_spec_snapshot_is_stable.
func TestRustCapabilities_web_search_exact_tool_spec_snapshot_is_stable(t *testing.T) {
	var toolName, description string
	var schema json.RawMessage
	for _, tool := range Tools {
		if tool.Name == SearchName {
			toolName, description, schema = tool.Name, tool.Description, tool.InputSchema
			break
		}
	}
	got := map[string]any{"name": toolName, "description": description, "input_schema": decodeRustCapabilityJSON(t, schema)}
	want := decodeRustCapabilityJSON(t, `{"name":"web.search","description":"Search the public web using Noema's configured search provider.","input_schema":{"type":"object","properties":{"query":{"type":"string","minLength":1,"maxLength":500,"description":"The exact internet search query to send to the configured search provider."},"reason":{"type":"string","maxLength":500,"description":"Brief reason this search is useful for the current response."},"max_results":{"type":"integer","minimum":1,"maximum":10,"description":"Maximum number of search results to return."}},"required":["query"],"additionalProperties":false}}`)
	if !reflect.DeepEqual(got, want) {
		t.Errorf("web.search spec = %#v, want %#v", got, want)
	}
}

// Rust source: crates/noema-capabilities/src/web/search.rs::parser_rejects_outer_authority_fields.
func TestRustCapabilities_parser_rejects_outer_authority_fields(t *testing.T) {
	_, err := parseSearch(json.RawMessage(`{"arguments":{"query":"rust"},"invoker_key":"forged"}`))
	if err == nil || err.Error() != "nested arguments payload cannot include outer fields" {
		t.Errorf("outer field error = %v", err)
	}
}

// Rust source: crates/noema-capabilities/src/web/search.rs::parser_normalizes_nested_arguments_and_enforces_bounds.
func TestRustCapabilities_web_search_parser_normalizes_nested_arguments_and_enforces_bounds(t *testing.T) {
	parsed, err := parseSearch(json.RawMessage(`{"arguments":{"query":"  rust  ","reason":"  docs  ","max_results":99}}`))
	if err != nil {
		t.Fatalf("nested search arguments: %v", err)
	}
	want := searchRequest{Query: "rust", Reason: "docs", MaxResults: 10}
	if !reflect.DeepEqual(parsed, want) {
		t.Errorf("nested search = %#v, want %#v", parsed, want)
	}
	empty, err := parseSearch(json.RawMessage(`{"query":"  "}`))
	if err == nil || err.Error() != "web.search query is invalid" || empty.Query != "" {
		t.Errorf("empty query = %#v, %v", empty, err)
	}
	defaulted, err := parseSearch(json.RawMessage(`{"query":"rust"}`))
	if err != nil {
		t.Fatalf("default search arguments: %v", err)
	}
	if defaulted.MaxResults != 5 {
		t.Errorf("default result count = %d", defaulted.MaxResults)
	}
}

// Rust source: crates/noema-capabilities/src/web/search.rs::parser_error_does_not_echo_secret_values.
func TestRustCapabilities_web_search_parser_error_does_not_echo_secret_values(t *testing.T) {
	_, err := parseSearch(json.RawMessage(`{"query":"rust","max_results":"secret-value-that-must-not-leak"}`))
	if err == nil {
		t.Fatal("wrong max_results type was accepted")
	}
	if err.Error() != "arguments do not match the web.search schema" {
		t.Errorf("parser error = %q", err)
	}
	if strings.Contains(err.Error(), "secret-value") {
		t.Error("parser error echoed the secret value")
	}
}

func decodeRustCapabilityJSON(t *testing.T, raw any) any {
	t.Helper()
	var data []byte
	switch value := raw.(type) {
	case string:
		data = []byte(value)
	case json.RawMessage:
		data = value
	default:
		t.Fatalf("unsupported JSON input %T", raw)
	}
	var value any
	if err := json.Unmarshal(data, &value); err != nil {
		t.Fatalf("decode JSON: %v", err)
	}
	return value
}
