package runtime

import (
	"context"
	"fmt"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestChatOutputPreservesMessagePhasesAndReasoningDisplay(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	turn, _, err := database.BeginConversationTurn(context.Background(), conversation.ID, "hello", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	stream := chat.outputStream(turn, 0, nil)
	stream.event(provider.StreamEvent{Kind: provider.MessageStarted, Index: 0, ID: "m1", Phase: "commentary"})
	stream.event(provider.StreamEvent{Kind: provider.TextDelta, Index: 0, Delta: "Checking."})
	stream.event(provider.StreamEvent{Kind: provider.MessageCompleted, Index: 0, Text: "Checking.", Phase: "commentary"})
	stream.event(provider.StreamEvent{Kind: provider.ReasoningDelta, Index: 1, SectionIndex: 0, Delta: "Readable reasoning"})
	stream.event(provider.StreamEvent{Kind: provider.ReasoningCompleted, Index: 1, SectionIndex: 0, Text: "Readable reasoning"})
	stream.event(provider.StreamEvent{Kind: provider.MessageCompleted, Index: 2, ID: "m2", Phase: "final_answer", Text: "Done."})
	start, end := 9, 14
	result := provider.GenerationResult{Text: "Checking.Done.", Citations: []provider.Citation{{URL: "https://example.com", StartIndex: &start, EndIndex: &end}}}
	if err := stream.finish(&result, nil); err != nil {
		t.Fatal(err)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Items) != 4 || page.Items[2].ContentText != "Readable reasoning" {
		t.Fatalf("readable output missing: %#v", page.Items)
	}
	if page.Items[1].Metadata["citations"] != nil {
		t.Fatal("final citation attached to progress")
	}
	citations, ok := page.Items[3].Metadata["citations"].([]any)
	if !ok || len(citations) != 1 {
		t.Fatal("final section citation missing")
	}
	citation := citations[0].(map[string]any)
	if citation["start_index"] != float64(0) || citation["end_index"] != float64(5) {
		t.Fatalf("citation offsets not rebased: %#v", citation)
	}
	page.Items = append(page.Items, store.ConversationItem{Kind: store.ConversationReasoning, TurnID: turn.ID, Metadata: map[string]any{"provider_round": float64(0), "provider": "codex"}, Payload: map[string]any{"provider_details": []any{map[string]any{"type": "reasoning", "encrypted_content": "opaque-test"}}}})
	replay, err := providerMessagesFromItems(page.Items, turn.ID, "codex")
	if err != nil {
		t.Fatal(err)
	}
	if len(replay) != 3 || replay[1].Content != "Checking." || replay[1].Phase != "commentary" || replay[2].Content != "Done." || replay[2].Phase != "final_answer" {
		t.Fatalf("replay lost message boundaries or phases: %#v", replay)
	}
	if len(replay[1].ReasoningDetails) != 1 || len(replay[2].ReasoningDetails) != 0 {
		t.Fatal("reasoning crossed response boundary")
	}
	if _, err := database.FailConversationTurn(context.Background(), turn, "Stopped", time.Now()); err != nil {
		t.Fatal(err)
	}
	result.Output[0].Text = "stale changed output"
	if err := stream.finish(&result, nil); err == nil {
		t.Fatal("failed turn accepted output")
	}
}

func TestChatOutputSavesParagraphsAndPreservesNativeReplay(t *testing.T) {
	for _, streamed := range []bool{false, true} {
		t.Run(fmt.Sprint(streamed), func(t *testing.T) {
			chat, database, conversation := chatFixture(t)
			ctx := context.Background()
			turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "hello", nil, time.Now())
			if err != nil {
				t.Fatal(err)
			}
			text := "😀 hello\n\n```text\none\n\ntwo\n```\n\nQuestion?"
			stream := chat.outputStream(turn, 0, nil)
			if streamed {
				stream.event(provider.StreamEvent{Kind: provider.TextDelta, Index: 0, Delta: "😀 hello\n"})
				stream.event(provider.StreamEvent{Kind: provider.MessageCompleted, Index: 0, ID: "m1", Phase: "final_answer", Text: text})
			}
			start, end := utf16CodeUnitCount(text)-9, utf16CodeUnitCount(text)
			result := provider.GenerationResult{Text: text, Citations: []provider.Citation{{URL: "https://example.com", StartIndex: &start, EndIndex: &end}}}
			if err := stream.finish(&result, nil); err != nil {
				t.Fatal(err)
			}
			if _, err := database.CompleteConversationTurn(ctx, turn, text, text, nil, time.Now()); err != nil {
				t.Fatal(err)
			}
			page, err := database.ConversationItemPage(ctx, conversation.ID, "", 20)
			if err != nil {
				t.Fatal(err)
			}
			if len(page.Items) != 4 {
				t.Fatalf("saved messages = %#v", page.Items)
			}
			for i, want := range []string{"😀 hello", "```text\none\n\ntwo\n```", "Question?"} {
				if page.Items[i+1].ContentText != want || page.Items[i+1].Status != "completed" || page.Items[i+1].Metadata["phase"] != "final_answer" {
					t.Fatalf("paragraph %d = %#v", i, page.Items[i+1])
				}
			}
			if page.Items[1].Metadata["citations"] != nil || page.Items[2].Metadata["citations"] != nil {
				t.Fatal("citation attached to the wrong paragraph")
			}
			citations := page.Items[3].Metadata["citations"].([]any)
			citation := citations[0].(map[string]any)
			if citation["start_index"] != float64(0) || citation["end_index"] != float64(9) {
				t.Fatalf("paragraph citation offsets = %#v", citation)
			}
			replay, err := providerMessagesFromItems(page.Items, turn.ID, conversation.Provider)
			if err != nil || len(replay) != 2 || replay[1].Content != text {
				t.Fatalf("native replay changed: %#v, %v", replay, err)
			}
		})
	}
}

func TestChatOutputSplitsDuringStreaming(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	ctx := context.Background()
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "hello", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	stream := chat.outputStream(turn, 0, nil)
	text := ""
	ids := map[int]string{}
	check := func(want []string, status string) {
		t.Helper()
		page, err := database.ConversationItemPage(ctx, conversation.ID, "", 20)
		if err != nil {
			t.Fatal(err)
		}
		if len(page.Items) != len(want)+1 {
			t.Fatalf("text %q: got %d items, want %d", text, len(page.Items), len(want)+1)
		}
		for i, expected := range want {
			item := page.Items[i+1]
			if item.ContentText != expected || item.Status != status {
				t.Fatalf("text %q: bubble %d = %q (%s), want %q (%s)", text, i, item.ContentText, item.Status, expected, status)
			}
			if previous, ok := ids[i]; ok && item.ID != previous {
				t.Fatalf("bubble %d changed identity", i)
			}
			ids[i] = item.ID
		}
	}
	steps := []struct {
		delta string
		want  []string
	}{
		{"😀 first", []string{"😀 first"}},
		{"\n-", []string{"😀 first"}},
		{"-", []string{"😀 first"}},
		{"-", []string{"😀 first"}},
		{"\r\nSecond", []string{"😀 first", "Second"}},
		{"\n\n```md\n-", []string{"😀 first", "Second", "```md\n-"}},
		{"--\n\ncode\n```", []string{"😀 first", "Second", "```md\n---\n\ncode\n```"}},
		{"\n---\nLast", []string{"😀 first", "Second", "```md\n---\n\ncode\n```", "Last"}},
		{"\n--", []string{"😀 first", "Second", "```md\n---\n\ncode\n```", "Last"}},
		{"x", []string{"😀 first", "Second", "```md\n---\n\ncode\n```", "Last\n--x"}},
	}
	for _, step := range steps {
		text += step.delta
		stream.event(provider.StreamEvent{Kind: provider.TextDelta, Index: 0, Delta: step.delta})
		stream.flush(true)
		if stream.err != nil {
			t.Fatal(stream.err)
		}
		check(step.want, "running")
	}
	result := provider.GenerationResult{Text: text}
	if err := stream.finish(&result, nil); err != nil {
		t.Fatal(err)
	}
	check(steps[len(steps)-1].want, "completed")
	page, err := database.ConversationItemPage(ctx, conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	replay, err := providerMessagesFromItems(page.Items, turn.ID, conversation.Provider)
	if err != nil || len(replay) != 2 || replay[1].Content != text {
		t.Fatalf("native replay changed: %#v, %v", replay, err)
	}
}
