package runtime

import (
	"context"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestChatOutputPreservesMessagePhasesAndReasoningDisplay(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	turn, _, err := database.BeginConversationTurn(context.Background(), conversation.ID, "hello", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	stream := chat.outputStream(turn, 0, nil)
	stream.event(provider.StreamEvent{Kind: provider.MessageStarted, Index: 0, ID: "m1", Phase: "commentary"})
	stream.event(provider.StreamEvent{Kind: provider.TextDelta, Index: 0, Delta: "Checking."})
	stream.event(provider.StreamEvent{Kind: provider.MessageCompleted, Index: 0, Text: "Checking.", Phase: "commentary"})
	stream.event(provider.StreamEvent{Kind: provider.ReasoningDelta, Index: 1, SectionIndex: 0, Delta: "Readable reasoning"})
	stream.event(provider.StreamEvent{Kind: provider.ReasoningCompleted, Index: 1, SectionIndex: 0, Text: "Readable reasoning"})
	stream.event(provider.StreamEvent{Kind: provider.MessageCompleted, Index: 2, ID: "m2", Phase: "final_answer", Text: "Done."})
	start, end := 9, 14
	result := provider.GenerationResult{Text: "Checking.Done.", Citations: []provider.Citation{{URL: "https://example.com", StartIndex: &start, EndIndex: &end}}}
	if err := stream.finish(&result, nil); err != nil {
		t.Fatal(err)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Items) != 4 || page.Items[2].ContentText != "Readable reasoning" {
		t.Fatalf("readable output missing: %#v", page.Items)
	}
	if page.Items[1].Metadata["citations"] != nil {
		t.Fatal("final citation attached to progress")
	}
	citations, ok := page.Items[3].Metadata["citations"].([]any)
	if !ok || len(citations) != 1 {
		t.Fatal("final section citation missing")
	}
	citation := citations[0].(map[string]any)
	if citation["start_index"] != float64(0) || citation["end_index"] != float64(5) {
		t.Fatalf("citation offsets not rebased: %#v", citation)
	}
	page.Items = append(page.Items, store.ConversationItem{Kind: store.ConversationReasoning, TurnID: turn.ID, Metadata: map[string]any{"provider_round": float64(0), "provider": "codex"}, Payload: map[string]any{"provider_details": []any{map[string]any{"type": "reasoning", "encrypted_content": "opaque-test"}}}})
	replay, err := providerMessagesFromItems(page.Items, turn.ID, "codex")
	if err != nil {
		t.Fatal(err)
	}
	if len(replay) != 3 || replay[1].Content != "Checking." || replay[1].Phase != "commentary" || replay[2].Content != "Done." || replay[2].Phase != "final_answer" {
		t.Fatalf("replay lost message boundaries or phases: %#v", replay)
	}
	if len(replay[1].ReasoningDetails) != 1 || len(replay[2].ReasoningDetails) != 0 {
		t.Fatal("reasoning crossed response boundary")
	}
	if _, err := database.FailConversationTurn(context.Background(), turn, "Stopped", time.Now()); err != nil {
		t.Fatal(err)
	}
	result.Output[0].Text = "stale changed output"
	if err := stream.finish(&result, nil); err == nil {
		t.Fatal("failed turn accepted output")
	}
}
