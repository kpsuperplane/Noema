package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"reflect"
	"strings"
	"time"

	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func (c *Chat) prepareMCPAction(
	conversation store.Conversation, turn store.ConversationTurn, callItem store.ConversationItem,
	assignment store.ModelAssignment, providerRound int, responseID string, hostedState bool,
	binding noemamcp.Binding, arguments json.RawMessage,
) (json.RawMessage, bool, *store.ConversationItem, error) {
	if binding.ReviewRoute == "" {
		payload, success, err := c.mcp.Call(c.ctx, binding, arguments)
		if errors.Is(err, noemamcp.ErrAuthenticationRequired) {
			token, _ := json.Marshal(map[string]any{"binding": binding, "assignment": modelAssignmentValue(assignment),
				"response_id": responseID, "hosted_state": hostedState})
			_, notice, saveErr := c.database.CreateMCPAuthRequest(c.ctx, store.MCPAuthRequest{
				OwnerHumanID: "human:local", ConversationID: conversation.ID, TurnID: turn.ID,
				CallItemID: callItem.ID, ServerID: binding.ServerID, CapabilityName: binding.Name,
				BindingJSON: string(token), ArgumentsJSON: string(arguments), ProviderRound: providerRound,
			}, time.Now())
			if saveErr != nil {
				return nil, false, nil, saveErr
			}
			return nil, false, &notice, nil
		}
		if err != nil {
			return toolFailure("mcp_call_failed", "MCP tool call failed"), false, nil, nil
		}
		return payload, success, nil, nil
	}
	authority, err := c.database.ConversationAuthorizationContext(c.ctx, conversation.ID, turn.ID)
	if err != nil {
		return nil, false, nil, err
	}
	server, err := c.mcp.Server(c.ctx, binding.ServerID)
	if err != nil {
		return nil, false, nil, err
	}
	contextValue := map[string]any{
		"origin": "primary_conversation", "context": authority, "conversation_id": conversation.ID,
		"execution_decision": string(binding.ReviewRoute), "provider_selection": modelAssignmentValue(assignment),
		"provider_round": providerRound,
		"destination": map[string]any{"service_id": server.DefinitionID, "connection_id": server.ID,
			"revision": server.ConnectionRevision},
		"service":              map[string]any{"display_name": server.DisplayName, "connection_label": server.ConnectionLabel},
		"mcp_binding":          binding,
		"provider_response_id": responseID, "hosted_state": hostedState,
	}
	action, err := c.database.CreateActionRequest(c.ctx, store.NewActionRequest{
		ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: callItem.ID,
		OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: binding.Name,
		OperationToken: binding.Name, ReviewRoute: binding.ReviewRoute, Behavior: binding.Behavior,
		Arguments: arguments, InputSchema: binding.InputSchema, AuthorizationContext: contextValue,
		SafeSummary: "Use " + strings.TrimPrefix(binding.Name, "mcp."+server.ID+".") + " on " + server.DisplayName,
	}, time.Now())
	if err != nil {
		return nil, false, nil, err
	}
	if action.ReviewRoute == store.ActionLLMReview {
		action, approval, err := c.database.RecordActionAssessment(c.ctx, action.ID, action.Revision,
			c.reviewActionRequest(action), time.Now())
		if err != nil {
			return nil, false, nil, err
		}
		if approval != nil {
			return nil, false, approval, nil
		}
		return c.executeReviewedMCP(action)
	}
	approval, err := c.database.VisibleConversationItem(c.ctx, action.ApprovalItemID)
	if err != nil || approval == nil {
		return nil, false, nil, errors.New("MCP approval request is unavailable")
	}
	return nil, false, approval, nil
}

