package store

import (
	"context"
	"errors"
	"testing"
	"time"
)

func TestPrimaryConversationIsDurableAndIdempotent(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	if current, err := database.PrimaryConversation(ctx); err != nil || current != nil {
		t.Fatalf("fresh primary conversation = %#v, %v", current, err)
	}
	created, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	again, err := database.EnsurePrimaryConversation(ctx, "codex", "/ignored", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if again.ID != created.ID || again.Provider != "openrouter" || again.CWD != "" {
		t.Fatalf("idempotent primary conversation = %#v, want %#v", again, created)
	}
	read, err := database.PrimaryConversation(ctx)
	if err != nil || read == nil || read.ID != created.ID {
		t.Fatalf("stored primary conversation = %#v, %v", read, err)
	}
	if _, err := database.Conversation(ctx, created.ID); err != nil {
		t.Fatalf("read owned conversation: %v", err)
	}
	if _, err := database.Conversation(ctx, "conversation:00000000000000000000000000000000"); !errors.Is(err, ErrConversationNotFound) {
		t.Fatalf("missing conversation error = %v", err)
	}
}
