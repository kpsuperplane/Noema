package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	taskInspectName       = "task.inspect"
	modelToolResultLimit  = 64 << 10
	modelToolPayloadLimit = modelToolResultLimit - (2 << 10)
)

var taskInspectSchema = json.RawMessage(`{
  "type":"object",
  "properties":{"task_id":{"type":"string","minLength":1,"maxLength":255}},
  "required":["task_id"],
  "additionalProperties":false
}`)

func taskInspectTool() provider.OpenRouterTool {
	return provider.OpenRouterTool{
		Name:        taskInspectName,
		Description: "Read one exact owner-authorized Task and its current documents.",
		InputSchema: append(json.RawMessage(nil), taskInspectSchema...),
	}
}

func (c *Chat) inspectTask(ctx context.Context, arguments json.RawMessage) (json.RawMessage, bool) {
	var fields map[string]json.RawMessage
	if err := decodeToolArguments(arguments, &fields); err != nil || len(fields) != 1 {
		return toolFailure("invalid_input", "task.inspect arguments are invalid"), false
	}
	rawID, exists := fields["task_id"]
	if !exists {
		return toolFailure("invalid_input", "task.inspect requires task_id"), false
	}
	var taskID string
	if json.Unmarshal(rawID, &taskID) != nil || len(taskID) < 1 || len(taskID) > 255 {
		return toolFailure("invalid_input", "task.inspect task_id is invalid"), false
	}
	task, err := c.database.Task(ctx, taskID)
	if err != nil {
		return toolFailure("not_found", "Task is unavailable"), false
	}
	document, err := home.ReadTaskDocument(c.home, taskID)
	if err != nil {
		return toolFailure("not_found", "Task document is unavailable"), false
	}
	payload, _ := json.Marshal(map[string]any{
		"task_id": task.ID, "title": task.Title,
		"task_document": document.Content, "task_document_digest": document.Digest,
		"result_document": nil, "review_document": nil,
		"stage_id": store.TaskStageID(task.State), "generation": 1, "revision": task.Revision,
		"project_id": nil, "scheduled_for": nil, "schedule_time_zone": nil,
		"recurrence_id": nil, "recurrence_revision": nil, "recurrence_scheduled_for": nil,
	})
	return payload, true
}

func decodeToolArguments(raw json.RawMessage, target any) error {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	if err := decoder.Decode(target); err != nil {
		return err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return errors.New("tool arguments have trailing JSON")
		}
		return err
	}
	return nil
}

func toolFailure(code string, message string) json.RawMessage {
	payload, _ := json.Marshal(map[string]any{"code": code, "message": message})
	return payload
}

func modelToolPayload(payload json.RawMessage) json.RawMessage {
	if len(payload) <= modelToolPayloadLimit {
		return append(json.RawMessage(nil), payload...)
	}
	preview := payload
	if len(preview) > 48<<10 {
		preview = preview[:48<<10]
	}
	for {
		for len(preview) > 0 && !utf8.Valid(preview) {
			preview = preview[:len(preview)-1]
		}
		bounded, _ := json.Marshal(map[string]any{
			"truncated": true,
			"message":   "Tool result exceeded the model-facing limit.",
			"preview":   string(preview),
		})
		if len(bounded) <= modelToolPayloadLimit || len(preview) == 0 {
			return bounded
		}
		next := len(preview) * modelToolPayloadLimit / len(bounded)
		if next >= len(preview) {
			next = len(preview) - 1
		}
		preview = preview[:next]
	}
}

