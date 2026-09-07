package artifact

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

// Rust source: crates/noema-host/src/artifact_store_composition_tests.rs:75::conversation_append_race_has_one_winner_and_cleans_loser
func TestRustHost_conversation_append_race_has_one_winner_and_cleans_loser(t *testing.T) {
	runRustHostAppendRace(t, false)
}

// Rust source: crates/noema-host/src/artifact_store_composition_tests.rs:88::task_append_race_has_one_winner_and_cleans_loser
func TestRustHost_task_append_race_has_one_winner_and_cleans_loser(t *testing.T) {
	runRustHostAppendRace(t, true)
}

// rustHostBarrierMetadataStore reproduces the Rust test's metadata boundary.
// Both appenders must commit the same loaded append target before either one
// can write its version.
type rustHostBarrierMetadataStore struct {
	store   *store.Store
	loaded  *sync.WaitGroup
	release <-chan struct{}
}

func (s *rustHostBarrierMetadataStore) ArtifactOwnerAuthorized(ctx context.Context, owner store.ArtifactOwner) (bool, error) {
	return s.store.ArtifactOwnerAuthorized(ctx, owner)
}

func (s *rustHostBarrierMetadataStore) CreateArtifact(ctx context.Context, artifact store.Artifact, version store.ArtifactVersion, now time.Time) (store.ArtifactWithVersions, error) {
	return s.store.CreateArtifact(ctx, artifact, version, now)
}

func (s *rustHostBarrierMetadataStore) AppendArtifactVersion(ctx context.Context, artifactID string, version store.ArtifactVersion, now time.Time) (store.ArtifactVersion, error) {
	return s.store.AppendArtifactVersion(ctx, artifactID, version, now)
}

func (s *rustHostBarrierMetadataStore) ArtifactWithVersionsByID(ctx context.Context, artifactID string) (store.ArtifactWithVersions, error) {
	value, err := s.store.ArtifactWithVersionsByID(ctx, artifactID)
	if err != nil {
		return store.ArtifactWithVersions{}, err
	}
	s.loaded.Done()
	<-s.release
	return value, nil
}

func (s *rustHostBarrierMetadataStore) AuthorizedLocalArtifactVersion(ctx context.Context, versionID string) (store.Artifact, store.ArtifactVersion, bool, error) {
	return s.store.AuthorizedLocalArtifactVersion(ctx, versionID)
}

