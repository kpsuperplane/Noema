package main

import (
	"context"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

// Rust source: crates/noema-host/src/composition/tests.rs:3::process_env_entrypoint_initializes_and_loads_first_run_home
func TestRustHost_process_env_entrypoint_initializes_and_loads_first_run_home(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv(home.EnvironmentName, paths.Root())
	t.Setenv("NOEMA_WEB__LOCAL_GRAPHQL_SOCKET", "false")
	if err := runHostForRustTest(t); err != nil {
		t.Fatalf("startup = %v", err)
	}
	config, err := os.ReadFile(paths.Config())
	if err != nil {
		t.Fatal(err)
	}
	if string(config) != rustHostDefaultConfigYAML {
		t.Fatalf("first-run config = %q, want %q", config, rustHostDefaultConfigYAML)
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
	if preference != nil {
		t.Fatalf("first-run default model = %#v", preference)
	}
	accounts, err := database.ActiveProviderAccounts(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	for _, account := range accounts {
		if isRustHostModelProvider(account.ProviderKind) {
			t.Fatalf("first-run model account = %#v", account)
		}
	}
}

// Rust source: crates/noema-host/src/composition/tests.rs:8::loaded_config_entrypoint_preserves_existing_config_bytes
func TestRustHost_loaded_config_entrypoint_preserves_existing_config_bytes(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	existing := []byte("provider: deliberately-not-loaded\n")
	if err := home.AtomicWritePrivate(paths.Config(), existing); err != nil {
		t.Fatal(err)
	}
	t.Setenv(home.EnvironmentName, paths.Root())
	t.Setenv("NOEMA_WEB__LOCAL_GRAPHQL_SOCKET", "false")
	if err := runHostForRustTest(t); err != nil {
		t.Fatalf("startup = %v", err)
	}
	config, err := os.ReadFile(paths.Config())
	if err != nil {
		t.Fatal(err)
	}
	if string(config) != string(existing) {
		t.Fatalf("existing config changed from %q to %q", existing, config)
	}
}

// Rust source: crates/noema-host/src/composition/tests.rs:102::fresh_unresolvable_local_default_starts_onboarding_without_an_account
func TestRustHost_fresh_unresolvable_local_default_starts_onboarding_without_an_account(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(paths.Config(), []byte("provider: local_models\nweb:\n  local_graphql_socket: false\n")); err != nil {
		t.Fatal(err)
	}
	t.Setenv(home.EnvironmentName, paths.Root())
	t.Setenv("NOEMA_WEB__LOCAL_GRAPHQL_SOCKET", "false")
	if err := runHostForRustTest(t); err != nil {
		t.Fatalf("startup = %v", err)
	}
	database, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	accounts, err := database.ActiveProviderAccounts(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	for _, account := range accounts {
		if isRustHostModelProvider(account.ProviderKind) {
			t.Fatalf("unresolvable local default fabricated an account: %#v", account)
		}
	}
}

// Rust source: crates/noema-host/src/composition/tests.rs:31::startup_entrypoint_child
func TestRustHost_startup_entrypoint_child(t *testing.T) {
	t.Fatal("unsupported port: Go server has no isolated startup test child entrypoint")
}

func runHostForRustTest(t *testing.T) error {
	t.Helper()
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return err
	}
	address := listener.Addr().String()
	if err := listener.Close(); err != nil {
		return err
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	output := &rustHostReadyWriter{ready: make(chan struct{})}
	result := make(chan error, 1)
	go func() { result <- run(ctx, address, output, nil) }()
	select {
	case <-output.ready:
		cancel()
	case <-time.After(10 * time.Second):
		cancel()
		return fmt.Errorf("host did not report readiness")
	}
	select {
	case err := <-result:
		return err
	case <-time.After(10 * time.Second):
		return fmt.Errorf("host did not stop after cancellation")
	}
}

type rustHostReadyWriter struct {
	ready chan struct{}
	once  sync.Once
}

func (w *rustHostReadyWriter) Write(value []byte) (int, error) {
	if strings.Contains(string(value), "Noema listening on ") {
		w.once.Do(func() { close(w.ready) })
	}
	return len(value), nil
}

func isRustHostModelProvider(providerKind string) bool {
	switch providerKind {
	case "codex", "openai", "foundation_local", "local_models", "openrouter":
		return true
	default:
		return false
	}
}

const rustHostDefaultConfigYAML = `# Noema configuration
provider: codex

codex:
  base_url: https://chatgpt.com/backend-api/codex
  # Explicit model overrides must also set reasoning_effort when supported.
  # model: <model-id>
  # reasoning_effort: medium
  # tool_classification_model defaults to gpt-5.4-mini when unset.
  # tool_classification_model: gpt-5.4-mini
  timeout_seconds: 300

# The daemon opens the embedded Noema store under this home directory.

browser:
  max_sessions: 2
  max_old_space_mb: 1024

web:
  host: 127.0.0.1
  port: 3737
  rp_id: localhost
  dev_no_auth: false
  local_graphql_socket: false
  graphiql: false
  # Set this to the exact HTTPS origin exposed by your reverse proxy.
  # For deployment, set both values to the stable public domain.
  # rp_id: noema.example.com
  # public_origin: https://noema.example.com

mcp:
  stdio_enabled: false
`

// Rust source: crates/noema-host/src/composition/tests.rs:157::configured_foundation_bridge_establishes_default_readiness
func TestRustHost_configured_foundation_bridge_establishes_default_readiness(t *testing.T) {
	if runtime.GOOS != "darwin" {
		t.Skip("Rust source is cfg(target_os = \"macos\")")
	}
	t.Fatal("unsupported port: Go host has no Foundation Local bridge composition authority")
}

// Rust source: crates/noema-host/src/composition/tests.rs:211::runtime_host_error_messages_are_plain_language
func TestRustHost_runtime_host_error_messages_are_plain_language(t *testing.T) {
	t.Fatal("unsupported port: Go host has no RuntimeHostError user-message authority")
}
