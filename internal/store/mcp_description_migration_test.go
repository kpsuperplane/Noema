package store

import (
	"database/sql"
	"path/filepath"
	"strings"
	"testing"
)

func TestMCPDescriptionUpgradePreservesToolsAndConverges(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v38.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	_, err = legacy.Exec(schemaAtVersion(38) + `
PRAGMA user_version=38;
INSERT INTO mcp_definitions VALUES('mcp_definition:'||printf('%032d',1),'mcp_definition_revision:'||printf('%032d',2),'Docs','streamable_http','{}',1);
INSERT INTO mcp_servers(mcp_server_id,mcp_definition_id,connection_revision,health_status,auth_status,created_at_ms,updated_at_ms)
 VALUES('mcp_server:'||printf('%032d',3),'mcp_definition:'||printf('%032d',1),'mcp_connection_revision:'||printf('%032d',4),'healthy','authenticated',1,1);
INSERT INTO mcp_tools(mcp_tool_id,mcp_server_id,name,description,input_schema_json,annotations_json,source_revision,read_only,read_only_source,status,policy_revision,created_at_ms,updated_at_ms)
 VALUES('mcp_tool:'||printf('%032d',5),'mcp_server:'||printf('%032d',3),'read','Original description','{}','{}',printf('%064d',6),1,'human','disabled',7,1,2);
`)
	if err != nil {
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	upgraded, err := Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer upgraded.Close()
	tools, err := upgraded.MCPTools(t.Context(), "mcp_server:"+strings.Repeat("0", 31)+"3")
	if err != nil || len(tools) != 1 {
		t.Fatalf("preserved tools = %v: %v", tools, err)
	}
	tool := tools[0]
	if tool.Description != "Original description" || tool.Status != "disabled" || tool.PolicyRevision != 7 || tool.ReadOnly.Value == nil || !*tool.ReadOnly.Value || tool.ReadOnly.Source != "human" || tool.SourceRevision != strings.Repeat("0", 63)+"6" {
		t.Fatalf("tool or permission changed: %#v", tool)
	}
	fresh := openTestStore(t)
	for _, name := range []string{"mcp_tools", "mcp_tools_server"} {
		var a, b string
		if err := upgraded.db.QueryRow("SELECT sql FROM sqlite_schema WHERE name=?", name).Scan(&a); err != nil {
			t.Fatal(err)
		}
		if err := fresh.db.QueryRow("SELECT sql FROM sqlite_schema WHERE name=?", name).Scan(&b); err != nil {
			t.Fatal(err)
		}
		if a != b {
			t.Fatalf("fresh and upgraded schema differ for %s", name)
		}
	}
	description := strings.Repeat("é", 32768)
	if _, err := upgraded.db.Exec("UPDATE mcp_tools SET description=?", description); err != nil {
		t.Fatalf("long description rejected: %v", err)
	}
	if _, err := upgraded.db.Exec("UPDATE mcp_tools SET description=?", description+"x"); err == nil {
		t.Fatal("description byte limit not enforced")
	}
	var violations int
	if err := upgraded.db.QueryRow("SELECT count(*) FROM pragma_foreign_key_check").Scan(&violations); err != nil || violations != 0 {
		t.Fatalf("foreign key violations=%d: %v", violations, err)
	}
}
