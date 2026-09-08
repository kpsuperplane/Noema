package mcp

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestProtectedSecretsAndStdioDoubleOptIn(t *testing.T) {
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(paths.Database()), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := home.AtomicWritePrivate(paths.Config(), []byte("mcp:\n  stdio_enabled: true\n")); err != nil {
		t.Fatal(err)
	}
	enabled, err := StdioEnabled(paths)
	if err != nil || !enabled {
		t.Fatalf("stdio enabled = %t, %v", enabled, err)
	}
	t.Setenv("NOEMA_MCP__STDIO_ENABLED", "false")
	enabled, err = StdioEnabled(paths)
	if err != nil || enabled {
		t.Fatalf("stdio override = %t, %v", enabled, err)
	}
	database, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	service, err := NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	secret := SecretMaterial{Headers: map[string]string{"Authorization": "private-token-value"}, Revision: strings.Repeat("a", 32)}
	serverID := "mcp_server:" + strings.Repeat("1", 32)
	if err := service.secrets.writeConnection(serverID, secret); err != nil {
		t.Fatal(err)
	}
	if strings.Contains(fmt.Sprintf("%v %#v", secret, secret), "private-token-value") {
		t.Fatal("secret formatter exposed credential")
	}
	credentialPath := filepath.Join(paths.Root(), "mcp", serverID, "credentials.json")
	if runtime.GOOS != "windows" {
		info, err := os.Stat(credentialPath)
		if err != nil {
			t.Fatal(err)
		}
		if info.Mode().Perm() != 0o600 {
			t.Fatalf("credential mode = %o", info.Mode().Perm())
		}
	}
	data, err := os.ReadFile(paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(data), "private-token-value") {
		t.Fatal("SQLite contains MCP credential")
	}
}
