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

func TestAdapterAuthenticationSchemaConvergesFromVersionTwentyFour(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v24.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec(schemaAtVersion(24) + `PRAGMA user_version=24;`); err != nil {
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	database, err := Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	var version int
	if err = database.db.QueryRow(`PRAGMA user_version`).Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("schema = %d, %v", version, err)
	}
	rows, err := database.db.Query(`PRAGMA table_info(mcp_auth_requests)`)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	found := map[string]bool{}
	for rows.Next() {
		var index, notNull, primary int
		var name, kind string
		var defaultValue any
		if err = rows.Scan(&index, &name, &kind, &notNull, &defaultValue, &primary); err != nil {
			t.Fatal(err)
		}
		found[name] = true
	}
	if !found["authority_kind"] || !found["authority_id"] {
		t.Fatalf("schema columns = %#v", found)
	}
	now := time.Now().UTC()
	conversation, err := database.EnsurePrimaryConversation(t.Context(), "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(t.Context(), conversation.ID, "Use the API.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(t.Context(), turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{ProviderCallID: "call-1", ProviderName: "example.lookup", Name: "example.lookup", Arguments: json.RawMessage(`{"ordinary":"value"}`)}}, now)
	if err != nil {
		t.Fatal(err)
	}
	call := items[len(items)-1]
	request, _, err := database.CreateMCPAuthRequest(t.Context(), MCPAuthRequest{OwnerHumanID: "human:local", ConversationID: conversation.ID, TurnID: turn.ID,
		CallItemID: call.ID, AuthorityKind: "adapter_connection", AuthorityID: strings.Repeat("a", 32), CapabilityName: "example.lookup",
		BindingJSON: `{"connection":"ordinary"}`, ArgumentsJSON: `{"ordinary":"value"}`}, now)
	if err != nil {
		t.Fatal(err)
	}
	adapterPending, err := database.PendingAdapterAuthRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(adapterPending) != 1 || adapterPending[0].AuthorityID != strings.Repeat("a", 32) {
		t.Fatalf("adapter requests = %#v, %v", adapterPending, err)
	}
	mcpPending, err := database.PendingMCPAuthRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(mcpPending) != 0 {
		t.Fatalf("MCP requests = %#v, %v", mcpPending, err)
	}
	connections, err := database.AwaitingAdapterAuthConnections(t.Context(), false)
	if err != nil || len(connections) != 1 || connections[0] != request.AuthorityID {
		t.Fatalf("awaiting adapter connections = %#v, %v", connections, err)
	}
	request, err = database.BeginAdapterAuthResume(t.Context(), request, now)
	if err != nil {
		t.Fatal(err)
	}
	request, err = database.RetryAdapterAuthentication(t.Context(), request, now)
	if err != nil || request.State != "awaiting_user" || request.Failure != "authentication_failed" {
		t.Fatalf("retry = %#v, %v", request, err)
	}
	if _, err = database.BeginAdapterAuthResume(t.Context(), request, now); err != nil {
		t.Fatal(err)
	}
	if err = database.Close(); err != nil {
		t.Fatal(err)
	}
	database, err = Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	recovered, err := database.RecoverConversationMCPAuthRequests(t.Context())
	if err != nil || len(recovered) != 1 || recovered[0].Failure != "outcome_uncertain" {
		t.Fatalf("recovered adapter request = %#v, %v", recovered, err)
	}
}

