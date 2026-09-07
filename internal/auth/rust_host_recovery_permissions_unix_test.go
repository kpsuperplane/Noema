//go:build !windows

package auth

import (
	"os"
	"testing"
)

func assertRustHostRecoveryPermissions(t *testing.T, path string) {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("config permissions = %v, %v", info, err)
	}
}
