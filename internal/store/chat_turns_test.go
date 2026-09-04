package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"path/filepath"
	"strings"
	"testing"
	"time"
	"unicode/utf8"
)

func TestConversationTurnSchemaFreshAndVersionSixUpgrade(t *testing.T) {
	database := openTestStore(t)
	var version int
	if err := database.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("fresh schema version = %d, %v", version, err)
	}
	for _, table := range []string{"conversation_turns", "conversation_items"} {
		var exists bool
		if err := database.db.QueryRow(`
SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?)`, table).Scan(&exists); err != nil || !exists {
			t.Fatalf("table %s exists = %v, %v", table, exists, err)
		}
	}

	path := filepath.Join(t.TempDir(), "version-six.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(schemaV4SQL + schemaV5SQL + schemaV6SQL + "\nPRAGMA user_version = 6;"); err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(`
INSERT INTO conversations (
  conversation_id, owner_human_id, provider, created_at_ms, updated_at_ms
) VALUES ('conversation:0123456789abcdef0123456789abcdef', 'human:local', 'openrouter', 1, 1)`); err != nil {
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}
	upgraded, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer upgraded.Close()
	var status string
	if err := upgraded.db.QueryRow(`
SELECT agent_status FROM conversations
WHERE conversation_id = 'conversation:0123456789abcdef0123456789abcdef'`).Scan(&status); err != nil || status != "idle" {
		t.Fatalf("upgraded conversation status = %q, %v", status, err)
	}
}

func TestConversationTurnCascadeAndConversationLinks(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Now()
	first, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, user, err := database.BeginConversationTurn(ctx, first.ID, "Delete this Chat", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CompleteConversationTurn(ctx, turn, "Done", "Done", nil, now); err != nil {
		t.Fatal(err)
	}
	secondID := "conversation:ffffffffffffffffffffffffffffffff"
	if _, err := database.db.Exec(`
INSERT INTO conversations (
  conversation_id, owner_human_id, provider, created_at_ms, updated_at_ms
) VALUES (?, 'human:local', 'openrouter', 1, 1)`, secondID); err != nil {
		t.Fatal(err)
	}
	if _, err := database.db.Exec(`
INSERT INTO conversation_items (
  item_id, conversation_id, parent_item_id, sequence_index, kind, status,
  author_actor_id, content_text, payload_json, metadata_json, created_at_ms, updated_at_ms
) VALUES (
  'item:ffffffffffffffffffffffffffffffff', ?, ?, 1, 'user_text', 'completed',
  'human:local', 'bad link', '{}', '{}', 1, 1
)`, secondID, user.ID); err == nil {
		t.Fatal("cross-conversation parent link succeeded")
	}
	if _, err := database.db.Exec("DELETE FROM conversations WHERE conversation_id = ?", first.ID); err != nil {
		t.Fatalf("delete completed conversation: %v", err)
	}
	for _, table := range []string{"conversation_turns", "conversation_items"} {
		var count int
		if err := database.db.QueryRow("SELECT COUNT(*) FROM "+table+" WHERE conversation_id = ?", first.ID).Scan(&count); err != nil || count != 0 {
			t.Fatalf("remaining %s rows = %d, %v", table, count, err)
		}
	}
}

func TestConversationTurnPersistsStreamReplacementAndReplay(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "chat.sqlite3")
	database, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	clientID := "client-message-1"
	turn, user, err := database.BeginConversationTurn(ctx, conversation.ID, "Hello", &clientID, now)
	if err != nil {
		t.Fatal(err)
	}
	if turn.TurnIndex != 1 || user.Kind != ConversationUserText ||
		user.Metadata["client_message_id"] != clientID || user.Cursor != "conversation_item:1" {
		t.Fatalf("started turn = %#v, %#v", turn, user)
	}
	if err := database.SetConversationAgentStatus(ctx, conversation.ID, "thinking", now); err != nil {
		t.Fatal(err)
	}
	assistant, err := database.CompleteConversationTurn(
		ctx, turn, "Hello back", "Hello back",
		&ProviderUsage{
			Provider: "openrouter", Model: "openai/gpt-5.6-luna",
			InputTokens: 10, OutputTokens: 2, TotalTokens: 12, CachedInputTokens: 5,
		}, now.Add(time.Second),
	)
	if err != nil {
		t.Fatal(err)
	}
	if assistant.Kind != ConversationAssistantText || assistant.ParentItemID != user.ID ||
		assistant.Cursor != "conversation_item:2" || assistant.Metadata["stream_id"] != assistantStreamID(turn.ID) {
		t.Fatalf("assistant item = %#v", assistant)
	}
	messages, err := database.ConversationProviderMessages(ctx, conversation.ID)
	if err != nil || len(messages) != 2 || messages[0].Role != "user" || messages[1].Content != "Hello back" {
		t.Fatalf("provider replay = %#v, %v", messages, err)
	}
	latest, err := database.ConversationItemPage(ctx, conversation.ID, "", 1)
	if err != nil || len(latest.Items) != 1 || !latest.HasMoreBefore || latest.Items[0].ID != assistant.ID {
		t.Fatalf("latest page = %#v, %v", latest, err)
	}
	older, err := database.ConversationItemPage(ctx, conversation.ID, latest.BeforeCursor, 1)
	if err != nil || len(older.Items) != 1 || older.HasMoreBefore || older.Items[0].ID != user.ID {
		t.Fatalf("older page = %#v, %v", older, err)
	}
	if err := database.Close(); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	replayed, err := reopened.ConversationItemPage(ctx, conversation.ID, "", 80)
	if err != nil || len(replayed.Items) != 2 || replayed.Items[0].ID != user.ID || replayed.Items[1].ID != assistant.ID {
		t.Fatalf("restart replay = %#v, %v", replayed, err)
	}
}

func TestConversationTurnFailureIsDurable(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Now()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, user, err := database.BeginConversationTurn(ctx, conversation.ID, "Fail safely", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	notice, err := database.FailConversationTurn(ctx, turn, strings.Repeat("é", 600), now)
	if err != nil {
		t.Fatal(err)
	}
	if notice.Kind != ConversationErrorNotice || notice.Status != "failed" ||
		notice.ParentItemID != user.ID || notice.Payload["recoverable"] != false ||
		!utf8.ValidString(notice.ContentText) || len(notice.ContentText) > 1000 {
		t.Fatalf("failure notice = %#v", notice)
	}
	var turnStatus, agentStatus string
	if err := database.db.QueryRow(`
SELECT turns.status, conversations.agent_status
FROM conversation_turns turns
JOIN conversations ON conversations.conversation_id = turns.conversation_id
WHERE turns.turn_id = ?`, turn.ID).Scan(&turnStatus, &agentStatus); err != nil {
		t.Fatal(err)
	}
	if turnStatus != "failed" || agentStatus != "error" {
		t.Fatalf("failure status = %q, %q", turnStatus, agentStatus)
	}
	if _, err := database.ConversationItemPage(ctx, conversation.ID, "bad-cursor", 80); err == nil {
		t.Fatal("invalid conversation cursor succeeded")
	}
}

func TestConversationTurnSerializesAndRecovers(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "chat-recovery.sqlite3")
	database, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	now := time.Now()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	type startResult struct {
		turn ConversationTurn
		err  error
	}
	start := make(chan struct{})
	results := make(chan startResult, 2)
	for _, input := range []string{"First", "Second"} {
		go func() {
			<-start
			turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, input, nil, now)
			results <- startResult{turn: turn, err: err}
		}()
	}
	close(start)
	var turn ConversationTurn
	var started, rejected int
	for range 2 {
		result := <-results
		switch {
		case result.err == nil:
			started++
			turn = result.turn
		case errors.Is(result.err, ErrConversationTurnActive):
			rejected++
		default:
			t.Fatalf("concurrent turn start error = %v", result.err)
		}
	}
	if started != 1 || rejected != 1 {
		t.Fatalf("concurrent starts = %d started, %d rejected", started, rejected)
	}
	if err := database.Close(); err != nil {
		t.Fatal(err)
	}
	database, err = Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	recovered, err := database.RecoverConversationTurns(ctx, now.Add(time.Minute))
	if err != nil || recovered != 1 {
		t.Fatalf("recovered turns = %d, %v", recovered, err)
	}
	if _, err := database.CompleteConversationTurn(ctx, turn, "stale", "stale", nil, now); err == nil {
		t.Fatal("cancelled turn completion succeeded")
	}
	if _, _, err := database.BeginConversationTurn(ctx, conversation.ID, "After recovery", nil, now); err != nil {
		t.Fatalf("turn after recovery: %v", err)
	}
	var status string
	if err := database.db.QueryRow(
		"SELECT agent_status FROM conversations WHERE conversation_id = ?", conversation.ID,
	).Scan(&status); err != nil || status != "input_received" {
		t.Fatalf("conversation status after recovery = %q, %v", status, err)
	}
}

func TestConversationToolCallIsAtomicRepeatSafeAndRecoverable(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 18, 0, 0, 0, time.UTC)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, user, err := database.BeginConversationTurn(ctx, conversation.ID, "Inspect it", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
		Commentary: "I will inspect it.",
		Reasoning:  []json.RawMessage{json.RawMessage(`{"type":"reasoning.encrypted","data":"opaque"}`)},
		Call: ConversationToolCallInput{
			ProviderRound: 0, OutputIndex: 2, ProviderCallID: "provider-call-1",
			ProviderName: "inspect", Name: "task.inspect",
			Arguments: json.RawMessage(`{"task_id":"task:one"}`),
		},
	}, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 3 || items[0].Kind != ConversationReasoning ||
		items[1].Kind != ConversationAssistantText || items[2].Kind != ConversationToolCall ||
		items[2].ParentItemID != user.ID || items[2].Status != "running" {
		t.Fatalf("tool round items = %#v", items)
	}
	if items[1].Metadata["stream_id"] != assistantStreamID(turn.ID) ||
		items[1].Metadata["response_index"] != float64(0) {
		t.Fatalf("commentary stream metadata = %#v", items[1].Metadata)
	}
	resultInput := ConversationToolResultInput{
		CallItemID: items[2].ID, ProviderRound: 0, OutputIndex: 2,
		ProviderCallID: "provider-call-1", ProviderName: "inspect", Name: "task.inspect",
		Success: true, Payload: json.RawMessage(`{"task_id":"task:one","title":"One"}`),
	}
	result, err := database.FinishConversationToolCall(ctx, turn, resultInput, now.Add(2*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if result.Kind != ConversationToolResult || result.ParentItemID != items[2].ID || result.Status != "completed" {
		t.Fatalf("tool result = %#v", result)
	}
	action := result.Payload["metadata"].(map[string]any)["action"].(map[string]any)
	callAction := items[2].Payload["metadata"].(map[string]any)["action"].(map[string]any)
	if action["call_id"] != callAction["id"] {
		t.Fatalf("result correlation = %#v, call = %#v", action["call_id"], callAction["id"])
	}
	repeated, err := database.FinishConversationToolCall(ctx, turn, resultInput, now.Add(3*time.Second))
	if err != nil || repeated.ID != result.ID {
		t.Fatalf("repeated result = %#v, %v", repeated, err)
	}
	resultInput.Payload = json.RawMessage(`{"different":true}`)
	if _, err := database.FinishConversationToolCall(ctx, turn, resultInput, now.Add(4*time.Second)); err == nil {
		t.Fatal("conflicting tool result repeat succeeded")
	}
	final, err := database.CompleteConversationTurnOutput(
		ctx, turn, "Done", "Done", nil,
		[]json.RawMessage{json.RawMessage(`{"type":"reasoning.summary","text":"Done"}`)}, 1,
		now.Add(5*time.Second),
	)
	if err != nil || final.Metadata["provider_round"] != float64(1) {
		t.Fatalf("final output = %#v, %v", final, err)
	}
	second, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Interrupt it", nil, now.Add(6*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	running, err := database.StartConversationToolRound(ctx, second, ConversationToolRound{
		Call: ConversationToolCallInput{
			ProviderRound: 0, OutputIndex: 0, ProviderCallID: "provider-call-2",
			ProviderName: "inspect", Name: "task.inspect", Arguments: json.RawMessage(`{"task_id":"task:two"}`),
		},
	}, now.Add(7*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if recovered, err := database.RecoverConversationTurns(ctx, now.Add(8*time.Second)); err != nil || recovered != 1 {
		t.Fatalf("recovered tool turn = %d, %v", recovered, err)
	}
	var recoveredStatus string
	if err := database.db.QueryRow(
		"SELECT status FROM conversation_items WHERE item_id = ?", running[len(running)-1].ID,
	).Scan(&recoveredStatus); err != nil || recoveredStatus != "cancelled" {
		t.Fatalf("recovered call status = %q, %v", recoveredStatus, err)
	}
}
