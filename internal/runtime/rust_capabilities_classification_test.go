package runtime

import (
	"context"
	"encoding/json"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

// Rust source: crates/noema-capabilities/src/integration.rs::classification_fills_only_missing_hints_and_defaults_fail_closed.
func TestRustCapabilities_classification_fills_only_missing_hints_and_defaults_fail_closed(t *testing.T) {
	policyReadOnly, policyDestructive := true, false
	policy := store.MCPTool{
		ID:          "tool",
		ReadOnly:    store.MCPHint{Value: &policyReadOnly, Source: "annotation"},
		Destructive: store.MCPHint{Value: &policyDestructive, Source: "annotation"},
		Status:      "pending", PolicyRevision: 1, SourceRevision: "revision",
	}
	completion, completionErr := noemamcp.ParseToolClassificationResponse(
		`{"idempotent":true,"openWorld":false}`,
		policy,
	)
	if completionErr != nil {
		t.Fatalf("completion: %v", completionErr)
	}
	mergedPolicy := noemamcp.ApplyToolClassification(policy, completion)
	if mergedPolicy.ReadOnly.Source != "annotation" {
		t.Fatalf("read-only source = %q", mergedPolicy.ReadOnly.Source)
	}
	if mergedPolicy.Idempotent.Source != "model" {
		t.Fatalf("idempotent source = %q", mergedPolicy.Idempotent.Source)
	}
	if mergedPolicy.Status != "ready" || mergedPolicy.ReadOnly.Value == nil || mergedPolicy.Idempotent.Value == nil ||
		mergedPolicy.Destructive.Value == nil || mergedPolicy.OpenWorld.Value == nil {
		t.Fatalf("merged tool is not callable = %#v", mergedPolicy)
	}

	defaultPending := mergedPolicy
	defaultPending.Idempotent = store.MCPHint{}
	defaultPending.OpenWorld = store.MCPHint{}
	defaultPending.Status = "pending"
	defaultedPolicy := noemamcp.ApplyToolSafeDefaults(defaultPending)
	if defaultedPolicy.Status != "defaulted" {
		t.Fatalf("defaulted status = %q", defaultedPolicy.Status)
	}
	if defaultedPolicy.Idempotent.Source != "safe_default" || defaultedPolicy.Idempotent.Value == nil || *defaultedPolicy.Idempotent.Value {
		t.Fatalf("idempotent default = %#v", defaultedPolicy.Idempotent)
	}
	if defaultedPolicy.OpenWorld.Source != "safe_default" || defaultedPolicy.OpenWorld.Value == nil || !*defaultedPolicy.OpenWorld.Value {
		t.Fatalf("open-world default = %#v", defaultedPolicy.OpenWorld)
	}

	chat, database, _ := chatFixture(t)
	paths, err := home.FromRoot(chat.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	mcpService, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(mcpService.Close)

	serverID := "mcp_server:" + strings.Repeat("1", 32)
	definition := store.MCPDefinition{ID: "mcp_definition:" + strings.Repeat("2", 32), Revision: "mcp_definition_revision:" + strings.Repeat("3", 32),
		DisplayName: "Capabilities", TransportKind: "streamable_http", SafeConfig: json.RawMessage(`{"url":"https://example.test"}`)}
	readOnly, destructive := true, false
	tool := store.MCPTool{ID: "mcp_tool:" + strings.Repeat("4", 32), ServerID: serverID, Name: "read", Description: "Read documents.",
		InputSchema: json.RawMessage(`{"type":"object"}`), Annotations: json.RawMessage(`{"readOnlyHint":true,"destructiveHint":false}`), SourceRevision: strings.Repeat("a", 64),
		ReadOnly: store.MCPHint{Value: &readOnly, Source: "annotation"}, Destructive: store.MCPHint{Value: &destructive, Source: "annotation"},
		Idempotent: store.MCPHint{}, OpenWorld: store.MCPHint{},
		Status: "defaulted", PolicyRevision: 1}
	server, err := database.CommitMCPConnection(t.Context(), store.NewMCPConnection{Definition: definition, ServerID: serverID,
		ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("4", 32), AuthStatus: "none", Tools: []store.MCPTool{tool}}, time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := mcpService.SaveConnectionPolicy(t.Context(), server.ID, server.ConnectionRevision, 0, "allow_automatically", "never_ask"); err != nil {
		t.Fatal(err)
	}

	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) != 1 || request.Tools[0].Name != mcpClassificationTool {
			t.Fatalf("classification tool = %#v", request.Tools)
		}
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: mcpClassificationTool,
			Payload: json.RawMessage(`{"read_only":true,"idempotent":true,"destructive":false,"open_world":false}`)}}}, nil
	})
	tools, err := database.MCPTools(t.Context(), server.ID)
	if err != nil || len(tools) != 1 {
		t.Fatalf("pending classification tool = %#v, %v", tools, err)
	}
	behavior, err := chat.MCPToolClassifier()(t.Context(), tools[0])
	if err != nil {
		t.Fatal(err)
	}
	classified, err := mcpService.ClassifyToolResponse(t.Context(), server.ID, server.ConnectionRevision, tools[0].ID, tools[0].SourceRevision, tools[0].PolicyRevision,
		`{"idempotent":true,"openWorld":false}`)
	if err != nil {
		t.Fatal(err)
	}
	if behavior != [4]bool{true, true, false, false} {
		t.Fatalf("classifier behavior = %#v", behavior)
	}
	if classified.ReadOnly.Source != "annotation" || classified.ReadOnly.Value == nil || !*classified.ReadOnly.Value ||
		classified.Idempotent.Source != "model" || classified.Idempotent.Value == nil || !*classified.Idempotent.Value ||
		classified.Destructive.Source != "annotation" || classified.Destructive.Value == nil || *classified.Destructive.Value ||
		classified.OpenWorld.Source != "model" || classified.OpenWorld.Value == nil || *classified.OpenWorld.Value || classified.Status != "ready" {
		t.Fatalf("classified source hints = %#v", classified)
	}
	if classified.ReadOnly.Value == nil || classified.Idempotent.Value == nil || classified.Destructive.Value == nil || classified.OpenWorld.Value == nil {
		t.Fatalf("classified tool is not callable = %#v", classified)
	}
	pending := classified
	pending.Idempotent = store.MCPHint{}
	pending.OpenWorld = store.MCPHint{}
	defaulted := noemamcp.ApplyToolSafeDefaults(pending)
	if defaulted.Idempotent.Source != "safe_default" || defaulted.Idempotent.Value == nil || *defaulted.Idempotent.Value ||
		defaulted.OpenWorld.Source != "safe_default" || defaulted.OpenWorld.Value == nil || !*defaulted.OpenWorld.Value || defaulted.Status != "defaulted" {
		t.Fatalf("safe defaults = %#v", defaulted)
	}
}
