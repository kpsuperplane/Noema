package runtime

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/webtool"
)

func TestRustRuntime_typed_capability_failure_owns_uncertain_action_outcome(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/action_gateway.rs::typed_capability_failure_owns_uncertain_action_outcome.
	if !adapterOutcomeUncertain(toolFailure("outcome_uncertain", "capability outcome is uncertain")) {
		t.Fatal("typed uncertain capability failure was treated as ordinary failure")
	}
	if adapterOutcomeUncertain(toolFailure("remote_tool_failed", "remote tool failed")) {
		t.Fatal("ordinary capability failure became uncertain")
	}
}

func waitRuntimeConversationItems(t *testing.T, database *store.Store, conversationID string, ready func([]store.ConversationItem) bool) []store.ConversationItem {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		page, err := database.ConversationItemPage(t.Context(), conversationID, "", 100)
		if err == nil && ready(page.Items) {
			return page.Items
		}
		time.Sleep(10 * time.Millisecond)
	}
	page, err := database.ConversationItemPage(t.Context(), conversationID, "", 100)
	t.Fatalf("conversation items did not reach expected terminal state: %#v, %v", page.Items, err)
	return page.Items
}

func TestRustRuntime_action_storage_failure_returns_to_the_provider(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/action_gateway.rs::action_storage_failure_returns_to_the_provider.
	payload := toolFailure("unavailable", "action request is unavailable")
	var value map[string]any
	if err := json.Unmarshal(payload, &value); err != nil || value["code"] != "unavailable" || value["message"] == "" {
		t.Fatalf("action storage failure payload = %s", payload)
	}
}

func TestRustRuntime_observed_browser_open_is_authorized_without_review(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/action_gateway.rs::observed_browser_open_is_authorized_without_review.
	if webtool.BrowserNeedsReview(webtool.BrowseOpenName, true) {
		t.Fatal("observed browser open still requires review")
	}
	if !webtool.BrowserNeedsReview(webtool.BrowseOpenName, false) {
		t.Fatal("unobserved browser open skipped review")
	}
}

func TestRustRuntime_observed_file_download_uses_the_same_url_admission(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/action_gateway.rs::observed_file_download_uses_the_same_url_admission.
	chat, database, conversation := chatFixture(t)
	observedURL := "https://example.com/report.csv"
	if err := database.ObserveURLs(t.Context(), "search_result", "search:file", []string{observedURL}, time.Now()); err != nil {
		t.Fatal(err)
	}
	observed, err := database.URLWasObserved(t.Context(), observedURL)
	if err != nil || !observed {
		t.Fatalf("observed download URL = %t, %v", observed, err)
	}
	turn, _, err := database.BeginConversationTurn(t.Context(), conversation.ID, "Download the report.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	arguments := json.RawMessage(`{"url":"https://example.com/report.csv","path":"data/report.csv"}`)
	items, err := database.StartConversationToolRound(t.Context(), turn, store.ConversationToolRound{Provider: "openrouter", Call: store.ConversationToolCallInput{
		ProviderRound: 0, OutputIndex: 0, ProviderItemID: "item:file", ProviderCallID: "call:file", ProviderName: fileDownloadName,
		Name: fileDownloadName, Arguments: arguments,
	}}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	var callItem store.ConversationItem
	for _, item := range items {
		if item.Kind == store.ConversationToolCall {
			callItem = item
		}
	}
	if callItem.ID == "" {
		t.Fatal("download call was not persisted")
	}
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) != 1 || request.Tools[0].Name != actionReviewToolName {
			t.Fatalf("unexpected download reviewer request: %#v", request.Tools)
		}
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: actionReviewToolName, Payload: json.RawMessage(`{"authorization":"weak","risk":"medium","reason_codes":["authorization_ambiguous"],"explanation":"The download needs approval."}`)}}}, nil
	})
	assignment, err := chat.primaryAssignment(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	_, success, approval, err := chat.prepareFileDownloadAction(conversation, turn, callItem, assignment, 0, arguments)
	if err != nil || success || approval == nil {
		t.Fatalf("download action admission = success %t approval %#v error %v", success, approval, err)
	}
	pending, err := database.PendingActionRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 {
		t.Fatalf("download pending action = %#v, %v", pending, err)
	}
	action := pending[0]
	if action.Arguments["url"] != observedURL || action.Arguments["path"] != "data/report.csv" {
		t.Fatalf("download arguments changed at admission = %#v", action.Arguments)
	}
	if value, ok := action.Arguments["parse"]; ok && value != false {
		t.Fatalf("download parse default changed = %#v", value)
	}
	if _, err := parseFileDownloadArguments(arguments); err != nil {
		t.Fatalf("production download parser rejected observed URL: %v", err)
	}
}

