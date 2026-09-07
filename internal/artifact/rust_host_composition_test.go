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

func runRustHostAppendRace(t *testing.T, taskOwner bool) {
	t.Helper()
	ctx := context.Background()
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
	serviceA, err := New(root, databaseA, nil)
	if err != nil {
		t.Fatal(err)
	}
	serviceB, err := New(root, databaseB, nil)
	if err != nil {
		t.Fatal(err)
	}
	initial, err := serviceA.CreateLocal(ctx, LocalInput{
		Owner: owner, Title: "Race report", Kind: "document", Filename: "report.txt",
		Bytes: []byte("initial"), MediaType: stringPtr("text/plain"), CreatedByActorID: "agent:test",
	})
	if err != nil {
		t.Fatal(err)
	}

	start := make(chan struct{})
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
			<-start
			version, err := contender.service.AppendLocal(ctx, initial.Artifact.ID, "report.txt", []byte(contender.bytes), nil, stringPtr("text/plain"), "agent:test", store.ArtifactSource{}, nil)
			results <- appendResult{version: version, err: err, bytes: contender.bytes}
		}()
	}
	close(start)
	group.Wait()
	close(results)

	var winner appendResult
	var losers int
	for result := range results {
		if result.err == nil {
			if winner.err == nil && winner.version.ID != "" {
				t.Fatalf("append race produced two winners: %#v and %#v", winner.version, result.version)
			}
			winner = result
			continue
		}
		losers++
	}
	if winner.version.ID == "" || losers != 1 {
		t.Fatalf("append race results = winner %#v, losers %d; Go must preserve one-winner CAS behavior", winner, losers)
	}
	if !strings.Contains(strings.ToLower(winner.version.ID), "artifact_version:") {
		t.Fatalf("winner version = %#v", winner.version)
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
