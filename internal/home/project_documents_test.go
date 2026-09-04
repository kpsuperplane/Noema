package home

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

const (
	projectRequestA = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	projectRequestB = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
)

func TestProjectCreateStageIsCentralAndVerifiesReuse(t *testing.T) {
	root := projectTestRoot(t)
	projectID := "project:0123456789abcdef0123456789abcdef"
	external := filepath.Join(t.TempDir(), "new", "project")
	stage, err := StageProjectDocument(root, projectID, projectRequestA, "# Proposed\n")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(external); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("staging touched external folder: %v", err)
	}
	reused, err := StageProjectDocument(root, projectID, projectRequestA, "# Proposed\n")
	if err != nil || !equalProjectStage(reused, stage) {
		t.Fatalf("reused stage = %#v, %v", reused, err)
	}
	if _, err := StageProjectDocument(root, projectID, projectRequestA, "# Different\n"); err == nil {
		t.Fatal("stage key accepted different content")
	}
	document, adopted, err := CommitProjectDocumentStage(root, stage, &external)
	if err != nil || adopted || document.Content != "# Proposed\n" {
		t.Fatalf("commit create = %#v, %v, %v", document, adopted, err)
	}
	stages, err := ProjectDocumentStages(root)
	if err != nil || len(stages) != 0 {
		t.Fatalf("stages after commit = %#v, %v", stages, err)
	}
	conflictID := "project:9123456789abcdef0123456789abcdef"
	conflict, err := StageProjectDocument(root, conflictID, projectRequestA, "# Staged\n")
	if err != nil {
		t.Fatal(err)
	}
	conflictFolder := t.TempDir()
	if err := os.WriteFile(filepath.Join(conflictFolder, projectDocumentName), []byte("# Changed\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, _, err := CommitProjectDocumentStage(root, conflict, &conflictFolder); err == nil {
		t.Fatal("create stage silently adopted a changed destination")
	}
	stages, err = ProjectDocumentStages(root)
	if err != nil || len(stages) != 1 || stages[0].ProjectID != conflictID {
		t.Fatalf("retained create stage = %#v, %v", stages, err)
	}
}

func TestProjectReplaceRetainsStageAfterConflict(t *testing.T) {
	root := projectTestRoot(t)
	projectID := "project:1123456789abcdef0123456789abcdef"
	created, err := StageProjectDocument(root, projectID, projectRequestA, "# Current\n")
	if err != nil {
		t.Fatal(err)
	}
	current, _, err := CommitProjectDocumentStage(root, created, nil)
	if err != nil {
		t.Fatal(err)
	}
	stage, _, err := PrepareProjectDocumentReplace(root, projectID, nil, current.Digest,
		"# Updated\n", projectRequestB)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root.Name(), "workspaces", "personal", "projects",
		"1123456789abcdef0123456789abcdef", "docs", projectDocumentName)
	if err := os.WriteFile(path, []byte("# Concurrent\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, _, err := CommitProjectDocumentStage(root, stage, nil); err == nil {
		t.Fatal("replacement overwrote a concurrent document")
	}
	stages, err := ProjectDocumentStages(root)
	if err != nil || len(stages) != 1 || stages[0].RequestDigest != projectRequestB {
		t.Fatalf("retained stages = %#v, %v", stages, err)
	}
	concurrent, err := ReadProjectDocument(root, projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := PrepareProjectDocumentReplace(root, projectID, nil, concurrent.Digest,
		"# Later\n", projectRequestA); err != nil {
		t.Fatal(err)
	}
	if err := DiscardProjectDocumentStage(root, projectID, projectRequestB); err != nil {
		t.Fatal(err)
	}
	stages, err = ProjectDocumentStages(root)
	if err != nil || len(stages) != 1 || stages[0].RequestDigest != projectRequestA {
		t.Fatalf("exact discard stages = %#v, %v", stages, err)
	}
}

func TestProjectMoveStagesOutsideExternalFolder(t *testing.T) {
	root := projectTestRoot(t)
	projectID := "project:2123456789abcdef0123456789abcdef"
	created, err := StageProjectDocument(root, projectID, projectRequestA, "# Move\n")
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := CommitProjectDocumentStage(root, created, nil); err != nil {
		t.Fatal(err)
	}
	external := t.TempDir()
	move, err := PrepareProjectDocumentMove(root, projectID, nil, &external, projectRequestB)
	if err != nil {
		t.Fatal(err)
	}
	entries, err := os.ReadDir(external)
	if err != nil || len(entries) != 0 {
		t.Fatalf("move staging touched external folder: %#v, %v", entries, err)
	}
	moved, _, err := CommitProjectDocumentStage(root, move, &external)
	if err != nil || moved.Content != "# Move\n" {
		t.Fatalf("commit move = %#v, %v", moved, err)
	}
	entries, err = os.ReadDir(external)
	if err != nil || len(entries) != 1 || entries[0].Name() != projectDocumentName {
		t.Fatalf("external publication entries = %#v, %v", entries, err)
	}
}

func TestProjectStageEnumerationExposesRecoveryFences(t *testing.T) {
	root := projectTestRoot(t)
	projectID := "project:3123456789abcdef0123456789abcdef"
	created, err := StageProjectDocument(root, projectID, projectRequestA, "# Current\n")
	if err != nil {
		t.Fatal(err)
	}
	current, _, err := CommitProjectDocumentStage(root, created, nil)
	if err != nil {
		t.Fatal(err)
	}
	stage, next, err := PrepareProjectDocumentReplace(root, projectID, nil, current.Digest,
		"# Recovery\n", projectRequestB)
	if err != nil {
		t.Fatal(err)
	}
	stages, err := ProjectDocumentStages(root)
	if err != nil || len(stages) != 1 {
		t.Fatalf("enumerated stages = %#v, %v", stages, err)
	}
	got := stages[0]
	if got.ProjectID != projectID || got.RequestDigest != projectRequestB ||
		got.ExpectedDocumentDigest != current.Digest || got.Document.Digest != next.Digest {
		t.Fatalf("recovery fences = %#v", got)
	}
	exact, err := ReadProjectDocumentStage(root, projectID, projectRequestB)
	if err != nil || !equalProjectStage(exact, stage) {
		t.Fatalf("exact recovery stage = %#v, %v", exact, err)
	}
	document, _, err := CommitProjectDocumentStage(root, stage, nil)
	if err != nil || document.Digest != next.Digest {
		t.Fatalf("recovered commit = %#v, %v", document, err)
	}
}

func projectTestRoot(t *testing.T) *os.Root {
	t.Helper()
	paths, err := FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	return root
}