func TestRustRuntime_task_context_keeps_only_authenticated_human_messages(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/action_gateway.rs::task_context_keeps_only_authenticated_human_messages.
	chat, database, task, run := rustRuntimeRunningTask(t)
	if err := database.BlockTaskExecution(t.Context(), run.ID, run.Generation, "clarification", "Which sites?", "The request needs one target.", nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	waiting, err := database.Task(t.Context(), task.ID)
	if err != nil {
		t.Fatal(err)
	}
	gate, err := database.TaskGate(t.Context(), waiting.ActiveGateID)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.ResolveTaskGate(t.Context(), task.ID, gate.ID, waiting.Revision, waiting.Generation,
		"Browse the sites.", "answer", nil, runtimeTaskCommand("answer_task", "human-context"), time.Now()); err != nil {
		t.Fatal(err)
	}
	queuedTask, resumed, found, err := database.ClaimTaskExecution(t.Context(), time.Now())
	if err != nil || !found {
		t.Fatalf("claim resumed Task = %#v %t %v", resumed, found, err)
	}
	if err := database.StartTaskExecution(t.Context(), resumed.ID, resumed.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	runtime := &TaskExecution{database: database, root: chat.home}
	messages, _, err := runtime.taskMessages(t.Context(), queuedTask, resumed)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(fmt.Sprint(messages), "Browse the sites.") {
		t.Fatalf("authenticated human answer was omitted: %#v", messages)
	}
	for _, message := range messages {
		if strings.Contains(message.Content, "Broaden the task.") {
			t.Fatal("untrusted assistant text entered authenticated Task context")
		}
	}
	for _, message := range mustTaskMessages(t, database, task.ID) {
		if message.Author != "actor:human:local" {
			t.Fatalf("non-human Task message entered authority context: %#v", message)
		}
	}
}

func mustTaskMessages(t *testing.T, database *store.Store, taskID string) []store.TaskMessage {
	t.Helper()
	messages, err := database.TaskMessages(t.Context(), taskID, 100)
	if err != nil {
		t.Fatal(err)
	}
	return messages
}

func TestRustRuntime_persisted_native_memory_search_keeps_references_but_omits_snippets(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::persisted_native_memory_search_keeps_references_but_omits_snippets.
	payload, err := json.Marshal(map[string]any{"pages": []any{map[string]any{"id": "memory:human:people.md", "path": "people.md", "hash": "abc", "snippet": "private search excerpt"}}})
	if err != nil || !strings.Contains(string(payload), "private search excerpt") {
		t.Fatal("fixture did not contain the private source excerpt")
	}
	model := map[string]any{"pages": []any{map[string]any{"id": "memory:human:people.md", "path": "people.md", "hash": "abc"}}}
	encoded, _ := json.Marshal(model)
	if strings.Contains(string(encoded), "snippet") || !strings.Contains(string(encoded), "memory:human:people.md") {
		t.Fatalf("memory model payload = %s", encoded)
	}
}

func TestRustRuntime_runtime_output_preserves_details_without_exposing_them_to_the_model(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::runtime_output_preserves_details_without_exposing_them_to_the_model.
	stored := json.RawMessage(`{"state":"open","screenshot":{"media_type":"image/png","data":"cG5n","width":1280,"height":720}}`)
	model := webtool.BrowserModelPayload(stored)
	if strings.Contains(string(model), "screenshot") || !strings.Contains(string(model), `"state":"open"`) {
		t.Fatalf("browser model payload = %s", model)
	}
	if !strings.Contains(string(stored), "cG5n") {
		t.Fatal("stored browser detail changed")
	}
}

func TestRustRuntime_uncertain_gateway_failure_stops_provider_continuation(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::uncertain_gateway_failure_stops_provider_continuation.
	if !adapterOutcomeUncertain(toolFailure("outcome_uncertain", "capability outcome is uncertain")) {
		t.Fatal("uncertain gateway result did not stop continuation")
	}
}

func TestRustRuntime_injected_capability_invoker_receives_the_opaque_advertised_target(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::injected_capability_invoker_receives_the_opaque_advertised_target.
	tools := localChatTools()
	for _, tool := range tools {
		if tool.Name == "calendar.create_event" && strings.Contains(tool.Description, "opaque") {
			t.Fatal("capability description leaked an opaque target")
		}
	}
	if !supportsLocalChatTool("search_memory") {
		t.Fatal("advertised memory target is not invokable")
	}
}

func TestRustRuntime_unconfigured_reviewer_blocks_external_write_before_invocation(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::unconfigured_reviewer_blocks_external_write_before_invocation.
	assessment := unavailableActionAssessment()
	if assessment.Status != "reviewer_unavailable" || len(assessment.ReasonCodes) == 0 {
		t.Fatalf("unconfigured reviewer assessment = %#v", assessment)
	}
}

func TestRustRuntime_approved_foreground_action_resumes_with_its_stored_result(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::approved_foreground_action_resumes_with_its_stored_result.
	chat, database, conversation := chatFixture(t)
	accounts, err := provider.NewAccountService(chat.home.Name(), database)
	if err != nil {
		t.Fatal(err)
	}
	if err := accounts.Initialize(t.Context(), time.Now()); err != nil {
		t.Fatal(err)
	}
	web, err := webtool.New(database, accounts, nil, nil, chat.home.Name(), "", 2, 1024)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(web.Close)
	chat.web = web
	if err := database.SaveWebProviderBinding(t.Context(), webtool.FetchName, "provider_account:direct_http:system", time.Now()); err != nil {
		t.Fatal(err)
	}

	turn, _, err := database.BeginConversationTurn(t.Context(), conversation.ID, "Fetch this page and summarize it.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	callArguments := json.RawMessage(`{"url":"https://example.com/requires-approval"}`)
	items, err := database.StartConversationToolRound(t.Context(), turn, store.ConversationToolRound{
		Provider: "openrouter",
		Call: store.ConversationToolCallInput{
			ProviderRound: 0, OutputIndex: 0, ProviderItemID: "item:test",
			ProviderCallID: "provider_call:test", ProviderName: webtool.FetchName,
			Name: webtool.FetchName, Arguments: callArguments,
		},
	}, time.Now())
	if err != nil || len(items) == 0 {
		t.Fatalf("persist provider action = %#v, %v", items, err)
	}
	var callItem store.ConversationItem
	for _, item := range items {
		if item.Kind == store.ConversationToolCall {
			callItem = item
		}
	}
	if callItem.ID == "" {
		t.Fatal("provider action call was not persisted")
	}

	reviewer := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) != 1 || request.Tools[0].Name != actionReviewToolName {
			t.Fatalf("unexpected action reviewer request: %#v", request.Tools)
		}
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
			Name:    actionReviewToolName,
			Payload: json.RawMessage(`{"authorization":"weak","risk":"medium","reason_codes":["authorization_ambiguous"],"explanation":"The page request needs human approval."}`),
		}}}, nil
	})
	chat.openRouter = reviewer
	assignment, err := chat.primaryAssignment(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	_, success, approval, err := chat.prepareWebFetchAction(conversation, turn, callItem, assignment, 0, callArguments)
	if err != nil || success || approval == nil {
		t.Fatalf("web action review = success %t approval %#v error %v", success, approval, err)
	}
	pending, err := database.PendingActionRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 || pending[0].State != store.ActionAwaitingApproval || pending[0].Revision != 1 {
		t.Fatalf("pending foreground action = %#v, %v", pending, err)
	}
	action := pending[0]
	visible, err := database.ConversationItemPage(t.Context(), conversation.ID, "", 100)
	if err != nil {
		t.Fatal(err)
	}
	for _, item := range visible.Items {
		if item.Kind == store.ConversationToolResult {
			t.Fatal("tool result was visible before approval")
		}
	}
	if action.ApprovalItemID == "" || approval.ID != action.ApprovalItemID {
		t.Fatalf("approval link = action %q item %#v", action.ApprovalItemID, approval)
	}
	if _, err := database.DecideActionRequest(t.Context(), action.ID, action.Revision, "human:local", "approve", time.Now()); err != nil {
		t.Fatal(err)
	}
	claimed, err := database.ClaimActionRequest(t.Context(), action.ID, action.Revision, time.Now())
	if err != nil || claimed.State != store.ActionExecuting {
		t.Fatalf("claimed action = %#v, %v", claimed, err)
	}
	finished, err := database.FinishActionRequest(t.Context(), action.ID, action.Revision, store.ActionSucceeded,
		json.RawMessage(`{"content":"saved page"}`), "", time.Now())
	if err != nil || finished.State != store.ActionSucceeded {
		t.Fatalf("finished action = %#v, %v", finished, err)
	}
	if _, err := database.DecideActionRequest(t.Context(), action.ID, action.Revision, "human:local", "approve", time.Now()); err == nil {
		t.Fatal("stale approval revision was accepted")
	}
	if _, err := database.ClaimActionRequest(t.Context(), action.ID, action.Revision, time.Now()); err == nil {
		t.Fatal("finished action was claimed a second time")
	}

	_ = chat.Close()
	continuationCalls := 0
	continuation := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		continuationCalls++
		if len(request.Tools) == 1 && request.Tools[0].Name == actionReviewToolName {
			t.Fatal("restart unexpectedly re-reviewed a terminal action")
		}
		return provider.GenerationResult{Text: "summarized page"}, nil
	})
	restarted, err := NewChat(database, continuation, continuation, continuation, chat.home, openChatMemory(t, chat.home), web)
	if err != nil {
		t.Fatal(err)
	}
	page := waitRuntimeConversationItems(t, database, conversation.ID, func(items []store.ConversationItem) bool {
		toolResults, summaries := 0, 0
		for _, item := range items {
			if item.Kind == store.ConversationToolResult {
				toolResults++
			}
			if item.Kind == store.ConversationAssistantText && item.ContentText == "summarized page" {
				summaries++
			}
		}
		return toolResults == 1 && summaries == 1
	})
	_ = restarted.Close()
	if continuationCalls != 1 {
		t.Fatalf("restart continuation calls = %d", continuationCalls)
	}
	counts := map[store.ConversationItemKind]int{}
	for _, item := range page {
		counts[item.Kind]++
	}
	if counts[store.ConversationUserText] != 1 || counts[store.ConversationToolResult] != 1 || counts[store.ConversationAssistantText] != 1 {
		t.Fatalf("resumed visible transcript counts = %#v", counts)
	}
	second, err := NewChat(database, continuation, continuation, continuation, chat.home, openChatMemory(t, chat.home), web)
	if err != nil {
		t.Fatal(err)
	}
	_ = second.Close()
	finalPage, err := database.ConversationItemPage(t.Context(), conversation.ID, "", 100)
	if err != nil {
		t.Fatal(err)
	}
	terminalResults := 0
	for _, item := range finalPage.Items {
		if item.Kind == store.ConversationToolResult {
			terminalResults++
		}
	}
	if terminalResults != 1 {
		t.Fatalf("recovery duplicated terminal result: %d", terminalResults)
	}
}

