package runtime

import (
	"encoding/json"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	"os"
	"strings"
	"testing"
)

// These references were extracted from Rust commit 4d29f6ba1f70a30b5959a0e460217feeeb5e8c04.
// They preserve the full output, including whitespace; they are not generated from Go.
func TestAuxiliaryPromptsMatchRustExactly(t *testing.T) {
	read := func(name string) string {
		t.Helper()
		b, err := os.ReadFile("testdata/rust_aux_prompts/" + name + ".txt")
		if err != nil {
			t.Fatal(err)
		}
		return string(b)
	}
	memory := strings.NewReplacer("{canonical}", "[]", "{correction}", "\nYour previous native tool call was rejected: bad citation. Correct that failure in the replacement tool call.").Replace(read("memory"))
	final := strings.NewReplacer("{reason}", "background task handoff completed", "{PRIMARY_USER_FACING_FILE_POLICY}", primaryUserFacingFilePolicy).Replace(read("finalization"))
	for _, tc := range []struct{ name, got, want string }{
		{"action reviewer", actionReviewerPrompt, read("action_reviewer")},
		{"progress audit", progressAuditPrompt, read("progress_audit")},
		{"memory", memoryUpdateInstructions("[]", "bad citation"), memory},
		{"finalization", toolFinalizationInstruction("background task handoff completed"), final},
	} {
		t.Run(tc.name, func(t *testing.T) {
			if tc.got != tc.want {
				t.Fatalf("prompt differs from Rust: got %d bytes, want %d", len(tc.got), len(tc.want))
			}
		})
	}
}

func TestNotificationPromptMatchesRustExactly(t *testing.T) {
	chat, _, conversation := chatFixture(t)
	raw, err := os.ReadFile("testdata/rust_aux_prompts/capability_notification.txt")
	if err != nil {
		t.Fatal(err)
	}
	want := string(raw)
	for _, value := range []string{"API", "Example", "connection:example", `["read"]`, "1"} {
		want = strings.Replace(want, "{}", value, 1)
	}
	_, got, _, err := chat.primaryNotification(conversation, store.WorkEvent{Kind: "capability.ready", Payload: map[string]any{"integration_kind": "api", "integration_name": "Example", "connection_id": "connection:example", "connection_revision": "1", "granted_scopes": []string{"read"}, "enabled_tool_count": float64(1)}})
	if err != nil {
		t.Fatal(err)
	}
	if got != want {
		t.Fatalf("capability notification differs from Rust:\n%s", got)
	}
	taskID, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err = home.CreatePendingTaskDocument(chat.home, taskID, "Do the work."); err != nil {
		t.Fatal(err)
	}
	if err = home.CommitTaskDocument(chat.home, taskID); err != nil {
		t.Fatal(err)
	}
	document, err := home.ReadTaskDocument(chat.home, taskID)
	if err != nil {
		t.Fatal(err)
	}
	raw, err = os.ReadFile("testdata/rust_aux_prompts/task_notification.txt")
	if err != nil {
		t.Fatal(err)
	}
	want = string(raw)
	for _, value := range []string{"task_completed", taskID, "Example", document.Content, "Done"} {
		want = strings.Replace(want, "{}", value, 1)
	}
	want += "The background task completed successfully. Tell the human what was delivered and point them to useful artifacts when appropriate.\nCurrent Task notes:\n" + document.Content + "\n"
	got, err = chat.taskNotificationPrompt(store.WorkEvent{Kind: "task.completed"}, store.Task{ID: taskID, Title: "Example", StageKey: "done"})
	if err != nil {
		t.Fatal(err)
	}
	if got != want {
		t.Fatalf("task notification differs from Rust:\n%s", got)
	}
}

func TestActionReviewerRustInputContext(t *testing.T) {
	var input map[string]any
	if err := json.Unmarshal(actionReviewInput(store.ActionRequest{CapabilityName: "web.browse.open", Arguments: map[string]any{"url": "https://example.com/é"}}), &input); err != nil {
		t.Fatal(err)
	}
	shape, _ := json.Marshal(input["argument_projection"])
	if string(shape) != `{"field_count":1,"fields":{"url":{"length":22,"type":"string"}},"truncated":false,"type":"object"}` {
		t.Fatalf("Rust argument shape differs: %s", shape)
	}
	for _, number := range []any{1, json.Number("1"), float64(1)} {
		var numeric map[string]any
		if err := json.Unmarshal(actionReviewInput(store.ActionRequest{Arguments: map[string]any{"count": number}}), &numeric); err != nil {
			t.Fatal(err)
		}
		shape, _ := json.Marshal(numeric["argument_projection"])
		if string(shape) != `{"field_count":1,"fields":{"count":{"type":"number"}},"truncated":false,"type":"object"}` {
			t.Fatalf("numeric Rust argument shape differs: %s", shape)
		}
	}

	verified, _ := json.Marshal(input["verified_context"])
	if string(verified) != `{"browser_session":{"cookies_and_storage_destroyed_on_session_end":true,"durable_profile":false,"owner_scope":"conversation_or_task_generation","storage_lifetime":"session_only"}}` {
		t.Fatalf("Rust browser context differs: %s", verified)
	}
	if input["arguments"].(map[string]any)["url"] != "https://example.com/é" {
		t.Fatal("exact arguments changed")
	}
}
