package runtime

import (
	"context"
	"encoding/json"
	"strings"
	"testing"
	"time"

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
	proposalEvents := collectCompletedTurns(t, events, 1)
	if !containsEventKind(proposalEvents, EventHumanInterventionsChanged) {
		t.Fatalf("proposal events = %#v", proposalEvents)
	}
	pending, err := database.PendingActionRequests(context.Background(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 || pending[0].State != store.ActionAwaitingApproval {
		t.Fatalf("pending actions = %#v, %v", pending, err)
	}
	resolved, err := chat.ResolveActionRequest(context.Background(), pending[0].ID, 1, "human:local", "decline")
	if err != nil || resolved.State != store.ActionDeclined {
		t.Fatalf("resolved action = %#v, %v", resolved, err)
	}
	resolutionEvents := collectCompletedTurns(t, events, 1)
	if !containsEventKind(resolutionEvents, EventHumanInterventionsChanged) {
		t.Fatalf("resolution events = %#v", resolutionEvents)
	}
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

func TestCredentialDownloadIsRejectedBeforeDurableToolCall(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	calls := 0
	generator := generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
			ProviderCallID: "download-secret", ProviderName: "download", Name: fileDownloadName,
			Payload: json.RawMessage(`{"url":"https://alice:password@example.net/report","path":"report"}`),
		}}}, nil
	})
	chat, err := NewChat(database, generator, original.codex, original.home, original.memory)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = chat.Close() })
	events, err := chat.Subscribe(t.Context(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(t.Context(), SendTurnInput{
		ConversationID: conversation.ID, Input: "Download the report.",
	}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	page, err := database.ConversationItemPage(t.Context(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	encoded, _ := json.Marshal(page.Items)
	if calls != 1 || strings.Contains(string(encoded), "password") {
		t.Fatalf("provider calls = %d, transcript = %s", calls, encoded)
	}
	for _, item := range page.Items {
		if item.Kind == store.ConversationToolCall || item.Kind == store.ConversationToolResult {
			t.Fatalf("credential call entered transcript: %#v", item)
		}
	}
	pending, err := database.PendingActionRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 0 {
		t.Fatalf("pending actions = %#v, %v", pending, err)
	}
}

func TestRecoveredUncertainDownloadResumesOnce(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	reviewer := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) == 1 && request.Tools[0].Name == actionReviewToolName {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				ProviderCallID: "review-recovery", ProviderName: actionReviewToolName, Name: actionReviewToolName,
				Payload: json.RawMessage(`{"authorization":"absent","risk":"high","reason_codes":["authorization_absent"],"explanation":"Approval is required."}`),
			}}}, nil
		}
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
			ProviderCallID: "download-recovery", ProviderName: "download", Name: fileDownloadName,
			Payload: json.RawMessage(`{"url":"https://example.net/report","path":"report"}`),
		}}}, nil
	})
	chat, err := NewChat(database, reviewer, original.codex, original.home, original.memory)
	if err != nil {
		t.Fatal(err)
	}
	events, err := chat.Subscribe(t.Context(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(t.Context(), SendTurnInput{ConversationID: conversation.ID, Input: "Download it."}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	pending, err := database.PendingActionRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 {
		t.Fatalf("pending actions = %#v, %v", pending, err)
	}
	action, err := database.DecideActionRequest(t.Context(), pending[0].ID, 1, "human:local", "approve", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err = database.ClaimActionRequest(t.Context(), action.ID, 1, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := chat.Close(); err != nil {
		t.Fatal(err)
	}

	started, release := make(chan bool), make(chan struct{})
	continuation := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		var uncertain bool
		for _, message := range request.Messages {
			if message.ToolResult != nil && message.ToolResult.ProviderCallID == "download-recovery" &&
				strings.Contains(string(message.ToolResult.Payload), "outcome_uncertain") {
				uncertain = true
			}
		}
		started <- uncertain
		<-release
		return provider.GenerationResult{Text: "The download outcome is uncertain."}, nil
	})
	restarted, err := NewChat(database, continuation, original.codex, original.home, original.memory)
	if err != nil {
		t.Fatal(err)
	}
	if !<-started {
		t.Fatal("recovered request lacks the uncertain result")
	}
	recoveryEvents, err := restarted.Subscribe(t.Context(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-recoveryEvents
	close(release)
	collectCompletedTurns(t, recoveryEvents, 1)
	if err := restarted.Close(); err != nil {
		t.Fatal(err)
	}
	continuations := 0
	quiet := generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		continuations++
		return provider.GenerationResult{Text: "unexpected"}, nil
	})
	reopened, err := NewChat(database, quiet, original.codex, original.home, original.memory)
	if err != nil {
		t.Fatal(err)
	}
	if err := reopened.Close(); err != nil {
		t.Fatal(err)
	}
	if continuations != 0 {
		t.Fatalf("second recovery continuations = %d", continuations)
	}

	page, err := database.ConversationItemPage(t.Context(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	results := 0
	for _, item := range page.Items {
		if item.Kind == store.ConversationToolResult {
			results++
		}
	}
	if results != 1 {
		t.Fatalf("tool results = %d in %#v", results, page.Items)
	}
}

func containsEventKind(events []Event, kind EventKind) bool {
	for _, event := range events {
		if event.Kind == kind {
			return true
		}
	}
	return false
}
