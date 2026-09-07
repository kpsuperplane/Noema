package artifact

import (
	"bytes"
	"context"
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/store"
)

const (
	rustArtifactOperationOne = "op-1111111111111111111111111111111111111111111111111111111111111111"
	rustArtifactOperationTwo = "op-2222222222222222222222222222222222222222222222222222222222222222"
)

// Rust source: crates/noema-artifacts/src/domain.rs::tests::external_artifact_urls_normalize_http_and_reject_other_schemes.
func TestRustArtifacts_ExternalArtifactURLsNormalizeHTTPAndRejectOtherSchemes(t *testing.T) {
	normalized, err := ValidateExternalURL("https://example.com/a b")
	if err != nil || normalized != "https://example.com/a%20b" {
		t.Fatalf("normalized URL = %q, %v", normalized, err)
	}
	if _, err := ValidateExternalURL("http://example.com"); err != nil {
		t.Fatalf("HTTP URL rejected: %v", err)
	}
	for _, value := range []string{
		"file:///tmp/report",
		"ssh://example.com/report",
		"not a URL",
	} {
		if _, err := ValidateExternalURL(value); err == nil {
			t.Fatalf("invalid URL accepted: %q", value)
		}
	}
}

// Rust source: crates/noema-artifacts/src/paths.rs::tests::safe_artifact_filename_rejects_unsafe_cross_platform_path_and_header_components.
func TestRustArtifacts_SafeArtifactFilenameRejectsUnsafeCrossPlatformPathAndHeaderComponents(t *testing.T) {
	if err := SafeFilename("report.md"); err != nil {
		t.Fatalf("safe filename rejected: %v", err)
	}
	for _, filename := range []string{
		"",
		"../report.md",
		"nested/report.md",
		"report\".md",
		"report\r.md",
		"report\n.md",
	} {
		if err := SafeFilename(filename); err == nil {
			t.Fatalf("unsafe filename accepted: %q", filename)
		}
	}
	for _, filename := range []string{
		"report:stream.txt",
		"report<draft>.txt",
		"report|draft.txt",
		"report?.txt",
		"report*.txt",
		"report.txt.",
		"report.txt ",
		"CON",
		"con.txt",
		"PRN.md",
		"AUX",
		"nul.json",
		"COM1.log",
		"com9",
		"LPT1.csv",
		"lpt9.txt",
	} {
		if err := SafeFilename(filename); err == nil {
			t.Fatalf("unsafe filename accepted: %q", filename)
		}
	}
	for _, filename := range []string{"computer.txt", "com10.txt", "lpt0.txt"} {
		if err := SafeFilename(filename); err != nil {
			t.Fatalf("safe filename %q rejected: %v", filename, err)
		}
	}
}

// Rust source: crates/noema-artifacts/src/paths.rs::tests::artifact_version_dirs_stay_below_supported_owner_roots.
func TestRustArtifacts_ArtifactVersionDirsStayBelowSupportedOwnerRoots(t *testing.T) {
	for _, test := range []struct {
		owner    store.ArtifactOwner
		expected string
	}{
		{
			owner:    store.ArtifactOwner{ObjectType: "conversation", ObjectID: "conversation:abc"},
			expected: "/tmp/noema/conversations/conversation_abc/artifacts/artifact_def/versions/2",
		},
		{
			owner:    store.ArtifactOwner{ObjectType: "task", ObjectID: "task:abc"},
			expected: "/tmp/noema/tasks/task_abc/artifacts/artifact_def/versions/2",
		},
	} {
		got := filepath.Join("/tmp/noema", versionDirectory(test.owner, "artifact:def", 2))
		if got != filepath.FromSlash(test.expected) {
			t.Fatalf("version directory for %#v = %q, want %q", test.owner, got, test.expected)
		}
	}
}

