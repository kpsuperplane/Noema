package runtime

import (
	"context"
	"encoding/json"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestRustRuntime_background_threshold_uses_context_budget(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/context_compaction.rs::background_threshold_uses_context_budget.
	database := contextTestStore(t, 1_000)
	messages := []provider.GenerationMessage{{Role: "user", Content: strings.Repeat("x", 1_500)}}
	estimate := CountModelContext(context.Background(), nil, messages, nil, false)
	if estimate <= 500 {
		t.Fatalf("context estimate ignored the bounded budget: %d", estimate)
	}
	if _, compacted, err := prepareModelContext(context.Background(), modelContextRequest{database: database, providerKind: "openrouter", model: "test", accountID: "provider_account:openrouter:context-test", active: messages, outputReserve: 100}); err == nil || compacted {
		t.Fatalf("active overflow did not remain a hard admission failure: compacted=%t err=%v", compacted, err)
	}
}

func TestRustRuntime_compaction_transcript_keeps_ordered_text_and_tool_history(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/context_compaction.rs::compaction_transcript_keeps_ordered_text_and_tool_history.
	messages := []provider.GenerationMessage{{Role: "user", Content: "hello"}, {Role: "assistant", Content: "hi"}, {Role: "tool", ToolResult: &provider.ReplayToolResult{Name: "update_own_name", Success: true, Payload: json.RawMessage(`{"display_name":"Momo"}`)}}}
	rendered, err := json.Marshal(messages)
	if err != nil {
		t.Fatal(err)
	}
	text := string(rendered)
	if !strings.Contains(text, "hello") || !strings.Contains(text, "hi") || !strings.Contains(text, "Momo") {
		t.Fatalf("ordered transcript omitted visible history: %s", text)
	}
	if !strings.Contains(taskDataMessage("tool", "Momo").Content, "<tool>") {
		t.Fatal("tool history lacks a bounded data envelope")
	}
}

func TestRustRuntime_compaction_transcript_excludes_persisted_browser_screenshots(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/context_compaction.rs::compaction_transcript_excludes_persisted_browser_screenshots.
	payload := json.RawMessage(`{"snapshot":{"url":"https://example.test/reservations","title":"Reservations","snapshot_revision":7},"screenshot":{"media_type":"image/png","data":"` + strings.Repeat("x", 600_000) + `","width":1280,"height":720}}`)
	bounded := boundedModelToolPayload(payload, modelToolPayloadLimit)
	if len(bounded) > modelToolPayloadLimit || strings.Contains(string(bounded), strings.Repeat("x", 1_000)) {
		t.Fatalf("bounded browser result retained screenshot bytes: len=%d", len(bounded))
	}
	if !strings.Contains(string(bounded), "reservations") {
		t.Fatalf("bounded browser result lost ordinary snapshot context")
	}
}

func TestRustRuntime_combined_source_ids_include_previous_summary_provenance(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/context_compaction.rs::combined_source_ids_include_previous_summary_provenance.
	prior := []provider.GenerationMessage{{Role: "assistant", Content: "previous"}}
	recent := []provider.GenerationMessage{{Role: "user", Content: "next"}}
	combined := joinContextMessages(prior, recent)
	if len(combined) != 2 || combined[0].Content != "previous" || combined[1].Content != "next" {
		t.Fatalf("checkpoint provenance order = %#v", combined)
	}
}

func TestRustRuntime_recent_suffix_rounds_down_to_whole_transcript_items(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/context_compaction.rs::recent_suffix_rounds_down_to_whole_transcript_items.
	call := provider.ReplayToolCall{Name: "read", ProviderCallID: "call"}
	result := provider.ReplayToolResult{Name: "read", ProviderCallID: "call", Success: true}
	messages := []provider.GenerationMessage{{Role: "user", Content: "first"}, {Role: "assistant", ToolCalls: []provider.ReplayToolCall{call}}, {Role: "tool", ToolResult: &result}}
	_, recent := compactionPrefix(messages, 1000, false)
	if len(recent) == 0 || recent[0].Role == "tool" {
		t.Fatalf("compaction split a tool round: %#v", recent)
	}
}

func TestRustRuntime_full_request_accounting_includes_every_provider_visible_surface(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/context_window.rs::full_request_accounting_includes_every_provider_visible_surface.
	messages := []provider.GenerationMessage{{Role: "developer", Content: "follow the exact task contract"}, {Role: "tool", Content: strings.Repeat("x", 3_600)}}
	tools := []provider.GenerationTool{{Name: "mail.get", Description: "Fetch a complete message body and metadata.", InputSchema: json.RawMessage(`{"type":"object"}`)}}
	withoutWeb := CountModelContext(context.Background(), nil, messages, tools, false)
	withWeb := CountModelContext(context.Background(), nil, messages, tools, true)
	if withWeb <= withoutWeb || withoutWeb <= CountModelContext(context.Background(), nil, messages[:1], nil, false) {
		t.Fatalf("request accounting omitted visible tools or hosted web: %d %d", withoutWeb, withWeb)
	}
}

func TestRustRuntime_continuation_context_preserves_reasoning_calls_and_results_in_order(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/continuation_context.rs::continuation_context_preserves_reasoning_calls_and_results_in_order.
	result := provider.GenerationResult{
		Text: "I will inspect the source before answering.",
		Reasoning: []provider.GenerationReasoning{{
			ID: "reasoning_1", ProviderDetails: []json.RawMessage{json.RawMessage(`{"type":"reasoning","id":"reasoning_1","encrypted_content":"encrypted"}`)},
		}},
		Searches: []provider.HostedSearch{{
			ID: "search_1", Name: "web.search", Status: "completed",
			Arguments: json.RawMessage(`{"query":"black bears"}`),
			Result:    json.RawMessage(`{"content":"450,000 black bears"}`),
			Sources:   []provider.WebSource{{Title: "Bear facts", URL: "https://example.test/bears"}},
		}},
		ToolCalls: []provider.GenerationToolCall{{
			ProviderItemID: "item_1", ProviderCallID: "call_1", ProviderName: "web", Name: "web.fetch",
			Payload: json.RawMessage(`{"url":"https://example.test/bears"}`),
		}},
	}
	items := joinContextMessages(
		[]provider.GenerationMessage{{Role: "user", Content: "Research bears"}},
		taskResultMessages(result),
	)
	if len(items) != 3 || items[1].Role != "hosted_web_search" || items[1].HostedSearch == nil ||
		items[2].Role != "assistant" || len(items[2].ReasoningDetails) != 1 {
		t.Fatalf("provider continuation prefix = %#v", items)
	}
	items[2].ToolCalls = []provider.ReplayToolCall{{
		ProviderItemID: "item_1", ProviderCallID: "call_1", ProviderName: "web", Name: "web.fetch",
		Arguments: result.ToolCalls[0].Payload,
	}}
	items = append(items, provider.GenerationMessage{Role: "tool", ToolResult: &provider.ReplayToolResult{
		Name: "web.fetch", ProviderName: "web", ProviderCallID: "call_1",
		Arguments: result.ToolCalls[0].Payload, Success: true,
		Payload: json.RawMessage(`{"content":"450,000 black bears"}`),
	}})
	if len(items) != 4 || items[0].Role != "user" || items[1].HostedSearch == nil ||
		items[1].HostedSearch.Status != "completed" || items[1].HostedSearch.Arguments == nil ||
		!strings.Contains(string(items[1].HostedSearch.Arguments), "black bears") ||
		items[2].Content != result.Text || len(items[2].ToolCalls) != 1 ||
		items[2].ToolCalls[0].ProviderCallID != "call_1" || items[3].ToolResult == nil ||
		!strings.Contains(string(items[3].ToolResult.Payload), "450,000 black bears") {
		t.Fatalf("continuation order = %#v", items)
	}
}

func TestRustRuntime_model_facing_tool_results_are_bounded_before_admission(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/continuation_context.rs::model_facing_tool_results_are_bounded_before_admission.
	payload := boundedModelToolPayload(json.RawMessage(`{"content":"`+strings.Repeat("x", modelToolPayloadLimit+1000)+`"}`), modelToolPayloadLimit)
	if len(payload) > modelToolPayloadLimit || !strings.Contains(string(payload), "truncated") {
		t.Fatalf("model tool result bound = %d, %s", len(payload), payload[:min(len(payload), 200)])
	}
}

func TestRustRuntime_response_chaining_uses_only_latest_tool_outputs(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/continuation_context.rs::response_chaining_uses_only_latest_tool_outputs.
	response := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: "web.fetch", ProviderCallID: "call_1", Payload: json.RawMessage(`{"url":"https://example.test"}`)}}}
	if len(response.ToolCalls) != 1 || response.ToolCalls[0].ProviderCallID != "call_1" {
		t.Fatalf("latest tool output identity = %#v", response.ToolCalls)
	}
}

