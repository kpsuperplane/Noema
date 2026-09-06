package adapter

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

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

func TestCompileEnforcesClosedLuaContract(t *testing.T) {
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
	manifest.Operations[0].Response.Transform.Source = "return function("
	if _, err := Compile(manifest); err == nil {
		t.Fatal("invalid Lua transform compiled")
	}
	manifest = testManifest()
	manifest.Operations[0].Pagination = Pagination{Kind: "response_token", ResponsePointer: "/next", RequestArgument: "cursor", PageSize: &PageSize{RequestArgument: "limit", Value: 1001}}
	if _, err := Compile(manifest); err == nil {
		t.Fatal("oversized pagination page compiled")
	}
	manifest = testManifest()
	reservedLimit := 16
	manifest.Operations[0].Response.OutputSchema.Properties["continuation"] = OutputSchema{Type: "string", MaxBytes: &reservedLimit}
	if _, err := Compile(manifest); err == nil {
		t.Fatal("reserved continuation output compiled")
	}
	raw, _ := json.Marshal(testManifest())
	raw = []byte(strings.Replace(string(raw), `"schema_version":9`, `"schema_version":9,"schema_version":9`, 1))
	if _, err := CompileJSON(raw); err == nil {
		t.Fatal("duplicate manifest field compiled")
	}
	raw, _ = json.Marshal(testManifest())
	raw = []byte(strings.Replace(string(raw), `"authentication":{"kind":"none"}`, `"authentication":{"kind":"none","setup":null}`, 1))
	if _, err := CompileJSON(raw); err == nil {
		t.Fatal("open none authentication compiled")
	}
}

func TestCompileReportsResponseLimitAndOperation(t *testing.T) {
	manifest := testManifest()
	limit := modelResultLimit
	manifest.Operations[0].Response.OutputSchema.Properties["name"] = OutputSchema{Type: "string", MaxBytes: &limit}
	_, err := Compile(manifest)
	if err == nil || !strings.Contains(err.Error(), "operations[0]: response.output_schema") ||
		!strings.Contains(err.Error(), "limit is 32768 bytes") {
		t.Fatalf("response limit error = %v", err)
	}
}

func TestDefinitionHelpExamplesCompile(t *testing.T) {
	help := definitionHelp()
	var input proposalInput
	if err := decodeExactJSON(help["proposal_template"].(json.RawMessage), &input); err != nil {
		t.Fatal(err)
	}
	manifest, err := buildManifest(input, nil)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateOperation(&manifest.Operations[0]); err != nil {
		t.Fatal(err)
	}
	if _, err := Compile(manifest); err != nil {
		t.Fatal(err)
	}
	if err := decodeExactJSON(help["credential_authentication_example"].(json.RawMessage), &manifest.Authentication); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"flat_object_response_example", "scalar_list_response_example", "custom_response_example"} {
		t.Run(name, func(t *testing.T) {
			response, err := proposalResponse(help[name].(json.RawMessage))
			if err != nil {
				t.Fatal(err)
			}
			manifest.Operations[0].Response = response
			if _, err := Compile(manifest); err != nil {
				t.Fatal(err)
			}
		})
	}
}

func testCredentialAuthentication() Authentication {
	return Authentication{Kind: "credential", Setup: &CredentialSetup{CredentialType: "API key", SetupURL: "https://example.com/keys",
		Instructions: []string{"Create one key."}, Input: CredentialInput{Kind: "fields", Fields: []CredentialField{{ID: "api_key", Label: "API key"}}}},
		RequestAuth: &Transform{Language: "lua", Source: `return function(input) return {headers={Authorization="Bearer "..input.credentials.api_key},query={signature=input.credentials.api_key}} end`}}
}

