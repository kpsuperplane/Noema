package home

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

// Rust source: crates/noema-home/src/initialization.rs:85::initialization_creates_layout_and_honors_config_write_policy
func TestRustHome_initialization_creates_layout_and_honors_config_write_policy(t *testing.T) {
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
	if runtime.GOOS != "windows" {
		info, err := os.Stat(paths.Root())
		if err != nil {
			t.Fatal(err)
		}
		if info.Mode().Perm() != 0o700 {
			t.Fatalf("home permissions = %#o, want 0700", info.Mode().Perm())
		}
	}

	// Go startup keeps configuration writes in auth.LoadConfig. The home layer
	// owns the same protected path and leaves an existing file untouched.
	initial := []byte("provider: test\n")
	if err := AtomicWritePrivate(paths.Config(), initial); err != nil {
		t.Fatalf("write initial config: %v", err)
	}
	custom := []byte("provider: custom\n")
	if err := os.WriteFile(paths.Config(), custom, 0o600); err != nil {
		t.Fatalf("write custom config: %v", err)
	}
	reopened, err := paths.Open()
	if err != nil {
		t.Fatalf("reopen home: %v", err)
	}
	if err := reopened.Close(); err != nil {
		t.Fatalf("close reopened home: %v", err)
	}
	got, err := os.ReadFile(paths.Config())
	if err != nil || string(got) != string(custom) {
		t.Fatalf("existing config = %q, error = %v", got, err)
	}

	// The Go home creates runtime subdirectories when their protected files are
	// first written. This is the approved Go equivalent of Rust's eager run/dir.
	if err := AtomicWritePrivate(paths.BrowserSessionKey(), []byte("session-key")); err != nil {
		t.Fatalf("write runtime key: %v", err)
	}
	runInfo, err := os.Stat(filepath.Dir(paths.BrowserSessionKey()))
	if err != nil || !runInfo.IsDir() {
		t.Fatalf("runtime layout = %v, %v", runInfo, err)
	}
	if runtime.GOOS != "windows" {
		if runInfo.Mode().Perm() != 0o700 {
			t.Fatalf("runtime permissions = %#o, want 0700", runInfo.Mode().Perm())
		}
		configInfo, err := os.Stat(paths.Config())
		if err != nil {
			t.Fatal(err)
		}
		if configInfo.Mode().Perm() != 0o600 {
			t.Fatalf("config permissions = %#o, want 0600", configInfo.Mode().Perm())
		}
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
		t.Fatalf("root = %q, want %q", override.Root(), abs)
	}
	if override.Config() != filepath.Join(abs, "config.yaml") {
		t.Fatalf("config = %q", override.Config())
	}
	if override.ErrorsLog() != filepath.Join(abs, "errors.log") {
		t.Fatalf("errors log = %q", override.ErrorsLog())
	}

	// Go stores the database at the home root. Rust used root/db/noema.sqlite3.
	if override.Database() != filepath.Join(abs, "noema.sqlite3") {
		t.Fatalf("database = %q", override.Database())
	}
	if override.BrowserSessionKey() != filepath.Join(abs, "run", "browser-session.key") {
		t.Fatalf("browser session key = %q", override.BrowserSessionKey())
	}
	if override.NativeOAuthRetries() != filepath.Join(abs, "run", "native-oauth-retries.json") {
		t.Fatalf("native OAuth retries = %q", override.NativeOAuthRetries())
	}
	if override.WebPushVAPID() != filepath.Join(abs, "notifications", "web-push-vapid.json") {
		t.Fatalf("Web Push VAPID = %q", override.WebPushVAPID())
	}
	if override.APNSProvider() != filepath.Join(abs, "notifications", "apns-provider.json") {
		t.Fatalf("APNs provider = %q", override.APNSProvider())
	}

	fallback, err := resolve(
		func(string) (string, bool) { return "", false },
		func() (string, error) { return filepath.Join(t.TempDir(), "home"), nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	if filepath.Base(fallback.Root()) != ".noema" || fallback.Config() != filepath.Join(fallback.Root(), "config.yaml") {
		t.Fatalf("fallback paths = %#v", fallback)
	}
	if _, err := resolve(
		func(string) (string, bool) { return "", false },
		func() (string, error) { return "", nil },
	); err == nil {
		t.Fatal("empty fallback home was accepted")
	}
	if _, err := resolve(
		func(string) (string, bool) { return "", true },
		func() (string, error) { return "", errors.New("must not run") },
	); err == nil {
		t.Fatal("empty explicit home was accepted")
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
