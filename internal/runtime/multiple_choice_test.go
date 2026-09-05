package runtime

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"strings"
	"sync/atomic"
	"testing"
)

func TestMultipleChoiceArgumentsAreStrictAndNormalized(t *testing.T) {
	valid, err := parseMultipleChoiceArguments(json.RawMessage(`{
  "prompt":" Pick one ","selection_mode":"pick_one",
  "options":[{"id":" b ","label":" Beta "},{"id":"a","label":"Alpha"}]
}`))
	if err != nil || valid.Prompt != "Pick one" || valid.Options[0].ID != "b" || valid.Options[0].Label != "Beta" {
		t.Fatalf("normalized choice = %#v, %v", valid, err)
	}
	for name, payload := range map[string]string{
		"unknown field": `{"prompt":"p","selection_mode":"pick_one","options":[{"id":"a","label":"A"}],"extra":true}`,
		"empty prompt":  `{"prompt":" ","selection_mode":"pick_one","options":[{"id":"a","label":"A"}]}`,
		"bad mode":      `{"prompt":"p","selection_mode":"all","options":[{"id":"a","label":"A"}]}`,
		"duplicate":     `{"prompt":"p","selection_mode":"pick_many","options":[{"id":"a","label":"A"},{"id":" a ","label":"Again"}]}`,
		"option field":  `{"prompt":"p","selection_mode":"pick_one","options":[{"id":"a","label":"A","x":1}]}`,
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := parseMultipleChoiceArguments(json.RawMessage(payload)); err == nil {
				t.Fatal("invalid choice arguments succeeded")
			}
		})
	}
	if _, err := parseMultipleChoiceArguments(json.RawMessage(strings.Repeat(" ", 256*1024+1))); err == nil {
		t.Fatal("oversized choice arguments succeeded")
	}
}

func TestChatMultipleChoicePausesAndResumesExactToolResult(t *testing.T) {
	chat, _, conversation := chatFixture(t)
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
			stream := `data: {"id":"choice-response","model":"openai/gpt-5.6-luna","choices":[{"index":0,"delta":{"content":"Choose.","tool_calls":[{"index":0,"id":"choice-call","type":"function","function":{"name":"present_multiple_choice","arguments":"{\"prompt\":\"Which?\",\"selection_mode\":\"pick_many\",\"options\":[{\"id\":\"b\",\"label\":\"Beta\"},{\"id\":\"a\",\"label\":\"Alpha\"}]}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}

data: [DONE]

`
			return &http.Response{StatusCode: http.StatusOK, Header: http.Header{"Content-Type": {"text/event-stream"}}, Body: io.NopCloser(strings.NewReader(stream))}, nil
		}
		return openRouterStreamResponse("Selected."), nil
	}))
	clientID := "choice-client"
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "Ask me", ClientMessageID: &clientID,
	}); err != nil {
		t.Fatal(err)
	}
	first := collectCompletedTurns(t, events, 1)
	var promptID string
	for _, event := range first {
		if event.Item != nil && event.Item.Kind == "multiple_choice_prompt" {
			promptID = event.Item.ID
		}
	}
	if promptID == "" {
		t.Fatalf("choice prompt events = %#v", first)
	}
	selectionClient := "selection-client"
	if _, err := chat.SendMultipleChoiceSelection(
		context.Background(), conversation.ID, promptID, []string{"a", "b"}, &selectionClient,
	); err != nil {
		t.Fatal(err)
	}
	second := collectCompletedTurns(t, events, 1)
	var selected []any
	for _, event := range second {
		if event.Item != nil && event.Item.Kind == "multiple_choice_selection" {
			selected, _ = event.Item.Payload["selected_options"].([]any)
		}
	}
	if len(selected) != 2 || selected[0].(map[string]any)["id"] != "b" || selected[1].(map[string]any)["id"] != "a" {
		t.Fatalf("stored selected options = %#v", selected)
	}
	<-requests
	continuation := <-requests
	messages := continuation["messages"].([]any)
	last := messages[len(messages)-1].(map[string]any)
	if last["role"] != "tool" || !strings.Contains(last["content"].(string), `"status":"resolved"`) ||
		!strings.Contains(last["content"].(string), `"id":"b"`) {
		t.Fatalf("choice continuation = %#v", continuation)
	}
	if _, err := chat.SendMultipleChoiceSelection(
		context.Background(), conversation.ID, promptID, []string{"b"}, nil,
	); err == nil {
		t.Fatal("repeated choice selection succeeded")
	}
}