func (c *Chat) executeReviewedMCP(action store.ActionRequest) (json.RawMessage, bool, *store.ConversationItem, error) {
	if c.mcp == nil {
		return nil, false, nil, errors.New("MCP service is unavailable")
	}
	var binding noemamcp.Binding
	if json.Unmarshal(mustJSON(action.AuthorizationContext["mcp_binding"]), &binding) != nil ||
		action.OperationToken != binding.Name || action.CapabilityName != binding.Name ||
		action.Behavior != binding.Behavior || string(mustJSON(action.InputSchema)) != string(binding.InputSchema) {
		action, err := c.database.SupersedeActionRequest(c.ctx, action.ID, action.Revision, "action_authority_changed", time.Now())
		if err != nil {
			return nil, false, nil, err
		}
		payload, _ := json.Marshal(actionResultPayload(action))
		return payload, false, nil, nil
	}
	claimed, err := c.database.ClaimActionRequest(c.ctx, action.ID, action.Revision, time.Now())
	if err != nil {
		return nil, false, nil, err
	}
	payload, success, callErr := c.mcp.Call(c.ctx, binding, mustJSON(claimed.Arguments))
	if errors.Is(callErr, noemamcp.ErrAuthenticationRequired) {
		assignment, assignmentErr := storedModelAssignment(claimed.AuthorizationContext)
		if assignmentErr != nil {
			return nil, false, nil, assignmentErr
		}
		token, _ := json.Marshal(map[string]any{"binding": binding, "assignment": modelAssignmentValue(assignment),
			"response_id":  textValue(claimed.AuthorizationContext["provider_response_id"]),
			"hosted_state": claimed.AuthorizationContext["hosted_state"] == true})
		_, notice, saveErr := c.database.CreateMCPAuthRequest(c.ctx, store.MCPAuthRequest{
			OwnerHumanID: "human:local", ConversationID: claimed.ConversationID, TurnID: claimed.TurnID,
			CallItemID: claimed.CallItemID, ServerID: binding.ServerID, CapabilityName: binding.Name,
			ActionID: claimed.ID, BindingJSON: string(token), ArgumentsJSON: string(mustJSON(claimed.Arguments)),
			ProviderRound: int(numberField(claimed.AuthorizationContext, "provider_round")),
		}, time.Now())
		return nil, false, &notice, saveErr
	}
	state, failure := store.ActionSucceeded, ""
	if callErr != nil {
		state, failure = store.ActionFailed, "mcp_call_failed"
		payload = toolFailure("mcp_call_failed", "MCP tool call failed")
	}
	if !success && callErr == nil {
		state, failure = store.ActionFailed, "remote_tool_failed"
	}
	_, err = c.database.FinishActionRequest(c.ctx, claimed.ID, claimed.Revision, state, payload, failure, time.Now())
	if err != nil {
		return nil, false, nil, err
	}
	return payload, state == store.ActionSucceeded, nil, nil
}

// ResolveActionRequest applies one human decision and resumes its Chat.
func (c *Chat) ResolveActionRequest(
	ctx context.Context, actionID string, revision int, humanID, decision string,
) (store.ActionRequest, error) {
	if actionID == "" || revision != 1 || humanID != "human:local" {
		return store.ActionRequest{}, errors.New("action decision is invalid")
	}
	request := actionResolution{ctx: ctx, actionID: actionID, revision: revision,
		humanID: humanID, decision: decision, reply: make(chan actionResolutionResult, 1)}
	select {
	case <-ctx.Done():
		return store.ActionRequest{}, ctx.Err()
	case <-c.ctx.Done():
		return store.ActionRequest{}, ErrChatClosed
	case c.actions <- request:
	}
	select {
	case <-ctx.Done():
		return store.ActionRequest{}, ctx.Err()
	case <-c.ctx.Done():
		return store.ActionRequest{}, ErrChatClosed
	case result := <-request.reply:
		return result.action, result.err
	}
}

