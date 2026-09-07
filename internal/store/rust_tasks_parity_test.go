package store

import (
	"encoding/json"
	"testing"
)

// Rust source: crates/noema-tasks/src/event.rs:309::recurrence_reason_preserves_text_and_rejects_nested_fields
func TestRustTasks_recurrence_reason_preserves_text_and_rejects_nested_fields(t *testing.T) {
	reason := "provider token feedback description"
	payload, err := NewRecurrenceChangedPayload("recurrence:daily", 1, reason)
	if err != nil {
		t.Fatal(err)
	}
	if payload.Value()["reason"] != reason {
		t.Fatalf("recurrence reason = %#v, want %q", payload.Value()["reason"], reason)
	}

	_, err = ParseRecurrenceChangedPayload(
		map[string]any{
			"v":             1,
			"recurrence_id": "recurrence:daily",
			"revision":      1,
			"reason":        map[string]any{"token": "not a scalar reason"},
		})
	if err == nil {
		t.Fatal("nested recurrence reason was accepted")
	}
}

// Rust source: crates/noema-tasks/src/gate.rs:315::exhausted_review_rounds_accept_retry_guidance_only
func TestRustTasks_exhausted_review_rounds_accept_retry_guidance_only(t *testing.T) {
	reason := "review_rounds_exhausted"
	runKind := "executor"
	gate := TaskGate{Kind: "recovery", RecoveryReason: &reason, RetryRunKind: &runKind}
	if !gate.AllowsResolution("retry") {
		t.Fatal("review-round exhaustion did not allow retry")
	}
	if gate.AllowsResolution("answer") {
		t.Fatal("review-round exhaustion allowed answer")
	}
}

// Rust source: crates/noema-tasks/src/ids.rs:57::task_ids_reject_wrong_prefix_controls_and_oversize_values
func TestRustTasks_task_ids_reject_wrong_prefix_controls_and_oversize_values(t *testing.T) {
	if taskIDIsAccepted(t, "workflow:one") {
		t.Fatal("wrong-prefix Task ID was accepted")
	}
	if taskIDIsAccepted(t, " ") {
		t.Fatal("blank Task ID was accepted")
	}
	if taskIDIsAccepted(t, "task:bad\nvalue") {
		t.Fatal("control character in Task ID was accepted")
	}
	if taskIDIsAccepted(t, "task:"+repeatTaskID("x", 251)) {
		t.Fatal("oversize Task ID was accepted")
	}
	if !taskIDIsAccepted(t, "task:one") {
		t.Fatal("valid Task ID was rejected")
	}
	id, err := ParseTaskID("task:one")
	if err != nil {
		t.Fatal(err)
	}
	if got := id.String(); got != "task:one" {
		t.Fatalf("Task ID string = %q, want %q", got, "task:one")
	}
}

// Rust source: crates/noema-tasks/src/ids.rs:66::ids_serialize_as_opaque_wire_strings_and_fail_closed
func TestRustTasks_ids_serialize_as_opaque_wire_strings_and_fail_closed(t *testing.T) {
	id, err := ParseWorkflowID("workflow:personal:default")
	if err != nil {
		t.Fatal(err)
	}
	wire, err := json.Marshal(id)
	if err != nil {
		t.Fatal(err)
	}
	if got, want := string(wire), `"workflow:personal:default"`; got != want {
		t.Fatalf("serialized workflow ID = %s, want %s", got, want)
	}
	var decoded WorkflowID
	if err := json.Unmarshal([]byte(`"task:one"`), &decoded); err == nil {
		t.Fatal("wrong-prefix workflow ID was accepted")
	}
}

func taskIDIsAccepted(t *testing.T, id string) bool {
	t.Helper()
	_, err := ParseTaskID(id)
	return err == nil
}

func repeatTaskID(value string, count int) string {
	result := ""
	for range count {
		result += value
	}
	return result
}
