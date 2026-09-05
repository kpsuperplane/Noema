package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/home"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/webtool"
)

func (r *TaskExecution) prepareTaskMCP(ctx context.Context, task store.Task, run store.TaskRun, call store.TaskRunItem,
	binding noemamcp.Binding, arguments json.RawMessage) (json.RawMessage, bool, bool, error) {
	if binding.ReviewRoute == "" {
		payload, success, err := r.mcp.Call(ctx, binding, arguments)
		if errors.Is(err, noemamcp.ErrAuthenticationRequired) {
			return nil, false, true, r.createTaskMCPAuth(ctx, task, run, call, binding, arguments, "")
		}
		if err != nil {
			return toolFailure("mcp_call_failed", "MCP tool call failed"), false, false, nil
		}
		return payload, success, false, nil
	}
	server, err := r.mcp.Server(ctx, binding.ServerID)
	if err != nil {
		return nil, false, false, err
	}
	document, err := home.ReadTaskDocument(r.root, task.ID)
	if err != nil {
		return nil, false, false, err
	}
	authority := map[string]any{
		"origin": "task_execution", "task_id": task.ID, "run_id": run.ID, "task_generation": run.Generation,
		"run_kind": run.Kind, "execution_decision": string(binding.ReviewRoute),
		"task_title": task.Title, "task_document": document.Content,
		"destination": map[string]any{"service_id": server.DefinitionID, "connection_id": server.ID, "revision": server.ConnectionRevision},
		"service":     map[string]any{"display_name": server.DisplayName, "connection_label": server.ConnectionLabel},
		"mcp_binding": binding, "provider_round": call.Round,
		"provider_call_id": taskPayloadText(call.Payload, "provider_call_id"), "provider_name": taskPayloadText(call.Payload, "provider_name"),
	}
	action, err := r.database.CreateActionRequest(ctx, store.NewActionRequest{
		TaskID: task.ID, RunID: run.ID, RunItemID: call.ID, TaskGeneration: run.Generation,
		OwnerHumanID: "human:local", RequestingAgentID: run.AgentID, CapabilityName: binding.Name,
		OperationToken: binding.Name, ReviewRoute: binding.ReviewRoute, Behavior: binding.Behavior,
		Arguments: arguments, InputSchema: binding.InputSchema, AuthorizationContext: authority,
		SafeSummary: "Use " + strings.TrimPrefix(binding.Name, "mcp."+server.ID+".") + " on " + server.DisplayName,
	}, time.Now())
	if err != nil {
		return nil, false, false, err
	}
	if action.ReviewRoute == store.ActionLLMReview {
		action, _, err = r.database.RecordActionAssessment(ctx, action.ID, action.Revision,
			reviewActionRequest(ctx, r.database, r.generator, action), time.Now())
		if err != nil {
			return nil, false, false, err
		}
	}
	if action.State == store.ActionAwaitingApproval {
		return nil, false, true, r.database.SuspendTaskExecution(ctx, run.ID, run.Generation, time.Now())
	}
	return r.executeTaskMCPAction(ctx, task, run, call, action)
}

func (r *TaskExecution) executeTaskMCPAction(ctx context.Context, task store.Task, run store.TaskRun, call store.TaskRunItem,
	action store.ActionRequest) (json.RawMessage, bool, bool, error) {
	var binding noemamcp.Binding
	if json.Unmarshal(mustJSON(action.AuthorizationContext["mcp_binding"]), &binding) != nil ||
		action.OperationToken != binding.Name || action.CapabilityName != binding.Name || action.Behavior != binding.Behavior {
		_, err := r.database.SupersedeActionRequest(ctx, action.ID, action.Revision, "action_authority_changed", time.Now())
		return toolFailure("action_authority_changed", "MCP tool authority changed"), false, false, err
	}
	claimed, err := r.database.ClaimActionRequest(ctx, action.ID, action.Revision, time.Now())
	if err != nil {
		return nil, false, false, err
	}
	payload, success, callErr := r.mcp.Call(ctx, binding, mustJSON(claimed.Arguments))
	if errors.Is(callErr, noemamcp.ErrAuthenticationRequired) {
		return nil, false, true, r.createTaskMCPAuth(ctx, task, run, call, binding, mustJSON(claimed.Arguments), claimed.ID)
	}
	state, failure := store.ActionSucceeded, ""
	if callErr != nil {
		state, failure, payload, success = store.ActionFailed, "mcp_call_failed", toolFailure("mcp_call_failed", "MCP tool call failed"), false
	} else if !success {
		state, failure = store.ActionFailed, "remote_tool_failed"
	}
	_, err = r.database.FinishActionRequest(ctx, claimed.ID, claimed.Revision, state, payload, failure, time.Now())
	return payload, success, false, err
}

