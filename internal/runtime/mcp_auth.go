package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"time"

	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

// ResumeMCPAuthentication resumes every exact call bound to one completed attempt.
func (c *Chat) ResumeMCPAuthentication(ctx context.Context, attemptID string) error {
	_, err := c.sendMCPAuth(ctx, mcpAuthResolution{attemptID: attemptID})
	return err
}

// SkipMCPAuthentication closes one exact call without credentials.
func (c *Chat) SkipMCPAuthentication(ctx context.Context, requestID string, revision int) (store.MCPAuthRequest, error) {
	return c.sendMCPAuth(ctx, mcpAuthResolution{requestID: requestID, revision: revision, skip: true})
}

func (c *Chat) sendMCPAuth(ctx context.Context, request mcpAuthResolution) (store.MCPAuthRequest, error) {
	request.reply = make(chan mcpAuthResult, 1)
	select {
	case c.mcpAuth <- request:
	case <-ctx.Done():
		return store.MCPAuthRequest{}, ctx.Err()
	case <-c.ctx.Done():
		return store.MCPAuthRequest{}, ErrChatClosed
	}
	select {
	case result := <-request.reply:
		return result.request, result.err
	case <-ctx.Done():
		return store.MCPAuthRequest{}, ctx.Err()
	case <-c.ctx.Done():
		return store.MCPAuthRequest{}, ErrChatClosed
	}
}

func (c *Chat) resolveMCPAuthentication(input mcpAuthResolution) (store.MCPAuthRequest, error) {
	if c.mcp == nil {
		return store.MCPAuthRequest{}, errors.New("MCP service is unavailable")
	}
	if input.skip {
		request, err := c.database.MCPAuthRequest(c.ctx, input.requestID, input.revision)
		if err != nil || request.OwnerHumanID != "human:local" {
			return request, errors.New("MCP authentication request is unavailable")
		}
		payload := toolFailure("authentication_skipped", "MCP authentication was skipped")
		if request.ActionID != "" {
			request, action, finishErr := c.database.FinishMCPAuthAction(c.ctx, request, store.ActionFailed, payload,
				"authentication_skipped", "cancelled", time.Now())
			if finishErr != nil {
				return request, finishErr
			}
			item, finishErr := c.appendActionResult(action)
			if finishErr == nil {
				c.continueAfterAction(action, item)
			}
			return request, finishErr
		}
		request, err = c.database.FinishMCPAuthRequest(c.ctx, request.ID, request.Revision, "cancelled", "authentication_skipped", time.Now())
		if err != nil {
			return request, err
		}
		return request, c.finishMCPAuthCall(request, payload, false)
	}
	requests, err := c.database.MCPAuthRequestsForAttempt(c.ctx, input.attemptID)
	if err != nil {
		return store.MCPAuthRequest{}, err
	}
	for _, request := range requests {
		if request.ActionID != "" {
			request, err = c.database.BeginMCPAuthResume(c.ctx, request, time.Now())
			if err != nil {
				return request, err
			}
		}
		binding, assignment, responseID, hostedState, decodeErr := decodeMCPAuthAuthority(request.BindingJSON)
		payload, success := toolFailure("mcp_authentication_failed", "MCP authentication failed"), false
		if decodeErr == nil {
			payload, success, decodeErr = c.mcp.Call(c.ctx, binding, json.RawMessage(request.ArgumentsJSON))
		}
		state, failure := "completed", ""
		if decodeErr != nil {
			state, failure = "cancelled", "authentication_failed"
		}
		if request.ActionID != "" {
			actionState, actionFailure := store.ActionSucceeded, ""
			if decodeErr != nil || !success {
				actionState, actionFailure = store.ActionFailed, "mcp_call_failed"
			}
			request, action, finishErr := c.database.FinishMCPAuthAction(c.ctx, request, actionState, payload,
				actionFailure, state, time.Now())
			if finishErr != nil {
				return request, finishErr
			}
			item, finishErr := c.appendActionResult(action)
			if finishErr != nil {
				return request, finishErr
			}
			c.continueAfterAction(action, item)
			continue
		}
		request, err = c.database.FinishMCPAuthRequest(c.ctx, request.ID, request.Revision, state, failure, time.Now())
		if err != nil {
			return request, err
		}
		if err := c.finishMCPAuthCallWithAssignment(request, payload, success, assignment, responseID, hostedState); err != nil {
			return request, err
		}
	}
	return store.MCPAuthRequest{}, nil
}