func TestAdapterOAuthAuthenticationSchemaConvergesFromVersionTwentyFive(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v25.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec(schemaAtVersion(25) + `PRAGMA user_version=25;`); err != nil {
		t.Fatal(err)
	}
	_ = legacy.Close()
	database, err := Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	var version int
	if err = database.db.QueryRow(`PRAGMA user_version`).Scan(&version); err != nil || version != 26 {
		t.Fatalf("schema = %d, %v", version, err)
	}
	rows, err := database.db.Query(`PRAGMA table_info(mcp_auth_requests)`)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	found := map[string]bool{}
	for rows.Next() {
		var index, notNull, primary int
		var name, kind string
		var defaultValue any
		if err = rows.Scan(&index, &name, &kind, &notNull, &defaultValue, &primary); err != nil {
			t.Fatal(err)
		}
		found[name] = true
	}
	for _, name := range []string{"adapter_connection_id", "adapter_semantic_digest", "adapter_authority_revision", "adapter_attempt_id"} {
		if !found[name] {
			t.Fatalf("missing column %s", name)
		}
	}
	now := time.Now().UTC()
	conversation, err := database.EnsurePrimaryConversation(t.Context(), "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(t.Context(), conversation.ID, "Use Google.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(t.Context(), turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{ProviderCallID: "call-oauth", ProviderName: "google.lookup", Name: "google.lookup", Arguments: json.RawMessage(`{"id":"ordinary"}`)}}, now)
	if err != nil {
		t.Fatal(err)
	}
	request, _, err := database.CreateMCPAuthRequest(t.Context(), MCPAuthRequest{OwnerHumanID: "human:local", ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: items[len(items)-1].ID, AuthorityKind: "adapter_grant", AuthorityID: strings.Repeat("a", 32), AdapterConnectionID: strings.Repeat("b", 32), AdapterSemanticDigest: strings.Repeat("c", 64), AdapterAuthorityRevision: 3, CapabilityName: "google.lookup", BindingJSON: `{"binding":"ordinary"}`, ArgumentsJSON: `{"id":"ordinary"}`}, now)
	if err != nil {
		t.Fatal(err)
	}
	request, err = database.BeginAdapterOAuthAuthentication(t.Context(), request, "attempt-ordinary", now)
	if err != nil || request.State != "authorizing" {
		t.Fatalf("begin OAuth = %#v, %v", request, err)
	}
	attached, err := database.AdapterAuthRequestsForAttempt(t.Context(), "attempt-ordinary")
	if err != nil || len(attached) != 1 || attached[0].ID != request.ID {
		t.Fatalf("attempt requests = %#v, %v", attached, err)
	}
	if err = database.RetryAdapterOAuthAuthentication(t.Context(), request, "oauth_attempt_denied", now); err != nil {
		t.Fatal(err)
	}
	request, err = database.MCPAuthRequest(t.Context(), request.ID, request.Revision)
	if err != nil || request.State != "awaiting_user" || request.Failure != "oauth_attempt_denied" || request.AdapterAttemptID != "" {
		t.Fatalf("OAuth retry = %#v, %v", request, err)
	}
	request, err = database.BeginAdapterOAuthAuthentication(t.Context(), request, "attempt-restart", now)
	if err != nil {
		t.Fatal(err)
	}
	if err = database.Close(); err != nil {
		t.Fatal(err)
	}
	database, err = Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	pending, err := database.PendingAdapterAuthRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 || pending[0].State != "awaiting_user" || pending[0].Failure != "server_restarted" || pending[0].AdapterAttemptID != "" {
		t.Fatalf("restart recovery = %#v, %v", pending, err)
	}
	connections, err := database.AwaitingAdapterAuthConnections(t.Context(), false)
	if err != nil || len(connections) != 1 || connections[0] != strings.Repeat("b", 32) {
		t.Fatalf("OAuth connections = %#v, %v", connections, err)
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
	classified, err := database.ClassifyMCPTool(ctx, server.ID, server.ConnectionRevision, tool.ID,
		tool.SourceRevision, 1, [4]bool{false, true, false, false}, now)
	if err != nil || classified.ReadOnly.Source != "annotation" || classified.ReadOnly.Value == nil || !*classified.ReadOnly.Value ||
		classified.Idempotent.Source != "model" || classified.Idempotent.Value == nil || !*classified.Idempotent.Value {
		t.Fatalf("classified tool = %#v, %v", classified, err)
	}
	if _, err = database.ClassifyMCPTool(ctx, server.ID, server.ConnectionRevision, tool.ID,
		tool.SourceRevision, 1, [4]bool{}, now); err == nil {
		t.Fatal("stale classification changed the tool")
	}
	server, err = database.SaveMCPConnectionPolicy(ctx, server.ID, server.ConnectionRevision, 0,
		"allow_automatically", "always_ask", now)
	if err != nil {
		t.Fatal(err)
	}
	override := [4]bool{true, true, false, true}
	current, err := database.SaveMCPToolPolicy(ctx, server.ID, server.ConnectionRevision, tool.ID,
		tool.SourceRevision, classified.PolicyRevision, nil, &override, now)
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
