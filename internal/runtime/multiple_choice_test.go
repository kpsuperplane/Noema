package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/store"
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
	var completedCall bool
	for _, event := range second {
		if event.Item != nil && event.Item.Kind == "multiple_choice_selection" {
			selected, _ = event.Item.Payload["selected_options"].([]any)
		}
		completedCall = completedCall || event.Item != nil && event.Item.Kind == "tool_call" && event.Item.Status == "completed"
	}
	if len(selected) != 2 || selected[0].(map[string]any)["id"] != "b" || selected[1].(map[string]any)["id"] != "a" {
		t.Fatalf("stored selected options = %#v", selected)
	}
	if !completedCall {
		t.Fatal("completed multiple-choice call was not published")
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

func TestMultipleChoiceResumeAuthorityRejectsChanges(t *testing.T) {
	chat, database, _ := chatFixture(t)
	assignment, err := chat.primaryAssignment(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	revision, digest, err := chat.multipleChoiceAuthority(context.Background(), assignment)
	if err != nil {
		t.Fatal(err)
	}
	payload := map[string]any{"credential_revision": float64(revision), "tool_catalog_digest": digest}
	if _, err := chat.validateMultipleChoiceAuthority(context.Background(), payload, assignment); err != nil {
		t.Fatal(err)
	}
	payload["tool_catalog_digest"] = "changed"
	if _, err := chat.validateMultipleChoiceAuthority(context.Background(), payload, assignment); err == nil {
		t.Fatal("changed tool catalog succeeded")
	}
	payload["tool_catalog_digest"] = digest
	account, err := database.ProviderAccount(context.Background(), assignment.ProviderAccountID)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.UpdateProviderCredential(
		context.Background(), account.ID, revision, account.AuthMethod, true, account.Metadata, time.Now(),
	); err != nil {
		t.Fatal(err)
	}
	if _, err := chat.validateMultipleChoiceAuthority(context.Background(), payload, assignment); err == nil {
		t.Fatal("changed provider credential succeeded")
	}
}

func TestMultipleChoiceWaitReleasesStateLockAndKeepsExactResult(t *testing.T) {
	_, database, conversation := chatFixture(t)
	waiter := &Chat{database: database, ctx: context.Background(), choices: make(chan choiceResolution, 1)}
	done := make(chan error, 1)
	go func() {
		_, err := waiter.SendMultipleChoiceSelection(context.Background(), conversation.ID, "prompt", []string{"one"}, nil)
		done <- err
	}()
	request := <-waiter.choices
	locked := make(chan struct{})
	go func() {
		waiter.stateMu.Lock()
		close(locked)
		waiter.stateMu.Unlock()
	}()
	select {
	case <-locked:
	case <-time.After(time.Second):
		t.Fatal("multiple-choice wait retained the Chat state lock")
	}
	request.reply <- errors.New("test complete")
	if err := <-done; err == nil {
		t.Fatal("test selection unexpectedly succeeded")
	}

	large := strings.Repeat("private ordinary choice ", 2<<10)
	item := store.ConversationItem{Payload: map[string]any{"metadata": map[string]any{"action": map[string]any{
		"provider_call_id": "call", "provider_name": "present_multiple_choice", "name": presentMultipleChoiceName,
		"success": true, "payload": map[string]any{"status": "resolved", "selected_options": []any{map[string]any{"id": "one", "label": large}}},
	}}}}
	result, err := storedMultipleChoiceToolResult(item)
	if err != nil || len(result.Payload) <= modelToolPayloadLimit || !bytes.Contains(result.Payload, []byte(large)) {
		t.Fatalf("exact multiple-choice result length = %d, %v", len(result.Payload), err)
	}
}