func (c *Chat) prepareFileDownloadAction(
	conversation store.Conversation,
	turn store.ConversationTurn,
	callItem store.ConversationItem,
	assignment store.ModelAssignment,
	providerRound int,
	arguments json.RawMessage,
) (json.RawMessage, bool, *store.ConversationItem, error) {
	if _, err := parseFileDownloadArguments(arguments); err != nil {
		payload := toolFailure("invalid_input", err.Error())
		return payload, false, nil, nil
	}
	authority, err := c.database.ConversationAuthorizationContext(c.ctx, conversation.ID, turn.ID)
	if err != nil {
		return nil, false, nil, err
	}
	messages, _ := authority["messages"].([]map[string]any)
	var sourceID any
	if len(messages) != 0 {
		sourceID = messages[len(messages)-1]["item_id"]
	}
	contextValue := map[string]any{
		"origin": "primary_conversation", "context": authority,
		"conversation_id": conversation.ID, "source_human_item_id": sourceID,
		"cwd": conversation.CWD, "execution_decision": "llm_review",
		"provider_selection": modelAssignmentValue(assignment), "provider_round": providerRound,
		"destination": map[string]any{
			"service_id": "public_web", "connection_id": "file_download", "revision": "1",
		},
		"service": map[string]any{"display_name": "Public web"},
	}
	action, err := c.database.CreateActionRequest(c.ctx, store.NewActionRequest{
		ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: callItem.ID,
		OwnerHumanID: "human:local", RequestingAgentID: "agent:primary",
		CapabilityName: fileDownloadName, OperationToken: fileDownloadName,
		ReviewRoute: store.ActionLLMReview,
		Behavior:    store.ActionBehavior{ReadOnly: false, RepeatSafe: false, Destructive: false, OpenWorld: true},
		Arguments:   arguments, InputSchema: fileDownloadSchema,
		AuthorizationContext: contextValue, SafeSummary: "Download a public file into the working directory",
	}, time.Now())
	if err != nil {
		return nil, false, nil, err
	}
	action, approval, err := c.database.RecordActionAssessment(
		c.ctx, action.ID, action.Revision, c.reviewActionRequest(action), time.Now(),
	)
	if err != nil || approval != nil {
		return nil, false, approval, err
	}
	return c.executeReviewedDownload(action, conversation)
}

func (c *Chat) executeReviewedDownload(
	action store.ActionRequest, conversation store.Conversation,
) (json.RawMessage, bool, *store.ConversationItem, error) {
	if !currentDownloadAction(action, conversation) {
		action, err := c.database.SupersedeActionRequest(
			c.ctx, action.ID, action.Revision, "action_authority_changed", time.Now(),
		)
		if err != nil {
			return nil, false, nil, err
		}
		payload, _ := json.Marshal(actionResultPayload(action))
		return payload, false, nil, nil
	}
	claimed, err := c.database.ClaimActionRequest(c.ctx, action.ID, action.Revision, time.Now())
	if err != nil {
		return nil, false, nil, err
	}
	payload, executeErr := executeFileDownload(c.ctx, conversation.CWD, mustJSON(claimed.Arguments))
	state, failure := store.ActionSucceeded, ""
	if executeErr != nil {
		state, failure = store.ActionFailed, "download_failed"
		payload, _ = json.Marshal(map[string]string{"error": executeErr.Error()})
		if errors.Is(executeErr, errDownloadOutcomeUncertain) {
			state, failure = store.ActionOutcomeUncertain, "outcome_uncertain"
		}
	}
	_, err = c.database.FinishActionRequest(
		c.ctx, claimed.ID, claimed.Revision, state, payload, failure, time.Now(),
	)
	if err != nil {
		return nil, false, nil, err
	}
	return payload, state == store.ActionSucceeded, nil, nil
}

