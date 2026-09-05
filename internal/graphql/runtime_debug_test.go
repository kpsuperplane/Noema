package graphql

import (
	"context"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestRuntimeDebugProfileProjectsTimingMetadataAndOwnership(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	conversation, err := resolver.Store.EnsurePrimaryConversation(ctx, "openai", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	started := time.Now().UTC()
	turn, _, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "Profile this.", nil, started)
	if err != nil {
		t.Fatal(err)
	}
	round, inputTokens := 1, 21
	spanID, err := resolver.Store.BeginRuntimeDebugSpan(ctx,
		store.RuntimeDebugScope{Kind: "conversation_turn", ID: turn.ID}, "provider", "Provider request",
		store.RuntimeDebugMetadata{Provider: "openai", Model: "gpt-test", Phase: "continuation",
			RoundIndex: &round, InputTokens: &inputTokens}, started.Add(10*time.Millisecond))
	if err != nil {
		t.Fatal(err)
	}
	if err = resolver.Store.FinishRuntimeDebugSpan(ctx, spanID, "completed",
		store.RuntimeDebugMetadata{Provider: "openai", Model: "gpt-test", Phase: "continuation",
			RoundIndex: &round, InputTokens: &inputTokens}, 30*time.Millisecond, started.Add(40*time.Millisecond)); err != nil {
		t.Fatal(err)
	}
	if _, err = resolver.Store.FailConversationTurn(ctx, turn, "Test failure.", started.Add(100*time.Millisecond)); err != nil {
		t.Fatal(err)
	}
	profile, err := resolver.runtimeDebugProfile(ctx, model.RuntimeDebugProfileInput{
		Kind: model.RuntimeDebugScopeKindConversationTurn, ScopeID: turn.ID,
	})
	if err != nil || profile == nil || len(profile.Spans) != 1 {
		t.Fatalf("profile = %#v, %v", profile, err)
	}
	span := profile.Spans[0]
	if span.DurationMilliseconds != 30 || span.StartOffsetMilliseconds != 10 ||
		span.Provider == nil || *span.Provider != "openai" || span.InputTokens == nil || *span.InputTokens != 21 {
		t.Fatalf("span = %#v", span)
	}
	if profile.AccountedMilliseconds != 30 || profile.UninstrumentedMilliseconds != 70 {
		t.Fatalf("profile timing = %#v", profile)
	}
	missing, err := resolver.runtimeDebugProfile(ctx, model.RuntimeDebugProfileInput{
		Kind: model.RuntimeDebugScopeKindConversationTurn, ScopeID: "turn:00000000000000000000000000000000",
	})
	if err != nil || missing != nil {
		t.Fatalf("unowned profile = %#v, %v", missing, err)
	}
	ended := started.Add(50 * time.Millisecond)
	runningStarted := started.Add(20 * time.Millisecond)
	projected := runtimeDebugProfileModel(&store.RuntimeDebugProfile{
		Scope: store.RuntimeDebugScope{Kind: "task_run", ID: "run:test"}, Status: "failed",
		StartedAt: started, EndedAt: &ended, Spans: []store.RuntimeDebugSpan{{
			ID: "debug_span:test", Category: "runtime", Name: "Work", Status: "running", StartedAt: runningStarted,
		}},
	}, ended)
	if projected.Kind != model.RuntimeDebugScopeKindTaskRun || projected.Spans[0].Status != model.RuntimeDebugStatusInterrupted ||
		projected.ElapsedMilliseconds != 50 || projected.AccountedMilliseconds != 30 || projected.UninstrumentedMilliseconds != 20 {
		t.Fatalf("terminal projection = %#v", projected)
	}
}
