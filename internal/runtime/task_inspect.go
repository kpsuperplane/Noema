package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/home"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	taskInspectName       = "task.inspect"
	modelToolResultLimit  = 64 << 10
	modelToolPayloadLimit = 32 << 10
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

func providerMessagesFromItems(
	items []store.ConversationItem,
	activeTurnID string,
	activeProvider string,
) ([]provider.GenerationMessage, error) {
	results := make(map[string]store.ConversationItem)
	for _, item := range items {
		if item.Kind == store.ConversationToolResult && item.ParentItemID != "" {
			results[item.ParentItemID] = item
		}
	}
	messages := make([]provider.GenerationMessage, 0, len(items))
	rounds := make([]int, 0, len(items))
	var reasoning []json.RawMessage
	hosted := make(map[int][]provider.GenerationMessage)
	var hostedOrder []int
	flushHosted := func(round int) {
		pending := hosted[round]
		if len(pending) == 0 {
			return
		}
		if len(reasoning) != 0 {
			messages = append(messages, provider.GenerationMessage{
				Role: "assistant", ReasoningDetails: reasoning,
			})
			rounds = append(rounds, round)
			reasoning = nil
		}
		messages = append(messages, pending...)
		for range pending {
			rounds = append(rounds, round)
		}
		delete(hosted, round)
	}
	for _, item := range items {
		switch item.Kind {
		case store.ConversationReasoning:
			details, err := storedReasoning(item)
			if err != nil {
				return nil, err
			}
			if item.TurnID != activeTurnID || textValue(item.Metadata["provider"]) != activeProvider {
				details = withoutHostedReasoning(details)
			}
			reasoning = append(reasoning, details...)
		case store.ConversationUserText:
			messages = append(messages, provider.GenerationMessage{Role: "user", Content: item.ContentText})
			rounds = append(rounds, providerRound(item))
		case store.ConversationAssistantText:
			round := providerRound(item)
			flushHosted(round)
			content := item.ContentText
			if item.ProviderContentText != "" {
				content = item.ProviderContentText
			}
			messages = append(messages, provider.GenerationMessage{
				Role: "assistant", Content: content, ReasoningDetails: reasoning,
			})
			rounds = append(rounds, round)
			reasoning = nil
		case store.ConversationToolCall:
			if stored, exists := results[item.ID]; exists && storedHostedSearch(item) {
				if item.TurnID != activeTurnID || textValue(item.Metadata["provider"]) != activeProvider {
					continue
				}
				search, err := replayHostedSearch(item, stored)
				if err != nil {
					return nil, err
				}
				round := providerRound(item)
				if len(hosted[round]) == 0 {
					hostedOrder = append(hostedOrder, round)
				}
				hosted[round] = append(hosted[round], provider.GenerationMessage{
					Role: "hosted_web_search", HostedSearch: &search,
				})
				continue
			}
			call, err := storedToolCall(item)
			if err != nil {
				return nil, err
			}
			round := providerRound(item)
			flushHosted(round)
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
	for _, round := range hostedOrder {
		flushHosted(round)
	}
	return messages, nil
}

func withoutHostedReasoning(details []json.RawMessage) []json.RawMessage {
	filtered := details[:0]
	for _, detail := range details {
		var value map[string]any
		if json.Unmarshal(detail, &value) == nil && value["type"] == "reasoning.server_tool_call" {
			continue
		}
		filtered = append(filtered, detail)
	}
	return filtered
}

func storedHostedSearch(item store.ConversationItem) bool {
	action, ok := nestedAction(item.Payload)
	hosted, _ := action["hosted_web_search"].(bool)
	return ok && hosted
}

func replayHostedSearch(
	callItem store.ConversationItem,
	resultItem store.ConversationItem,
) (provider.HostedSearch, error) {
	call, ok := nestedAction(callItem.Payload)
	if !ok {
		return provider.HostedSearch{}, errors.New("stored hosted web search is invalid")
	}
	result, ok := nestedAction(resultItem.Payload)
	if !ok {
		return provider.HostedSearch{}, errors.New("stored hosted web result is invalid")
	}
	arguments, err := json.Marshal(call["payload"])
	if err != nil {
		return provider.HostedSearch{}, errors.New("stored hosted web arguments are invalid")
	}
	payload, err := json.Marshal(result["payload"])
	if err != nil {
		return provider.HostedSearch{}, errors.New("stored hosted web result is invalid")
	}
	var action json.RawMessage
	if call["provider_action"] != nil {
		action, err = json.Marshal(call["provider_action"])
		if err != nil {
			return provider.HostedSearch{}, errors.New("stored hosted web provider action is invalid")
		}
	}
	sources, err := replayHostedSources(call["sources"])
	if err != nil {
		return provider.HostedSearch{}, err
	}
	return provider.HostedSearch{
		Index: providerOutputIndex(callItem), ID: textValue(call["provider_item_id"]),
		Name: textValue(call["name"]), Status: textValue(call["status"]),
		Arguments: arguments, Result: payload, Sources: sources, ProviderAction: action,
	}, nil
}

func replayHostedSources(value any) ([]provider.WebSource, error) {
	values, ok := value.([]any)
	if value == nil {
		return nil, nil
	}
	if !ok {
		return nil, errors.New("stored hosted web sources are invalid")
	}
	sources := make([]provider.WebSource, 0, len(values))
	for _, value := range values {
		source, ok := value.(map[string]any)
		if !ok || textValue(source["url"]) == "" {
			return nil, errors.New("stored hosted web source is invalid")
		}
		sources = append(sources, provider.WebSource{
			Title: textValue(source["title"]), URL: textValue(source["url"]),
		})
	}
	return sources, nil
}

func providerOutputIndex(item store.ConversationItem) int {
	value, _ := item.Metadata["output_index"].(float64)
	return int(value)
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

func (c *Chat) executeChatToolRounds(
	request queuedTurn,
	turn store.ConversationTurn,
	assignment store.ModelAssignment,
	generator provider.Generator,
	initial provider.GenerationResult,
	memoryContext string,
	initialProviderRound int,
	initialHostedState bool,
) {
	result := initial
	usage := provider.Usage{}
	progress := newToolProgress(request.input.Input)
	hostedState := initialHostedState
	for providerRound := initialProviderRound; ; providerRound++ {
		if err := addProviderUsage(&usage, result.Usage); err != nil {
			c.failTurn(request.input, turn, err)
			return
		}
		if err := c.persistHostedSearches(
			request, turn, assignment, result.Searches, providerRound,
		); err != nil {
			c.failTurn(request.input, turn, err)
			return
		}
		hostedState = hostedState || len(result.Searches) != 0
		if len(result.ToolCalls) == 0 {
			c.finishGeneratedTurn(request.input, turn, assignment, result, providerRound, usage)
			return
		}
		if len(result.ToolCalls) != 1 || !c.supportsChatTool(c.ctx, result.ToolCalls[0].Name) {
			c.failTurn(request.input, turn, errors.New("provider returned an unsupported tool sequence"))
			return
		}
		call := result.ToolCalls[0]
		toolPayload, success, pending, err := c.persistChatToolRound(
			request, turn, assignment, result, call, providerRound, hostedState,
		)
		if err != nil {
			c.failTurn(request.input, turn, err)
			return
		}
		if pending {
			return
		}
		sideEffect := call.Name == updateOwnNameToolName || call.Name == fileDownloadName
		if strings.HasPrefix(call.Name, "mcp.") && c.mcp != nil {
			if binding, err := c.mcp.Binding(c.ctx, call.Name); err == nil {
				sideEffect = !binding.Behavior.ReadOnly
			}
		}
		stopReason := progress.observe(call, toolPayload, success, sideEffect)
		if providerRound >= providerRoundLimit {
			stopReason = "maximum provider tool continuations reached"
		}
		nextRound := providerRound + 1
		incremental := provider.GenerationMessage{Role: "tool", ToolResult: &provider.ReplayToolResult{
			ProviderCallID: call.ProviderCallID, ProviderName: call.ProviderName,
			Name: call.Name, Arguments: call.Payload, Success: success, Payload: toolPayload,
		}}
		if stopReason == "" && nextRound%progressAuditInterval == 0 {
			if err := c.saveProgressAuditActivity(
				request, turn, nextRound, "Checking progress", "Checking progress", "running",
				map[string]any{"status": "running"},
			); err != nil {
				c.failTurn(request.input, turn, err)
				return
			}
			outcome, auditErr := runProgressAudit(c.ctx, c.database, c.generatorFor, progress.digest(nextRound))
			if auditErr == nil {
				title := map[string]string{
					"continue": "Still making progress", "finalize": "Ready to wrap up",
					"ask_human": "Needs your input", "pause": "Paused",
				}[outcome.Decision]
				if err := c.saveProgressAuditActivity(
					request, turn, nextRound, title, outcome.UserSummary, "completed",
					map[string]any{"status": "completed", "decision": outcome.Decision},
				); err != nil {
					c.failTurn(request.input, turn, err)
					return
				}
				progress.apply(outcome)
				switch outcome.Decision {
				case "finalize":
					stopReason = "progress audit requested finalization"
				case "ask_human", "pause":
					c.finishProgressAuditPause(request.input, turn, outcome.UserSummary, nextRound)
					return
				}
			} else {
				if err := c.saveProgressAuditActivity(
					request, turn, nextRound, "Progress check unavailable",
					"The progress check is unavailable.", "completed",
					map[string]any{"status": "completed"},
				); err != nil {
					c.failTurn(request.input, turn, err)
					return
				}
				progress.window = progressAuditStats{ToolCounts: make(map[string]int)}
				if !errors.Is(auditErr, errProgressAuditUnavailable) {
					stopReason = "progress audit failed"
				}
			}
		}
		var forcedFinalization bool
		result, forcedFinalization, err = c.generateChatToolContinuation(
			request, turn, assignment, generator, nextRound, stopReason, memoryContext,
			result.ID, hostedState, incremental, nil,
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

func (c *Chat) persistChatToolRound(
	request queuedTurn,
	turn store.ConversationTurn,
	assignment store.ModelAssignment,
	generation provider.GenerationResult,
	call provider.GenerationToolCall,
	providerRound int,
	hostedState bool,
) (json.RawMessage, bool, bool, error) {
	var mcpBinding *noemamcp.Binding
	var adapterBinding *adapter.Binding
	var multipleChoice *multipleChoiceArguments
	var choiceCredentialRevision uint64
	var choiceToolCatalogDigest string
	if call.Name == fileDownloadName {
		if _, err := parseFileDownloadArguments(call.Payload); err != nil {
			return nil, false, false, errors.New("file.download arguments are invalid")
		}
	} else if call.Name == presentMultipleChoiceName {
		multipleChoice, _ = parseMultipleChoiceArguments(call.Payload)
		if multipleChoice != nil {
			var authorityErr error
			choiceCredentialRevision, choiceToolCatalogDigest, authorityErr = c.multipleChoiceAuthority(c.ctx, assignment)
			if authorityErr != nil {
				return nil, false, false, authorityErr
			}
		}
	} else if call.Name != noemamcp.ConnectServiceToolName && call.Name != adapter.DefinitionTemplateTool && call.Name != adapter.ProposeDefinitionTool && !supportsLocalChatTool(call.Name) {
		if c.adapters != nil {
			if binding, bindErr := c.adapters.Binding(call.Name); bindErr == nil {
				if c.adapters.Validate(binding, call.Payload) != nil {
					return nil, false, false, errors.New("adapter tool arguments are invalid")
				}
				adapterBinding = &binding
			}
		}
		if adapterBinding != nil {
		} else {
			if c.mcp == nil {
				return nil, false, false, errors.New("MCP tool is unavailable")
			}
			binding, err := c.mcp.Binding(c.ctx, call.Name)
			if err != nil || noemamcp.ValidateArguments(binding.InputSchema, call.Payload) != nil {
				return nil, false, false, errors.New("MCP tool arguments or authority are invalid")
			}
			mcpBinding = &binding
		}
	}
	normalized := normalizeProviderText(generation.Text, generation.Citations)
	items, err := c.database.StartConversationToolRound(c.ctx, turn, store.ConversationToolRound{
		Provider:   assignment.ProviderKind,
		Commentary: normalized.Text, ProviderCommentary: generation.Text,
		Citations:                 generationCitations(normalized.Citations),
		UnresolvedCitationMarkers: normalized.UnresolvedMarkers,
		Reasoning:                 generationReasoning(generation),
		Call: store.ConversationToolCallInput{
			ProviderRound: providerRound, OutputIndex: call.Index, ProviderItemID: call.ProviderItemID,
			ProviderCallID: call.ProviderCallID,
			ProviderName:   call.ProviderName, Name: call.Name, Arguments: call.Payload,
		},
		MultipleChoice: storedMultipleChoiceInput(
			multipleChoice, assignment, generation.ID, hostedState,
			choiceCredentialRevision, choiceToolCatalogDigest,
		),
	}, time.Now())
	if err != nil {
		return nil, false, false, err
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
		return nil, false, false, errors.New("stored tool call is unavailable")
	}
	if multipleChoice != nil {
		c.publish(Event{Kind: EventAgentStatus, ConversationID: turn.ConversationID, Status: AgentStatusIdle})
		c.publish(Event{Kind: EventTurnCompleted, ConversationID: turn.ConversationID,
			ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID})
		return nil, false, true, nil
	}
	if call.Name == fileDownloadName {
		payload, success, approval, err := c.prepareFileDownloadAction(
			request.conversation, turn, callItem, assignment, providerRound, call.Payload,
		)
		if err != nil {
			return nil, false, false, err
		}
		if approval != nil {
			c.publish(Event{Kind: EventConversationItem, ConversationID: turn.ConversationID,
				ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: approval})
			c.publish(Event{Kind: EventHumanInterventionsChanged, ConversationID: turn.ConversationID})
			c.publish(Event{Kind: EventAgentStatus, ConversationID: turn.ConversationID, Status: AgentStatusIdle})
			c.publish(Event{Kind: EventTurnCompleted, ConversationID: turn.ConversationID,
				ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID})
			return nil, false, true, nil
		}
		resultItem, err := c.database.FinishConversationToolCall(c.ctx, turn, store.ConversationToolResultInput{
			CallItemID: callItem.ID, Provider: assignment.ProviderKind,
			ProviderRound: providerRound, OutputIndex: call.Index,
			ProviderCallID: call.ProviderCallID, ProviderName: call.ProviderName,
			Name: call.Name, Success: success, Payload: payload,
		}, time.Now())
		if err != nil {
			return nil, false, false, err
		}
		c.publish(Event{Kind: EventConversationItem, ConversationID: turn.ConversationID,
			ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: &resultItem})
		return payload, success, false, nil
	}
	if mcpBinding != nil {
		payload, success, approval, err := c.prepareMCPAction(
			request.conversation, turn, callItem, assignment, providerRound, generation.ID, hostedState,
			*mcpBinding, call.Payload,
		)
		if err != nil {
			return nil, false, false, err
		}
		if approval != nil {
			c.publish(Event{Kind: EventConversationItem, ConversationID: turn.ConversationID,
				ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: approval})
			c.publish(Event{Kind: EventHumanInterventionsChanged, ConversationID: turn.ConversationID})
			c.publish(Event{Kind: EventAgentStatus, ConversationID: turn.ConversationID, Status: AgentStatusIdle})
			c.publish(Event{Kind: EventTurnCompleted, ConversationID: turn.ConversationID,
				ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID})
			return nil, false, true, nil
		}
		resultItem, err := c.database.FinishConversationToolCall(c.ctx, turn, store.ConversationToolResultInput{
			CallItemID: callItem.ID, Provider: assignment.ProviderKind, ProviderRound: providerRound,
			OutputIndex: call.Index, ProviderCallID: call.ProviderCallID, ProviderName: call.ProviderName,
			Name: call.Name, Success: success, Payload: payload,
		}, time.Now())
		if err != nil {
			return nil, false, false, err
		}
		c.publish(Event{Kind: EventConversationItem, ConversationID: turn.ConversationID,
			ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: &resultItem})
		return payload, success, false, nil
	}
	if adapterBinding != nil {
		payload, success, approval, err := c.prepareAdapterAction(request.conversation, turn, callItem, assignment, providerRound, generation.ID, hostedState, *adapterBinding, call.Payload)
		if err != nil {
			return nil, false, false, err
		}
		if approval != nil {
			c.publish(Event{Kind: EventConversationItem, ConversationID: turn.ConversationID, ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: approval})
			c.publish(Event{Kind: EventHumanInterventionsChanged, ConversationID: turn.ConversationID})
			c.publish(Event{Kind: EventAgentStatus, ConversationID: turn.ConversationID, Status: AgentStatusIdle})
			c.publish(Event{Kind: EventTurnCompleted, ConversationID: turn.ConversationID, ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID})
			return nil, false, true, nil
		}
		resultItem, err := c.database.FinishConversationToolCall(c.ctx, turn, store.ConversationToolResultInput{CallItemID: callItem.ID, Provider: assignment.ProviderKind, ProviderRound: providerRound, OutputIndex: call.Index, ProviderCallID: call.ProviderCallID, ProviderName: call.ProviderName, Name: call.Name, Success: success, Payload: payload}, time.Now())
		if err != nil {
			return nil, false, false, err
		}
		c.publish(Event{Kind: EventConversationItem, ConversationID: turn.ConversationID, ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: &resultItem})
		if adapterOutcomeUncertain(payload) {
			return payload, false, true, c.failUncertainTurn(request.input, turn)
		}
		return payload, success, false, nil
	}
	toolPayload, success := c.executeChatTool(c.ctx, request.conversation, call.Name, call.Payload)
	resultItem, err := c.database.FinishConversationToolCall(c.ctx, turn, store.ConversationToolResultInput{
		CallItemID: callItem.ID, Provider: assignment.ProviderKind,
		ProviderRound: providerRound, OutputIndex: call.Index,
		ProviderCallID: call.ProviderCallID, ProviderName: call.ProviderName,
		Name: call.Name, Success: success, Payload: toolPayload,
	}, time.Now())
	if err != nil {
		return nil, false, false, err
	}
	c.publish(Event{
		Kind: EventConversationItem, ConversationID: turn.ConversationID,
		ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: &resultItem,
	})
	if success && call.Name == adapter.ProposeDefinitionTool {
		c.publish(Event{Kind: EventHumanInterventionsChanged, ConversationID: turn.ConversationID})
	}
	c.publishMemoryChanged()
	return toolPayload, success, false, nil
}

func (c *Chat) persistHostedSearches(
	request queuedTurn,
	turn store.ConversationTurn,
	assignment store.ModelAssignment,
	searches []provider.HostedSearch,
	providerRound int,
) error {
	if len(searches) == 0 {
		return nil
	}
	values := make([]store.ConversationHostedSearch, 0, len(searches))
	for _, search := range searches {
		sources := make([]store.ConversationWebSource, 0, len(search.Sources))
		for _, source := range search.Sources {
			sources = append(sources, store.ConversationWebSource{Title: source.Title, URL: source.URL})
		}
		values = append(values, store.ConversationHostedSearch{
			OutputIndex: search.Index, ID: search.ID, Name: search.Name, Status: search.Status,
			Arguments: search.Arguments, Result: search.Result,
			Sources: sources, ProviderAction: search.ProviderAction,
		})
	}
	items, err := c.database.StoreConversationHostedSearches(
		c.ctx, turn, assignment.ProviderKind, providerRound, values, time.Now(),
	)
	if err != nil {
		return err
	}
	for index := range items {
		c.publish(Event{
			Kind: EventConversationItem, ConversationID: turn.ConversationID,
			ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: &items[index],
		})
	}
	return nil
}

func (c *Chat) generateChatToolContinuation(
	request queuedTurn,
	turn store.ConversationTurn,
	assignment store.ModelAssignment,
	generator provider.Generator,
	providerRound int,
	stopReason string,
	memoryContext string,
	previousResponseID string,
	hostedState bool,
	incremental provider.GenerationMessage,
	expectedCredentialRevision *uint64,
) (provider.GenerationResult, bool, error) {
	stored, err := c.database.ConversationProviderItems(c.ctx, turn.ConversationID)
	if err != nil {
		return provider.GenerationResult{}, false, err
	}
	messages, err := providerMessagesFromItems(stored, turn.ID, assignment.ProviderKind)
	if err != nil {
		return provider.GenerationResult{}, false, err
	}
	environment, err := c.modelEnvironment(c.ctx, request.conversation, request.location, time.Now())
	if err != nil {
		return provider.GenerationResult{}, false, err
	}
	tools, err := c.chatTools(c.ctx)
	if err != nil {
		return provider.GenerationResult{}, false, err
	}
	transport := provider.ToolTransportNative
	if stopReason != "" {
		environment += "\n\n" + toolFinalizationInstruction(stopReason)
		messages = compactToolFinalizationMessages(messages, modelToolPayloadLimit)
		tools = nil
		transport = provider.ToolTransportNone
	}
	hostedWeb := hostedWebSearchEnabled(assignment.ProviderKind, transport)
	developer := developerMessages(environment, memoryContext, hostedWeb)
	messages = append(developer, messages...)
	replayMessages := messages
	responseContinuation := responseIDContinuationProvider(assignment.ProviderKind)
	continuingResponse := responseContinuation && previousResponseID != ""
	if responseContinuation && previousResponseID == "" && hostedState {
		return provider.GenerationResult{}, false,
			fmt.Errorf("%s provider-hosted web state is unavailable", assignment.ProviderKind)
	}
	if continuingResponse {
		messages = []provider.GenerationMessage{incremental}
		if stopReason != "" {
			messages = append(messages, provider.GenerationMessage{
				Role: "developer", Content: toolFinalizationInstruction(stopReason),
			})
		}
	}
	streamID := store.ConversationAssistantStreamID(turn.ID, providerRound)
	generate := func() (provider.GenerationResult, error) {
		return generator.Generate(c.ctx, provider.GenerateRequest{
			AccountID: assignment.ProviderAccountID, Model: assignment.ModelProfile,
			Messages: messages, ReasoningEffort: string(assignment.ReasoningEffort),
			ConversationID: turn.ConversationID, MaxOutputTokens: toolOutputTokens(stopReason != ""),
			Tools: tools, ToolTransport: transport, ToolChoice: provider.ToolChoiceAuto,
			HostedWebSearch:            hostedWeb,
			PreviousResponseID:         previousResponseID,
			StoreResponse:              responseContinuation,
			ExpectedCredentialRevision: expectedCredentialRevision,
			FastMode:                   assignment.FastMode,
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
	if continuingResponse {
		if hostedState || (!requestTooLarge && !requestRejected) {
			return result, stopReason != "", err
		}
		messages = replayMessages
		previousResponseID = ""
		result, err = generate()
		return result, stopReason != "", err
	}
	if !requestTooLarge && !requestRejected {
		return result, stopReason != "", err
	}
	payloadLimit := modelToolPayloadLimit
	if requestRejected || stopReason != "" {
		payloadLimit = 1 << 10
	}
	if stopReason == "" {
		stopReason = "provider replay limit reached"
		environment += "\n\n" + toolFinalizationInstruction(stopReason)
	}
	messages = compactToolFinalizationMessages(messages[len(developer):], payloadLimit)
	messages = append(developerMessages(environment, memoryContext, false), messages...)
	tools = nil
	transport = provider.ToolTransportNone
	hostedWeb = false
	result, err = generate()
	return result, true, err
}

func compactToolFinalizationMessages(
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
	hosted := make([]provider.GenerationMessage, 0)
	var assistant *provider.GenerationMessage
	for index := lastUser + 1; index < len(messages); index++ {
		message := messages[index]
		switch {
		case message.Role == "hosted_web_search":
			hosted = append(hosted, message)
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
	result := make([]provider.GenerationMessage, 0, 1+len(hosted)+2*len(pairs))
	result = append(result, user)
	result = append(result, hosted...)
	for _, pair := range pairs {
		result = append(result, pair[0], pair[1])
	}
	return result
}

func toolOutputTokens(finalization bool) *uint32 {
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

func toolFinalizationInstruction(reason string) string {
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
	normalized := normalizeProviderText(result.Text, result.Citations)
	model := result.Model
	if model == "" {
		model = assignment.ModelProfile
	}
	item, err := c.database.CompleteConversationTurnOutput(
		c.ctx, turn, normalized.Text, result.Text,
		&store.ProviderUsage{
			Provider: assignment.ProviderKind, Model: model, InputTokens: usage.InputTokens,
			OutputTokens: usage.OutputTokens, TotalTokens: usage.TotalTokens,
			CachedInputTokens: usage.CachedInputTokens, WebSearchRequests: usage.WebSearchRequests,
		}, generationReasoning(result), generationCitations(normalized.Citations),
		normalized.UnresolvedMarkers, providerRound, time.Now(),
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
	c.publishMemoryChanged()
	c.publish(Event{
		Kind: EventAgentStatus, ConversationID: turn.ConversationID, Status: AgentStatusIdle,
	})
	c.publish(Event{
		Kind: EventTurnCompleted, ConversationID: turn.ConversationID,
		ClientMessageID: input.ClientMessageID, TurnID: turn.ID,
	})
	c.maybeScheduleMemoryUpdate(turn.ConversationID)
}

func (c *Chat) finishProgressAuditPause(
	input SendTurnInput,
	turn store.ConversationTurn,
	text string,
	providerRound int,
) {
	item, err := c.database.CompleteConversationProgressAuditPause(
		c.ctx, turn, text, providerRound, time.Now(),
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
	c.publishMemoryChanged()
	c.publish(Event{Kind: EventAgentStatus, ConversationID: turn.ConversationID, Status: AgentStatusIdle})
	c.publish(Event{
		Kind: EventTurnCompleted, ConversationID: turn.ConversationID,
		ClientMessageID: input.ClientMessageID, TurnID: turn.ID,
	})
	c.maybeScheduleMemoryUpdate(turn.ConversationID)
}

func generationCitations(values []provider.Citation) []store.ProviderCitation {
	citations := make([]store.ProviderCitation, 0, len(values))
	for _, citation := range values {
		citations = append(citations, store.ProviderCitation{
			Title: citation.Title, URL: citation.URL,
			StartIndex: citation.StartIndex, EndIndex: citation.EndIndex,
		})
	}
	return citations
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
