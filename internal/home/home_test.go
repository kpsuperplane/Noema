package home

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestResolveUsesExplicitHome(t *testing.T) {
	want := filepath.Join(t.TempDir(), "state")
	paths, err := resolve(
		func(name string) (string, bool) {
			if name != EnvironmentName {
				t.Fatalf("unexpected environment name %q", name)
			}
			return want, true
		},
		func() (string, error) {
			t.Fatal("user home must not be read")
			return "", nil
		},
	)
	if err != nil {
		t.Fatalf("resolve explicit home: %v", err)
	}

	abs, err := filepath.Abs(want)
	if err != nil {
		t.Fatalf("resolve expected path: %v", err)
	}
	if paths.Root() != abs {
		t.Fatalf("root = %q, want %q", paths.Root(), abs)
	}
}

func TestResolveRejectsEmptySources(t *testing.T) {
	_, err := resolve(
		func(string) (string, bool) { return "", true },
		func() (string, error) { return "", errors.New("must not run") },
	)
	if err == nil {
		t.Fatal("empty NOEMA_HOME must fail")
	}

	_, err = resolve(
		func(string) (string, bool) { return "", false },
		func() (string, error) { return "", nil },
	)
	if err == nil {
		t.Fatal("empty user home must fail")
	}
}

func TestOpenConfinesFileAccess(t *testing.T) {
	parent := t.TempDir()
	paths, err := FromRoot(filepath.Join(parent, "home"))
	if err != nil {
		t.Fatalf("resolve home: %v", err)
	}
	if err := os.MkdirAll(paths.Root(), 0o777); err != nil {
		t.Fatalf("create broad home: %v", err)
	}
	if err := os.Chmod(paths.Root(), 0o777); err != nil {
		t.Fatalf("set broad home mode: %v", err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatalf("open home: %v", err)
	}
	t.Cleanup(func() {
		if err := root.Close(); err != nil {
			t.Errorf("close home: %v", err)
		}
	})
	if runtime.GOOS != "windows" {
		info, err := os.Stat(paths.Root())
		if err != nil {
			t.Fatalf("inspect home: %v", err)
		}
		if info.Mode().Perm() != 0o700 {
			t.Fatalf("home permissions = %#o, want 0700", info.Mode().Perm())
		}
	}

	if err := root.WriteFile("inside.txt", []byte("inside"), 0o600); err != nil {
		t.Fatalf("write inside home: %v", err)
	}
	if _, err := os.Stat(filepath.Join(paths.Root(), "inside.txt")); err != nil {
		t.Fatalf("inspect inside file: %v", err)
	}

	if err := root.WriteFile("../outside.txt", []byte("outside"), 0o600); err == nil {
		t.Fatal("root escape must fail")
	}
	if _, err := os.Stat(filepath.Join(parent, "outside.txt")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("outside file exists or has unexpected error: %v", err)
	}
}
