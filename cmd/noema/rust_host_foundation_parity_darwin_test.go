//go:build darwin

package main

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/kpsuperplane/noema/internal/foundation"
)

// Rust source: crates/noema-host/src/composition/tests.rs:157::configured_foundation_bridge_establishes_default_readiness
func TestRustHost_configured_foundation_bridge_establishes_default_readiness(t *testing.T) {
	root := t.TempDir()
	bridgePath := filepath.Join(root, "foundation-bridge")
	bridge := `#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":3}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"foundation-live","label":"Foundation Live"}],"unavailable_reason":null}}' ;;
  esac
done
`
	if err := os.WriteFile(bridgePath, []byte(bridge), 0o700); err != nil {
		t.Fatal(err)
	}
	selection, err := foundation.ResolveDefaultSelection(context.Background(), bridgePath, "foundation-live")
	if err != nil {
		t.Fatal(err)
	}
	if selection.ProviderKind != "foundation_local" {
		t.Fatalf("Foundation default provider = %q", selection.ProviderKind)
	}
	if selection.ModelProfile != "foundation-live" {
		t.Fatalf("Foundation default profile = %q", selection.ModelProfile)
	}
}
