package localmodel

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/store"
)

func newMaterializationParityService(t *testing.T) (*Service, *store.Store, string) {
	t.Helper()
	home := t.TempDir()
	databaseRoot := t.TempDir()
	database, err := store.Open(context.Background(), filepath.Join(databaseRoot, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	service, err := New(database, home, "")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	return service, database, home
}

func queueMaterializationParityInstallation(t *testing.T, database *store.Store, id string, total int64) store.LocalModelInstallation {
	t.Helper()
	value, err := database.QueueLocalModel(context.Background(), store.LocalModelInstallation{
		ID: id, ModelID: id, Name: id, File: id + ".gguf", SourceKind: "public_gguf",
		Backend: "cpu", TotalBytes: total, CreatedAt: time.Now().UTC(),
	})
	if err != nil {
		t.Fatal(err)
	}
	return value
}

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::digest_mismatch_removes_partial_and_publishes_nothing (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DigestMismatchRemovesPartialAndPublishesNothing(t *testing.T) {
	service, database, root := newMaterializationParityService(t)
	expected := []byte("GGUF expected evaluation model")
	digestBytes := sha256.Sum256(expected)
	digest := hex.EncodeToString(digestBytes[:])
	queued := queueMaterializationParityInstallation(t, database, "installation:digest-mismatch", int64(len(expected)))
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) {
		_, _ = writer.Write([]byte("GGUF incorrect evaluation bytes"))
	}))
	t.Cleanup(server.Close)
	err := service.download(t.Context(), queued, server.URL, digest, false)
	if err == nil || !strings.Contains(err.Error(), "digest mismatch") {
		t.Fatalf("checksum error = %v", err)
	}
	if entries, readErr := os.ReadDir(filepath.Join(root, "system", "tmp")); readErr != nil || len(entries) != 0 {
		t.Fatalf("partial cache entries = %v, %v", entries, readErr)
	}
}

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::successful_download_is_atomic_and_store_free (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SuccessfulDownloadIsAtomicAndStoreFree(t *testing.T) {
	service, database, root := newMaterializationParityService(t)
	bytesValue := []byte("GGUF successful evaluation model")
	digestBytes := sha256.Sum256(bytesValue)
	digest := hex.EncodeToString(digestBytes[:])
	queued := queueMaterializationParityInstallation(t, database, "installation:successful", int64(len(bytesValue)))
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) {
		_, _ = writer.Write(bytesValue)
	}))
	t.Cleanup(server.Close)
	if err := service.download(t.Context(), queued, server.URL, digest, false); err != nil {
		t.Fatal(err)
	}
	installed, err := database.LocalModelInstallation(t.Context(), queued.ID)
	if err != nil || installed.Status != "installed" || installed.SHA256 != digest {
		t.Fatalf("materialized installation = %#v, %v", installed, err)
	}
	path := filepath.Join(root, filepath.FromSlash(installed.BlobPath))
	got, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(got, bytesValue) {
		t.Fatalf("materialized bytes = %q, %v", got, err)
	}
	err = filepath.WalkDir(root, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() {
			return nil
		}
		name := entry.Name()
		if strings.HasSuffix(name, ".partial") || strings.Contains(name, "sqlite") {
			t.Fatalf("non-atomic cache entry = %q", path)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
}
