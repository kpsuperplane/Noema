package runtime

import (
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestChatSessionRejectsChangedContextAndAuthority(t *testing.T) {
	for _, change := range []string{"conversation", "model", "credentials", "instructions", "history", "compaction"} {
		t.Run(change, func(t *testing.T) {
			database := contextTestStore(t, 128000)
			generator := &sessionTestGenerator{closed: make(chan struct{}, 2)}
			chat := &Chat{ctx: t.Context(), database: database}
			t.Cleanup(chat.closeChatSession)
			assignment := store.ModelAssignment{ProviderKind: "codex", ProviderAccountID: "provider_account:openrouter:context-test", ModelProfile: "test"}
			turn := store.ConversationTurn{ConversationID: "conversation:one", ID: "turn:one"}
			base := []provider.GenerationMessage{{Role: "system", Instructions: true, Content: "Instructions"}}
			history := []provider.GenerationMessage{{Role: "user", Content: "private earlier message"}}
			active := []provider.GenerationMessage{{Role: "user", Content: "next"}}
			full := joinContextMessages(base, history, active)
			_, _, _, err := chat.chatSessionRequest(generator, assignment, turn, base, history, active, full, false)
			if err != nil {
				t.Fatal(err)
			}
			chat.session.responseID = "response:one"
			chat.session.history = chatHistoryDigest(history)
			compacted := false
			switch change {
			case "conversation":
				turn.ConversationID = "conversation:two"
			case "model":
				assignment.ModelProfile = "other"
			case "credentials":
				chat.session.credentialRevision++
			case "instructions":
				base = []provider.GenerationMessage{{Role: "system", Instructions: true, Content: "Changed instructions"}}
			case "history":
				history = []provider.GenerationMessage{{Role: "user", Content: "edited message"}}
			case "compaction":
				compacted = true
			}
			full = joinContextMessages(base, history, active)
			_, messages, previousID, err := chat.chatSessionRequest(generator, assignment, turn, base, history, active, full, compacted)
			if err != nil || previousID != "" || len(messages) != len(full) || generator.opens != 2 || generator.closes != 1 {
				t.Fatalf("changed %s retained old context: previous=%q opens=%d closes=%d error=%v", change, previousID, generator.opens, generator.closes, err)
			}
		})
	}
}
