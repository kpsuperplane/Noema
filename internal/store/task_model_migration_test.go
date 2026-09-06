package store

import (
	"database/sql"
	"path/filepath"
	"testing"
)

func TestTaskModelMigrationPreservesConfiguredChoices(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v35.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(schemaAtVersion(35) + `PRAGMA user_version=35;
INSERT INTO provider_accounts(provider_account_id,provider_kind,account_key,display_name,auth_method,is_active,is_default,status,created_at_ms,updated_at_ms) VALUES ('provider:test','openai','test','Test','secret_input',1,1,'authenticated',1,1);
INSERT INTO hosted_model_assignments(role,provider_kind,provider_account_id,selection_mode,model_profile,reasoning_effort,fast_mode) VALUES ('simple_tasks','openai','provider:test','explicit_profile','kept-model','high',1);
UPDATE task_model_pool_settings SET enabled=0, label='Routine', sort_order=7 WHERE complexity='simple';`); err != nil {
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}
	upgraded, err := Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer upgraded.Close()
	for _, database := range []*Store{upgraded, openTestStore(t)} {
		var columns, version int
		if err := database.db.QueryRow("SELECT count(*) FROM pragma_table_info('task_model_pool_settings') WHERE name='enabled'").Scan(&columns); err != nil || columns != 0 {
			t.Fatalf("enabled columns = %d: %v", columns, err)
		}
		if err := database.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
			t.Fatalf("version = %d: %v", version, err)
		}
	}
	var label string
	var order int
	if err := upgraded.db.QueryRow("SELECT label,sort_order FROM task_model_pool_settings WHERE complexity='simple'").Scan(&label, &order); err != nil || label != "Routine" || order != 7 {
		t.Fatalf("preserved choice = %q, %d: %v", label, order, err)
	}
	entries, err := upgraded.TaskModelPoolEntries(t.Context(), nil)
	if err != nil || len(entries) != 1 || entries[0].Assignment.ModelProfile != "kept-model" || !entries[0].Assignment.FastMode || entries[0].Assignment.ReasoningEffort != "high" {
		t.Fatalf("preserved assignments = %#v: %v", entries, err)
	}
}
