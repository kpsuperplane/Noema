package webtool

import (
	"encoding/json"
	"reflect"
	"strings"
	"testing"
)

// Rust source: crates/noema-capabilities/src/web/fetch.rs::credential_url_components_are_removed_at_every_persisted_position.
func TestRustCapabilities_credential_url_components_are_removed_at_every_persisted_position(t *testing.T) {
	sanitized := sanitizePayloadForStorage(map[string]any{
		"url": "https://user:secret@example.com/path?view=full&access_token=private#section",
	}).(map[string]any)
	if sanitized["url"] != "https://example.com/path?view=full#section" || sanitized["__noema_rejected_sensitive_url"] != true {
		t.Fatalf("sanitized URL = %#v", sanitized)
	}
	nested := sanitizePayloadForStorage(map[string]any{"result": map[string]any{
		"url":       "https://example.com/safe#overview",
		"final_url": "https://example.com/path?x-amz-signature=private&part=1#download",
	}}).(map[string]any)
	result, ok := nested["result"].(map[string]any)
	if !ok || result["url"] != "https://example.com/safe#overview" ||
		result["final_url"] != "https://example.com/path?part=1#download" ||
		result["__noema_rejected_sensitive_url"] != true {
		t.Fatalf("nested sanitized URLs = %#v", nested)
	}
	if got := sanitizedDisplayURL("malformed secret-value"); got != "[redacted sensitive web.fetch URL]" {
		t.Fatalf("malformed URL marker = %q", got)
	}
}

// Rust source: crates/noema-capabilities/src/web/fetch.rs::summary_size_policy_is_stable.
func TestRustCapabilities_summary_size_policy_is_stable(t *testing.T) {
	if summaryStrategyForChars(rawMarkdownLimit) != fetchSummaryRaw {
		t.Fatal("raw threshold changed")
	}
	if summaryStrategyForChars(rawMarkdownLimit+1) != fetchSummarySingle {
		t.Fatal("single-pass threshold changed")
	}
	if summaryStrategyForChars(singlePassLimit+1) != fetchSummaryChunked {
		t.Fatal("chunked threshold changed")
	}
	if summaryStrategyForChars(chunkedSummaryLimit+1) != fetchSummaryRefuse {
		t.Fatal("refusal threshold changed")
	}
	if len([]rune(rawExcerpt(strings.Repeat("x", rawExcerptLimit+1)))) != rawExcerptLimit {
		t.Fatal("raw excerpt bound changed")
	}
	if !reflect.DeepEqual(sanitizePayloadForStorage(map[string]any{"url": "https://example.com"}), map[string]any{"url": "https://example.com"}) {
		t.Fatal("ordinary URL changed")
	}
	if _, err := json.Marshal(fetchSummaryRaw); err != nil {
		t.Fatalf("summary decision is not serializable: %v", err)
	}
}
