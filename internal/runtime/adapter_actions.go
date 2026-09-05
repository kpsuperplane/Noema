package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func (c *Chat) prepareAdapterAction(conversation store.Conversation, turn store.ConversationTurn, call store.ConversationItem,
	assignment store.ModelAssignment, round int, responseID string, hostedState bool, binding adapter.Binding, arguments json.RawMessage) (json.RawMessage, bool, *store.ConversationItem, error) {
	if binding.ReviewRoute == "" {
		return c.callAdapter(binding, arguments)
	}
	authority, err := c.database.ConversationAuthorizationContext(c.ctx, conversation.ID, turn.ID)
	if err != nil {
		return nil, false, nil, err
	}
	value := map[string]any{"origin": "primary_conversation", "context": authority, "conversation_id": conversation.ID,
		"execution_decision": string(binding.ReviewRoute), "provider_selection": modelAssignmentValue(assignment), "provider_round": round,
		"destination":     map[string]any{"service_id": binding.DefinitionID, "connection_id": binding.ConnectionID, "revision": binding.ConnectionRevision},
		"adapter_binding": binding, "provider_response_id": responseID, "hosted_state": hostedState}
	action, err := c.database.CreateActionRequest(c.ctx, store.NewActionRequest{ConversationID: conversation.ID, TurnID: turn.ID,
		CallItemID: call.ID, OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: binding.Name,
		OperationToken: binding.Name, ReviewRoute: binding.ReviewRoute, Behavior: binding.Behavior, Arguments: arguments,
		InputSchema: binding.InputSchema, AuthorizationContext: value, SafeSummary: "Use " + binding.OperationID + " on " + binding.DefinitionID}, time.Now())
	if err != nil {
		return nil, false, nil, err
	}
	if action.ReviewRoute == store.ActionLLMReview {
		action, approval, err := c.database.RecordActionAssessment(c.ctx, action.ID, action.Revision, c.reviewActionRequest(action), time.Now())
		if err != nil || approval != nil {
			return nil, false, approval, err
		}
		return c.executeReviewedAdapter(action)
	}
	approval, err := c.database.VisibleConversationItem(c.ctx, action.ApprovalItemID)
	if err != nil || approval == nil {
		return nil, false, nil, errors.New("adapter approval request is unavailable")
	}
	return nil, false, approval, nil
}

func (c *Chat) callAdapter(binding adapter.Binding, arguments json.RawMessage) (json.RawMessage, bool, *store.ConversationItem, error) {
	payload, success, err := c.adapters.Call(c.ctx, binding, arguments)
	if err != nil {
		return toolFailure("adapter_call_failed", "Adapter call failed"), false, nil, nil
	}
	return payload, success, nil, nil
}

