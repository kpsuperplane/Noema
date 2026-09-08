package runtime

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/project"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestAuditProjectTaskSharedReadsAndFreshContext(t *testing.T) {
	chat, db, _ := chatFixture(t)
	ctx := t.Context()
	folder := filepath.Join(chat.home.Name(), "tasks")
	if err := os.MkdirAll(folder, 0700); err != nil {
		t.Fatal(err)
	}
	document := "# Project\nInitial café 日本語\n"
	created, err := chat.projects.Create(ctx, project.CreateInput{WorkspaceID: "workspace:personal", Name: "Shared Project", Folder: &folder, Document: &document, Command: project.Command{ActorID: "actor:human:local", RequestID: "audit-project", CorrelationID: "correlation:audit-project"}})
	if err != nil {
		t.Fatal(err)
	}
	id, _ := store.NewTaskID()
	if _, err = home.CreatePendingTaskDocument(chat.home, id, "# Task\nRead shared context.\n"); err != nil {
		t.Fatal(err)
	}
	saved, err := db.CreateTaskWithOptions(ctx, id, "Project Task", runtimeTaskCommand("task.capture", "audit-project-task"), store.TaskCreateOptions{ProjectID: created.Project.ID, ExecutorAgentID: store.TaskExecutorAgentID}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if err = home.CommitTaskDocument(chat.home, id); err != nil {
		t.Fatal(err)
	}
	task := saved.Task
	const shared = "Shared café 日本語"
	sharedPath := filepath.Join(folder, "shared.md")
	if err = os.WriteFile(sharedPath, []byte(shared), 0600); err != nil {
		t.Fatal(err)
	}
	outside := filepath.Join(chat.home.Name(), "outside.md")
	if err = os.WriteFile(outside, []byte("Outside café 日本語"), 0600); err != nil {
		t.Fatal(err)
	}
	taskDir := filepath.Join(folder, strings.TrimPrefix(id, "task:"))
	if err = home.WriteTaskFile(chat.home, task.ID, "RESULT.md", "Current result"); err != nil {
		t.Fatal(err)
	}
	runtime := &TaskExecution{database: db, root: chat.home}
	for _, role := range []string{"planner", "executor", "reviewer"} {
		t.Run(role+" current Project context", func(t *testing.T) {
			current := fmt.Sprintf("# Project\nCurrent %s café 日本語\n", role)
			if err := os.WriteFile(filepath.Join(folder, "PROJECT.md"), []byte(current), 0600); err != nil {
				t.Fatal(err)
			}
			messages, _, err := runtime.taskMessages(ctx, task, store.TaskRun{Kind: role})
			if err != nil {
				t.Fatal(err)
			}
			found := false
			for _, message := range messages {
				if strings.Contains(message.Content, "<PROJECT_DOCUMENT>\n"+current+"\n</PROJECT_DOCUMENT>") {
					found = true
				}
			}
			if !found {
				t.Error("current Project text absent from role context")
			}
		})
		t.Run(role+" shared read", func(t *testing.T) {
			payload, success, _, _ := runtime.executeTaskTool(ctx, task, store.TaskRun{Kind: role}, taskFilesRead, json.RawMessage(`{"path":"../shared.md"}`), false)
			var result map[string]any
			_ = json.Unmarshal(payload, &result)
			if !success || result["content"] != shared {
				t.Errorf("permitted Project read success=%t payload=%s", success, payload)
			}
		})
		t.Run(role+" outside read", func(t *testing.T) {
			if err := os.Symlink(outside, filepath.Join(taskDir, "escape.md")); err != nil {
				t.Fatal(err)
			}
			defer os.Remove(filepath.Join(taskDir, "escape.md"))
			for _, path := range []string{"../../outside.md", "escape.md"} {
				raw, _ := json.Marshal(map[string]string{"path": path})
				payload, success, _, _ := runtime.executeTaskTool(ctx, task, store.TaskRun{Kind: role}, taskFilesRead, raw, false)
				if success || strings.Contains(string(payload), "Outside café 日本語") {
					t.Errorf("outside read %q succeeded: %s", path, payload)
				}
			}
		})
	}
	got, err := os.ReadFile(sharedPath)
	if err != nil || string(got) != shared {
		t.Errorf("shared source changed: %q, %v", got, err)
	}
}
