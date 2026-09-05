package graphql

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/vektah/gqlparser/v2/gqlerror"
)

func TestProjectAuthorityFlow(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	createInput := model.CreateProjectInput{
		WorkspaceID: personalWorkspaceID, Name: "Plan", Description: "Exact",
		ClientMutationID: "project-create",
	}
	created, err := resolver.createProject(ctx, createInput)
	if err != nil || created.Project.Revision != 1 {
		t.Fatalf("create Project = %#v, %v", created, err)
	}
	assertProjectReceipt(t, resolver.Store, "project.create", createInput.ClientMutationID, createInput)
	document, err := resolver.projectDocument(ctx, created.Project.ProjectID)
	if err != nil || document.Content != "# Plan\n\nExact\n" {
		t.Fatalf("Project document = %#v, %v", document, err)
	}
	saveInput := model.UpdateProjectDocumentInput{
		ProjectID: created.Project.ProjectID, ExpectedRevision: 1,
		ExpectedDocumentDigest: document.Digest, Content: "# Current\n",
		ClientMutationID: "project-document",
	}
	saved, err := resolver.updateProjectDocument(ctx, saveInput)
	if err != nil || saved.Project.Revision != 2 || saved.Document.Content != "# Current\n" {
		t.Fatalf("save Project document = %#v, %v", saved, err)
	}
	assertProjectReceipt(t, resolver.Store, "project.update", saveInput.ClientMutationID, saveInput)
	folder := t.TempDir()
	updated, err := resolver.updateProject(ctx, model.UpdateProjectInput{
		ProjectID: created.Project.ProjectID, ExpectedRevision: 2, Folder: &folder,
		ClientMutationID: "project-folder",
	})
	if err != nil || updated.Project.Folder == nil {
		t.Fatalf("move Project = %#v, %v", updated, err)
	}
	archiveInput := model.ArchiveProjectInput{ProjectID: created.Project.ProjectID,
		ExpectedRevision: 3, ClientMutationID: "project-archive"}
	archived, err := resolver.setProjectArchived(ctx, archiveInput.ProjectID,
		archiveInput.ExpectedRevision, archiveInput.ClientMutationID, true)
	if err != nil || archived.Project.ArchivedAt == nil {
		t.Fatalf("archive Project = %#v, %v", archived, err)
	}
	assertProjectReceipt(t, resolver.Store, "project.archive", archiveInput.ClientMutationID, struct {
		ProjectID        string `json:"projectId"`
		ExpectedRevision int    `json:"expectedRevision"`
	}{archiveInput.ProjectID, archiveInput.ExpectedRevision})
	reopenInput := model.ReopenProjectInput{ProjectID: created.Project.ProjectID,
		ExpectedRevision: 4, ClientMutationID: "project-reopen"}
	reopened, err := resolver.setProjectArchived(ctx, reopenInput.ProjectID,
		reopenInput.ExpectedRevision, reopenInput.ClientMutationID, false)
	if err != nil || reopened.Project.ArchivedAt != nil {
		t.Fatalf("reopen Project = %#v, %v", reopened, err)
	}
	assertProjectReceipt(t, resolver.Store, "project.reopen", reopenInput.ClientMutationID, struct {
		ProjectID        string `json:"projectId"`
		ExpectedRevision int    `json:"expectedRevision"`
	}{reopenInput.ProjectID, reopenInput.ExpectedRevision})
	current, err := resolver.projectDocument(ctx, created.Project.ProjectID)
	if err != nil {
		t.Fatal(err)
	}
	recoveryInput := model.UpdateProjectDocumentInput{ProjectID: created.Project.ProjectID,
		ExpectedRevision: 5, ExpectedDocumentDigest: current.Digest, Content: "# Recovered\n",
		ClientMutationID: "project-document-recovery"}
	encoded, err := json.Marshal(recoveryInput)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(encoded)
	recoveryCommand := store.ProjectCommand{ActorID: projectActorID, Name: "project.update",
		ClientMutationID: recoveryInput.ClientMutationID, RequestDigest: hex.EncodeToString(digest[:])}
	stage, next, err := home.PrepareProjectDocumentReplace(resolver.home, created.Project.ProjectID,
		updated.Project.Folder, current.Digest, recoveryInput.Content, recoveryCommand.RequestDigest)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.UpdateProject(ctx, created.Project.ProjectID, 5,
		store.ProjectChanges{DocumentChanged: true, DocumentDigest: next.Digest}, recoveryCommand, time.Now()); err != nil {
		t.Fatal(err)
	}
	if _, err := home.ReadProjectDocumentStage(resolver.home, stage.ProjectID, stage.RequestDigest); err != nil {
		t.Fatal(err)
	}
	recovered, err := resolver.updateProjectDocument(ctx, recoveryInput)
	if err != nil || recovered.Document.Content != recoveryInput.Content || recovered.Project.Revision != 6 {
		t.Fatalf("receipt recovery = %#v, %v", recovered, err)
	}
	if _, err := home.ReadProjectDocumentStage(resolver.home, stage.ProjectID, stage.RequestDigest); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("recovered stage remains: %v", err)
	}

	existingFolder := t.TempDir()
	if err := os.WriteFile(filepath.Join(existingFolder, "PROJECT.md"), []byte("# Existing\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	existing, err := resolver.createProject(ctx, model.CreateProjectInput{WorkspaceID: personalWorkspaceID,
		Name: "Adopt", Folder: &existingFolder, ClientMutationID: "project-existing"})
	if err != nil {
		t.Fatal(err)
	}
	existingDocument, err := resolver.projectDocument(ctx, existing.Project.ProjectID)
	if err != nil || existingDocument.Content != "# Existing\n" {
		t.Fatalf("existing Project document = %#v, %v", existingDocument, err)
	}
}

