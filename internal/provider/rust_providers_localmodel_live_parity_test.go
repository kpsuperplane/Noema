package provider_test

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/localmodel"
	"github.com/kpsuperplane/noema/internal/store"
)

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::cache_hit_returns_verified_path_without_network (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CacheHitReturnsVerifiedPathWithoutNetwork(t *testing.T) {
	ctx := context.Background()
	home := t.TempDir()
	database, err := store.Open(ctx, filepath.Join(home, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	service, err := localmodel.New(database, home, "")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)

	bytesValue := []byte("GGUF cached evaluation model")
	digestBytes := sha256.Sum256(bytesValue)
	digest := hex.EncodeToString(digestBytes[:])
	destination := filepath.Join(home, "models", "blobs", digest+".gguf")
	if err := os.MkdirAll(filepath.Dir(destination), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(destination, bytesValue, 0o600); err != nil {
		t.Fatal(err)
	}
	source := filepath.Join(home, "cached-source.gguf")
	if err := os.WriteFile(source, bytesValue, 0o600); err != nil {
		t.Fatal(err)
	}
	queued, err := service.Import(ctx, localmodel.ImportInput{
		Name: "Cached evaluation model", SourceKind: "local_file", LocalPath: source,
	})
	if err != nil {
		t.Fatal(err)
	}
	deadline := time.Now().Add(3 * time.Second)
	var installed store.LocalModelInstallation
	for time.Now().Before(deadline) {
		installed, err = database.LocalModelInstallation(ctx, queued.ID)
		if err == nil && (installed.Status == "installed" || installed.Status == "failed" || installed.Status == "cancelled") {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	if err != nil || installed.Status != "installed" || installed.SHA256 != digest {
		t.Fatalf("cache hit installation = %#v, %v", installed, err)
	}
	if installed.BlobPath != filepath.ToSlash(filepath.Join("models", "blobs", digest+".gguf")) {
		t.Fatalf("cache hit blob path = %q", installed.BlobPath)
	}
	path := filepath.Join(home, filepath.FromSlash(installed.BlobPath))
	got, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(got, bytesValue) || path != destination {
		t.Fatalf("cache hit = %q, %v", path, err)
	}
}