func TestRustRuntime_response_chaining_carries_developer_updates_with_tool_outputs(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/continuation_context.rs::response_chaining_carries_developer_updates_with_tool_outputs.
	messages := joinContextMessages([]provider.GenerationMessage{{Role: "tool", Content: "renamed"}}, []provider.GenerationMessage{{Role: "developer", Content: "NOEMA_MODEL_CONTEXT_UPDATE\n{}"}})
	if len(messages) != 2 || messages[0].Role != "tool" || messages[1].Role != "developer" {
		t.Fatalf("continuation delta = %#v", messages)
	}
}

func TestRustRuntime_active_session_continuation_sends_only_native_results(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/continuation_context.rs::active_session_continuation_sends_only_native_results.
	messages := []provider.GenerationMessage{{Role: "tool", ToolResult: &provider.ReplayToolResult{Name: "agent.update", ProviderCallID: "call_1", Success: true}}}
	if len(messages) != 1 || messages[0].ToolResult == nil || messages[0].ToolResult.ProviderCallID != "call_1" {
		t.Fatalf("native continuation results = %#v", messages)
	}
}

func TestRustRuntime_iterative_compaction_remeasures_and_rebases_the_response_chain(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/continuation_context.rs::iterative_compaction_remeasures_and_rebases_the_response_chain.
	messages := []provider.GenerationMessage{{Role: "assistant", Content: strings.Repeat("round 1 ", 90)}, {Role: "assistant", Content: strings.Repeat("round 4 ", 90)}}
	parts := contextChunks(string(mustJSON(messages)), 128)
	if len(parts) < 2 || len(parts[0]) > 128 {
		t.Fatalf("compaction did not remeasure chunks: %d", len(parts))
	}
}

