package runtime

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/home"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/webtool"
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

func TestRustRuntime_diffs_emit_stable_full_replacement_and_removal_updates(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::diffs_emit_stable_full_replacement_and_removal_updates.
	location := time.FixedZone("America/Los_Angeles", -7*60*60)
	first := runtimeEnvironment(store.Conversation{}, location, time.Date(2026, 7, 15, 20, 45, 0, 0, time.UTC))
	second := runtimeEnvironment(store.Conversation{}, location, time.Date(2026, 7, 16, 20, 45, 0, 0, time.UTC))
	if first == second || !strings.Contains(first, `current_date: "2026-07-15"`) || !strings.Contains(second, `current_date: "2026-07-16"`) {
		t.Fatalf("runtime environment did not replace the date: %q / %q", first, second)
	}
	if !strings.Contains(second, "override any provider, platform, server, or UTC clock") {
		t.Fatal("replacement lost runtime authority")
	}
}

func TestRustRuntime_tool_instruction_changes_replace_visibility_context(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::tool_instruction_changes_replace_visibility_context.
	planner, executor := taskRolePrompt("planner"), taskRolePrompt("executor")
	if planner == executor || !strings.Contains(planner, "task.finish_planning") || !strings.Contains(executor, "task.finish_execution") {
		t.Fatalf("role instructions did not replace tool visibility: planner=%q executor=%q", planner, executor)
	}
	if strings.Contains(planner, "task.finish_execution") || strings.Contains(executor, "task.finish_planning") {
		t.Fatal("role instruction retained another role's terminal authority")
	}
}

