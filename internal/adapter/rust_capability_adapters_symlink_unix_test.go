//go:build aix || android || darwin || dragonfly || freebsd || hurd || illumos || ios || linux || netbsd || openbsd || solaris

package adapter

import (
	"os"
	"path/filepath"
	"testing"
)

func assertSymlinkedDefinition(t *testing.T, authority *fileAuthority, manifestPath, digest string) {
	t.Helper()
	if err := os.Remove(manifestPath); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(filepath.Join(filepath.Dir(manifestPath), "other.json"), manifestPath); err != nil {
		t.Fatal(err)
	}
	if _, err := authority.loadDefinition(digest); err == nil {
		t.Fatal("symlinked manifest was accepted")
	}
	scan, err := authority.scanDefinitions()
	if err != nil || len(scan.Definitions) != 0 || len(scan.Diagnostics) != 1 || scan.Diagnostics[0].Code != "object_file" {
		t.Fatalf("symlinked definition scan = %#v diagnostics=%#v, %v", scan.Definitions, scan.Diagnostics, err)
	}
}
