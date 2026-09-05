package runtime

import (
	"context"
	"encoding/json"
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestGovernedDownloadRequiresApprovalAndResumesExactChatCall(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	primary := make([]provider.GenerateRequest, 0, 2)
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) == 1 && request.Tools[0].Name == actionReviewToolName {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				ProviderCallID: "review-1", ProviderName: actionReviewToolName, Name: actionReviewToolName,
				Payload: json.RawMessage(`{"authorization":"absent","risk":"high","reason_codes":["authorization_absent"],"explanation":"The destination needs human approval."}`),
			}}}, nil
		}
		primary = append(primary, request)
		if len(primary) == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				Index: 0, ProviderCallID: "download-1", ProviderName: "download", Name: fileDownloadName,
				Payload: json.RawMessage(`{"url":"https://example.net/report.pdf","path":"report.pdf"}`),
			}}}, nil
		}
		return provider.GenerationResult{Text: "The download was declined."}, nil
	})
	chat, err := NewChat(database, generator, original.codex, original.home, original.memory)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = chat.Close() })
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "Save the report if it is appropriate.",
	}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	pending, err := database.PendingActionRequests(context.Background(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 || pending[0].State != store.ActionAwaitingApproval {
		t.Fatalf("pending actions = %#v, %v", pending, err)
	}
	resolved, err := chat.ResolveActionRequest(context.Background(), pending[0].ID, 1, "human:local", "decline")
	if err != nil || resolved.State != store.ActionDeclined {
		t.Fatalf("resolved action = %#v, %v", resolved, err)
	}
	collectCompletedTurns(t, events, 1)
	if len(primary) != 2 {
		t.Fatalf("primary requests = %d", len(primary))
	}
	var replay *provider.ReplayToolResult
	for _, message := range primary[1].Messages {
		if message.ToolResult != nil && message.ToolResult.ProviderCallID == "download-1" {
			replay = message.ToolResult
		}
	}
	if replay == nil || replay.Name != fileDownloadName || replay.Success {
		t.Fatalf("resumed tool result = %#v", replay)
	}
}
