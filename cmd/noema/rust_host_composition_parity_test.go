package main

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

// Rust source: crates/noema-host/src/composition/tests.rs:3::process_env_entrypoint_initializes_and_loads_first_run_home
func TestRustHost_process_env_entrypoint_initializes_and_loads_first_run_home(t *testing.T) {
	runStartupEntryPointChild(t, "process_env")
}

// Rust source: crates/noema-host/src/composition/tests.rs:8::loaded_config_entrypoint_preserves_existing_config_bytes
func TestRustHost_loaded_config_entrypoint_preserves_existing_config_bytes(t *testing.T) {
	runStartupEntryPointChild(t, "loaded_config")
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
	providerConfig, err := auth.ResolveProviderConfig(paths, "")
	if err != nil {
		t.Fatal(err)
	}
	if providerConfig.Provider != "local_models" || providerConfig.Model != "default" {
		t.Fatalf("local model provider config = %#v", providerConfig)
	}
	if err := home.AtomicWritePrivate(paths.Config(), []byte("provider: local_models\nmodel: missing-local-model\nlocal_models:\n  default_model: missing-local-model\nweb:\n  local_graphql_socket: false\n")); err != nil {
		t.Fatal(err)
	}
	providerConfig, err = auth.ResolveProviderConfig(paths, "")
	if err != nil || providerConfig.Model != "missing-local-model" {
		t.Fatalf("missing local model provider config = %#v, %v", providerConfig, err)
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
	preference, err := database.DefaultModelPreference(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if preference != nil {
		t.Fatalf("unresolvable local default preference = %#v", preference)
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
	mode, ok := os.LookupEnv("NOEMA_HOST_STARTUP_TEST_MODE")
	if !ok {
		return
	}
	paths, err := home.Resolve()
	if err != nil {
		t.Fatal(err)
	}
	os.Setenv("NOEMA_WEB__LOCAL_GRAPHQL_SOCKET", "false")
	if mode == "process_env" {
		if err := runHostForRustTest(t); err != nil {
			t.Fatalf("process environment startup = %v", err)
		}
		config, err := os.ReadFile(paths.Config())
		if err != nil {
			t.Fatal(err)
		}
		if string(config) != home.DefaultConfigYAML {
			t.Fatalf("first-run config = %q, want %q", config, home.DefaultConfigYAML)
		}
		webConfig, _, err := auth.LoadConfig(paths, "")
		if err != nil {
			t.Fatal(err)
		}
		if webConfig.ListenAddress != "127.0.0.1:3737" || webConfig.Authority != "localhost:3737" ||
			webConfig.Origin != "http://localhost:3737" || webConfig.RPID != "localhost" || webConfig.Secure ||
			webConfig.DevNoAuth || webConfig.GraphiQL || webConfig.LocalGraphQLSocket ||
			webConfig.BrowserMaxSessions != 2 || webConfig.BrowserMaxOldSpaceMB != 1024 {
			t.Fatalf("first-run web config = %#v", webConfig)
		}
		database, err := store.Open(context.Background(), paths.Database())
		if err != nil {
			t.Fatal(err)
		}
		defer database.Close()
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
		return
	}
	if mode != "loaded_config" {
		t.Fatalf("unexpected startup test mode %q", mode)
	}
	existing := []byte("provider: deliberately-not-loaded\n")
	if err := home.AtomicWritePrivate(paths.Config(), existing); err != nil {
		t.Fatal(err)
	}
	loaded := auth.Config{
		Authority: "localhost:4848", Origin: "http://localhost:4848", RPID: "localhost",
		ListenAddress: "127.0.0.1:4848", BrowserMaxSessions: 2, BrowserMaxOldSpaceMB: 1024,
		Secure: false, DevNoAuth: false, GraphiQL: false, LocalGraphQLSocket: false,
	}
	if err := runLoadedHostForRustTest(t, loaded); err != nil {
		t.Fatalf("loaded configuration startup = %v", err)
	}
	config, err := os.ReadFile(paths.Config())
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(config, existing) {
		t.Fatalf("loaded configuration changed from %q to %q", existing, config)
	}
}

func runStartupEntryPointChild(t *testing.T, mode string) {
	t.Helper()
	homeRoot := filepath.Join(t.TempDir(), "home")
	command := exec.Command(os.Args[0], "-test.run", "^TestRustHost_startup_entrypoint_child$", "-test.v")
	command.Env = append(os.Environ(), home.EnvironmentName+"="+homeRoot, "NOEMA_HOST_STARTUP_TEST_MODE="+mode)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("isolated %s startup failed: %v\n%s", mode, err, output)
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

func runLoadedHostForRustTest(t *testing.T, config auth.Config) error {
	t.Helper()
	if config.Authority != "localhost:4848" || config.Origin != "http://localhost:4848" ||
		config.RPID != "localhost" || config.ListenAddress != "127.0.0.1:4848" ||
		config.Secure || config.DevNoAuth || config.GraphiQL || config.LocalGraphQLSocket ||
		config.BrowserMaxSessions != 2 || config.BrowserMaxOldSpaceMB != 1024 {
		return fmt.Errorf("loaded web config = %#v", config)
	}
	output := &rustHostReadyWriter{ready: make(chan struct{})}
	result := make(chan error, 1)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	go func() { result <- runWithLoadedConfig(ctx, config.ListenAddress, output, nil, &config) }()
	select {
	case <-output.ready:
		if !strings.Contains(output.String(), config.ListenAddress) {
			return fmt.Errorf("loaded startup address = %q, want %q", output.String(), config.ListenAddress)
		}
		cancel()
	case <-time.After(10 * time.Second):
		cancel()
		return fmt.Errorf("loaded host did not report readiness")
	}
	select {
	case err := <-result:
		return err
	case <-time.After(10 * time.Second):
		return fmt.Errorf("loaded host did not stop after cancellation")
	}
}

type rustHostReadyWriter struct {
	ready  chan struct{}
	once   sync.Once
	buffer bytes.Buffer
}

func (w *rustHostReadyWriter) Write(value []byte) (int, error) {
	_, _ = w.buffer.Write(value)
	if strings.Contains(string(value), "Noema listening on ") {
		w.once.Do(func() { close(w.ready) })
	}
	return len(value), nil
}

func (w *rustHostReadyWriter) String() string { return w.buffer.String() }

func isRustHostModelProvider(providerKind string) bool {
	switch providerKind {
	case "codex", "openai", "foundation_local", "local_models", "openrouter":
		return true
	default:
		return false
	}
}

// Rust source: crates/noema-host/src/composition/tests.rs:211::runtime_host_error_messages_are_plain_language
func TestRustHost_runtime_host_error_messages_are_plain_language(t *testing.T) {
	cases := []struct {
		error RuntimeHostError
		want  string
	}{
		{NewRuntimeHostStoreError(errors.New("failed")), "Noema could not start its local memory store."},
		{NewRuntimeHostCompositionError("failed"), "Noema could not start the local assistant service."},
	}
	for _, test := range cases {
		if got := test.error.UserMessage(); got != test.want {
			t.Fatalf("user message = %q, want %q", got, test.want)
		}
	}
}
