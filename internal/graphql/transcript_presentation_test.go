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
	for _, tc := range []struct{ name, kind, phase, want string }{
		{"progress", "message", "commentary", "marker"},
		{"reasoning", "reasoning", "", "bubble"},
		{"reasoning with commentary phase", "reasoning", "commentary", "bubble"},
		{"answer", "message", "final_answer", "bubble"},
		{"untyped response", "message", "", "bubble"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			metadata := map[string]any{"provider_output_kind": tc.kind, "provider_phase": tc.phase, "phase": "commentary", "model": "ordinary-model-id"}
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
			if page.Items[0].Metadata["presentation"] != tc.want || !reflect.DeepEqual(page.Items[0].Metadata, live.Metadata) {
				t.Fatalf("replay/live presentation: %#v / %#v", page.Items[0].Metadata, live.Metadata)
			}
			if live.Item.(model.AssistantText).Text != source.ContentText || live.Metadata["model"] != metadata["model"] || metadata["presentation"] != nil {
				t.Fatal("Chat content or saved metadata changed")
			}
			section := map[string]any{"kind": tc.kind, "phase": tc.phase, "text": source.ContentText, "status": "running", "index": float64(3)}
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
		{map[string]any{"phase": "commentary"}, "marker"},
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
