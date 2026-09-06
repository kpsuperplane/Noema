package main

import (
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/schedule"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestDesktopStartupInputIsBoundedAndLeavesShutdownPipeOpen(t *testing.T) {
	token := base64.RawURLEncoding.EncodeToString(make([]byte, 32))
	runtimeRoot := filepath.Join(t.TempDir(), "runtime")
	input := fmt.Sprintf(`{"token":%q,"runtimeRoot":%q}`+"\nshutdown", token, runtimeRoot)
	options, reader := readDesktopOptions(strings.NewReader(input))
	if options == nil || options.Token != token || options.RuntimeRoot != runtimeRoot {
		t.Fatalf("desktop options = %#v", options)
	}
	remainder, err := io.ReadAll(reader)
	if err != nil || string(remainder) != "shutdown" {
		t.Fatalf("startup remainder = %q, %v", remainder, err)
	}
	if invalid, _ := readDesktopOptions(strings.NewReader(`{"token":"x","extra":true}` + "\n")); invalid != nil {
		t.Fatal("unknown desktop startup field was accepted")
	}
	called := false
	handler := desktopHandler(token, http.HandlerFunc(func(response http.ResponseWriter, request *http.Request) {
		if request.URL.Path == "/mcp/oauth/callback" {
			response.WriteHeader(http.StatusNoContent)
			return
		}
		called = auth.DesktopAccess(request.Context())
	}), http.NotFoundHandler())
	request := httptest.NewRequest(http.MethodPost, "/graphql", nil)
	request.Header.Set("Authorization", "Bearer "+token)
	handler.ServeHTTP(httptest.NewRecorder(), request)
	if !called {
		t.Fatal("desktop credential did not authorize GraphQL")
	}
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/mcp/oauth/callback?code=x", nil))
	if response.Code != http.StatusNoContent {
		t.Fatalf("desktop callback response = %d", response.Code)
	}
}

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
		"Committed", "", nil, stage.Document.Digest, false, command, time.Now()); err != nil {
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

func TestRecurrenceDocumentRecoveryUsesTaskReceipt(t *testing.T) {
	root, database := openScheduleTestHome(t)
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	_, recurrenceID := createScheduleTestRecurrence(t, root, database, now, now.Add(time.Minute))
	current, err := home.EnsureRecurrenceDocument(root, recurrenceID, "# Original\n")
	if err != nil {
		t.Fatal(err)
	}
	command := store.TaskCommand{Name: "update_task_recurrence", ClientMutationID: "update",
		RequestDigest: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
		CorrelationID: "correlation:test:update"}
	stage, err := home.PrepareRecurrenceDocumentReplace(root, recurrenceID, current.Digest,
		"# Recovered\n", command.RequestDigest)
	if err != nil {
		t.Fatal(err)
	}
	title := "Recovered"
	if _, err := database.UpdateTaskRecurrence(context.Background(), recurrenceID, 1,
		store.RecurrenceChanges{Title: &title}, command, now); err != nil {
		t.Fatal(err)
	}
	if err := recoverRecurrenceDocuments(context.Background(), root, database); err != nil {
		t.Fatal(err)
	}
	document, err := home.ReadRecurrenceDocument(root, recurrenceID)
	if err != nil || document != stage.Document {
		t.Fatalf("recovered recurrence document = %#v, %v", document, err)
	}
}

func TestDueRecurrenceStagesTaskDocumentBeforeCommit(t *testing.T) {
	root, database := openScheduleTestHome(t)
	now := time.Date(2026, 9, 4, 12, 5, 0, 0, time.UTC)
	_, recurrenceID := createScheduleTestRecurrence(t, root, database, now, now.Add(-2*time.Minute))
	if _, err := home.EnsureRecurrenceDocument(root, recurrenceID, "# Occurrence\n"); err != nil {
		t.Fatal(err)
	}
	created, _, err := database.ProcessDueTaskSchedules(context.Background(), now, true,
		func(value store.DueTask) error {
			return home.StageRecurrenceDocumentToTask(root, value.RecurrenceID, value.TaskID)
		})
	if err != nil || len(created) != 1 {
		t.Fatalf("due recurrence = %#v, %v", created, err)
	}
	if err := home.RecoverTaskDocuments(root, func(taskID string) (bool, error) {
		return database.TaskExists(context.Background(), taskID)
	}); err != nil {
		t.Fatal(err)
	}
	document, err := home.ReadTaskDocument(root, created[0].TaskID)
	if err != nil || document.Content != "# Occurrence\n" {
		t.Fatalf("recovered occurrence document = %#v, %v", document, err)
	}
}

func TestTaskScheduleLoopRetriesStoreErrors(t *testing.T) {
	root, database := openScheduleTestHome(t)
	if err := database.Close(); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer cancel()
	if err := taskScheduleLoop(ctx, root, database, io.Discard); err != nil {
		t.Fatalf("schedule loop returned a store error: %v", err)
	}
}

func openScheduleTestHome(t *testing.T) (*os.Root, *store.Store) {
	t.Helper()
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
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
	t.Cleanup(func() { _ = database.Close() })
	return root, database
}

func createScheduleTestRecurrence(
	t *testing.T, root *os.Root, database *store.Store, now, first time.Time,
) (string, string) {
	t.Helper()
	taskID, _ := store.NewTaskID()
	if _, err := home.CreatePendingTaskDocument(root, taskID, "# Source\n"); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(root, taskID); err != nil {
		t.Fatal(err)
	}
	value, err := schedule.Normalize(schedule.Schedule{ScheduledFor: first, TimeZone: "UTC",
		MissedRunPolicy: schedule.MissedRunOnce, Recurrence: &schedule.Recurrence{StartsAt: first,
			CronExpression: "* * * * *", OverlapPolicy: schedule.OverlapAllow}}, now)
	if err != nil {
		t.Fatal(err)
	}
	command := store.TaskCommand{Name: "capture_task", ClientMutationID: "capture",
		RequestDigest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		CorrelationID: "correlation:test:capture"}
	result, err := database.CreateTaskWithOptions(context.Background(), taskID, "Recurring", command,
		store.TaskCreateOptions{Schedule: &value}, now)
	if err != nil {
		t.Fatal(err)
	}
	return taskID, result.RecurrenceID
}
