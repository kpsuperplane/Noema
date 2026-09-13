package runtime

import (
	"context"
	"encoding/json"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestModelContextSavedChangesAndRemovals(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	turn, _, err := database.BeginConversationTurn(t.Context(), conversation.ID, "hello", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	sections := []provider.GenerationMessage{
		{Role: "system", Instructions: true, Content: "Instructions"},
		modelContextSectionMessage("agent.identity", "Noema"),
		modelContextSectionMessage("runtime.environment", "time one"),
		modelContextSectionMessage("projects.catalog", "private project /work/alpha"),
		modelContextSectionMessage("tools.visibility", "read"),
	}
	read := func() store.ConversationContext {
		t.Helper()
		state, err := database.ConversationProviderContext(t.Context(), conversation.ID, "openrouter", "")
		if err != nil {
			t.Fatal(err)
		}
		return state
	}
	state := read()
	base, delta, _, err := chat.syncModelContext(turn, &state, sections, false)
	if err != nil || len(delta) != 4 || len(base) != 1 {
		t.Fatalf("initial sync: %d, %d, %v", len(delta), len(base), err)
	}
	original, _, _, err := chatContextParts(read(), "", "openrouter")
	if err != nil {
		t.Fatal(err)
	}
	state = read()
	_, delta, _, err = chat.syncModelContext(turn, &state, sections, false)
	if err != nil || len(delta) != 0 {
		t.Fatalf("unchanged sync: %d, %v", len(delta), err)
	}
	sections[2] = modelContextSectionMessage("runtime.environment", "time two")
	sections = append(sections[:3], sections[4:]...)
	state = read()
	_, delta, _, err = chat.syncModelContext(turn, &state, sections, false)
	if err != nil || len(delta) != 2 {
		t.Fatalf("changed sync: %d, %v", len(delta), err)
	}
	if !strings.Contains(delta[0].Content, `"operation":"replacement"`) || !strings.Contains(delta[1].Content, `"operation":"removal"`) {
		t.Fatalf("changes: %#v", delta)
	}
	replay, _, _, err := chatContextParts(read(), "", "openrouter")
	if err != nil || !reflect.DeepEqual(replay[:len(original)], original) || !reflect.DeepEqual(replay[len(original):], delta) {
		t.Fatalf("saved prefix or changes lost: %v", err)
	}
	if !strings.Contains(original[3].Content, "private project /work/alpha") {
		t.Fatal("original private context changed")
	}
	if _, err := database.CompleteConversationTurn(t.Context(), turn, "done", "done", nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	sections[2] = modelContextSectionMessage("runtime.environment", "time three")
	state = read()
	if _, _, _, err := chat.syncModelContext(turn, &state, sections, false); err == nil {
		t.Fatal("completed turn accepted a context change")
	}
}

func TestChatContextPrefixSurvivesNextTurn(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	requests := make(chan provider.GenerateRequest, 2)
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests <- request
		return provider.GenerationResult{Text: "reply"}, nil
	})
	events, err := chat.Subscribe(t.Context(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	zones := []string{"UTC", "America/Los_Angeles"}
	for i, text := range []string{"hello", "world"} {
		if _, err := chat.SendTurn(t.Context(), SendTurnInput{ConversationID: conversation.ID, Input: text, ClientTimeZone: &zones[i]}); err != nil {
			t.Fatal(err)
		}
		collectCompletedTurns(t, events, 1)
	}
	first, second := <-requests, <-requests
	if len(second.Messages) <= len(first.Messages) || !reflect.DeepEqual(first.Messages, second.Messages[:len(first.Messages)]) {
		t.Fatal("new turn rewrote the previous request prefix")
	}
	suffix := second.Messages[len(first.Messages):]
	if len(suffix) != 3 || suffix[0].Content != "reply" || suffix[1].Content != "world" || !strings.Contains(suffix[2].Content, `"operation":"replacement"`) || !strings.Contains(suffix[2].Content, "America/Los_Angeles") {
		t.Fatalf("new turn suffix: %#v", suffix)
	}
	state, err := database.ConversationProviderContext(t.Context(), conversation.ID, "openrouter", "")
	if err != nil {
		t.Fatal(err)
	}
	updates := 0
	for _, item := range state.Items {
		if item.Kind == store.ConversationModelContextUpdate {
			updates++
		}
	}
	if updates != 5 {
		t.Fatalf("saved section updates: %d", updates)
	}
}

func TestModelContextCompactionAndResetRestoreSections(t *testing.T) {
	database := contextTestStore(t, 4_000)
	chat := &Chat{ctx: t.Context(), database: database}
	conversation, err := database.EnsurePrimaryConversation(t.Context(), "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(t.Context(), conversation.ID, "old", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	sections := []provider.GenerationMessage{modelContextSectionMessage("runtime.environment", "current time"), modelContextSectionMessage("tools.visibility", "read")}
	state, err := database.ConversationProviderContext(t.Context(), conversation.ID, "openrouter", "test")
	if err != nil {
		t.Fatal(err)
	}
	if _, _, _, err = chat.syncModelContext(turn, &state, sections, false); err != nil {
		t.Fatal(err)
	}
	if _, err = database.CompleteConversationTurn(t.Context(), turn, strings.Repeat("old history ", 1500), "", nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	turn, _, err = database.BeginConversationTurn(t.Context(), conversation.ID, "continue", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	state, err = database.ConversationProviderContext(t.Context(), conversation.ID, "openrouter", "test")
	if err != nil {
		t.Fatal(err)
	}
	base, _, snapshot, err := chat.syncModelContext(turn, &state, sections, false)
	if err != nil {
		t.Fatal(err)
	}
	completed, active, through, err := chatContextParts(state, turn.ID, "openrouter")
	if err != nil {
		t.Fatal(err)
	}
	messages, compacted, err := prepareModelContext(t.Context(), modelContextRequest{
		database: database, accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test",
		generator: generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
			return provider.GenerationResult{Text: "Earlier history"}, nil
		}),
		base: base, completed: completed, active: active, restoredContext: snapshot, outputReserve: 512,
		persist: func(summary string, recent []provider.GenerationMessage) error {
			if err := database.AppendConversationContextUpdate(t.Context(), turn, "openrouter", "test", summary, recent, through, time.Now()); err != nil {
				return err
			}
			_, _, _, err := chat.syncModelContext(turn, &state, sections, true)
			return err
		},
	})
	if err != nil || !compacted {
		t.Fatalf("compaction: %t, %v", compacted, err)
	}
	state, err = database.ConversationProviderContext(t.Context(), conversation.ID, "openrouter", "test")
	if err != nil {
		t.Fatal(err)
	}
	completed, active, _, err = chatContextParts(state, turn.ID, "openrouter")
	if err != nil || !reflect.DeepEqual(messages, joinContextMessages(base, completed, active)) {
		t.Fatalf("compacted request and saved replay differ: %v", err)
	}
	_, delta, _, err := chat.syncModelContext(turn, &state, sections, false)
	if err != nil || len(delta) != 0 {
		t.Fatalf("restored context was not saved: %v", err)
	}
	if _, err = database.CompleteConversationTurn(t.Context(), turn, "done", "done", nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	if _, err = database.AppendConversationContextReset(t.Context(), conversation.ID, nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	turn, _, err = database.BeginConversationTurn(t.Context(), conversation.ID, "new", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	state, err = database.ConversationProviderContext(t.Context(), conversation.ID, "openrouter", "test")
	if err != nil {
		t.Fatal(err)
	}
	_, delta, _, err = chat.syncModelContext(turn, &state, sections, false)
	if err != nil || len(delta) != 2 {
		t.Fatalf("reset: %d, %v", len(delta), err)
	}
	for _, m := range delta {
		var u store.ModelContextUpdate
		if json.Unmarshal([]byte(strings.TrimPrefix(m.Content, "NOEMA_MODEL_CONTEXT_UPDATE\n")), &u) != nil || u.Operation != "full" {
			t.Fatal("reset retained an earlier snapshot")
		}
	}
}
