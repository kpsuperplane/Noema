package provider_test

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/localmodel"
	"github.com/kpsuperplane/noema/internal/store"
)

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::cache_hit_returns_verified_path_without_network (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CacheHitReturnsVerifiedPathWithoutNetwork(t *testing.T) {
	ctx := context.Background()
	home := t.TempDir()
	databaseRoot := t.TempDir()
	database, err := store.Open(ctx, filepath.Join(databaseRoot, "noema.sqlite3"))
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
	cacheRoot := filepath.Join(home, "evaluation-cache")
	destination := filepath.Join(cacheRoot, digest+".gguf")
	if err := os.MkdirAll(filepath.Dir(destination), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(destination, bytesValue, 0o600); err != nil {
		t.Fatal(err)
	}
	request := localmodel.MaterializeVerifiedEvalModelRequest{
		Source: localmodel.VerifiedEvalModelSource{
			Repo: "owner/repository", Revision: "a" + "000000000000000000000000000000000000000", File: "model.gguf",
		},
		ExpectedBytes: int64(len(bytesValue)),
		SHA256:        digest,
		CacheRoot:     cacheRoot,
	}
	if !strings.Contains(fmt.Sprintf("%+v", request), cacheRoot) {
		t.Fatal("debug omitted the ordinary cache path")
	}
	path, err := service.MaterializeVerifiedEvalModel(ctx, request)
	if err != nil {
		t.Fatal(err)
	}
	if path != destination {
		t.Fatalf("cache hit path = %q, want %q", path, destination)
	}
	got, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(got, bytesValue) || path != destination {
		t.Fatalf("cache hit = %q, %v", path, err)
	}
}