func TestCredentialManifestAndRequestAuthenticationAreClosed(t *testing.T) {
	manifest := testManifest()
	manifest.Authentication = testCredentialAuthentication()
	definition, err := Compile(manifest)
	if err != nil {
		t.Fatal(err)
	}
	request, _, err := encodeRequest(definition, definition.Operations[0], json.RawMessage(`{"id":"ordinary-id"}`), "")
	if err != nil {
		t.Fatal(err)
	}
	sensitive, secretValues, err := applyCredentialAuth(manifest.Authentication, map[string]string{"api_key": "secret-marker"}, definition.Operations[0], &request)
	if err != nil || request.headers["Authorization"] != "Bearer secret-marker" || !strings.Contains(request.rawURL, "signature=secret-marker") || !sensitive["authorization"] || !sensitive["signature"] || len(secretValues) != 2 {
		t.Fatalf("request authentication = %#v, %#v, %#v, %v", request, sensitive, secretValues, err)
	}
	manifest.Authentication.RequestAuth.Source = `return function(input) return {headers={Host=input.credentials.api_key}} end`
	definition, err = Compile(manifest)
	if err != nil {
		t.Fatal(err)
	}
	request, _, _ = encodeRequest(definition, definition.Operations[0], json.RawMessage(`{"id":"ordinary-id"}`), "")
	if _, _, err = applyCredentialAuth(manifest.Authentication, map[string]string{"api_key": "secret-marker"}, definition.Operations[0], &request); err == nil {
		t.Fatal("dangerous credential header was accepted")
	}
	manifest.Authentication.RequestAuth.Source = `return function(input) return {body=input.credentials.api_key} end`
	if _, err = Compile(manifest); err != nil {
		t.Fatal(err)
	}
	request, _, _ = encodeRequest(definition, definition.Operations[0], json.RawMessage(`{"id":"ordinary-id"}`), "")
	if _, _, err = applyCredentialAuth(manifest.Authentication, map[string]string{"api_key": "secret-marker"}, definition.Operations[0], &request); err == nil {
		t.Fatal("open credential result was accepted")
	}
	manifest.Authentication = testCredentialAuthentication()
	manifest.Authentication.Setup.SetupURL = "https://example.com/keys?account=ordinary#new"
	if _, err = Compile(manifest); err == nil {
		t.Fatal("credential setup URL with query and fragment compiled")
	}
}

