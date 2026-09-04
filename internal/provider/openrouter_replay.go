package provider

import (
	"encoding/json"
	"errors"
	"strings"
)

func lowerOpenRouterMessage(
	message GenerationMessage,
	toolNames openRouterToolNameMap,
) (openRouterMessagePayload, bool, error) {
	if message.Role == "developer" {
		if len(message.ToolCalls) != 0 || message.ToolResult != nil ||
			len(message.ReasoningDetails) != 0 || message.EncryptedReasoning != "" {
			return openRouterMessagePayload{}, false, errors.New("OpenRouter developer message is invalid")
		}
		if strings.TrimSpace(message.Content) == "" {
			return openRouterMessagePayload{}, false, nil
		}
		content := wrapOpenRouterApplicationContext(message.Content)
		return openRouterMessagePayload{Role: "user", Content: &content}, true, nil
	}
	if message.Role != "system" && message.Role != "user" &&
		message.Role != "assistant" && message.Role != "tool" {
		return openRouterMessagePayload{}, false, errors.New("OpenRouter generation message role is invalid")
	}
	if message.Role == "tool" {
		return lowerOpenRouterToolResultMessage(message, toolNames)
	}
	if (message.Role == "system" || message.Role == "user") &&
		(len(message.ToolCalls) != 0 || message.ToolResult != nil ||
			len(message.ReasoningDetails) != 0 || message.EncryptedReasoning != "") {
		return openRouterMessagePayload{}, false, errors.New("OpenRouter generation message fields are invalid")
	}

	lowered := openRouterMessagePayload{Role: message.Role}
	if strings.TrimSpace(message.Content) != "" {
		content := message.Content
		lowered.Content = &content
	}
	if message.Role == "assistant" {
		for _, call := range message.ToolCalls {
			wire, err := lowerOpenRouterReplayCall(call, toolNames)
			if err != nil {
				return openRouterMessagePayload{}, false, err
			}
			lowered.ToolCalls = append(lowered.ToolCalls, wire)
		}
		details, err := lowerOpenRouterReasoning(message)
		if err != nil {
			return openRouterMessagePayload{}, false, err
		}
		lowered.ReasoningDetails = details
	}
	keep := lowered.Content != nil || len(lowered.ToolCalls) != 0 || len(lowered.ReasoningDetails) != 0
	return lowered, keep, nil
}

func lowerOpenRouterReplayCall(
	call ReplayToolCall,
	toolNames openRouterToolNameMap,
) (openRouterToolCallPayload, error) {
	if strings.TrimSpace(call.ProviderCallID) == "" ||
		len(call.ProviderCallID) > openRouterProviderCallIDLimit || strings.TrimSpace(call.Name) == "" {
		return openRouterToolCallPayload{}, errors.New("OpenRouter replay tool call is invalid")
	}
	providerName := call.ProviderName
	if providerName == "" {
		providerName = toolNames.canonicalToName[call.Name]
		if providerName == "" {
			providerName = openRouterProviderSafeName(call.Name)
		}
	}
	arguments, err := openRouterJSONObject(call.Arguments)
	if err != nil {
		return openRouterToolCallPayload{}, errors.New("OpenRouter replay tool arguments must be an object")
	}
	encoded, err := json.Marshal(arguments)
	if err != nil {
		return openRouterToolCallPayload{}, errors.New("OpenRouter replay tool arguments are invalid")
	}
	return openRouterToolCallPayload{
		ID: call.ProviderCallID, Type: "function",
		Function: openRouterToolCallFunctionPayload{Name: providerName, Arguments: string(encoded)},
	}, nil
}

func lowerOpenRouterToolResultMessage(
	message GenerationMessage,
	toolNames openRouterToolNameMap,
) (openRouterMessagePayload, bool, error) {
	if len(message.ToolCalls) != 0 || len(message.ReasoningDetails) != 0 ||
		message.EncryptedReasoning != "" {
		return openRouterMessagePayload{}, false, errors.New("OpenRouter tool result message is invalid")
	}
	if message.ToolResult == nil {
		if strings.TrimSpace(message.ToolCallID) == "" || strings.TrimSpace(message.Content) == "" {
			return openRouterMessagePayload{}, false, errors.New("OpenRouter tool result is required")
		}
		content := message.Content
		return openRouterMessagePayload{
			Role: "tool", Content: &content, ToolCallID: message.ToolCallID,
		}, true, nil
	}
	result := message.ToolResult
	if strings.TrimSpace(result.ProviderCallID) == "" ||
		len(result.ProviderCallID) > openRouterProviderCallIDLimit || strings.TrimSpace(result.Name) == "" {
		return openRouterMessagePayload{}, false, errors.New("OpenRouter tool result is invalid")
	}
	providerName := result.ProviderName
	if providerName == "" {
		providerName = toolNames.canonicalToName[result.Name]
	}
	var providerNameValue any
	if providerName != "" {
		providerNameValue = providerName
	}
	payload, err := decodeOptionalOpenRouterJSON(result.Payload)
	if err != nil {
		return openRouterMessagePayload{}, false, errors.New("OpenRouter tool result payload is invalid")
	}
	contentJSON, err := json.Marshal(map[string]any{
		"call_id": result.ProviderCallID, "name": result.Name,
		"provider_name": providerNameValue, "success": result.Success, "payload": payload,
	})
	if err != nil {
		return openRouterMessagePayload{}, false, errors.New("OpenRouter tool result is invalid")
	}
	content := string(contentJSON)
	return openRouterMessagePayload{
		Role: "tool", Content: &content, ToolCallID: result.ProviderCallID,
	}, true, nil
}

func lowerOpenRouterReasoning(message GenerationMessage) ([]json.RawMessage, error) {
	if len(message.ReasoningDetails) != 0 {
		details := make([]json.RawMessage, 0, len(message.ReasoningDetails))
		for _, detail := range message.ReasoningDetails {
			value, err := decodeOpenRouterJSON(detail)
			if err != nil {
				return nil, errors.New("OpenRouter replay reasoning is invalid")
			}
			encoded, err := json.Marshal(value)
			if err != nil {
				return nil, errors.New("OpenRouter replay reasoning is invalid")
			}
			details = append(details, encoded)
		}
		return details, nil
	}
	if strings.TrimSpace(message.EncryptedReasoning) == "" {
		return nil, nil
	}
	detail := map[string]any{
		"type": "reasoning.encrypted", "data": message.EncryptedReasoning,
	}
	if strings.TrimSpace(message.ReasoningID) != "" {
		detail["id"] = message.ReasoningID
	}
	encoded, _ := json.Marshal(detail)
	return []json.RawMessage{encoded}, nil
}