func providerMessagesFromItems(items []store.ConversationItem) ([]provider.OpenRouterChatMessage, error) {
	results := make(map[string]store.ConversationItem)
	for _, item := range items {
		if item.Kind == store.ConversationToolResult && item.ParentItemID != "" {
			results[item.ParentItemID] = item
		}
	}
	messages := make([]provider.OpenRouterChatMessage, 0, len(items))
	rounds := make([]int, 0, len(items))
	var reasoning []json.RawMessage
	for _, item := range items {
		switch item.Kind {
		case store.ConversationReasoning:
			details, err := storedReasoning(item)
			if err != nil {
				return nil, err
			}
			reasoning = append(reasoning, details...)
		case store.ConversationUserText:
			messages = append(messages, provider.OpenRouterChatMessage{Role: "user", Content: item.ContentText})
			rounds = append(rounds, providerRound(item))
		case store.ConversationAssistantText:
			content := item.ContentText
			if item.ProviderContentText != "" {
				content = item.ProviderContentText
			}
			messages = append(messages, provider.OpenRouterChatMessage{
				Role: "assistant", Content: content, ReasoningDetails: reasoning,
			})
			rounds = append(rounds, providerRound(item))
			reasoning = nil
		case store.ConversationToolCall:
			call, err := storedToolCall(item)
			if err != nil {
				return nil, err
			}
			round := providerRound(item)
			last := len(messages) - 1
			if last < 0 || messages[last].Role != "assistant" || rounds[last] != round {
				messages = append(messages, provider.OpenRouterChatMessage{
					Role: "assistant", ReasoningDetails: reasoning,
				})
				rounds = append(rounds, round)
				reasoning = nil
				last++
			}
			messages[last].ToolCalls = append(messages[last].ToolCalls, call)
			if stored, exists := results[item.ID]; exists {
				result, err := storedToolResult(stored)
				if err != nil {
					return nil, err
				}
				messages = append(messages, provider.OpenRouterChatMessage{Role: "tool", ToolResult: &result})
				rounds = append(rounds, round)
			} else {
				payload := toolFailure(
					"tool_execution_interrupted",
					"Tool execution ended before Noema recorded a result. Its outcome is unknown, and the action was not retried.",
				)
				result := provider.OpenRouterReplayToolResult{
					ProviderCallID: call.ProviderCallID, ProviderName: call.ProviderName,
					Name: call.Name, Arguments: call.Arguments, Success: false, Payload: payload,
				}
				messages = append(messages, provider.OpenRouterChatMessage{Role: "tool", ToolResult: &result})
				rounds = append(rounds, round)
			}
		}
	}
	return messages, nil
}

func storedReasoning(item store.ConversationItem) ([]json.RawMessage, error) {
	values, ok := item.Payload["provider_details"].([]any)
	if !ok {
		return nil, errors.New("stored provider reasoning is invalid")
	}
	result := make([]json.RawMessage, 0, len(values))
	for _, value := range values {
		encoded, err := json.Marshal(value)
		if err != nil {
			return nil, errors.New("stored provider reasoning is invalid")
		}
		result = append(result, encoded)
	}
	return result, nil
}

func storedToolCall(item store.ConversationItem) (provider.OpenRouterReplayToolCall, error) {
	action, ok := nestedAction(item.Payload)
	if !ok {
		return provider.OpenRouterReplayToolCall{}, errors.New("stored tool call is invalid")
	}
	arguments, err := json.Marshal(action["payload"])
	if err != nil {
		return provider.OpenRouterReplayToolCall{}, errors.New("stored tool call is invalid")
	}
	call := provider.OpenRouterReplayToolCall{
		ProviderCallID: textValue(action["provider_call_id"]),
		ProviderName:   textValue(action["provider_name"]), Name: textValue(action["name"]),
		Arguments: arguments,
	}
	if strings.TrimSpace(call.ProviderCallID) == "" || strings.TrimSpace(call.Name) == "" {
		return provider.OpenRouterReplayToolCall{}, errors.New("stored tool call is invalid")
	}
	return call, nil
}

func storedToolResult(item store.ConversationItem) (provider.OpenRouterReplayToolResult, error) {
	action, ok := nestedAction(item.Payload)
	if !ok {
		return provider.OpenRouterReplayToolResult{}, errors.New("stored tool result is invalid")
	}
	payload, err := json.Marshal(action["payload"])
	if err != nil {
		return provider.OpenRouterReplayToolResult{}, errors.New("stored tool result is invalid")
	}
	result := provider.OpenRouterReplayToolResult{
		ProviderCallID: textValue(action["provider_call_id"]),
		ProviderName:   textValue(action["provider_name"]), Name: textValue(action["name"]),
		Success: action["success"] == true, Payload: modelToolPayload(payload),
	}
	if strings.TrimSpace(result.ProviderCallID) == "" || strings.TrimSpace(result.Name) == "" {
		return provider.OpenRouterReplayToolResult{}, errors.New("stored tool result is invalid")
	}
	return result, nil
}

func nestedAction(payload map[string]any) (map[string]any, bool) {
	metadata, ok := payload["metadata"].(map[string]any)
	if !ok {
		return nil, false
	}
	action, ok := metadata["action"].(map[string]any)
	return action, ok
}

func providerRound(item store.ConversationItem) int {
	value, _ := item.Metadata["provider_round"].(float64)
	return int(value)
}

func textValue(value any) string {
	text, _ := value.(string)
	return text
}

