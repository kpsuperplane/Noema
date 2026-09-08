package store

import (
	"context"
	"encoding/json"
	"testing"
	"time"
)

func TestConversationToolRoundReturnsRustMarkersForCallAndResult(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Unix(0, 0)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Read the task file.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
		Provider: "openrouter",
		Call: ConversationToolCallInput{
			ProviderRound:  0,
			OutputIndex:    0,
			ProviderCallID: "call-read",
			ProviderName:   "task.files.read",
			Name:           "task.files.read",
			Arguments:      json.RawMessage(`{"path":"TASK.md"}`),
		},
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	call := items[len(items)-1]
	callMarker := conversationToolMarker(t, call)
	if callMarker["identity"] != "Read Task file" || callMarker["summary"] != "Reading TASK.md" || callMarker["status"] != "running" {
		t.Fatalf("call marker = %#v", callMarker)
	}
	result, err := database.FinishConversationToolCall(ctx, turn, ConversationToolResultInput{
		CallItemID: call.ID, Provider: "openrouter", ProviderRound: 0, OutputIndex: 0,
		ProviderCallID: "call-read", ProviderName: "task.files.read", Name: "task.files.read",
		Success: true, Payload: json.RawMessage(`{"path":"TASK.md","content":"# Task"}`),
	}, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	resultMarker := conversationToolMarker(t, result)
	if resultMarker["identity"] != "Read Task file" || resultMarker["summary"] != "Read TASK.md" || resultMarker["status"] != "completed" {
		t.Fatalf("result marker = %#v", resultMarker)
	}
}

func TestConversationToolResultUsesCallMarkerForFailedBrowserOpen(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Unix(0, 0)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Open the page.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
		Provider: "openrouter",
		Call: ConversationToolCallInput{
			ProviderRound:  0,
			OutputIndex:    0,
			ProviderCallID: "call-open",
			ProviderName:   "web.browse.open",
			Name:           "web.browse.open",
			Arguments:      json.RawMessage(`{"url":"https://www.example.com/docs"}`),
		},
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	result, err := database.FinishConversationToolCall(ctx, turn, ConversationToolResultInput{
		CallItemID: items[len(items)-1].ID, Provider: "openrouter", ProviderRound: 0, OutputIndex: 0,
		ProviderCallID: "call-open", ProviderName: "web.browse.open", Name: "web.browse.open",
		Success: false, Payload: json.RawMessage(`{"error":"connection failed"}`),
	}, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	marker := conversationToolMarker(t, result)
	if marker["summary"] != "Could not open example.com" || marker["status"] != "failed" {
		t.Fatalf("failed browser marker = %#v", marker)
	}
}

func conversationToolMarker(t *testing.T, item ConversationItem) map[string]any {
	t.Helper()
	metadata, ok := item.Payload["metadata"].(map[string]any)
	if !ok {
		t.Fatalf("item metadata = %#v", item.Payload)
	}
	display, ok := metadata["display"].(map[string]any)
	if !ok {
		t.Fatalf("item display = %#v", metadata)
	}
	marker, ok := display["marker"].(map[string]any)
	if !ok {
		t.Fatalf("item marker = %#v", display)
	}
	return marker
}
