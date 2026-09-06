package runtime

import (
	"encoding/json"
	"testing"
	"time"
)

func TestAuditExecutorDownloadCapability(t *testing.T) {
	chat, db, _ := chatFixture(t)
	ctx := t.Context()
	now := time.Now()
	task := createQueuedRuntimeTask(t, db, chat.home, "Download the requested public report.")
	_, planner, found, err := db.ClaimTaskExecution(ctx, now)
	if err != nil || !found {
		t.Fatal("planner unavailable", err)
	}
	if err = db.StartTaskExecution(ctx, planner.ID, planner.Generation, now); err != nil {
		t.Fatal(err)
	}
	if err = db.FinishTaskPlanning(ctx, planner.ID, planner.Generation, "simple", now); err != nil {
		t.Fatal(err)
	}
	task, executor, found, err := db.ClaimTaskExecution(ctx, now)
	if err != nil || !found {
		t.Fatal("executor unavailable", err)
	}
	present := false
	for _, tool := range taskExecutionTools("executor") {
		if tool.Name == fileDownloadName {
			present = true
		}
	}
	if !present || !taskToolAllowed("executor", fileDownloadName) {
		t.Fatal("Executor catalog or dispatcher omits file.download")
	}
	runtime := &TaskExecution{database: db, root: chat.home}
	payload, success, _, _ := runtime.executeTaskTool(ctx, task, executor, fileDownloadName, json.RawMessage(`{"url":"invalid","path":"report.txt"}`), true)
	if success || string(payload) == "" || string(payload) == `{"code":"unsupported_tool","message":"Tool is unavailable for this Task role"}` {
		t.Fatalf("file.download did not reach its dispatcher: success=%t payload=%s", success, payload)
	}
}