func (c *Chat) executeTaskInspectRound(
	request queuedTurn,
	turn store.ConversationTurn,
	assignment store.ModelAssignment,
	initial provider.OpenRouterGenerationResult,
) {
	call := initial.ToolCalls[0]
	items, err := c.database.StartConversationToolRound(c.ctx, turn, store.ConversationToolRound{
		Commentary: initial.Text, Reasoning: generationReasoning(initial),
		Call: store.ConversationToolCallInput{
			ProviderRound: 0, OutputIndex: call.Index, ProviderCallID: call.ProviderCallID,
			ProviderName: call.ProviderName, Name: call.Name, Arguments: call.Payload,
		},
	}, time.Now())
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	var callItem store.ConversationItem
	for index := range items {
		if items[index].Kind == store.ConversationReasoning {
			continue
		}
		if items[index].Kind == store.ConversationToolCall {
			callItem = items[index]
		}
		c.publish(Event{
			Kind: EventConversationItem, ConversationID: turn.ConversationID,
			ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: &items[index],
		})
	}
	if callItem.ID == "" {
		c.failTurn(request.input, turn, errors.New("stored tool call is unavailable"))
		return
	}
	toolPayload, success := c.inspectTask(c.ctx, call.Payload)
	resultItem, err := c.database.FinishConversationToolCall(c.ctx, turn, store.ConversationToolResultInput{
		CallItemID: callItem.ID, ProviderRound: 0, OutputIndex: call.Index,
		ProviderCallID: call.ProviderCallID, ProviderName: call.ProviderName,
		Name: call.Name, Success: success, Payload: toolPayload,
	}, time.Now())
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	c.publish(Event{
		Kind: EventConversationItem, ConversationID: turn.ConversationID,
		ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: &resultItem,
	})
	stored, err := c.database.ConversationProviderItems(c.ctx, turn.ConversationID)
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	messages, err := providerMessagesFromItems(stored)
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	messages = append([]provider.OpenRouterChatMessage{{
		Role: "developer", Content: runtimeEnvironment(request.conversation, request.location, time.Now()),
	}}, messages...)
	streamID := "assistant_stream:" + turn.ID + ":continuation:response:0"
	continuation, err := c.generator.Generate(c.ctx, provider.OpenRouterGenerateRequest{
		AccountID: assignment.ProviderAccountID, Model: assignment.ModelProfile,
		Messages: messages, ReasoningEffort: string(assignment.ReasoningEffort),
		ConversationID: turn.ConversationID, MaxOutputTokens: maxOutputTokens(),
	}, func(event provider.StreamEvent) {
		if event.Kind == provider.TextDelta {
			c.publish(Event{
				Kind: EventAssistantDelta, ConversationID: turn.ConversationID,
				TurnID: turn.ID, StreamID: streamID, ResponseIndex: 0, Delta: event.Delta,
			})
		}
	})
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	c.finishGeneratedTurn(request.input, turn, assignment, continuation, 1, &initial.Usage)
}

func (c *Chat) finishGeneratedTurn(
	input SendTurnInput,
	turn store.ConversationTurn,
	assignment store.ModelAssignment,
	result provider.OpenRouterGenerationResult,
	providerRound int,
	prior *provider.Usage,
) {
	model := result.Model
	if model == "" {
		model = assignment.ModelProfile
	}
	usage := result.Usage
	if prior != nil {
		usage.InputTokens += prior.InputTokens
		usage.CachedInputTokens += prior.CachedInputTokens
		usage.OutputTokens += prior.OutputTokens
		usage.TotalTokens += prior.TotalTokens
	}
	item, err := c.database.CompleteConversationTurnOutput(
		c.ctx, turn, result.Text, result.Text,
		&store.ProviderUsage{
			Provider: "openrouter", Model: model, InputTokens: usage.InputTokens,
			OutputTokens: usage.OutputTokens, TotalTokens: usage.TotalTokens,
			CachedInputTokens: usage.CachedInputTokens,
		}, generationReasoning(result), providerRound, time.Now(),
	)
	if err != nil {
		if c.ctx.Err() != nil {
			c.cancelTurn(input, turn)
		} else {
			c.failTurn(input, turn, err)
		}
		return
	}
	c.publish(Event{
		Kind: EventConversationItem, ConversationID: turn.ConversationID,
		ClientMessageID: input.ClientMessageID, TurnID: turn.ID, Item: &item,
	})
	c.publish(Event{
		Kind: EventAgentStatus, ConversationID: turn.ConversationID, Status: AgentStatusIdle,
	})
	c.publish(Event{
		Kind: EventTurnCompleted, ConversationID: turn.ConversationID,
		ClientMessageID: input.ClientMessageID, TurnID: turn.ID,
	})
}

func generationReasoning(result provider.OpenRouterGenerationResult) []json.RawMessage {
	var details []json.RawMessage
	for _, item := range result.Reasoning {
		for _, detail := range item.ProviderDetails {
			details = append(details, append(json.RawMessage(nil), detail...))
		}
	}
	return details
}
