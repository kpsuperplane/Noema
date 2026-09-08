package runtime

import (
	"encoding/json"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	"os"
	"testing"
)

// These outputs were evaluated from Rust 4d29f6ba's task_run_context.rs and
// runtime/task_continuation.rs. Expected text does not use Go prompt builders.
func TestTaskPromptsMatchCompleteRustOutputs(t *testing.T) {
	raw, err := os.ReadFile("testdata/rust_prompts/task_roles.json")
	if err != nil {
		t.Fatal(err)
	}
	var expected map[string]struct{ Input, Instructions, Finalization string }
	if err = json.Unmarshal(raw, &expected); err != nil {
		t.Fatal(err)
	}
	for kind, reference := range expected {
		t.Run(kind, func(t *testing.T) {
			task := store.Task{ID: "task:reference", Title: "Exact task"}
			input := formatTaskRolePrompt(kind, task, "Human source request", "Captured with the source request: date=2026-08-12, time=12:00:00, timezone=UTC. Use these values only to interpret relative terms in that request. They are not the current run clock.", "Project snapshot: Project café — 日本語\n")
			if input != reference.Input {
				t.Errorf("role input differs from frozen Rust output\ngot: %q\nwant: %q", input, reference.Input)
			}
			if actual := taskRoleInstructions(kind); actual != reference.Instructions {
				t.Errorf("instructions differ: %q", actual)
			}
			if actual := taskFinalizationPrompt(kind, "ceiling", "original input"); actual != reference.Finalization {
				t.Errorf("finalization differs: %q", actual)
			}
		})
	}
}

func TestTaskToolCatalogMatchesRustServiceRows(t *testing.T) {
	tools := []provider.GenerationTool{
		{Name: "task.files.read", Description: "Read a file"},
		{Name: "mail.read", Description: "Read mail", ServiceCatalogRow: "- service\tconnection:mail\tname=\"Mail\"", ServiceConnectionID: "connection:mail"},
		{Name: "mail.search", Description: "Search mail", ServiceCatalogRow: "- service\tconnection:mail\tname=\"Mail\"", ServiceConnectionID: "connection:mail"},
	}
	bindings := map[string]noemamcp.Binding{"mail.read": {}, "mail.search": {}}
	expected := "- service\tconnection:mail\tname=\"Mail\"\n- builtin\ttask.files.read\tRead a file\n- capability\tmail.read\tservice=connection:mail\tRead mail\n- capability\tmail.search\tservice=connection:mail\tSearch mail"
	if actual := taskToolPromptRows(tools, bindings, nil); actual != expected {
		t.Fatalf("catalog differs from Rust: %q", actual)
	}
}
