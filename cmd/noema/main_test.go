package main

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestProjectDocumentRecoveryUsesCommittedReceipt(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	database, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()

	const committedRequest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	committedID, _ := store.NewProjectID()
	stage, err := home.StageProjectDocument(root, committedID, committedRequest, "# Committed\n")
	if err != nil {
		t.Fatal(err)
	}
	command := store.ProjectCommand{ActorID: "actor:human:local", Name: "project.create",
		ClientMutationID: "committed", RequestDigest: committedRequest}
	if _, err := database.CreateProject(context.Background(), committedID, "workspace:personal",
		"Committed", "", nil, stage.Document.Digest, command, time.Now()); err != nil {
		t.Fatal(err)
	}

	const orphanRequest = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	orphanID, _ := store.NewProjectID()
	if _, err := home.StageProjectDocument(root, orphanID, orphanRequest, "# Orphan\n"); err != nil {
		t.Fatal(err)
	}
	if err := recoverProjectDocuments(context.Background(), root, database); err != nil {
		t.Fatal(err)
	}
	document, err := home.ReadProjectDocument(root, committedID, nil)
	if err != nil || document.Content != "# Committed\n" {
		t.Fatalf("recovered document = %#v, %v", document, err)
	}
	if _, err := home.ReadProjectDocumentStage(root, committedID, committedRequest); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("committed stage remains: %v", err)
	}
	if _, err := home.ReadProjectDocumentStage(root, orphanID, orphanRequest); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("uncommitted stage remains: %v", err)
	}
}
