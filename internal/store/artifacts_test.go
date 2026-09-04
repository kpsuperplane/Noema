package store

import (
	"context"
	"database/sql"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestArtifactSchemaOwnerChecksAndImmutableVersions(t *testing.T) {
	ctx := context.Background()
	v10Path := filepath.Join(t.TempDir(), "v10.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(v10Path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(schemaAtVersion(10) + "\nPRAGMA user_version = 10;"); err != nil {
		t.Fatal(err)
	}
	_ = legacy.Close()
	database, err := Open(ctx, v10Path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	var artifactTable bool
	if err := database.db.QueryRow(`SELECT EXISTS(
        SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'artifacts'
    )`).Scan(&artifactTable); err != nil || !artifactTable {
		t.Fatalf("Artifact migration = %v, %v", artifactTable, err)
	}
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	task, err := database.CreateTask(ctx, "task:0123456789abcdef0123456789abcdef", "Artifacts", "correlation:test", now)
	if err != nil {
		t.Fatal(err)
	}
	path1, digest1, media := "tasks/task/artifact.txt", strings.Repeat("a", 64), "text/plain"
	created, err := database.CreateArtifact(ctx, Artifact{
		ID:    "artifact:0123456789abcdef0123456789abcdef",
		Owner: ArtifactOwner{ObjectType: "task", ObjectID: task.ID},
		Title: " Source ", Kind: "source_file", StorageKind: ArtifactLocalFile,
		CreatedByActorID: "human:local", Metadata: map[string]any{"filename": "artifact.txt"},
	}, ArtifactVersion{
		ID:                "artifact_version:0123456789abcdef0123456789abcdef",
		LocalRelativePath: &path1, MediaType: &media, ByteSize: pointer(int64(4)),
		ContentSHA256: &digest1, CreatedByActorID: "human:local", Metadata: map[string]any{"source": "capture"},
	}, now)
	if err != nil || created.Artifact.Title != "Source" || created.CurrentVersion.Index != 1 {
		t.Fatalf("created Artifact = %#v, %v", created, err)
	}
	path2, digest2 := "tasks/task/artifact-2.txt", strings.Repeat("b", 64)
	second, err := database.AppendArtifactVersion(ctx, created.Artifact.ID, ArtifactVersion{
		ID: "artifact_version:abcdef0123456789abcdef0123456789", Index: 2,
		LocalRelativePath: &path2, ByteSize: pointer(int64(5)), ContentSHA256: &digest2,
		CreatedByActorID: "human:local", Metadata: map[string]any{"reason": "revision"},
	}, now.Add(time.Minute))
	if err != nil || second.Index != 2 {
		t.Fatalf("appended version = %#v, %v", second, err)
	}
	loaded, err := database.ArtifactWithVersionsByID(ctx, created.Artifact.ID)
	if err != nil || len(loaded.Versions) != 2 || loaded.Versions[0].LocalRelativePath == nil ||
		*loaded.Versions[0].LocalRelativePath != path1 || loaded.CurrentVersion.ID != second.ID ||
		loaded.Artifact.Metadata["filename"] != "artifact.txt" || loaded.Versions[0].Metadata["source"] != "capture" ||
		loaded.Versions[1].Metadata["reason"] != "revision" {
		t.Fatalf("stored immutable versions = %#v, %v", loaded, err)
	}
	oversizedPath := "tasks/task/artifact-3.txt"
	_, err = database.AppendArtifactVersion(ctx, created.Artifact.ID, ArtifactVersion{
		ID: "artifact_version:11111111111111111111111111111111", LocalRelativePath: &oversizedPath,
		CreatedByActorID: "human:local",
		Metadata:         map[string]any{"value": strings.Repeat("x", maxArtifactMetadataBytes)},
	}, now.Add(2*time.Minute))
	if err == nil {
		t.Fatal("oversized Artifact metadata was accepted")
	}
	listed, err := database.ArtifactsForOwner(ctx, created.Artifact.Owner, 20)
	if err != nil || len(listed) != 1 {
		t.Fatalf("listed Artifacts = %#v, %v", listed, err)
	}
	if allowed, err := database.ArtifactOwnerAuthorized(ctx, ArtifactOwner{
		ObjectType: "task", ObjectID: "task:ffffffffffffffffffffffffffffffff",
	}); err != nil || allowed {
		t.Fatalf("unknown owner authorization = %v, %v", allowed, err)
	}
}

func pointer[T any](value T) *T { return &value }