// Rust source: crates/noema-artifacts/src/paths.rs::tests::artifact_download_route_uses_and_validates_public_version_slugs.
func TestRustArtifacts_ArtifactDownloadRouteUsesAndValidatesPublicVersionSlugs(t *testing.T) {
	versionID := "artifact_version:18c0aa78b3e7c5e86"
	if got := DownloadURL(versionID); got != "/artifacts/versions/18c0aa78b3e7c5e86/download" {
		t.Fatalf("download URL = %q", got)
	}
	if got := PreviewURL(versionID); got != "/artifacts/versions/18c0aa78b3e7c5e86/preview" {
		t.Fatalf("preview URL = %q", got)
	}
	if got, ok := VersionIDFromSlug("18c0aa78b3e7c5e86"); !ok || got != versionID {
		t.Fatalf("version ID = %q, %v", got, ok)
	}
	if got, ok := VersionIDFromSlug(""); ok || got != "" {
		t.Fatalf("empty slug = %q, %v", got, ok)
	}
	if got, ok := VersionIDFromSlug(versionID); ok || got != "" {
		t.Fatalf("canonical ID accepted as slug = %q, %v", got, ok)
	}
	if got, ok := VersionIDFromSlug("nested/path"); ok || got != "" {
		t.Fatalf("nested slug accepted = %q, %v", got, ok)
	}
}

