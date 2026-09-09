package provider

import (
	"encoding/json"
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
		a.output = append(a.output, GenerationOutput{Kind: kind, ID: id, Index: index, Status: "completed"})
		if kind == "message" {
			emit(StreamEvent{Kind: MessageStarted, Index: index, ID: id})
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
	emit(StreamEvent{Kind: eventKind, Index: index, ID: part.ID, Delta: delta})
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
