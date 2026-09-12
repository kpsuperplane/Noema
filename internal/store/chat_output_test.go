package store

import (
	"context"
	"testing"
	"time"
)

func TestConversationOutputSnapshotsReconcileAndFenceTerminalTurns(t *testing.T) {
	ctx := context.Background()
	database := openTestStore(t)
	now := time.Now()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "hello", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	first, err := database.SaveConversationOutput(ctx, turn, 0, 0, 0, 0, "message", "commentary", "m1", "Checking", "Checking", "running", false, nil, now)
	if err != nil {
		t.Fatal(err)
	}
	next, err := database.SaveConversationOutput(ctx, turn, 0, 0, 0, 0, "message", "commentary", "m1", "Checking files.", "Checking files.", "completed", false, nil, now)
	if err != nil {
		t.Fatal(err)
	}
	if first.ID != next.ID || first.Sequence != next.Sequence {
		t.Fatal("snapshot identity changed")
	}
	reasoning, err := database.SaveConversationOutput(ctx, turn, 0, 1, 0, 0, "reasoning", "", "r1", "Readable summary", "Readable summary", "completed", true, nil, now)
	if err != nil {
		t.Fatal(err)
	}
	if reasoning.Metadata["reasoning_summary"] != true {
		t.Fatal("summary type was not saved")
	}
	final, err := database.SaveConversationOutput(ctx, turn, 0, 2, 0, 0, "message", "final_answer", "m2", "Done.", "Done.", "completed", false, nil, now)
	if err != nil {
		t.Fatal(err)
	}
	completed, err := database.CompleteConversationTurnOutput(ctx, turn, "Checking files.Done.", "Checking files.Done.", &ProviderUsage{Provider: "openrouter", TotalTokens: 7}, nil, nil, 0, 0, now)
	if err != nil {
		t.Fatal(err)
	}
	if completed.ID != final.ID || completed.ContentText != "Done." || completed.Metadata["provider_phase"] != "final_answer" {
		t.Fatalf("completion replaced section: %#v", completed)
	}
	page, err := database.ConversationItemPage(ctx, conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Items) != 4 || page.Items[1].ContentText != "Checking files." || page.Items[2].ContentText != "Readable summary" {
		t.Fatalf("reloaded output: %#v", page.Items)
	}
	if _, err := database.SaveConversationOutput(ctx, turn, 0, 0, 0, 0, "message", "commentary", "m1", "stale", "stale", "running", false, nil, now); err == nil {
		t.Fatal("terminal turn accepted stale output")
	}
}
