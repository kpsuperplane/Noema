package artifact

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestPortableArtifactNamesRoutesAndExternalURLs(t *testing.T) {
	for _, valid := range []string{"report.md", "computer.txt", "com10.txt", "évidence.pdf"} {
		if err := SafeFilename(valid); err != nil {
			t.Fatalf("safe filename %q: %v", valid, err)
		}
	}
	for _, invalid := range []string{
		"", "../report.md", "nested/report.md", `report\draft.md`, "report\n.md", "report.txt.",
		"report.txt ", "CON", "con.txt", "PRN.md", "AUX", "nul.json", "COM1.log", "lpt9.txt",
	} {
		if err := SafeFilename(invalid); err == nil {
			t.Fatalf("unsafe filename accepted: %q", invalid)
		}
	}
	normalized, err := ValidateExternalURL("HTTPS://example.com/a b")
	if err != nil || normalized != "https://example.com/a%20b" {
		t.Fatalf("normalized URL = %q, %v", normalized, err)
	}
	for _, invalid := range []string{"file:///tmp/report", "ssh://example.com/report", "not a URL"} {
		if _, err := ValidateExternalURL(invalid); err == nil {
			t.Fatalf("invalid URL accepted: %q", invalid)
		}
	}
	versionID, ok := VersionIDFromSlug("0123456789abcdef0123456789abcdef")
	if !ok || DownloadURL(versionID) != "/artifacts/versions/0123456789abcdef0123456789abcdef/download" {
		t.Fatalf("version route = %q, %v", versionID, ok)
	}
	for _, invalid := range []string{"", "artifact_version:value", "nested/path"} {
		if _, ok := VersionIDFromSlug(invalid); ok {
			t.Fatalf("invalid slug accepted: %q", invalid)
		}
	}
}

func TestLocalPublicationRecoveryIntegrityAndHTTPDelivery(t *testing.T) {
	ctx := context.Background()
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	database, err := store.Open(ctx, paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	op := "op-" + strings.Repeat("a", 64)
	stage := filepath.Join(stagingRootName, op)
	if err := root.MkdirAll(stage, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := root.WriteFile(filepath.Join(stage, "stale.txt"), []byte("stale"), 0o600); err != nil {
		t.Fatal(err)
	}
	old := time.Now().Add(-25 * time.Hour)
	if err := root.Chtimes(stage, old, old); err != nil {
		t.Fatal(err)
	}
	service, err := New(root, database)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := root.Stat(stage); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("stale stage remains: %v", err)
	}
	task, err := database.CreateTask(ctx, "task:0123456789abcdef0123456789abcdef", "Source", "correlation:test", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	mediaType := "text/plain; charset=utf-8"
	created, err := service.CreateLocal(ctx, LocalInput{
		Owner: store.ArtifactOwner{ObjectType: "task", ObjectID: task.ID},
		Title: "Evidence", Kind: "source_file", Filename: "evidence.txt",
		Bytes: []byte("version one"), MediaType: &mediaType, CreatedByActorID: "human:local",
	})
	if err != nil {
		t.Fatal(err)
	}
	wantPrefix := "tasks/task_0123456789abcdef0123456789abcdef/artifacts/artifact_"
	path := *created.CurrentVersion.LocalRelativePath
	if !strings.HasPrefix(path, wantPrefix) || !strings.HasSuffix(path, "/evidence.txt") {
		t.Fatalf("published path = %q", path)
	}
	file, err := service.Read(created.Artifact, created.CurrentVersion)
	if err != nil || string(file.Bytes) != "version one" {
		t.Fatalf("verified file = %#v, %v", file, err)
	}
	request := httptest.NewRequest(http.MethodGet, DownloadURL(created.CurrentVersion.ID), nil)
	response := httptest.NewRecorder()
	service.Handler().ServeHTTP(response, request)
	if response.Code != http.StatusOK || response.Body.String() != "version one" ||
		!strings.HasPrefix(response.Header().Get("Content-Disposition"), "attachment;") {
		t.Fatalf("download response = %d, %q, %q", response.Code, response.Body.String(), response.Header().Get("Content-Disposition"))
	}
	preview := httptest.NewRecorder()
	service.Handler().ServeHTTP(preview, httptest.NewRequest(http.MethodGet, PreviewURL(created.CurrentVersion.ID), nil))
	if preview.Code != http.StatusUnsupportedMediaType {
		t.Fatalf("text preview status = %d", preview.Code)
	}
	pdfType := "application/pdf"
	pdf, err := service.CreateLocal(ctx, LocalInput{
		Owner: store.ArtifactOwner{ObjectType: "task", ObjectID: task.ID},
		Title: "Report", Kind: "report", Filename: "report.pdf",
		Bytes: []byte("%PDF fixture"), MediaType: &pdfType, CreatedByActorID: "human:local",
	})
	if err != nil {
		t.Fatal(err)
	}
	preview = httptest.NewRecorder()
	service.Handler().ServeHTTP(preview, httptest.NewRequest(http.MethodGet, PreviewURL(pdf.CurrentVersion.ID), nil))
	if preview.Code != http.StatusOK || preview.Header().Get("Content-Type") != "application/pdf" ||
		!strings.Contains(preview.Header().Get("Content-Security-Policy"), "default-src 'none'") {
		t.Fatalf("PDF preview = %d, %#v", preview.Code, preview.Header())
	}
	second, err := service.AppendLocal(ctx, created.Artifact.ID, "evidence.txt", []byte("version two"), nil, &mediaType, "human:local", store.ArtifactSource{})
	if err != nil || second.Index != 2 || second.ID == created.CurrentVersion.ID {
		t.Fatalf("second version = %#v, %v", second, err)
	}
	if err := root.WriteFile(filepath.FromSlash(path), []byte("version evil"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Read(created.Artifact, created.CurrentVersion); !errors.Is(err, ErrUnavailable) {
		t.Fatalf("tampered Artifact read = %v", err)
	}
}
