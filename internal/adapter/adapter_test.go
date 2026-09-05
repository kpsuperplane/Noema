package adapter

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/store"
)

func testManifest() Manifest {
	truth, model, closed, limit := true, "model", false, 64
	hint := func(value bool) Hint { copy := value; return Hint{Value: &copy, Source: &model} }
	return Manifest{SchemaVersion: 9, DefinitionID: "public-example", AdapterID: "example", DisplayName: "Example",
		DefinitionRevision: "2026-09-05", Origin: "https://api.example.com/", Authentication: Authentication{Kind: "none"},
		Operations: []Operation{{OperationID: "lookup", Description: "Look up one record.", Method: "GET", Path: "/v1/items/{id}",
			Authorization: Authorization{Kind: "none"}, Arguments: []Argument{{Name: "id", Description: "Record identifier.", Location: "path", Type: "string", Required: true}},
			Behavior: BehaviorHints{ReadOnly: hint(truth), Idempotent: hint(truth), Destructive: hint(false), OpenWorld: hint(truth)},
			Retry:    "transport_safe_read", Pagination: Pagination{Kind: "none"}, Response: Response{AcceptedContentTypes: []string{"application/json"},
				OutputSchema: OutputSchema{Type: "object", Properties: map[string]OutputSchema{"name": {Type: "string", MaxBytes: &limit}}, Required: []string{"name"}, AdditionalProperties: &closed}}}}}
}

func TestCompileEnforcesClosedCredentialFreeLuaContract(t *testing.T) {
	manifest := testManifest()
	if _, err := Compile(manifest); err != nil {
		t.Fatalf("Compile() = %v", err)
	}
	manifest.Authentication.Kind = "bearer"
	if _, err := Compile(manifest); err == nil {
		t.Fatal("credential-bearing manifest compiled")
	}
	manifest = testManifest()
	manifest.Operations[0].Response.Transform = &Transform{Language: "luau", Source: "return function(v) return v end"}
	if _, err := Compile(manifest); err == nil {
		t.Fatal("Luau transform compiled")
	}
	manifest.Operations[0].Response.Transform.Language = "lua"
	if _, err := Compile(manifest); err != nil {
		t.Fatalf("Lua transform = %v", err)
	}
	raw, _ := json.Marshal(testManifest())
	raw = []byte(strings.Replace(string(raw), `"schema_version":9`, `"schema_version":9,"schema_version":9`, 1))
	if _, err := CompileJSON(raw); err == nil {
		t.Fatal("duplicate manifest field compiled")
	}
}

func TestRequestEncodingKeepsReviewedOriginAndTypedValues(t *testing.T) {
	manifest := testManifest()
	manifest.Operations[0].Method, manifest.Operations[0].Retry = "POST", "never"
	manifest.Operations[0].Arguments = append(manifest.Operations[0].Arguments,
		Argument{Name: "tag", Description: "Tags.", Location: "query", Type: "string_array"},
		Argument{Name: "enabled", Description: "State.", Location: "json_body", Type: "boolean", Required: true})
	manifest.Operations[0].JSONBodyTemplate = map[string]any{"state": map[string]any{"$argument": "enabled"}}
	definition, err := Compile(manifest)
	if err != nil {
		t.Fatal(err)
	}
	request, arguments, err := encodeRequest(definition, definition.Operations[0], json.RawMessage(`{"id":"a/b","tag":["x","y"],"enabled":true}`), "")
	if err != nil {
		t.Fatal(err)
	}
	if request.rawURL != "https://api.example.com/v1/items/a%2Fb?tag=x&tag=y" {
		t.Fatalf("URL = %q", request.rawURL)
	}
	if string(request.body) != `{"state":true}` || arguments["id"] != "a/b" {
		t.Fatalf("request = %#v %s", arguments, request.body)
	}
	if _, _, err = encodeRequest(definition, definition.Operations[0], json.RawMessage(`{"id":"..","enabled":true}`), ""); err == nil {
		t.Fatal("path escape was accepted")
	}
}

