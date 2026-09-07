package provider_test

import (
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

func newProviderBlobService(t *testing.T) (*localmodel.Service, *store.Store, string) {
	t.Helper()
	home := t.TempDir()
	database, err := store.Open(context.Background(), filepath.Join(home, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	service, err := localmodel.New(database, home, "")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	return service, database, home
}

func seedProviderBlobInstallation(t *testing.T, database *store.Store, id, modelID, digest, blobPath string, size int64) store.LocalModelInstallation {
	t.Helper()
	now := time.Now().UTC()
	value, err := database.QueueLocalModel(context.Background(), store.LocalModelInstallation{
		ID: id, ModelID: modelID, Name: modelID, File: modelID + ".gguf", SourceKind: "local_file",
		Backend: "cpu", TotalBytes: size, CreatedAt: now,
	})
	if err != nil {
		t.Fatal(err)
	}
	for _, status := range []string{"downloading", "verifying"} {
		value, err = database.UpdateLocalModel(context.Background(), value.ID, status, size, size, 0, "", "", "", "", now)
		if err != nil {
			t.Fatal(err)
		}
	}
	value, err = database.UpdateLocalModel(context.Background(), value.ID, "installed", size, size, size, digest, blobPath, "", "", now)
	if err != nil {
		t.Fatal(err)
	}
	return value
}

// Rust source: crates/noema-providers/src/local_models/manager/process.rs::verified_model_blob_path_accepts_canonical_untampered_blob (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_VerifiedModelBlobPathAcceptsCanonicalUntamperedBlob(t *testing.T) {
	service, database, root := newProviderBlobService(t)
	bytesValue := []byte("verified model bytes")
	digestBytes := sha256.Sum256(bytesValue)
	digest := hex.EncodeToString(digestBytes[:])
	blob := filepath.Join(root, "models", "blobs", digest+".gguf")
	if err := os.MkdirAll(filepath.Dir(blob), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blob, bytesValue, 0o600); err != nil {
		t.Fatal(err)
	}
	installation := seedProviderBlobInstallation(t, database, "installation:verified", "verified", digest, filepath.ToSlash(filepath.Join("models", "blobs", digest+".gguf")), int64(len(bytesValue)))
	verified, err := service.VerifiedModelBlobPath(context.Background(), installation.ID)
	if err != nil || verified != blob {
		t.Fatalf("verified model blob = %q, %v", verified, err)
	}
}

// Rust source: crates/noema-providers/src/local_models/manager/process.rs::verified_model_blob_path_rejects_missing_and_tampered_blob (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_VerifiedModelBlobPathRejectsMissingAndTamperedBlob(t *testing.T) {
	service, database, root := newProviderBlobService(t)
	bytesValue := []byte("verified model bytes")
	digestBytes := sha256.Sum256(bytesValue)
	digest := hex.EncodeToString(digestBytes[:])
	missing := seedProviderBlobInstallation(t, database, "installation:missing", "missing", digest, filepath.ToSlash(filepath.Join("models", "blobs", digest+".gguf")), int64(len(bytesValue)))
	if _, err := service.VerifiedModelBlobPath(context.Background(), missing.ID); err == nil || err.Error() != "installed model blob is unavailable" {
		t.Fatalf("missing blob error = %v", err)
	}
	blob := filepath.Join(root, "models", "blobs", digest+".gguf")
	if err := os.MkdirAll(filepath.Dir(blob), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blob, []byte("tampered model bytes"), 0o600); err != nil {
		t.Fatal(err)
	}
	tampered := seedProviderBlobInstallation(t, database, "installation:tampered", "tampered", digest, filepath.ToSlash(filepath.Join("models", "blobs", digest+".gguf")), int64(len(bytesValue)))
	if _, err := service.VerifiedModelBlobPath(context.Background(), tampered.ID); err == nil || err.Error() != "installed model blob digest does not match its verified digest" {
		t.Fatalf("tampered blob error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/local_models/manager/process.rs::verified_model_blob_path_rejects_noncanonical_durable_path (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_VerifiedModelBlobPathRejectsNoncanonicalDurablePath(t *testing.T) {
	service, database, root := newProviderBlobService(t)
	bytesValue := []byte("verified model bytes")
	digestBytes := sha256.Sum256(bytesValue)
	digest := hex.EncodeToString(digestBytes[:])
	errText := "models/blobs/other.gguf"
	installation := seedProviderBlobInstallation(t, database, "installation:noncanonical", "noncanonical", digest, errText, int64(len(bytesValue)))
	if _, err := service.VerifiedModelBlobPath(context.Background(), installation.ID); err == nil || err.Error() != "verified blob path does not match its digest" {
		t.Fatalf("noncanonical path error = %v", err)
	}
	_ = root
}
