package runtime

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"sync/atomic"
	"testing"

	"github.com/kpsuperplane/noema/internal/store"
)

const validA2UIForm = `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1","sendDataModel":true}}
{"version":"v0.9.1","updateDataModel":{"surfaceId":"main","value":{"form":{"name":"Grace"}}}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Column","children":["name","submit"]},{"id":"name","component":"TextField","label":"Name","value":{"path":"/form/name"},"variant":"obscured"},{"id":"submit","component":"Button","child":"label","action":{"event":{"name":"submit","context":{"source":"surface","model":{"path":"/"}}}},"variant":"primary"},{"id":"label","component":"Text","text":"Continue"}]}}`

func TestA2UIValidationReducesPinnedCatalogAndRejectsUnsafeOrUnboundedInput(t *testing.T) {
	batch, repair := reduceA2UI("conversation:one", validA2UIForm)
	if repair != nil || len(batch.Surfaces) != 1 || len(batch.Messages) != 3 {
		t.Fatalf("valid batch = %#v, %#v", batch, repair)
	}
	var surface *a2uiSurface
	for _, surface = range batch.Surfaces {
	}
	field := surface.Components["name"].(map[string]any)
	if surface.Revision != 3 || len(surface.Actions) != 1 || field["variant"] != "obscured" ||
		surface.NamespacedSurfaceID != "a2ui/conversation%3Aone/main" {
		t.Fatalf("reduced surface = %#v", surface)
	}
	invalid := map[string]string{
		"version":      strings.Replace(validA2UIForm, `"v0.9.1"`, `"v1"`, 1),
		"html":         strings.Replace(validA2UIForm, "Continue", "<script>", 1),
		"component":    strings.Replace(validA2UIForm, `"Text"`, `"Image"`, 1),
		"unbound":      strings.Replace(validA2UIForm, `{"path":"/form/name"}`, `"Grace"`, 1),
		"reference":    strings.Replace(validA2UIForm, `"child":"label"`, `"child":"missing"`, 1),
		"string bound": strings.Replace(validA2UIForm, "Continue", strings.Repeat("x", a2uiStringLimit+1), 1),
	}
	for name, input := range invalid {
		t.Run(name, func(t *testing.T) {
			if _, failure := reduceA2UI("conversation:one", input); failure == nil {
				t.Fatal("invalid batch succeeded")
			}
		})
	}
	readOnly := `{"version":"v0.9.1","createSurface":{"surfaceId":"summary","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"summary","components":[{"id":"root","component":"Text","text":"Done"}]}}`
	readOnlyBatch, failure := reduceA2UI("conversation:one", readOnly)
	if failure != nil || len(readOnlyBatch.Surfaces) != 1 {
		t.Fatalf("action-free batch = %#v, %#v", readOnlyBatch, failure)
	}
	for _, surface := range readOnlyBatch.Surfaces {
		if len(surface.Actions) != 0 {
			t.Fatalf("action-free surface = %#v", surface)
		}
	}
	if _, err := parseA2UIArguments(json.RawMessage(`{"jsonl":"{}","extra":true}`)); err == nil {
		t.Fatal("unknown outer field succeeded")
	}
	bindings := resolveA2UIBindings(map[string]any{"choice": map[string]any{"path": "/items/1"}},
		map[string]any{"items": []any{"first", "second"}}).(map[string]any)
	if bindings["choice"] != "second" {
		t.Fatalf("array binding = %#v", bindings)
	}
	contextValue, ok := resolveA2UIActionContext(map[string]any{"path": "/literal"}, map[string]any{"literal": "bound"})
	if !ok || contextValue["path"] != "/literal" {
		t.Fatalf("outer action context = %#v, %t", contextValue, ok)
	}
	if value := resolveA2UIBindings(map[string]any{"path": "/items/01"},
		map[string]any{"items": []any{"first", "second"}}); value != nil {
		t.Fatalf("noncanonical array binding = %#v", value)
	}
}