func (r *TaskExecution) createTaskMCPAuth(ctx context.Context, task store.Task, run store.TaskRun, call store.TaskRunItem,
	binding noemamcp.Binding, arguments json.RawMessage, actionID string) error {
	token, _ := json.Marshal(map[string]any{"binding": binding})
	_, _, err := r.database.CreateMCPAuthRequest(ctx, store.MCPAuthRequest{OwnerHumanID: "human:local",
		TaskID: task.ID, RunID: run.ID, RunItemID: call.ID, TaskGeneration: run.Generation,
		ServerID: binding.ServerID, CapabilityName: binding.Name, ActionID: actionID,
		BindingJSON: string(token), ArgumentsJSON: string(arguments), ProviderRound: int(call.Round)}, time.Now())
	return err
}

func taskPayloadText(value map[string]any, key string) string {
	text, _ := value[key].(string)
	return text
}

// ResolveActionRequest applies one human decision to an exact Task MCP call.
func (r *TaskExecution) ResolveActionRequest(ctx context.Context, actionID string, revision int, humanID, decision string) (store.ActionRequest, error) {
	action, err := r.database.ActionRequest(ctx, actionID, revision)
	if err != nil || action.TaskID == "" {
		return store.ActionRequest{}, errors.New("Task action is unavailable")
	}
	action, err = r.database.DecideActionRequest(ctx, actionID, revision, humanID, decision, time.Now())
	if err != nil {
		return store.ActionRequest{}, err
	}
	call, err := r.taskActionCall(ctx, action)
	if err != nil {
		return store.ActionRequest{}, err
	}
	if decision == "decline" {
		payload, _ := json.Marshal(actionResultPayload(action))
		return action, r.completeTaskMCPResult(ctx, action, call, payload, false)
	}
	round := int(call.Round)
	toolStarted := time.Now()
	toolSpan, _ := r.database.BeginRuntimeDebugSpan(ctx,
		store.RuntimeDebugScope{Kind: "task_run", ID: action.RunID}, "tool", "Task tool call",
		store.RuntimeDebugMetadata{Phase: "execution", ToolName: action.CapabilityName,
			CorrelationID: taskPayloadText(call.Payload, "provider_call_id"), RoundIndex: &round}, toolStarted)
	var payload json.RawMessage
	var success, paused bool
	if action.AuthorizationContext["adapter_binding"] != nil {
		payload, success, paused, err = r.executeTaskAdapterAction(ctx, action)
	} else if action.CapabilityName == webtool.FetchName {
		payload, success, paused, err = r.executeTaskWebFetch(ctx, action)
	} else if webtool.IsBrowserTool(action.CapabilityName) {
		var result webtool.BrowserResult
		result, paused, err = r.executeTaskBrowser(ctx, action)
		payload, success = result.Stored, result.Success
	} else {
		payload, success, paused, err = r.executeTaskMCPAction(ctx,
			store.Task{ID: action.TaskID}, store.TaskRun{ID: action.RunID, TaskID: action.TaskID, Generation: action.TaskGeneration}, call, action)
	}
	toolStatus := "completed"
	if err != nil || (!success && !paused) {
		toolStatus = "failed"
	}
	if toolSpan != "" {
		_ = r.database.FinishRuntimeDebugSpan(context.WithoutCancel(ctx), toolSpan, toolStatus,
			store.RuntimeDebugMetadata{Phase: "execution", ToolName: action.CapabilityName,
				CorrelationID: taskPayloadText(call.Payload, "provider_call_id"), RoundIndex: &round},
			time.Since(toolStarted), time.Now())
	}
	if err != nil || (paused && payload == nil) {
		return action, err
	}
	action, err = r.database.ActionRequest(ctx, action.ID, action.Revision)
	if err != nil {
		return store.ActionRequest{}, err
	}
	return action, r.completeTaskMCPResult(ctx, action, call, payload, success)
}