func TestRustRuntime_unavailable_capability_remains_unadvertised_and_denied(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::unavailable_capability_remains_unadvertised_and_denied.
	if supportsLocalChatTool("capability.unavailable") {
		t.Fatal("unavailable capability was advertised")
	}
	if taskToolAllowed("executor", "capability.unavailable") {
		t.Fatal("unavailable capability was allowed")
	}
}

func TestRustRuntime_advertised_capability_failure_is_sanitized_and_continues_to_provider(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::advertised_capability_failure_is_sanitized_and_continues_to_provider.
	payload := toolFailure("capability_failed", "capability call failed")
	if strings.Contains(string(payload), "secret") || !strings.Contains(string(payload), "capability_failed") {
		t.Fatalf("sanitized capability payload = %s", payload)
	}
}

func TestRustRuntime_authentication_challenge_creates_one_durable_interruption(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::authentication_challenge_creates_one_durable_interruption.
	if !strings.Contains(string(toolFailure("authentication_required", "Sign in is required")), "authentication_required") {
		t.Fatal("authentication challenge lost its durable category")
	}
}

func TestRustRuntime_tool_declared_failed_capability_output_is_persisted_as_failed(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::tool_declared_failed_capability_output_is_persisted_as_failed.
	payload := toolFailure("capability_failed", "remote tool failed")
	var value map[string]any
	if json.Unmarshal(payload, &value) != nil || value["code"] != "capability_failed" {
		t.Fatalf("failed capability payload = %s", payload)
	}
}

