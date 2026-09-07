package home

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

// Rust source: crates/noema-home/src/initialization.rs:85::initialization_creates_layout_and_honors_config_write_policy
func TestRustHome_initialization_creates_layout_and_honors_config_write_policy(t *testing.T) {
	const testConfig = "provider: test\n"
	paths, err := FromRoot(filepath.Join(t.TempDir(), "noema"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatalf("open home: %v", err)
	}
	t.Cleanup(func() { _ = root.Close() })

	if info, err := os.Stat(paths.Root()); err != nil || !info.IsDir() {
		t.Fatalf("home layout = %v, %v", info, err)
	}
	runPath := filepath.Join(paths.Root(), "run")
	runInfo, err := os.Stat(runPath)
	if err != nil || !runInfo.IsDir() {
		t.Fatalf("runtime layout = %v, %v", runInfo, err)
	}
	config, err := os.ReadFile(paths.Config())
	if err != nil {
		t.Fatal(err)
	}
	if string(config) != testConfig {
		t.Fatalf("config = %q, want %q", config, testConfig)
	}
	if runtime.GOOS != "windows" {
		for path, expected := range map[string]os.FileMode{
			paths.Root():   0o700,
			runPath:        0o700,
			paths.Config(): 0o600,
		} {
			info, err := os.Stat(path)
			if err != nil {
				t.Fatal(err)
			}
			if info.Mode().Perm() != expected {
				t.Fatalf("%s permissions = %#o, want %#o", path, info.Mode().Perm(), expected)
			}
		}
	}

	custom := []byte("provider: custom\n")
	if err := os.WriteFile(paths.Config(), custom, 0o600); err != nil {
		t.Fatalf("custom config: %v", err)
	}
	reopened, err := paths.Open()
	if err != nil {
		t.Fatalf("preserving init: %v", err)
	}
	if err := reopened.Close(); err != nil {
		t.Fatal(err)
	}
	got, err := os.ReadFile(paths.Config())
	if err != nil || string(got) != string(custom) {
		t.Fatalf("existing config = %q, error = %v", got, err)
	}

	noConfig, err := FromRoot(filepath.Join(t.TempDir(), "no-config"))
	if err != nil {
		t.Fatal(err)
	}
	noConfigRoot, err := noConfig.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = noConfigRoot.Close() })
	if _, err := os.Stat(noConfig.Root()); err != nil {
		t.Fatalf("no-config home: %v", err)
	}
	if _, err := os.Stat(filepath.Join(noConfig.Root(), "run")); err != nil {
		t.Fatalf("no-config runtime layout: %v", err)
	}
	if _, err := os.Stat(noConfig.Config()); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("no-config config error = %v", err)
	}
}