func TestLuaResponseTransformAndPaginationAreBounded(t *testing.T) {
	limit, closed := 16, false
	contract := Response{AcceptedContentTypes: []string{"application/json"}, Transform: &Transform{Language: "lua", Source: `return function(response) local value = json.decode(response.body); return { name = value.value } end`},
		OutputSchema: OutputSchema{Type: "object", Properties: map[string]OutputSchema{"name": {Type: "string", MaxBytes: &limit}}, Required: []string{"name"}, AdditionalProperties: &closed}}
	value, err := decodeResponse(httpResponse{status: 200, contentType: "application/json", body: []byte(`{"value":"Ada"}`)}, contract)
	if err != nil || value.(map[string]any)["name"] != "Ada" {
		t.Fatalf("transform = %#v, %v", value, err)
	}
	if _, err = decodeResponse(httpResponse{status: 200, contentType: "application/json", body: []byte(`{"value":"this value exceeds the limit"}`)}, contract); err == nil {
		t.Fatal("oversized transform result accepted")
	}
	safe := sanitizeOutput(map[string]any{"opaque_id": "ordinary-value", "access_token": "secret", "url": "https://user:pass@example.com/path?x=keep&sig=secret"}).(map[string]any)
	if safe["opaque_id"] != "ordinary-value" || safe["access_token"] != "[REDACTED]" || safe["url"] != "https://example.com/path?x=keep" {
		t.Fatalf("sanitized output = %#v", safe)
	}
	page := map[string]any{"items": []any{"a"}, "next": "token"}
	if token, ok := removePointer(page, "/next"); !ok || token != "token" || page["next"] != nil {
		t.Fatalf("pagination = %#v, %q, %v", page, token, ok)
	}
	arrayPage := map[string]any{"metadata": []any{"keep", "token", "after"}}
	if token, ok := removePointer(arrayPage, "/metadata/1"); !ok || token != "token" || len(arrayPage["metadata"].([]any)) != 2 {
		t.Fatalf("array pagination = %#v, %q, %v", arrayPage, token, ok)
	}
}

func TestPrivateNetworkTargetIsRejectedBeforeHTTP(t *testing.T) {
	_, err := executeHTTPOnce(context.Background(), encodedRequest{method: "GET", rawURL: "https://127.0.0.1/private"})
	if err == nil {
		t.Fatal("private network target was accepted")
	}
}

func TestProposalReviewInstallPolicyAndRecovery(t *testing.T) {
	directory := t.TempDir()
	root, err := os.OpenRoot(directory)
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	database, err := store.Open(context.Background(), filepath.Join(directory, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	service, err := NewService(root, database)
	if err != nil {
		t.Fatal(err)
	}
	operation := testManifest().Operations[0]
	response := map[string]any{"kind": "custom", "accepted_content_types": operation.Response.AcceptedContentTypes, "output_schema": operation.Response.OutputSchema}
	proposal := map[string]any{"source_reference": "https://docs.example.com/api", "new_definition": map[string]any{"definition_id": "public-example", "adapter_id": "example", "display_name": "Example", "definition_revision": "2026-09-05", "origin": "https://api.example.com/", "authentication": map[string]any{"kind": "none"}},
		"upsert_operations": []any{map[string]any{"operation_id": "lookup", "description": "Look up one record.", "method": "GET", "path": "/v1/items/{id}", "authorization": map[string]any{"kind": "none"}, "arguments": operation.Arguments, "read_only": true, "idempotent": true, "destructive": false, "open_world": true, "response": response}}}
	raw, _ := json.Marshal(proposal)
	result, ok := service.ExecuteSetup(ProposeDefinitionTool, raw)
	if !ok {
		t.Fatalf("proposal = %s", result)
	}
	var proposed struct {
		SemanticDigest string `json:"semantic_digest"`
	}
	if json.Unmarshal(result, &proposed) != nil || proposed.SemanticDigest == "" {
		t.Fatalf("proposal result = %s", result)
	}
	pending, err := service.files.loadDefinition(proposed.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	interrupted := pending.Manifest
	interrupted.Reviewed = true
	if _, err = service.files.installDefinition(interrupted, pending.SourceReference, pending.Replaces); err != nil {
		t.Fatal(err)
	}
	reviewed, err := service.Approve(context.Background(), proposed.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, _ := service.Snapshot()
	if len(snapshot.Connections) != 1 {
		t.Fatalf("connections = %d", len(snapshot.Connections))
	}
	connection := snapshot.Connections[0]
	if _, err = service.SaveConnectionPolicy(context.Background(), connection.ConnectionID, "1", 1, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	bindings, err := service.Bindings()
	if err != nil || len(bindings) != 1 || bindings[0].SemanticDigest != reviewed.SemanticDigest {
		t.Fatalf("bindings = %#v, %v", bindings, err)
	}
	if err = root.Mkdir("adapters/connections/.staging-dead", 0o700); err != nil {
		t.Fatal(err)
	}
	restarted, err := NewService(root, database)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = root.Lstat("adapters/connections/.staging-dead"); !os.IsNotExist(err) {
		t.Fatalf("staging recovery = %v", err)
	}
	bindings, err = restarted.Bindings()
	if err != nil || len(bindings) != 1 {
		t.Fatalf("recovered bindings = %#v, %v", bindings, err)
	}
	definitions, connections, err := database.AdapterIndexCounts(context.Background())
	if err != nil || definitions != 2 || connections != 1 {
		t.Fatalf("index = %d, %d, %v", definitions, connections, err)
	}
}