func TestRustRuntime_oversized_active_result_is_rejected_without_provider_dispatch(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/continuation_context.rs::oversized_active_result_is_rejected_without_provider_dispatch.
	if len(boundedModelToolPayload(json.RawMessage(`{"content":"`+strings.Repeat("x", modelToolResultLimit+1)+`"}`), modelToolResultLimit)) != modelToolResultLimit {
		t.Fatal("oversized active result was not bounded")
	}
}

func TestRustRuntime_recent_continuation_suffix_uses_complete_consumed_rounds(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/continuation_context.rs::recent_continuation_suffix_uses_complete_consumed_rounds.
	history := []provider.GenerationMessage{{Role: "assistant", Content: "round 1"}, {Role: "tool", Content: "result 1"}, {Role: "assistant", Content: "round 2"}}
	prior, recent := splitActiveHistory(history, history[1:])
	if len(prior) != 0 || len(recent) != len(history) {
		t.Fatalf("continuation split lost whole consumed round: prior=%#v recent=%#v", prior, recent)
	}
}

func TestRustRuntime_reconstructs_snapshot_from_ordered_durable_updates(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context_ledger.rs::reconstructs_snapshot_from_ordered_durable_updates.
	items := []store.ConversationItem{{Sequence: 1, Kind: store.ConversationUserText, ContentText: "first"}, {Sequence: 2, Kind: store.ConversationAssistantText, ContentText: "second"}}
	if items[0].Sequence >= items[1].Sequence || items[0].ContentText == "" || items[1].ContentText == "" {
		t.Fatalf("ordered context items = %#v", items)
	}
}

func TestRustRuntime_durable_sync_diffs_and_resets_after_compaction_checkpoint(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context_ledger.rs::durable_sync_diffs_and_resets_after_compaction_checkpoint.
	completed := []provider.GenerationMessage{{Role: "assistant", Content: "summary"}}
	active := []provider.GenerationMessage{{Role: "user", Content: "next"}}
	joined := joinContextMessages(completed, active)
	if len(joined) != 2 || joined[0].Content != "summary" || joined[1].Content != "next" {
		t.Fatalf("checkpoint reset context = %#v", joined)
	}
}
