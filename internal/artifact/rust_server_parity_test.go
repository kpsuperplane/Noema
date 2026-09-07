package artifact

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

// Rust source: crates/noema-server/src/web/router/tests.rs::artifact_preview_adapter_allows_only_inert_browser_formats.
func TestRustServer_artifact_preview_adapter_allows_only_inert_browser_formats(t *testing.T) {
	service, database := newServerArtifactService(t)
	defer database.Close()
	handler := service.Handler()

	for _, fixture := range []struct {
		filename  string
		mediaType string
		status    int
	}{
		{filename: "label.png", mediaType: "image/png", status: http.StatusOK},
		{filename: "unsafe.svg", mediaType: "image/svg+xml", status: http.StatusUnsupportedMediaType},
		{filename: "unsafe.html", mediaType: "text/html", status: http.StatusUnsupportedMediaType},
	} {
		mediaType := fixture.mediaType
		created, err := service.CreateLocal(context.Background(), LocalInput{
			Owner: store.ArtifactOwner{ObjectType: "task", ObjectID: serverArtifactTaskID},
			Title: fixture.filename, Kind: "source_file", Filename: fixture.filename,
			Bytes: []byte("fixture"), MediaType: &mediaType, CreatedByActorID: "human:local",
		})
		if err != nil {
			t.Fatalf("create %s Artifact: %v", fixture.mediaType, err)
		}
		response := httptest.NewRecorder()
		handler.ServeHTTP(response, httptest.NewRequest(http.MethodGet, PreviewURL(created.CurrentVersion.ID), nil))
		if response.Code != fixture.status {
			t.Fatalf("%s preview status = %d, want %d", fixture.mediaType, response.Code, fixture.status)
		}
		if fixture.status == http.StatusOK {
			if response.Header().Get("Content-Type") != "image/png" ||
				response.Header().Get("Content-Disposition") != `inline; filename="label.png"` ||
				response.Header().Get("Content-Security-Policy") != "default-src 'none'; frame-ancestors 'self'; base-uri 'none'; form-action 'none'" {
				t.Fatalf("PNG preview response = %d %#v", response.Code, response.Header())
			}
		}
	}
}

// Rust source: crates/noema-server/src/web/router/tests.rs::artifact_download_adapter_sanitizes_response_headers.
func TestRustServer_artifact_download_adapter_sanitizes_response_headers(t *testing.T) {
	service, database := newServerArtifactService(t)
	defer database.Close()
	unsafeFilename := "report\"\r\nx-injected: yes.md"
	mediaType := "text/markdown\r\nx-injected: yes"
	service.testAuthorizedFile = func(_ context.Context, _ string) (File, bool, error) {
		return File{Filename: unsafeFilename, MediaType: mediaType, Bytes: []byte("report")}, true, nil
	}
	response := httptest.NewRecorder()
	service.Handler().ServeHTTP(response, httptest.NewRequest(http.MethodGet, DownloadURL("artifact_version:server-parity"), nil))
	if response.Code != http.StatusOK || response.Header().Get("Content-Type") != "application/octet-stream" ||
		response.Body.String() != "report" {
		t.Fatalf("download response = %d %q %#v", response.Code, response.Body.String(), response.Header())
	}
	if response.Header().Get("Content-Disposition") != `attachment; filename="report\"__x-injected: yes.md"` {
		t.Fatalf("download disposition = %q", response.Header().Get("Content-Disposition"))
	}
	if response.Header().Get("X-Injected") != "" {
		t.Fatal("response injected a header")
	}
	if strings.Contains(response.Header().Get("Content-Type"), "x-injected") {
		t.Fatal("media type injection survived response sanitization")
	}
}

const serverArtifactTaskID = "task:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

func newServerArtifactService(t *testing.T) (*Service, *store.Store) {
	t.Helper()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	database, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CreateTask(context.Background(), serverArtifactTaskID, "Artifact parity", "correlation:server-parity", time.Now()); err != nil {
		_ = database.Close()
		t.Fatal(err)
	}
	service, err := New(root, database, nil)
	if err != nil {
		_ = database.Close()
		t.Fatal(err)
	}
	return service, database
}
