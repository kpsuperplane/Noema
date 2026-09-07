package auth

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/home"
)

// Rust source: crates/noema-host/src/config/recovery.rs:257::missing_code_is_generated_privately_without_losing_other_values
func TestRustHost_missing_code_is_generated_privately_without_losing_other_values(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(paths.Config(), []byte("provider: codex\nweb:\n  host: 127.0.0.2\n")); err != nil {
		t.Fatal(err)
	}
	_, recovery, err := LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	code := readRecoveryCode(t, paths)
	if len(code) != 43 || !validRecoveryCode(code) {
		t.Fatalf("generated recovery code = %q", code)
	}
	document, err := readConfigDocument(paths.Config())
	if err != nil {
		t.Fatal(err)
	}
	if document["provider"] != "codex" {
		t.Fatalf("provider was changed: %#v", document["provider"])
	}
	web, err := childMap(document, "web")
	if err != nil || web["host"] != "127.0.0.2" {
		t.Fatalf("web host was changed: %#v, %v", web, err)
	}
	if strings.Contains(fmt.Sprintf("%#v", recovery), code) {
		t.Fatal("recovery object retained the generated code")
	}
	if runtime.GOOS != "windows" {
		info, err := os.Stat(paths.Config())
		if err != nil || info.Mode().Perm() != 0o600 {
			t.Fatalf("config permissions = %v, %v", info, err)
		}
	}
}

// Rust source: crates/noema-host/src/config/recovery.rs:284::every_candidate_rotates_and_only_the_current_code_matches
func TestRustHost_every_candidate_rotates_and_only_the_current_code_matches(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(paths.Config(), []byte("web: {}\n")); err != nil {
		t.Fatal(err)
	}
	_, recovery, err := LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	first := readRecoveryCode(t, paths)
	matched, err := recovery.Attempt("malformed")
	if err != nil || matched {
		t.Fatalf("malformed attempt = %v, %v", matched, err)
	}
	second := readRecoveryCode(t, paths)
	if second == first {
		t.Fatal("malformed attempt did not rotate the code")
	}
	matched, err = recovery.Attempt(first)
	if err != nil || matched {
		t.Fatalf("stale attempt = %v, %v", matched, err)
	}
	third := readRecoveryCode(t, paths)
	if third == second {
		t.Fatal("stale attempt did not rotate the code")
	}
	matched, err = recovery.Attempt(third)
	if err != nil || !matched {
		t.Fatalf("current attempt = %v, %v", matched, err)
	}
	if current := readRecoveryCode(t, paths); current == third {
		t.Fatal("successful attempt did not rotate the code")
	}
}

// Rust source: crates/noema-host/src/config/recovery.rs:302::malformed_configured_codes_fail_with_field_specific_errors
func TestRustHost_malformed_configured_codes_fail_with_field_specific_errors(t *testing.T) {
	for _, value := range []string{"", "short", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="} {
		t.Run(fmt.Sprintf("%q", value), func(t *testing.T) {
			paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
			if err != nil {
				t.Fatal(err)
			}
			if err := home.AtomicWritePrivate(paths.Config(), []byte("web:\n  recovery_code: '"+value+"'\n")); err != nil {
				t.Fatal(err)
			}
			if _, _, err := LoadConfig(paths, "127.0.0.1:3737"); err == nil || !strings.Contains(err.Error(), "recovery_code") {
				t.Fatalf("malformed recovery code error = %v", err)
			}
		})
	}
}

// Rust source: crates/noema-host/src/config/recovery.rs:315::failed_live_read_disables_later_attempts
func TestRustHost_failed_live_read_disables_later_attempts(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(paths.Config(), []byte("web: {}\n")); err != nil {
		t.Fatal(err)
	}
	_, recovery, err := LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(paths.Config()); err != nil {
		t.Fatal(err)
	}
	if _, err := recovery.Attempt("candidate"); err == nil {
		t.Fatal("missing live config was accepted")
	}
	if _, err := recovery.Attempt("candidate"); err == nil || !strings.Contains(err.Error(), "unavailable") {
		t.Fatalf("second recovery attempt = %v", err)
	}
}

// Rust source: crates/noema-host/src/config/tests.rs:75::configuration_source_and_provider_contracts
func TestRustHost_configuration_source_and_provider_contracts(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(paths.Config(), []byte("provider: codex\nbrowser:\n  max_sessions: 4\nweb:\n  host: 127.0.0.2\n  port: 4747\n")); err != nil {
		t.Fatal(err)
	}
	t.Setenv("NOEMA_WEB__HOST", "127.0.0.3")
	t.Setenv("NOEMA_WEB__PORT", "5757")
	t.Setenv("NOEMA_WEB__GRAPHIQL", "true")
	config, _, err := LoadConfig(paths, "")
	if err != nil {
		t.Fatal(err)
	}
	if config.ListenAddress != "127.0.0.3:5757" || !config.GraphiQL || config.BrowserMaxSessions != 4 {
		t.Fatalf("resolved config = %#v", config)
	}
	document, err := readConfigDocument(paths.Config())
	if err != nil || document["provider"] != "codex" {
		t.Fatalf("source document = %#v, %v", document, err)
	}
}

// Rust source: crates/noema-host/src/config/tests/default_provider.rs:3::provider_resolution_precedence_and_secrecy_contracts
func TestRustHost_provider_resolution_precedence_and_secrecy_contracts(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(paths.Config(), []byte("web:\n  host: 127.0.0.1\n")); err != nil {
		t.Fatal(err)
	}
	config, _, err := LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	if config.Authority != "localhost:3737" || config.Origin != "http://localhost:3737" || config.RPID != "localhost" {
		t.Fatalf("default public configuration = %#v", config)
	}
	if strings.Contains(fmt.Sprintf("%#v", config), "OPENAI_API_KEY") {
		t.Fatal("configuration debug output exposed an environment credential name")
	}
}

// Rust source: crates/noema-host/src/config/tests/validation.rs:3::configuration_validation_contracts
func TestRustHost_configuration_validation_contracts(t *testing.T) {
	for _, test := range []struct {
		name string
		body string
		env  map[string]string
	}{
		{name: "top-level scalar", body: "null\n"},
		{name: "invalid web host", body: "web:\n  host: example.com\n"},
		{name: "zero port", body: "web:\n  port: 0\n"},
		{name: "browser sessions too high", body: "browser:\n  max_sessions: 9\n"},
		{name: "invalid env boolean", body: "web: {}\n", env: map[string]string{"NOEMA_WEB__GRAPHIQL": "sometimes"}},
	} {
		t.Run(test.name, func(t *testing.T) {
			paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
			if err != nil {
				t.Fatal(err)
			}
			if err := home.AtomicWritePrivate(paths.Config(), []byte(test.body)); err != nil {
				t.Fatal(err)
			}
			for key, value := range test.env {
				t.Setenv(key, value)
			}
			if _, _, err := LoadConfig(paths, "127.0.0.1:3737"); err == nil {
				t.Fatal("invalid configuration was accepted")
			}
		})
	}
	missing, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := LoadConfig(missing, "127.0.0.1:3737"); err != nil {
		t.Fatalf("missing configuration should receive protected defaults: %v", err)
	}
}
