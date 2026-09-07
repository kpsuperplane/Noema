package auth

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
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
	if debug := fmt.Sprintf("%#v", recovery); !strings.Contains(debug, "Recovery") {
		t.Fatalf("recovery debug output was empty: %q", debug)
	}
	parsed, err := parseRecoveryCode(code)
	if err != nil {
		t.Fatal(err)
	}
	debug := fmt.Sprintf("%#v", parsed)
	if strings.Contains(debug, code) || !strings.Contains(debug, "RecoveryCode") || !strings.Contains(debug, "[REDACTED]") {
		t.Fatal("parsed recovery code retained the source code")
	}
	assertRustHostRecoveryPermissions(t, paths.Config())
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
			if _, _, err := LoadConfig(paths, "127.0.0.1:3737"); err == nil {
				t.Fatalf("malformed recovery code error = %v", err)
			} else {
				var recoveryErr *RecoveryError
				if !errors.As(err, &recoveryErr) || recoveryErr.Kind != RecoveryMalformedField {
					t.Fatalf("malformed recovery code error = %v", err)
				}
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
	} else {
		var recoveryErr *RecoveryError
		if !errors.As(err, &recoveryErr) || recoveryErr.Kind != RecoveryRead {
			t.Fatalf("first recovery attempt = %v", err)
		}
	}
	if _, err := recovery.Attempt("candidate"); err == nil {
		t.Fatal("disabled recovery accepted a later attempt")
	} else {
		var recoveryErr *RecoveryError
		if !errors.As(err, &recoveryErr) || recoveryErr.Kind != RecoveryUnavailable || err.Error() != "recovery is unavailable until Noema restarts" {
			t.Fatalf("second recovery attempt = %v", err)
		}
	}
}