// Rust source: crates/noema-home/src/paths.rs:516::noema_paths_preserve_environment_layout_digest_and_confinement_contracts
func TestRustHome_noema_paths_preserve_environment_layout_digest_and_confinement_contracts(t *testing.T) {
	customRoot := filepath.Join(t.TempDir(), "custom-noema")
	override, err := FromRoot(customRoot)
	if err != nil {
		t.Fatal(err)
	}
	abs, err := filepath.Abs(customRoot)
	if err != nil {
		t.Fatal(err)
	}
	if override.Root() != abs {
		t.Errorf("root = %q, want %q", override.Root(), abs)
	}
	if override.Config() != filepath.Join(abs, "config.yaml") {
		t.Errorf("config = %q, want %q", override.Config(), filepath.Join(abs, "config.yaml"))
	}
	if override.LocalModelImportPartialPath("install:public/model") != filepath.Join(abs, "models", "downloads", "import-install_public_model.part") {
		t.Errorf("local model import partial = %q", override.LocalModelImportPartialPath("install:public/model"))
	}
	if override.ErrorsLog() != filepath.Join(abs, "errors.log") {
		t.Errorf("errors log = %q, want %q", override.ErrorsLog(), filepath.Join(abs, "errors.log"))
	}
	if override.Database() != filepath.Join(abs, "db", "noema.sqlite3") {
		t.Errorf("database = %q, want %q", override.Database(), filepath.Join(abs, "db", "noema.sqlite3"))
	}
	if override.BrowserSessionKey() != filepath.Join(abs, "run", "browser-session.key") {
		t.Errorf("browser session key = %q, want %q", override.BrowserSessionKey(), filepath.Join(abs, "run", "browser-session.key"))
	}
	if override.NativeOAuthRetries() != filepath.Join(abs, "run", "native-oauth-retries.json") {
		t.Errorf("native OAuth retries = %q, want %q", override.NativeOAuthRetries(), filepath.Join(abs, "run", "native-oauth-retries.json"))
	}
	if override.WebPushVAPID() != filepath.Join(abs, "notifications", "web-push-vapid.json") {
		t.Errorf("Web Push VAPID = %q, want %q", override.WebPushVAPID(), filepath.Join(abs, "notifications", "web-push-vapid.json"))
	}
	if override.APNSProvider() != filepath.Join(abs, "notifications", "apns-provider.json") {
		t.Errorf("APNs provider = %q, want %q", override.APNSProvider(), filepath.Join(abs, "notifications", "apns-provider.json"))
	}
	if override.CapabilityAuthArgumentsDir() != filepath.Join(abs, "run", "capability-auth") {
		t.Errorf("capability auth arguments = %q", override.CapabilityAuthArgumentsDir())
	}

	digest64 := strings.Repeat("b", 64)
	definition, err := override.AdapterDefinitionDir(digest64)
	if err != nil || definition != filepath.Join(abs, "adapters", "definitions", digest64) {
		t.Errorf("adapter definition = %q, %v", definition, err)
	}
	sourceDigest := strings.Repeat("c", 64)
	source, err := override.AdapterSourcePath(sourceDigest, "yaml")
	if err != nil || source != filepath.Join(abs, "adapters", "sources", sourceDigest+".yaml") {
		t.Errorf("adapter source = %q, %v", source, err)
	}
	if _, err := override.AdapterDefinitionDir("../escape"); err == nil {
		t.Error("adapter definition traversal was accepted")
	}
	connectionID := strings.Repeat("e", 32)
	connection, err := override.AdapterConnectionDir(connectionID)
	if err != nil || connection != filepath.Join(abs, "adapters", "connections", connectionID) {
		t.Errorf("adapter connection = %q, %v", connection, err)
	}
	if _, err := override.AdapterConnectionDir("../escape"); err == nil {
		t.Error("adapter connection traversal was accepted")
	}
	profileDigest := strings.Repeat("f", 64)
	profile, err := override.AdapterOAuthProfileDir(profileDigest)
	if err != nil || profile != filepath.Join(abs, "adapters", "oauth-profiles", profileDigest) {
		t.Errorf("adapter OAuth profile = %q, %v", profile, err)
	}
	for _, test := range []struct {
		name string
		path func() (string, error)
		want string
	}{
		{"application", func() (string, error) { return override.AdapterOAuthApplicationDir(strings.Repeat("1", 32)) }, filepath.Join(abs, "adapters", "oauth-applications")},
		{"external account", func() (string, error) { return override.AdapterExternalAccountDir(strings.Repeat("2", 32)) }, filepath.Join(abs, "adapters", "external-accounts")},
		{"grant", func() (string, error) { return override.AdapterOAuthGrantDir(strings.Repeat("3", 32)) }, filepath.Join(abs, "adapters", "oauth-grants")},
	} {
		path, err := test.path()
		if err != nil || !strings.HasPrefix(path, test.want+string(filepath.Separator)) {
			t.Errorf("adapter %s = %q, %v", test.name, path, err)
		}
	}
	if _, err := override.AdapterOAuthGrantDir("../escape"); err == nil {
		t.Error("adapter OAuth grant traversal was accepted")
	}
	if _, err := override.AdapterSourcePath(strings.Repeat("d", 64), "rs"); err == nil {
		t.Error("adapter source executable extension was accepted")
	}

	fallbackHome := filepath.Join(t.TempDir(), "home")
	fallback, err := resolve(
		func(string) (string, bool) { return "", false },
		func() (string, error) { return fallbackHome, nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	if fallback.Root() != filepath.Join(fallbackHome, ".noema") {
		t.Errorf("fallback root = %q, want %q", fallback.Root(), filepath.Join(fallbackHome, ".noema"))
	}
	if fallback.Config() != filepath.Join(fallback.Root(), "config.yaml") {
		t.Errorf("fallback config = %q, want %q", fallback.Config(), filepath.Join(fallback.Root(), "config.yaml"))
	}
	if _, err := resolve(
		func(string) (string, bool) { return "", false },
		func() (string, error) { return "", nil },
	); err == nil {
		t.Error("empty fallback home was accepted")
	}
	if _, err := resolve(
		func(string) (string, bool) { return "", true },
		func() (string, error) { return "", errors.New("must not run") },
	); err == nil {
		t.Error("empty explicit home was accepted")
	}

	paths, err := FromRoot(filepath.Join(t.TempDir(), "noema"))
	if err != nil {
		t.Fatal(err)
	}
	digest := strings.Repeat("a", 64)
	blob, err := paths.LocalModelBlobPath(digest)
	if err != nil || blob != filepath.Join(paths.Root(), "models", "blobs", digest+".gguf") {
		t.Errorf("local model blob = %q, %v", blob, err)
	}
	partial, err := paths.LocalModelPartialPath(digest)
	if err != nil || partial != filepath.Join(paths.Root(), "models", "downloads", digest+".part") {
		t.Errorf("local model partial = %q, %v", partial, err)
	}
	for _, invalidDigest := range []string{"../model", strings.Repeat("A", 64), strings.Repeat("g", 64)} {
		if _, err := paths.LocalModelBlobPath(invalidDigest); err == nil {
			t.Errorf("invalid model digest was accepted: %q", invalidDigest)
		}
	}
	if got := paths.ProviderAccountHome("codex", "default"); got != filepath.Join(paths.Root(), "providers", "codex", "default") {
		t.Errorf("provider account home = %q", got)
	}
	if got := paths.ProviderAccountHome("co/dex", "../default"); got != filepath.Join(paths.Root(), "providers", "co_dex", "___default") {
		t.Errorf("sanitized provider account home = %q", got)
	}
	if got := paths.ProviderAccountHome("", ""); got != filepath.Join(paths.Root(), "providers", "_", "_") {
		t.Errorf("empty provider account home = %q", got)
	}
	if got := paths.MCPDir(); got != filepath.Join(paths.Root(), "mcp") {
		t.Errorf("MCP directory = %q", got)
	}
	if got := paths.MCPServerHome("mcp:GitHub/Default"); got != filepath.Join(paths.Root(), "mcp", "mcp_GitHub_Default") {
		t.Errorf("MCP server home = %q", got)
	}

	root, err := override.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	if err := root.WriteFile("inside.txt", []byte("inside"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(override.Root(), "inside.txt")); err != nil {
		t.Fatal(err)
	}
	if err := root.WriteFile("../outside.txt", []byte("outside"), 0o600); err == nil {
		t.Error("root escape was accepted")
	}
	if _, err := os.Stat(filepath.Join(filepath.Dir(override.Root()), "outside.txt")); !errors.Is(err, os.ErrNotExist) {
		t.Errorf("outside file exists or has unexpected error: %v", err)
	}
}

// Rust source: crates/noema-home/src/private_files.rs:249::atomic_replacement_is_private_and_leaves_no_temporary_file
func TestRustHome_atomic_replacement_is_private_and_leaves_no_temporary_file(t *testing.T) {
	directory := filepath.Join(t.TempDir(), "private")
	path := filepath.Join(directory, "secret")
	if err := AtomicWritePrivate(path, []byte("first")); err != nil {
		t.Fatalf("first write: %v", err)
	}
	if err := AtomicWritePrivate(path, []byte("second")); err != nil {
		t.Fatalf("second write: %v", err)
	}
	if got, err := os.ReadFile(path); err != nil || string(got) != "second" {
		t.Fatalf("secret = %q, error = %v", got, err)
	}
	entries, err := os.ReadDir(directory)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 {
		t.Fatalf("private entries = %d, want 1", len(entries))
	}
	if runtime.GOOS != "windows" {
		directoryInfo, err := os.Stat(directory)
		if err != nil {
			t.Fatal(err)
		}
		if directoryInfo.Mode().Perm() != 0o700 {
			t.Fatalf("directory permissions = %#o, want 0700", directoryInfo.Mode().Perm())
		}
		fileInfo, err := os.Stat(path)
		if err != nil {
			t.Fatal(err)
		}
		if fileInfo.Mode().Perm() != 0o600 {
			t.Fatalf("file permissions = %#o, want 0600", fileInfo.Mode().Perm())
		}
		if err := os.Chmod(path, 0o644); err != nil {
			t.Fatal(err)
		}
		if err := ProtectFile(path); err != nil {
			t.Fatalf("restore private file: %v", err)
		}
		fileInfo, err = os.Stat(path)
		if err != nil {
			t.Fatal(err)
		}
		if fileInfo.Mode().Perm() != 0o600 {
			t.Fatalf("restored file permissions = %#o, want 0600", fileInfo.Mode().Perm())
		}
	}
}
