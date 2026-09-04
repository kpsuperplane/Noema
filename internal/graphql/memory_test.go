package graphql

import (
	"context"
	"net/http/httptest"
	"testing"
	"time"

	noemamemory "github.com/kpsuperplane/noema/internal/memory"
)

func TestMemoryGraphQLPreservesReadContractsAndInitialEvent(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	ctx := context.Background()
	conversation, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	turn, human, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "Alice likes tea.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	assistant, err := resolver.Store.CompleteConversationTurn(ctx, turn, "Noted.", "Noted.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	nativeMemory, err := noemamemory.New(resolver.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = nativeMemory.Close() })
	resolver.Memory = nativeMemory
	root, err := nativeMemory.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	if err := nativeMemory.Publish(noemamemory.ChangeSet{Upserts: []noemamemory.PageChange{
		{ID: root.ID, ExpectedHash: root.Hash, Path: noemamemory.RootPagePath, Title: root.Title, Icon: root.Icon, Body: "People and preferences."},
		{Path: "people.md", Title: "People", Icon: "users", Body: "Known people."},
		{Path: "people/alice.md", Title: "Alice", Icon: "user", Body: "Alice likes tea.[^1]", Citations: []noemamemory.Citation{{Sources: []string{human.ID, assistant.ID}}}},
	}}, noemamemory.State{}); err != nil {
		t.Fatal(err)
	}

	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)
	response := postGraphQL(t, server.URL, `query MemoryRead($pageId: String!) {
  memorySettings { modelPreference { providerKind providerAccountId modelProfile reasoningEffort selectionMode fastMode } modelOptions { providerKind providerAccountId } }
  memoryTree { root { id path title icon body hash citations { sources { source kind excerpt createdAt } } parent ancestors { id path title } children { id path title icon excerpt hash } } pages { id path title icon excerpt hash } pendingCount updateStatus { state active lastConsolidatedSequence lastConsolidatedItem error updatedAt } }
  memoryPage(pageId: $pageId) { id path title icon body hash citations { sources { source kind excerpt createdAt } } parent ancestors { id path title icon excerpt hash } children { id path title icon excerpt hash } }
}`, map[string]any{"pageId": "people/alice.md"})
	if len(response.Errors) != 0 {
		t.Fatalf("Memory GraphQL errors = %#v", response.Errors)
	}
	settings := response.Data["memorySettings"].(map[string]any)
	preference := settings["modelPreference"].(map[string]any)
	if preference["providerKind"] != "openrouter" || len(settings["modelOptions"].([]any)) == 0 {
		t.Fatalf("Memory settings = %#v", settings)
	}
	tree := response.Data["memoryTree"].(map[string]any)
	if tree["pendingCount"] != float64(2) || len(tree["pages"].([]any)) != 3 {
		t.Fatalf("Memory tree = %#v", tree)
	}
	page := response.Data["memoryPage"].(map[string]any)
	if page["parent"] != "people.md" || len(page["ancestors"].([]any)) != 1 {
		t.Fatalf("Memory page = %#v", page)
	}
	citations := page["citations"].([]any)
	source := citations[0].(map[string]any)["sources"].([]any)[0].(map[string]any)
	if source["kind"] != "HUMAN_MESSAGE" || source["excerpt"] != "Alice likes tea." {
		t.Fatalf("Memory citation source = %#v", source)
	}
	unavailable := citations[0].(map[string]any)["sources"].([]any)[1].(map[string]any)
	if unavailable["kind"] != "UNAVAILABLE" || unavailable["createdAt"] == nil {
		t.Fatalf("unavailable Memory citation = %#v", unavailable)
	}

	eventContext, cancel := context.WithCancel(context.Background())
	events, err := resolver.memoryEvents(eventContext)
	if err != nil {
		t.Fatal(err)
	}
	initial := <-events
	if initial.Root == nil || initial.Root.Path != noemamemory.RootPagePath || initial.PendingCount != 2 {
		t.Fatalf("initial Memory event = %#v", initial)
	}
	cancel()
	if _, open := <-events; open {
		t.Fatal("Memory event stream stayed open")
	}
}
