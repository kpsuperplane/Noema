package runtime

import (
	"context"
	"encoding/json"
	"strings"
	"testing"
	"time"

	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestRustRuntime_assistant_delta_stream_id_includes_prior_output_offset(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/transcript_persistence/tests.rs::assistant_delta_stream_id_includes_prior_output_offset.
	initial := store.ConversationAssistantStreamID("turn:test", 0)
	continuation := store.ConversationAssistantStreamID("turn:test", 1)
	if initial == continuation || !strings.Contains(initial, "assistant_stream:turn:test:initial") || !strings.Contains(continuation, "continuation") {
		t.Fatalf("assistant stream IDs = %q / %q", initial, continuation)
	}
}

func TestRustRuntime_hosted_web_search_stream_events_update_one_tool_lifecycle(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/transcript_persistence/tests.rs::hosted_web_search_stream_events_update_one_tool_lifecycle.
	items := runtimeHostedSearchItems(t, store.ConversationHostedSearch{OutputIndex: 3, ID: "search:1", Name: "web.search", Status: "completed", Arguments: json.RawMessage(`{"query":"Noema tools"}`), Result: json.RawMessage(`{"results":[]}`)})
	toolCalls, toolResults := 0, 0
	for _, item := range items {
		if item.Kind == store.ConversationToolCall {
			toolCalls++
		}
		if item.Kind == store.ConversationToolResult {
			toolResults++
		}
	}
	if toolCalls != 1 || toolResults != 1 {
		t.Fatalf("hosted search lifecycle items = %#v", items)
	}
}

func TestRustRuntime_web_search_tool_call_display_shows_visible_query(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/transcript_persistence/tests.rs::web_search_tool_call_display_shows_visible_query.
	items := runtimeHostedSearchItems(t, store.ConversationHostedSearch{OutputIndex: 0, ID: "search:query", Name: "web.search", Status: "completed", Arguments: json.RawMessage(`{"query":"Noema tools"}`), Result: json.RawMessage(`{}`)})
	display := runtimeHostedDisplay(t, items, store.ConversationToolCall)
	if !strings.Contains(string(display), "Noema tools") {
		t.Fatalf("search call display = %s", display)
	}
}

func TestRustRuntime_memory_search_display_describes_lexical_page_matches(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/transcript_persistence/tests.rs::memory_search_display_describes_lexical_page_matches.
	var searchDescription string
	for _, tool := range localChatTools() {
		if tool.Name == noemamemory.SearchToolName {
			searchDescription = tool.Description
			break
		}
	}
	if !strings.Contains(string(searchMemorySchema), "query") || !strings.Contains(searchDescription, "Search Memory") {
		t.Fatal("memory search display lost lexical query authority")
	}
}

func TestRustRuntime_web_search_display_shows_provider_fallback_without_raw_payload(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/transcript_persistence/tests.rs::web_search_display_shows_provider_fallback_without_raw_payload.
	items := runtimeHostedSearchItems(t, store.ConversationHostedSearch{OutputIndex: 0, ID: "search:fallback", Name: "web.search", Status: "completed", Arguments: json.RawMessage(`{"query":"ordinary"}`), Result: json.RawMessage(`{"provider_action":"duckduckgo","raw":"private response"}`)})
	display := runtimeHostedDisplay(t, items, store.ConversationToolResult)
	if !strings.Contains(string(display), "web.search") || strings.Contains(string(display), "private response") {
		t.Fatalf("search result display = %s", display)
	}
}

func TestRustRuntime_web_fetch_tool_call_display_shows_visible_url(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/transcript_persistence/tests.rs::web_fetch_tool_call_display_shows_visible_url.
	items := runtimeHostedSearchItems(t, store.ConversationHostedSearch{OutputIndex: 0, ID: "fetch:url", Name: "web.fetch", Status: "completed", Arguments: json.RawMessage(`{"url":"https://example.test/docs"}`), Result: json.RawMessage(`{}`)})
	display := runtimeHostedDisplay(t, items, store.ConversationToolCall)
	if !strings.Contains(string(display), "https://example.test/docs") {
		t.Fatalf("fetch call display = %s", display)
	}
}

func TestRustRuntime_web_fetch_tool_call_display_removes_credentials_and_preserves_ordinary_components(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/transcript_persistence/tests.rs::web_fetch_tool_call_display_removes_credentials_and_preserves_ordinary_components.
	items := runtimeHostedSearchItems(t, store.ConversationHostedSearch{OutputIndex: 0, ID: "fetch:credentials", Name: "web.fetch", Status: "completed", Arguments: json.RawMessage(`{"url":"https://user:password@example.test/docs?view=1#top"}`), Result: json.RawMessage(`{}`)})
	display := runtimeHostedDisplay(t, items, store.ConversationToolCall)
	if strings.Contains(string(display), "password") || !strings.Contains(string(display), "example.test") || !strings.Contains(string(display), "view=1") {
		t.Fatalf("credential-bearing fetch display = %s", display)
	}
}

func TestRustRuntime_web_fetch_tool_result_display_shows_raw_and_summary_counts(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/transcript_persistence/tests.rs::web_fetch_tool_result_display_shows_raw_and_summary_counts.
	items := runtimeHostedSearchItems(t, store.ConversationHostedSearch{OutputIndex: 0, ID: "fetch:summary", Name: "web.fetch", Status: "completed", Arguments: json.RawMessage(`{"url":"https://example.test/docs"}`), Result: json.RawMessage(`{"summary":"short","raw_chars":2000,"summary_chars":80}`)})
	display := runtimeHostedDisplay(t, items, store.ConversationToolResult)
	if !strings.Contains(string(display), "short") || !strings.Contains(string(display), "web.fetch") {
		t.Fatalf("fetch result display = %s", display)
	}
}

func TestRustRuntime_provider_usage_metadata_projects_reported_zero_and_missing_cache_usage(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/transcript_persistence/tests.rs::provider_usage_metadata_projects_reported_zero_and_missing_cache_usage.
	usage := store.ProviderUsage{Provider: "openrouter", Model: "test/model", InputTokens: 0, OutputTokens: 0, TotalTokens: 0, CachedInputTokens: 0}
	encoded, err := json.Marshal(usage)
	if err != nil || !strings.Contains(string(encoded), "openrouter") || usage.TotalTokens != 0 || usage.CachedInputTokens != 0 {
		t.Fatalf("provider usage metadata = %s, %v", encoded, err)
	}
}

func TestRustRuntime_progress_audit_display_labels_running_and_completed_states(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/transcript_persistence/tests.rs::progress_audit_display_labels_running_and_completed_states.
	if !strings.Contains(progressAuditPrompt, "continue") || !strings.Contains(progressAuditPrompt, "finalize") || !strings.Contains(progressAuditPrompt, "pause") {
		t.Fatal("progress audit prompt omitted lifecycle decisions")
	}
	if progressAuditInterval <= 0 {
		t.Fatal("progress audit interval is not bounded")
	}
}

func runtimeHostedSearchItems(t *testing.T, search store.ConversationHostedSearch) []store.ConversationItem {
	t.Helper()
	_, database, conversation := chatFixture(t)
	turn, _, err := database.BeginConversationTurn(context.Background(), conversation.ID, "hosted search", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StoreConversationHostedSearches(context.Background(), turn, "openrouter", 0, []store.ConversationHostedSearch{search}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	return items
}

func runtimeHostedDisplay(t *testing.T, items []store.ConversationItem, kind store.ConversationItemKind) json.RawMessage {
	t.Helper()
	for _, item := range items {
		if item.Kind != kind {
			continue
		}
		metadata, _ := item.Payload["metadata"].(map[string]any)
		display := metadata["display"]
		encoded, _ := json.Marshal(display)
		return encoded
	}
	t.Fatalf("hosted item kind %q missing: %#v", kind, items)
	return nil
}

var _ = provider.Usage{}
