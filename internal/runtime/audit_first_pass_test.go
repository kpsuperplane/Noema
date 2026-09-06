package runtime

import (
	"encoding/json"
	"testing"

	"github.com/kpsuperplane/noema/internal/store"
)

func TestAuditTaskReadAnotherOwnerAuthorizedTask(t *testing.T) {
	chat, database, _ := chatFixture(t)
	source := createQueuedRuntimeTask(t, database, chat.home, "Source café 日本語")
	target := createQueuedRuntimeTask(t, database, chat.home, "Target café 日本語")
	runtime := &TaskExecution{database: database, root: chat.home}
	arguments, _ := json.Marshal(map[string]string{"task_id": target.ID})
	// Primary Chat verifies that this owner can read the target Task.
	if payload, allowed := chat.inspectTask(t.Context(), arguments); !allowed {
		t.Fatalf("owner read failed: %s", payload)
	}
	for _, role := range []string{"executor", "reviewer"} {
		payload, success, _, _ := runtime.executeTaskTool(t.Context(), source, store.TaskRun{Kind: role}, taskInspectName, arguments, false)
		var result map[string]any
		_ = json.Unmarshal(payload, &result)
		if !success || result["task_id"] != target.ID || result["task_document"] != "Target café 日本語" {
			t.Errorf("%s cross-Task read success=%t payload=%s", role, success, payload)
		}
	}
}
