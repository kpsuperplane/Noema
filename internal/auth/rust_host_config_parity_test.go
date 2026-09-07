package auth

import (
	"encoding/base64"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/mcp"
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
	decoded, err := base64.RawURLEncoding.DecodeString(code)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(fmt.Sprintf("%#v", decoded), code) {
		t.Fatal("parsed recovery code retained the source code")
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
	providerSource, err := home.FromRoot(filepath.Join(t.TempDir(), "provider-source"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(providerSource.Config(), []byte("provider: openai\nmodel: noema-home-model\nreasoning_effort: medium\n")); err != nil {
		t.Fatal(err)
	}
	t.Setenv("NOEMA_OPENAI__API_KEY", "env-key")
	providerDocument, err := readConfigDocument(providerSource.Config())
	if err != nil || providerDocument["provider"] != "openai" || providerDocument["model"] != "noema-home-model" || providerDocument["reasoning_effort"] != "medium" {
		t.Fatalf("provider source document = %#v, %v", providerDocument, err)
	}

	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(paths.Config(), []byte("provider: codex\nbrowser:\n  max_sessions: 4\nweb:\n  host: 127.0.0.2\n  port: 4747\n")); err != nil {
		t.Fatal(err)
	}
	for key, value := range map[string]string{
		"NOEMA_WEB__HOST":                 "127.0.0.3",
		"NOEMA_WEB__PORT":                 "5757",
		"NOEMA_WEB__RP_ID":                "noema.example",
		"NOEMA_WEB__PUBLIC_ORIGIN":        "https://noema.example",
		"NOEMA_WEB__DEV_NO_AUTH":          "true",
		"NOEMA_WEB__LOCAL_GRAPHQL_SOCKET": "true",
		"NOEMA_WEB__GRAPHIQL":             "true",
		"NOEMA_MCP__STDIO_ENABLED":        "true",
	} {
		t.Setenv(key, value)
	}
	config, _, err := LoadConfig(paths, "")
	if err != nil {
		t.Fatal(err)
	}
	if config.ListenAddress != "127.0.0.3:5757" || config.Authority != "noema.example" ||
		config.Origin != "https://noema.example" || config.RPID != "noema.example" ||
		!config.Secure || !config.DevNoAuth || !config.GraphiQL || !config.LocalGraphQLSocket ||
		config.BrowserMaxSessions != 4 || config.BrowserMaxOldSpaceMB != 1024 {
		t.Fatalf("resolved config = %#v", config)
	}
	stdio, err := mcp.StdioEnabled(paths)
	if err != nil || !stdio {
		t.Fatalf("resolved MCP config = %t, %v", stdio, err)
	}
	document, err := readConfigDocument(paths.Config())
	if err != nil || document["provider"] != "codex" {
		t.Fatalf("source document = %#v, %v", document, err)
	}
	localSource, err := home.FromRoot(filepath.Join(t.TempDir(), "local-source"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(localSource.Config(), []byte("provider: local_models\nmodel: ternary-bonsai-8b\nlocal_models:\n  context_window_tokens: 16384\n  timeout_seconds: 900\n  startup_timeout_seconds: 240\n")); err != nil {
		t.Fatal(err)
	}
	localDocument, err := readConfigDocument(localSource.Config())
	if err != nil || localDocument["provider"] != "local_models" || localDocument["model"] != "ternary-bonsai-8b" {
		t.Fatalf("local model source document = %#v, %v", localDocument, err)
	}
	codexSource, err := home.FromRoot(filepath.Join(t.TempDir(), "codex-source"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(codexSource.Config(), []byte("provider: codex\nmodel: yaml-model\nreasoning_effort: medium\ncodex:\n  base_url: https://yaml.example/codex\n  model: yaml-codex-model\n  tool_classification_model: yaml-codex-tool-classifier\n  timeout_seconds: 120\n")); err != nil {
		t.Fatal(err)
	}
	for key, value := range map[string]string{
		"NOEMA_MODEL":                            "env-model",
		"NOEMA_REASONING_EFFORT":                 "high",
		"NOEMA_CODEX__TOOL_CLASSIFICATION_MODEL": "env-codex-tool-classifier",
		"NOEMA_CODEX__BASE_URL":                  "https://env.example/codex/",
		"NOEMA_CODEX__TIMEOUT_SECONDS":           "123",
	} {
		t.Setenv(key, value)
	}
	codexDocument, err := readConfigDocument(codexSource.Config())
	if err != nil || codexDocument["provider"] != "codex" || codexDocument["model"] != "yaml-model" {
		t.Fatalf("codex source document = %#v, %v", codexDocument, err)
	}
	t.Fatal("unsupported port: Go host has no provider configuration source and resolution authority")
}

// Rust source: crates/noema-host/src/config/tests/default_provider.rs:3::provider_resolution_precedence_and_secrecy_contracts
func TestRustHost_provider_resolution_precedence_and_secrecy_contracts(t *testing.T) {
	openAIFile, err := home.FromRoot(filepath.Join(t.TempDir(), "openai-source"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(openAIFile.Config(), []byte("provider: openai\n")); err != nil {
		t.Fatal(err)
	}
	t.Setenv("NOEMA_OPENAI__API_KEY", "env-key")
	openAIDocument, err := readConfigDocument(openAIFile.Config())
	if err != nil || openAIDocument["provider"] != "openai" {
		t.Fatalf("OpenAI source document = %#v, %v", openAIDocument, err)
	}

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
	debug := fmt.Sprintf("%#v", config)
	if strings.Contains(debug, "OPENAI_API_KEY") || strings.Contains(debug, "noema-debug-secret-sentinel") {
		t.Fatal("configuration debug output exposed an environment credential name")
	}
	t.Fatal("unsupported port: Go host has no provider resolution precedence and credential-redaction authority")
}

// Rust source: crates/noema-host/src/config/tests/validation.rs:3::configuration_validation_contracts
func TestRustHost_configuration_validation_contracts(t *testing.T) {
	t.Run("top-level tool classification model", func(t *testing.T) {
		paths, err := home.FromRoot(filepath.Join(t.TempDir(), "codex-model"))
		if err != nil {
			t.Fatal(err)
		}
		if err := home.AtomicWritePrivate(paths.Config(), []byte("provider: codex\ntool_classification_model: yaml-tool-classifier\ncodex:\n  tool_classification_model: yaml-codex-tool-classifier\n")); err != nil {
			t.Fatal(err)
		}
		document, err := readConfigDocument(paths.Config())
		if err != nil || document["tool_classification_model"] != "yaml-tool-classifier" {
			t.Fatalf("tool classification source = %#v, %v", document, err)
		}
		t.Fatal("unsupported port: Go host has no Codex provider configuration resolver")
	})
	t.Run("legacy and secret provider fields", func(t *testing.T) {
		paths, err := home.FromRoot(filepath.Join(t.TempDir(), "openai-fields"))
		if err != nil {
			t.Fatal(err)
		}
		t.Setenv("OPENAI_API_KEY", "old-key")
		if err := home.AtomicWritePrivate(paths.Config(), []byte("provider: openai\nopenai:\n  api_key: not-allowed\n")); err != nil {
			t.Fatal(err)
		}
		document, err := readConfigDocument(paths.Config())
		if err != nil || document["provider"] != "openai" {
			t.Fatalf("OpenAI validation source = %#v, %v", document, err)
		}
		t.Fatal("unsupported port: Go host has no provider credential source validation authority")
	})
	t.Run("Codex model selection", func(t *testing.T) {
		for _, yaml := range []string{"provider: codex\ncodex:\n  model: gpt-5.5\n", "provider: codex\ncodex:\n  reasoning_effort: medium\n"} {
			paths, err := home.FromRoot(filepath.Join(t.TempDir(), "partial-codex"))
			if err != nil {
				t.Fatal(err)
			}
			if err := home.AtomicWritePrivate(paths.Config(), []byte(yaml)); err != nil {
				t.Fatal(err)
			}
		}
		t.Fatal("unsupported port: Go host has no Codex model and reasoning-effort validation authority")
	})

	for _, test := range []struct {
		name string
		body string
		env  map[string]string
		want string
	}{
		{name: "top-level scalar", body: "null\n", want: "config.yaml must contain a mapping"},
		{name: "invalid web host", body: "web:\n  host: example.com\n", want: "web.host must be a numeric IP address"},
		{name: "zero port", body: "web:\n  port: 0\n", want: "config.yaml field web.port must be an integer from 1 through 65535"},
		{name: "browser sessions too high", body: "browser:\n  max_sessions: 9\n", want: "config.yaml field browser.max_sessions is invalid"},
		{name: "invalid env boolean", body: "web: {}\n", env: map[string]string{"NOEMA_WEB__GRAPHIQL": "sometimes"}, want: "NOEMA_WEB__GRAPHIQL must be true or false"},
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
			} else if !strings.Contains(err.Error(), test.want) {
				t.Fatalf("invalid configuration error = %q, want %q", err, test.want)
			}
		})
	}
	t.Setenv("NOEMA_PROVIDER", "codex")
	t.Setenv("NOEMA_BROWSER__MAX_SESSIONS", "4")
	t.Setenv("NOEMA_BROWSER__MAX_OLD_SPACE_MB", "2048")
	validPaths, err := home.FromRoot(filepath.Join(t.TempDir(), "browser-limits"))
	if err != nil {
		t.Fatal(err)
	}
	valid, _, err := LoadConfig(validPaths, "127.0.0.1:3737")
	if err != nil {
		t.Fatalf("browser limits should resolve: %v", err)
	}
	if valid.BrowserMaxSessions != 4 || valid.BrowserMaxOldSpaceMB != 2048 {
		t.Fatalf("browser limits = %#v", valid)
	}
	for name, value := range map[string]string{
		"NOEMA_BROWSER__MAX_SESSIONS":     "9",
		"NOEMA_BROWSER__MAX_OLD_SPACE_MB": "255",
	} {
		t.Setenv(name, value)
		if _, _, err := LoadConfig(validPaths, "127.0.0.1:3737"); err == nil || !strings.Contains(err.Error(), name) {
			t.Fatalf("invalid browser limit %s=%s error = %v", name, value, err)
		}
		t.Setenv("NOEMA_BROWSER__MAX_SESSIONS", "4")
		t.Setenv("NOEMA_BROWSER__MAX_OLD_SPACE_MB", "2048")
	}
	missing, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := LoadConfig(missing, "127.0.0.1:3737"); err != nil {
		t.Fatalf("missing configuration should receive protected defaults: %v", err)
	}
	t.Fatal("unsupported port: Go host has no provider validation, typed configuration errors, or explicit-file source authority")
}
