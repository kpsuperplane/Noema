package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestMCPSchemaConvergesAndRecoversSafeOAuthState(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v18.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec(schemaAtVersion(18) + `PRAGMA user_version=18;`); err != nil {
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	database, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	now := time.Now().UTC()
	if err := database.CreateMCPOAuthAttempt(context.Background(), MCPOAuthAttempt{
		ID: "mcp_oauth:" + strings.Repeat("a", 32), OwnerHumanID: "human:local",
		ExpiresAt: now.Add(time.Hour), CreatedAt: now, UpdatedAt: now,
	}); err != nil {
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
	value, err := database.MCPOAuthAttempt(context.Background(), "mcp_oauth:"+strings.Repeat("a", 32), "human:local", now)
	if err != nil || value.Status != "failed" || value.FailureCode != "server_restarted" {
		t.Fatalf("recovered OAuth attempt = %#v, %v", value, err)
	}
	var version int
	if err := database.db.QueryRow(`PRAGMA user_version`).Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("schema = %d, %v", version, err)
	}
	var schema string
	if err := database.db.QueryRow(`SELECT group_concat(sql) FROM sqlite_schema WHERE name LIKE 'mcp_%'`).Scan(&schema); err != nil {
		t.Fatal(err)
	}
	for _, forbidden := range []string{"access_token", "refresh_token", "client_secret", "code_verifier"} {
		if strings.Contains(strings.ToLower(schema), forbidden) {
			t.Fatalf("MCP schema stores %s", forbidden)
		}
	}
	v19Path := filepath.Join(t.TempDir(), "v19.sqlite3")
	v19, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(v19Path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := v19.Exec(schemaAtVersion(19) + `PRAGMA user_version=19;`); err != nil {
		t.Fatal(err)
	}
	if err := v19.Close(); err != nil {
		t.Fatal(err)
	}
	upgraded, err := Open(context.Background(), v19Path)
	if err != nil {
		t.Fatal(err)
	}
	defer upgraded.Close()
	var authTable bool
	if err := upgraded.db.QueryRow(`SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='mcp_auth_requests')`).Scan(&authTable); err != nil || !authTable {
		t.Fatalf("v19 to v20 auth table = %t, %v", authTable, err)
	}
}

func TestMCPAuthorityPreservesPoliciesAndFencesDeletion(t *testing.T) {
	database := openTestStore(t)
	ctx, now := context.Background(), time.Now().UTC()
	serverID := "mcp_server:" + strings.Repeat("1", 32)
	definition := MCPDefinition{ID: "mcp_definition:" + strings.Repeat("2", 32),
		Revision: "mcp_definition_revision:" + strings.Repeat("3", 32), DisplayName: "Calendar",
		TransportKind: "streamable_http", SafeConfig: json.RawMessage(`{"url":"http://localhost:9999"}`)}
	tool := MCPTool{ID: "mcp_tool:" + strings.Repeat("4", 32), Name: "events", InputSchema: json.RawMessage(`{"type":"object"}`),
		Annotations: json.RawMessage(`{"readOnlyHint":true}`), SourceRevision: strings.Repeat("5", 64),
		ReadOnly: MCPHint{Value: boolTest(true), Source: "annotation"}, Idempotent: MCPHint{Value: boolTest(false), Source: "safe_default"},
		Destructive: MCPHint{Value: boolTest(true), Source: "safe_default"}, OpenWorld: MCPHint{Value: boolTest(true), Source: "safe_default"},
		Status: "defaulted", PolicyRevision: 1}
	server, err := database.CommitMCPConnection(ctx, NewMCPConnection{Definition: definition, ServerID: serverID,
		ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("6", 32), AuthStatus: "none", Tools: []MCPTool{tool}}, now)
	if err != nil {
		t.Fatal(err)
	}
	server, err = database.SaveMCPConnectionPolicy(ctx, server.ID, server.ConnectionRevision, 0,
		"allow_automatically", "always_ask", now)
	if err != nil {
		t.Fatal(err)
	}
	override := [4]bool{true, true, false, true}
	current, err := database.SaveMCPToolPolicy(ctx, server.ID, server.ConnectionRevision, tool.ID,
		tool.SourceRevision, 1, nil, &override, now)
	if err != nil {
		t.Fatal(err)
	}
	replacement := tool
	replacement.ID = "mcp_tool:" + strings.Repeat("7", 32)
	server, err = database.ReconcileMCPConnection(ctx, server.ID, server.ConnectionRevision, "", "none", []MCPTool{replacement}, now)
	if err != nil {
		t.Fatal(err)
	}
	tools, _ := database.MCPTools(ctx, server.ID)
	if len(tools) != 1 || tools[0].ID != tool.ID || tools[0].PolicyRevision != current.PolicyRevision || tools[0].ReadOnly.Source != "human" {
		t.Fatalf("reconciled tool = %#v", tools)
	}
	if err := database.FenceMCPServer(ctx, server.ID, server.ConnectionRevision, now); err != nil {
		t.Fatal(err)
	}
	fenced, err := database.MCPServer(ctx, server.ID)
	if err != nil || fenced.Enabled || fenced.HealthStatus != "unavailable" {
		t.Fatalf("fenced server = %#v, %v", fenced, err)
	}
	if deleted, err := database.DeleteMCPServer(ctx, server.ID); err != nil || !deleted {
		t.Fatalf("delete = %t, %v", deleted, err)
	}
	tools, err = database.MCPTools(ctx, server.ID)
	if err != nil || len(tools) != 0 {
		t.Fatalf("remaining tools = %#v, %v", tools, err)
	}
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Connect it.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter",
		Call: ConversationToolCallInput{ProviderCallID: "setup-1", ProviderName: "mcp.connect_service",
			Name: "mcp.connect_service", Arguments: json.RawMessage(`{"service_url":"https://example.test"}`)}}, now)
	if err != nil {
		t.Fatal(err)
	}
	call := items[len(items)-1]
	payload := json.RawMessage(`{"status":"ready_for_policy","service_url":"https://example.test/","display_name":"Example","endpoint_url":"https://mcp.example.test/","setup_result":{"setup_status":"ready_for_policy","discovered_tool_count":1}}`)
	if _, err := database.FinishConversationToolCall(ctx, turn, ConversationToolResultInput{CallItemID: call.ID,
		Provider: "openrouter", ProviderCallID: "setup-1", ProviderName: "mcp.connect_service",
		Name: "mcp.connect_service", Success: true, Payload: payload}, now); err != nil {
		t.Fatal(err)
	}
	pending, err := database.PendingMCPSetupItems(ctx, conversation.ID, 10)
	if err != nil || len(pending) != 1 {
		t.Fatalf("pending setups = %#v, %v", pending, err)
	}
	if changed, err := database.ResolveMCPSetupItem(ctx, conversation.ID, pending[0].ID, serverID); err != nil || !changed {
		t.Fatalf("resolve setup = %t, %v", changed, err)
	}
	pending, err = database.PendingMCPSetupItems(ctx, conversation.ID, 10)
	if err != nil || len(pending) != 0 {
		t.Fatalf("resolved setups = %#v, %v", pending, err)
	}
}

func boolTest(value bool) *bool { return &value }
