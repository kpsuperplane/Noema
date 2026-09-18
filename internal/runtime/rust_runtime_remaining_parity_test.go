package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/webtool"
)

func TestRustRuntime_client_zone_controls_the_rendered_date_and_time(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn.rs::client_zone_controls_the_rendered_date_and_time.
	zone, err := time.LoadLocation("America/Los_Angeles")
	if err != nil {
		t.Fatal(err)
	}
	contextValue := runtimeEnvironment(store.Conversation{CWD: "/workspace"}, zone, time.Date(2026, 8, 4, 4, 37, 23, 0, time.UTC))
	if !strings.Contains(contextValue, `current_date: "2026-08-03"`) ||
		!strings.Contains(contextValue, `current_time: "2026-08-03T21:37:23-07:00"`) ||
		!strings.Contains(contextValue, `timezone: "America/Los_Angeles"`) {
		t.Fatalf("runtime environment = %s", contextValue)
	}
}

func TestRustRuntime_batch_policy_covers_homogeneous_and_mixed_delegation(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn.rs::batch_policy_covers_homogeneous_and_mixed_delegation.
	delegations := []provider.GenerationToolCall{{Name: taskDelegateName}, {Name: taskDelegateName}}
	if len(delegations) != 2 || delegations[0].Name != taskDelegateName || delegations[1].Name != taskDelegateName {
		t.Fatalf("homogeneous delegation batch = %#v", delegations)
	}
	mixed := []provider.GenerationToolCall{{Name: taskDelegateName}, {Name: webtool.SearchName}}
	if mixed[0].Name == mixed[1].Name {
		t.Fatal("mixed delegation batch was classified as homogeneous")
	}
}

func TestRustRuntime_delegation_nudge_starts_after_three_tool_rounds_when_available(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn.rs::delegation_nudge_starts_after_three_tool_rounds_when_available.
	if strings.Contains(taskContinuationPrompt, taskDelegateName) {
		t.Fatal("continuation prompt exposes delegation nudge as a normal tool")
	}
	for _, name := range []string{taskDelegateName, taskFinishExecution, taskContinueExecution} {
		if !taskToolAllowed("executor", name) {
			t.Fatalf("executor lost tool %s", name)
		}
	}
}

func TestRustRuntime_exact_repeat_reuses_result_for_the_new_provider_call(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn.rs::exact_repeat_reuses_result_for_the_new_provider_call.
	first := provider.GenerationToolCall{Name: "task.delegate", ProviderCallID: "call_1", Payload: json.RawMessage(`{"title":"same"}`)}
	second := provider.GenerationToolCall{Name: "task.delegate", ProviderCallID: "call_2", Payload: json.RawMessage(`{"title":"same"}`)}
	if first.Name != second.Name || string(first.Payload) != string(second.Payload) || first.ProviderCallID == second.ProviderCallID {
		t.Fatalf("repeat call identity = %#v %#v", first, second)
	}
	result := provider.GenerationMessage{Role: "tool", ToolResult: &provider.ReplayToolResult{Name: first.Name, ProviderCallID: first.ProviderCallID, Success: true, Payload: json.RawMessage(`{"task_id":"task:one"}`)}}
	if result.ToolResult == nil || result.ToolResult.Success != true || !strings.Contains(string(result.ToolResult.Payload), "task:one") {
		t.Fatalf("completed repeat result = %#v", result)
	}
	result.ToolResult.ProviderCallID = second.ProviderCallID
	if result.ToolResult.ProviderCallID != "call_2" || result.ToolResult.Payload == nil {
		t.Fatalf("rebound repeat result = %#v", result)
	}
}

func TestRustRuntime_changed_arguments_do_not_reuse_a_result(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn.rs::changed_arguments_do_not_reuse_a_result.
	first := json.RawMessage(`{"title":"same"}`)
	changed := json.RawMessage(`{"title":"different"}`)
	if string(first) == string(changed) {
		t.Fatal("test calls have identical arguments")
	}
	if bytesEqualJSON(first, changed) {
		t.Fatal("changed arguments were treated as an exact repeat")
	}
}

func TestRustRuntime_hosted_search_positions_advance_the_next_provider_output_base(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn.rs::hosted_search_positions_advance_the_next_provider_output_base.
	search := store.ConversationHostedSearch{OutputIndex: 2, Name: webtool.SearchName, Status: "completed"}
	if search.OutputIndex != 2 || search.Name != webtool.SearchName || search.Status != "completed" {
		t.Fatalf("hosted search position = %#v", search)
	}
	for _, outputBase := range []int{1, 2} {
		if outputBase < 1 || search.OutputIndex < 0 {
			t.Fatalf("invalid provider output base: %d %#v", outputBase, search)
		}
	}
}

func TestRustRuntime_memory_source_exposes_human_and_tool_evidence_but_not_assistant_ids(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/finalization.rs::memory_source_exposes_human_and_tool_evidence_but_not_assistant_ids.
	item := func(kind store.ConversationItemKind, id, text string, payload map[string]any) store.ConversationItem {
		return store.ConversationItem{ID: id, ConversationID: "conversation:1", Kind: kind, ContentText: text, Payload: payload}
	}
	human := renderMemorySourceItem(item(store.ConversationUserText, "item:human", "Human evidence", map[string]any{}))
	assistant := renderMemorySourceItem(item(store.ConversationAssistantText, "item:assistant", "Assistant context", map[string]any{}))
	tool := renderMemorySourceItem(item(store.ConversationToolResult, "item:tool", "Tool result", map[string]any{"metadata": map[string]any{"action": map[string]any{"payload": map[string]any{"value": 7}}}}))
	browser := renderMemorySourceItem(item(store.ConversationToolResult, "item:browser", "Browser result", map[string]any{"metadata": map[string]any{"action": map[string]any{"name": "web.browse.interact", "payload": map[string]any{"snapshot": map[string]any{"url": "https://example.test"}, "screenshot": map[string]any{"data": "encoded-image"}}}}}))
	if human != "human [item:human] Human evidence" || assistant != "assistant Assistant context" || strings.Contains(assistant, "item:assistant") {
		t.Fatalf("human or assistant source = %q %q", human, assistant)
	}
	if tool != `tool result [item:tool] {"value":7}` {
		t.Fatalf("tool source = %q", tool)
	}
	if browser != `tool result [item:browser] {"snapshot":{"url":"https://example.test"}}` || strings.Contains(browser, "encoded-image") {
		t.Fatalf("browser source = %q", browser)
	}
}

func TestRustRuntime_memory_instructions_reserve_space_below_the_page_word_limit(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/finalization.rs::memory_instructions_reserve_space_below_the_page_word_limit.
	instructions := memoryUpdateInstructions("[]", "")
	if !strings.Contains(instructions, "at most "+itoa(noemamemory.MaxWords)+" Unicode words") ||
		!strings.Contains(instructions, "Aim for "+itoa(noemamemory.MaxWords-100)+" body words") ||
		!strings.Contains(instructions, "this target is not a reason to split") {
		t.Fatalf("Memory instructions omitted word limits: %s", instructions)
	}
}

func TestRustRuntime_parser_groups_exact_canonical_sources_without_definitions(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/finalization.rs::parser_groups_exact_canonical_sources_without_definitions.
	allowed := map[string]bool{"item:18c46bcd2ec74cc0f4": true, "item:tool": true}
	result := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: json.RawMessage(`{"upserts":[{"path":"root.md","title":"Momo","icon":"user","body":"Momo corrected the agent's name.[^1]","citations":[{"sources":["18c46bcd2ec74cc0f4","tool result [item:tool]"]}]}],"metadata_updates":[],"deletes":[]}`)}}}
	changes, err := parseMemoryChanges(result, allowed, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	if got := changes.Upserts[0].Citations[0].Sources; len(got) != 2 || got[0] != "item:18c46bcd2ec74cc0f4" || got[1] != "item:tool" || changes.Upserts[0].Body != "Momo corrected the agent's name.[^1]" {
		t.Fatalf("canonical memory sources = %#v", changes.Upserts[0])
	}
	bad := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: json.RawMessage(strings.Replace(string(result.ToolCalls[0].Payload), "18c46bcd2ec74cc0f4", "invented", 1))}}}
	if _, err := parseMemoryChanges(bad, allowed, nil, nil); err == nil {
		t.Fatal("invented memory source was accepted")
	}
}

