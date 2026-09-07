//go:build darwin

package main

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/kpsuperplane/noema/internal/foundation"
	"github.com/kpsuperplane/noema/internal/home"
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
	paths, err := home.FromRoot(filepath.Join(root, "home"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.Initialize(paths, []byte(home.DefaultConfigYAML)); err != nil {
		t.Fatal(err)
	}
	host, err := foundation.AssembleHost(context.Background(), paths, bridgePath, "foundation-live")
	if err != nil {
		t.Fatalf("start configured Foundation host: %v", err)
	}
	shutdown := false
	t.Cleanup(func() {
		if !shutdown {
			_ = host.Shutdown()
		}
	})
	preference, err := host.Store.DefaultModelPreference(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if preference == nil {
		t.Fatal("Foundation default preference was not persisted")
	}
	if preference.ProviderKind != "foundation_local" {
		t.Fatalf("Foundation default provider = %q", preference.ProviderKind)
	}
	if preference.ModelProfile != "foundation-live" {
		t.Fatalf("Foundation default profile = %q", preference.ModelProfile)
	}
	if err := host.Shutdown(); err != nil {
		t.Fatal(err)
	}
	shutdown = true
}