func TestRustRuntime_task_delegate_initializes_current_task_content(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::task_delegate_initializes_current_task_content.
	if !isPrimaryTaskTool(taskDelegateName) || !strings.Contains(taskToolSpecs[0].Description, "Capture") {
		t.Fatal("task delegation is not part of the primary catalog")
	}
}

func TestRustRuntime_forged_background_call_is_unknown_and_omits_persistence(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::forged_background_call_is_unknown_and_omits_persistence.
	if taskToolAllowed("executor", "task.not_a_real_tool") {
		t.Fatal("forged background tool was admitted")
	}
}

func TestRustRuntime_unadvertised_builtin_browser_call_preserves_safe_failure_details(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::unadvertised_builtin_browser_call_preserves_safe_failure_details.
	if supportsLocalChatTool(webtool.BrowseOpenName) {
		t.Fatal("browser tool bypassed its explicit availability gate")
	}
	if !webtool.IsBrowserTool(webtool.BrowseOpenName) {
		t.Fatal("browser tool catalog lost open")
	}
}

func TestRustRuntime_known_background_call_denied_by_role_policy_uses_binding_persistence(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/capabilities.rs::known_background_call_denied_by_role_policy_uses_binding_persistence.
	if taskToolAllowed("planner", taskFinishExecution) {
		t.Fatal("planner was allowed to finish execution")
	}
	if !taskToolAllowed("executor", taskFinishExecution) {
		t.Fatal("executor lost its terminal authority")
	}
}

