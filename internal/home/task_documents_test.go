package home

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestTaskDocumentStageReconciliationDiscardsStaleCommit(t *testing.T) {
	path := filepath.Join(t.TempDir(), "home")
	if err := os.Mkdir(path, 0o700); err != nil {
		t.Fatal(err)
	}
	paths, err := FromRoot(path)
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	taskID := "task:00000000000000000000000000000001"
	if _, err := CreatePendingTaskDocument(root, taskID, "First"); err != nil {
		t.Fatal(err)
	}
	if err := CommitTaskDocument(root, taskID); err != nil {
		t.Fatal(err)
	}
	first, _ := ReadTaskDocument(root, taskID)
	request := strings.Repeat("a", 64)
	stale, err := PrepareTaskDocumentReplace(root, taskID, first.Digest, "Second", request)
	if err != nil {
		t.Fatal(err)
	}
	newer, err := PrepareTaskDocumentReplace(root, taskID, first.Digest, "Third", strings.Repeat("b", 64))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := CommitTaskDocumentStage(root, newer); err != nil {
		t.Fatal(err)
	}
	if err := root.MkdirAll("tasks/task_00000000000000000000000000000001/artifacts", 0o700); err != nil {
		t.Fatal(err)
	}
	if err := ReconcileTaskDocumentStages(root, 1, func(id, digest string) (string, bool, error) {
		return stale.Document.Digest, id == taskID && digest == request, nil
	}); err != nil {
		t.Fatal(err)
	}
	current, err := ReadTaskDocument(root, taskID)
	if err != nil || current.Content != "Third" {
		t.Fatalf("current document = %#v, %v", current, err)
	}
	task, err := openTaskDocumentRoot(root, taskID)
	if err != nil {
		t.Fatal(err)
	}
	defer task.Close()
	for _, name := range []string{taskDocumentStageName(request), taskDocumentStageBaseName(request)} {
		if _, err := task.Stat(name); !errors.Is(err, os.ErrNotExist) {
			t.Fatalf("stale stage %s remains: %v", name, err)
		}
	}
}
