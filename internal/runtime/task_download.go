package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func taskDownloadBehavior() store.ActionBehavior {
	return store.ActionBehavior{ReadOnly: false, RepeatSafe: false, Destructive: false, OpenWorld: true}
}

func (r *TaskExecution) prepareTaskDownload(ctx context.Context, task store.Task, run store.TaskRun, call store.TaskRunItem, arguments json.RawMessage) (json.RawMessage, bool, bool, error) {
	if _, err := parseFileDownloadArguments(arguments); err != nil {
		return toolFailure("invalid_input", err.Error()), false, false, nil
	}
	if payload, success, observed, err := executeObservedDownload(ctx, r.database, taskWorkspace(r.root, task.ID), arguments); err != nil || observed {
		return payload, success, false, err
	}
	document, err := home.ReadTaskDocument(r.root, task.ID)
	if err != nil {
		return nil, false, false, err
	}
	authority := map[string]any{
		"origin": "task_execution", "task_id": task.ID, "run_id": run.ID, "task_generation": run.Generation,
		"run_kind": run.Kind, "execution_decision": "llm_review", "task_title": task.Title, "task_document": document.Content,
		"destination":    map[string]any{"service_id": "public_web", "connection_id": "file_download", "revision": "1"},
		"provider_round": call.Round, "provider_call_id": taskPayloadText(call.Payload, "provider_call_id"), "provider_name": taskPayloadText(call.Payload, "provider_name"),
	}
	action, err := r.database.CreateActionRequest(ctx, store.NewActionRequest{
		TaskID: task.ID, RunID: run.ID, RunItemID: call.ID, TaskGeneration: run.Generation,
		OwnerHumanID: "human:local", RequestingAgentID: run.AgentID, CapabilityName: fileDownloadName,
		OperationToken: fileDownloadName, ReviewRoute: store.ActionLLMReview, Behavior: taskDownloadBehavior(),
		Arguments: arguments, InputSchema: fileDownloadSchema, AuthorizationContext: authority,
		SafeSummary: "Download a public file into the Task directory",
	}, time.Now())
	if err != nil {
		return nil, false, false, err
	}
	action, _, err = r.database.RecordActionAssessment(ctx, action.ID, action.Revision, reviewActionRequest(ctx, r.database, r.generator, action), time.Now())
	if err != nil {
		return nil, false, false, err
	}
	if action.State == store.ActionAwaitingApproval {
		return nil, false, true, r.database.SuspendTaskExecution(ctx, run.ID, run.Generation, time.Now())
	}
	return r.executeTaskDownloadAction(ctx, action)
}

func (r *TaskExecution) executeTaskDownloadAction(ctx context.Context, action store.ActionRequest) (json.RawMessage, bool, bool, error) {
	var schema map[string]any
	_ = json.Unmarshal(fileDownloadSchema, &schema)
	if action.CapabilityName != fileDownloadName || action.OperationToken != fileDownloadName || action.Behavior != taskDownloadBehavior() || !reflect.DeepEqual(action.InputSchema, schema) {
		_, err := r.database.SupersedeActionRequest(ctx, action.ID, action.Revision, "action_authority_changed", time.Now())
		return toolFailure("action_authority_changed", "download authority changed"), false, false, err
	}
	claimed, err := r.database.ClaimActionRequest(ctx, action.ID, action.Revision, time.Now())
	if err != nil {
		return nil, false, false, err
	}
	workspace := taskWorkspace(r.root, claimed.TaskID)
	payload, executeErr := executeFileDownload(ctx, workspace, mustJSON(claimed.Arguments))
	state, failure, success := store.ActionSucceeded, "", true
	if executeErr != nil {
		state, failure, success = store.ActionFailed, "download_failed", false
		payload = toolFailure("download_failed", "File download failed")
		if errors.Is(executeErr, errDownloadOutcomeUncertain) {
			state, failure = store.ActionOutcomeUncertain, "outcome_uncertain"
		}
	}
	_, err = r.database.FinishActionRequest(ctx, claimed.ID, claimed.Revision, state, payload, failure, time.Now())
	return payload, success, state == store.ActionOutcomeUncertain, err
}

func taskWorkspace(root *os.Root, taskID string) string {
	return filepath.Join(root.Name(), "tasks", strings.TrimPrefix(taskID, "task:"))
}