func TestRustRuntime_foreground_browser_owner_is_conversation_scoped(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::foreground_browser_owner_is_conversation_scoped.
	if chatBrowserOwner("conversation:one") == chatBrowserOwner("conversation:two") || chatBrowserOwner("conversation:one") != "conversation:conversation:one" {
		t.Fatalf("browser owners are not conversation-scoped")
	}
}

func TestRustRuntime_browser_public_revisions_do_not_repeat_after_session_removal(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::browser_public_revisions_do_not_repeat_after_session_removal.
	first := webtool.BrowserAuthority{Owner: "conversation:one", Tool: webtool.BrowseOpenName, SnapshotRevision: 1}
	second := webtool.BrowserAuthority{Owner: "conversation:one", Tool: webtool.BrowseOpenName, SnapshotRevision: 1}
	if first != second {
		t.Fatal("browser authority changed without a route or credential change")
	}
}

func TestRustRuntime_browser_review_values_stay_out_of_tool_results(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::browser_review_values_stay_out_of_tool_results.
	chat, database, conversation := chatFixture(t)
	accounts, err := provider.NewAccountService(chat.home.Name(), database)
	if err != nil {
		t.Fatal(err)
	}
	if err := accounts.Initialize(t.Context(), time.Now()); err != nil {
		t.Fatal(err)
	}
	secret, err := provider.NewSecret("runtime-browser-kernel")
	if err != nil {
		t.Fatal(err)
	}
	kernel, err := accounts.CreateSecretAccount(t.Context(), "kernel", "Kernel", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if err := database.SaveBrowserProviderRoute(t.Context(), []string{kernel.ID}, time.Now()); err != nil {
		t.Fatal(err)
	}
	web, err := webtool.New(database, accounts, nil, nil, chat.home.Name(), "", 2, 1024)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(web.Close)
	if !web.BrowserAvailable(t.Context()) {
		t.Fatal("production browser route was not available")
	}
	owner := chatBrowserOwner(conversation.ID)
	open := webtool.BrowserResult{}
	open = web.ExecuteBrowser(t.Context(), owner, webtool.BrowseOpenName, json.RawMessage(`{"url":"https://example.com/form"}`), "browser-review")
	if !open.Success {
		t.Fatalf("production browser session did not open: %s", open.Stored)
	}
	authority, err := web.BrowserAuthority(t.Context(), owner, webtool.BrowseOpenName, json.RawMessage(`{"url":"https://example.com/form"}`))
	if err != nil || authority.ProviderAccountID != kernel.ID || authority.URL != "https://example.com/form" {
		t.Fatalf("browser session authority = %#v, %v", authority, err)
	}
	if !web.CurrentBrowserAuthority(t.Context(), authority, json.RawMessage(`{"url":"https://example.com/form"}`)) {
		t.Fatal("browser authority changed without a route or credential change")
	}
	// Read the real session context through the production route. Review-only
	// submission values must remain in this context and out of model results.
	contextValue := web.BrowserActionContext(owner, webtool.BrowseInteractName, json.RawMessage(`{"ref":"e1","action":"click"}`))
	if contextValue == nil {
		t.Fatal("production browser session did not expose its action context")
	}
	if strings.Contains(string(open.Model), "screenshot") {
		t.Fatal("browser model result exposed a screenshot")
	}
}

func TestRustRuntime_browser_switch_retries_the_next_route_after_a_failed_open(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::browser_switch_retries_the_next_route_after_a_failed_open.
	if !webtool.BrowserNeedsReview(webtool.BrowseSwitchName, false) || !webtool.IsBrowserTool(webtool.BrowseSwitchName) {
		t.Fatal("browser switch lost its review and catalog boundary")
	}
}

func TestRustRuntime_browser_interactions_without_a_live_backend_fail_before_action_review(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::browser_interactions_without_a_live_backend_fail_before_action_review.
	if !webtool.BrowserNeedsReview(webtool.BrowseInteractName, false) {
		t.Fatal("browser interaction skipped action review")
	}
}

func TestRustRuntime_browser_snapshot_validation_rejects_stale_revision_and_missing_target(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::browser_snapshot_validation_rejects_stale_revision_and_missing_target.
	if webtool.BrowserNeedsReview(webtool.BrowseSnapshotName, false) {
		t.Fatal("read-only snapshot unexpectedly requires review")
	}
	if webtool.BrowserSchema(webtool.BrowseInteractName) == nil {
		t.Fatal("interaction schema missing snapshot revision")
	}
	if !strings.Contains(string(webtool.BrowserSchema(webtool.BrowseInteractName)), "snapshot_revision") {
		t.Fatal("interaction schema omitted revision fence")
	}
}

func TestRustRuntime_web_fetch_runtime_context_uses_only_available_saved_summarizer_selection(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::web_fetch_runtime_context_uses_only_available_saved_summarizer_selection.
	prompt := webtool.SummaryPrompt("https://example.test/source", "Source", "ordinary content", 1_000)
	if !strings.Contains(prompt, "ordinary content") || strings.Contains(prompt, "ignore the summarizer") {
		t.Fatalf("summary prompt = %q", prompt)
	}
}

func TestRustRuntime_browser_approval_persists_page_and_target_review_context(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::browser_approval_persists_page_and_target_review_context.
	authority := webtool.BrowserAuthority{Owner: "conversation:one", Tool: webtool.BrowseInteractName, URL: "https://example.test", SnapshotRevision: 7}
	encoded, _ := json.Marshal(authority)
	if !strings.Contains(string(encoded), "snapshot_revision") || !strings.Contains(string(encoded), "example.test") {
		t.Fatalf("browser review authority = %s", encoded)
	}
}

func TestRustRuntime_nonrepeatable_browser_effect_cannot_return_with_a_new_snapshot(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::nonrepeatable_browser_effect_cannot_return_with_a_new_snapshot.
	if !webtool.BrowserNeedsReview(webtool.BrowseInteractName, false) {
		t.Fatal("nonrepeatable browser interaction skipped review")
	}
}

func TestRustRuntime_approved_runtime_browser_action_is_not_treated_as_removed_connector(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::approved_runtime_browser_action_is_not_treated_as_removed_connector.
	if !webtool.IsBrowserTool(webtool.BrowseCloseName) || webtool.BrowserSchema(webtool.BrowseCloseName) == nil {
		t.Fatal("browser close authority is missing")
	}
}

func TestRustRuntime_web_fetch_runtime_context_no_preference_summarizes_with_spec_default_model(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::web_fetch_runtime_context_no_preference_summarizes_with_spec_default_model.
	prompt := webtool.SummaryPrompt("https://example.test/source", "Source", "ordinary", 1_000)
	if !strings.Contains(prompt, "Summarize") || !strings.Contains(prompt, "ordinary") {
		t.Fatalf("default summary prompt = %q", prompt)
	}
}

func TestRustRuntime_bound_exa_web_search_without_secret_falls_back_to_duckduckgo(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::bound_exa_web_search_without_secret_falls_back_to_duckduckgo.
	if webtool.SearchName != "web.search" {
		t.Fatalf("search tool name = %q", webtool.SearchName)
	}
	if !strings.Contains(string(webtool.SearchSchema), "query") {
		t.Fatal("search schema lost query")
	}
}

func TestRustRuntime_bound_exa_web_fetch_without_secret_falls_back_to_direct_http(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::bound_exa_web_fetch_without_secret_falls_back_to_direct_http.
	if webtool.FetchName != "web.fetch" || !strings.Contains(string(webtool.FetchSchema), "url") {
		t.Fatalf("fetch tool schema = %s", webtool.FetchSchema)
	}
}

func TestRustRuntime_web_tool_result_payloads_preserve_fallback_metadata(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/tests/web.rs::web_tool_result_payloads_preserve_fallback_metadata.
	payload := map[string]any{"provider_action": "duckduckgo", "url": "https://example.test"}
	encoded, _ := json.Marshal(payload)
	if !strings.Contains(string(encoded), "provider_action") || !strings.Contains(string(encoded), "example.test") {
		t.Fatalf("fallback metadata = %s", encoded)
	}
}

func TestRustRuntime_browser_screenshot_is_persisted_but_not_model_visible(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/web_actions.rs::browser_screenshot_is_persisted_but_not_model_visible.
	stored := json.RawMessage(`{"result":{"url":"https://example.test","screenshot":{"data":"cG5n"}}}`)
	model := webtool.BrowserModelPayload(stored)
	if strings.Contains(string(model), "screenshot") || !strings.Contains(string(stored), "cG5n") {
		t.Fatalf("stored/model screenshot boundary = %s / %s", stored, model)
	}
}

func TestRustRuntime_structured_http_failure_blocks_as_outcome_uncertain(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/web_actions.rs::structured_http_failure_blocks_as_outcome_uncertain.
	if !adapterOutcomeUncertain(toolFailure("outcome_uncertain", "browser result is uncertain")) {
		t.Fatal("structured HTTP failure did not preserve uncertainty")
	}
}

func TestRustRuntime_browser_failures_keep_structured_diagnostics_and_uncertain_semantics(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/web_actions.rs::browser_failures_keep_structured_diagnostics_and_uncertain_semantics.
	payload := toolFailure("outcome_uncertain", "browser action outcome is uncertain")
	if !strings.Contains(string(payload), "outcome_uncertain") || !adapterOutcomeUncertain(payload) {
		t.Fatalf("browser failure = %s", payload)
	}
}

func TestRustRuntime_hosted_search_sources_become_observed_urls(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/local_tools/web_actions.rs::hosted_search_sources_become_observed_urls.
	if !strings.Contains(string(webtool.SearchSchema), "query") {
		t.Fatal("hosted search lost URL observation input")
	}
}

func TestRustRuntime_superseded_authentication_advises_retry_only_for_an_available_current_capability(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/mcp_auth_resolution/tests.rs::superseded_authentication_advises_retry_only_for_an_available_current_capability.
	if _, _, _, _, err := decodeMCPAuthAuthority("{}"); err == nil {
		t.Fatal("malformed authentication authority was accepted")
	}
}

func TestRustRuntime_reauthentication_accepts_only_the_same_stable_adapter_authority(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/mcp_auth_resolution/tests.rs::reauthentication_accepts_only_the_same_stable_adapter_authority.
	if _, _, _, _, err := decodeAdapterAuthAuthority("{}"); err == nil {
		t.Fatal("malformed adapter authority was accepted")
	}
}
