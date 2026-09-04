package home

import (
	"errors"
	"os"
	"testing"
)

func TestRecurrenceDocumentDigestCopyAndDelete(t *testing.T) {
	root, err := os.OpenRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	id := "recurrence:0123456789abcdef0123456789abcdef"
	document, err := EnsureRecurrenceDocument(root, id, "# Repeat\n")
	if err != nil {
		t.Fatal(err)
	}
	wrong := "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
	if _, err := WriteRecurrenceDocument(root, id, "changed", &wrong); !errors.Is(err, ErrRecurrenceDocumentChanged) {
		t.Fatalf("stale digest error = %v", err)
	}
	replaced, err := WriteRecurrenceDocument(root, id, "# Changed\n", &document.Digest)
	if err != nil || replaced.Digest == document.Digest {
		t.Fatalf("replacement = %#v, %v", replaced, err)
	}
	taskID := "task:0123456789abcdef0123456789abcdef"
	if err := CopyRecurrenceDocumentToTask(root, id, taskID); err != nil {
		t.Fatal(err)
	}
	copied, err := ReadTaskDocument(root, taskID)
	if err != nil || copied != replaced {
		t.Fatalf("copied = %#v, %v", copied, err)
	}
	if err := DeleteRecurrenceDocument(root, id); err != nil {
		t.Fatal(err)
	}
	if _, err := ReadRecurrenceDocument(root, id); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("deleted recurrence error = %v", err)
	}
}
