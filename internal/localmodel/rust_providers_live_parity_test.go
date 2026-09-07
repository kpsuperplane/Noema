package localmodel

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/store"
)

func waitForTerminalInstallation(t *testing.T, database interface {
	LocalModelInstallation(context.Context, string) (store.LocalModelInstallation, error)
}, id string) store.LocalModelInstallation {
	t.Helper()
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		value, err := database.LocalModelInstallation(t.Context(), id)
		if err == nil && (value.Status == "installed" || value.Status == "failed" || value.Status == "cancelled") {
			return value
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatal("installation did not reach a terminal state")
	return store.LocalModelInstallation{}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::local_file_import_is_verified_and_content_addressed (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LocalFileImportIsVerifiedAndContentAddressed(t *testing.T) {
	service, database, home := newTestService(t)
	source := filepath.Join(home, "user-model.gguf")
	payload := []byte("GGUF small deterministic test payload")
	if err := os.WriteFile(source, payload, 0o600); err != nil {
		t.Fatal(err)
	}
	queued, err := service.Import(t.Context(), ImportInput{Name: "User model", SourceKind: "local_file", LocalPath: source})
	if err != nil {
		t.Fatal(err)
	}
	installed := waitForInstallation(t, database, queued.ID)
	sum := sha256.Sum256(payload)
	digest := hex.EncodeToString(sum[:])
	if installed.Status != "installed" || installed.SHA256 != digest {
		t.Fatalf("local import = %#v", installed)
	}
	blob := filepath.Join(home, filepath.FromSlash(installed.BlobPath))
	got, err := os.ReadFile(blob)
	if err != nil || !bytes.Equal(got, payload) {
		t.Fatalf("installed blob = %q, %v", got, err)
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::verified_import_preserves_a_corrupt_shared_blob_and_fails_closed (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_VerifiedImportPreservesACorruptSharedBlobAndFailsClosed(t *testing.T) {
	service, database, home := newTestService(t)
	payload := []byte("GGUF same verified payload")
	first, second := filepath.Join(home, "first.gguf"), filepath.Join(home, "second.gguf")
	for _, path := range []string{first, second} {
		if err := os.WriteFile(path, payload, 0o600); err != nil {
			t.Fatal(err)
		}
	}
	queued, err := service.Import(t.Context(), ImportInput{Name: "First", SourceKind: "local_file", LocalPath: first})
	if err != nil {
		t.Fatal(err)
	}
	installed := waitForInstallation(t, database, queued.ID)
	blob := filepath.Join(home, filepath.FromSlash(installed.BlobPath))
	if err := os.WriteFile(blob, []byte("corrupt"), 0o600); err != nil {
		t.Fatal(err)
	}
	secondQueued, err := service.Import(t.Context(), ImportInput{Name: "Second", SourceKind: "local_file", LocalPath: second})
	if err != nil {
		t.Fatal(err)
	}
	failed := waitForTerminalInstallation(t, database, secondQueued.ID)
	if failed.Status != "failed" || !strings.Contains(failed.ErrorMessage, "blob digest conflict") {
		t.Fatalf("occupied corrupt blob error = %#v", failed)
	}
	got, err := os.ReadFile(blob)
	if err != nil || string(got) != "corrupt" {
		t.Fatalf("corrupt shared blob = %q, %v", got, err)
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::verified_import_repairs_an_unreferenced_corrupt_blob (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_VerifiedImportRepairsAnUnreferencedCorruptBlob(t *testing.T) {
	service, database, home := newTestService(t)
	payload := []byte("GGUF orphan recovery payload")
	source := filepath.Join(home, "recovery.gguf")
	if err := os.WriteFile(source, payload, 0o600); err != nil {
		t.Fatal(err)
	}
	sum := sha256.Sum256(payload)
	digest := hex.EncodeToString(sum[:])
	blob := filepath.Join(home, "models", "blobs", digest+".gguf")
	if err := os.MkdirAll(filepath.Dir(blob), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blob, []byte("orphaned corrupt bytes"), 0o600); err != nil {
		t.Fatal(err)
	}
	queued, err := service.Import(t.Context(), ImportInput{Name: "Recovered", SourceKind: "local_file", LocalPath: source})
	if err != nil {
		t.Fatal(err)
	}
	installed := waitForInstallation(t, database, queued.ID)
	got, err := os.ReadFile(blob)
	if err != nil || installed.Status != "installed" || !bytes.Equal(got, payload) {
		t.Fatalf("orphan recovery = %#v, %q, %v", installed, got, err)
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::local_file_import_rejects_a_non_gguf_payload (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LocalFileImportRejectsANonGgufPayload(t *testing.T) {
	service, _, home := newTestService(t)
	source := filepath.Join(home, "not-a-model.gguf")
	if err := os.WriteFile(source, []byte("this is not a model"), 0o600); err != nil {
		t.Fatal(err)
	}
	_, err := service.Import(t.Context(), ImportInput{Name: "Invalid", SourceKind: "local_file", LocalPath: source})
	if err == nil || !strings.Contains(err.Error(), "does not have a GGUF header") {
		t.Fatalf("non-GGUF error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::cancelled_local_import_persists_terminal_state_and_can_retry (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CancelledLocalImportPersistsTerminalStateAndCanRetry(t *testing.T) {
	service, database, home := newTestService(t)
	source := filepath.Join(home, "retry.gguf")
	if err := os.WriteFile(source, []byte("GGUF retry payload"), 0o600); err != nil {
		t.Fatal(err)
	}
	queued, err := service.Import(t.Context(), ImportInput{Name: "Retry", SourceKind: "local_file", LocalPath: source})
	if err != nil {
		t.Fatal(err)
	}
	cancelled, err := service.Cancel(t.Context(), queued.ID)
	if err != nil || cancelled.Status != "cancelled" {
		t.Fatalf("cancelled import = %#v, %v", cancelled, err)
	}
	retried, err := service.Import(t.Context(), ImportInput{Name: "Retry", SourceKind: "local_file", LocalPath: source})
	if err != nil {
		t.Fatal(err)
	}
	installed := waitForInstallation(t, database, retried.ID)
	if installed.Status != "installed" || installed.SHA256 == "" {
		t.Fatalf("retry import = %#v", installed)
	}
}

// Rust source: crates/noema-providers/src/local_models/download_tests.rs::large_local_import_persists_incremental_progress_before_verification (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LargeLocalImportPersistsIncrementalProgressBeforeVerification(t *testing.T) {
	service, database, home := newTestService(t)
	payload := make([]byte, 9*1024*1024)
	copy(payload[:4], []byte("GGUF"))
	source := filepath.Join(home, "large.gguf")
	if err := os.WriteFile(source, payload, 0o600); err != nil {
		t.Fatal(err)
	}
	eventContext, cancel := context.WithCancel(t.Context())
	defer cancel()
	events, err := service.Subscribe(eventContext, "")
	if err != nil {
		t.Fatal(err)
	}
	queued, err := service.Import(t.Context(), ImportInput{Name: "Large", SourceKind: "local_file", LocalPath: source})
	if err != nil {
		t.Fatal(err)
	}
	installed := waitForInstallation(t, database, queued.ID)
	progress, verifying := false, false
	deadline := time.After(3 * time.Second)
	for !(progress && verifying) {
		select {
		case event := <-events:
			if event.Installation == nil || event.Installation.ID != queued.ID {
				continue
			}
			if event.Kind == "transfer_progress" && event.Installation.CompletedBytes >= 8*1024*1024 {
				progress = true
			}
			if event.Installation.Status == "verifying" && event.Installation.CompletedBytes == installed.TotalBytes {
				verifying = true
			}
		case <-deadline:
			t.Fatal("large import events did not include progress and verification")
		}
	}
}

// Rust source: crates/noema-providers/src/local_models/eval/materialize.rs::cancellation_leaves_no_partial_or_published_file (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_CancellationLeavesNoPartialOrPublishedFile(t *testing.T) {
	service, database, home := newTestService(t)
	bytesValue := []byte("GGUF cancelled evaluation model")
	source := filepath.Join(home, "cancelled.gguf")
	if err := os.WriteFile(source, bytesValue, 0o600); err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueLocalModel(t.Context(), store.LocalModelInstallation{
		ID: "installation:cancelled", ModelID: "cancelled", Name: "Cancelled", File: filepath.Base(source),
		SourceKind: "local_file", Backend: "cpu", TotalBytes: int64(len(bytesValue)), CreatedAt: time.Now(),
	})
	if err != nil {
		t.Fatal(err)
	}
	cancelled, cancel := context.WithCancel(t.Context())
	cancel()
	err = service.copyLocal(cancelled, queued, source)
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("cancelled materialization = %v", err)
	}
	for _, directory := range []string{filepath.Join(home, "system", "tmp"), filepath.Join(home, "models", "blobs")} {
		entries, readErr := os.ReadDir(directory)
		if readErr != nil && os.IsNotExist(readErr) {
			continue
		}
		if readErr != nil || len(entries) != 0 {
			t.Fatalf("cancelled cache entries in %s = %v, %v", directory, entries, readErr)
		}
	}
}