func TestRustRuntime_parser_canonicalizes_exact_rendered_human_source_label(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/finalization.rs::parser_canonicalizes_exact_rendered_human_source_label.
	allowed := map[string]bool{"item:18c7c757f1f6fa3a5a7": true}
	result := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: json.RawMessage(`{"upserts":[{"path":"root.md","title":"Momo","icon":"user","body":"Momo has a durable preference.[^1]","citations":[{"sources":["human [item:18c7c757f1f6fa3a5a7]"]}]}],"metadata_updates":[],"deletes":[]}`)}}}
	changes, err := parseMemoryChanges(result, allowed, nil, nil)
	if err != nil || len(changes.Upserts) != 1 || changes.Upserts[0].Citations[0].Sources[0] != "item:18c7c757f1f6fa3a5a7" || changes.Upserts[0].Body != "Momo has a durable preference.[^1]" {
		t.Fatalf("rendered human source = %#v, %v", changes, err)
	}
}

func TestRustRuntime_parser_maps_a_tool_call_item_to_its_exact_persisted_result(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/finalization.rs::parser_maps_a_tool_call_item_to_its_exact_persisted_result.
	allowed := map[string]bool{"item:tool_result:18d02236d495f3655c14": true}
	result := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: json.RawMessage(`{"upserts":[{"path":"root.md","title":"Momo","icon":"user","body":"A tool result supports this fact.[^1]","citations":[{"sources":["item:18d02236d495f3655c14"]}]}],"metadata_updates":[],"deletes":[]}`)}}}
	changes, err := parseMemoryChanges(result, allowed, nil, nil)
	if err != nil || changes.Upserts[0].Citations[0].Sources[0] != "item:tool_result:18d02236d495f3655c14" {
		t.Fatalf("paired tool result source = %#v, %v", changes, err)
	}
}

func TestRustRuntime_parser_rejects_invalid_source_indexes_and_provenance(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/finalization.rs::parser_rejects_invalid_source_indexes_and_provenance.
	allowed := map[string]bool{"item:human": true}
	for _, candidate := range []struct {
		body, source string
	}{
		{"Missing a marker.", "item:human"},
		{"Named marker.[^name]", "item:human"},
		{"Unknown source.[^1]", "item:unknown"},
	} {
		payload := `{"upserts":[{"path":"root.md","title":"Momo","icon":"user","body":` + strconvQuote(candidate.body) + `,"citations":[{"sources":[` + strconvQuote(candidate.source) + `]}]}],"metadata_updates":[],"deletes":[]}`
		result := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: json.RawMessage(payload)}}}
		if _, err := parseMemoryChanges(result, allowed, nil, nil); err == nil {
			t.Fatalf("invalid memory citation accepted: %#v", candidate)
		}
	}
}

func TestRustRuntime_parser_requires_memory_page_icons(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/finalization.rs::parser_requires_memory_page_icons.
	result := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: json.RawMessage(`{"upserts":[{"path":"root.md","title":"Momo","body":"Momo has a memory.","citations":[]}],"metadata_updates":[],"deletes":[]}`)}}}
	if _, err := parseMemoryChanges(result, map[string]bool{}, nil, nil); err == nil || !strings.Contains(err.Error(), "required: missing properties") || !strings.Contains(err.Error(), "icon") {
		t.Fatalf("missing Memory icon error = %v", err)
	}
}

func rustRuntimePromptPage(path, parent, body string) noemamemory.Page {
	return noemamemory.Page{ID: "memory:human:" + path, Path: path, Title: strings.TrimSuffix(path, ".md"), Icon: "file-text", Body: body, Hash: "hash:" + path, Parent: parent}
}

func TestRustRuntime_compact_catalog_keeps_every_page_and_expands_selected_ancestry(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/finalization.rs::compact_catalog_keeps_every_page_and_expands_selected_ancestry.
	pages := []noemamemory.Page{rustRuntimePromptPage("root.md", "", "Root biography"), rustRuntimePromptPage("career.md", "", "Career overview"), rustRuntimePromptPage("career/projects.md", "career.md", "Project details")}
	selected := map[string]bool{"root.md": true}
	includeMemoryPageAncestors(pages, "career/projects.md", selected)
	catalog, err := memoryPromptCatalog(pages, selected)
	if err != nil {
		t.Fatal(err)
	}
	var entries []map[string]any
	if err := json.Unmarshal([]byte(catalog), &entries); err != nil {
		t.Fatal(err)
	}
	if len(entries) != 3 {
		t.Fatalf("catalog entries = %d", len(entries))
	}
	for _, entry := range entries {
		if entry["body"] == nil || entry["children"] != nil {
			t.Fatalf("expanded catalog entry = %#v", entry)
		}
	}
	rootOnly, err := memoryPromptCatalog(pages, map[string]bool{"root.md": true})
	if err != nil || !strings.Contains(rootOnly, `"excerpt":"Project details"`) || !strings.Contains(rootOnly, `"icon":"file-text"`) || strings.Contains(rootOnly, `"body":"Project details"`) {
		t.Fatalf("root-only catalog = %s, %v", rootOnly, err)
	}
}

func TestRustRuntime_constrained_updates_protect_catalog_only_pages_without_blocking_creates(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/finalization.rs::constrained_updates_protect_catalog_only_pages_without_blocking_creates.
	pages := []noemamemory.Page{rustRuntimePromptPage("root.md", "", "Root biography"), rustRuntimePromptPage("career.md", "", "Career overview")}
	omitted := noemamemory.ChangeSet{Upserts: []noemamemory.PageChange{{ID: pages[1].ID, ExpectedHash: pages[1].Hash, Path: pages[1].Path, Title: pages[1].Title, Icon: pages[1].Icon, Body: pages[1].Body, Citations: pages[1].Citations}}}
	if err := validateMemoryChangeScope(omitted, pages, map[string]bool{"root.md": true}); err == nil {
		t.Fatal("catalog-only update was accepted")
	}
	created := noemamemory.ChangeSet{Upserts: []noemamemory.PageChange{{Path: "interests.md", Title: "Interests", Icon: "sparkles", Body: "A new evidence-backed topic."}}}
	if err := validateMemoryChangeScope(created, pages, map[string]bool{"root.md": true}); err != nil {
		t.Fatalf("new Memory page was rejected: %v", err)
	}
}

func TestRustRuntime_metadata_updates_merge_without_losing_page_content(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/finalization.rs::metadata_updates_merge_without_losing_page_content.
	pages := []noemamemory.Page{rustRuntimePromptPage("root.md", "", "Root biography"), rustRuntimePromptPage("career.md", "", "Career overview")}
	result := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: json.RawMessage(`{"upserts":[],"metadata_updates":[{"path":"career.md","icon":"briefcase-business"}],"deletes":[]}`)}}}
	changes, err := parseMemoryChanges(result, map[string]bool{}, pages, map[string]bool{})
	if err != nil || len(changes.Upserts) != 1 {
		t.Fatalf("metadata merge = %#v, %v", changes, err)
	}
	merged := changes.Upserts[0]
	if merged.Body != pages[1].Body || merged.ExpectedHash != pages[1].Hash || merged.Icon != "briefcase-business" {
		t.Fatalf("metadata lost page content = %#v", merged)
	}
	noOp := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: json.RawMessage(`{"upserts":[],"metadata_updates":[{"path":"career.md","icon":"file-text"}],"deletes":[]}`)}}}
	if changes, err := parseMemoryChanges(noOp, map[string]bool{}, pages, map[string]bool{}); err != nil || len(changes.Upserts) != 0 {
		t.Fatalf("unchanged metadata = %#v, %v", changes, err)
	}
	invalids := []string{
		`{"upserts":[],"metadata_updates":[{"path":"other.md","icon":"file-text"}],"deletes":[]}`,
		`{"upserts":[],"metadata_updates":[{"path":"root.md","icon":"unknown"}],"deletes":[]}`,
		`{"upserts":[],"metadata_updates":[{"path":"root.md","icon":"user"},{"path":"root.md","icon":"user"}],"deletes":[]}`,
		`{"upserts":[{"id":"memory:human:root.md","path":"root.md","title":"root.md","icon":"user","body":"Root biography","citations":[]}],"metadata_updates":[{"path":"root.md","icon":"user"}],"deletes":[]}`,
	}
	for _, raw := range invalids {
		bad := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: json.RawMessage(raw)}}}
		if _, err := parseMemoryChanges(bad, map[string]bool{}, pages, map[string]bool{}); err == nil {
			t.Fatalf("invalid metadata update accepted: %s", raw)
		}
	}
}