func (c *Chat) resolveActionRequest(request actionResolution) (store.ActionRequest, error) {
	if err := request.ctx.Err(); err != nil {
		return store.ActionRequest{}, err
	}
	action, err := c.database.DecideActionRequest(
		c.ctx, request.actionID, request.revision, request.humanID, request.decision, time.Now(),
	)
	if err != nil {
		return store.ActionRequest{}, err
	}
	c.publish(Event{Kind: EventHumanInterventionsChanged, ConversationID: action.ConversationID})
	if action.State == store.ActionExecutable {
		if action.AuthorizationContext["adapter_binding"] != nil {
			_, _, _, err = c.executeReviewedAdapter(action)
		} else if strings.HasPrefix(action.CapabilityName, "mcp.") {
			var notice *store.ConversationItem
			_, _, notice, err = c.executeReviewedMCP(action)
			if err == nil && notice != nil {
				c.publish(Event{Kind: EventConversationItem, ConversationID: action.ConversationID, TurnID: action.TurnID, Item: notice})
				c.publish(Event{Kind: EventHumanInterventionsChanged, ConversationID: action.ConversationID})
				c.publish(Event{Kind: EventAgentStatus, ConversationID: action.ConversationID, Status: AgentStatusIdle})
				c.publish(Event{Kind: EventTurnCompleted, ConversationID: action.ConversationID, TurnID: action.TurnID})
				return c.database.ActionRequest(c.ctx, action.ID, action.Revision)
			}
		} else {
			conversation, conversationErr := c.database.Conversation(c.ctx, action.ConversationID)
			if conversationErr != nil {
				return store.ActionRequest{}, conversationErr
			}
			_, _, _, err = c.executeReviewedDownload(action, conversation)
		}
		if err != nil {
			return store.ActionRequest{}, err
		}
		action, err = c.database.ActionRequest(c.ctx, action.ID, action.Revision)
		if err != nil {
			return store.ActionRequest{}, err
		}
	}
	resultItem, err := c.appendActionResult(action)
	if err != nil {
		return action, err
	}
	if action.State == store.ActionOutcomeUncertain {
		turn, _, turnErr := c.database.ActionConversationCall(c.ctx, action)
		if turnErr != nil {
			return action, turnErr
		}
		return action, c.failUncertainTurn(SendTurnInput{ConversationID: action.ConversationID}, turn)
	}
	c.continueAfterAction(action, resultItem)
	return action, nil
}

func numberField(value map[string]any, key string) float64 {
	number, _ := value[key].(float64)
	return number
}

func (c *Chat) appendActionResult(action store.ActionRequest) (store.ConversationItem, error) {
	turn, input, err := c.database.ActionConversationCall(c.ctx, action)
	if err != nil {
		return store.ConversationItem{}, err
	}
	input.Success = action.State == store.ActionSucceeded
	input.Payload = mustJSON(actionResultPayload(action))
	item, err := c.database.FinishConversationToolCall(c.ctx, turn, input, time.Now())
	if err != nil {
		return store.ConversationItem{}, err
	}
	c.publish(Event{Kind: EventConversationItem, ConversationID: action.ConversationID,
		TurnID: action.TurnID, Item: &item})
	return item, nil
}

