package store

import (
	"context"
	"database/sql"
	"errors"
	"path/filepath"
	"testing"
	"time"
)

func TestLocalModelSchemaLifecycleAssignmentsAndEvents(t *testing.T) {
	ctx := context.Background()
	legacyPath := filepath.Join(t.TempDir(), "v30.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(legacyPath))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(schemaAtVersion(30) + "\nPRAGMA user_version=30;"); err != nil {
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}
	database, err := Open(ctx, legacyPath)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Now()); err != nil {
		t.Fatal(err)
	}

	var version int
	if err := database.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("schema version = %d, %v", version, err)
	}
	hosted := createReadyModelAccount(t, database)
	if created, err := database.ConfirmHostedModelAssignments(
		ctx, hosted.ID, testModelAssignments(hosted, "model-a"),
	); err != nil || !created {
		t.Fatalf("seed hosted assignments = %v, %v", created, err)
	}

	now := time.Now().UTC()
	first := installStoreModel(t, database, "installation:one", "local-one", "a", now)
	active, err := database.ActivateLocalModel(ctx, first.ID, true, now.Add(time.Second))
	if err != nil || !active.Active {
		t.Fatalf("activate local model = %#v, %v", active, err)
	}
	assignments, err := database.HostedModelAssignments(ctx)
	if err != nil || len(assignments) != len(hostedModelRoles) {
		t.Fatalf("routed assignments = %#v, %v", assignments, err)
	}
	for _, assignment := range assignments {
		if assignment.Role == HostedModelActionReviewer {
			if assignment.ProviderKind != hosted.ProviderKind {
				t.Fatalf("action reviewer changed to %q", assignment.ProviderKind)
			}
			continue
		}
		if assignment.ProviderKind != "local_models" || assignment.ModelProfile != first.ModelID {
			t.Fatalf("role %s was not routed locally: %#v", assignment.Role, assignment)
		}
	}
	preference, err := database.DefaultModelPreference(ctx)
	if err != nil || preference == nil || preference.ModelProfile != first.ModelID {
		t.Fatalf("default preference = %#v, %v", preference, err)
	}
	if _, err := database.RemoveLocalModel(ctx, first.ID, now); !errors.Is(err, ErrLocalModelState) {
		t.Fatalf("active removal error = %v", err)
	}

	second := installStoreModel(t, database, "installation:two", "local-two", "b", now)
	if _, err := database.RemoveLocalModel(ctx, second.ID, now); err != nil {
		t.Fatalf("remove unused model: %v", err)
	}
	events, err := database.LocalModelEvents(ctx, 0, 256)
	if err != nil || len(events) < 9 {
		t.Fatalf("local events = %#v, %v", events, err)
	}
	latest, err := database.LatestLocalModelEvent(ctx)
	if err != nil || latest.Cursor != events[len(events)-1].Cursor {
		t.Fatalf("latest event = %#v, %v", latest, err)
	}

	fresh := openTestStore(t)
	var table string
	if err := fresh.db.QueryRow(
		"SELECT name FROM sqlite_schema WHERE type='table' AND name='local_model_installations'",
	).Scan(&table); err != nil || table == "" {
		t.Fatalf("fresh local model schema = %q, %v", table, err)
	}
}

func installStoreModel(
	t *testing.T,
	database *Store,
	id, model, digestCharacter string,
	now time.Time,
) LocalModelInstallation {
	t.Helper()
	digest := ""
	for range 64 {
		digest += digestCharacter
	}
	_, err := database.QueueLocalModel(context.Background(), LocalModelInstallation{
		ID: id, ModelID: model, Name: model, File: model + ".gguf",
		SourceKind: "local_file", Backend: "cpu", TotalBytes: 4, CreatedAt: now,
	})
	if err != nil {
		t.Fatal(err)
	}
	_, err = database.UpdateLocalModel(context.Background(), id, "downloading", 4, 4, 0, "", "", "", "", now)
	if err != nil {
		t.Fatal(err)
	}
	_, err = database.UpdateLocalModel(context.Background(), id, "verifying", 4, 4, 0, "", "", "", "", now)
	if err != nil {
		t.Fatal(err)
	}
	value, err := database.UpdateLocalModel(
		context.Background(), id, "installed", 4, 4, 4, digest,
		"models/blobs/"+digest+".gguf", "", "", now,
	)
	if err != nil {
		t.Fatal(err)
	}
	return value
}