func TestChatA2UIActionPausesValidatesAndResumesExactPrivateInput(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	requests := make(chan map[string]any, 2)
	var count atomic.Int32
	replaceDefaultTransport(t, roundTripFunc(func(request *http.Request) (*http.Response, error) {
		var body map[string]any
		if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
			return nil, err
		}
		requests <- body
		if count.Add(1) == 1 {
			return a2uiToolResponse(validA2UIForm), nil
		}
		return openRouterStreamResponse("Form received."), nil
	}))
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Show a form"}); err != nil {
		t.Fatal(err)
	}
	first := collectCompletedTurns(t, events, 1)
	var interactionID string
	for _, event := range first {
		if event.Item == nil || event.Item.Kind != store.ConversationA2UICard {
			continue
		}
		projection, _ := event.Item.Payload["payload"].(map[string]any)
		interactionID, _ = projection["interaction_id"].(string)
	}
	if interactionID == "" {
		t.Fatalf("pending events = %#v", first)
	}
	model := map[string]any{"form": map[string]any{"name": "Ada private"}}
	if _, err := chat.SendA2UIAction(context.Background(), conversation.ID, interactionID, 1,
		"main", "submit", "submit", map[string]any{"source": "forged", "model": model}, model, nil); err == nil {
		t.Fatal("forged context succeeded")
	}
	clientID := "a2ui-client"
	contextValue := map[string]any{"source": "surface", "model": model}
	if _, err := chat.SendA2UIAction(context.Background(), conversation.ID, interactionID, 1,
		"main", "submit", "submit", contextValue, model, &clientID); err != nil {
		t.Fatal(err)
	}
	second := collectCompletedTurns(t, events, 1)
	if !eventHasA2UILifecycle(second, "answered") {
		t.Fatalf("settled events = %#v", second)
	}
	<-requests
	continuation := <-requests
	messages := continuation["messages"].([]any)
	last := messages[len(messages)-1].(map[string]any)
	content, _ := last["content"].(string)
	if last["role"] != "tool" || !strings.Contains(content, `"interaction_id":"`+interactionID+`"`) ||
		!strings.Contains(content, "Ada private") || !strings.Contains(content, `"action_name":"submit"`) {
		t.Fatalf("A2UI continuation = %#v", continuation)
	}
	if _, err := chat.SendA2UIAction(context.Background(), conversation.ID, interactionID, 1,
		"main", "submit", "submit", contextValue, model, nil); err == nil {
		t.Fatal("stale action succeeded")
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 30)
	if err != nil || len(page.Items) == 0 {
		t.Fatalf("durable replay = %#v, %v", page, err)
	}
}

func a2uiToolResponse(jsonl string) *http.Response {
	arguments, _ := json.Marshal(map[string]any{"jsonl": jsonl})
	chunk, _ := json.Marshal(map[string]any{"id": "a2ui-response", "model": "openai/gpt-5.6-luna",
		"choices": []any{map[string]any{"index": 0, "delta": map[string]any{"content": "Fill this form.",
			"tool_calls": []any{map[string]any{"index": 0, "id": "a2ui-call", "type": "function",
				"function": map[string]any{"name": "present_a2ui", "arguments": string(arguments)}}}}, "finish_reason": "tool_calls"}},
		"usage": map[string]any{"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}})
	body := fmt.Sprintf("data: %s\n\ndata: [DONE]\n\n", chunk)
	return &http.Response{StatusCode: http.StatusOK, Header: http.Header{"Content-Type": {"text/event-stream"}}, Body: io.NopCloser(strings.NewReader(body))}
}

func eventHasA2UILifecycle(events []Event, lifecycle string) bool {
	for _, event := range events {
		if event.Item == nil || event.Item.Kind != store.ConversationA2UICard {
			continue
		}
		projection, _ := event.Item.Payload["payload"].(map[string]any)
		if projection["lifecycle"] == lifecycle {
			return true
		}
	}
	return false
}
