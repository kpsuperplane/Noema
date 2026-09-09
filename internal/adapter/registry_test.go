package adapter

import (
	"encoding/json"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/kpsuperplane/noema/internal/store"
)

func TestCompiledRegistryReadsOwnReturnedValues(t *testing.T) {
	service, binding, _ := rustCursorFixture(t)
	original, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	// Removing source access distinguishes registry reads from repeated compilation.
	if err := service.files.root.Rename("adapters/definitions", "adapters/saved-definitions"); err != nil {
		t.Fatal(err)
	}
	snapshot, err := service.Snapshot()
	if err != nil || !reflect.DeepEqual(snapshot, original) {
		t.Fatalf("cached snapshot changed: %v", err)
	}
	*snapshot.Definitions[0].Manifest.Operations[0].Behavior.ReadOnly.Value = false
	snapshot.Definitions[0].Operations[0].InputSchema[0] = '!'
	snapshot.Definitions[0].Operations[0].Response.OutputSchema.Properties["unexpected"] = OutputSchema{Type: "null"}
	next, err := service.Snapshot()
	if err != nil || !reflect.DeepEqual(next, original) {
		t.Fatalf("returned snapshot changed the registry: %v", err)
	}
	bindings, err := service.Bindings()
	if err != nil || len(bindings) != 1 {
		t.Fatalf("catalog read = %v, %v", bindings, err)
	}
	bindings[0].InputSchema[0] = '!'
	current, err := service.Binding(binding.Name)
	if err != nil || !json.Valid(current.InputSchema) {
		t.Fatalf("returned binding changed the registry: %v", err)
	}
	if _, err := service.AvailabilityNotices(); err != nil {
		t.Fatal(err)
	}
	if raw, ok := service.ExecuteSetup(DefinitionTemplateTool, []byte(`{}`)); !ok {
		t.Fatalf("template read = %s", raw)
	}
	if err := service.Validate(binding, []byte(`{}`)); err == nil {
		t.Fatal("execution validation ignored missing definition authority")
	}
	if err := service.files.root.Rename("adapters/saved-definitions", "adapters/definitions"); err != nil {
		t.Fatal(err)
	}
}

func TestRegistryPublicationFailureRejectsStaleBindingsAndRecovers(t *testing.T) {
	service, old, _ := rustCursorFixture(t)
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	connection := snapshot.Connections[0]
	disabled, err := service.SetActive(t.Context(), connection.ConnectionID, connection.ConnectionRevision, false)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.Binding(old.Name); err == nil {
		t.Fatal("disabled connection stayed callable")
	}
	enabled, err := service.SetActive(t.Context(), disabled.ConnectionID, disabled.ConnectionRevision, true)
	if err != nil {
		t.Fatal(err)
	}
	if err := service.Validate(old, []byte(`{}`)); err == nil {
		t.Fatal("old connection revision remained valid")
	}
	if err := service.database.Close(); err != nil {
		t.Fatal(err)
	}
	// File publication succeeds, but the following SQLite index transaction fails.
	if _, err := service.SetActive(t.Context(), enabled.ConnectionID, enabled.ConnectionRevision, false); err == nil {
		t.Fatal("publication succeeded with a closed database")
	}
	if _, err := service.Snapshot(); err == nil {
		t.Fatal("incomplete publication returned an old registry")
	}
	if _, err := service.Bindings(); err == nil {
		t.Fatal("incomplete publication authorized catalog reads")
	}
	if _, _, err := service.Call(t.Context(), old, []byte(`{}`)); err == nil {
		t.Fatal("incomplete publication authorized execution")
	}
	database, err := store.Open(t.Context(), filepath.Join(service.files.root.Name(), "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	recovered, err := NewService(service.files.root, database)
	if err != nil {
		t.Fatal(err)
	}
	after, err := recovered.Snapshot()
	if err != nil || len(after.Definitions) != 1 || after.Connections[0].Status != "suspended" {
		t.Fatalf("recovery lost the published state: %v", err)
	}
	if values, err := recovered.Bindings(); err != nil || len(values) != 0 {
		t.Fatalf("recovery restored stale authority: %v", err)
	}
}

func TestRegistrySnapshotPreservesExactBodyNumbers(t *testing.T) {
	service, _, _, _ := rustAdapterService(t)
	manifest := testManifest()
	manifest.Operations[0].Method = "POST"
	manifest.Operations[0].Retry = "never"
	manifest.Operations[0].JSONBodyTemplate = map[string]any{"record_id": json.Number("9007199254740993")}
	pending, err := service.files.installDefinition(manifest, "https://example.com/docs", nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	reviewed, err := service.Approve(t.Context(), pending.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatal(err)
	}
	for _, definition := range snapshot.Definitions {
		if definition.SemanticDigest != reviewed.SemanticDigest {
			continue
		}
		expected, _ := json.Marshal(reviewed.Manifest)
		actual, _ := json.Marshal(definition.Manifest)
		if string(actual) != string(expected) {
			t.Fatal("snapshot changed the reviewed request body")
		}
		return
	}
	t.Fatal("reviewed definition missing")
}
