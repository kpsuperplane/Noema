package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"path/filepath"
	"testing"
	"time"
)

func TestActionRequestSchemaConvergesFromVersionSeventeen(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v17.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec(schemaAtVersion(17) + `PRAGMA user_version=17;`); err != nil {
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	database, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	var version int
	if err := database.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("schema version = %d, %v", version, err)
	}
	for _, table := range []string{"action_requests", "action_request_assessments", "action_request_decisions", "action_request_events"} {
		var exists bool
		if err := database.db.QueryRow(`SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?)`, table).Scan(&exists); err != nil || !exists {
			t.Fatalf("table %s exists = %t, %v", table, exists, err)
		}
	}
}

func TestActionRequestFencesReviewDecisionAndSingleExecutionClaim(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 1, 0, 0, 0, time.UTC)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "/workspace", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Download the report", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
		Provider: "openrouter", Call: ConversationToolCallInput{
			ProviderRound: 0, OutputIndex: 0, ProviderCallID: "call-1", ProviderName: "download",
			Name: "file.download", Arguments: json.RawMessage(`{"url":"https://example.net/report.pdf","path":"report.pdf"}`),
		},
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{
		ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: items[len(items)-1].ID,
		OwnerHumanID: "human:local", RequestingAgentID: "agent:primary",
		CapabilityName: "file.download", OperationToken: "file.download", ReviewRoute: ActionLLMReview,
		Behavior: ActionBehavior{OpenWorld: true}, Arguments: json.RawMessage(`{"path":"report.pdf","url":"https://example.net/report.pdf"}`),
		InputSchema: json.RawMessage(`{"type":"object"}`), AuthorizationContext: map[string]any{"source": "human"},
		SafeSummary: "Download the report",
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	action, approval, err := database.RecordActionAssessment(ctx, action.ID, 1, ActionAssessment{
		Status: "completed", Authorization: "absent", Risk: "medium",
		ReviewerSelection: map[string]any{"model": "reviewer"}, ReasonCodes: []string{"authorization_absent"},
		Explanation: "The request does not authorize this destination.",
	}, now)
	if err != nil || action.State != ActionAwaitingApproval || approval == nil {
		t.Fatalf("reviewed action = %#v, approval = %#v, %v", action, approval, err)
	}
	if _, err := database.ClaimActionRequest(ctx, action.ID, 1, now); err == nil {
		t.Fatal("unapproved action was claimed")
	}
	if _, err := database.DecideActionRequest(ctx, action.ID, 2, "human:local", "approve", now); err == nil {
		t.Fatal("stale action revision was approved")
	}
	action, err = database.DecideActionRequest(ctx, action.ID, 1, "human:local", "approve", now)
	if err != nil || action.State != ActionExecutable {
		t.Fatalf("approved action = %#v, %v", action, err)
	}
	action, err = database.ClaimActionRequest(ctx, action.ID, 1, now)
	if err != nil || action.State != ActionExecuting {
		t.Fatalf("claimed action = %#v, %v", action, err)
	}
	if _, err := database.ClaimActionRequest(ctx, action.ID, 1, now); err == nil {
		t.Fatal("action received a second execution claim")
	}
	recovered, err := database.RecoverActionRequests(ctx, now.Add(time.Second))
	if err != nil || len(recovered) != 1 || recovered[0].ID != action.ID ||
		recovered[0].State != ActionOutcomeUncertain || recovered[0].FailureCode != "outcome_uncertain" {
		t.Fatalf("recovered actions = %#v, %v", recovered, err)
	}
}

func TestTaskCancellationPreservesUncertainActionOutcome(t *testing.T) {
	database := openTestStore(t)
	ctx, now := t.Context(), time.Date(2026, 9, 6, 0, 0, 0, 0, time.UTC)
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	id, _ := NewTaskID()
	_, err := database.CreateTaskWithOptions(ctx, id, "External cancellation", testTaskLifecycleCommand("create_task", "external"), TaskCreateOptions{ExecutorAgentID: TaskExecutorAgentID, InitialRunKind: "executor", ExecutionComplexity: "simple"}, now)
	if err != nil {
		t.Fatal(err)
	}
	_, run, found, err := database.ClaimTaskExecution(ctx, now)
	if err != nil || !found {
		t.Fatalf("claim = %t, %v", found, err)
	}
	if err = database.StartTaskExecution(ctx, run.ID, run.Generation, now); err != nil {
		t.Fatal(err)
	}
	if err = database.AppendTaskRunItems(ctx, run.ID, run.Generation, []TaskRunItemInput{{Kind: "tool_call", Status: "running", Payload: map[string]any{"name": "audit.write", "arguments": map[string]any{"value": "café 日本語"}}}}, TaskRunUsage{}, now); err != nil {
		t.Fatal(err)
	}
	items, err := database.TaskRunReplayItems(ctx, run.ID)
	if err != nil || len(items) == 0 {
		t.Fatalf("call = %#v, %v", items, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{TaskID: id, RunID: run.ID, RunItemID: items[len(items)-1].ID, TaskGeneration: run.Generation, OwnerHumanID: "human:local", RequestingAgentID: run.AgentID, CapabilityName: "audit.write", OperationToken: "audit.write", ReviewRoute: ActionHumanReview, Behavior: ActionBehavior{OpenWorld: true}, Arguments: json.RawMessage(`{"value":"café 日本語"}`), InputSchema: json.RawMessage(`{"type":"object"}`), AuthorizationContext: map[string]any{"source": "audit"}, SafeSummary: "Write the audit value"}, now)
	if err != nil {
		t.Fatal(err)
	}
	action, err = database.DecideActionRequest(ctx, action.ID, action.Revision, "human:local", "approve", now)
	if err != nil {
		t.Fatal(err)
	}
	action, err = database.ClaimActionRequest(ctx, action.ID, action.Revision, now)
	if err != nil || action.State != ActionExecuting {
		t.Fatalf("action claim = %#v, %v", action, err)
	}
	task, err := database.Task(ctx, id)
	if err != nil {
		t.Fatal(err)
	}
	cancelled, err := database.CancelTask(ctx, id, task.Revision, task.Generation, "Stop", testTaskLifecycleCommand("cancel_task", "external"), now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	after, err := database.ActionRequest(ctx, action.ID, action.Revision)
	if err != nil || after.State != ActionOutcomeUncertain {
		t.Fatalf("cancelled external outcome = %#v, %v", after, err)
	}
	if after.Arguments["value"] != "café 日本語" {
		t.Fatalf("cancellation changed exact arguments: %#v", after.Arguments)
	}
	if _, err = database.FinishActionRequest(ctx, action.ID, action.Revision, ActionSucceeded, json.RawMessage(`{"ok":true}`), "", now.Add(2*time.Second)); err == nil {
		t.Fatal("late success replaced the uncertain outcome")
	}
	if _, err = database.ClaimActionRequest(ctx, action.ID, action.Revision, now.Add(2*time.Second)); err == nil {
		t.Fatal("cancelled action was claimed again")
	}
	if _, err = database.RecoverTaskActionRequests(ctx, now.Add(3*time.Second)); err != nil {
		t.Fatal(err)
	}
	after, err = database.ActionRequest(ctx, action.ID, action.Revision)
	if err != nil || after.State != ActionOutcomeUncertain {
		t.Fatalf("recovery lost uncertainty: %#v, %v", after, err)
	}
	task, err = database.Task(ctx, id)
	if err != nil || task.StageKey != "cancelled" || task.Generation != cancelled.Task.Generation {
		t.Fatalf("late outcome changed cancellation: %#v, %v", task, err)
	}
}
