package store

import (
	"context"
	"testing"
	"time"
)

func TestMemorySourceRangeDoesNotAdvanceAcrossRunningEligibleItem(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Remember this", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.db.ExecContext(ctx, `
INSERT INTO conversation_items (
  item_id, conversation_id, turn_id, sequence_index, kind, status,
  author_actor_id, content_text, payload_json, metadata_json, created_at_ms, updated_at_ms
) VALUES ('item:11111111111111111111111111111111', ?, ?, 2, 'assistant_text', 'running',
  'agent:primary', 'Pending answer', '{}', '{}', 1, 1)`, conversation.ID, turn.ID); err != nil {
		t.Fatal(err)
	}
	first, err := database.CaptureMemorySourceRange(ctx, conversation.ID, 0)
	if err != nil {
		t.Fatal(err)
	}
	if first.CapturedHead != 1 || len(first.Items) != 1 {
		t.Fatalf("first Memory range = %#v", first)
	}
	if _, err := database.db.ExecContext(ctx, `
UPDATE conversation_items SET status = 'completed'
WHERE item_id = 'item:11111111111111111111111111111111'`); err != nil {
		t.Fatal(err)
	}
	second, err := database.CaptureMemorySourceRange(ctx, conversation.ID, first.CapturedHead)
	if err != nil {
		t.Fatal(err)
	}
	if second.CapturedHead != 2 || len(second.Items) != 1 || second.Items[0].ID != "item:11111111111111111111111111111111" {
		t.Fatalf("second Memory range = %#v", second)
	}
}