func assertProjectReceipt(t *testing.T, database *store.Store, name, clientID string, input any) {
	t.Helper()
	encoded, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(encoded)
	_, found, err := database.LookupProjectReceipt(context.Background(), store.ProjectCommand{
		ActorID: projectActorID, Name: name, ClientMutationID: clientID,
		RequestDigest: hex.EncodeToString(digest[:]),
	})
	if err != nil || !found {
		t.Fatalf("Project receipt %q changed: found %t, error %v", name, found, err)
	}
}

func TestProjectReceiptsUseNormalizedCommands(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	first, err := resolver.createProject(ctx, model.CreateProjectInput{
		WorkspaceID: personalWorkspaceID, Name: " Plan ", Description: " Exact ",
		ClientMutationID: "normalized-create",
	})
	if err != nil {
		t.Fatal(err)
	}
	replay, err := resolver.createProject(ctx, model.CreateProjectInput{
		WorkspaceID: personalWorkspaceID, Name: "Plan", Description: "Exact",
		ClientMutationID: "normalized-create",
	})
	if err != nil || replay.Project.ProjectID != first.Project.ProjectID || replay.EventCursor != first.EventCursor {
		t.Fatalf("normalized create replay = %#v, %v", replay, err)
	}
	nameWithSpace, name := " Updated ", "Updated"
	changed, err := resolver.updateProject(ctx, model.UpdateProjectInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 1, Name: &nameWithSpace, ClientMutationID: "normalized-update"})
	if err != nil {
		t.Fatal(err)
	}
	changedReplay, err := resolver.updateProject(ctx, model.UpdateProjectInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 1, Name: &name, ClientMutationID: "normalized-update"})
	if err != nil || changedReplay.EventCursor != changed.EventCursor {
		t.Fatalf("normalized update replay = %#v, %v", changedReplay, err)
	}
	folder := t.TempDir()
	folderWithSpace := " " + folder + " "
	moved, err := resolver.updateProject(ctx, model.UpdateProjectInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 2, Folder: &folderWithSpace, ClientMutationID: "normalized-folder"})
	if err != nil || moved.Project.Folder == nil || *moved.Project.Folder != folder {
		t.Fatalf("normalized folder = %#v, %v", moved, err)
	}
	conflictFolder := t.TempDir()
	if err := os.WriteFile(filepath.Join(conflictFolder, "PROJECT.md"), []byte("# Other\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	_, err = resolver.updateProject(ctx, model.UpdateProjectInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 3, Folder: &conflictFolder, ClientMutationID: "conflicting-folder"})
	var graphQLError *gqlerror.Error
	if !errors.As(err, &graphQLError) || graphQLError.Extensions["code"] != "invalid_input" {
		t.Fatalf("folder conflict error = %#v", err)
	}
	if _, err := resolver.updateProject(ctx, model.UpdateProjectInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 0, Name: &name, ClientMutationID: "bad-revision"}); err == nil {
		t.Fatal("nonpositive Project revision was accepted")
	}
	if _, err := resolver.updateProjectDocument(ctx, model.UpdateProjectDocumentInput{ProjectID: first.Project.ProjectID,
		ExpectedRevision: 2, ExpectedDocumentDigest: "bad", Content: "x", ClientMutationID: "bad-digest"}); err == nil {
		t.Fatal("malformed Project document digest was accepted")
	}
}

