package graphql

import (
	"github.com/kpsuperplane/noema/internal/store"
	"testing"
)

func TestTaskConnectedToolDisplayUsesRustName(t *testing.T) {
	const name = "synthetic_expense_api_personal-d4d0e0fd.submit_reimbursement"
	for _, kind := range []string{"tool_call", "tool_result"} {
		payload := map[string]any{"name": name, "display": map[string]any{"description": "Submit the approved expense"}}
		item := taskRunItemModel(store.TaskRunItem{Kind: kind, Payload: payload})
		if item.Payload["name"] != name || item.Payload["display"].(map[string]any)["name"] != "Submit reimbursement" {
			t.Fatalf("wrong display: %#v", item.Payload)
		}
		if _, changed := payload["display"].(map[string]any)["name"]; changed {
			t.Fatal("display projection changed stored payload")
		}
	}
}
