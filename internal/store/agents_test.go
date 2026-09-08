package store

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"
	"time"
)

func TestAgentSchemaConvergesAndRepairsBuiltIns(t *testing.T) {
	path := filepath.Join(t.TempDir(), "version-nine.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(schemaAtVersion(9) +
		"\nPRAGMA user_version = 9;"); err != nil {
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}
	database, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	agents, err := database.Agents(context.Background())
	if err != nil || len(agents) != 3 {
		t.Fatalf("upgraded Agents = %#v, %v", agents, err)
	}
	if agents[0].ID != PrimaryAgentID ||
		agents[0].DisplayName != nil || agents[2].ID != TaskReviewerAgentID {
		t.Fatalf("built-in Agents = %#v, %v", agents, err)
	}
	if _, err := database.db.Exec(`
UPDATE agents SET display_name = 'Noema', system_role = NULL WHERE agent_id = ?`, PrimaryAgentID); err != nil {
		t.Fatal(err)
	}
	if err := database.Close(); err != nil {
		t.Fatal(err)
	}
	database, err = Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	primary, err := database.Agent(context.Background(), PrimaryAgentID)
	if err != nil || primary.SystemRole == nil || *primary.SystemRole != "primary" ||
		primary.DisplayName == nil || *primary.DisplayName != "Noema" {
		t.Fatalf("repaired primary Agent = %#v, %v", primary, err)
	}
}

func TestAgentPreferenceAndTaskModelPoolsShareAssignments(t *testing.T) {
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	assignments := testModelAssignments(account, "model-a")
	if created, err := database.ConfirmHostedModelAssignments(context.Background(), account.ID, assignments); err != nil || !created {
		t.Fatalf("confirm assignments = %v, %v", created, err)
	}
	primary := assignments[0]
	primary.ModelProfile = "model-b"
	saved, err := database.SaveHostedModelAssignment(context.Background(), primary)
	if err != nil || saved.ModelProfile != "model-b" {
		t.Fatalf("save primary preference = %#v, %v", saved, err)
	}
	pools, err := database.TaskModelPoolEntries(context.Background(), nil)
	if err != nil || len(pools) != 3 || pools[0].ID != "task_pool:setting:simple" {
		t.Fatalf("Task model pools = %#v, %v", pools, err)
	}
	var simple TaskModelPoolEntry
	for _, entry := range pools {
		if entry.Complexity == "simple" {
			simple = entry
		}
	}
	label := "Routine"
	updated, err := database.UpdateTaskModelPoolEntry(context.Background(), simple.ID, "simple",
		&label, simple.Assignment, simple.Enabled, 2, time.Now())
	if err != nil || updated.Label == nil || *updated.Label != label || updated.SortOrder != 2 {
		t.Fatalf("updated Task model pool = %#v, %v", updated, err)
	}
}