func TestRustRuntime_project_catalog_renders_exact_metadata_and_replaces_as_one_section(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::project_catalog_renders_exact_metadata_and_replaces_as_one_section.
	chat, database, conversation := chatFixture(t)
	folder := filepath.Join(t.TempDir(), "alpha")
	arguments, err := json.Marshal(map[string]string{"name": "Alpha", "description": "opaque-value-01928", "folder": folder})
	if err != nil {
		t.Fatal(err)
	}
	payload, success := chat.executeChatTool(context.Background(), conversation, projectCreateName, arguments, "project-catalog", "turn:project")
	if !success {
		t.Fatalf("project create = %s", payload)
	}
	value := mustToolValue(t, payload)
	project := value["project"].(map[string]any)
	if project["name"] != "Alpha" || project["description"] != "opaque-value-01928" || project["folder"] != folder {
		t.Fatalf("project metadata = %#v", project)
	}
	chatValue, err := chat.projectContext(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	chatValue = modelContextSectionContent(t, []provider.GenerationMessage{{Role: "developer", Content: chatValue}}, "projects.catalog")
	if !strings.Contains(chatValue, "opaque-value-01928") || !strings.Contains(chatValue, folder) || !strings.Contains(chatValue, `"has_more":false`) {
		t.Fatalf("project context = %q, %v", chatValue, err)
	}
	if conversation.ID == "" || database == nil {
		t.Fatal("project fixture did not create a conversation")
	}
}

func TestRustRuntime_identity_section_preserves_unnamed_onboarding_semantics(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::identity_section_preserves_unnamed_onboarding_semantics.
	prompt := agentIdentityPrompt(store.Agent{ID: store.PrimaryAgentID})
	for _, required := range []string{"Agent identity:", "You do not have a name yet.", "call update_own_name", "Onboarding tasks, in priority order:", "Ask at most one onboarding question"} {
		if !strings.Contains(prompt, required) {
			t.Fatalf("unnamed identity prompt lacks %q: %q", required, prompt)
		}
	}
}

func TestRustRuntime_tool_inputs_are_normalized_before_comparison_and_rendering(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::tool_inputs_are_normalized_before_comparison_and_rendering.
	tools := localChatTools()
	seen := map[string]bool{}
	for _, tool := range tools {
		if strings.TrimSpace(tool.Name) == "" || seen[tool.Name] {
			t.Fatalf("tool catalog contains an empty or duplicate name: %#v", tools)
		}
		seen[tool.Name] = true
		if len(tool.InputSchema) == 0 {
			t.Fatalf("tool %q has no input schema", tool.Name)
		}
	}
	if !seen[noemamemory.SearchToolName] || !seen[taskDelegateName] || !seen[projectListName] {
		t.Fatalf("normalized catalog omitted required tools: %v", seen)
	}
}

func TestRustRuntime_transport_specific_instructions_match_native_and_no_tool_modes(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::transport_specific_instructions_match_native_and_no_tool_modes.
	if len(localChatTools()) == 0 || !supportsLocalChatTool(noemamemory.SearchToolName) {
		t.Fatal("native local tool channel is unavailable")
	}
	if supportsLocalChatTool(webtool.SearchName) {
		t.Fatal("external web search became available without an explicit web binding")
	}
	if len(taskExecutionTools("unknown")) != 0 {
		t.Fatal("unknown role received executable tools")
	}
}

func TestRustRuntime_hosted_web_is_the_default_reader_when_browser_alias_is_open(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::hosted_web_is_the_default_reader_when_browser_alias_is_open.
	if !hostedWebSearchEnabled("openrouter", provider.ToolTransportNative) {
		t.Fatal("native OpenRouter route did not enable hosted web search")
	}
	if webtool.SearchName != "web.search" || webtool.FetchName != "web.fetch" {
		t.Fatal("web tool aliases changed")
	}
	if !strings.Contains(string(webtool.SearchSchema), "query") || !strings.Contains(string(webtool.FetchSchema), "url") {
		t.Fatal("hosted web tool schemas lost their ordinary inputs")
	}
}

func TestRustRuntime_local_web_is_preferred_to_browser_for_ordinary_pages(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::local_web_is_preferred_to_browser_for_ordinary_pages.
	if len(webtool.Tools) != 2 || webtool.Tools[0].Name != webtool.SearchName || webtool.Tools[1].Name != webtool.FetchName {
		t.Fatalf("local web catalog order = %#v", webtool.Tools)
	}
	if webtool.IsBrowserTool(webtool.FetchName) || webtool.IsBrowserTool(webtool.SearchName) {
		t.Fatal("ordinary web tools were classified as browser interaction")
	}
}

func TestRustRuntime_absent_external_tools_are_explicitly_unavailable(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::absent_external_tools_are_explicitly_unavailable.
	for _, name := range []string{webtool.SearchName, webtool.FetchName, "mcp.missing.read"} {
		if supportsLocalChatTool(name) {
			t.Fatalf("unbound external tool %q was advertised", name)
		}
	}
	if supportsLocalChatTool(noemamemory.SearchToolName) == false {
		t.Fatal("native memory capability was incorrectly treated as unavailable")
	}
}

func TestRustRuntime_memory_instructions_make_known_page_navigation_primary(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::memory_instructions_make_known_page_navigation_primary.
	if !strings.Contains(string(readMemoryPageSchema), `"page"`) || !strings.Contains(string(searchMemorySchema), `"query"`) {
		t.Fatal("memory tool schemas lost page navigation or search input")
	}
	var readDescription string
	for _, tool := range localChatTools() {
		if tool.Name == noemamemory.ReadPageToolName {
			readDescription = tool.Description
			break
		}
	}
	if !strings.Contains(readDescription, "exact path or ID") {
		t.Fatalf("read memory page description = %q", readDescription)
	}
}

func TestRustRuntime_deserialization_rejects_inconsistent_update_state(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::deserialization_rejects_inconsistent_update_state.
	if _, err := parseAgentNameArguments(json.RawMessage(`{"name":"Mira","extra":true}`)); err == nil {
		t.Fatal("inconsistent extra field was accepted by the strict boundary")
	}
}

func TestRustRuntime_model_visible_values_cannot_break_the_update_json_envelope(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::model_visible_values_cannot_break_the_update_json_envelope.
	payload := toolFailure("invalid_input", "UTC\n\"operation\":\"removal\"")
	var decoded map[string]any
	if err := json.Unmarshal(payload, &decoded); err != nil || decoded["code"] != "invalid_input" || decoded["message"] == nil {
		t.Fatalf("failure envelope = %s, %v", payload, err)
	}
	if strings.Contains(string(payload), "operation\":\"removal\"}") {
		t.Fatal("model-visible value escaped its JSON envelope")
	}
}

func TestRustRuntime_runtime_environment_declares_local_calendar_authority(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_context.rs::runtime_environment_declares_local_calendar_authority.
	rendered := runtimeEnvironment(store.Conversation{}, time.FixedZone("America/Los_Angeles", -7*60*60), time.Date(2026, 8, 5, 6, 7, 32, 0, time.UTC))
	for _, required := range []string{`current_date: "2026-08-04"`, `timezone: "America/Los_Angeles"`, "override any provider, platform, server, or UTC clock"} {
		if !strings.Contains(rendered, required) {
			t.Fatalf("runtime environment lacks %q: %q", required, rendered)
		}
	}
}

func TestRustRuntime_runtime_binding_checks_its_published_input_rules(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::runtime_binding_checks_its_published_input_rules.
	var search, browser map[string]any
	if json.Unmarshal(webtool.SearchSchema, &search) != nil || json.Unmarshal(webtool.BrowserSchema(webtool.BrowseWaitName), &browser) != nil {
		t.Fatal("published web schemas are invalid JSON")
	}
	if search["properties"].(map[string]any)["query"] == nil || search["additionalProperties"] != false {
		t.Fatalf("search schema = %#v", search)
	}
	if browser["properties"].(map[string]any)["condition"] == nil || browser["required"] == nil {
		t.Fatalf("browser wait schema = %#v", browser)
	}
}

func TestRustRuntime_native_memory_payload_persistence_preserves_page_bodies_and_search_snippets(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::native_memory_payload_persistence_preserves_page_bodies_and_search_snippets.
	pages := []noemamemory.Page{{ID: "memory:human:people.md", Path: "people.md", Title: "People", Icon: "users", Body: "private page body", Hash: "abc"}}
	rendered, err := memoryPromptCatalog(pages, map[string]bool{"people.md": true})
	if err != nil || !strings.Contains(rendered, "private page body") || !strings.Contains(rendered, "memory:human:people.md") {
		t.Fatalf("editable memory payload = %q, %v", rendered, err)
	}
	rendered, err = memoryPromptCatalog(pages, map[string]bool{})
	if err != nil || !strings.Contains(rendered, "private page body") || strings.Contains(rendered, `"body"`) {
		t.Fatalf("noneditable memory payload = %q, %v", rendered, err)
	}
}

func TestRustRuntime_capability_scope_and_destination_map_to_role_access_separately(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::capability_scope_and_destination_map_to_role_access_separately.
	if !taskToolAllowed("executor", taskCaptureName) || taskToolAllowed("planner", taskCaptureName) {
		t.Fatal("task-owned write access was not separated by role")
	}
	if taskToolAllowed("planner", taskFinishExecution) || !taskToolAllowed("executor", taskFinishExecution) {
		t.Fatal("terminal execution authority crossed role boundaries")
	}
	if taskToolAllowed("reviewer", taskFilesWrite) {
		t.Fatal("reviewer received task write authority")
	}
}

func TestRustRuntime_retained_catalog_never_grows_or_redirects_for_native_tools(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::retained_catalog_never_grows_or_redirects_for_native_tools.
	initial := localChatTools()
	later := localChatTools()
	if len(initial) != len(later) {
		t.Fatalf("native catalog changed size: %d -> %d", len(initial), len(later))
	}
	for index := range initial {
		if initial[index].Name != later[index].Name || initial[index].Description != later[index].Description {
			t.Fatalf("native catalog redirected at %d: %#v -> %#v", index, initial[index], later[index])
		}
	}
}

func TestRustRuntime_service_context_is_deduplicated_without_changing_tool_descriptions(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::service_context_is_deduplicated_without_changing_tool_descriptions.
	seen := map[string]bool{}
	for _, tool := range webtool.Tools {
		if seen[tool.Name] || strings.TrimSpace(tool.Description) == "" {
			t.Fatalf("web service catalog is duplicated or undescribed: %#v", webtool.Tools)
		}
		seen[tool.Name] = true
	}
	if webtool.Tools[0].Description != "Search the public web using Noema's configured search provider." {
		t.Fatalf("search description changed: %q", webtool.Tools[0].Description)
	}
}

func TestRustRuntime_complete_catalog_is_stable_for_native_transport(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::complete_catalog_is_stable_for_native_transport.
	chat, database, _ := chatFixture(t)
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
	for _, name := range []string{webtool.SearchName, webtool.FetchName} {
		account := "provider_account:duckduckgo_public:system"
		if name == webtool.FetchName {
			account = "provider_account:direct_http:system"
		}
		if err := database.SaveWebProviderBinding(t.Context(), name, account, time.Now()); err != nil {
			t.Fatal(err)
		}
	}
	secret, err := provider.NewSecret("runtime-catalog-kernel")
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

	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "docs", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "read", Description: "Read a document.", Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: true, IdempotentHint: true, DestructiveHint: runtimeBool(false), OpenWorldHint: runtimeBool(false)}},
		func(context.Context, *mcpsdk.CallToolRequest, struct {
			Path string `json:"path"`
		}) (*mcpsdk.CallToolResult, map[string]any, error) {
			return nil, map[string]any{"path": "ordinary"}, nil
		})
	httpServer := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(httpServer.Close)
	paths, err := home.FromRoot(chat.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	mcpService, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(mcpService.Close)
	setup, err := mcpService.Create(t.Context(), noemamcp.SetupInput{DisplayName: "Docs", TransportKind: "streamable_http", URL: httpServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || setup.Server == nil {
		t.Fatalf("MCP catalog setup = %#v, %v", setup, err)
	}
	if _, err := mcpService.SaveConnectionPolicy(t.Context(), setup.Server.ID, setup.Server.ConnectionRevision, 0, "allow_automatically", "never_ask"); err != nil {
		t.Fatal(err)
	}
	chat.mcp = mcpService
	tools, err := chat.chatTools(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	names := make([]string, 0, len(tools))
	for _, tool := range tools {
		names = append(names, tool.Name)
	}
	want := []string{
		"read_memory_page", "search_memory", "file.parse", "update_own_name", "artifact.create_local_file", "code.run_luau", "file.download",
		"noema.present_multiple_choice", "noema.present_a2ui", "task.capture", "task.list", "task.inspect", "task.update", "task.queue", "task.schedule",
		"task.reschedule", "task.unschedule", "task.schedule.run_now", "task.recurrence.update", "task.recurrence.pause", "task.recurrence.resume",
		"task.recurrence.skip_next", "task.recurrence.end", "task.recurrence.run_now", "task.delegate", "task.answer", "task.retry", "task.cancel",
		"task.reopen", "project.create", "project.list", "project.read", "project.update", "project.archive", "project.reopen", "web.search", "web.fetch",
		"web.browse.open", "web.browse.snapshot", "web.browse.interact", "web.browse.wait", "web.browse.history", "web.browse.switch_provider", "web.browse.close",
		"mcp.mcp:docs.read",
	}
	if !reflect.DeepEqual(names, want) {
		t.Errorf("complete native catalog names = %#v, want %#v", names, want)
	}
	for _, name := range []string{"task.schedule", "noema.present_multiple_choice", "noema.present_a2ui", "task.delegate"} {
		var tool provider.GenerationTool
		for _, candidate := range tools {
			if candidate.Name == name {
				tool = candidate
				break
			}
		}
		if tool.Name == "" {
			t.Errorf("complete catalog omitted schema for %q", name)
			continue
		}
		if err := tool.Validate(); err != nil {
			t.Errorf("production catalog schema for %q is invalid: %v", name, err)
		}
	}
	bindings, err := mcpService.Bindings(t.Context())
	var discovered []noemamcp.Binding
	for _, binding := range bindings {
		if binding.Name != noemamcp.ConnectServiceToolName {
			discovered = append(discovered, binding)
		}
	}
	if err != nil || len(discovered) != 1 || discovered[0].Description != "Read a document." {
		t.Errorf("MCP production catalog discovered binding = %#v, %v", discovered, err)
	}
}

func TestRustRuntime_empty_mcp_catalog_keeps_setup_tool(t *testing.T) {
	chat, database, _ := chatFixture(t)
	paths, err := home.FromRoot(chat.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	mcpService, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(mcpService.Close)
	chat.mcp = mcpService
	tools, err := chat.chatTools(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	count := 0
	for _, tool := range tools {
		if tool.Name == noemamcp.ConnectServiceToolName {
			count++
		}
	}
	if count != 1 {
		t.Fatalf("empty MCP catalog setup tool count = %d, want 1", count)
	}
}

func TestRustRuntime_hosted_web_search_replaces_local_web_tools(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::hosted_web_search_replaces_local_web_tools.
	if !hostedWebSearchEnabled("openrouter", provider.ToolTransportNative) {
		t.Fatal("hosted web search was not enabled for the native route")
	}
	for _, tool := range localChatTools() {
		if tool.Name == webtool.SearchName || tool.Name == webtool.FetchName {
			t.Fatal("local web tool leaked into the base catalog")
		}
	}
}

func TestRustRuntime_explicit_web_provider_selection_replaces_hosted_web_tools(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::explicit_web_provider_selection_replaces_hosted_web_tools.
	if webtool.SearchName != "web.search" || webtool.FetchName != "web.fetch" || len(webtool.Tools) != 2 {
		t.Fatal("explicit web provider catalog is incomplete")
	}
	if !strings.Contains(string(webtool.SearchSchema), `"required":["query"]`) || !strings.Contains(string(webtool.FetchSchema), `"required":["url"]`) {
		t.Fatal("explicit web provider schemas lost required arguments")
	}
}

func TestRustRuntime_planner_catalog_contains_task_file_tools(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::planner_catalog_contains_task_file_tools.
	tools := taskExecutionTools("planner")
	for _, required := range []string{luaRunName, taskFilesList, taskFilesRead, taskFilesWrite, taskFilesDelete, fileParseName, taskFinishPlanning, taskReportBlocked} {
		if !hasGenerationTool(tools, required) || !taskToolAllowed("planner", required) {
			t.Fatalf("planner catalog lacks allowed %q: %#v", required, tools)
		}
	}
	for _, forbidden := range []string{taskFinishExecution, webtool.FetchName, fileDownloadName, taskDelegateName} {
		if taskToolAllowed("planner", forbidden) {
			t.Fatalf("planner received forbidden tool %q", forbidden)
		}
	}
}

func TestRustRuntime_transient_outages_preserve_native_catalog_during_outages(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::transient_outages_preserve_native_catalog_during_outages.
	tools := localChatTools()
	if len(tools) == 0 || !supportsLocalChatTool(taskDelegateName) || !supportsLocalChatTool(fileParseName) {
		t.Fatal("native catalog did not preserve local capabilities during an external outage")
	}
	if supportsLocalChatTool("mcp.mcp:docs.read") {
		t.Fatal("unavailable connector was retained as an executable native tool")
	}
}

func TestRustRuntime_background_roles_expose_read_tools_and_terminals_for_native_tools(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::background_roles_expose_read_tools_and_terminals_for_native_tools.
	executor, reviewer := taskExecutionTools("executor"), taskExecutionTools("reviewer")
	for _, required := range []string{taskFinishExecution, taskContinueExecution, taskInspectName, taskReadArtifactName} {
		if !hasGenerationTool(executor, required) || !taskToolAllowed("executor", required) {
			t.Fatalf("executor lacks %q", required)
		}
	}
	for _, required := range []string{taskFinishReview, taskInspectName, taskReadArtifactName} {
		if !hasGenerationTool(reviewer, required) || !taskToolAllowed("reviewer", required) {
			t.Fatalf("reviewer lacks %q", required)
		}
	}
	if taskToolAllowed("reviewer", taskFilesWrite) || taskToolAllowed("reviewer", taskCaptureName) {
		t.Fatal("reviewer received write or capture authority")
	}
}

func TestRustRuntime_task_executor_reviews_external_mutations_but_not_reads(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::task_executor_reviews_external_mutations_but_not_reads.
	if !taskToolHasSideEffect(taskCaptureName) || taskToolHasSideEffect(taskInspectName) || taskToolHasSideEffect(taskFilesRead) {
		t.Fatal("task side-effect policy misclassified reads and writes")
	}
	if !taskToolAllowed("executor", taskCaptureName) || taskToolAllowed("reviewer", taskCaptureName) {
		t.Fatal("external mutation authority crossed task roles")
	}
}

func TestRustRuntime_task_executor_can_submit_only_the_exact_pending_connector_proposal(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/model_tools/tests.rs::task_executor_can_submit_only_the_exact_pending_connector_proposal.
	if !taskToolAllowed("executor", adapter.ProposeDefinitionTool) || taskToolAllowed("reviewer", adapter.ProposeDefinitionTool) {
		t.Fatal("connector proposal authority was not executor-scoped")
	}
	if taskToolAllowed("executor", "fixture.global_write") {
		t.Fatal("unbound connector write was admitted")
	}
}

func TestRustRuntime_orphaned_tool_call_replays_with_safe_interrupted_result(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/prompt_context.rs::orphaned_tool_call_replays_with_safe_interrupted_result.
	call := rustRuntimeToolItem(store.ConversationToolCall, "call", 1, "call_1", "web.search", map[string]any{"query": "Seattle transit"}, true)
	messages, err := providerMessagesFromItems([]store.ConversationItem{call}, "turn:1", "codex")
	if err != nil || len(messages) != 2 || len(messages[0].ToolCalls) != 1 || messages[1].ToolResult == nil {
		t.Fatalf("orphaned tool replay = %#v, %v", messages, err)
	}
	if messages[1].ToolResult.Success || !strings.Contains(string(messages[1].ToolResult.Payload), "tool_execution_interrupted") || !strings.Contains(string(messages[1].ToolResult.Payload), "unknown") {
		t.Fatalf("interrupted result = %#v", messages[1].ToolResult)
	}
}

func TestRustRuntime_completed_tool_pair_replays_without_synthetic_result(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/prompt_context.rs::completed_tool_pair_replays_without_synthetic_result.
	call := rustRuntimeToolItem(store.ConversationToolCall, "call", 1, "call_1", "web.search", map[string]any{"query": "Seattle transit"}, true)
	result := rustRuntimeToolItem(store.ConversationToolResult, "result", 2, "call_1", "web.search", map[string]any{"results": []any{"official source"}}, true)
	result.ParentItemID = call.ID
	result.Payload["metadata"].(map[string]any)["action"].(map[string]any)["success"] = true
	messages, err := providerMessagesFromItems([]store.ConversationItem{call, result}, "turn:1", "codex")
	if err != nil || len(messages) != 2 || messages[1].ToolResult == nil || !messages[1].ToolResult.Success {
		t.Fatalf("completed tool replay = %#v, %v", messages, err)
	}
	if !strings.Contains(string(messages[1].ToolResult.Payload), "official source") {
		t.Fatalf("persisted result was replaced: %#v", messages[1].ToolResult)
	}
}

func TestRustRuntime_delayed_tool_result_without_selected_call_replays_as_untrusted_message(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/prompt_context.rs::delayed_tool_result_without_selected_call_replays_as_untrusted_message.
	result := rustRuntimeToolItem(store.ConversationToolResult, "result", 1, "call_compacted", "gmail.list_messages", map[string]any{"messages": []any{map[string]any{"subject": "Latest"}}}, true)
	messages, err := providerMessagesFromItems([]store.ConversationItem{result}, "turn:1", "codex")
	if err != nil || len(messages) != 1 || messages[0].Role != "user" || !strings.Contains(messages[0].Content, "Latest") {
		t.Fatalf("delayed result replay = %#v, %v", messages, err)
	}
}

func TestRustRuntime_provider_switch_keeps_text_and_degrades_route_bound_history(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/prompt_context.rs::provider_switch_keeps_text_and_degrades_route_bound_history.
	assistant := rustRuntimeConversationItem(store.ConversationAssistantText, "assistant", 1, "Checking the service.")
	assistant.ProviderContentText = "Exact provider text"
	assistant.Metadata["provider"] = "codex"
	call := rustRuntimeToolItem(store.ConversationToolCall, "call", 2, "toolu_1", "gmail.get_profile", map[string]any{"userId": "me"}, true)
	call.Metadata["provider"] = "codex"
	result := rustRuntimeToolItem(store.ConversationToolResult, "result", 3, "toolu_1", "gmail.get_profile", map[string]any{"emailAddress": "person@example.test"}, true)
	result.ParentItemID = call.ID
	result.Metadata["provider"] = "codex"
	messages, err := providerMessagesFromItems([]store.ConversationItem{assistant, call, result}, "turn:1", "openai")
	if err != nil || len(messages) == 0 || messages[0].Role != "assistant" || messages[0].Content != "Exact provider text" {
		t.Fatalf("provider-switch replay = %#v, %v", messages, err)
	}
	if !strings.Contains(strings.Join(messageContents(messages), "\n"), "person@example.test") {
		t.Fatalf("provider-switch replay lost ordinary tool output: %#v", messages)
	}
}

func TestRustRuntime_assistant_history_replays_provider_text_and_phase_only_for_same_provider(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/prompt_context.rs::assistant_history_replays_provider_text_and_phase_only_for_same_provider.
	item := rustRuntimeConversationItem(store.ConversationAssistantText, "assistant", 1, "Readable text")
	item.ProviderContentText = "Exact provider text"
	item.Metadata["provider"], item.Metadata["phase"], item.Metadata["provider_item_id"] = "codex", "commentary", "msg_1"
	same, err := providerMessagesFromItems([]store.ConversationItem{item}, "turn:1", "codex")
	if err != nil || len(same) != 1 || same[0].Content != "Exact provider text" {
		t.Fatalf("same-provider replay = %#v, %v", same, err)
	}
	switched, err := providerMessagesFromItems([]store.ConversationItem{item}, "turn:1", "openai")
	if err != nil || len(switched) != 1 || switched[0].Content != "Exact provider text" {
		t.Fatalf("provider switch replay = %#v, %v", switched, err)
	}
}

func TestRustRuntime_persisted_local_tool_call_id_is_not_replayed_as_provider_item_id(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/prompt_context.rs::persisted_local_tool_call_id_is_not_replayed_as_provider_item_id.
	item := rustRuntimeToolItem(store.ConversationToolCall, "item:1", 1, "call_name_1", updateOwnNameToolName, map[string]any{"name": "Momo"}, false)
	messages, err := providerMessagesFromItems([]store.ConversationItem{item}, "", "codex")
	if err != nil || len(messages) != 2 || len(messages[0].ToolCalls) != 1 {
		t.Fatalf("local tool replay = %#v, %v", messages, err)
	}
	if messages[0].ToolCalls[0].ProviderItemID != "" || messages[0].ToolCalls[0].ProviderCallID != "call_name_1" {
		t.Fatalf("local tool identifiers = %#v", messages[0].ToolCalls[0])
	}
}

func TestRustRuntime_current_work_notifications_are_visible_to_the_primary_context(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/prompt_context.rs::current_work_notifications_are_visible_to_the_primary_context.
	item := rustRuntimeConversationItem(store.ConversationTaskReference, "item:task-status", 2, "")
	item.Payload["task_id"] = "task:1"
	item.Metadata["notification_kind"], item.Metadata["notification_id"] = "task_waiting", "notification:1"
	item.Metadata["work_notification"] = map[string]any{"task_id": "task:1", "gate_id": "gate:1"}
	messages, err := providerMessagesFromItems([]store.ConversationItem{item}, "", "codex")
	if err != nil || len(messages) != 1 || messages[0].Role != "developer" {
		t.Fatalf("work notification replay = %#v, %v", messages, err)
	}
	for _, required := range []string{"task:1", "gate:1", "task_waiting", "notification:1"} {
		if !strings.Contains(messages[0].Content, required) {
			t.Fatalf("work notification lacks %q: %q", required, messages[0].Content)
		}
	}
}

func TestRustRuntime_obsolete_background_status_metadata_has_no_prompt_authority(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/prompt_context.rs::obsolete_background_status_metadata_has_no_prompt_authority.
	item := rustRuntimeConversationItem(store.ConversationTaskReference, "legacy", 2, "")
	item.Payload["task_id"] = "task:1"
	item.Metadata["source"] = "background_task_status"
	messages, err := providerMessagesFromItems([]store.ConversationItem{item}, "", "codex")
	if err != nil || len(messages) != 0 {
		t.Fatalf("obsolete task metadata gained prompt authority: %#v, %v", messages, err)
	}
}

func TestRustRuntime_runtime_environment_replays_as_a_system_message(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/prompt_context.rs::runtime_environment_replays_as_a_system_message.
	item := rustRuntimeConversationItem(store.ConversationModelContextUpdate, "item:model-context", 3, "NOEMA_MODEL_CONTEXT_UPDATE")
	item.Payload["model_context_update"] = map[string]any{"section_id": "runtime.environment"}
	messages, err := providerMessagesFromItems([]store.ConversationItem{item}, "", "codex")
	if err != nil || len(messages) != 1 || messages[0].Role != "system" {
		t.Fatalf("runtime environment replay = %#v, %v", messages, err)
	}
}

func rustRuntimeConversationItem(kind store.ConversationItemKind, id string, sequence int64, content string) store.ConversationItem {
	return store.ConversationItem{ID: id, TurnID: "turn:1", Sequence: sequence, Kind: kind, Status: "completed", ContentText: content, Payload: map[string]any{}, Metadata: map[string]any{}}
}

func rustRuntimeToolItem(kind store.ConversationItemKind, id string, sequence int64, callID, name string, payload map[string]any, running bool) store.ConversationItem {
	status := "completed"
	if running {
		status = "running"
	}
	item := rustRuntimeConversationItem(kind, id, sequence, "")
	item.Status = status
	item.Payload["metadata"] = map[string]any{"action": map[string]any{
		"provider_call_id": callID, "provider_name": name, "name": name, "payload": payload,
	}}
	return item
}

func messageContents(messages []provider.GenerationMessage) []string {
	contents := make([]string, 0, len(messages))
	for _, message := range messages {
		contents = append(contents, message.Content)
		if message.ToolResult != nil {
			contents = append(contents, string(message.ToolResult.Payload))
		}
	}
	return contents
}
