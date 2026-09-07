//go:build aix || android || darwin || dragonfly || freebsd || illumos || ios || linux || netbsd || openbsd || solaris

package provider

import (
	"os"
	"path/filepath"
	"testing"
)

// Rust source: crates/noema-providers/src/adapters/codex/oauth/token_store.rs::written_tokens_have_private_unix_permissions (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_WrittenTokensHavePrivateUnixPermissions(t *testing.T) {
	path := filepath.Join(t.TempDir(), "providers", "codex", "default", "codex_tokens.json")
	if err := writeCodexTokens(path, CodexTokens{accessToken: "access", refreshToken: "refresh"}); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("Codex token mode = %v, %v", info, err)
	}
}
