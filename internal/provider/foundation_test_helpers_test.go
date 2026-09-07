package provider

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

// foundationHealthyBridgeConfig points the production Foundation process at
// the same line protocol used by the Rust healthy bridge fixture. The fixture
// is an executable process; the provider and session code remain production
// authorities throughout these tests.
func foundationHealthyBridgeConfig(t *testing.T, extraCases string) foundationBridgeConfig {
	t.Helper()
	directory := t.TempDir()
	path := filepath.Join(directory, "bridge")
	script := fmt.Sprintf(`#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":3}}' ;;
    *'"id":"health"'*) printf '%%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    %s
    *) printf '%%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
`, extraCases)
	if err := os.WriteFile(path, []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	return foundationBridgeConfig{BridgePath: path}
}

// foundationUnavailableBridgeConfig starts the production Foundation process
// and lets its health response report the Rust unavailable state. This keeps
// the failure at the provider health boundary instead of at executable lookup.
func foundationUnavailableBridgeConfig(t *testing.T) foundationBridgeConfig {
	t.Helper()
	directory := t.TempDir()
	path := filepath.Join(directory, "bridge")
	script := `#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":3}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":false,"profiles":[],"unavailable_reason":"Foundation Models runtime is unavailable."}}' ;;
  esac
done
`
	if err := os.WriteFile(path, []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	return foundationBridgeConfig{BridgePath: path}
}

func foundationHealthyProvider(t *testing.T, extraCases string) *foundationProvider {
	t.Helper()
	provider := newFoundationProvider(foundationProviderConfig{
		DefaultProfile: "default",
		BridgePath:     foundationHealthyBridgeConfig(t, extraCases).BridgePath,
	})
	t.Cleanup(func() {
		provider.mu.Lock()
		process := provider.process
		provider.mu.Unlock()
		process.close()
	})
	return provider
}
