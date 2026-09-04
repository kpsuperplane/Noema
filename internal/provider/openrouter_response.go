package provider

import (
	"encoding/json"
	"errors"
	"strings"
)

// OpenRouterToolCall is one validated native tool call.
type OpenRouterToolCall struct {
	Index          int
	ProviderCallID string
	ProviderName   string
	Name           string
	Payload        json.RawMessage
}

// OpenRouterReasoningItem holds provider reasoning for replay.
type OpenRouterReasoningItem struct {
	ID               string
	EncryptedContent string
	Summary          []string
	ProviderDetails  []json.RawMessage
}

func normalizeOpenRouterGeneration(
	request OpenRouterGenerateRequest,
	parsed ChatStreamResult,
	toolNames openRouterToolNameMap,
	finishReason string,
) (OpenRouterGenerationResult, error) {
	toolCalls, err := normalizeOpenRouterToolCalls(parsed.ToolCalls, request.ToolTransport, toolNames)
	if err != nil {
		return OpenRouterGenerationResult{}, err
	}
	if parsed.Text == "" && len(toolCalls) == 0 {
		return OpenRouterGenerationResult{}, errors.New("OpenRouter response did not contain assistant content or tool calls")
	}
	model := parsed.Model
	if model == "" {
		model = strings.TrimSpace(request.Model)
	}
	return OpenRouterGenerationResult{
		ID: parsed.ID, Model: model, Text: parsed.Text, FinishReason: finishReason,
		Usage: parsed.Usage, ToolCalls: toolCalls,
		Reasoning: normalizeOpenRouterReasoning(parsed.Reasoning),
		Citations: append([]Citation(nil), parsed.Citations...),
		Searches:  append([]HostedSearch(nil), parsed.Searches...),
	}, nil
}

func normalizeOpenRouterToolCalls(
	calls []ToolCall,
	transport OpenRouterToolTransport,
	names openRouterToolNameMap,
) ([]OpenRouterToolCall, error) {
	if len(calls) != 0 && transport != OpenRouterToolTransportNative {
		return nil, errors.New("OpenRouter returned native tool calls when native tools were disabled")
	}
	result := make([]OpenRouterToolCall, 0, len(calls))
	seen := make(map[string]struct{}, len(calls))
	for _, call := range calls {
		if strings.TrimSpace(call.ID) == "" {
			return nil, errors.New("OpenRouter native tool call is missing call id")
		}
		if strings.TrimSpace(call.Name) == "" {
			return nil, errors.New("OpenRouter native tool call is missing name")
		}
		rule, exists := names.providerToRule[call.Name]
		if !exists {
			return nil, errors.New("OpenRouter returned an unadvertised tool name")
		}
		if _, duplicate := seen[call.ID]; duplicate {
			return nil, errors.New("OpenRouter returned a duplicate provider call id")
		}
		seen[call.ID] = struct{}{}
		payload, err := openRouterJSONObject(json.RawMessage(call.Arguments))
		if err != nil {
			return nil, errors.New("OpenRouter native tool arguments must be a JSON object")
		}
		if rule.strict {
			restoreOpenRouterOptionalNulls(payload, rule.sourceSchema)
		}
		encoded, err := json.Marshal(payload)
		if err != nil {
			return nil, errors.New("OpenRouter native tool arguments are invalid")
		}
		result = append(result, OpenRouterToolCall{
			Index: call.Index, ProviderCallID: call.ID, ProviderName: call.Name,
			Name: rule.canonical, Payload: encoded,
		})
	}
	return result, nil
}

func normalizeOpenRouterReasoning(details []json.RawMessage) []OpenRouterReasoningItem {
	if len(details) == 0 {
		return nil
	}
	item := OpenRouterReasoningItem{ProviderDetails: cloneRawMessages(details)}
	for _, detail := range details {
		value, err := decodeOpenRouterJSON(detail)
		if err != nil {
			continue
		}
		object := jsonObject(value)
		switch jsonString(object["type"]) {
		case "reasoning.encrypted":
			if encrypted := firstJSONText(object, "data", "encrypted_content"); encrypted != "" {
				item.EncryptedContent = encrypted
			}
			if id := jsonString(object["id"]); id != "" {
				item.ID = id
			}
		case "reasoning.summary":
			if summary := firstJSONText(object, "summary", "text"); summary != "" {
				item.Summary = append(item.Summary, summary)
			}
		}
	}
	return []OpenRouterReasoningItem{item}
}

func dedupeOpenRouterStreamEvents(onEvent func(StreamEvent)) func(StreamEvent) {
	if onEvent == nil {
		return nil
	}
	seenSearches := make(map[string]struct{})
	return func(event StreamEvent) {
		if event.Kind == HostedSearchStarted && event.ID != "" {
			if _, exists := seenSearches[event.ID]; exists {
				return
			}
			seenSearches[event.ID] = struct{}{}
		}
		onEvent(event)
	}
}
