package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"reflect"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func adapterOutcomeUncertain(payload json.RawMessage) bool {
	var value struct {
		Code string `json:"code"`
	}
	return json.Unmarshal(payload, &value) == nil && value.Code == "outcome_uncertain"
}

func (c *Chat) prepareAdapterAction(conversation store.Conversation, turn store.ConversationTurn, call store.ConversationItem,
	assignment store.ModelAssignment, round int, responseID string, hostedState bool, binding adapter.Binding, arguments json.RawMessage) (json.RawMessage, bool, *store.ConversationItem, error) {
	if binding.ReviewRoute == "" {
		payload, success, notice, err := c.callAdapter(binding, arguments)
		if errors.Is(err, adapter.ErrAuthenticationRequired) {
			notice, err = c.createChatAdapterAuth(conversation, turn, call, binding, arguments, "", modelAssignmentValue(assignment), round, responseID, hostedState)
			return nil, false, notice, err
		}
		return payload, success, notice, err
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
	if errors.Is(err, adapter.ErrOutcomeUncertain) {
		return toolFailure("outcome_uncertain", "Adapter call outcome is uncertain"), false, nil, nil
	}
	if errors.Is(err, adapter.ErrAuthenticationRequired) {
		return nil, false, nil, err
	}
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
		action.CapabilityName != binding.Name || action.Behavior != binding.Behavior || !sameJSON(mustJSON(action.InputSchema), binding.InputSchema) {
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
	payload, success, callErr := c.adapters.CallReviewed(c.ctx, binding, mustJSON(claimed.Arguments), adapter.ReviewedAuthorization{ActionID: claimed.ID, Revision: claimed.Revision, ArgumentsSHA256: claimed.ArgumentsSHA256})
	if errors.Is(callErr, adapter.ErrAuthenticationRequired) {
		notice, authErr := c.createChatAdapterAuth(store.Conversation{ID: action.ConversationID}, store.ConversationTurn{ID: action.TurnID}, store.ConversationItem{ID: action.CallItemID}, binding, mustJSON(claimed.Arguments), claimed.ID, action.AuthorizationContext["provider_selection"], int(numberField(action.AuthorizationContext, "provider_round")), textField(action.AuthorizationContext, "provider_response_id"), boolField(action.AuthorizationContext, "hosted_state"))
		return nil, false, notice, authErr
	}
	state, failure := store.ActionSucceeded, ""
	if callErr != nil {
		state, failure, success = store.ActionFailed, "adapter_call_failed", false
		payload = toolFailure("adapter_call_failed", "Adapter call failed")
	}
	if errors.Is(callErr, adapter.ErrOutcomeUncertain) {
		state, failure = store.ActionOutcomeUncertain, "outcome_uncertain"
		payload = toolFailure("outcome_uncertain", "Adapter call outcome is uncertain")
	}
	if !success && callErr == nil {
		state, failure = store.ActionFailed, "remote_tool_failed"
	}
	_, err = c.database.FinishActionRequest(c.ctx, claimed.ID, claimed.Revision, state, payload, failure, time.Now())
	return payload, state == store.ActionSucceeded, nil, err
}

func sameJSON(left, right json.RawMessage) bool {
	var leftValue, rightValue any
	if json.Unmarshal(left, &leftValue) != nil || json.Unmarshal(right, &rightValue) != nil {
		return false
	}
	return reflect.DeepEqual(leftValue, rightValue)
}

func (c *Chat) createChatAdapterAuth(conversation store.Conversation, turn store.ConversationTurn, call store.ConversationItem,
	binding adapter.Binding, arguments json.RawMessage, actionID string, assignment any, round int, responseID string, hosted bool) (*store.ConversationItem, error) {
	authority, _ := json.Marshal(map[string]any{"binding": binding, "assignment": assignment, "response_id": responseID, "hosted_state": hosted})
	authorityKind, authorityID := "adapter_connection", binding.ConnectionID
	if binding.GrantID != "" {
		authorityKind, authorityID = "adapter_grant", binding.GrantID
	}
	_, notice, err := c.database.CreateMCPAuthRequest(c.ctx, store.MCPAuthRequest{OwnerHumanID: "human:local", ConversationID: conversation.ID,
		TurnID: turn.ID, CallItemID: call.ID, AuthorityKind: authorityKind, AuthorityID: authorityID, AdapterConnectionID: nullableAdapterConnection(binding), AdapterSemanticDigest: nullableAdapterDigest(binding), AdapterAuthorityRevision: nullableAdapterRevision(binding),
		CapabilityName: binding.Name, ActionID: actionID, BindingJSON: string(authority), ArgumentsJSON: string(arguments), ProviderRound: round}, time.Now())
	if err != nil {
		return nil, err
	}
	return &notice, nil
}

func textField(values map[string]any, name string) string {
	value, _ := values[name].(string)
	return value
}
func boolField(values map[string]any, name string) bool {
	value, _ := values[name].(bool)
	return value
}

func (r *TaskExecution) prepareTaskAdapter(ctx context.Context, task store.Task, run store.TaskRun, call store.TaskRunItem,
	binding adapter.Binding, arguments json.RawMessage) (json.RawMessage, bool, bool, error) {
	if err := r.adapters.Validate(binding, arguments); err != nil {
		return toolFailure("invalid_input", "Adapter arguments are invalid"), false, false, nil
	}
	if binding.ReviewRoute == "" {
		payload, success, err := r.adapters.Call(ctx, binding, arguments)
		if errors.Is(err, adapter.ErrAuthenticationRequired) {
			return nil, false, true, r.createTaskAdapterAuth(ctx, task, run, call, binding, arguments, "")
		}
		if errors.Is(err, adapter.ErrOutcomeUncertain) {
			return toolFailure("outcome_uncertain", "Adapter call outcome is uncertain"), false, true, nil
		}
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
	payload, success, callErr := r.adapters.CallReviewed(ctx, binding, mustJSON(claimed.Arguments), adapter.ReviewedAuthorization{ActionID: claimed.ID, Revision: claimed.Revision, ArgumentsSHA256: claimed.ArgumentsSHA256})
	if errors.Is(callErr, adapter.ErrAuthenticationRequired) {
		call, loadErr := r.taskActionCall(ctx, claimed)
		if loadErr != nil {
			return nil, false, false, loadErr
		}
		return nil, false, true, r.createTaskAdapterAuth(ctx, store.Task{ID: claimed.TaskID}, store.TaskRun{ID: claimed.RunID, TaskID: claimed.TaskID, Generation: claimed.TaskGeneration}, call, binding, mustJSON(claimed.Arguments), claimed.ID)
	}
	state, failure := store.ActionSucceeded, ""
	if callErr != nil {
		state, failure, success, payload = store.ActionFailed, "adapter_call_failed", false, toolFailure("adapter_call_failed", "Adapter call failed")
	}
	if errors.Is(callErr, adapter.ErrOutcomeUncertain) {
		state, failure = store.ActionOutcomeUncertain, "outcome_uncertain"
		payload = toolFailure("outcome_uncertain", "Adapter call outcome is uncertain")
	}
	if !success && callErr == nil {
		state, failure = store.ActionFailed, "remote_tool_failed"
	}
	_, err = r.database.FinishActionRequest(ctx, claimed.ID, claimed.Revision, state, payload, failure, time.Now())
	return payload, state == store.ActionSucceeded, state == store.ActionOutcomeUncertain, err
}

func (r *TaskExecution) createTaskAdapterAuth(ctx context.Context, task store.Task, run store.TaskRun, call store.TaskRunItem,
	binding adapter.Binding, arguments json.RawMessage, actionID string) error {
	authority, _ := json.Marshal(map[string]any{"binding": binding})
	authorityKind, authorityID := "adapter_connection", binding.ConnectionID
	if binding.GrantID != "" {
		authorityKind, authorityID = "adapter_grant", binding.GrantID
	}
	_, _, err := r.database.CreateMCPAuthRequest(ctx, store.MCPAuthRequest{OwnerHumanID: "human:local", TaskID: task.ID,
		RunID: run.ID, RunItemID: call.ID, TaskGeneration: run.Generation, AuthorityKind: authorityKind, AuthorityID: authorityID, AdapterConnectionID: nullableAdapterConnection(binding), AdapterSemanticDigest: nullableAdapterDigest(binding), AdapterAuthorityRevision: nullableAdapterRevision(binding),
		CapabilityName: binding.Name, ActionID: actionID, BindingJSON: string(authority), ArgumentsJSON: string(arguments), ProviderRound: int(call.Round)}, time.Now())
	return err
}

func nullableAdapterConnection(binding adapter.Binding) string {
	if binding.GrantID != "" {
		return binding.ConnectionID
	}
	return ""
}
func nullableAdapterDigest(binding adapter.Binding) string {
	if binding.GrantID != "" {
		return binding.SemanticDigest
	}
	return ""
}
func nullableAdapterRevision(binding adapter.Binding) int {
	if binding.GrantID != "" {
		return binding.CredentialRevision
	}
	return 0
}
