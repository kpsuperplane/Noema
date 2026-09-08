package store

import (
	"database/sql"
	"errors"
	"reflect"
	"testing"
	"time"
)

func TestRetiredExecutorUpgradePreservesHistoryAndStopsExecution(t *testing.T) {
	database := openRustStoreMigrationFixtureWithSetup(t, 37, func(db *sql.DB) error {
		_, err := db.Exec(`
INSERT INTO agents(agent_id,display_name,created_at_ms,updated_at_ms) VALUES('agent:retired','Retired executor',1,1);
INSERT INTO acp_agents(agent_id,command,created_at_ms,updated_at_ms) VALUES('agent:retired','must-not-launch',1,1);
INSERT INTO acp_auth_attempts(attempt_id,agent_id,connection_revision,method_id,state,created_at_ms)
 VALUES('acp_auth:retired','agent:retired',1,'login','pending',1);
INSERT INTO tasks(task_id,title,state,current_run_id,revision,created_at_ms,updated_at_ms,executor_agent_id,executor_acp_connection_revision)
 VALUES('task:retired','Keep my work','running','run:retired',1,1,1,'agent:retired',1),
 ('task:history','Completed work','completed',NULL,1,1,1,'agent:retired',1),
 ('task:provider','Provider work','running','run:provider',1,1,1,'agent:task-executor',NULL);
INSERT INTO task_runs(run_id,task_id,task_generation,instance_name,run_kind,status,agent_id,executor_backend,executor_agent_id,queued_at_ms,created_at_ms,updated_at_ms)
 VALUES('run:retired','task:retired',1,'Executor','executor','running','agent:retired','acp','agent:retired',1,1,1),
 ('run:history','task:history',1,'Executor','executor','completed','agent:retired','acp','agent:retired',1,1,1),
 ('run:provider','task:provider',1,'Executor','executor','running','agent:task-executor','provider','agent:task-executor',1,1,1);
INSERT INTO task_run_items(item_id,run_id,sequence_index,item_kind,status,content_text,created_at_ms,updated_at_ms)
 VALUES('run_item:retired','run:retired',0,'tool_call','running','Keep exact evidence',1,1);
INSERT INTO task_gates(gate_id,task_id,task_generation,gate_kind,gate_state,prompt,opened_by,opened_at_ms)
 VALUES('gate:retired','task:retired',1,'approval','open','Pending approval','actor:system',1);
INSERT INTO task_recurrences(recurrence_id,workspace_id,title,executor_agent_id,executor_acp_connection_revision,starts_at_ms,cron_expression,time_zone,missed_run_policy,overlap_policy,lifecycle,next_run_at_ms,created_at_ms,updated_at_ms)
 VALUES('recurrence:retired','workspace:personal','Keep schedule','agent:retired',1,1,'0 9 * * *','UTC','skip','skip','active',1,1,1);
`)
		return err
	})
	ctx := t.Context()
	task, err := database.Task(ctx, "task:retired")
	if err != nil || task.State != TaskFailed || task.CurrentRunID != "" || task.Generation != 2 || task.Title != "Keep my work" {
		t.Fatalf("retired Task = %#v: %v", task, err)
	}
	run, err := taskRunTx(ctx, database.db, "run:retired")
	if err != nil || run.Status != "failed" || run.ErrorCode == nil || *run.ErrorCode != "unsupported_executor" {
		t.Fatalf("retired run = %#v: %v", run, err)
	}
	var status, content string
	if err := database.db.QueryRowContext(ctx, "SELECT status,content_text FROM task_run_items WHERE item_id='run_item:retired'").Scan(&status, &content); err != nil || status != "interrupted" || content != "Keep exact evidence" {
		t.Fatalf("retained evidence = %q %q: %v", status, content, err)
	}
	if err := database.db.QueryRowContext(ctx, "SELECT gate_state FROM task_gates WHERE gate_id='gate:retired'").Scan(&status); err != nil || status != "superseded" {
		t.Fatalf("retired gate = %q: %v", status, err)
	}
	for _, id := range []string{"task:history", "task:provider"} {
		got, err := database.Task(ctx, id)
		want := TaskCompleted
		if id == "task:provider" {
			want = TaskRunning
		}
		if err != nil || got.State != want || got.Generation != 1 {
			t.Fatalf("preserved Task = %#v: %v", got, err)
		}
	}
	recurrence, err := database.TaskRecurrence(ctx, "recurrence:retired")
	if err != nil || recurrence.Lifecycle != RecurrencePaused || recurrence.Title != "Keep schedule" {
		t.Fatalf("retired recurrence = %#v: %v", recurrence, err)
	}
	if _, err := database.SetTaskRecurrenceLifecycle(ctx, recurrence.ID, recurrence.Revision, RecurrenceActive, testTaskLifecycleCommand("recurrence.resume", "retired"), time.Now()); !errors.Is(err, ErrUnsupportedTaskExecutor) {
		t.Fatalf("resume retired recurrence = %v", err)
	}
	tx, err := database.db.BeginTx(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback()
	if _, err := insertQueuedTaskRun(ctx, tx, task, "planner", nil, time.Now()); !errors.Is(err, ErrUnsupportedTaskExecutor) {
		t.Fatalf("retired planner fallback = %v", err)
	}
	if _, err := validateTaskExecutorTx(ctx, tx, "agent:retired"); !errors.Is(err, ErrUnsupportedTaskExecutor) {
		t.Fatalf("retired executor selection = %v", err)
	}
	if err := tx.Rollback(); err != nil {
		t.Fatal(err)
	}
	if err := database.RecoverTaskExecutions(ctx, time.Now()); err != nil {
		t.Fatal(err)
	}
	claimed, _, found, err := database.ClaimTaskExecution(ctx, time.Now())
	if err != nil || !found || claimed.ID != "task:provider" {
		t.Fatalf("claim after retirement = %#v, %t: %v", claimed, found, err)
	}
	agents, err := database.Agents(ctx)
	if err != nil || len(agents) != 3 {
		t.Fatalf("supported agents = %#v: %v", agents, err)
	}
	fresh := openTestStore(t)
	schema := func(s *Store) []string {
		t.Helper()
		var result []string
		if err := s.db.NewRaw("SELECT type || ':' || name || ':' || sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY type,name").Scan(ctx, &result); err != nil {
			t.Fatal(err)
		}
		return result
	}
	if !reflect.DeepEqual(schema(database), schema(fresh)) {
		t.Fatal("upgraded and fresh schemas differ")
	}
	for _, name := range []string{"acp_agents", "acp_auth_attempts"} {
		if rustStoreSchemaObject(t, database, "table", name) || rustStoreSchemaObject(t, fresh, "table", name) {
			t.Fatalf("retired table remains: %s", name)
		}
	}
}