// Rust source: crates/noema-artifacts/src/filesystem/tests.rs::publication_lifecycle_and_cancellation_contracts.
func TestRustArtifacts_PublicationLifecycleAndCancellationContracts(t *testing.T) {
	// Case: create_append_and_read_verify_immutable_bytes.
	env := newRustArtifactEnvironment(t)
	env.service.testOperationIDs = []string{rustArtifactOperationOne, rustArtifactOperationTwo}
	created, err := env.service.CreateLocal(context.Background(), rustArtifactLocalInput(env.owner, []byte("first")))
	if err != nil {
		t.Fatalf("create artifact: %v", err)
	}
	secondTitle := "second"
	mediaType := "text/plain"
	appended, err := env.service.AppendLocal(
		context.Background(), created.Artifact.ID, "report.txt", []byte("second"), &secondTitle,
		&mediaType, "actor-1", store.ArtifactSource{}, map[string]any{},
	)
	if err != nil {
		t.Fatalf("append artifact: %v", err)
	}
	firstContent, err := env.service.Read(created.Artifact, created.CurrentVersion)
	if err != nil {
		t.Fatalf("read first artifact version: %v", err)
	}
	content, err := env.service.Read(created.Artifact, appended)
	if err != nil {
		t.Fatalf("read artifact: %v", err)
	}
	if !bytes.Equal(firstContent.Bytes, []byte("first")) {
		t.Fatalf("first bytes = %q", firstContent.Bytes)
	}
	if content.Filename != "report.txt" {
		t.Fatalf("appended filename = %q", content.Filename)
	}
	if !bytes.Equal(content.Bytes, []byte("second")) {
		t.Fatalf("appended bytes = %q", content.Bytes)
	}
	if created.CurrentVersion.LocalRelativePath == nil || appended.LocalRelativePath == nil ||
		*created.CurrentVersion.LocalRelativePath == *appended.LocalRelativePath {
		t.Fatalf("published paths = %#v and %#v", created.CurrentVersion.LocalRelativePath, appended.LocalRelativePath)
	}
	incompleteVersion := created.CurrentVersion
	incompleteVersion.ByteSize = nil
	if _, err := env.service.Read(created.Artifact, incompleteVersion); !errors.Is(err, ErrUnavailable) {
		t.Fatalf("incomplete local metadata error = %v", err)
	}
	assertRustArtifactStagingEmpty(t, env.rootPath)

	// Case: metadata_failure_removes_only_operation_private_publication.
	env = newRustArtifactEnvironment(t)
	env.service.testOperationIDs = []string{rustArtifactOperationOne}
	unrelated := filepath.Join(env.rootPath, "unrelated.txt")
	if err := os.WriteFile(unrelated, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	failedInput := rustArtifactLocalInput(env.owner, []byte("discard"))
	failedInput.Metadata = map[string]any{"value": strings.Repeat("x", 64*1024)}
	if _, err := env.service.CreateLocal(context.Background(), failedInput); err == nil {
		t.Fatal("metadata failure was accepted")
	}
	artifacts, err := env.database.ArtifactsForOwner(context.Background(), env.owner, 20)
	if err != nil {
		t.Fatalf("list artifacts after metadata failure: %v", err)
	}
	if len(artifacts) != 0 {
		t.Fatalf("metadata failure committed %#v", artifacts)
	}
	if got, err := os.ReadFile(unrelated); err != nil || !bytes.Equal(got, []byte("keep")) {
		t.Fatalf("unrelated bytes = %q, %v", got, err)
	}
	assertRustArtifactObjectDirectoriesEmpty(t, filepath.Join(env.rootPath, "tasks", sanitizeSegment(env.owner.ObjectID), "artifacts"))
	assertRustArtifactStagingEmpty(t, env.rootPath)

	// Case: cancelling_blocked_metadata_future_commits_nothing_and_removes_bytes.
	env = newRustArtifactEnvironment(t)
	env.service.testOperationIDs = []string{rustArtifactOperationOne}
	metadataEntered := make(chan struct{})
	metadataRelease := make(chan struct{})
	nowCalls := 0
	env.service.now = func() time.Time {
		nowCalls++
		if nowCalls == 2 {
			close(metadataEntered)
			<-metadataRelease
		}
		return time.Now()
	}
	ctx, cancel := context.WithCancel(context.Background())
	metadataDone := make(chan error, 1)
	go func() {
		_, err := env.service.CreateLocal(ctx, rustArtifactLocalInput(env.owner, []byte("cancel")))
		metadataDone <- err
	}()
	<-metadataEntered
	if got := len(rustArtifactRegularFiles(env.rootPath)); got != 1 {
		t.Fatalf("published files while metadata is blocked = %d", got)
	}
	cancel()
	close(metadataRelease)
	if err := <-metadataDone; err == nil {
		t.Fatal("cancelled metadata write succeeded")
	}
	artifacts, err = env.database.ArtifactsForOwner(context.Background(), env.owner, 20)
	if err != nil {
		t.Fatalf("list artifacts after cancellation: %v", err)
	}
	if len(artifacts) != 0 {
		t.Fatalf("cancelled metadata write committed %#v", artifacts)
	}
	if got := rustArtifactRegularFiles(env.rootPath); len(got) != 0 {
		t.Fatalf("published files after cancellation = %#v", got)
	}
	assertRustArtifactStagingEmpty(t, env.rootPath)

	// Case: cancelling_before_publication_cleans_detached_staging_worker.
	env = newRustArtifactEnvironment(t)
	env.service.testOperationIDs = []string{rustArtifactOperationOne}
	publishEntered := make(chan struct{})
	publishRelease := make(chan struct{})
	env.service.testPublishHook = func() error {
		close(publishEntered)
		<-publishRelease
		return errors.New("injected pre-publication failure")
	}
	ctx, cancel = context.WithCancel(context.Background())
	publishDone := make(chan error, 1)
	go func() {
		_, err := env.service.CreateLocal(ctx, rustArtifactLocalInput(env.owner, []byte("cancel")))
		publishDone <- err
	}()
	<-publishEntered
	if got := len(rustArtifactRegularFiles(env.rootPath)); got != 1 {
		t.Fatalf("staged files while publication is blocked = %d", got)
	}
	cancel()
	close(publishRelease)
	if err := <-publishDone; err == nil {
		t.Fatal("cancelled publication succeeded")
	}
	artifacts, err = env.database.ArtifactsForOwner(context.Background(), env.owner, 20)
	if err != nil {
		t.Fatalf("list artifacts after publication cancellation: %v", err)
	}
	if len(artifacts) != 0 {
		t.Fatalf("cancelled publication committed %#v", artifacts)
	}
	if got := rustArtifactRegularFiles(env.rootPath); len(got) != 0 {
		t.Fatalf("staged files after cancellation = %#v", got)
	}
	assertRustArtifactStagingEmpty(t, env.rootPath)
}

// Rust source: crates/noema-artifacts/src/filesystem/tests.rs::confinement_identity_and_cleanup_contracts.
func TestRustArtifacts_ConfinementIdentityAndCleanupContracts(t *testing.T) {
	// Case: replacing_root_after_construction_fails_closed.
	env := newRustArtifactEnvironment(t)
	env.service.testOperationIDs = []string{rustArtifactOperationOne}
	retained := env.rootPath + "-retained"
	if err := os.Rename(env.rootPath, retained); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(env.rootPath, 0o700); err != nil {
		t.Fatal(err)
	}
	if _, err := env.service.CreateLocal(context.Background(), rustArtifactLocalInput(env.owner, []byte("confined"))); err == nil {
		t.Fatal("root replacement was accepted")
	}
	artifacts, err := env.database.ArtifactsForOwner(context.Background(), env.owner, 20)
	if err != nil {
		t.Fatalf("list artifacts after root replacement: %v", err)
	}
	if len(artifacts) != 0 {
		t.Fatalf("root replacement committed %#v", artifacts)
	}
	if got := rustArtifactRegularFiles(env.rootPath); len(got) != 0 {
		t.Fatalf("replacement root files = %#v", got)
	}
	if got := rustArtifactRegularFiles(retained); len(got) != 0 {
		t.Fatalf("retained root files = %#v", got)
	}

	// Case: conversation_local_file_artifact_rejects_symlinked_artifact_root.
	if runtime.GOOS != "windows" {
		env = newRustArtifactEnvironment(t)
		env.service.testOperationIDs = []string{rustArtifactOperationOne}
		ownerRoot := filepath.Join(env.rootPath, "tasks", sanitizeSegment(env.owner.ObjectID))
		if err := os.MkdirAll(ownerRoot, 0o700); err != nil {
			t.Fatal(err)
		}
		outside := filepath.Join(env.rootPath, "outside")
		if err := os.Mkdir(outside, 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, filepath.Join(ownerRoot, "artifacts")); err != nil {
			t.Fatal(err)
		}
		if _, err := env.service.CreateLocal(context.Background(), rustArtifactLocalInput(env.owner, []byte("report"))); err == nil {
			t.Fatal("symlinked artifact root was accepted")
		}
	}

	// Case: publication_rejects_clobber_and_malformed_operation_ids.
	env = newRustArtifactEnvironment(t)
	stagingPath := filepath.Join(env.rootPath, "staging")
	objectPath := filepath.Join(env.rootPath, "object")
	if err := os.MkdirAll(stagingPath, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(objectPath, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(stagingPath, "report.txt"), []byte("new"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(objectPath, "report.txt"), []byte("existing"), 0o600); err != nil {
		t.Fatal(err)
	}
	rootRelativeStage := filepath.Join("staging", "report.txt")
	rootRelativeObject := filepath.Join("object", "report.txt")
	if err := env.root.Link(rootRelativeStage, rootRelativeObject); err == nil {
		t.Fatal("clobbering hard link was accepted")
	}
	if got, err := os.ReadFile(filepath.Join(objectPath, "report.txt")); err != nil || !bytes.Equal(got, []byte("existing")) {
		t.Fatalf("destination bytes = %q, %v", got, err)
	}

	invalid := newRustArtifactEnvironment(t)
	invalid.service.testOperationIDs = []string{"op-not-random"}
	if _, err := invalid.service.CreateLocal(context.Background(), rustArtifactLocalInput(invalid.owner, []byte("never-written"))); err == nil || err.Error() != "unsafe Artifact operation id" {
		t.Fatalf("malformed operation ID error = %v", err)
	}
	if got := rustArtifactRegularFiles(invalid.rootPath); len(got) != 0 {
		t.Fatalf("malformed operation ID published files = %#v", got)
	}

	// Case: startup_cleanup_only_removes_recognized_real_private_entries.
	env = newRustArtifactEnvironment(t)
	staging := filepath.Join(env.rootPath, stagingRootName)
	stale := filepath.Join(staging, rustArtifactOperationOne)
	invalidEntry := filepath.Join(staging, "not-governed")
	if err := os.MkdirAll(stale, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(stale, "report.txt"), []byte("stale"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Chtimes(stale, time.Now().Add(-25*time.Hour), time.Now().Add(-25*time.Hour)); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(invalidEntry, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(invalidEntry, "keep.txt"), []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	var outside string
	if runtime.GOOS != "windows" {
		outside = filepath.Join(env.rootPath, "outside")
		if err := os.Mkdir(outside, 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(outside, "victim.txt"), []byte("keep"), 0o600); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, filepath.Join(staging, rustArtifactOperationTwo)); err != nil {
			t.Fatal(err)
		}
	}
	if err := env.service.CleanupStaging(); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(stale); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("stale stage still exists: %v", err)
	}
	if got, err := os.ReadFile(filepath.Join(invalidEntry, "keep.txt")); err != nil || !bytes.Equal(got, []byte("keep")) {
		t.Fatalf("invalid entry bytes = %q, %v", got, err)
	}
	if runtime.GOOS != "windows" {
		if got, err := os.ReadFile(filepath.Join(outside, "victim.txt")); err != nil || !bytes.Equal(got, []byte("keep")) {
			t.Fatalf("outside bytes = %q, %v", got, err)
		}
		if _, err := os.Lstat(filepath.Join(staging, rustArtifactOperationTwo)); err != nil {
			t.Fatalf("staging symlink removed: %v", err)
		}
	}
}

type rustArtifactEnvironment struct {
	rootPath string
	root     *os.Root
	database *store.Store
	service  *Service
	owner    store.ArtifactOwner
}

func newRustArtifactEnvironment(t *testing.T) *rustArtifactEnvironment {
	t.Helper()
	ctx := context.Background()
	rootPath := t.TempDir()
	database, err := store.Open(ctx, filepath.Join(t.TempDir(), "metadata.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := os.OpenRoot(rootPath)
	if err != nil {
		_ = database.Close()
		t.Fatal(err)
	}
	owner := store.ArtifactOwner{ObjectType: "task", ObjectID: "task:0123456789abcdef0123456789abcdef"}
	if _, err := database.CreateTask(ctx, owner.ObjectID, "Artifact owner", "correlation:artifact-test", time.Now()); err != nil {
		_ = root.Close()
		_ = database.Close()
		t.Fatal(err)
	}
	service, err := New(root, database, nil)
	if err != nil {
		_ = root.Close()
		_ = database.Close()
		t.Fatal(err)
	}
	environment := &rustArtifactEnvironment{rootPath: rootPath, root: root, database: database, service: service, owner: owner}
	t.Cleanup(func() {
		_ = root.Close()
		_ = database.Close()
	})
	return environment
}

func rustArtifactLocalInput(owner store.ArtifactOwner, bytes []byte) LocalInput {
	mediaType := "text/plain"
	return LocalInput{
		Owner: owner, Title: "Report", Kind: "report", Filename: "report.txt", Bytes: bytes,
		MediaType: &mediaType, CreatedByActorID: "actor-1", Source: store.ArtifactSource{}, Metadata: map[string]any{},
	}
}

func rustArtifactRegularFiles(root string) []string {
	files := make([]string, 0)
	_ = filepath.WalkDir(root, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.Type().IsRegular() {
			files = append(files, path)
		}
		return nil
	})
	sort.Strings(files)
	return files
}

func assertRustArtifactStagingEmpty(t *testing.T, root string) {
	t.Helper()
	staging := filepath.Join(root, stagingRootName)
	info, err := os.Stat(staging)
	if err != nil || !info.IsDir() {
		t.Fatalf("staging root = %v, %v", info, err)
	}
	entries, err := os.ReadDir(staging)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("staging entries = %#v", entries)
	}
}

func assertRustArtifactObjectDirectoriesEmpty(t *testing.T, root string) {
	t.Helper()
	count := 0
	err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() && entry.Name() == "objects" {
			count++
			entries, readErr := os.ReadDir(path)
			if readErr != nil {
				return readErr
			}
			if len(entries) != 0 {
				t.Fatalf("objects entries = %#v", entries)
			}
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if count == 0 {
		t.Fatal("published object directory was not created")
	}
}
