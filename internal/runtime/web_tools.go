package runtime

import (
	"context"
	"encoding/json"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/webtool"
)

func (c *Chat) prepareWebFetchAction(conversation store.Conversation, turn store.ConversationTurn,
	call store.ConversationItem, assignment store.ModelAssignment, providerRound int, arguments json.RawMessage,
) (json.RawMessage, bool, *store.ConversationItem, error) {
	if c.web == nil {
		return toolFailure("unavailable", "web fetch is unavailable"), false, nil, nil
	}
	observed, err := c.web.FetchObserved(c.ctx, arguments)
	if err != nil {
		return toolFailure("invalid_input", err.Error()), false, nil, nil
	}
	if observed {
		payload, success := c.web.Execute(c.ctx, webtool.FetchName, arguments, "conversation:"+conversation.ID+":"+call.ID)
		return payload, success, nil, nil
	}
	binding, err := c.web.CurrentBinding(c.ctx, webtool.FetchName)
	if err != nil {
		return nil, false, nil, err
	}
	authority, err := c.database.ConversationAuthorizationContext(c.ctx, conversation.ID, turn.ID)
	if err != nil {
		return nil, false, nil, err
	}
	contextValue := map[string]any{"origin": "primary_conversation", "context": authority,
		"conversation_id": conversation.ID, "execution_decision": "llm_review", "web_binding": binding,
		"provider_selection": modelAssignmentValue(assignment), "provider_round": providerRound,
		"destination": map[string]any{"service_id": "public_web", "connection_id": binding.ProviderAccountID, "revision": binding.CredentialRevision},
		"service":     map[string]any{"display_name": "Public web"}}
	action, err := c.database.CreateActionRequest(c.ctx, store.NewActionRequest{ConversationID: conversation.ID, TurnID: turn.ID,
		CallItemID: call.ID, OwnerHumanID: "human:local", RequestingAgentID: "agent:primary",
		CapabilityName: webtool.FetchName, OperationToken: webtool.FetchName, ReviewRoute: store.ActionLLMReview,
		Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true, OpenWorld: true}, Arguments: arguments,
		InputSchema: webtool.FetchSchema, AuthorizationContext: contextValue, SafeSummary: "Fetch one public web page"}, time.Now())
	if err != nil {
		return nil, false, nil, err
	}
	action, approval, err := c.database.RecordActionAssessment(c.ctx, action.ID, action.Revision, c.reviewActionRequest(action), time.Now())
	if err != nil || approval != nil {
		return nil, false, approval, err
	}
	return c.executeReviewedWebFetch(action)
}

func (c *Chat) executeReviewedWebFetch(action store.ActionRequest) (json.RawMessage, bool, *store.ConversationItem, error) {
	if c.web == nil || !currentWebBinding(c.ctx, c.web, action) {
		changed, err := c.database.SupersedeActionRequest(c.ctx, action.ID, action.Revision, "action_authority_changed", time.Now())
		if err != nil {
			return nil, false, nil, err
		}
		payload, _ := json.Marshal(actionResultPayload(changed))
		return payload, false, nil, nil
	}
	claimed, err := c.database.ClaimActionRequest(c.ctx, action.ID, action.Revision, time.Now())
	if err != nil {
		return nil, false, nil, err
	}
	payload, success := c.web.Execute(c.ctx, webtool.FetchName, mustJSON(claimed.Arguments), "action:"+claimed.ID)
	state, failure := store.ActionSucceeded, ""
	if !success {
		state, failure = store.ActionFailed, "web_fetch_failed"
	}
	_, err = c.database.FinishActionRequest(c.ctx, claimed.ID, claimed.Revision, state, payload, failure, time.Now())
	return payload, success, nil, err
}

