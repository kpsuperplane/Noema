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

	"github.com/kpsuperplane/noema/internal/store"
)

func newMaterializationParityService(t *testing.T) (*Service, string) {
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
	return service, home
}

func materializationParityRequest(root string, bytesValue []byte) MaterializeVerifiedEvalModelRequest {
	digestBytes := sha256.Sum256(bytesValue)
	return MaterializeVerifiedEvalModelRequest{
		Source:        VerifiedEvalModelSource{Repo: "owner/repository", Revision: "a" + "000000000000000000000000000000000000000", File: "model.gguf"},
		ExpectedBytes: int64(len(bytesValue)),
		SHA256:        hex.EncodeToString(digestBytes[:]),
		CacheRoot:     root,
	}
}

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::digest_mismatch_removes_partial_and_publishes_nothing (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_DigestMismatchRemovesPartialAndPublishesNothing(t *testing.T) {
	service, root := newMaterializationParityService(t)
	expected := []byte("GGUF expected evaluation model")
	request := materializationParityRequest(root, expected)
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) {
		// Flush before writing so the response has no fixed content length. This
		// reaches the pinned digest check with the Rust fixture's bytes.
		writer.WriteHeader(http.StatusOK)
		if flusher, ok := writer.(http.Flusher); ok {
			flusher.Flush()
		}
		_, _ = writer.Write([]byte("GGUF incorrect evaluation byte"))
	}))
	t.Cleanup(server.Close)
	_, err := service.materializeVerifiedEvalModelFromURL(t.Context(), request, server.URL)
	if err == nil || err.Error() != "downloaded artifact did not match its pinned SHA-256" {
		t.Fatalf("checksum error = %v", err)
	}
	destination := filepath.Join(root, request.SHA256+".gguf")
	if _, statErr := os.Stat(destination); !os.IsNotExist(statErr) {
		t.Fatalf("published destination = %v", statErr)
	}
	entries, readErr := os.ReadDir(root)
	if readErr != nil {
		t.Fatal(readErr)
	}
	for _, entry := range entries {
		if strings.HasSuffix(entry.Name(), ".partial") {
			t.Fatalf("partial cache entry = %q", entry.Name())
		}
	}
}

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::successful_download_is_atomic_and_store_free (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_SuccessfulDownloadIsAtomicAndStoreFree(t *testing.T) {
	service, root := newMaterializationParityService(t)
	bytesValue := []byte("GGUF successful evaluation model")
	request := materializationParityRequest(root, bytesValue)
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) {
		_, _ = writer.Write(bytesValue)
	}))
	t.Cleanup(server.Close)
	path, err := service.materializeVerifiedEvalModelFromURL(t.Context(), request, server.URL)
	if err != nil {
		t.Fatal(err)
	}
	expectedPath := filepath.Join(root, request.SHA256+".gguf")
	if path != expectedPath {
		t.Fatalf("materialized path = %q, want %q", path, expectedPath)
	}
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
