package main

import (
	"context"
	"io"
	"net"
	"os"
	"path/filepath"
	"strings"
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
	if !strings.Contains(string(config), "provider: codex") || !strings.Contains(string(config), "browser:") {
		t.Fatalf("first-run config = %q", config)
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
		if account.ProviderKind == "codex" || account.ProviderKind == "openai" || account.ProviderKind == "openrouter" || account.ProviderKind == "local_models" {
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
		if account.ProviderKind == "local_models" {
			t.Fatalf("unresolvable local default fabricated an account: %#v", account)
		}
	}
}

// Rust source: crates/noema-host/src/composition/tests.rs:31::startup_entrypoint_child
func TestRustHost_startup_entrypoint_child(t *testing.T) {
	token := strings.Repeat("a", 43)
	runtimeRoot := filepath.Join(t.TempDir(), "runtime")
	options, reader := readDesktopOptions(strings.NewReader(`{"token":"` + token + `","runtimeRoot":"` + runtimeRoot + `"}` + "\nshutdown"))
	if options == nil || options.Token != token || options.RuntimeRoot != runtimeRoot {
		t.Fatalf("desktop startup options = %#v", options)
	}
	remainder, err := io.ReadAll(reader)
	if err != nil || string(remainder) != "shutdown" {
		t.Fatalf("startup child remainder = %q, %v", remainder, err)
	}
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
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	return run(ctx, address, io.Discard, nil)
}
