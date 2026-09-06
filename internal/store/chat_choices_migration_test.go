package store

import (
	"database/sql"
	"path/filepath"
	"testing"
	"time"
)

func TestChoiceMigrationReleasesPauseAndPreservesSelection(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v33.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	defer legacy.Close()
	if _, err := legacy.Exec(schemaAtVersion(33) + `
PRAGMA user_version=33;
INSERT INTO conversations(conversation_id,owner_human_id,provider,created_at_ms,updated_at_ms)
VALUES ('conversation:00000000000000000000000000000001','human:local','openrouter',1,1);
INSERT INTO conversation_turns(turn_id,conversation_id,status,metadata_json,started_at_ms,created_at_ms,updated_at_ms)
VALUES ('turn:00000000000000000000000000000001','conversation:00000000000000000000000000000001','waiting_for_tool','{"turn_index":1}',1,1,1);
INSERT INTO conversation_items(item_id,conversation_id,turn_id,sequence_index,kind,status,author_actor_id,payload_json,metadata_json,created_at_ms,updated_at_ms)
VALUES ('item:00000000000000000000000000000001','conversation:00000000000000000000000000000001','turn:00000000000000000000000000000001',1,'tool_call','running','agent:primary',
'{"id":"tool_call:turn:00000000000000000000000000000001:0:0","metadata":{"action":{"provider_call_id":"call","provider_name":"present_multiple_choice","name":"noema.present_multiple_choice","payload":{"prompt":"Which?"}}}}',
'{"provider_round":0,"output_index":0,"source":"provider_action"}',1,1);
INSERT INTO conversation_items(item_id,conversation_id,turn_id,parent_item_id,sequence_index,kind,status,author_actor_id,payload_json,created_at_ms,updated_at_ms)
VALUES ('item:00000000000000000000000000000002','conversation:00000000000000000000000000000001','turn:00000000000000000000000000000001','item:00000000000000000000000000000001',2,'multiple_choice_prompt','completed','agent:primary',
'{"prompt":"Which?","selection_mode":"pick_one","options":[{"id":"a","label":"Alpha 日本語"}],"lifecycle":"pending","provider_selection":{},"response_id":"old-response"}',1,1);
`); err != nil {
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
	fresh := openTestStore(t)
	for _, database := range []*Store{upgraded, fresh} {
		var version int
		if err := database.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
			t.Fatalf("version = %d: %v", version, err)
		}
		var issues int
		if err := database.db.QueryRow("SELECT count(*) FROM pragma_foreign_key_check").Scan(&issues); err != nil || issues != 0 {
			t.Fatalf("foreign keys = %d: %v", issues, err)
		}
	}
	checks := map[string]int{
		"SELECT count(*) FROM conversation_turns WHERE status='completed'":                      1,
		"SELECT count(*) FROM conversation_items WHERE kind='tool_call' AND status='completed'": 1,
		"SELECT count(*) FROM conversation_items WHERE kind='tool_result' AND json_extract(payload_json,'$.metadata.action.payload.status')='displayed' AND json_extract(payload_json,'$.id')='tool_result:turn:00000000000000000000000000000001:0:0'": 1,
		"SELECT count(*) FROM conversation_items WHERE json_extract(payload_json,'$.provider_selection') IS NOT NULL OR json_extract(payload_json,'$.response_id') IS NOT NULL":                                                                        0,
	}
	for query, want := range checks {
		var got int
		if err := upgraded.db.QueryRow(query).Scan(&got); err != nil || got != want {
			t.Fatalf("%s = %d, %v", query, got, err)
		}
	}
	choice := ConversationChoiceSelection{PromptItemID: "item:00000000000000000000000000000002", OptionIDs: []string{"a"}}
	_, selection, err := upgraded.BeginConversationChoiceTurn(t.Context(), "conversation:00000000000000000000000000000001", choice, nil, time.Now())
	if err != nil || selection.ContentText != "Alpha 日本語" {
		t.Fatalf("old question selection = %#v, %v", selection, err)
	}
}
