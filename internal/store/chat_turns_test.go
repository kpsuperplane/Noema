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
	if _, err := legacy.Exec(schemaAtVersion(6) + "\nPRAGMA user_version = 6;"); err != nil {
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
	if err := database.AppendConversationContextUpdate(ctx, turn, "openrouter", "", "Earlier context", nil, user.Sequence, now); err != nil {
		t.Fatal(err)
	}
	contextState, err := database.ConversationProviderContext(ctx, conversation.ID, "openrouter", "")
	if err != nil || contextState.Summary != "Earlier context" {
		t.Fatalf("default model checkpoint = %#v, %v", contextState, err)
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
		assistant.Cursor != "conversation_item:3" || assistant.Metadata["stream_id"] != ConversationAssistantStreamID(turn.ID, 0) {
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
	citationEnd := 18
	items, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
		Provider: "openrouter", Commentary: "I will inspect it.",
		ProviderCommentary:        "I will inspect it.\ue200cite\ue202private-reference\ue201",
		Citations:                 []ProviderCitation{{Title: "Source", URL: "https://example.test", EndIndex: &citationEnd}},
		UnresolvedCitationMarkers: 1,
		Reasoning:                 []json.RawMessage{json.RawMessage(`{"type":"reasoning.encrypted","data":"opaque"}`)},
		Call: ConversationToolCallInput{
			ProviderRound: 0, OutputIndex: 2, ProviderItemID: "provider-item-1",
			ProviderCallID: "provider-call-1",
			ProviderName:   "inspect", Name: "task.inspect",
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
	if items[1].Metadata["stream_id"] != ConversationAssistantStreamID(turn.ID, 0) ||
		items[1].Metadata["response_index"] != float64(0) ||
		items[1].Metadata["phase"] != "commentary" ||
		items[1].ProviderContentText == "" || items[1].Metadata["citations"] == nil ||
		items[1].Metadata["provider_citation_diagnostic"] == nil {
		t.Fatalf("commentary stream metadata = %#v", items[1].Metadata)
	}
	encodedMetadata, _ := json.Marshal(items[1].Metadata)
	if strings.Contains(string(encodedMetadata), "private-reference") {
		t.Fatalf("citation diagnostic stored a provider reference: %s", encodedMetadata)
	}
	resultInput := ConversationToolResultInput{
		CallItemID: items[2].ID, Provider: "openrouter", ProviderRound: 0, OutputIndex: 2,
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
	if action["call_id"] != callAction["id"] || callAction["provider_item_id"] != "provider-item-1" {
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
	later, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
		Provider:   "openrouter",
		Commentary: "I will inspect another Task.",
		Call: ConversationToolCallInput{
			ProviderRound: 1, OutputIndex: 0, ProviderCallID: "provider-call-later",
			ProviderName: "inspect", Name: "task.inspect",
			Arguments: json.RawMessage(`{"task_id":"task:later"}`),
		},
	}, now.Add(5*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if later[0].Metadata["stream_id"] != ConversationAssistantStreamID(turn.ID, 1) ||
		later[0].Metadata["phase"] != "commentary" {
		t.Fatalf("later commentary metadata = %#v", later[0].Metadata)
	}
	if _, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
		Provider: "openrouter",
		Call: ConversationToolCallInput{
			ProviderRound: 2, OutputIndex: 0, ProviderCallID: "provider-call-overlap",
			ProviderName: "inspect", Name: "task.inspect", Arguments: json.RawMessage(`{}`),
		},
	}, now.Add(6*time.Second)); err == nil {
		t.Fatal("overlapping later tool call succeeded")
	}
	if _, err := database.FinishConversationToolCall(ctx, turn, ConversationToolResultInput{
		CallItemID: later[len(later)-1].ID, Provider: "openrouter", ProviderRound: 1, OutputIndex: 0,
		ProviderCallID: "provider-call-later", ProviderName: "inspect", Name: "task.inspect",
		Success: false, Payload: json.RawMessage(`{"code":"not_found"}`),
	}, now.Add(7*time.Second)); err != nil {
		t.Fatal(err)
	}
	final, err := database.CompleteConversationTurnOutput(
		ctx, turn, "Done", "Done", nil,
		[]json.RawMessage{json.RawMessage(`{"type":"reasoning.summary","text":"Done"}`)}, nil, 0, 2,
		now.Add(8*time.Second),
	)
	if err != nil || final.Metadata["provider_round"] != float64(2) ||
		final.Metadata["stream_id"] != ConversationAssistantStreamID(turn.ID, 2) {
		t.Fatalf("final output = %#v, %v", final, err)
	}
	second, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Interrupt it", nil, now.Add(9*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	running, err := database.StartConversationToolRound(ctx, second, ConversationToolRound{
		Provider: "openrouter",
		Call: ConversationToolCallInput{
			ProviderRound: 0, OutputIndex: 0, ProviderCallID: "provider-call-2",
			ProviderName: "inspect", Name: "task.inspect", Arguments: json.RawMessage(`{"task_id":"task:two"}`),
		},
	}, now.Add(10*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.FinishConversationToolCall(ctx, second, ConversationToolResultInput{
		CallItemID: running[len(running)-1].ID, Provider: "openrouter", ProviderRound: 0, OutputIndex: 0,
		ProviderCallID: "provider-call-2", ProviderName: "inspect", Name: "task.inspect",
		Success: true, Payload: json.RawMessage(`{"task_id":"task:two"}`),
	}, now.Add(11*time.Second)); err != nil {
		t.Fatal(err)
	}
	running, err = database.StartConversationToolRound(ctx, second, ConversationToolRound{
		Provider: "openrouter",
		Call: ConversationToolCallInput{
			ProviderRound: 1, OutputIndex: 0, ProviderCallID: "provider-call-3",
			ProviderName: "inspect", Name: "task.inspect", Arguments: json.RawMessage(`{"task_id":"task:three"}`),
		},
	}, now.Add(12*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if recovered, err := database.RecoverConversationTurns(ctx, now.Add(13*time.Second)); err != nil || recovered != 1 {
		t.Fatalf("recovered tool turn = %d, %v", recovered, err)
	}
	var recoveredStatus string
	if err := database.db.QueryRow(
		"SELECT status FROM conversation_items WHERE item_id = ?", running[len(running)-1].ID,
	).Scan(&recoveredStatus); err != nil || recoveredStatus != "cancelled" {
		t.Fatalf("recovered call status = %q, %v", recoveredStatus, err)
	}
}

func TestConversationMultipleChoiceSelectionIsAtomicAndOrdered(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 5, 0, 0, 0, time.UTC)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Ask", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
		Provider: "openrouter",
		Call: ConversationToolCallInput{
			ProviderRound: 0, OutputIndex: 0, ProviderCallID: "call-choice",
			ProviderName: "present_multiple_choice", Name: "noema.present_multiple_choice",
			Arguments: json.RawMessage(`{"prompt":"Which?","selection_mode":"pick_many","options":[{"id":"b","label":"Beta"},{"id":"a","label":"Alpha"}]}`),
		},
		MultipleChoice: &ConversationMultipleChoiceInput{
			Prompt: "Which?", SelectionMode: "pick_many",
			Options: []ConversationMultipleChoiceOption{{ID: "b", Label: "Beta"}, {ID: "a", Label: "Alpha"}},
		},
	}, now)
	if err != nil || len(items) != 2 || items[1].Kind != ConversationMultipleChoicePrompt {
		t.Fatalf("choice publication = %#v, %v", items, err)
	}
	prompt := items[1]
	choice := ConversationChoiceSelection{PromptItemID: prompt.ID, OptionIDs: []string{"a", "b"}}
	if _, _, err := database.BeginConversationChoiceTurn(ctx, conversation.ID, choice, nil, now); !errors.Is(err, ErrConversationTurnActive) {
		t.Fatalf("active turn: %v", err)
	}
	if _, err := database.CompleteConversationTurn(ctx, turn, "Choose.", "", nil, now); err != nil {
		t.Fatal(err)
	}
	for _, selected := range [][]string{nil, {"b", "b"}, {"missing"}} {
		if _, _, err := database.BeginConversationChoiceTurn(ctx, conversation.ID, ConversationChoiceSelection{PromptItemID: prompt.ID, OptionIDs: selected}, nil, now); err == nil {
			t.Fatalf("invalid selection succeeded: %#v", selected)
		}
	}
	clientID := "choice-answer"
	next, selection, err := database.BeginConversationChoiceTurn(ctx, conversation.ID, choice, &clientID, now)
	if err != nil {
		t.Fatal(err)
	}
	selected := selection.Payload["selected_options"].([]any)
	if next.ID == turn.ID || selection.TurnID != next.ID || selection.ContentText != "Beta, Alpha" || selected[0].(map[string]any)["id"] != "b" || selected[1].(map[string]any)["id"] != "a" {
		t.Fatalf("selection = %#v", selection)
	}
	if _, err := database.CompleteConversationTurn(ctx, next, "Received.", "", nil, now); err != nil {
		t.Fatal(err)
	}
	if _, _, err := database.BeginConversationChoiceTurn(ctx, conversation.ID, choice, nil, now); err == nil {
		t.Fatal("second selection succeeded")
	}
	var count int
	if err := database.db.QueryRow(`SELECT count(*) FROM conversation_turns`).Scan(&count); err != nil || count != 2 {
		t.Fatalf("failed sends created turns: %d, %v", count, err)
	}
}

func TestConversationA2UIActionIsAtomicAndRecoverable(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 6, 0, 0, 0, time.UTC)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Show a form", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	projection := map[string]any{"protocol_version": "v0.9.1", "catalog": map[string]any{"catalog_id": "com.noema.a2ui/catalog/v0.9.1"},
		"messages": []any{}, "deleted_surface_ids": []any{}, "surfaces": map[string]any{
			"main": map[string]any{"surface_id": "main", "namespaced_surface_id": "main", "version": "v0.9.1",
				"catalog_id": "com.noema.a2ui/catalog/v0.9.1", "send_data_model": true, "revision": 2,
				"components": map[string]any{"root": map[string]any{"id": "root", "component": "Text", "text": "Ready"}},
				"data_model": map[string]any{}, "actions": []any{map[string]any{"source_component_id": "root", "name": "submit"}},
			}}}
	items, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter",
		Call: ConversationToolCallInput{ProviderRound: 0, OutputIndex: 0, ProviderCallID: "call-a2ui",
			ProviderName: "present_a2ui", Name: "noema.present_a2ui", Arguments: json.RawMessage(`{"jsonl":"test"}`)},
		A2UI: &ConversationA2UIInput{Projection: projection, HasActions: true,
			ProviderSelection: map[string]any{"role": "noema", "provider_kind": "openrouter", "provider_account_id": "provider_account:test", "model_profile": "openai/test"},
			ToolCatalogDigest: "catalog:test"}}, now)
	if err != nil || len(items) != 2 || items[1].Kind != ConversationA2UICard {
		t.Fatalf("A2UI publication = %#v, %v", items, err)
	}
	surface := items[1]
	storedProjection := surface.Payload["payload"].(map[string]any)
	interactionID := textJSON(storedProjection["interaction_id"])
	settled := cloneJSONMap(storedProjection)
	settled["interaction_revision"], settled["lifecycle"] = 2, "answered"
	clientID := "a2ui-answer"
	continuation, err := database.ResolveConversationA2UI(ctx, conversation.ID, surface.ID, interactionID, 1,
		settled, map[string]any{"status": "resolved", "interaction_id": interactionID}, &clientID, now.Add(time.Second))
	if err != nil || continuation.Call.Status != "completed" || continuation.Action.ParentItemID != surface.ID || continuation.Result.ParentItemID != items[0].ID {
		t.Fatalf("A2UI resolution = %#v, %v", continuation, err)
	}
	if _, err := database.ResolveConversationA2UI(ctx, conversation.ID, surface.ID, interactionID, 1, settled, map[string]any{}, nil, now); err == nil {
		t.Fatal("second A2UI action succeeded")
	}
	claimed, err := database.ClaimConversationA2UI(ctx, surface.ID, now.Add(2*time.Second))
	if err != nil || claimed.Turn.Status != "running" {
		t.Fatalf("A2UI claim = %#v, %v", claimed, err)
	}
	recovered, err := database.RecoverConversationA2UI(ctx, now.Add(3*time.Second))
	if err != nil || len(recovered) != 1 || recovered[0].Surface.ID != surface.ID || recovered[0].Turn.Status != "waiting_for_tool" {
		t.Fatalf("A2UI recovery = %#v, %v", recovered, err)
	}
	if count, err := database.RecoverConversationTurns(ctx, now.Add(4*time.Second)); err != nil || count != 0 {
		t.Fatalf("generic recovery changed A2UI turn = %d, %v", count, err)
	}
	claimed, err = database.ClaimConversationA2UI(ctx, surface.ID, now.Add(5*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.FailConversationTurn(ctx, claimed.Turn, "failed", now.Add(6*time.Second)); err != nil {
		t.Fatal(err)
	}
	var lifecycle string
	if err := database.db.QueryRow(`SELECT json_extract(payload_json, '$.payload.lifecycle') FROM conversation_items WHERE item_id = ?`, continuation.Action.ID).Scan(&lifecycle); err != nil || lifecycle != "failed" {
		t.Fatalf("failed A2UI lifecycle = %q, %v", lifecycle, err)
	}
}