// ResumeMCPAuthentication resumes Task calls bound to one completed OAuth attempt.
func (r *TaskExecution) ResumeMCPAuthentication(ctx context.Context, attemptID string) (bool, error) {
	requests, err := r.database.MCPAuthRequestsForAttempt(ctx, attemptID)
	if err != nil {
		return false, err
	}
	handled := false
	for _, request := range requests {
		if request.TaskID == "" {
			continue
		}
		handled = true
		request, err = r.database.BeginMCPAuthResume(ctx, request, time.Now())
		if err != nil {
			return true, err
		}
		var authority struct {
			Binding noemamcp.Binding `json:"binding"`
		}
		if json.Unmarshal([]byte(request.BindingJSON), &authority) != nil {
			return true, errors.New("stored MCP Task authority is invalid")
		}
		payload, success, callErr := r.mcp.Call(ctx, authority.Binding, json.RawMessage(request.ArgumentsJSON))
		state, failure := store.ActionSucceeded, ""
		if callErr != nil {
			state, failure, payload, success = store.ActionFailed, "mcp_call_failed", toolFailure("mcp_call_failed", "MCP tool call failed"), false
		} else if !success {
			state, failure = store.ActionFailed, "remote_tool_failed"
		}
		if request.ActionID != "" {
			if _, _, err = r.database.FinishMCPAuthAction(ctx, request, state, payload, failure, "completed", time.Now()); err != nil {
				return true, err
			}
		} else if _, err = r.database.FinishMCPAuthRequest(ctx, request.ID, request.Revision, "completed", failure, time.Now()); err != nil {
			return true, err
		}
		call, loadErr := r.taskAuthCall(ctx, request)
		if loadErr != nil {
			return true, loadErr
		}
		if err = r.completeTaskMCPResult(ctx, store.ActionRequest{TaskID: request.TaskID, RunID: request.RunID,
			TaskGeneration: request.TaskGeneration, CapabilityName: request.CapabilityName,
			AuthorizationContext: map[string]any{"provider_call_id": request.ProviderCallID, "provider_name": request.ProviderName}}, call, payload, success); err != nil {
			return true, err
		}
	}
	return handled, nil
}

// ResumeAdapterAuthentication resumes Task calls after one credential replacement.
func (r *TaskExecution) ResumeAdapterAuthentication(ctx context.Context, connectionID string) (bool, error) {
	requests, err := r.database.AdapterAuthRequestsForConnection(ctx, connectionID)
	if err != nil {
		return false, err
	}
	handled := false
	var firstErr error
	for _, request := range requests {
		if request.TaskID == "" {
			continue
		}
		handled = true
		var authority struct {
			Binding adapter.Binding `json:"binding"`
		}
		if json.Unmarshal([]byte(request.BindingJSON), &authority) != nil {
			continue
		}
		binding, callErr := currentAdapterCredentialBinding(r.adapters, authority.Binding)
		if callErr != nil {
			continue
		}
		request, err = r.database.BeginAdapterAuthResume(ctx, request, time.Now())
		if err != nil {
			firstErr = errors.Join(firstErr, err)
			continue
		}
		payload, success := toolFailure("adapter_authentication_failed", "Adapter authentication failed"), false
		payload, success, callErr = r.adapters.Call(ctx, binding, json.RawMessage(request.ArgumentsJSON))
		if errors.Is(callErr, adapter.ErrAuthenticationRequired) {
			if _, err = r.database.RetryAdapterAuthentication(ctx, request, time.Now()); err != nil {
				firstErr = errors.Join(firstErr, err)
			}
			continue
		}
		state, failure := store.ActionSucceeded, ""
		if errors.Is(callErr, adapter.ErrOutcomeUncertain) {
			state, failure, payload, success = store.ActionOutcomeUncertain, "outcome_uncertain", toolFailure("outcome_uncertain", "Adapter call outcome is uncertain"), false
		} else if callErr != nil || !success {
			state, failure = store.ActionFailed, "adapter_call_failed"
		}
		if request.ActionID != "" {
			if _, _, err = r.database.FinishMCPAuthAction(ctx, request, state, payload, failure, "completed", time.Now()); err != nil {
				firstErr = errors.Join(firstErr, err)
				continue
			}
		}
		call, loadErr := r.taskAuthCall(ctx, request)
		if loadErr != nil {
			firstErr = errors.Join(firstErr, loadErr)
			continue
		}
		if err = r.completeTaskMCPResult(ctx, store.ActionRequest{TaskID: request.TaskID, RunID: request.RunID, TaskGeneration: request.TaskGeneration, CapabilityName: request.CapabilityName, State: state, AuthorizationContext: map[string]any{"provider_call_id": request.ProviderCallID, "provider_name": request.ProviderName}}, call, payload, success); err != nil {
			firstErr = errors.Join(firstErr, err)
			continue
		}
		if request.ActionID == "" {
			if _, err = r.database.FinishMCPAuthRequest(ctx, request.ID, request.Revision, "completed", failure, time.Now()); err != nil {
				firstErr = errors.Join(firstErr, err)
			}
		}
	}
	return handled, firstErr
}