func TestCredentialSetupPublishesAndReplacesProtectedGeneration(t *testing.T) {
	directory := t.TempDir()
	root, err := os.OpenRoot(directory)
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	database, err := store.Open(t.Context(), filepath.Join(directory, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	service, err := NewService(root, database)
	if err != nil {
		t.Fatal(err)
	}
	manifest := testManifest()
	manifest.Reviewed, manifest.Authentication = true, testCredentialAuthentication()
	definition, err := service.files.installDefinition(manifest, "https://example.com/docs", nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	definition, connection, err := service.SetupCredentialConnection(t.Context(), definition.SemanticDigest, "", CredentialInputValue{FieldValues: map[string]string{"api_key": "first-secret"}})
	if err != nil {
		t.Fatal(err)
	}
	credentialPath := filepath.Join(directory, "adapters", "connections", connection.ConnectionID, "credentials", connection.Authentication.GenerationID+".json")
	info, err := os.Stat(credentialPath)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm() != 0o600 {
		t.Fatalf("credential mode = %v", info.Mode().Perm())
	}
	connectionRaw, _ := os.ReadFile(filepath.Join(directory, "adapters", "connections", connection.ConnectionID, "connection.json"))
	if strings.Contains(string(connectionRaw), "first-secret") {
		t.Fatal("connection descriptor contains credential material")
	}
	connection.Status = "authentication_required"
	connection.ConnectionRevision++
	if _, err = service.files.replaceConnection(connection); err != nil {
		t.Fatal(err)
	}
	_, replaced, err := service.SetupCredentialConnection(t.Context(), definition.SemanticDigest, connection.ConnectionID, CredentialInputValue{FieldValues: map[string]string{"api_key": "second-secret"}})
	if err != nil || replaced.Authentication.Revision != 2 {
		t.Fatalf("replacement = %#v, %v", replaced, err)
	}
	entries, err := os.ReadDir(filepath.Dir(credentialPath))
	if err != nil || len(entries) != 1 || entries[0].Name() != replaced.Authentication.GenerationID+".json" {
		t.Fatalf("credential generations = %#v, %v", entries, err)
	}
	fields, err := service.files.loadCredential(replaced)
	if err != nil || fields["api_key"] != "second-secret" {
		t.Fatalf("credential = %#v, %v", fields, err)
	}
}

func TestDocumentCredentialAndDynamicSanitizationPreserveOrdinaryFields(t *testing.T) {
	setup := CredentialSetup{Input: CredentialInput{Kind: "document", MediaType: "application/json", Fields: []CredentialField{{ID: "token", Label: "Token"}},
		Normalize: &Transform{Language: "lua", Source: `return function(input) local value=json.decode(input.document); return {token=value.token} end`}}}
	fields, err := normalizeCredential(setup, CredentialInputValue{Document: []byte(`{"token":"secret-marker"}`)})
	if err != nil || fields["token"] != "secret-marker" {
		t.Fatalf("document = %#v, %v", fields, err)
	}
	value := sanitizeSensitiveOutput(map[string]any{"ordinary_id": "ordinary-value", "token": "secret-marker", "debug": "echo Bearer secret-marker", "url": "https://example.com/a?keep=ordinary&token=secret-marker"}, map[string]bool{"token": true}, []string{"Bearer secret-marker", "secret-marker"}).(map[string]any)
	if value["ordinary_id"] != "ordinary-value" || value["token"] != "[REDACTED]" || value["debug"] != "echo [REDACTED]" || value["url"] != "https://example.com/a?keep=ordinary" {
		t.Fatalf("sanitized result = %#v", value)
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
	if _, err = service.files.installDefinition(interrupted, pending.SourceReference, pending.Replaces, pending.AffectedConnections); err != nil {
		t.Fatal(err)
	}
	reviewed, err := service.Approve(context.Background(), proposed.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	originalReviewed := reviewed
	snapshot, _ := service.Snapshot()
	if len(snapshot.Connections) != 1 {
		t.Fatalf("connections = %d", len(snapshot.Connections))
	}
	connection := snapshot.Connections[0]
	if connection, err = service.SaveConnectionPolicy(context.Background(), connection.ConnectionID, "1", 1, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	overridden := store.ActionBehavior{Destructive: true, OpenWorld: true}
	if _, err = service.ChangeTool(context.Background(), connection.ConnectionID, "2", reviewed.Operations[0].OperationID, reviewed.Operations[0].Digest, 1, nil, &overridden, false); err != nil {
		t.Fatal(err)
	}
	snapshot, err = service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	connection = snapshot.Connections[0]
	bindings, err := service.Bindings()
	if err != nil || len(bindings) != 1 || bindings[0].SemanticDigest != reviewed.SemanticDigest {
		t.Fatalf("bindings = %#v, %v", bindings, err)
	}
	if err = root.Mkdir("adapters/connections/.staging-dead", 0o700); err != nil {
		t.Fatal(err)
	}
	crashFile, err := root.OpenFile("adapters/connections/"+connection.ConnectionID+"/.connection-dead.json", os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = crashFile.WriteString("incomplete"); err != nil {
		t.Fatal(err)
	}
	if err = crashFile.Close(); err != nil {
		t.Fatal(err)
	}
	restarted, err := NewService(root, database)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = root.Lstat("adapters/connections/.staging-dead"); !os.IsNotExist(err) {
		t.Fatalf("staging recovery = %v", err)
	}
	if _, err = root.Lstat("adapters/connections/" + connection.ConnectionID + "/.connection-dead.json"); !os.IsNotExist(err) {
		t.Fatalf("replacement recovery = %v", err)
	}
	bindings, err = restarted.Bindings()
	if err != nil || len(bindings) != 1 {
		t.Fatalf("recovered bindings = %#v, %v", bindings, err)
	}
	definitions, connections, err := database.AdapterIndexCounts(context.Background())
	if err != nil || definitions != 2 || connections != 1 {
		t.Fatalf("index = %d, %d, %v", definitions, connections, err)
	}
	cursor := Cursor{Reference: randomHex(), ConnectionID: connection.ConnectionID, ConnectionRevision: connection.ConnectionRevision,
		SemanticDigest: reviewed.SemanticDigest, OperationID: reviewed.Operations[0].OperationID, OperationDigest: reviewed.Operations[0].Digest,
		ArgumentsDigest: strings.Repeat("a", 64), Token: "ordinary-token", ExpiresAt: time.Now().Add(time.Hour)}
	if err = restarted.putCursor(cursor); err != nil {
		t.Fatal(err)
	}
	loadedCursor, err := restarted.loadCursor(cursor.Reference)
	if err != nil || loadedCursor.ConnectionRevision != connection.ConnectionRevision || loadedCursor.Token != "ordinary-token" {
		t.Fatalf("cursor = %#v, %v", loadedCursor, err)
	}
	if err = restarted.retireCursor(cursor.Reference); err != nil {
		t.Fatal(err)
	}
	revisionArguments := append([]Argument(nil), operation.Arguments...)
	revisionArguments[0].Description = "Current record identifier."
	revision := map[string]any{
		"source_reference":     "https://docs.example.com/api/v2",
		"base_semantic_digest": reviewed.SemanticDigest,
		"revision":             map[string]any{"definition_revision": "2026-09-06"},
		"upsert_operations": []any{map[string]any{"operation_id": "lookup", "description": "Look up one current record.", "method": "GET", "path": "/v1/items/{id}", "authorization": map[string]any{"kind": "none"}, "arguments": revisionArguments,
			"read_only": true, "idempotent": true, "destructive": false, "open_world": true, "response": response}},
	}
	raw, _ = json.Marshal(revision)
	result, ok = restarted.ExecuteSetup(ProposeDefinitionTool, raw)
	if !ok || json.Unmarshal(result, &proposed) != nil {
		t.Fatalf("revision proposal = %s", result)
	}
	pending, err = restarted.files.loadDefinition(proposed.SemanticDigest)
	if err != nil || len(pending.AffectedConnections) != 1 || pending.AffectedConnections[0] != connection.ConnectionID {
		t.Fatalf("revision transition = %#v, %v", pending.AffectedConnections, err)
	}
	reviewed, err = restarted.Approve(context.Background(), proposed.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	repeated, err := restarted.Approve(context.Background(), proposed.SemanticDigest)
	if err != nil || repeated.SemanticDigest != reviewed.SemanticDigest {
		t.Fatalf("replayed approval = %q, %v", repeated.SemanticDigest, err)
	}
	repeated, err = restarted.Approve(context.Background(), reviewed.SemanticDigest)
	if err != nil || repeated.SemanticDigest != reviewed.SemanticDigest {
		t.Fatalf("reviewed approval = %q, %v", repeated.SemanticDigest, err)
	}
	snapshot, err = restarted.Snapshot()
	if err != nil || len(snapshot.Connections) != 1 || snapshot.Connections[0].ConnectionID != connection.ConnectionID ||
		snapshot.Connections[0].SemanticDigest != reviewed.SemanticDigest || snapshot.Connections[0].ConnectionRevision != connection.ConnectionRevision+1 ||
		snapshot.Connections[0].DataSharingPolicy != "allow_automatically" || snapshot.Connections[0].UnsafeActionPolicy != "always_ask" ||
		snapshot.Connections[0].Overrides["lookup"].Behavior == nil || *snapshot.Connections[0].Overrides["lookup"].Behavior != overridden {
		t.Fatalf("adopted connection = %#v, %v", snapshot.Connections, err)
	}
	baseRaw, _ := json.Marshal(map[string]any{"semantic_digest": originalReviewed.SemanticDigest})
	if result, ok = restarted.ExecuteSetup(DefinitionTemplateTool, baseRaw); ok {
		t.Fatalf("superseded template = %s", result)
	}
	staleRevision := map[string]any{
		"source_reference":     "https://docs.example.com/api/stale",
		"base_semantic_digest": originalReviewed.SemanticDigest,
		"revision":             map[string]any{"definition_revision": "stale"},
	}
	staleRaw, _ := json.Marshal(staleRevision)
	if result, ok = restarted.ExecuteSetup(ProposeDefinitionTool, staleRaw); ok {
		t.Fatalf("superseded proposal = %s", result)
	}

	oldConnection := snapshot.Connections[0]
	oldConnection.SemanticDigest = originalReviewed.SemanticDigest
	if _, err = restarted.files.replaceConnection(oldConnection); err != nil {
		t.Fatal(err)
	}
	thirdRevision := revision
	thirdRevision["source_reference"] = "https://docs.example.com/api/v3"
	thirdRevision["base_semantic_digest"] = reviewed.SemanticDigest
	thirdRevision["revision"] = map[string]any{"definition_revision": "2026-09-07"}
	raw, _ = json.Marshal(thirdRevision)
	result, ok = restarted.ExecuteSetup(ProposeDefinitionTool, raw)
	if !ok || json.Unmarshal(result, &proposed) != nil {
		t.Fatalf("third revision proposal = %s", result)
	}
	pending, err = restarted.files.loadDefinition(proposed.SemanticDigest)
	if err != nil || len(pending.AffectedConnections) != 1 || pending.AffectedConnections[0] != oldConnection.ConnectionID {
		t.Fatalf("revision lineage = %#v, %v", pending.AffectedConnections, err)
	}
	thirdReviewed, err := restarted.Approve(context.Background(), proposed.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err = restarted.Snapshot()
	if err != nil || len(snapshot.Connections) != 1 || snapshot.Connections[0].SemanticDigest != thirdReviewed.SemanticDigest {
		t.Fatalf("lineage adoption = %#v, %v", snapshot.Connections, err)
	}
}

func TestOutcomeUncertaintyUsesReviewedBehavior(t *testing.T) {
	if !errors.Is(classifyHTTPOutcome(errOutcomeUncertain, store.ActionBehavior{}), ErrOutcomeUncertain) {
		t.Fatal("write uncertainty was not preserved")
	}
	if errors.Is(classifyHTTPOutcome(errOutcomeUncertain, store.ActionBehavior{ReadOnly: true}), ErrOutcomeUncertain) {
		t.Fatal("read-only uncertainty required recovery")
	}
}
