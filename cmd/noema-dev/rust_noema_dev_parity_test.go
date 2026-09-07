package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// Rust source: crates/noema-dev/src/main.rs::nested_cargo_does_not_inherit_native_compiler_wrappers (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_nested_cargo_does_not_inherit_native_compiler_wrappers(t *testing.T) {
	command := exec.Command("cargo")
	stripCargoRunEnv(command)
	for _, key := range []string{"CC", "CXX"} {
		for _, entry := range command.Env {
			if strings.HasPrefix(entry, key+"=") {
				t.Fatalf("%s should be removed: %q", key, entry)
			}
		}
	}
}

// Rust source: crates/noema-dev/src/main.rs::server_watcher_ignores_web_sources_and_generated_assets (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_server_watcher_ignores_web_sources_and_generated_assets(t *testing.T) {
	want := []string{"apps/web/**", "crates/noema-server/target/web-assets/**"}
	if len(webServerWatchIgnoreGlobs) != len(want) {
		t.Fatalf("watch ignore globs = %#v", webServerWatchIgnoreGlobs)
	}
	for index := range want {
		if webServerWatchIgnoreGlobs[index] != want[index] {
			t.Fatalf("watch ignore globs = %#v", webServerWatchIgnoreGlobs)
		}
	}
}

// Rust source: crates/noema-dev/src/main.rs::server_watcher_uses_fast_development_profile (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_server_watcher_uses_fast_development_profile(t *testing.T) {
	command := exec.Command("cargo")
	configureWebServerWatcher(command, "/workspace/noema-dev")
	arguments := command.Args[1:]
	hasPair := func(left, right string) bool {
		for index := 0; index+1 < len(arguments); index++ {
			if arguments[index] == left && arguments[index+1] == right {
				return true
			}
		}
		return false
	}
	containsNoProcessGroup := false
	for _, argument := range arguments {
		if argument == "--no-process-group" {
			containsNoProcessGroup = true
			break
		}
	}
	if containsNoProcessGroup || !hasPair("--delay", "1.5") ||
		!hasPair("-E", "CARGO_PROFILE_DEV_INCREMENTAL=true") ||
		!hasPair("-E", "CARGO_PROFILE_DEV_DEBUG=0") {
		t.Fatalf("fast development watcher arguments = %#v", arguments)
	}
}

// Rust source: crates/noema-dev/src/main.rs::server_watcher_runs_the_budgeted_server_command (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_server_watcher_runs_the_budgeted_server_command(t *testing.T) {
	command := exec.Command("cargo")
	configureWebServerWatcher(command, "/workspace/noema-dev")
	arguments := command.Args[1:]
	found := false
	for index := 0; index+2 < len(arguments); index++ {
		if arguments[index] == "--" && arguments[index+1] == "/workspace/noema-dev" && arguments[index+2] == "serve" {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("budgeted server command = %#v", arguments)
	}
}

// Rust source: crates/noema-dev/src/workflow.rs::accepts_only_named_noema_caches_below_repository_target (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_accepts_only_named_noema_caches_below_repository_target(t *testing.T) {
	root := "/workspace/noema"
	if !cacheTargetIsSafe(root, filepath.Join(root, "target/noema-validation")) ||
		!cacheTargetIsSafe(root, filepath.Join(root, "target/noema-dev")) ||
		cacheTargetIsSafe(root, filepath.Join(root, "target")) ||
		cacheTargetIsSafe(root, root) ||
		cacheTargetIsSafe(root, "/tmp/noema-validation") ||
		cacheTargetIsSafe(root, filepath.Join(root, "target/noema-dev/nested")) ||
		cacheTargetIsSafe(root, filepath.Join(root, "target/noema-other")) {
		t.Fatal("cache target safety changed")
	}
}

// Rust source: crates/noema-dev/src/workflow.rs::cleans_only_after_cache_exceeds_its_byte_budget (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_cleans_only_after_cache_exceeds_its_byte_budget(t *testing.T) {
	if shouldCleanCache(20, 20) || !shouldCleanCache(21, 20) {
		t.Fatal("cache budget boundary changed")
	}
}

// Rust source: crates/noema-dev/src/workflow.rs::development_server_does_not_override_authentication (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_development_server_does_not_override_authentication(t *testing.T) {
	command := developmentServerCommand("/workspace/noema", true)
	for _, entry := range command.Env {
		if strings.HasPrefix(entry, "NOEMA_WEB__DEV_NO_AUTH=") {
			t.Fatalf("development server overrides authentication: %q", entry)
		}
	}
}

// Rust source: crates/noema-dev/src/workflow.rs::development_server_binds_only_to_loopback (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_development_server_binds_only_to_loopback(t *testing.T) {
	command := developmentServerCommand("/workspace/noema", true)
	var host string
	for _, entry := range command.Env {
		if strings.HasPrefix(entry, "NOEMA_WEB__HOST=") {
			host = strings.TrimPrefix(entry, "NOEMA_WEB__HOST=")
		}
	}
	if host != "127.0.0.1" {
		t.Fatalf("development server host = %q", host)
	}
}

// Rust source: crates/noema-dev/src/workflow.rs::root_development_server_drops_identity_and_uses_the_instance_home (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_root_development_server_drops_identity_and_uses_the_instance_home(t *testing.T) {
	command := developmentServerCommand("/workspace/noema", true)
	if command.Args[0] != "/workspace/noema/scripts/run-noema-dev-server" {
		t.Fatalf("root development server program = %q", command.Args[0])
	}
	if command.Dir != "/workspace/noema" {
		t.Fatalf("root development server directory = %q", command.Dir)
	}
}

// Rust source: crates/noema-dev/src/workflow.rs::prepares_cache_with_cargo_ownership_tag (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_prepares_cache_with_cargo_ownership_tag(t *testing.T) {
	directory := filepath.Join(t.TempDir(), "cache")
	if err := prepareCacheTarget(directory); err != nil {
		t.Fatal(err)
	}
	contents, err := os.ReadFile(filepath.Join(directory, "CACHEDIR.TAG"))
	if err != nil || string(contents) != cargoCacheTag {
		t.Fatalf("cache ownership tag = %q, %v", contents, err)
	}
	if info, err := os.Stat(filepath.Join(directory, cacheTempDirectory)); err != nil || !info.IsDir() {
		t.Fatalf("cache temporary directory = %v, %v", info, err)
	}
}

// Rust source: crates/noema-dev/src/workflow.rs::repairs_invalid_cargo_ownership_tag (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_repairs_invalid_cargo_ownership_tag(t *testing.T) {
	directory := t.TempDir()
	if err := os.WriteFile(filepath.Join(directory, "CACHEDIR.TAG"), []byte("invalid"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := prepareCacheTarget(directory); err != nil {
		t.Fatal(err)
	}
	contents, err := os.ReadFile(filepath.Join(directory, "CACHEDIR.TAG"))
	if err != nil || string(contents) != cargoCacheTag {
		t.Fatalf("repaired cache ownership tag = %q, %v", contents, err)
	}
}
