package provider

import (
	"context"
	"encoding/json"
	"io"
	"strings"
	"testing"
)

func TestCodexReadableOutputPreservesSectionsAndReplayPhase(t *testing.T) {
	frames := []string{
		`{"type":"response.output_item.added","output_index":0,"item":{"type":"message","id":"m1","phase":"commentary","content":[]}}`,
		`{"type":"response.output_text.delta","output_index":0,"item_id":"m1","delta":"Checking."}`,
		`{"type":"response.output_item.done","output_index":0,"item":{"type":"message","id":"m1","phase":"commentary","content":[{"type":"output_text","text":"Checking."}]}}`,
		`{"type":"response.reasoning_summary_text.delta","output_index":1,"item_id":"r1","summary_index":0,"delta":"First"}`,
		`{"type":"response.reasoning_summary_text.done","output_index":1,"item_id":"r1","summary_index":0,"text":"First"}`,
		`{"type":"response.output_item.done","output_index":1,"item":{"type":"reasoning","id":"r1","encrypted_content":"opaque","summary":[{"type":"summary_text","text":"First"},{"type":"summary_text","text":"Second\nsection"}]}}`,
		`{"type":"response.output_item.done","output_index":2,"item":{"type":"message","id":"m2","phase":"final_answer","content":[{"type":"output_text","text":"Done."}]}}`,
		`{"type":"response.completed","response":{"id":"response","status":"completed"}}`,
	}
	var events []StreamEvent
	stream := "data: " + strings.Join(frames, "\n\ndata: ") + "\n\n"
	parsed, err := parseCodexGenerationStream(context.Background(), io.NopCloser(strings.NewReader(stream)), func(e StreamEvent) { events = append(events, e) })
	if err != nil {
		t.Fatal(err)
	}
	result, err := normalizeCodexGeneration(GenerateRequest{}, parsed, openRouterToolNameMap{})
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Reasoning) != 1 || result.Reasoning[0].Index != 1 || len(result.Output) != 4 || result.Text != "Checking.Done." || result.Output[0].Phase != "commentary" || result.Output[3].Phase != "final_answer" || result.Output[2].Text != "Second\nsection" || result.Output[2].SectionIndex != 1 {
		t.Fatalf("output: %#v", result.Output)
	}
	if events[0].Kind != MessageStarted || events[1].Phase != "commentary" || events[1].ID != "m1" {
		t.Fatalf("events: %#v", events)
	}
	for _, event := range events {
		if strings.Contains(event.Text+event.Delta, "opaque") {
			t.Fatal("encrypted content exposed")
		}
	}
	for _, phase := range []string{"commentary", "final_answer", ""} {
		items, err := lowerCodexMessage(GenerationMessage{Role: "assistant", Content: "Text", Phase: phase, ProviderItemID: "m1"}, openRouterToolNameMap{})
		if err != nil {
			t.Fatal(err)
		}
		item := items[0].(map[string]any)
		if item["id"] != "m1" {
			t.Fatal("message identity changed")
		}
		if phase == "" {
			if _, ok := item["phase"]; ok {
				t.Fatal("phase invented")
			}
		} else if item["phase"] != phase {
			t.Fatal("phase changed")
		}
	}
}

func TestChatReadableOutputStreamsPlainAndStructuredReasoningOnce(t *testing.T) {
	for _, field := range []string{"reasoning", "reasoning_content"} {
		stream := "data: {\"choices\":[{\"delta\":{\"" + field + "\":\"Checking\",\"reasoning_details\":[{\"type\":\"reasoning.text\",\"id\":\"r\",\"text\":\"Checking\"}]}}]}\n\n" +
			"data: {\"choices\":[{\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.summary\",\"id\":\"s\",\"summary\":\"Next\"},{\"type\":\"reasoning.encrypted\",\"data\":\"opaque\"}],\"content\":\"Answer\"}}]}\n\ndata: [DONE]\n\n"
		var events []StreamEvent
		result, err := ParseChatStream(context.Background(), io.NopCloser(strings.NewReader(stream)), func(e StreamEvent) { events = append(events, e) })
		if err != nil {
			t.Fatal(err)
		}
		if len(result.Output) != 3 || result.Output[0].Text != "Checking" || result.Output[1].Text != "Next" || result.Output[2].Text != "Answer" {
			t.Fatalf("output: %#v", result.Output)
		}
		encoded, _ := json.Marshal(result.Output)
		if strings.Contains(string(encoded), "opaque") {
			t.Fatal("encrypted content exposed")
		}
		if events[0].Kind != ReasoningDelta || events[len(events)-1].Kind != MessageCompleted {
			t.Fatalf("events: %#v", events)
		}
	}
}

func TestReplayMessagesPreservesMixedProviderOrderOnce(t *testing.T) {
	result := GenerationResult{
		Text: "Checking.Done.",
		Output: []GenerationOutput{
			{Kind: "message", Index: 0, ID: "m1", Phase: "commentary", Text: "Checking."},
			{Kind: "reasoning", Index: 1, Text: "Readable summary"},
			{Kind: "message", Index: 4, ID: "m2", Phase: "final_answer", Text: "Done."},
		},
		Reasoning: []GenerationReasoning{{Index: 1, ID: "r1", EncryptedContent: "opaque", ProviderDetails: []json.RawMessage{json.RawMessage(`{"type":"reasoning","id":"r1","encrypted_content":"opaque"}`)}}, {Index: 5, ID: "r2", EncryptedContent: "opaque-only"}},
		Searches:  []HostedSearch{{Index: 2, ID: "s1"}},
		ToolCalls: []GenerationToolCall{{Index: 3, ProviderCallID: "call1", Name: "inspect", Payload: json.RawMessage(`{}`)}},
	}
	messages := result.ReplayMessages()
	if len(messages) != 6 || messages[0].ProviderItemID != "m1" || messages[0].Phase != "commentary" || messages[1].ReasoningID != "r1" || messages[2].HostedSearch == nil || len(messages[3].ToolCalls) != 1 || messages[4].ProviderItemID != "m2" || messages[5].EncryptedReasoning != "opaque-only" {
		t.Fatalf("replay: %#v", messages)
	}
	for _, message := range messages {
		if message.Content == "Readable summary" {
			t.Fatal("summary duplicated as assistant text")
		}
	}
	if fallback := (GenerationResult{Text: "Legacy"}).ReplayMessages(); len(fallback) != 1 || fallback[0].Content != "Legacy" || fallback[0].Phase != "" {
		t.Fatalf("fallback: %#v", fallback)
	}
}

func TestChatOutputIndexesKeepTextBeforeNativeCall(t *testing.T) {
	stream := "data: {\"choices\":[{\"delta\":{\"reasoning\":\"Checking\"}}]}\n\n" +
		"data: {\"choices\":[{\"delta\":{\"content\":\"Progress\",\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"inspect\",\"arguments\":\"{}\"}}]}}]}\n\ndata: [DONE]\n\n"
	parsed, err := ParseChatStream(context.Background(), io.NopCloser(strings.NewReader(stream)), nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(parsed.Output) != 2 || parsed.Output[1].Index != 1 || len(parsed.ToolCalls) != 1 || parsed.ToolCalls[0].OutputIndex != 2 {
		t.Fatalf("ordered output: %#v %#v", parsed.Output, parsed.ToolCalls)
	}
}
