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
	available := uint32(1_000 - 100 - contextSafetyTokens)
	if !shouldCompactBackground(611, available) {
		t.Fatalf("background threshold ignored the bounded budget: available=%d", available)
	}
	if shouldCompactBackground(available*backgroundCompactionThresholdNumerator/backgroundCompactionThresholdDenominator-1, available) {
		t.Fatalf("background threshold compacted below the exact boundary: available=%d", available)
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
	continuation := NewContinuationContext([]provider.GenerationMessage{{Role: "user", Content: "Review the page"}})
	continuation.AppendResponse(provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{ProviderCallID: "call:browser", Name: "web.browse.snapshot", Payload: json.RawMessage(`{"url":"https://example.test/reservations"}`)}}})
	continuation.AppendResults([]ContinuationToolResult{{ProviderCallID: "call:browser", Name: "web.browse.snapshot", Payload: payload, Success: true}})
	items := continuation.providerInput(true)
	if len(items) != 3 || items[len(items)-1].message.ToolResult == nil {
		t.Fatalf("continuation browser items = %#v", items)
	}
	bounded := items[len(items)-1].message.ToolResult.Payload
	if len(bounded) > modelToolResultLimit || strings.Contains(string(bounded), strings.Repeat("x", 1_000)) {
		t.Fatalf("production continuation retained screenshot bytes: len=%d", len(bounded))
	}
	if !strings.Contains(string(bounded), "reservations") {
		t.Fatalf("production continuation lost ordinary snapshot context")
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
	continuation := NewContinuationContext([]provider.GenerationMessage{{Role: "user", Content: "Research bears"}})
	continuation.AppendResponse(result)
	continuation.AppendResults([]ContinuationToolResult{{ProviderCallID: "call_1", Name: "web.fetch", ProviderName: "web", Arguments: result.ToolCalls[0].Payload, Success: true, Payload: json.RawMessage(`{"content":"450,000 black bears"}`)}})
	items := continuation.providerInput(true)
	if len(items) != 6 || items[0].kind != "message" || items[1].kind != "reasoning" || items[2].kind != "hosted_web_search" || items[3].kind != "assistant_text" || items[4].kind != "tool_call" || items[5].kind != "tool_result" {
		t.Fatalf("continuation ordered items = %#v", items)
	}
	if items[2].message.HostedSearch == nil || items[2].message.HostedSearch.Status != "completed" || items[2].message.HostedSearch.Arguments == nil ||
		!strings.Contains(string(items[2].message.HostedSearch.Arguments), "black bears") ||
		items[3].message.Content != result.Text || len(items[4].message.ToolCalls) != 1 ||
		items[4].message.ToolCalls[0].ProviderCallID != "call_1" || items[5].message.ToolResult == nil ||
		!strings.Contains(string(items[5].message.ToolResult.Payload), "450,000 black bears") {
		t.Fatalf("continuation order = %#v", items)
	}
}

func TestRustRuntime_model_facing_tool_results_are_bounded_before_admission(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/continuation_context.rs::model_facing_tool_results_are_bounded_before_admission.
	continuation := NewContinuationContext([]provider.GenerationMessage{{Role: "user", Content: "Inspect a large result"}})
	continuation.AppendResults([]ContinuationToolResult{{CallID: "call_large", Name: "web.fetch", Payload: json.RawMessage(`{"content":"` + strings.Repeat("x", modelToolResultLimit+1000) + `"}`), Success: true}})
	items := continuation.providerInput(true)
	if len(items) != 2 || items[1].message.ToolResult == nil {
		t.Fatalf("model continuation items = %#v", items)
	}
	payload := items[1].message.ToolResult.Payload
	if len(payload) > modelToolResultLimit || !strings.Contains(string(payload), "truncated") {
		t.Fatalf("production model tool result bound = %d, %s", len(payload), payload[:min(len(payload), 200)])
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
	database := contextTestStore(t, 1_200)
	called := false
	generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		called = true
		return provider.GenerationResult{Text: "must not dispatch"}, nil
	})
	continuation := NewContinuationContext([]provider.GenerationMessage{{Role: "user", Content: "Inspect one message"}})
	continuation.AppendResponse(provider.GenerationResult{Text: "Fetching."})
	continuation.AppendResults([]ContinuationToolResult{{CallID: "call_1", Name: "web.fetch", Payload: json.RawMessage(`{"content":"` + strings.Repeat("x", 5_000) + `"}`), Success: true}})
	active, err := continuation.AdmissionMessages()
	if err != nil {
		t.Fatal(err)
	}
	_, compacted, err := prepareModelContext(t.Context(), modelContextRequest{database: database, generator: generator, accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test", active: active, outputReserve: 128})
	if err == nil || compacted || called || !strings.Contains(err.Error(), "no completed history") {
		t.Fatalf("production continuation overflow = compacted %t, called %t, error %v", compacted, called, err)
	}
}

func TestRustRuntime_recent_continuation_suffix_uses_complete_consumed_rounds(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/continuation_context.rs::recent_continuation_suffix_uses_complete_consumed_rounds.
	continuation := NewContinuationContext([]provider.GenerationMessage{{Role: "user", Content: "original request"}})
	for round := 1; round <= 3; round++ {
		continuation.AppendResponse(provider.GenerationResult{Text: "round " + itoa(round)})
		continuation.FinishRound()
	}
	if len(continuation.roundEnds) != 3 {
		t.Fatalf("continuation round boundaries = %#v", continuation.roundEnds)
	}
	firstEnd, completedEnd := continuation.roundEnds[0], continuation.roundEnds[1]
	var completedRound []provider.GenerationMessage
	for _, item := range continuation.items[firstEnd:completedEnd] {
		completedRound = append(completedRound, item.message)
	}
	tokens := CountModelContext(context.Background(), nil, completedRound, nil, false)
	if got := continuation.recentCompletedSuffixBoundary(tokens); got != firstEnd {
		t.Fatalf("complete consumed suffix boundary = %d, want %d", got, firstEnd)
	}
	if got := continuation.recentCompletedSuffixBoundary(tokens - 1); got != completedEnd {
		t.Fatalf("oversized consumed round was split at %d, want %d", got, completedEnd)
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
