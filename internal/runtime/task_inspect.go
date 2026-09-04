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
	modelToolPayloadLimit = 32 << 10
	repeatedToolLimit     = 4
	failedToolLimit       = 6
	providerRoundLimit    = 80
)

var taskInspectSchema = json.RawMessage(`{
  "type":"object",
  "properties":{"task_id":{"type":"string","minLength":1,"maxLength":255}},
  "required":["task_id"],
  "additionalProperties":false
}`)

func taskInspectTool() provider.GenerationTool {
	return provider.GenerationTool{
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
	return boundedModelToolPayload(payload, modelToolPayloadLimit)
}

func boundedModelToolPayload(payload json.RawMessage, limit int) json.RawMessage {
	if len(payload) <= limit {
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
		if len(bounded) <= limit || len(preview) == 0 {
			return bounded
		}
		next := len(preview) * limit / len(bounded)
		if next >= len(preview) {
			next = len(preview) - 1
		}
		preview = preview[:next]
	}
}

func providerMessagesFromItems(items []store.ConversationItem) ([]provider.GenerationMessage, error) {
	results := make(map[string]store.ConversationItem)
	for _, item := range items {
		if item.Kind == store.ConversationToolResult && item.ParentItemID != "" {
			results[item.ParentItemID] = item
		}
	}
	messages := make([]provider.GenerationMessage, 0, len(items))
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
			messages = append(messages, provider.GenerationMessage{Role: "user", Content: item.ContentText})
			rounds = append(rounds, providerRound(item))
		case store.ConversationAssistantText:
			content := item.ContentText
			if item.ProviderContentText != "" {
				content = item.ProviderContentText
			}
			messages = append(messages, provider.GenerationMessage{
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
				messages = append(messages, provider.GenerationMessage{
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
				messages = append(messages, provider.GenerationMessage{Role: "tool", ToolResult: &result})
				rounds = append(rounds, round)
			} else {
				payload := toolFailure(
					"tool_execution_interrupted",
					"Tool execution ended before Noema recorded a result. Its outcome is unknown, and the action was not retried.",
				)
				result := provider.ReplayToolResult{
					ProviderCallID: call.ProviderCallID, ProviderName: call.ProviderName,
					Name: call.Name, Arguments: call.Arguments, Success: false, Payload: payload,
				}
				messages = append(messages, provider.GenerationMessage{Role: "tool", ToolResult: &result})
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

func storedToolCall(item store.ConversationItem) (provider.ReplayToolCall, error) {
	action, ok := nestedAction(item.Payload)
	if !ok {
		return provider.ReplayToolCall{}, errors.New("stored tool call is invalid")
	}
	arguments, err := json.Marshal(action["payload"])
	if err != nil {
		return provider.ReplayToolCall{}, errors.New("stored tool call is invalid")
	}
	call := provider.ReplayToolCall{
		ProviderItemID: textValue(action["provider_item_id"]),
		ProviderCallID: textValue(action["provider_call_id"]),
		ProviderName:   textValue(action["provider_name"]), Name: textValue(action["name"]),
		Arguments: arguments,
	}
	if strings.TrimSpace(call.ProviderCallID) == "" || strings.TrimSpace(call.Name) == "" {
		return provider.ReplayToolCall{}, errors.New("stored tool call is invalid")
	}
	return call, nil
}

func storedToolResult(item store.ConversationItem) (provider.ReplayToolResult, error) {
	action, ok := nestedAction(item.Payload)
	if !ok {
		return provider.ReplayToolResult{}, errors.New("stored tool result is invalid")
	}
	payload, err := json.Marshal(action["payload"])
	if err != nil {
		return provider.ReplayToolResult{}, errors.New("stored tool result is invalid")
	}
	result := provider.ReplayToolResult{
		ProviderCallID: textValue(action["provider_call_id"]),
		ProviderName:   textValue(action["provider_name"]), Name: textValue(action["name"]),
		Success: action["success"] == true, Payload: modelToolPayload(payload),
	}
	if strings.TrimSpace(result.ProviderCallID) == "" || strings.TrimSpace(result.Name) == "" {
		return provider.ReplayToolResult{}, errors.New("stored tool result is invalid")
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
	generator provider.Generator,
	initial provider.GenerationResult,
) {
	result := initial
	usage := provider.Usage{}
	progress := taskInspectProgress{
		argumentCounts: make(map[string]int), results: make(map[string]struct{}),
	}
	for providerRound := 0; ; providerRound++ {
		if err := addProviderUsage(&usage, result.Usage); err != nil {
			c.failTurn(request.input, turn, err)
			return
		}
		if len(result.ToolCalls) == 0 {
			c.finishGeneratedTurn(request.input, turn, assignment, result, providerRound, usage)
			return
		}
		if len(result.ToolCalls) != 1 || result.ToolCalls[0].Name != taskInspectName {
			c.failTurn(request.input, turn, errors.New("provider returned an unsupported tool sequence"))
			return
		}
		call := result.ToolCalls[0]
		toolPayload, success, err := c.persistTaskInspectRound(
			request, turn, assignment, result, call, providerRound,
		)
		if err != nil {
			c.failTurn(request.input, turn, err)
			return
		}
		stopReason := progress.observe(call, toolPayload, success)
		if providerRound >= providerRoundLimit {
			stopReason = "maximum provider tool continuations reached"
		}
		nextRound := providerRound + 1
		var forcedFinalization bool
		result, forcedFinalization, err = c.generateTaskInspectContinuation(
			request, turn, assignment, generator, nextRound, stopReason,
		)
		if err != nil {
			c.failTurn(request.input, turn, err)
			return
		}
		if stopReason != "" || forcedFinalization {
			if err := addProviderUsage(&usage, result.Usage); err != nil {
				c.failTurn(request.input, turn, err)
				return
			}
			if len(result.ToolCalls) != 0 {
				c.failTurn(request.input, turn, errors.New("provider returned a tool call during finalization"))
				return
			}
			c.finishGeneratedTurn(request.input, turn, assignment, result, nextRound, usage)
			return
		}
	}
}

func (c *Chat) persistTaskInspectRound(
	request queuedTurn,
	turn store.ConversationTurn,
	assignment store.ModelAssignment,
	generation provider.GenerationResult,
	call provider.GenerationToolCall,
	providerRound int,
) (json.RawMessage, bool, error) {
	items, err := c.database.StartConversationToolRound(c.ctx, turn, store.ConversationToolRound{
		Provider:   assignment.ProviderKind,
		Commentary: generation.Text, Reasoning: generationReasoning(generation),
		Call: store.ConversationToolCallInput{
			ProviderRound: providerRound, OutputIndex: call.Index, ProviderItemID: call.ProviderItemID,
			ProviderCallID: call.ProviderCallID,
			ProviderName:   call.ProviderName, Name: call.Name, Arguments: call.Payload,
		},
	}, time.Now())
	if err != nil {
		return nil, false, err
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
		return nil, false, errors.New("stored tool call is unavailable")
	}
	toolPayload, success := c.inspectTask(c.ctx, call.Payload)
	resultItem, err := c.database.FinishConversationToolCall(c.ctx, turn, store.ConversationToolResultInput{
		CallItemID: callItem.ID, Provider: assignment.ProviderKind,
		ProviderRound: providerRound, OutputIndex: call.Index,
		ProviderCallID: call.ProviderCallID, ProviderName: call.ProviderName,
		Name: call.Name, Success: success, Payload: toolPayload,
	}, time.Now())
	if err != nil {
		return nil, false, err
	}
	c.publish(Event{
		Kind: EventConversationItem, ConversationID: turn.ConversationID,
		ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: &resultItem,
	})
	return toolPayload, success, nil
}

func (c *Chat) generateTaskInspectContinuation(
	request queuedTurn,
	turn store.ConversationTurn,
	assignment store.ModelAssignment,
	generator provider.Generator,
	providerRound int,
	stopReason string,
) (provider.GenerationResult, bool, error) {
	stored, err := c.database.ConversationProviderItems(c.ctx, turn.ConversationID)
	if err != nil {
		return provider.GenerationResult{}, false, err
	}
	messages, err := providerMessagesFromItems(stored)
	if err != nil {
		return provider.GenerationResult{}, false, err
	}
	environment := runtimeEnvironment(request.conversation, request.location, time.Now())
	tools := []provider.GenerationTool{taskInspectTool()}
	transport := provider.ToolTransportNative
	if stopReason != "" {
		environment += "\n\n" + taskInspectFinalizationInstruction(stopReason)
		messages = compactTaskInspectFinalizationMessages(messages, modelToolPayloadLimit)
		tools = nil
		transport = provider.ToolTransportNone
	}
	messages = append([]provider.GenerationMessage{{
		Role: "developer", Content: environment,
	}}, messages...)
	streamID := store.ConversationAssistantStreamID(turn.ID, providerRound)
	generate := func() (provider.GenerationResult, error) {
		return generator.Generate(c.ctx, provider.GenerateRequest{
			AccountID: assignment.ProviderAccountID, Model: assignment.ModelProfile,
			Messages: messages, ReasoningEffort: string(assignment.ReasoningEffort),
			ConversationID: turn.ConversationID, MaxOutputTokens: taskInspectOutputTokens(stopReason != ""),
			Tools: tools, ToolTransport: transport, ToolChoice: provider.ToolChoiceAuto,
			FastMode: assignment.FastMode,
		}, func(event provider.StreamEvent) {
			if event.Kind == provider.TextDelta {
				c.publish(Event{
					Kind: EventAssistantDelta, ConversationID: turn.ConversationID,
					TurnID: turn.ID, StreamID: streamID, ResponseIndex: 0, Delta: event.Delta,
				})
			}
		})
	}
	result, err := generate()
	if err == nil {
		return result, stopReason != "", err
	}
	requestTooLarge := errors.Is(err, provider.ErrGenerationRequestTooLarge)
	requestRejected := errors.Is(err, provider.ErrProviderRequestRejected)
	if !requestTooLarge && !requestRejected {
		return result, stopReason != "", err
	}
	payloadLimit := modelToolPayloadLimit
	if requestRejected || stopReason != "" {
		payloadLimit = 1 << 10
	}
	if stopReason == "" {
		stopReason = "provider replay limit reached"
		environment = runtimeEnvironment(request.conversation, request.location, time.Now()) +
			"\n\n" + taskInspectFinalizationInstruction(stopReason)
	}
	messages = compactTaskInspectFinalizationMessages(messages[1:], payloadLimit)
	messages = append([]provider.GenerationMessage{{Role: "developer", Content: environment}}, messages...)
	tools = nil
	transport = provider.ToolTransportNone
	result, err = generate()
	return result, true, err
}

func compactTaskInspectFinalizationMessages(
	messages []provider.GenerationMessage,
	payloadLimit int,
) []provider.GenerationMessage {
	lastUser := -1
	for index := range messages {
		if messages[index].Role == "user" {
			lastUser = index
		}
	}
	if lastUser < 0 {
		return nil
	}
	user := messages[lastUser]
	pairs := make([][2]provider.GenerationMessage, 0, providerRoundLimit+1)
	var assistant *provider.GenerationMessage
	for index := lastUser + 1; index < len(messages); index++ {
		message := messages[index]
		switch {
		case message.Role == "assistant" && len(message.ToolCalls) != 0:
			message.ReasoningDetails = nil
			message.Content = boundedUTF8(message.Content, 2<<10)
			assistant = &message
		case message.Role == "tool" && assistant != nil:
			if message.ToolResult != nil {
				toolResult := *message.ToolResult
				toolResult.Payload = boundedModelToolPayload(toolResult.Payload, payloadLimit)
				message.ToolResult = &toolResult
			}
			pairs = append(pairs, [2]provider.GenerationMessage{*assistant, message})
			assistant = nil
		}
	}
	result := make([]provider.GenerationMessage, 0, 1+2*len(pairs))
	result = append(result, user)
	for _, pair := range pairs {
		result = append(result, pair[0], pair[1])
	}
	return result
}

func taskInspectOutputTokens(finalization bool) *uint32 {
	if !finalization {
		return maxOutputTokens()
	}
	value := uint32(1024)
	return &value
}

func boundedUTF8(value string, limit int) string {
	if len(value) <= limit {
		return value
	}
	value = value[:limit]
	for !utf8.ValidString(value) {
		value = value[:len(value)-1]
	}
	return value
}

type taskInspectProgress struct {
	argumentCounts map[string]int
	results        map[string]struct{}
	repeated       int
	failures       int
}

func (p *taskInspectProgress) observe(
	call provider.GenerationToolCall,
	payload json.RawMessage,
	success bool,
) string {
	argumentKey := call.Name + "\x00" + string(call.Payload)
	p.argumentCounts[argumentKey]++
	resultKey := argumentKey + "\x00" + string(payload)
	if success {
		resultKey += "\x00success"
		p.failures = 0
	} else {
		resultKey += "\x00failure"
		p.failures++
	}
	_, seenResult := p.results[resultKey]
	p.results[resultKey] = struct{}{}
	if p.argumentCounts[argumentKey] > 1 && seenResult {
		p.repeated++
	}
	if p.failures >= failedToolLimit {
		return "consecutive tool failures"
	}
	if p.repeated >= repeatedToolLimit {
		return "repeated tool arguments and results"
	}
	return ""
}

func taskInspectFinalizationInstruction(reason string) string {
	return "The tool loop must stop because: " + reason +
		". Give one concise final answer from the saved results. Do not call tools."
}

func addProviderUsage(total *provider.Usage, next provider.Usage) error {
	counts := [][2]*int{
		{&total.InputTokens, &next.InputTokens},
		{&total.CachedInputTokens, &next.CachedInputTokens},
		{&total.OutputTokens, &next.OutputTokens},
		{&total.TotalTokens, &next.TotalTokens},
		{&total.WebSearchRequests, &next.WebSearchRequests},
	}
	maximum := int(^uint(0) >> 1)
	for _, count := range counts {
		if *count[1] < 0 || *count[0] > maximum-*count[1] {
			return errors.New("provider usage total is too large")
		}
	}
	for _, count := range counts {
		*count[0] += *count[1]
	}
	return nil
}

func (c *Chat) finishGeneratedTurn(
	input SendTurnInput,
	turn store.ConversationTurn,
	assignment store.ModelAssignment,
	result provider.GenerationResult,
	providerRound int,
	usage provider.Usage,
) {
	model := result.Model
	if model == "" {
		model = assignment.ModelProfile
	}
	item, err := c.database.CompleteConversationTurnOutput(
		c.ctx, turn, result.Text, result.Text,
		&store.ProviderUsage{
			Provider: assignment.ProviderKind, Model: model, InputTokens: usage.InputTokens,
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

func generationReasoning(result provider.GenerationResult) []json.RawMessage {
	var details []json.RawMessage
	for _, item := range result.Reasoning {
		for _, detail := range item.ProviderDetails {
			details = append(details, append(json.RawMessage(nil), detail...))
		}
	}
	return details
}