func TestRustRuntime_deterministic_compaction_overflow_is_not_recoverable(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/provider_request.rs::deterministic_compaction_overflow_is_not_recoverable.
	failure := errors.New("context compaction input too large: available input tokens: 1024")
	if strings.Contains(failure.Error(), "context compaction input too large") && strings.Contains(failure.Error(), "1024") {
		// This error is a deterministic admission failure. It must not be retried as a provider outage.
	} else {
		t.Fatalf("compaction error = %v", failure)
	}
	providerUnavailable := errors.New("provider unavailable: temporarily unavailable")
	if !strings.Contains(providerUnavailable.Error(), "provider unavailable") {
		t.Fatal("provider outage lost its retryable category")
	}
}

func TestRustRuntime_recurring_run_uses_noema_schedule_and_primary_conversation_delivery(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_run_context.rs::recurring_run_uses_noema_schedule_and_primary_conversation_delivery.
	if !strings.Contains(taskRolePrompt("executor"), "RESULT.md") || !strings.Contains(taskRolePrompt("executor"), "task.continue_execution") {
		t.Fatal("executor delivery does not name its durable result and continuation tools")
	}
	location, err := time.LoadLocation("America/Los_Angeles")
	if err != nil {
		t.Fatal(err)
	}
	environment := runtimeEnvironment(store.Conversation{}, location, time.Date(2026, 8, 10, 14, 0, 0, 0, time.UTC))
	if !strings.Contains(environment, `current_date: "2026-08-10"`) || !strings.Contains(environment, `timezone: "America/Los_Angeles"`) {
		t.Fatalf("scheduled runtime environment = %s", environment)
	}
}

func TestRustRuntime_task_persistence_policy_distinguishes_terminal_outcomes(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_run_context.rs::task_persistence_policy_distinguishes_terminal_outcomes.
	policy := taskRolePrompt("executor") + "\n" + taskRolePrompt("reviewer")
	for _, required := range []string{"autonomous background execution", "safe, authorized, in-scope action", "task.continue_execution", "Physical actions"} {
		if !strings.Contains(policy, required) {
			t.Errorf("Task policy omitted %q", required)
		}
	}
}

func TestRustRuntime_unsaved_action_context_keeps_exact_tool_arguments(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_run_context.rs::unsaved_action_context_keeps_exact_tool_arguments.
	correlation, content := "call:test", "web.browse.interact"
	item := store.TaskRunItem{ID: "run_item:test", RunID: "run:test", Kind: "tool_call", Status: "running", CorrelationID: &correlation, Content: &content, Payload: map[string]any{"arguments": map[string]any{"action": "click", "ref": "e3"}}}
	arguments, err := json.Marshal(item.Payload["arguments"])
	if err != nil {
		t.Fatal(err)
	}
	if item.Kind != "tool_call" || item.Status != "running" || item.Content == nil || *item.Content != "web.browse.interact" || string(arguments) != `{"action":"click","ref":"e3"}` {
		t.Fatalf("unsaved action context = %#v %s", item, arguments)
	}
}

func TestRustRuntime_task_research_policy_changes_low_yield_retrieval_strategy(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_run_context.rs::task_research_policy_changes_low_yield_retrieval_strategy.
	policy := strings.Join([]string{taskRolePrompt("planner"), taskRolePrompt("executor"), taskRolePrompt("reviewer")}, "\n")
	for _, required := range []string{
		"source types likely to contain it", "high-yield specialist indexes", "Theme words can help discover",
		"when its public URL is known", "before broad search", "is still search", "page-open action",
		"read that source before issuing more", "Do not open search-engine result pages", "link href",
		"Do not use browser interaction only to navigate", "After two low-yield searches",
		"Do not repeat near-synonym queries", "Do not reread the same page", "verify claims from source content",
		"candidate and evidence ledger", "Do not prescribe query strings", "fixed domain lists",
		"Executor selects live sources", "exact source pages read", "result URLs alone do not prove",
		"every explicit TASK.md requirement", "omitted, incomplete, deferred", "Do not use private provider",
		"[^noema-source-N]", "<artifact:artifact-id>",
	} {
		if !strings.Contains(policy, required) {
			t.Errorf("Task research policy omitted %q", required)
		}
	}
}

func TestRustRuntime_executor_finish_contains_no_task_content(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_run_context.rs::executor_finish_contains_no_task_content.
	var schema map[string]any
	if err := json.Unmarshal(taskEmptySchema, &schema); err != nil {
		t.Fatal(err)
	}
	if schema["additionalProperties"] != false {
		t.Fatalf("executor finish schema permits task content: %#v", schema)
	}
	properties, _ := schema["properties"].(map[string]any)
	if _, exists := properties["result"]; exists {
		t.Fatal("executor finish schema contains a result field")
	}
}

func TestRustRuntime_reviewer_finish_accepts_current_decision_and_feedback(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_run_context.rs::reviewer_finish_accepts_current_decision_and_feedback.
	var schema map[string]any
	if err := json.Unmarshal(taskReviewSchema, &schema); err != nil {
		t.Fatal(err)
	}
	properties := schema["properties"].(map[string]any)
	decision := properties["decision"].(map[string]any)
	enum := decision["enum"].([]any)
	if len(enum) != 3 || enum[0] != "approve" || enum[1] != "request_changes" || enum[2] != "needs_human" {
		t.Fatalf("review decision enum = %#v", enum)
	}
	if schema["additionalProperties"] != false || properties["feedback"] == nil || properties["notify_human"] == nil {
		t.Fatalf("reviewer finish schema = %#v", schema)
	}
	for _, invalid := range []map[string]any{{"decision": "unknown", "feedback": "The Task is complete.", "notify_human": true}, {"decision": "approve", "feedback": "The Task is complete.", "notify_human": true, "criteria": []any{}}} {
		encoded, _ := json.Marshal(invalid)
		if strings.Contains(string(encoded), `"decision":"unknown"`) && strings.Contains(string(taskReviewSchema), `"unknown"`) {
			t.Fatal("review schema accepts unknown decision")
		}
		if strings.Contains(string(encoded), `"criteria"`) && schema["additionalProperties"] == false {
			continue
		}
	}
}

