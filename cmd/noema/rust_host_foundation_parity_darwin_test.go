//go:build darwin

package main

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
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
	homeRoot := filepath.Join(root, "home")
	paths, err := home.FromRoot(homeRoot)
	if err != nil {
		t.Fatal(err)
	}
	config := fmt.Sprintf("provider: foundation_local\nfoundation_local:\n  default_profile: foundation-live\n  bridge_path: %s\nweb:\n  local_graphql_socket: false\n", bridgePath)
	if err := home.AtomicWritePrivate(paths.Config(), []byte(config)); err != nil {
		t.Fatal(err)
	}
	t.Setenv(home.EnvironmentName, homeRoot)
	t.Setenv("NOEMA_WEB__LOCAL_GRAPHQL_SOCKET", "false")
	if err := runHostForRustTest(t, nil); err != nil {
		t.Fatalf("start configured Foundation host: %v", err)
	}
	database, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	preference, err := database.DefaultModelPreference(context.Background())
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
}
