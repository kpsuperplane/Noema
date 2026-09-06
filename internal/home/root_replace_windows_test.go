//go:build windows

package home

import (
	"os"
	"testing"
)

func TestRootedWindowsReplacementAndDurabilityContract(t *testing.T) {
	root, err := os.OpenRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	if err := root.WriteFile("page.md", []byte("old"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := root.WriteFile("temporary.md", []byte("new"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := ReplaceRootFile(root, "temporary.md", "page.md"); err != nil {
		t.Fatal(err)
	}
	content, err := root.ReadFile("page.md")
	if err != nil || string(content) != "new" {
		t.Fatalf("replacement content = %q, %v", content, err)
	}
	if err := SyncRootDirectory(root, "."); err != nil {
		t.Fatal(err)
	}
}