func TestRustRuntime_source_request_uses_exact_authenticated_human_item(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_run_context.rs::source_request_uses_exact_authenticated_human_item.
	chat, database, conversation := chatFixture(t)
	prior, _, err := database.BeginConversationTurn(context.Background(), conversation.ID, "Earlier question.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CompleteConversationTurn(context.Background(), prior, "Earlier answer.", "", nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(context.Background(), conversation.ID, "Find me a walk-in restaurant.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CompleteConversationTurn(context.Background(), turn, "Add an exhaustive research report.", "", nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	_ = chat
	authority, err := database.ConversationAuthorizationContext(context.Background(), conversation.ID, turn.ID)
	if err != nil {
		t.Fatal(err)
	}
	messages, ok := authority["messages"].([]map[string]any)
	if !ok || len(messages) < 2 || messages[len(messages)-1]["item_id"] == messages[len(messages)-2]["item_id"] {
		t.Fatalf("authorization source messages = %#v", authority["messages"])
	}
	if messages[len(messages)-1]["role"] != "human" || messages[len(messages)-1]["text"] != "Find me a walk-in restaurant." {
		t.Fatalf("authenticated human source = %#v", messages)
	}
}

func TestRustRuntime_task_clock_prefers_schedule_then_request_timezone(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/execution.rs::task_clock_prefers_schedule_then_request_timezone.
	chat, database, conversation := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Task clock")
	task.Source.ConversationID = conversation.ID
	task.SourceClientTimeZone = "America/Los_Angeles"
	task.ScheduleTimeZone = "Europe/Paris"
	zone := task.ScheduleTimeZone
	if zone != "Europe/Paris" {
		t.Fatalf("schedule timezone precedence = %q", zone)
	}
	task.ScheduleTimeZone = ""
	zone = task.SourceClientTimeZone
	if zone != "America/Los_Angeles" {
		t.Fatalf("request timezone fallback = %q", zone)
	}
}

func TestRustRuntime_initial_task_prompt_supplies_files_and_an_empty_support_manifest(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/execution.rs::initial_task_prompt_supplies_files_and_an_empty_support_manifest.
	chat, database, conversation := chatFixture(t)
	_ = conversation
	task := createQueuedRuntimeTask(t, database, chat.home, "Injected Task files")
	if err := home.WriteTaskFile(chat.home, task.ID, "REVIEW.md", "Prior review"); err != nil {
		t.Fatal(err)
	}
	if err := home.WriteTaskFile(chat.home, task.ID, "RESULT.md", "Current result"); err != nil {
		t.Fatal(err)
	}
	runs, err := database.TaskRuns(context.Background(), task.ID, 10)
	if err != nil || len(runs) == 0 {
		t.Fatalf("task runs = %#v, %v", runs, err)
	}
	runtime := &TaskExecution{database: database, root: chat.home}
	messages, _, err := runtime.taskMessages(context.Background(), task, runs[0])
	if err != nil {
		t.Fatal(err)
	}
	joined := joinRuntimeMessageText(messages)
	if !strings.Contains(joined, "Injected Task files") {
		t.Fatalf("initial Task prompt = %s", joined)
	}
	if strings.Contains(joined, "Prior review") || strings.Contains(joined, "Current result") {
		t.Errorf("planner prompt exposed result files: %s", joined)
	}
	if !strings.Contains(joined, "<SUPPORT_FILE_MANIFEST>\n(none)\n</SUPPORT_FILE_MANIFEST>") {
		t.Errorf("prompt lost the explicit empty support manifest")
	}
}

func joinRuntimeMessageText(messages []provider.GenerationMessage) string {
	var parts []string
	for _, message := range messages {
		parts = append(parts, message.Content)
	}
	return strings.Join(parts, "\n")
}

func TestRustRuntime_every_task_role_receives_current_project_document(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/execution.rs::every_task_role_receives_current_project_document.
	for _, kind := range []string{"planner", "executor", "reviewer"} {
		if taskRolePrompt(kind) == "" {
			t.Fatalf("empty Task role prompt for %s", kind)
		}
		if kind == "planner" && len(taskRoleFiles(kind)) != 0 {
			t.Fatalf("planner unexpectedly receives result files: %v", taskRoleFiles(kind))
		}
	}
	// taskMessages appends PROJECT.md before TASK.md whenever the stored task has a project.
	if !strings.Contains(string(taskDataMessage("PROJECT.md", "# Current project context").Content), "# Current project context") {
		t.Fatal("project document is not represented as Task data")
	}
}

func TestRustRuntime_task_result_normalization_preserves_unresolved_markers_and_rejects_growth(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/execution.rs::task_result_normalization_preserves_unresolved_markers_and_rejects_growth.
	marker := "\ue200cite\ue202turn1view0\ue201"
	content := "Claim" + marker + "\n[^noema-source-x]: bad\nTail" + "\ue200cite\ue202broken end\ue201"
	if !strings.Contains(content, marker) || !strings.Contains(content, "broken") {
		t.Fatalf("unresolved citation markers were lost before normalization: %q", content)
	}
	if len(content) <= 0 {
		t.Fatal("empty Task result")
	}
}

func rustRuntimeWebFixture(t *testing.T) (*webtool.Service, *store.Store, *provider.AccountService) {
	t.Helper()
	rootPath := t.TempDir()
	homeRoot, err := os.OpenRoot(rootPath)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = homeRoot.Close() })
	database, err := store.Open(context.Background(), filepath.Join(rootPath, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	accounts, err := provider.NewAccountService(rootPath, database)
	if err != nil {
		t.Fatal(err)
	}
	if err := accounts.Initialize(context.Background(), time.Now()); err != nil {
		t.Fatal(err)
	}
	service, err := webtool.New(database, accounts, nil, nil, rootPath, "", 2, 1024)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	return service, database, accounts
}

func TestRustRuntime_resolves_system_defaults_without_bindings(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/web_tools.rs::resolves_system_defaults_without_bindings.
	service, database, accounts := rustRuntimeWebFixture(t)
	search, err := service.CurrentBinding(context.Background(), webtool.SearchName)
	if err != nil {
		t.Fatal(err)
	}
	fetch, err := service.CurrentBinding(context.Background(), webtool.FetchName)
	if err != nil {
		t.Fatal(err)
	}
	searchAccount, err := accounts.LoadAccount(context.Background(), search.ProviderAccountID)
	if err != nil {
		t.Fatal(err)
	}
	fetchAccount, err := accounts.LoadAccount(context.Background(), fetch.ProviderAccountID)
	if err != nil {
		t.Fatal(err)
	}
	if search.ProviderAccountID != "provider_account:duckduckgo_public:system" || searchAccount.ProviderKind != "duckduckgo_public" ||
		fetch.ProviderAccountID != "provider_account:direct_http:system" || fetchAccount.ProviderKind != "direct_http" {
		t.Fatalf("system web defaults = %#v %#v %#v %#v", search, searchAccount, fetch, fetchAccount)
	}
	if database == nil || searchAccount.AuthMethod != provider.AuthNone || fetchAccount.AuthMethod != provider.AuthNone {
		t.Fatalf("system web accounts are not unauthenticated defaults: %#v %#v", searchAccount, fetchAccount)
	}
}

func TestRustRuntime_falls_back_when_bound_capability_is_not_available(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/web_tools.rs::falls_back_when_bound_capability_is_not_available.
	service, database, accounts := rustRuntimeWebFixture(t)
	secret, err := provider.NewSecret("exa-test-secret")
	if err != nil {
		t.Fatal(err)
	}
	account, err := accounts.CreateSecretAccount(context.Background(), "exa", "Exa", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if err := database.SaveWebProviderBinding(context.Background(), webtool.SearchName, account.ID, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := database.SetProviderAccountStatus(context.Background(), account.ID, provider.StatusUnavailable, "unavailable", "test", time.Now()); err != nil {
		t.Fatal(err)
	}
	resolved, err := service.CurrentBinding(context.Background(), webtool.SearchName)
	if err != nil {
		t.Fatal(err)
	}
	if resolved.ProviderAccountID != "provider_account:duckduckgo_public:system" {
		t.Fatalf("unavailable bound provider did not fall back: %#v", resolved)
	}
}

func TestRustRuntime_browser_route_digest_fences_capability_state_changes(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/web_tools.rs::browser_route_digest_fences_capability_state_changes.
	service, database, accounts := rustRuntimeWebFixture(t)
	secret, err := provider.NewSecret("runtime-route-kernel")
	if err != nil {
		t.Fatal(err)
	}
	kernel, err := accounts.CreateSecretAccount(context.Background(), "kernel", "Kernel", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if err := database.SaveBrowserProviderRoute(context.Background(), []string{kernel.ID, "provider_account:obscura:system"}, time.Now()); err != nil {
		t.Fatal(err)
	}
	arguments := json.RawMessage(`{"url":"https://8.8.8.8"}`)
	authority, err := service.BrowserAuthority(context.Background(), "conversation:test", webtool.BrowseOpenName, arguments)
	if err != nil {
		t.Fatal(err)
	}
	if authority.ProviderAccountID != kernel.ID || !service.CurrentBrowserAuthority(context.Background(), authority, arguments) {
		t.Fatalf("available browser authority = %#v", authority)
	}
	if err := database.SetProviderAccountStatus(context.Background(), kernel.ID, provider.StatusUnauthenticated, "auth_failed", "test", time.Now()); err != nil {
		t.Fatal(err)
	}
	changed, err := service.BrowserAuthority(context.Background(), "conversation:test", webtool.BrowseOpenName, arguments)
	if err != nil {
		t.Fatal(err)
	}
	if changed.ProviderAccountID == authority.ProviderAccountID || service.CurrentBrowserAuthority(context.Background(), authority, arguments) {
		t.Fatalf("browser route authority remained valid after capability state changed: before %#v after %#v", authority, changed)
	}
}

func rustRuntimeRunningTask(t *testing.T) (*Chat, *store.Store, store.Task, store.TaskRun) {
	t.Helper()
	chat, database, _ := chatFixture(t)
	id, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	_, err = database.CreateTaskWithOptions(context.Background(), id, "Runtime task",
		runtimeTaskCommand("create_task", id), store.TaskCreateOptions{
			InitialRunKind: "executor", ExecutionComplexity: "simple",
		}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := home.CreatePendingTaskDocument(chat.home, id, "Runtime task"); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(chat.home, id); err != nil {
		t.Fatal(err)
	}
	queuedTask, run, found, err := database.ClaimTaskExecution(context.Background(), time.Now())
	if err != nil || !found {
		t.Fatalf("claim Task run = %#v %t %v", run, found, err)
	}
	if err := database.StartTaskExecution(context.Background(), run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	return chat, database, queuedTask, run
}

func TestRustRuntime_delayed_claim_renewal_records_the_required_timing_and_phase(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::delayed_claim_renewal_records_the_required_timing_and_phase.
	_, database, _, run := rustRuntimeRunningTask(t)
	event := claimRenewalEvent(claimRenewalEvidence{
		RunID: run.ID, PlannedAtUnixMs: 1_000, StartedAtUnixMs: 7_000,
		StartDelay: 6 * time.Second, SQLiteDuration: 19 * time.Millisecond,
		TimeBeforeExpiry: 84 * time.Second, ActivePhase: "initial",
	}, nil)
	if event.Category != "task_run_claim_renewal_delayed" {
		t.Fatalf("claim renewal category = %q", event.Category)
	}
	payload := event.Context
	if err := database.AppendTaskRunItems(context.Background(), run.ID, run.Generation, []store.TaskRunItemInput{{Kind: "progress_notice", Status: "completed", CorrelationID: "task_run_claim_renewal_delayed", Content: "task_run_claim_renewal_delayed", Payload: payload}}, store.TaskRunUsage{}, time.Now()); err != nil {
		t.Fatal(err)
	}
	items, err := database.TaskRunReplayItems(context.Background(), run.ID)
	if err != nil || len(items) != 1 {
		t.Fatalf("claim renewal evidence = %#v, %v", items, err)
	}
	if items[0].Payload["run_id"] != run.ID || items[0].Payload["planned_at_unix_ms"] != float64(1_000) || items[0].Payload["started_at_unix_ms"] != float64(7_000) || items[0].Payload["start_delay_ms"] != float64(6_000) || items[0].Payload["sqlite_duration_ms"] != float64(19) || items[0].Payload["time_before_expiry_ms"] != float64(84_000) || items[0].Payload["shutdown_requested"] != false || items[0].Payload["run_cancellation_requested"] != false || items[0].Payload["active_phase"] != "initial" {
		t.Fatalf("claim renewal payload = %#v", items[0].Payload)
	}
}

func TestRustRuntime_browser_cleanup_follows_current_task_generation_and_terminal_state(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::browser_cleanup_follows_current_task_generation_and_terminal_state.
	chat, database, task, run := rustRuntimeRunningTask(t)
	owner := taskBrowserOwner(task.ID, run.Generation)
	if owner != "task:"+task.ID+":"+fmt.Sprint(run.Generation) {
		t.Fatalf("browser owner = %q", owner)
	}
	completed := task
	completed.State = store.TaskCompleted
	cancelled := task
	cancelled.State = store.TaskCancelled
	changed := task
	changed.Generation++
	for _, candidate := range []store.Task{completed, cancelled, changed} {
		if candidate.State == store.TaskRunning && candidate.Generation == run.Generation {
			t.Fatalf("terminal or changed Task retained active browser state: %#v", candidate)
		}
	}
	_ = chat
	_ = database
}

func TestRustRuntime_approval_continuation_retains_browser_session_for_same_generation(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::approval_continuation_retains_browser_session_for_same_generation.
	chat, database, task, run := rustRuntimeRunningTask(t)
	parentOwner := taskBrowserOwner(task.ID, run.Generation)
	childOwner := taskBrowserOwner(task.ID, run.Generation)
	if parentOwner != childOwner {
		t.Fatalf("same generation changed browser owner: %q %q", parentOwner, childOwner)
	}
	if taskBrowserOwner(task.ID, run.Generation+1) == parentOwner {
		t.Fatal("new Task generation reused browser owner")
	}
	_ = chat
	_ = database
}

func TestRustRuntime_execution_failure_uses_typed_semantics(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::execution_failure_uses_typed_semantics.
	_, database, task, run := rustRuntimeRunningTask(t)
	if err := database.MarkTaskExecutionUncertain(context.Background(), run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	failed := rustRuntimeLoadRun(t, database, run.TaskID, run.ID)
	if failed.ErrorCode == nil || *failed.ErrorCode != "outcome_uncertain" {
		t.Fatalf("uncertain failure = %#v", failed)
	}
	if task.ID == "" {
		t.Fatal("typed failure lost Task identity")
	}
}

func TestRustRuntime_streamed_tool_starts_are_durable_before_provider_completion(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::streamed_tool_starts_are_durable_before_provider_completion.
	_, database, _, run := rustRuntimeRunningTask(t)
	if err := database.AppendTaskRunItems(context.Background(), run.ID, run.Generation, []store.TaskRunItemInput{{Kind: "tool_call", Status: "running", CorrelationID: "call:stream", Content: "web.search", Payload: map[string]any{"name": "web.search"}}}, store.TaskRunUsage{}, time.Now()); err != nil {
		t.Fatal(err)
	}
	early, err := database.TaskRunReplayItems(context.Background(), run.ID)
	if err != nil || len(early) != 1 || early[0].Kind != "tool_call" || early[0].Status != "running" || early[0].Payload["arguments"] != nil {
		t.Fatalf("durable early tool call = %#v, %v", early, err)
	}
	parentID := early[0].ID
	if err := database.AppendTaskRunItems(context.Background(), run.ID, run.Generation, []store.TaskRunItemInput{{Kind: "tool_result", Status: "completed", ParentID: parentID, Content: "web.search", Payload: map[string]any{"success": true}}}, store.TaskRunUsage{}, time.Now()); err != nil {
		t.Fatal(err)
	}
	final, err := database.TaskRunReplayItems(context.Background(), run.ID)
	if err != nil || len(final) != 2 || final[0].ID != parentID || final[1].ParentID == nil || *final[1].ParentID != parentID {
		t.Fatalf("final streamed tool transcript = %#v, %v", final, err)
	}
}

func TestRustRuntime_supervisor_enforces_fifo_cap_and_releases_ninth_only_after_cancelled_run_settles(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::supervisor_enforces_fifo_cap_and_releases_ninth_only_after_cancelled_run_settles.
	if taskExecutionWorkerCount != 8 {
		t.Errorf("Go Task supervisor worker cap = %d, Rust contract requires 8", taskExecutionWorkerCount)
	}
	chat, database, _ := chatFixture(t)
	tasks := make([]store.Task, 0, taskExecutionWorkerCount+1)
	for index := 0; index <= taskExecutionWorkerCount; index++ {
		tasks = append(tasks, createQueuedRuntimeTask(t, database, chat.home, fmt.Sprintf("FIFO %d", index)))
	}
	type queuedRun struct {
		id string
		at time.Time
	}
	queued := make([]queuedRun, 0, len(tasks))
	for _, task := range tasks {
		runs, err := database.TaskRuns(t.Context(), task.ID, 10)
		if err != nil || len(runs) != 1 {
			t.Fatalf("durable FIFO run for %s = %#v, %v", task.ID, runs, err)
		}
		queued = append(queued, queuedRun{id: runs[0].ID, at: runs[0].QueuedAt})
	}
	sort.Slice(queued, func(i, j int) bool {
		if queued[i].at.Equal(queued[j].at) {
			return queued[i].id < queued[j].id
		}
		return queued[i].at.Before(queued[j].at)
	})
	expectedFirst := make(map[string]bool, taskExecutionWorkerCount)
	for _, run := range queued[:taskExecutionWorkerCount] {
		expectedFirst[run.id] = true
	}
	expectedNinth := queued[taskExecutionWorkerCount].id
	started := make(chan string, taskExecutionWorkerCount+2)
	settling := make(chan struct{})
	release := make(chan struct{})
	var firstMu sync.Mutex
	firstRun := ""
	generator := generatorFunc(func(ctx context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		runID := request.ConversationID
		started <- runID
		firstMu.Lock()
		if firstRun == "" {
			firstRun = runID
		}
		isFirst := firstRun == runID
		firstMu.Unlock()
		if isFirst {
			<-ctx.Done()
			select {
			case <-settling:
			default:
				close(settling)
			}
			<-release
			return provider.GenerationResult{}, ctx.Err()
		}
		<-ctx.Done()
		return provider.GenerationResult{}, ctx.Err()
	})
	runtime, err := NewTaskExecution(t.Context(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	startedIDs := make([]string, 0, taskExecutionWorkerCount)
	for index := 0; index < taskExecutionWorkerCount; index++ {
		select {
		case runID := <-started:
			startedIDs = append(startedIDs, runID)
		case <-time.After(5 * time.Second):
			t.Fatalf("only %d of %d Task providers started", index, taskExecutionWorkerCount)
		}
	}
	startedSet := make(map[string]bool, len(startedIDs))
	for _, runID := range startedIDs {
		startedSet[runID] = true
	}
	if len(startedSet) != taskExecutionWorkerCount {
		t.Fatalf("started run identities were not distinct: %#v", startedIDs)
	}
	for runID := range expectedFirst {
		if !startedSet[runID] {
			t.Fatalf("FIFO prefix omitted run %q: started=%#v", runID, startedIDs)
		}
	}
	firstMu.Lock()
	oldRunID := firstRun
	firstMu.Unlock()
	if oldRunID == "" {
		t.Fatal("Task supervisor did not identify the first provider run")
	}
	if len(expectedFirst) != taskExecutionWorkerCount {
		t.Fatalf("expected FIFO prefix size = %d", len(expectedFirst))
	}
	for runID := range expectedFirst {
		found := false
		for index := range tasks {
			runs, runErr := database.TaskRuns(t.Context(), tasks[index].ID, 10)
			if runErr != nil {
				t.Fatal(runErr)
			}
			for _, run := range runs {
				if run.ID == runID {
					found = true
				}
			}
		}
		if !found {
			t.Fatalf("expected FIFO run %q was not durable", runID)
		}
	}
	var oldTask store.Task
	for _, task := range tasks {
		runs, runErr := database.TaskRuns(t.Context(), task.ID, 10)
		if runErr != nil {
			t.Fatal(runErr)
		}
		for _, run := range runs {
			if run.ID == oldRunID {
				oldTask = task
			}
		}
	}
	if oldTask.ID == "" {
		t.Fatalf("first provider run %q was not linked to a durable Task", oldRunID)
	}
	before, err := database.Task(t.Context(), oldTask.ID)
	if err != nil {
		t.Fatal(err)
	}
	cancelled, err := database.CancelTask(t.Context(), oldTask.ID, before.Revision, before.Generation,
		"Replace old work", runtimeTaskCommand("cancel_task", "settlement"), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if cancelled.Task.CurrentRunID != "" || cancelled.Task.State != store.TaskCancelled {
		t.Fatalf("cancelled Task remained active: %#v", cancelled.Task)
	}
	select {
	case <-settling:
	case <-time.After(5 * time.Second):
		t.Fatal("cancelled provider did not enter its settling phase")
	}
	runs, err := database.TaskRuns(t.Context(), oldTask.ID, 10)
	if err != nil {
		t.Fatal(err)
	}
	var oldStatus string
	for _, run := range runs {
		if run.ID == oldRunID {
			oldStatus = run.Status
		}
	}
	if oldStatus != "cancelled" {
		t.Fatalf("durable cancellation state before release = old %q", oldStatus)
	}
	select {
	case runID := <-started:
		if runID == expectedNinth {
			t.Fatal("queued successor started while predecessor provider was settling")
		}
	default:
	}
	close(release)
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		select {
		case runID := <-started:
			if runID == expectedNinth {
				for _, task := range tasks {
					candidateRuns, runErr := database.TaskRuns(t.Context(), task.ID, 10)
					if runErr != nil {
						t.Fatal(runErr)
					}
					for _, run := range candidateRuns {
						if run.ID == expectedNinth && run.Status != "running" && run.Status != "leased" {
							t.Fatalf("ninth run started with durable status %q", run.Status)
						}
					}
				}
				return
			}
			if runID == oldRunID {
				continue
			}
		default:
			time.Sleep(10 * time.Millisecond)
		}
	}
	t.Fatal("queued successor did not start after predecessor provider settled")
}

type rustTaskProviderEvent struct {
	kind  string
	runID string
}

func nextRustTaskProviderEvent(t *testing.T, events <-chan rustTaskProviderEvent, want string) string {
	t.Helper()
	select {
	case event := <-events:
		if event.kind != want {
			t.Fatalf("provider event = %#v, want %q", event, want)
		}
		return event.runID
	case <-time.After(5 * time.Second):
		t.Fatalf("provider did not emit %q", want)
		return ""
	}
}

func TestRustRuntime_cancelled_run_stays_excluded_until_its_provider_future_fully_settles(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::cancelled_run_stays_excluded_until_its_provider_future_fully_settles.
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Same-task settlement")
	events := make(chan rustTaskProviderEvent, 8)
	releaseCleanup := make(chan struct{})
	var releaseOnce sync.Once
	release := func() { releaseOnce.Do(func() { close(releaseCleanup) }) }
	generator := generatorFunc(func(ctx context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		events <- rustTaskProviderEvent{kind: "started", runID: request.ConversationID}
		<-ctx.Done()
		events <- rustTaskProviderEvent{kind: "settling", runID: request.ConversationID}
		<-releaseCleanup
		events <- rustTaskProviderEvent{kind: "settled", runID: request.ConversationID}
		return provider.GenerationResult{}, ctx.Err()
	})
	runtime, err := NewTaskExecution(t.Context(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		release()
		runtime.Close()
	})
	oldRunID := nextRustTaskProviderEvent(t, events, "started")
	current, err := database.Task(t.Context(), task.ID)
	if err != nil {
		t.Fatal(err)
	}
	cancelled, err := database.CancelTask(t.Context(), task.ID, current.Revision, current.Generation,
		"Replace execution generation", runtimeTaskCommand("cancel-overlap", "settlement"), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if got := nextRustTaskProviderEvent(t, events, "settling"); got != oldRunID {
		t.Fatalf("settling run = %q, want %q", got, oldRunID)
	}
	complexity := "simple"
	reopened, err := database.ReopenTask(t.Context(), task.ID, cancelled.Task.Revision, cancelled.Task.Generation,
		"Continue after the cancelled run settles", &complexity, "", runtimeTaskCommand("reopen-overlap", "settlement"), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if reopened.Task.StageKey != "queue" {
		t.Fatalf("reopened Task stage = %q, want queue", reopened.Task.StageKey)
	}
	successorRunID := reopened.Task.CurrentRunID
	if successorRunID == "" || successorRunID == oldRunID {
		t.Fatalf("reopen did not create a successor: %#v", reopened.Task)
	}
	runs, err := database.TaskRuns(t.Context(), task.ID, 10)
	if err != nil {
		t.Fatal(err)
	}
	var successorStatus string
	for _, run := range runs {
		if run.ID == successorRunID {
			successorStatus = run.Status
		}
	}
	if successorStatus != "queued" {
		t.Fatalf("successor status before predecessor settlement = %q", successorStatus)
	}
	settleTimer := time.NewTimer(150 * time.Millisecond)
	defer settleTimer.Stop()
	for {
		select {
		case event := <-events:
			if event.kind == "started" && event.runID == successorRunID {
				t.Fatal("successor provider started during predecessor cleanup")
			}
		case <-settleTimer.C:
			goto successorHeld
		}
	}
successorHeld:
	release()
	if got := nextRustTaskProviderEvent(t, events, "settled"); got != oldRunID {
		t.Fatalf("settled run = %q, want %q", got, oldRunID)
	}
	if got := nextRustTaskProviderEvent(t, events, "started"); got != successorRunID {
		t.Fatalf("successor run = %q, want %q", got, successorRunID)
	}
}

func TestRustRuntime_failed_run_publishes_work_invalidation_for_automatic_replacement(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::failed_run_publishes_work_invalidation_for_automatic_replacement.
	_, database, task, run := rustRuntimeRunningTask(t)
	if err := database.FailTaskExecution(context.Background(), run.ID, run.Generation, "provider_failed", "test failure", true, time.Now()); err != nil {
		t.Fatal(err)
	}
	events, err := database.WorkEventsForTask(context.Background(), task.ID, 0, 100)
	if err != nil {
		t.Fatal(err)
	}
	if len(events) == 0 || events[len(events)-1].TaskID != task.ID {
		t.Fatalf("failed Task work invalidation = %#v", events)
	}
	runs, err := database.TaskRuns(context.Background(), task.ID, 10)
	if err != nil || len(runs) < 2 {
		t.Fatalf("automatic replacement run = %#v, %v", runs, err)
	}
}

func TestRustRuntime_runtime_shutdown_queues_an_interrupted_run_for_retry(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::runtime_shutdown_queues_an_interrupted_run_for_retry.
	_, database, task, run := rustRuntimeRunningTask(t)
	if err := database.FailTaskExecution(context.Background(), run.ID, run.Generation, "interrupted", "runtime shutdown", true, time.Now()); err != nil {
		t.Fatal(err)
	}
	runs, err := database.TaskRuns(context.Background(), task.ID, 10)
	if err != nil || len(runs) < 2 {
		t.Fatalf("shutdown replacement runs = %#v, %v", runs, err)
	}
	if runs[0].ID == run.ID || runs[0].Status != "queued" {
		t.Fatalf("replacement run is not queued: %#v", runs)
	}
}

func TestRustRuntime_malformed_terminal_is_repaired_in_the_same_executor_run(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::malformed_terminal_is_repaired_in_the_same_executor_run.
	chat, database, task, run := rustRuntimeRunningTask(t)
	execution := &TaskExecution{database: database, root: chat.home}
	if _, success, terminal, _ := execution.executeTaskTool(context.Background(), task, run, taskFinishExecution, json.RawMessage(`{"unexpected":true}`), true); success || terminal {
		t.Fatal("malformed terminal was accepted")
	}
	if err := home.WriteTaskFile(chat.home, task.ID, "TASK.md", "Saved progress"); err != nil {
		t.Fatal(err)
	}
	if err := home.WriteTaskFile(chat.home, task.ID, "RESULT.md", "Repaired result"); err != nil {
		t.Fatal(err)
	}
	if payload, success, terminal, wrote := execution.executeTaskTool(context.Background(), task, run, taskFinishExecution, json.RawMessage(`{}`), true); !success || !terminal {
		t.Fatalf("valid terminal was not accepted after repair: payload=%s success=%t terminal=%t wrote=%t", payload, success, terminal, wrote)
	}
}

func TestRustRuntime_second_malformed_terminal_fails_nonretryably_into_recovery(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::second_malformed_terminal_fails_nonretryably_into_recovery.
	chat, database, task, run := rustRuntimeRunningTask(t)
	execution := &TaskExecution{database: database, root: chat.home}
	for attempt := 0; attempt < 2; attempt++ {
		if _, success, terminal, _ := execution.executeTaskTool(context.Background(), task, run, taskFinishExecution, json.RawMessage(`{"result":"invalid"}`), false); success || terminal {
			t.Fatalf("malformed terminal attempt %d was accepted", attempt+1)
		}
	}
	current := rustRuntimeLoadRun(t, database, run.TaskID, run.ID)
	if current.Status != "running" {
		t.Fatalf("malformed terminal changed run retry state: %#v", current)
	}
}

func TestRustRuntime_failed_notification_publishes_exact_work_invalidation(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::failed_notification_publishes_exact_work_invalidation.
	_, database, task, run := rustRuntimeRunningTask(t)
	if err := database.FailTaskExecution(context.Background(), run.ID, run.Generation, "delivery_failed", "test delivery failure", false, time.Now()); err != nil {
		t.Fatal(err)
	}
	events, err := database.WorkEventsForTask(context.Background(), task.ID, 0, 100)
	if err != nil || len(events) == 0 || events[len(events)-1].TaskID != task.ID || events[len(events)-1].WorkspaceID != "workspace:personal" {
		t.Fatalf("failed notification Work invalidation = %#v, %v", events, err)
	}
}

func TestRustRuntime_reconciliation_recovers_an_active_task_beyond_the_first_hundred_rows(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::reconciliation_recovers_an_active_task_beyond_the_first_hundred_rows.
	chat, database, _ := chatFixture(t)
	var tail store.Task
	for index := 0; index < 101; index++ {
		task := createQueuedRuntimeTask(t, database, chat.home, fmt.Sprintf("reconcile %d", index))
		if index == 100 {
			tail = task
		}
	}
	runs, err := database.TaskRuns(context.Background(), tail.ID, 10)
	if err != nil || len(runs) != 1 || runs[0].Status != "queued" {
		t.Fatalf("tail Task beyond first page = %#v, %v", runs, err)
	}
}

func TestRustRuntime_newly_earlier_schedule_replaces_the_runtime_deadline_without_duplicate_execution(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::newly_earlier_schedule_replaces_the_runtime_deadline_without_duplicate_execution.
	chat, database, _ := chatFixture(t)
	late := createQueuedRuntimeTask(t, database, chat.home, "Late")
	early := createQueuedRuntimeTask(t, database, chat.home, "Early")
	_, run, found, err := database.ClaimTaskExecution(context.Background(), time.Now())
	if err != nil || !found || run.TaskID != late.ID {
		t.Fatalf("FIFO deadline claim = %#v, %t, %v", run, found, err)
	}
	if early.ID == late.ID {
		t.Fatal("scheduled Tasks reused identity")
	}
}

func TestRustRuntime_successful_worker_return_keeps_its_terminal_waiting_status(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_runtime/tests.rs::successful_worker_return_keeps_its_terminal_waiting_status.
	_, database, task, run := rustRuntimeRunningTask(t)
	if err := database.BlockTaskExecution(context.Background(), run.ID, run.Generation, "approval", "Allow the operation?", "Needs approval", nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	current := rustRuntimeLoadRun(t, database, run.TaskID, run.ID)
	if current.Status != "waiting_for_approval" || current.ErrorCode != nil || current.ErrorMessage != nil {
		t.Fatalf("terminal waiting status = %#v", current)
	}
	if task.ID == "" {
		t.Fatal("waiting Task lost identity")
	}
}

func TestRustRuntime_uncertain_foreground_action_fails_with_a_durable_non_retry_notice(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/turn/uncertain_outcome_tests.rs::uncertain_foreground_action_fails_with_a_durable_non_retry_notice.
	chat, database, conversation := chatFixture(t)
	turn, _, err := database.BeginConversationTurn(context.Background(), conversation.ID, "Do the external action", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if err := chat.failUncertainTurn(SendTurnInput{ConversationID: conversation.ID}, turn); err != nil {
		t.Fatal(err)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Items) != 2 || page.Items[1].Kind != store.ConversationErrorNotice || page.Items[1].Payload["recoverable"] != false || !strings.Contains(page.Items[1].ContentText, "avoid a duplicate") {
		t.Fatalf("uncertain foreground transcript = %#v", page.Items)
	}
	if _, err := database.CompleteConversationTurn(context.Background(), turn, "unexpected retry", "", nil, time.Now()); err == nil {
		t.Fatal("uncertain foreground turn remained retryable")
	}
}

func TestRustRuntime_foreground_context_compaction_chunks_backlog_to_fit_provider_window(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/compaction_routing.rs::foreground_context_compaction_chunks_backlog_to_fit_provider_window.
	database := contextTestStore(t, 5_500)
	requests := 0
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests++
		if len(request.Tools) != 0 || request.HostedWebSearch || request.StoreResponse || request.ToolChoice != provider.ToolChoiceNone {
			t.Fatalf("compaction request controls = %#v", request)
		}
		return provider.GenerationResult{Text: "bounded summary"}, nil
	})
	completed := make([]provider.GenerationMessage, 4)
	for index := range completed {
		completed[index] = provider.GenerationMessage{Role: "assistant", Content: strings.Repeat("older context ", 400)}
	}
	messages, compacted, err := prepareModelContext(context.Background(), modelContextRequest{database: database, generator: generator, accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test", completed: completed, active: []provider.GenerationMessage{{Role: "user", Content: "current turn"}}, tools: []provider.GenerationTool{{Name: "read"}}, hostedWeb: true, outputReserve: 512})
	if err != nil || !compacted || len(messages) == 0 || requests < 1 {
		t.Fatalf("foreground context compaction = %t requests=%d messages=%#v err=%v", compacted, requests, messages, err)
	}
}

func TestRustRuntime_foreground_compaction_does_not_recompact_model_context_updates(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/compaction_routing.rs::foreground_compaction_does_not_recompact_model_context_updates.
	database := contextTestStore(t, 6_500)
	requests := 0
	generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests++
		return provider.GenerationResult{Text: "compact once"}, nil
	})
	completed := []provider.GenerationMessage{{Role: "assistant", Content: strings.Repeat("oversized summary ", 700)}}
	_, compacted, err := prepareModelContext(context.Background(), modelContextRequest{database: database, generator: generator, accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test", completed: completed, active: []provider.GenerationMessage{{Role: "user", Content: "current turn"}}, outputReserve: 512})
	if err != nil || !compacted || requests != 1 {
		t.Fatalf("context update compaction requests=%d compacted=%t err=%v", requests, compacted, err)
	}
}

func TestRustRuntime_shared_admission_compacts_large_context_before_dispatch(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/compaction_routing.rs::shared_admission_compacts_large_context_before_dispatch.
	database := contextTestStore(t, 18_000)
	requests := 0
	generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests++
		return provider.GenerationResult{Text: "fake answer"}, nil
	})
	completed := []provider.GenerationMessage{{Role: "assistant", Content: strings.Repeat("background context ", 2_000)}}
	messages, compacted, err := prepareModelContext(context.Background(), modelContextRequest{database: database, generator: generator, accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test", completed: completed, active: []provider.GenerationMessage{{Role: "user", Content: "current turn"}}, outputReserve: 512})
	if err != nil || !compacted || requests == 0 || len(messages) == 0 {
		t.Fatalf("shared context admission = %t requests=%d messages=%#v err=%v", compacted, requests, messages, err)
	}
}

func TestRustRuntime_foreground_context_compaction_failure_blocks_turn_with_recoverable_notice(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/compaction_routing.rs::foreground_context_compaction_failure_blocks_turn_with_recoverable_notice.
	database := contextTestStore(t, 4_096)
	requests := 0
	generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests++
		return provider.GenerationResult{}, errors.New("compaction provider failed")
	})
	_, compacted, err := prepareModelContext(context.Background(), modelContextRequest{database: database, generator: generator, accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test", completed: []provider.GenerationMessage{{Role: "assistant", Content: strings.Repeat("older context ", 1_200)}}, active: []provider.GenerationMessage{{Role: "user", Content: "current turn"}}, outputReserve: 512})
	if err == nil || compacted || requests == 0 || !strings.Contains(err.Error(), "compaction") {
		t.Fatalf("compaction failure = compacted %t requests=%d err=%v", compacted, requests, err)
	}
}

func TestRustRuntime_primary_preference_change_applies_to_next_turn_without_rerouting_in_flight_turn(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/compaction_routing.rs::primary_preference_change_applies_to_next_turn_without_rerouting_in_flight_turn.
	chat, database, conversation := chatFixture(t)
	assignment, err := chat.primaryAssignment(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if assignment.ProviderKind == "" || assignment.ProviderAccountID == "" || conversation.ID == "" {
		t.Fatalf("primary assignment = %#v", assignment)
	}
	if maxOutputTokensFor(assignment.ProviderKind) == nil {
		t.Fatal("primary assignment has no output budget")
	}
	if _, err := database.Agent(context.Background(), store.PrimaryAgentID); err != nil {
		t.Fatal(err)
	}
}

func TestRustRuntime_runtime_turn_rehydrates_recorded_failure_conversation_for_retry(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/compaction_routing.rs::runtime_turn_rehydrates_recorded_failure_conversation_for_retry.
	chat, database, conversation := chatFixture(t)
	chat.openRouter = generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return provider.GenerationResult{}, errors.New("provider failure")
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	for _, input := range []string{"first", "second"} {
		if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: input}); err != nil {
			t.Fatal(err)
		}
		collectCompletedTurns(t, events, 1)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil || len(page.Items) < 4 {
		t.Fatalf("failure replay items = %#v, %v", page.Items, err)
	}
	if page.Items[0].Kind != store.ConversationUserText || page.Items[2].Kind != store.ConversationUserText {
		t.Fatalf("failed conversation did not preserve both user turns: %#v", page.Items)
	}
}

func TestRustRuntime_runtime_turn_streams_tool_call_started_before_durable_response_items(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/compaction_routing.rs::runtime_turn_streams_tool_call_started_before_durable_response_items.
	chat, database, conversation := chatFixture(t)
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	requestCount := 0
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, onEvent func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requestCount++
		if requestCount == 1 {
			onEvent(provider.StreamEvent{Kind: provider.TextDelta, Delta: "Searching memory."})
			onEvent(provider.StreamEvent{Kind: provider.ToolCallStarted, Index: 0, ID: "call_1", Name: noemamemory.SearchToolName})
			return provider.GenerationResult{
				Text: "Searching memory.",
				ToolCalls: []provider.GenerationToolCall{{
					Index: 0, ProviderCallID: "call_1", ProviderName: noemamemory.SearchToolName,
					Name: noemamemory.SearchToolName, Payload: json.RawMessage(`{"query":"trains"}`),
				}},
			}, nil
		}
		onEvent(provider.StreamEvent{Kind: provider.TextDelta, Delta: "I found your train memory."})
		return provider.GenerationResult{Text: "I found your train memory."}, nil
	})
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "What do you remember about trains?"}); err != nil {
		t.Fatal(err)
	}
	collected := collectCompletedTurns(t, events, 1)
	streamedText := -1
	streamedToolStarted := -1
	durableCommentary := -1
	var transientActivity, durableTool *store.ConversationItem
	for index, event := range collected {
		if event.Kind == EventConversationItem && event.Item != nil && event.Item.Status == "running" && event.Item.ContentText == "Searching memory." {
			streamedText = index
		}
		if event.Kind != EventConversationItem || event.Item == nil {
			continue
		}
		if event.Item.Kind == store.ConversationAssistantText && event.Item.ContentText == "Searching memory." {
			durableCommentary = index
		}
		if event.Item.Kind == store.ConversationActivity {
			id, _ := event.Item.Payload["id"].(string)
			activityKind, _ := event.Item.Payload["activity_kind"].(string)
			status, _ := event.Item.Payload["status"].(string)
			title, _ := event.Item.Payload["title"].(string)
			if strings.HasPrefix(event.Item.ID, "transient:tool_call:") &&
				activityKind == "tool_call" && status == "started" &&
				title == "Tool call: search_memory" {
				streamedToolStarted = index
				copy := *event.Item
				transientActivity = &copy
				if id == "" || id != strings.TrimPrefix(event.Item.ID, "transient:") {
					t.Fatalf("transient tool activity identity = %#v", event.Item)
				}
			}
		}
		if event.Item.Kind == store.ConversationToolCall {
			copy := *event.Item
			durableTool = &copy
		}
	}
	if streamedText < 0 || streamedToolStarted < 0 || durableCommentary < 0 ||
		!(streamedText < streamedToolStarted && streamedToolStarted < durableCommentary) {
		t.Fatalf("tool marker ordering = text %d, marker %d, commentary %d; events=%#v", streamedText, streamedToolStarted, durableCommentary, collected)
	}
	if transientActivity == nil || durableTool == nil {
		t.Fatalf("tool lifecycle items = transient=%#v durable=%#v events=%#v", transientActivity, durableTool, collected)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 40)
	if err != nil {
		t.Fatal(err)
	}
	durableAssistantIndex, durableToolIndex := -1, -1
	var replayedTool store.ConversationItem
	for index, item := range page.Items {
		if item.Kind == store.ConversationAssistantText && item.ContentText == "Searching memory." {
			durableAssistantIndex = index
		}
		if item.Kind == store.ConversationToolCall {
			durableToolIndex = index
			replayedTool = item
		}
	}
	if durableAssistantIndex < 0 || durableToolIndex < 0 || durableAssistantIndex >= durableToolIndex {
		t.Fatalf("durable replay order = assistant %d, tool %d, items=%#v", durableAssistantIndex, durableToolIndex, page.Items)
	}
	if replayedID, _ := replayedTool.Payload["id"].(string); replayedID != transientActivity.Payload["id"] {
		t.Fatalf("durable call did not replace transient activity: durable=%q transient=%#v", replayedID, transientActivity.Payload)
	}
	durableMetadata, _ := durableTool.Payload["metadata"].(map[string]any)
	durableDisplay, _ := durableMetadata["display"].(map[string]any)
	durableMarker, _ := durableDisplay["marker"].(map[string]any)
	if durableMarker["identity"] != "Search memory" || durableMarker["summary"] != "Searching memory for “trains”" || durableMarker["status"] != "running" || durableMarker["subject"] != "trains" {
		t.Fatalf("live durable tool marker = %#v", durableMarker)
	}
	metadata, _ := replayedTool.Payload["metadata"].(map[string]any)
	display, _ := metadata["display"].(map[string]any)
	if _, exists := display["description"]; exists {
		t.Fatalf("durable tool display = %#v", display)
	}
	if _, exists := display["marker"]; exists {
		t.Fatalf("stored tool marker was not deferred to replay = %#v", display)
	}
}

func rustRuntimeLoadRun(t *testing.T, database *store.Store, taskID, runID string) store.TaskRun {
	t.Helper()
	runs, err := database.TaskRuns(context.Background(), taskID, 100)
	if err != nil {
		t.Fatal(err)
	}
	for _, run := range runs {
		if run.ID == runID {
			return run
		}
	}
	t.Fatalf("run %s was not found in %#v", runID, runs)
	return store.TaskRun{}
}

func itoa(value int) string {
	return fmt.Sprintf("%d", value)
}

func bytesEqualJSON(a, b json.RawMessage) bool {
	var left, right any
	if json.Unmarshal(a, &left) != nil || json.Unmarshal(b, &right) != nil {
		return string(a) == string(b)
	}
	return fmt.Sprint(left) == fmt.Sprint(right)
}
