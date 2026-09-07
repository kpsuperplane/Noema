package store

import (
	"context"
	"encoding/json"
	"fmt"
	"slices"
	"strings"
	"testing"
	"time"
)

func TestRustConversationAuthorizationExcerptContract(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 7, 12, 0, 0, 0, time.UTC)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turnID, err := newID("turn")
	if err != nil {
		t.Fatal(err)
	}
	metadata, _ := json.Marshal(map[string]any{})
	if _, err := database.db.ExecContext(ctx, `INSERT INTO conversation_turns
(turn_id, conversation_id, status, metadata_json, started_at_ms, created_at_ms, updated_at_ms)
VALUES (?, ?, 'input_received', ?, ?, ?, ?)`, turnID, conversation.ID, string(metadata), millis(now), millis(now), millis(now)); err != nil {
		t.Fatal(err)
	}

	kinds := []ConversationItemKind{
		ConversationUserText,
		ConversationAssistantText,
		ConversationMultipleChoicePrompt,
		ConversationMultipleChoiceSelection,
		ConversationAssistantText,
		ConversationUserText,
		ConversationAssistantText,
		ConversationAssistantText,
		ConversationUserText,
	}
	source := ""
	for index, kind := range kinds {
		author := "agent:primary"
		if kind == ConversationUserText || kind == ConversationMultipleChoiceSelection {
			author = "human:local"
		}
		source = insertConversationAuthorizationItem(t, database, conversation.ID, turnID,
			int64(index)+1, kind, author, fmt.Sprintf("message-%d", index))
	}
	insertConversationAuthorizationItem(t, database, conversation.ID, turnID,
		int64(len(kinds))+1, ConversationAssistantText, "agent:primary", "same-turn output")

	if _, err := database.ConversationAuthorizationContext(ctx, conversation.ID, "turn:wrong"); err == nil {
		t.Fatal("wrong turn unexpectedly supplied authorization context")
	}
	value, err := database.ConversationAuthorizationContext(ctx, conversation.ID, turnID)
	if err != nil {
		t.Fatal(err)
	}
	if value["kind"] != "conversation_excerpt" {
		t.Fatalf("authorization context kind = %#v", value["kind"])
	}
	messages, ok := value["messages"].([]map[string]any)
	if !ok {
		t.Fatalf("messages = %#v", value["messages"])
	}
	texts := make([]string, 0, len(messages))
	humanTexts := make([]string, 0)
	for _, message := range messages {
		text, ok := message["text"].(string)
		if !ok {
			t.Fatalf("message text = %#v", message["text"])
		}
		texts = append(texts, text)
		if message["role"] == "human" {
			humanTexts = append(humanTexts, text)
		}
	}
	if got, want := texts, []string{"message-2", "message-3", "message-4", "message-5", "message-6", "message-7", "message-8"}; !slices.Equal(got, want) {
		t.Fatalf("excerpt texts = %#v, want %#v", got, want)
	}
	if got, want := humanTexts, []string{"message-3", "message-5", "message-8"}; !slices.Equal(got, want) {
		t.Fatalf("human excerpt texts = %#v, want %#v", got, want)
	}
	if got := messages[len(messages)-1]["item_id"]; got != source {
		t.Fatalf("last excerpt item = %#v, want %q", got, source)
	}

	forged := insertConversationAuthorizationItem(t, database, conversation.ID, turnID,
		int64(len(kinds))+2, ConversationUserText, "agent:primary", "forged human kind")
	if _, err := database.ConversationAuthorizationContext(ctx, conversation.ID, turnID); err == nil {
		t.Fatalf("forged source %q unexpectedly supplied authorization context", forged)
	}
}

func TestRustConversationAuthorizationContextRejectsOversizedText(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 7, 12, 0, 0, 0, time.UTC)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, item, err := database.BeginConversationTurn(ctx, conversation.ID, "short", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.db.ExecContext(ctx, "UPDATE conversation_items SET content_text = ? WHERE item_id = ?", strings.Repeat("x", actionContextLimit), item.ID); err != nil {
		t.Fatal(err)
	}
	if _, err := database.ConversationAuthorizationContext(ctx, conversation.ID, turn.ID); err == nil {
		t.Fatal("oversized authorization context was accepted")
	}
}

func insertConversationAuthorizationItem(t *testing.T, database *Store, conversationID, turnID string, sequence int64, kind ConversationItemKind, author, content string) string {
	t.Helper()
	id, err := newID("item")
	if err != nil {
		t.Fatal(err)
	}
	payload, _ := json.Marshal(map[string]any{})
	metadata, _ := json.Marshal(map[string]any{})
	_, err = database.db.ExecContext(t.Context(), `INSERT INTO conversation_items (
item_id, conversation_id, turn_id, sequence_index, kind, status,
author_actor_id, content_text, payload_json, metadata_json, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, 'completed', ?, ?, ?, ?, ?, ?)`, id, conversationID, turnID,
		sequence, kind, author, content, string(payload), string(metadata), 1, 1)
	if err != nil {
		t.Fatal(err)
	}
	return id
}