// SkipMCPAuthentication closes one exact Task call without credentials.
func (r *TaskExecution) SkipMCPAuthentication(ctx context.Context, request store.MCPAuthRequest) (store.MCPAuthRequest, error) {
	return r.finishAdapterAuthentication(ctx, request, "cancelled", "authentication_skipped")
}

// SupersedeAdapterAuthentication closes one Task call attached to an obsolete OAuth attempt.
func (r *TaskExecution) SupersedeAdapterAuthentication(ctx context.Context, request store.MCPAuthRequest) error {
	_, err := r.finishAdapterAuthentication(ctx, request, "superseded", "oauth_attempt_superseded")
	return err
}

func (r *TaskExecution) finishAdapterAuthentication(ctx context.Context, request store.MCPAuthRequest, authState, failure string) (store.MCPAuthRequest, error) {
	if request.TaskID == "" {
		return store.MCPAuthRequest{}, errors.New("Task authentication request is unavailable")
	}
	label := "MCP"
	if request.AuthorityKind == "adapter_connection" || request.AuthorityKind == "adapter_grant" {
		label = "Adapter"
	}
	message := label + " authentication was skipped"
	if authState == "superseded" {
		message = label + " authentication changed"
	}
	payload := toolFailure(failure, message)
	if request.ActionID != "" {
		var err error
		request, _, err = r.database.FinishMCPAuthAction(ctx, request, store.ActionFailed, payload,
			failure, authState, time.Now())
		if err != nil {
			return store.MCPAuthRequest{}, err
		}
		call, err := r.taskAuthCall(ctx, request)
		if err == nil {
			err = r.completeTaskMCPResult(ctx, store.ActionRequest{TaskID: request.TaskID, RunID: request.RunID,
				TaskGeneration: request.TaskGeneration, CapabilityName: request.CapabilityName,
				AuthorizationContext: map[string]any{"provider_call_id": request.ProviderCallID, "provider_name": request.ProviderName}}, call, payload, false)
		}
		return request, err
	}
	call, err := r.taskAuthCall(ctx, request)
	if err == nil {
		err = r.completeTaskMCPResult(ctx, store.ActionRequest{TaskID: request.TaskID, RunID: request.RunID,
			TaskGeneration: request.TaskGeneration, CapabilityName: request.CapabilityName,
			AuthorizationContext: map[string]any{"provider_call_id": request.ProviderCallID, "provider_name": request.ProviderName}}, call, payload, false)
	}
	if err != nil {
		return store.MCPAuthRequest{}, err
	}
	return r.database.FinishMCPAuthRequest(ctx, request.ID, request.Revision, authState, failure, time.Now())
}

func (r *TaskExecution) taskActionCall(ctx context.Context, action store.ActionRequest) (store.TaskRunItem, error) {
	items, err := r.database.TaskRunReplayItems(ctx, action.RunID)
	if err != nil {
		return store.TaskRunItem{}, err
	}
	for _, item := range items {
		if item.ID == action.RunItemID && item.Kind == "tool_call" && item.Status == "running" {
			return item, nil
		}
	}
	return store.TaskRunItem{}, errors.New("Task action call is unavailable")
}

func (r *TaskExecution) taskAuthCall(ctx context.Context, request store.MCPAuthRequest) (store.TaskRunItem, error) {
	return r.taskActionCall(ctx, store.ActionRequest{RunID: request.RunID, RunItemID: request.RunItemID})
}

func (r *TaskExecution) completeTaskMCPResult(ctx context.Context, action store.ActionRequest, call store.TaskRunItem,
	payload json.RawMessage, success bool) error {
	status := "completed"
	if !success {
		status = "failed"
	}
	arguments, _ := call.Payload["arguments"]
	sideEffect, _ := call.Payload["side_effect"].(bool)
	return r.database.CompleteTaskIntervention(ctx, action.RunID, action.TaskGeneration, store.TaskRunItemInput{
		Kind: "tool_result", Status: status, Round: call.Round, ParentID: call.ID,
		Payload: map[string]any{"name": action.CapabilityName, "arguments": arguments, "result": payload,
			"success": success, "side_effect": sideEffect, "provider_call_id": action.AuthorizationContext["provider_call_id"], "provider_name": action.AuthorizationContext["provider_name"]},
	}, action.State == store.ActionOutcomeUncertain, time.Now())
}
