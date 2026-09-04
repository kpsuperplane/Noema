package store

import (
	"context"
	"database/sql"
	"errors"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestAgentSchemaConvergesAndRepairsBuiltIns(t *testing.T) {
	path := filepath.Join(t.TempDir(), "version-nine.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(strings.TrimSuffix(strings.TrimSuffix(schemaSQL, schemaV11SQL), schemaV10SQL) +
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

func TestAcpAgentConfigurationRevisionAndAuthentication(t *testing.T) {
	path := filepath.Join(t.TempDir(), "noema.sqlite3")
	database, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	if _, err := database.CreateAcpAgent(context.Background(), "Agent", "run", []string{strings.Repeat("x", 4097)}, now); !errors.Is(err, ErrInvalidAcpAgent) {
		t.Fatalf("oversized argument error = %v", err)
	}
	created, err := database.CreateAcpAgent(context.Background(), "  Local Agent  ", "  run  ", []string{"--stdio"}, now)
	if err != nil || created.DisplayName != "Local Agent" || created.Command != "run" ||
		created.ConnectionRevision != 1 || !created.Enabled {
		t.Fatalf("created ACP Agent = %#v, %v", created, err)
	}
	updated, err := database.UpdateAcpAgent(context.Background(), created.AgentID, 1,
		"Renamed", "agent-bin", []string{"serve"}, false, now.Add(time.Minute))
	if err != nil || updated.ConnectionRevision != 2 || updated.Enabled ||
		updated.HealthStatus != "unknown" || updated.AuthStatus != "unknown" {
		t.Fatalf("updated ACP Agent = %#v, %v", updated, err)
	}
	if _, err := database.UpdateAcpAgent(context.Background(), created.AgentID, 1,
		"Stale", "run", nil, true, now); !errors.Is(err, ErrAcpAgentRevisionConflict) {
		t.Fatalf("stale update error = %v", err)
	}
	name, version, message := "test-agent", "1.0", "ready"
	probed, err := database.RecordAcpAgentProbe(context.Background(), created.AgentID, 2,
		"healthy", "required", &name, &version,
		map[string]any{"authMethods": []any{map[string]any{"id": "login"}}}, &message, now)
	if err != nil || probed.HealthStatus != "healthy" || probed.AuthStatus != "required" {
		t.Fatalf("probed ACP Agent = %#v, %v", probed, err)
	}
	attemptID, err := database.BeginAcpAuthentication(context.Background(), created.AgentID, 2, "login", now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.DeleteAcpAgent(context.Background(), created.AgentID, 2); !errors.Is(err, ErrAcpAgentAuthenticationBusy) {
		t.Fatalf("active authentication delete error = %v", err)
	}
	authenticated, err := database.FinishAcpAuthentication(context.Background(), attemptID, true, nil, now)
	if err != nil || authenticated.AuthStatus != "authenticated" {
		t.Fatalf("authenticated ACP Agent = %#v, %v", authenticated, err)
	}
	recoveryAttempt, err := database.BeginAcpAuthentication(context.Background(), created.AgentID, 2, "login", now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.BeginAcpAuthentication(context.Background(), created.AgentID, 2, "login", now); !errors.Is(err, ErrAcpAgentAuthenticationBusy) {
		t.Fatalf("concurrent authentication error = %v", err)
	}
	if err := database.Close(); err != nil {
		t.Fatal(err)
	}
	database, err = Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	recovered, err := database.AcpAgent(context.Background(), created.AgentID)
	if err != nil || recovered.AuthStatus != "failed" || recovered.LastError == nil {
		t.Fatalf("recovered ACP Agent = %#v, %v", recovered, err)
	}
	if _, err := database.FinishAcpAuthentication(context.Background(), recoveryAttempt, true, nil, now); !errors.Is(err, ErrAcpAuthenticationNotPending) {
		t.Fatalf("recovered attempt completion error = %v", err)
	}
	deleted, err := database.DeleteAcpAgent(context.Background(), created.AgentID, 2)
	if err != nil || !deleted {
		t.Fatalf("delete ACP Agent = %v, %v", deleted, err)
	}
	if _, err := database.AcpAgent(context.Background(), created.AgentID); !errors.Is(err, ErrAcpAgentNotFound) {
		t.Fatalf("deleted ACP Agent error = %v", err)
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
	changed := simple.Assignment
	changed.ModelProfile = "model-b"
	if _, err := database.UpdateTaskModelPoolEntry(context.Background(), simple.ID, "simple",
		nil, changed, false, 0, time.Now()); err == nil {
		t.Fatal("disabled pool route change succeeded")
	}
	label := "Routine"
	updated, err := database.UpdateTaskModelPoolEntry(context.Background(), simple.ID, "simple",
		&label, simple.Assignment, false, 2, time.Now())
	if err != nil || updated.Enabled || updated.Label == nil || *updated.Label != label || updated.SortOrder != 2 {
		t.Fatalf("updated Task model pool = %#v, %v", updated, err)
	}
}