func (r *TaskExecution) prepareTaskWebFetch(ctx context.Context, task store.Task, run store.TaskRun,
	call store.TaskRunItem, arguments json.RawMessage,
) (json.RawMessage, bool, bool, error) {
	if r.web == nil {
		return toolFailure("unavailable", "web fetch is unavailable"), false, false, nil
	}
	observed, err := r.web.FetchObserved(ctx, arguments)
	if err != nil {
		return toolFailure("invalid_input", err.Error()), false, false, nil
	}
	if observed {
		payload, success := r.web.Execute(ctx, webtool.FetchName, arguments, "task:"+run.ID+":"+call.ID)
		return payload, success, false, nil
	}
	binding, err := r.web.CurrentBinding(ctx, webtool.FetchName)
	if err != nil {
		return nil, false, false, err
	}
	document, err := home.ReadTaskDocument(r.root, task.ID)
	if err != nil {
		return nil, false, false, err
	}
	authority := map[string]any{"origin": "task_execution", "task_id": task.ID, "run_id": run.ID,
		"task_generation": run.Generation, "run_kind": run.Kind, "execution_decision": "llm_review",
		"task_title": task.Title, "task_document": document.Content, "web_binding": binding,
		"provider_round": call.Round, "provider_call_id": taskPayloadText(call.Payload, "provider_call_id"),
		"provider_name": taskPayloadText(call.Payload, "provider_name"),
		"destination":   map[string]any{"service_id": "public_web", "connection_id": binding.ProviderAccountID, "revision": binding.CredentialRevision}}
	action, err := r.database.CreateActionRequest(ctx, store.NewActionRequest{TaskID: task.ID, RunID: run.ID, RunItemID: call.ID,
		TaskGeneration: run.Generation, OwnerHumanID: "human:local", RequestingAgentID: run.AgentID,
		CapabilityName: webtool.FetchName, OperationToken: webtool.FetchName, ReviewRoute: store.ActionLLMReview,
		Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true, OpenWorld: true}, Arguments: arguments,
		InputSchema: webtool.FetchSchema, AuthorizationContext: authority, SafeSummary: "Fetch one public web page"}, time.Now())
	if err != nil {
		return nil, false, false, err
	}
	action, _, err = r.database.RecordActionAssessment(ctx, action.ID, action.Revision,
		reviewActionRequest(ctx, r.database, r.generator, action), time.Now())
	if err != nil {
		return nil, false, false, err
	}
	if action.State == store.ActionAwaitingApproval {
		return nil, false, true, r.database.SuspendTaskExecution(ctx, run.ID, run.Generation, time.Now())
	}
	return r.executeTaskWebFetch(ctx, action)
}

func (r *TaskExecution) executeTaskWebFetch(ctx context.Context, action store.ActionRequest) (json.RawMessage, bool, bool, error) {
	if r.web == nil || !currentWebBinding(ctx, r.web, action) {
		_, err := r.database.SupersedeActionRequest(ctx, action.ID, action.Revision, "action_authority_changed", time.Now())
		return toolFailure("action_authority_changed", "web fetch authority changed"), false, false, err
	}
	claimed, err := r.database.ClaimActionRequest(ctx, action.ID, action.Revision, time.Now())
	if err != nil {
		return nil, false, false, err
	}
	payload, success := r.web.Execute(ctx, webtool.FetchName, mustJSON(claimed.Arguments), "action:"+claimed.ID)
	state, failure := store.ActionSucceeded, ""
	if !success {
		state, failure = store.ActionFailed, "web_fetch_failed"
	}
	_, err = r.database.FinishActionRequest(ctx, claimed.ID, claimed.Revision, state, payload, failure, time.Now())
	return payload, success, false, err
}

func currentWebBinding(ctx context.Context, service *webtool.Service, action store.ActionRequest) bool {
	var saved webtool.BindingSnapshot
	if json.Unmarshal(mustJSON(action.AuthorizationContext["web_binding"]), &saved) != nil {
		return false
	}
	current, err := service.CurrentBinding(ctx, webtool.FetchName)
	return err == nil && current == saved && action.CapabilityName == webtool.FetchName && action.OperationToken == webtool.FetchName &&
		action.Behavior == (store.ActionBehavior{ReadOnly: true, RepeatSafe: true, OpenWorld: true})
}
