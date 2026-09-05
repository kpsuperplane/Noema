package home

import (
	"os"
	"path/filepath"
	"testing"
)

func TestTaskFilesStayRootedAndProtectAuthorityDocuments(t *testing.T) {
	path := t.TempDir()
	root, err := os.OpenRoot(path)
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	taskID := "task:00000000000000000000000000000001"
	if _, err := CreatePendingTaskDocument(root, taskID, "# Task\n"); err != nil {
		t.Fatal(err)
	}
	if err := CommitTaskDocument(root, taskID); err != nil {
		t.Fatal(err)
	}
	if err := WriteTaskFile(root, taskID, "notes/progress.md", "done\n"); err != nil {
		t.Fatal(err)
	}
	content, err := ReadTaskFile(root, taskID, "notes/progress.md")
	if err != nil || content != "done\n" {
		t.Fatalf("read = %q, %v", content, err)
	}
	entries, err := ListTaskFiles(root, taskID, "notes")
	if err != nil || len(entries) != 1 || entries[0].Path != "notes/progress.md" {
		t.Fatalf("entries = %#v, %v", entries, err)
	}
	if err := WriteTaskFile(root, taskID, "../outside", "bad"); err == nil {
		t.Fatal("parent traversal succeeded")
	}
	if err := DeleteTaskFile(root, taskID, "TASK.md"); err == nil {
		t.Fatal("TASK.md deletion succeeded")
	}
	name, _ := taskName(taskID)
	if err := os.Symlink(path, filepath.Join(path, taskRootName, name, "link")); err != nil {
		t.Fatal(err)
	}
	if _, err := ReadTaskFile(root, taskID, "link/anything"); err == nil {
		t.Fatal("symbolic-link read succeeded")
	}
}
