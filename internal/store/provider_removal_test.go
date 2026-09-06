package store

import (
	"database/sql"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestAppleModelRemovalPreservesOtherSelectionsAndHistory(t *testing.T) {
	for _, defaultKind := range []string{"foundation_local", "openai"} {
		t.Run(defaultKind, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "v32.sqlite3")
			legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
			if err != nil {
				t.Fatal(err)
			}
			defer legacy.Close()
			if _, err := legacy.Exec(schemaAtVersion(32) + "PRAGMA user_version=32;"); err != nil {
				t.Fatal(err)
			}
			for _, kind := range []string{"foundation_local", "openai", "local_models"} {
				if _, err := legacy.Exec(`INSERT INTO provider_accounts
(provider_account_id,provider_kind,account_key,display_name,auth_method,is_active,is_default,status,metadata_json,created_at_ms,updated_at_ms)
VALUES (?,?,'default',?,'none',1,1,'authenticated','{"credentialRevision":7}',1,1)`, "provider_account:"+kind+":default", kind, kind); err != nil {
					t.Fatal(err)
				}
			}
			if _, err := legacy.Exec(`
INSERT INTO hosted_model_assignments VALUES ('noema','foundation_local','provider_account:foundation_local:default','explicit_profile','default',NULL,0);
INSERT INTO hosted_model_assignments VALUES ('action_reviewer','openai','provider_account:openai:default','explicit_profile','kept','low',1);
INSERT INTO hosted_model_assignments VALUES ('simple_tasks','local_models','provider_account:local_models:default','explicit_profile','local-kept',NULL,0);
INSERT INTO conversations(conversation_id,owner_human_id,provider,created_at_ms,updated_at_ms)
VALUES ('conversation:00000000000000000000000000000001','human:local','foundation_local',1,1);
INSERT INTO tasks(task_id,title,state,stage_key,revision,created_at_ms,updated_at_ms)
VALUES ('task:00000000000000000000000000000001','Kept history','completed','done',1,1,1);
INSERT INTO task_runs(run_id,task_id,task_generation,instance_name,run_kind,status,agent_id,provider_kind,provider_account_id,
executor_backend,executor_agent_id,queued_at_ms,created_at_ms,updated_at_ms)
VALUES ('run:kept','task:00000000000000000000000000000001',1,'primary','executor','completed','agent:task-executor',
'foundation_local','provider_account:foundation_local:default','model','agent:task-executor',1,1,1);
`); err != nil {
				t.Fatal(err)
			}
			if _, err := legacy.Exec(`INSERT INTO default_model_preference VALUES
('default',?,?,'explicit_profile','chosen',NULL,0,1)`, defaultKind, "provider_account:"+defaultKind+":default"); err != nil {
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
			if err := upgraded.EnsureBuiltinProviderAccounts(t.Context(), time.Now()); err != nil {
				t.Fatal(err)
			}
			checks := map[string]int{
				"PRAGMA user_version": schemaVersion,
				"SELECT count(*) FROM provider_accounts WHERE provider_kind='foundation_local'":                                                                                                          0,
				"SELECT count(*) FROM hosted_model_assignments WHERE role='noema'":                                                                                                                       0,
				"SELECT count(*) FROM hosted_model_assignments":                                                                                                                                          2,
				"SELECT count(*) FROM hosted_model_assignments WHERE role='action_reviewer' AND model_profile='kept' AND reasoning_effort='low' AND fast_mode=1":                                         1,
				"SELECT count(*) FROM hosted_model_assignments WHERE provider_kind='local_models' AND model_profile='local-kept'":                                                                        1,
				"SELECT count(*) FROM provider_accounts WHERE provider_kind='openai' AND json_extract(metadata_json,'$.credentialRevision')=7":                                                           1,
				"SELECT count(*) FROM conversations WHERE provider='foundation_local'":                                                                                                                   1,
				"SELECT count(*) FROM task_runs WHERE run_id='run:kept' AND provider_kind='foundation_local' AND provider_account_id='provider_account:foundation_local:default' AND status='completed'": 1,
				"SELECT count(*) FROM pragma_foreign_key_check":                                                                                                                                          0,
			}
			checks["SELECT count(*) FROM default_model_preference"] = 0
			if defaultKind == "openai" {
				checks["SELECT count(*) FROM default_model_preference"] = 1
				checks["SELECT count(*) FROM default_model_preference WHERE provider_kind='openai' AND model_profile='chosen'"] = 1
			}
			for query, want := range checks {
				var got int
				if err := upgraded.db.QueryRow(query).Scan(&got); err != nil || got != want {
					t.Fatalf("%s = %d, want %d: %v", query, got, want, err)
				}
			}
			fresh := openTestStore(t)
			if err := fresh.EnsureBuiltinProviderAccounts(t.Context(), time.Now()); err != nil {
				t.Fatal(err)
			}
			for _, table := range []string{"hosted_model_assignments", "default_model_preference", "provider_accounts"} {
				var oldSQL, newSQL string
				if err := upgraded.db.QueryRow("SELECT sql FROM sqlite_schema WHERE name=?", table).Scan(&oldSQL); err != nil {
					t.Fatal(err)
				}
				if err := fresh.db.QueryRow("SELECT sql FROM sqlite_schema WHERE name=?", table).Scan(&newSQL); err != nil || oldSQL != newSQL {
					t.Fatalf("schema differs for %s: %v", table, err)
				}
			}
			var removed int
			if err := fresh.db.QueryRow("SELECT count(*) FROM provider_accounts WHERE provider_kind='foundation_local'").Scan(&removed); err != nil || removed != 0 {
				t.Fatalf("fresh removed account count = %d: %v", removed, err)
			}
			if _, err := upgraded.db.Exec(`INSERT INTO hosted_model_assignments VALUES
('noema','foundation_local','provider_account:openai:default','explicit_profile','default',NULL,0)`); err == nil {
				t.Fatal("removed model provider was accepted")
			}
			assignments := make([]ModelAssignment, 0, len(hostedModelRoles))
			for _, role := range hostedModelRoles {
				assignments = append(assignments, ModelAssignment{Role: role, ProviderKind: "openai", ProviderAccountID: "provider_account:openai:default", SelectionMode: ModelSelectionNoemaRecommended})
			}
			if filled, err := upgraded.ConfirmHostedModelAssignments(t.Context(), "provider_account:openai:default", assignments); err != nil || !filled {
				t.Fatalf("replacement setup = %t: %v", filled, err)
			}
			for _, query := range []string{
				"SELECT count(*) FROM hosted_model_assignments WHERE role='noema' AND provider_kind='openai'",
				"SELECT count(*) FROM hosted_model_assignments WHERE role='action_reviewer' AND model_profile='kept' AND reasoning_effort='low' AND fast_mode=1",
				"SELECT count(*) FROM hosted_model_assignments WHERE role='simple_tasks' AND model_profile='local-kept'",
			} {
				var got int
				if err := upgraded.db.QueryRow(query).Scan(&got); err != nil || got != 1 {
					t.Fatalf("replacement setup changed selections: %s = %d, %v", query, got, err)
				}
			}
			if saved, err := upgraded.HostedModelAssignments(t.Context()); err != nil || len(saved) != len(hostedModelRoles) {
				t.Fatalf("repaired assignments = %#v, %v", saved, err)
			}
			if _, err := upgraded.db.Exec("DELETE FROM hosted_model_assignments WHERE role='action_reviewer'"); err != nil {
				t.Fatal(err)
			}
			local := make([]ModelAssignment, 0, len(hostedModelRoles)-1)
			for _, role := range hostedModelRoles {
				if role != HostedModelActionReviewer {
					local = append(local, ModelAssignment{Role: role, ProviderKind: "local_models", ProviderAccountID: "provider_account:local_models:default", SelectionMode: ModelSelectionExplicitProfile, ModelProfile: "local-kept"})
				}
			}
			if _, err := upgraded.ConfirmHostedModelAssignments(t.Context(), "provider_account:local_models:default", local); err == nil || !strings.Contains(err.Error(), "select a hosted provider") {
				t.Fatalf("missing hosted reviewer must explain setup recovery: %v", err)
			}
		})
	}
}