func TestTaskEventReplayUsesSharedGlobalCursor(t *testing.T) {
	resolver := openTestResolver(t)
	ctx := context.Background()
	captured, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID,
		Title: "Shared cursor", TaskDocument: "Task", ClientMutationID: "shared-task"})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.createProject(ctx, model.CreateProjectInput{WorkspaceID: personalWorkspaceID,
		Name: "Interleaved", ClientMutationID: "shared-project"}); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.StartTask(ctx, captured.Task.TaskID, "run:shared", time.Now()); err != nil {
		t.Fatal(err)
	}
	after := captured.EventCursor
	taskCtx, cancelTask := context.WithCancel(ctx)
	stream, err := resolver.taskEvents(taskCtx, captured.Task.TaskID, &after)
	if err != nil {
		t.Fatal(err)
	}
	event := <-stream
	wantThird, _ := store.EncodeWorkEventCursor(3)
	if event.Kind != "task.started" || event.Cursor != wantThird {
		t.Fatalf("Task event = %#v", event)
	}
	cancelTask()
	workspaceCtx, cancelWorkspace := context.WithCancel(ctx)
	all, err := resolver.tasksEvents(workspaceCtx, personalWorkspaceID, &after)
	if err != nil {
		t.Fatal(err)
	}
	first, second := <-all, <-all
	if first.ProjectID == nil || second.TaskID == nil || wantThird != second.Cursor {
		t.Fatalf("workspace events = %#v, %#v", first, second)
	}
	liveCtx, cancelLive := context.WithCancel(ctx)
	live, err := resolver.taskEvents(liveCtx, captured.Task.TaskID, nil)
	if err != nil {
		t.Fatal(err)
	}
	select {
	case historical := <-live:
		t.Fatalf("cursor-free subscription replayed history: %#v", historical)
	case <-time.After(25 * time.Millisecond):
	}
	if _, err := resolver.Store.FinishTask(ctx, captured.Task.TaskID, "run:shared", store.TaskCompleted, time.Now()); err != nil {
		t.Fatal(err)
	}
	if completed := <-live; completed.Kind != "task.completed" {
		t.Fatalf("live runtime event = %#v", completed)
	}
	cancelLive()
	if workspaceCompleted := <-all; workspaceCompleted.Kind != "task.completed" {
		t.Fatalf("workspace runtime event = %#v", workspaceCompleted)
	}
	for index := 0; index < 120; index++ {
		taskID, _ := store.NewTaskID()
		if _, err := resolver.Store.CreateTask(ctx, taskID, "Backpressure", "correlation:test:"+taskID, time.Now()); err != nil {
			t.Fatal(err)
		}
	}
	lastSequence := int64(4)
	for index := 0; index < 120; index++ {
		event := <-all
		sequence, err := store.DecodeWorkEventCursor(event.Cursor)
		if err != nil || sequence != lastSequence+1 {
			t.Fatalf("backpressure event %d = %#v, %v", index, event, err)
		}
		lastSequence = sequence
	}
	cancelWorkspace()
}
