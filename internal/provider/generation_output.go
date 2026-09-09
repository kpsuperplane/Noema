package provider

import (
	"encoding/json"
	"sort"
	"strings"
)

func readableCodexOutput(index int, item map[string]json.RawMessage) []GenerationOutput {
	kind, _ := rawString(item["type"])
	id, _ := rawString(item["id"])
	phase, _ := rawString(item["phase"])
	if kind == "message" {
		value, _, err := codexOutputText(item, 0)
		if err != nil || value == "" {
			return nil
		}
		return []GenerationOutput{{Kind: kind, ID: id, Index: index, Phase: phase, Text: value, Status: "completed"}}
	}
	if kind != "reasoning" {
		return nil
	}
	var result []GenerationOutput
	for _, field := range []string{"summary", "content"} {
		var parts []struct{ Type, Text string }
		if json.Unmarshal(item[field], &parts) != nil {
			continue
		}
		for section, part := range parts {
			if field == "content" {
				section += maxItems
			}
			if (part.Type == "summary_text" || part.Type == "reasoning_text") && strings.TrimSpace(part.Text) != "" {
				result = append(result, GenerationOutput{Kind: kind, ID: id, Index: index, SectionIndex: section, Text: part.Text, Status: "completed"})
			}
		}
	}
	return result
}

func emitCodexCompletedOutput(index int, raw json.RawMessage, streamed string, emit func(StreamEvent)) {
	var item map[string]json.RawMessage
	if json.Unmarshal(raw, &item) != nil {
		return
	}
	for _, part := range readableCodexOutput(index, item) {
		kind := MessageCompleted
		if part.Kind == "reasoning" {
			kind = ReasoningCompleted
		} else if streamed != "" {
			part.Text = streamed
		}
		emit(StreamEvent{Kind: kind, Index: part.Index, SectionIndex: part.SectionIndex, ID: part.ID, Phase: part.Phase, Text: part.Text})
	}
}

func (a *chatAccumulator) updateReadable(key, kind, id, value string, emit func(StreamEvent)) {
	if value == "" {
		return
	}
	if a.outputKeys == nil {
		a.outputKeys = map[string]int{}
	}
	index, exists := a.outputKeys[key]
	if !exists {
		index = len(a.output)
		a.outputKeys[key] = index
		a.output = append(a.output, GenerationOutput{Kind: kind, ID: id, Index: a.nextOutputIndex, Status: "completed"})
		a.nextOutputIndex++
		if kind == "message" {
			emit(StreamEvent{Kind: MessageStarted, Index: a.output[index].Index, ID: id})
		}
	}
	part := &a.output[index]
	delta := strings.TrimPrefix(value, part.Text)
	if value == part.Text {
		return
	}
	part.Text = value
	if id != "" {
		part.ID = id
	}
	eventKind := TextDelta
	if kind == "reasoning" {
		eventKind = ReasoningDelta
	}
	emit(StreamEvent{Kind: eventKind, Index: part.Index, ID: part.ID, Delta: delta})
}

func (a *chatAccumulator) finish(emit func(StreamEvent)) ChatStreamResult {
	for _, part := range a.output {
		kind := MessageCompleted
		if part.Kind == "reasoning" {
			kind = ReasoningCompleted
		}
		emit(StreamEvent{Kind: kind, Index: part.Index, ID: part.ID, Text: part.Text})
	}
	return a.result()
}

func hasReadableReasoning(details []json.RawMessage) bool {
	for _, raw := range details {
		var item map[string]any
		if json.Unmarshal(raw, &item) == nil && (item["type"] == "reasoning.text" || item["type"] == "reasoning.summary") && firstJSONText(item, "summary", "text") != "" {
			return true
		}
	}
	return false
}

// ReplayMessages retains provider output order without replaying displayed
// reasoning summaries a second time alongside their original provider items.
func (result GenerationResult) ReplayMessages() []GenerationMessage {
	type indexedMessage struct {
		index   int
		message GenerationMessage
	}
	var ordered []indexedMessage
	for _, reasoning := range result.Reasoning {
		if reasoning.EncryptedContent == "" && len(reasoning.ProviderDetails) == 0 && reasoning.ID == "" {
			continue
		}
		ordered = append(ordered, indexedMessage{reasoning.Index, GenerationMessage{
			Role: "assistant", ReasoningID: reasoning.ID, EncryptedReasoning: reasoning.EncryptedContent,
			ReasoningDetails: cloneRawMessages(reasoning.ProviderDetails),
		}})
	}
	for _, search := range result.Searches {
		searchCopy := search
		ordered = append(ordered, indexedMessage{search.Index, GenerationMessage{Role: "hosted_web_search", HostedSearch: &searchCopy}})
	}
	hasMessage := false
	for _, output := range result.Output {
		if output.Kind != "message" {
			continue
		}
		hasMessage = true
		ordered = append(ordered, indexedMessage{output.Index, GenerationMessage{
			Role: "assistant", Content: output.Text, Phase: output.Phase, ProviderItemID: output.ID,
		}})
	}
	legacyIndex := 0
	if !hasMessage && result.Text != "" {
		for _, item := range ordered {
			if item.index >= legacyIndex {
				legacyIndex = item.index + 1
			}
		}
		ordered = append(ordered, indexedMessage{legacyIndex, GenerationMessage{Role: "assistant", Content: result.Text}})
	}
	for _, call := range result.ToolCalls {
		index := call.Index
		if !hasMessage && result.Text != "" {
			index = legacyIndex + 1
		}
		ordered = append(ordered, indexedMessage{index, GenerationMessage{Role: "assistant", ToolCalls: []ReplayToolCall{{
			ProviderItemID: call.ProviderItemID, ProviderCallID: call.ProviderCallID,
			Name: call.Name, ProviderName: call.ProviderName, Arguments: append(json.RawMessage(nil), call.Payload...),
		}}}})
	}
	sort.SliceStable(ordered, func(i, j int) bool { return ordered[i].index < ordered[j].index })
	messages := make([]GenerationMessage, 0, len(ordered))
	for _, item := range ordered {
		messages = append(messages, item.message)
	}
	return messages
}
