package store

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"strings"
	"testing"
	"time"
)

func TestProjectCommandsReceiptsAndLifecycle(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	id, _ := NewProjectID()
	create := testProjectCommand("project.create", "create", "create")
	documentDigest := testProjectDigest("# Plan\n\nExact\n")
	result, err := database.CreateProject(ctx, id, "workspace:personal", "Plan", "Exact", nil, documentDigest, create, now)
	if err != nil || result.Project.Revision != 1 || result.Event.Kind != "project.created" {
		t.Fatalf("create Project = %#v, %v", result, err)
	}
	if result.Event.ActorID != "actor:human:local" || result.Event.CorrelationID != "correlation:graphql:create" {
		t.Fatalf("create audit event = %#v", result.Event)
	}
	if !strings.HasPrefix(result.Event.EventID, "event:") || result.Event.Payload["v"] != 1 {
		t.Fatalf("create event identity or payload = %#v", result.Event)
	}
	cursor, err := EncodeWorkEventCursor(result.Event.ID)
	if err != nil {
		t.Fatal(err)
	}
	if decoded, err := DecodeWorkEventCursor(cursor); err != nil || decoded != result.Event.ID {
		t.Fatalf("event cursor %q decoded as %d, %v", cursor, decoded, err)
	}
	if _, err := DecodeWorkEventCursor("1"); !errors.Is(err, ErrInvalidCursor) {
		t.Fatalf("decimal event cursor error = %v", err)
	}
	replay, err := database.CreateProject(ctx, id, "workspace:personal", "Plan", "Exact", nil, documentDigest, create, now)
	if err != nil || !replay.Replayed || replay.Event.ID != result.Event.ID {
		t.Fatalf("create replay = %#v, %v", replay, err)
	}
	name := "Updated"
	updated, err := database.UpdateProject(ctx, id, 1, ProjectChanges{Name: &name},
		testProjectCommand("project.update", "update", "update"), now.Add(time.Second))
	if err != nil || updated.Project.Name != name || updated.Project.Revision != 2 {
		t.Fatalf("update Project = %#v, %v", updated, err)
	}
	archived, err := database.SetProjectArchived(ctx, id, 2, true,
		testProjectCommand("project.archive", "archive", "archive"), now.Add(2*time.Second))
	if err != nil || archived.Project.ArchivedAt == nil {
		t.Fatalf("archive Project = %#v, %v", archived, err)
	}
	reopened, err := database.SetProjectArchived(ctx, id, 3, false,
		testProjectCommand("project.reopen", "reopen", "reopen"), now.Add(3*time.Second))
	if err != nil || reopened.Project.ArchivedAt != nil || reopened.Project.Revision != 4 {
		t.Fatalf("reopen Project = %#v, %v", reopened, err)
	}
	events, err := database.WorkEvents(ctx, "workspace:personal", 0, 100)
	if err != nil || len(events) != 4 {
		t.Fatalf("Project events = %d, %v", len(events), err)
	}
}

func TestProjectReceiptRejectsChangedRequest(t *testing.T) {
	database := openTestStore(t)
	id, _ := NewProjectID()
	command := testProjectCommand("project.create", "same", "first")
	if _, err := database.CreateProject(context.Background(), id, "workspace:personal", "One", "", nil,
		testProjectDigest("# One\n"), command, time.Now()); err != nil {
		t.Fatal(err)
	}
	command.RequestDigest = testProjectCommand("project.create", "same", "second").RequestDigest
	_, _, err := database.LookupProjectReceipt(context.Background(), command)
	if !errors.Is(err, ErrCommandConflict) {
		t.Fatalf("receipt conflict = %v", err)
	}
}

func TestProjectPaginationUsesEveryEdgeCursor(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	for index := 0; index < 3; index++ {
		id, _ := NewProjectID()
		_, err := database.CreateProject(ctx, id, "workspace:personal", string(rune('A'+index)), "", nil,
			testProjectDigest(string(rune('A'+index))),
			testProjectCommand("project.create", string(rune('a'+index)), string(rune('a'+index))), now.Add(time.Duration(index)*time.Second))
		if err != nil {
			t.Fatal(err)
		}
	}
	page, err := database.ListProjects(ctx, "workspace:personal", false, 2, nil)
	if err != nil || len(page.Projects) != 2 || len(page.Cursors) != 2 || !page.HasNextPage {
		t.Fatalf("first Project page = %#v, %v", page, err)
	}
	for index := range page.Cursors {
		next, err := database.ListProjects(ctx, "workspace:personal", false, 2, &page.Cursors[index])
		if err != nil || len(next.Projects) != 2-index {
			t.Fatalf("cursor %d page = %#v, %v", index, next, err)
		}
	}
}

func testProjectCommand(name, clientID, value string) ProjectCommand {
	digest := sha256.Sum256([]byte(value))
	return ProjectCommand{ActorID: "actor:human:local", Name: name, ClientMutationID: clientID,
		RequestDigest: hex.EncodeToString(digest[:])}
}

func testProjectDigest(value string) string {
	digest := sha256.Sum256([]byte(value))
	return hex.EncodeToString(digest[:])
}