func (c *Chat) executeReviewedAdapter(action store.ActionRequest) (json.RawMessage, bool, *store.ConversationItem, error) {
	if c.adapters == nil {
		return nil, false, nil, errors.New("adapter service is unavailable")
	}
	var binding adapter.Binding
	if json.Unmarshal(mustJSON(action.AuthorizationContext["adapter_binding"]), &binding) != nil || action.OperationToken != binding.Name ||
		action.CapabilityName != binding.Name || action.Behavior != binding.Behavior || string(mustJSON(action.InputSchema)) != string(binding.InputSchema) {
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
	payload, success, callErr := c.adapters.Call(c.ctx, binding, mustJSON(claimed.Arguments))
	state, failure := store.ActionSucceeded, ""
	if callErr != nil {
		state, failure, success = store.ActionFailed, "adapter_call_failed", false
		payload = toolFailure("adapter_call_failed", "Adapter call failed")
	}
	if errors.Is(callErr, adapter.ErrOutcomeUncertain) {
		state, failure = store.ActionOutcomeUncertain, "outcome_uncertain"
	}
	if !success && callErr == nil {
		state, failure = store.ActionFailed, "remote_tool_failed"
	}
	_, err = c.database.FinishActionRequest(c.ctx, claimed.ID, claimed.Revision, state, payload, failure, time.Now())
	return payload, state == store.ActionSucceeded, nil, err
}

func (r *TaskExecution) prepareTaskAdapter(ctx context.Context, task store.Task, run store.TaskRun, call store.TaskRunItem,
	binding adapter.Binding, arguments json.RawMessage) (json.RawMessage, bool, bool, error) {
	if err := r.adapters.Validate(binding, arguments); err != nil {
		return toolFailure("invalid_input", "Adapter arguments are invalid"), false, false, nil
	}
	if binding.ReviewRoute == "" {
		payload, success, err := r.adapters.Call(ctx, binding, arguments)
		if err != nil {
			return toolFailure("adapter_call_failed", "Adapter call failed"), false, false, nil
		}
		return payload, success, false, nil
	}
	document, err := home.ReadTaskDocument(r.root, task.ID)
	if err != nil {
		return nil, false, false, err
	}
	authority := map[string]any{"origin": "task_execution", "task_id": task.ID, "run_id": run.ID, "task_generation": run.Generation,
		"run_kind": run.Kind, "execution_decision": string(binding.ReviewRoute), "task_title": task.Title, "task_document": document.Content,
		"destination":     map[string]any{"service_id": binding.DefinitionID, "connection_id": binding.ConnectionID, "revision": binding.ConnectionRevision},
		"adapter_binding": binding, "provider_round": call.Round, "provider_call_id": taskPayloadText(call.Payload, "provider_call_id"), "provider_name": taskPayloadText(call.Payload, "provider_name")}
	action, err := r.database.CreateActionRequest(ctx, store.NewActionRequest{TaskID: task.ID, RunID: run.ID, RunItemID: call.ID,
		TaskGeneration: run.Generation, OwnerHumanID: "human:local", RequestingAgentID: run.AgentID, CapabilityName: binding.Name,
		OperationToken: binding.Name, ReviewRoute: binding.ReviewRoute, Behavior: binding.Behavior, Arguments: arguments,
		InputSchema: binding.InputSchema, AuthorizationContext: authority, SafeSummary: "Use " + strings.TrimSpace(binding.OperationID) + " on " + binding.DefinitionID}, time.Now())
	if err != nil {
		return nil, false, false, err
	}
	if action.ReviewRoute == store.ActionLLMReview {
		action, _, err = r.database.RecordActionAssessment(ctx, action.ID, action.Revision, reviewActionRequest(ctx, r.database, r.generator, action), time.Now())
		if err != nil {
			return nil, false, false, err
		}
	}
	if action.State == store.ActionAwaitingApproval {
		return nil, false, true, r.database.SuspendTaskExecution(ctx, run.ID, run.Generation, time.Now())
	}
	return r.executeTaskAdapterAction(ctx, action)
}

func (r *TaskExecution) executeTaskAdapterAction(ctx context.Context, action store.ActionRequest) (json.RawMessage, bool, bool, error) {
	var binding adapter.Binding
	if r.adapters == nil || json.Unmarshal(mustJSON(action.AuthorizationContext["adapter_binding"]), &binding) != nil || action.OperationToken != binding.Name || action.CapabilityName != binding.Name || action.Behavior != binding.Behavior {
		_, err := r.database.SupersedeActionRequest(ctx, action.ID, action.Revision, "action_authority_changed", time.Now())
		return toolFailure("action_authority_changed", "Adapter authority changed"), false, false, err
	}
	claimed, err := r.database.ClaimActionRequest(ctx, action.ID, action.Revision, time.Now())
	if err != nil {
		return nil, false, false, err
	}
	payload, success, callErr := r.adapters.Call(ctx, binding, mustJSON(claimed.Arguments))
	state, failure := store.ActionSucceeded, ""
	if callErr != nil {
		state, failure, success, payload = store.ActionFailed, "adapter_call_failed", false, toolFailure("adapter_call_failed", "Adapter call failed")
	}
	if errors.Is(callErr, adapter.ErrOutcomeUncertain) {
		state, failure = store.ActionOutcomeUncertain, "outcome_uncertain"
	}
	if !success && callErr == nil {
		state, failure = store.ActionFailed, "remote_tool_failed"
	}
	_, err = r.database.FinishActionRequest(ctx, claimed.ID, claimed.Revision, state, payload, failure, time.Now())
	return payload, state == store.ActionSucceeded, false, err
}