func (c *Chat) continueAfterAction(action store.ActionRequest, trigger store.ConversationItem) {
	conversation, err := c.database.Conversation(c.ctx, action.ConversationID)
	if err != nil {
		return
	}
	turn, err := c.database.BeginConversationContinuation(c.ctx, action.ConversationID, trigger.ID, time.Now())
	if err != nil {
		return
	}
	assignment, err := storedModelAssignment(action.AuthorizationContext)
	if err != nil {
		c.failTurn(SendTurnInput{ConversationID: action.ConversationID}, turn, err)
		return
	}
	generator, err := c.generatorFor(assignment.ProviderKind)
	if err != nil {
		c.failTurn(SendTurnInput{ConversationID: action.ConversationID}, turn, err)
		return
	}
	request := queuedTurn{input: SendTurnInput{ConversationID: action.ConversationID},
		conversation: conversation, location: time.UTC}
	c.publish(Event{Kind: EventAgentStatus, ConversationID: action.ConversationID, Status: AgentStatusThinking})
	hostedState := action.AuthorizationContext["hosted_state"] == true
	responseID := textValue(action.AuthorizationContext["provider_response_id"])
	incremental := provider.GenerationMessage{}
	if responseID != "" {
		replay, replayErr := storedToolResult(trigger)
		if replayErr != nil {
			c.failTurn(request.input, turn, replayErr)
			return
		}
		replay.Arguments = mustJSON(action.Arguments)
		incremental = provider.GenerationMessage{Role: "tool", ToolResult: &replay}
	}
	result, _, err := c.generateChatToolContinuation(
		request, turn, assignment, generator, 0, "", c.memoryRootContext(),
		responseID, hostedState, incremental,
	)
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	if len(result.ToolCalls) == 0 {
		c.finishGeneratedTurn(request.input, turn, assignment, result, 0, result.Usage)
		return
	}
	if len(result.ToolCalls) != 1 || !c.supportsChatTool(c.ctx, result.ToolCalls[0].Name) {
		c.failTurn(request.input, turn, errors.New("provider returned an unsupported tool sequence"))
		return
	}
	c.executeChatToolRounds(request, turn, assignment, generator, result, c.memoryRootContext())
}

func (c *Chat) actionTurnHasHostedState(action store.ActionRequest) (bool, error) {
	items, err := c.database.ConversationProviderItems(c.ctx, action.ConversationID)
	if err != nil {
		return false, err
	}
	for _, item := range items {
		if item.TurnID == action.TurnID && storedHostedSearch(item) {
			return true, nil
		}
	}
	return false, nil
}

func currentDownloadAction(action store.ActionRequest, conversation store.Conversation) bool {
	var schema map[string]any
	_ = json.Unmarshal(fileDownloadSchema, &schema)
	return action.CapabilityName == fileDownloadName && action.OperationToken == fileDownloadName &&
		action.ReviewRoute == store.ActionLLMReview &&
		action.Behavior == (store.ActionBehavior{ReadOnly: false, RepeatSafe: false, Destructive: false, OpenWorld: true}) &&
		reflect.DeepEqual(action.InputSchema, schema) && action.AuthorizationContext["cwd"] == conversation.CWD &&
		func() bool { _, err := parseFileDownloadArguments(mustJSON(action.Arguments)); return err == nil }()
}

func actionResultPayload(action store.ActionRequest) map[string]any {
	var output any
	if action.Output != nil {
		output = action.Output
	}
	var failure any
	if action.FailureCode != "" {
		failure = action.FailureCode
	}
	return map[string]any{
		"status": action.State, "action_id": action.ID,
		"failure_code": failure, "result": output,
	}
}

func storedModelAssignment(contextValue map[string]any) (store.ModelAssignment, error) {
	value, ok := contextValue["provider_selection"].(map[string]any)
	if !ok {
		return store.ModelAssignment{}, errors.New("saved provider selection is unavailable")
	}
	assignment := store.ModelAssignment{
		Role:              store.HostedModelRole(textValue(value["role"])),
		ProviderKind:      textValue(value["provider_kind"]),
		ProviderAccountID: textValue(value["provider_account_id"]),
		SelectionMode:     store.ModelSelectionMode(textValue(value["selection_mode"])),
		ModelProfile:      textValue(value["model_profile"]),
		ReasoningEffort:   store.ModelReasoningEffort(textValue(value["reasoning_effort"])),
	}
	assignment.FastMode, _ = value["fast_mode"].(bool)
	if assignment.Role != store.HostedModelNoema || assignment.ProviderKind == "" ||
		assignment.ProviderAccountID == "" || assignment.ModelProfile == "" {
		return store.ModelAssignment{}, errors.New("saved provider selection is invalid")
	}
	return assignment, nil
}

func mustJSON(value any) json.RawMessage {
	encoded, _ := json.Marshal(value)
	return encoded
}
