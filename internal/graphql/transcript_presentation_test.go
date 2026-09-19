package graphql

import (
	"context"
	"reflect"
	"testing"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestAssistantPresentationAcrossChatAndTasks(t *testing.T) {
	for _, tc := range []struct {
		name, kind, phase, want string
		summary                 bool
	}{
		{"progress", "message", "commentary", "bubble", false},
		{"reasoning trace", "reasoning", "", "bubble", false},
		{"reasoning summary", "reasoning", "", "marker", true},
		{"reasoning trace with commentary phase", "reasoning", "commentary", "bubble", false},
		{"answer", "message", "final_answer", "bubble", false},
		{"untyped response", "message", "", "bubble", false},
	} {
		t.Run(tc.name, func(t *testing.T) {
			metadata := map[string]any{"provider_output_kind": tc.kind, "reasoning_summary": tc.summary, "provider_phase": tc.phase, "phase": "commentary", "model": "ordinary-model-id"}
			source := store.ConversationItem{ID: "item", Kind: store.ConversationAssistantText, ContentText: "Exact text https://example.test", Metadata: metadata}
			resolver := &Resolver{}
			page, err := resolver.conversationTranscriptPageModel(context.Background(), store.ConversationItemPage{Items: []store.ConversationItem{source}})
			if err != nil {
				t.Fatal(err)
			}
			event, err := resolver.conversationEventModel(context.Background(), runtime.Event{Kind: runtime.EventConversationItem, Item: &source})
			if err != nil {
				t.Fatal(err)
			}
			live := event.(model.ConversationItemEvent)
			if tc.want == "marker" {
				if len(page.Items) != 0 || live.Item != nil {
					t.Fatalf("Chat exposed reasoning summary: %#v / %#v", page.Items, live.Item)
				}
			} else {
				if len(page.Items) != 1 || page.Items[0].Metadata["presentation"] != tc.want || !reflect.DeepEqual(page.Items[0].Metadata, live.Metadata) {
					t.Fatalf("replay/live presentation: %#v / %#v", page.Items, live.Metadata)
				}
				if live.Item.(model.AssistantText).Text != source.ContentText || live.Metadata["model"] != metadata["model"] {
					t.Fatal("Chat content or metadata changed")
				}
			}
			if metadata["presentation"] != nil {
				t.Fatal("saved metadata changed")
			}
			section := map[string]any{"kind": tc.kind, "reasoning_summary": tc.summary, "phase": tc.phase, "text": source.ContentText, "status": "running", "index": float64(3)}
			payload := map[string]any{"output": []any{section}, "response_index": 7}
			task := taskRunItemModel(store.TaskRunItem{Kind: "assistant_output", Payload: payload})
			output := task.Payload["output"].([]any)[0].(map[string]any)
			if output["presentation"] != tc.want || output["text"] != source.ContentText || output["index"] != float64(3) || output["status"] != "running" {
				t.Fatalf("Task output: %#v", output)
			}
			if section["presentation"] != nil || payload["presentation"] != nil || task.Payload["response_index"] != 7 {
				t.Fatal("Task source changed or metadata lost")
			}
		})
	}
}

func TestChatPresentationUsesRecordedKindBeforeGenericPhase(t *testing.T) {
	for _, tc := range []struct {
		metadata map[string]any
		want     string
	}{
		{map[string]any{"phase": "commentary"}, "bubble"},
		{map[string]any{"phase": "final_answer"}, "bubble"},
		{map[string]any{"phase": "commentary", "provider_output_kind": "reasoning"}, "bubble"},
		{nil, "bubble"},
	} {
		got := transcriptMetadata(store.ConversationItem{Kind: store.ConversationAssistantText, Metadata: tc.metadata})
		if got["presentation"] != tc.want {
			t.Fatalf("%#v: got %#v", tc.metadata, got)
		}
	}
}

func TestSavedCodexSummaryPresentation(t *testing.T) {
	for _, tc := range []struct {
		kind, text, want string
		section          float64
	}{
		{"message", "i’ll look for NYC-based boutiques and labels with Theory’s polished, minimal feel, prioritizing linen, cotton, wool, silk, and cashmere over synthetics", "bubble", 0},
		{"reasoning", "**Planning NYC-focused web searches on natural fiber shops**", "marker", 0},
		{"reasoning", "A full reasoning trace", "bubble", 4096},
	} {
		item := store.ConversationItem{Kind: store.ConversationAssistantText, ContentText: tc.text, Metadata: map[string]any{"provider": "codex", "provider_output_kind": tc.kind, "phase": "commentary", "section_index": tc.section}}
		if got := transcriptMetadata(item); got["presentation"] != tc.want {
			t.Fatalf("%s: %#v", tc.text, got)
		}
		visible, err := transcriptItemModel(item)
		if err != nil || (visible == nil) != (tc.want == "marker") {
			t.Fatalf("saved summary visibility: %#v, %v", visible, err)
		}
	}
}