func runRustHostAppendRace(t *testing.T, taskOwner bool) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	databaseA, err := store.Open(ctx, paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = databaseA.Close() })
	databaseB, err := store.Open(ctx, paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = databaseB.Close() })

	owner := store.ArtifactOwner{ObjectType: "conversation"}
	if taskOwner {
		taskID := "task:0123456789abcdef0123456789abcdef"
		if _, err := databaseA.CreateTask(ctx, taskID, "Artifact append race", "correlation:artifact-race", time.Now()); err != nil {
			t.Fatal(err)
		}
		owner = store.ArtifactOwner{ObjectType: "task", ObjectID: taskID}
	} else {
		conversation, err := databaseA.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
		if err != nil {
			t.Fatal(err)
		}
		owner.ObjectID = conversation.ID
	}
	initialService, err := New(root, databaseA, nil)
	if err != nil {
		t.Fatal(err)
	}
	initial, err := initialService.CreateLocal(ctx, LocalInput{
		Owner: owner, Title: "Race report", Kind: "document", Filename: "report.txt",
		Bytes: []byte("initial"), MediaType: stringPtr("text/plain"), CreatedByActorID: "agent:test",
		Metadata: map[string]any{},
	})
	if err != nil {
		t.Fatal(err)
	}

	loaded := &sync.WaitGroup{}
	loaded.Add(2)
	release := make(chan struct{})
	serviceA, err := newService(root, &rustHostBarrierMetadataStore{
		store: databaseA, loaded: loaded, release: release,
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	serviceB, err := newService(root, &rustHostBarrierMetadataStore{
		store: databaseB, loaded: loaded, release: release,
	}, nil)
	if err != nil {
		t.Fatal(err)
	}

	type appendResult struct {
		version store.ArtifactVersion
		err     error
		bytes   string
	}
	results := make(chan appendResult, 2)
	var group sync.WaitGroup
	for _, contender := range []struct {
		service *Service
		bytes   string
	}{
		{serviceA, "first contender"},
		{serviceB, "second contender"},
	} {
		contender := contender
		group.Add(1)
		go func() {
			defer group.Done()
			version, err := contender.service.AppendLocal(ctx, initial.Artifact.ID, "report.txt", []byte(contender.bytes), nil, stringPtr("text/plain"), "agent:test", store.ArtifactSource{}, map[string]any{})
			results <- appendResult{version: version, err: err, bytes: contender.bytes}
		}()
	}
	loadedDone := make(chan struct{})
	go func() {
		loaded.Wait()
		close(loadedDone)
	}()
	select {
	case <-loadedDone:
	case <-ctx.Done():
		close(release)
		t.Fatalf("append race metadata barrier did not load both targets: %v", ctx.Err())
	}
	close(release)
	groupDone := make(chan struct{})
	go func() {
		group.Wait()
		close(groupDone)
	}()
	select {
	case <-groupDone:
	case <-ctx.Done():
		t.Fatalf("append race did not finish within five seconds: %v", ctx.Err())
	}
	close(results)

	var winner appendResult
	var loser appendResult
	for result := range results {
		if result.err == nil {
			if winner.version.ID != "" {
				t.Fatalf("append race produced two winners: %#v and %#v", winner.version, result.version)
			}
			winner = result
		} else {
			if loser.err != nil {
				t.Fatalf("append race produced two losers: %v and %v", loser.err, result.err)
			}
			loser = result
		}
	}
	if winner.version.ID == "" || loser.err == nil {
		t.Fatalf("append race results = winner %#v, loser %v; Go must preserve one-winner CAS behavior", winner, loser.err)
	}
	if !strings.Contains(strings.ToLower(winner.version.ID), "artifact_version:") {
		t.Fatalf("winner version = %#v", winner.version)
	}
	if !errors.Is(loser.err, ErrMetadata) ||
		!strings.Contains(loser.err.Error(), "expected_next_version_index=2") ||
		!strings.Contains(loser.err.Error(), "actual_next_version_index=3") {
		t.Fatalf("unexpected loser error: %v", loser.err)
	}

	stored, err := databaseA.ArtifactWithVersionsByID(ctx, initial.Artifact.ID)
	if err != nil {
		t.Fatal(err)
	}
	if len(stored.Versions) != 2 || stored.CurrentVersion.ID != winner.version.ID {
		t.Fatalf("stored artifact = %#v; winner = %#v", stored, winner.version)
	}
	content, err := serviceA.Read(stored.Artifact, stored.CurrentVersion)
	if err != nil || string(content.Bytes) != winner.bytes {
		t.Fatalf("winning bytes = %#v, %v; want %q", content, err, winner.bytes)
	}
	if stored.Versions[0].LocalRelativePath == nil || winner.version.LocalRelativePath == nil ||
		*stored.Versions[0].LocalRelativePath == *winner.version.LocalRelativePath {
		t.Fatalf("version paths = %#v and %#v", stored.Versions[0].LocalRelativePath, winner.version.LocalRelativePath)
	}
	artifactPath := filepath.FromSlash(*stored.Versions[0].LocalRelativePath)
	artifactRoot := filepath.Join(paths.Root(), filepath.Dir(filepath.Dir(filepath.Dir(filepath.Dir(filepath.Dir(artifactPath))))))
	files := regularFiles(t, artifactRoot)
	want := []string{filepath.Join(paths.Root(), filepath.FromSlash(*stored.Versions[0].LocalRelativePath)), filepath.Join(paths.Root(), filepath.FromSlash(*winner.version.LocalRelativePath))}
	sort.Strings(want)
	if len(files) != len(want) {
		t.Fatalf("published files = %#v; want %#v", files, want)
	}
	for index := range want {
		if files[index] != want[index] {
			t.Fatalf("published files = %#v; want %#v", files, want)
		}
	}
	entries, err := os.ReadDir(filepath.Join(paths.Root(), stagingRootName))
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("staging entries = %#v", entries)
	}
}

func regularFiles(t *testing.T, root string) []string {
	t.Helper()
	var files []string
	err := filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() || filepath.Base(path) == stagingRootName {
			return nil
		}
		if entry.Type().IsRegular() {
			files = append(files, path)
		}
		return nil
	})
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		t.Fatal(err)
	}
	sort.Strings(files)
	return files
}

func stringPtr(value string) *string { return &value }
