package runtime

import (
	"context"
	"encoding/json"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/webtool"
)

func TestExplicitWebToolsGateChatAndTaskRoles(t *testing.T) {
	chat, database, _ := chatFixture(t)
	accounts, err := provider.NewAccountService(t.TempDir(), database)
	if err != nil {
		t.Fatal(err)
	}
	if err := accounts.Initialize(context.Background(), time.Now()); err != nil {
		t.Fatal(err)
	}
	service, err := webtool.New(database, accounts, nil, "", 2, 1024)
	if err != nil {
		t.Fatal(err)
	}
	chat.web = service
	if service.Explicit(context.Background()) || !hostedWebSearchEnabled("openrouter", provider.ToolTransportNative) {
		t.Fatal("native hosted web default is unavailable")
	}
	if err := database.SaveWebProviderBinding(context.Background(), webtool.SearchName,
		"provider_account:duckduckgo_public:system", time.Now()); err != nil {
		t.Fatal(err)
	}
	if !service.Explicit(context.Background()) {
		t.Fatal("explicit binding was not detected")
	}
	chatTools, err := chat.chatTools(context.Background())
	if err != nil || !hasGenerationTool(chatTools, webtool.SearchName) || !hasGenerationTool(chatTools, webtool.FetchName) {
		t.Fatalf("Chat tools = %#v, %v", chatTools, err)
	}
	tasks := &TaskExecution{database: database, web: service}
	executor, _, _ := tasks.taskExecutionTools(context.Background(), "executor")
	planner, _, _ := tasks.taskExecutionTools(context.Background(), "planner")
	reviewer, _, _ := tasks.taskExecutionTools(context.Background(), "reviewer")
	if !hasGenerationTool(executor, webtool.SearchName) || hasGenerationTool(planner, webtool.SearchName) || hasGenerationTool(reviewer, webtool.SearchName) {
		t.Fatal("web tools do not match Task role policy")
	}
	browserService, err := webtool.New(database, accounts, nil, "/bin/false", 2, 1024)
	if err != nil {
		t.Fatal(err)
	}
	chat.web = browserService
	browserChatTools, err := chat.chatTools(context.Background())
	if err != nil || !hasGenerationTool(browserChatTools, webtool.BrowseOpenName) {
		t.Fatalf("browser Chat tools = %#v, %v", browserChatTools, err)
	}
	browserTasks := &TaskExecution{database: database, web: browserService}
	browserExecutor, _, _ := browserTasks.taskExecutionTools(context.Background(), "executor")
	browserPlanner, _, _ := browserTasks.taskExecutionTools(context.Background(), "planner")
	if !hasGenerationTool(browserExecutor, webtool.BrowseCloseName) || hasGenerationTool(browserPlanner, webtool.BrowseOpenName) {
		t.Fatal("browser tools do not match Task role policy")
	}
	chat.openRouter = generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: actionReviewToolName,
			Payload: json.RawMessage(`{"authorization":"weak","risk":"medium","reason_codes":["authorization_ambiguous"],"explanation":"Review is required."}`)}}}, nil
	})
	conversation, err := database.EnsurePrimaryConversation(context.Background(), "openrouter", "/workspace", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(context.Background(), conversation.ID, "Read the page", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(context.Background(), turn, store.ConversationToolRound{Provider: "openrouter",
		Call: store.ConversationToolCallInput{Name: webtool.FetchName, ProviderCallID: "fetch-review", ProviderName: webtool.FetchName, Arguments: json.RawMessage(`{"url":"https://1.1.1.1/private"}`)}}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	call := items[len(items)-1]
	assignment, err := chat.primaryAssignment(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	_, success, approval, err := chat.prepareWebFetchAction(conversation, turn, call, assignment, 0, json.RawMessage(`{"url":"https://1.1.1.1/private"}`))
	if err != nil || success || approval == nil {
		t.Fatalf("reviewed fetch = %v, %#v, %v", success, approval, err)
	}
	browserChat, browserDatabase, _ := chatFixture(t)
	browserAccounts, err := provider.NewAccountService(t.TempDir(), browserDatabase)
	if err != nil {
		t.Fatal(err)
	}
	if err := browserAccounts.Initialize(context.Background(), time.Now()); err != nil {
		t.Fatal(err)
	}
	browserChat.web, err = webtool.New(browserDatabase, browserAccounts, nil, "/bin/false", 2, 1024)
	if err != nil {
		t.Fatal(err)
	}
	browserChat.openRouter = chat.openRouter
	browserConversation, err := browserDatabase.EnsurePrimaryConversation(context.Background(), "openrouter", "/workspace", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	browserTurn, _, err := browserDatabase.BeginConversationTurn(context.Background(), browserConversation.ID, "Browse", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	browserItems, err := browserDatabase.StartConversationToolRound(context.Background(), browserTurn, store.ConversationToolRound{Provider: "openrouter",
		Call: store.ConversationToolCallInput{Name: webtool.BrowseOpenName, ProviderCallID: "browser-review", ProviderName: webtool.BrowseOpenName, Arguments: json.RawMessage(`{"url":"https://1.1.1.1/browser"}`)}}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	browserAssignment, err := browserChat.primaryAssignment(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	result, browserApproval, err := browserChat.prepareChatBrowser(browserConversation, browserTurn, browserItems[len(browserItems)-1], browserAssignment, 0, webtool.BrowseOpenName, json.RawMessage(`{"url":"https://1.1.1.1/browser"}`))
	if err != nil || result.Success || browserApproval == nil {
		t.Fatalf("reviewed browser = %#v, %#v, %v", result, browserApproval, err)
	}
}

func hasGenerationTool(tools []provider.GenerationTool, name string) bool {
	for _, tool := range tools {
		if tool.Name == name {
			return true
		}
	}
	return false
}