// Rust source: crates/noema-host/src/config/tests.rs:75::configuration_source_and_provider_contracts
func TestRustHost_configuration_source_and_provider_contracts(t *testing.T) {
	t.Run("bare Codex does not require OpenAI credentials", func(t *testing.T) {
		paths, err := home.FromRoot(filepath.Join(t.TempDir(), "bare-codex"))
		if err != nil {
			t.Fatal(err)
		}
		if err := home.AtomicWritePrivate(paths.Config(), []byte("provider: codex\n")); err != nil {
			t.Fatal(err)
		}
		for _, name := range []string{
			"NOEMA_PROVIDER", "NOEMA_MODEL", "NOEMA_REASONING_EFFORT", "NOEMA_CODEX__MODEL",
			"NOEMA_CODEX__REASONING_EFFORT", "NOEMA_CODEX__BASE_URL", "NOEMA_CODEX__TIMEOUT_SECONDS",
			"NOEMA_CODEX__TOOL_CLASSIFICATION_MODEL", "NOEMA_OPENAI__API_KEY",
		} {
			unsetRustHostEnv(t, name)
		}
		resolved, err := ResolveProviderConfig(paths, "")
		if err != nil {
			t.Fatal(err)
		}
		if resolved.Provider != "codex" || resolved.Model != "gpt-5.6-terra" ||
			resolved.Codex.BaseURL != "https://chatgpt.com/backend-api/codex" ||
			resolved.Codex.DefaultModel != "gpt-5.6-terra" || resolved.Codex.TimeoutSeconds != 300 {
			t.Fatalf("bare Codex resolution = %#v", resolved)
		}
	})

	providerSource, err := home.FromRoot(filepath.Join(t.TempDir(), "provider-source"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(providerSource.Config(), []byte("provider: openai\nmodel: noema-home-model\nreasoning_effort: medium\n")); err != nil {
		t.Fatal(err)
	}
	t.Setenv("NOEMA_OPENAI__API_KEY", "env-key")
	providerConfig, err := ResolveProviderConfig(providerSource, "")
	if err != nil {
		t.Fatal(err)
	}
	var providerKey string
	if err := providerConfig.OpenAI.APIKey.Use(func(value string) error { providerKey = value; return nil }); err != nil {
		t.Fatal(err)
	}
	if providerConfig.Provider != "openai" || providerConfig.Model != "noema-home-model" ||
		providerConfig.ReasoningEffort != "medium" || providerKey != "env-key" {
		t.Fatalf("provider source resolution = %#v", providerConfig)
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
	localConfig, err := ResolveProviderConfig(localSource, "")
	if err != nil {
		t.Fatal(err)
	}
	if localConfig.Provider != "local_models" || localConfig.Model != "ternary-bonsai-8b" ||
		localConfig.LocalModels.ContextWindowTokens != 16384 || localConfig.LocalModels.TimeoutSeconds != 900 ||
		localConfig.LocalModels.StartupTimeoutSeconds != 240 {
		t.Fatalf("local model resolution = %#v", localConfig)
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
	codexConfig, err := ResolveProviderConfig(codexSource, "")
	if err != nil {
		t.Fatal(err)
	}
	if codexConfig.Provider != "codex" || codexConfig.Model != "env-model" ||
		codexConfig.ReasoningEffort != "high" || codexConfig.Codex.ToolClassificationModel != "env-codex-tool-classifier" ||
		codexConfig.Codex.BaseURL != "https://env.example/codex" || codexConfig.Codex.TimeoutSeconds != 123 {
		t.Fatalf("codex resolution = %#v", codexConfig)
	}
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
	openAIConfig, err := ResolveProviderConfig(openAIFile, "")
	if err != nil {
		t.Fatal(err)
	}
	if openAIConfig.Provider != "openai" || openAIConfig.Model != "gpt-5.6-terra" || openAIConfig.ReasoningEffort != "medium" {
		t.Fatalf("OpenAI defaults = %#v", openAIConfig)
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
	precedence, err := home.FromRoot(filepath.Join(t.TempDir(), "precedence"))
	if err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(precedence.Config(), []byte("provider: openai\nmodel: yaml-model\nreasoning_effort: medium\nopenai:\n  base_url: https://yaml.example/v1\n  organization_id: yaml-org\n  project_id: yaml-project\n  timeout_seconds: 22\n")); err != nil {
		t.Fatal(err)
	}
	for key, value := range map[string]string{
		"NOEMA_OPENAI__BASE_URL":        "https://env.example/v1",
		"NOEMA_OPENAI__ORGANIZATION_ID": "env-org",
		"NOEMA_OPENAI__PROJECT_ID":      "env-project",
		"NOEMA_OPENAI__TIMEOUT_SECONDS": "33",
		"NOEMA_MODEL":                   "env-model",
		"NOEMA_REASONING_EFFORT":        "high",
	} {
		t.Setenv(key, value)
	}
	resolved, err := ResolveProviderConfig(precedence, "")
	if err != nil {
		t.Fatal(err)
	}
	if resolved.OpenAI.BaseURL != "https://env.example/v1" || resolved.OpenAI.OrganizationID != "env-org" ||
		resolved.OpenAI.ProjectID != "env-project" || resolved.OpenAI.TimeoutSeconds != 33 ||
		resolved.Model != "env-model" || resolved.ReasoningEffort != "high" {
		t.Fatalf("provider precedence = %#v", resolved)
	}
	secretHome, err := home.FromRoot(filepath.Join(t.TempDir(), "secret"))
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv("NOEMA_MODEL", "")
	t.Setenv("NOEMA_REASONING_EFFORT", "")
	const sentinel = "noema-debug-secret-sentinel"
	t.Setenv("NOEMA_OPENAI__API_KEY", sentinel)
	resolved, err = ResolveProviderConfig(secretHome, "")
	if err != nil {
		t.Fatal(err)
	}
	debug = fmt.Sprintf("%#v", resolved)
	if strings.Contains(debug, sentinel) || !strings.Contains(debug, "[REDACTED]") {
		t.Fatalf("resolved provider debug = %s", debug)
	}
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
		resolved, err := ResolveProviderConfig(paths, "")
		if err != nil {
			t.Fatal(err)
		}
		if resolved.Provider != "codex" || resolved.Codex.ToolClassificationModel != "yaml-tool-classifier" {
			t.Fatalf("tool classification resolution = %#v", resolved)
		}
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
		_, err = ResolveProviderConfig(paths, "")
		if err == nil {
			t.Fatal("YAML API key was accepted")
		}
		var configErr *ProviderConfigError
		if !errors.As(err, &configErr) || configErr.Kind != ProviderConfigInvalid || configErr.Message != "openai.api_key must be supplied through NOEMA_OPENAI__API_KEY" {
			t.Fatalf("YAML API key error = %v", err)
		}
	})
	t.Run("legacy OpenAI environment is ignored", func(t *testing.T) {
		paths, err := home.FromRoot(filepath.Join(t.TempDir(), "legacy-openai-env"))
		if err != nil {
			t.Fatal(err)
		}
		if err := home.AtomicWritePrivate(paths.Config(), []byte("provider: openai\n")); err != nil {
			t.Fatal(err)
		}
		t.Setenv("NOEMA_PROVIDER", "openai")
		t.Setenv("OPENAI_API_KEY", "old-key")
		t.Setenv("NOEMA_OPENAI__API_KEY", "")
		_, err = ResolveProviderConfig(paths, "")
		if err == nil {
			t.Fatal("legacy OPENAI_API_KEY satisfied provider credentials")
		}
		var configErr *ProviderConfigError
		if !errors.As(err, &configErr) || configErr.Kind != ProviderConfigMissingSecret ||
			configErr.Provider != "openai" || configErr.Credential != "NOEMA_OPENAI__API_KEY" {
			t.Fatalf("legacy OpenAI credential error = %v", err)
		}
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
			if _, err := ResolveProviderConfig(paths, ""); err == nil || !strings.Contains(err.Error(), "reasoning_effort") {
				t.Fatalf("partial Codex config error = %v", err)
			}
		}
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
		{name: "browser sessions too high", body: "browser:\n  max_sessions: 9\n", want: "invalid integer value for browser.max_sessions"},
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

	missingExplicit, err := home.FromRoot(filepath.Join(t.TempDir(), "missing-explicit"))
	if err != nil {
		t.Fatal(err)
	}
	_, err = ResolveProviderConfig(missingExplicit, filepath.Join(missingExplicit.Root(), "missing-config.yaml"))
	if err == nil {
		t.Fatal("missing explicit config was accepted")
	}
	var configErr *ProviderConfigError
	expectedMissingPath := filepath.Join(missingExplicit.Root(), "missing-config.yaml")
	if !errors.As(err, &configErr) || configErr.Kind != ProviderConfigFileNotFound ||
		configErr.Path != expectedMissingPath || err.Error() != "config file not found: "+expectedMissingPath {
		t.Fatalf("missing explicit config error = %v", err)
	}

	t.Run("zero Codex timeout", func(t *testing.T) {
		paths, err := home.FromRoot(filepath.Join(t.TempDir(), "zero-timeout"))
		if err != nil {
			t.Fatal(err)
		}
		t.Setenv("NOEMA_PROVIDER", "codex")
		t.Setenv("NOEMA_CODEX__TIMEOUT_SECONDS", "0")
		_, err = ResolveProviderConfig(paths, "")
		if err == nil {
			t.Fatal("zero Codex timeout was accepted")
		}
		var configErr *ProviderConfigError
		if !errors.As(err, &configErr) || configErr.Kind != ProviderConfigInvalidNumber || configErr.Name != "NOEMA_CODEX__TIMEOUT_SECONDS" {
			t.Fatalf("zero Codex timeout error = %v", err)
		}
	})
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
		if _, _, err := LoadConfig(validPaths, "127.0.0.1:3737"); err == nil {
			t.Fatalf("invalid browser limit %s=%s error = %v", name, value, err)
		} else {
			var configErr *ProviderConfigError
			if !errors.As(err, &configErr) || configErr.Kind != ProviderConfigInvalidNumber ||
				configErr.Path != validPaths.Config() || configErr.Name != name || configErr.Value != value {
				t.Fatalf("invalid browser limit %s=%s error = %v", name, value, err)
			}
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
	t.Setenv("NOEMA_PROVIDER", "unknown")
	_, err = ResolveProviderConfig(missing, "")
	if err == nil {
		t.Fatal("unsupported provider was accepted")
	}
	if !errors.As(err, &configErr) || configErr.Kind != ProviderConfigUnsupported || configErr.Provider != "unknown" {
		t.Fatalf("unsupported provider error = %v", err)
	}
}

func unsetRustHostEnv(t *testing.T, name string) {
	t.Helper()
	previous, existed := os.LookupEnv(name)
	if err := os.Unsetenv(name); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if existed {
			_ = os.Setenv(name, previous)
		} else {
			_ = os.Unsetenv(name)
		}
	})
}