func decodeMCPAuthAuthority(raw string) (noemamcp.Binding, store.ModelAssignment, string, bool, error) {
	var value struct {
		Binding    noemamcp.Binding `json:"binding"`
		Assignment map[string]any   `json:"assignment"`
		ResponseID string           `json:"response_id"`
		Hosted     bool             `json:"hosted_state"`
	}
	if json.Unmarshal([]byte(raw), &value) != nil {
		return value.Binding, store.ModelAssignment{}, "", false, errors.New("MCP authentication authority is invalid")
	}
	assignment, err := storedModelAssignment(map[string]any{"provider_selection": value.Assignment})
	return value.Binding, assignment, value.ResponseID, value.Hosted, err
}

func (c *Chat) finishMCPAuthCall(request store.MCPAuthRequest, payload json.RawMessage, success bool) error {
	_, assignment, responseID, hostedState, err := decodeMCPAuthAuthority(request.BindingJSON)
	if err != nil {
		return err
	}
	return c.finishMCPAuthCallWithAssignment(request, payload, success, assignment, responseID, hostedState)
}

func (c *Chat) finishMCPAuthCallWithAssignment(request store.MCPAuthRequest, payload json.RawMessage, success bool,
	assignment store.ModelAssignment, responseID string, hostedState bool) error {
	turn, input, err := c.database.MCPAuthConversationCall(c.ctx, request)
	if err != nil {
		return err
	}
	input.Payload, input.Success = payload, success
	item, err := c.database.FinishConversationToolCall(c.ctx, turn, input, time.Now())
	if err != nil {
		return err
	}
	c.publish(Event{Kind: EventConversationItem, ConversationID: request.ConversationID, TurnID: request.TurnID, Item: &item})
	c.publish(Event{Kind: EventHumanInterventionsChanged, ConversationID: request.ConversationID})
	conversation, err := c.database.Conversation(c.ctx, request.ConversationID)
	if err != nil {
		return err
	}
	continuation, err := c.database.BeginConversationContinuation(c.ctx, request.ConversationID, item.ID, time.Now())
	if err != nil {
		return err
	}
	generator, err := c.generatorFor(assignment.ProviderKind)
	if err != nil {
		return err
	}
	generator, closeSession, _ := openGenerationSession(generator)
	defer closeSession()
	queued := queuedTurn{input: SendTurnInput{ConversationID: request.ConversationID}, conversation: conversation, location: time.UTC}
	incremental := provider.GenerationMessage{Role: "tool", ToolResult: &provider.ReplayToolResult{ProviderCallID: request.ProviderCallID,
		ProviderName: request.ProviderName, Name: request.CapabilityName, Arguments: json.RawMessage(request.ArgumentsJSON), Success: success, Payload: payload}}
	result, _, err := c.generateChatToolContinuation(queued, continuation, assignment, generator, request.ProviderRound+1,
		"", c.memoryRootContext(), responseID, hostedState, incremental, nil)
	if err != nil {
		c.failTurn(queued.input, continuation, err)
		return nil
	}
	if len(result.ToolCalls) == 0 {
		c.finishGeneratedTurn(queued.input, continuation, assignment, result, request.ProviderRound+1, result.Usage)
		return nil
	}
	if len(result.ToolCalls) != 1 || !c.supportsChatTool(c.ctx, result.ToolCalls[0].Name) {
		return errors.New("provider returned an unsupported tool sequence")
	}
	c.executeChatToolRounds(queued, continuation, assignment, generator, result, c.memoryRootContext(), request.ProviderRound+1, hostedState)
	return nil
}
