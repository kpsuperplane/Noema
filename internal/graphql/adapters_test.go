package graphql

import (
	"crypto/sha256"
	"encoding/hex"
	"testing"

	"github.com/kpsuperplane/noema/internal/adapter"
)

func TestAdapterDefinitionProjectsTransformSourceAndStableTransition(t *testing.T) {
	truth, modelSource, closed, limit := true, "model", false, 64
	hint := func(value bool) adapter.Hint { copy := value; return adapter.Hint{Value: &copy, Source: &modelSource} }
	source := `return function(response) return { name = response.body } end`
	manifest := adapter.Manifest{SchemaVersion: 9, DefinitionID: "public-example", AdapterID: "example", DefinitionRevision: "v2",
		Origin: "https://api.example.com/", Authentication: adapter.Authentication{Kind: "none"}, Reviewed: true,
		Operations: []adapter.Operation{{OperationID: "lookup", Description: "Look up one record.", Method: "GET", Path: "/lookup",
			Authorization: adapter.Authorization{Kind: "none"}, Behavior: adapter.BehaviorHints{ReadOnly: hint(truth), Idempotent: hint(truth), Destructive: hint(false), OpenWorld: hint(truth)},
			Retry: "transport_safe_read", Pagination: adapter.Pagination{Kind: "none"}, Response: adapter.Response{AcceptedContentTypes: []string{"application/json"},
				Transform: &adapter.Transform{Language: "lua", Source: source}, OutputSchema: adapter.OutputSchema{Type: "object", Properties: map[string]adapter.OutputSchema{"name": {Type: "string", MaxBytes: &limit}}, Required: []string{"name"}, AdditionalProperties: &closed}}}},
	}
	definition, err := adapter.Compile(manifest)
	if err != nil {
		t.Fatal(err)
	}
	definition.AffectedConnections = []string{"connection-one", "connection-two"}
	projected := definitionModel(definition, adapter.ServiceSnapshot{Definitions: []adapter.Definition{definition}})
	digest := sha256.Sum256([]byte(source))
	if projected.Transition.AffectedConnections != 2 || projected.Operations[0].ResponseTransform == nil ||
		projected.Operations[0].ResponseTransform.SourceDigest != hex.EncodeToString(digest[:]) ||
		projected.Operations[0].ResponseTransform.SourceDigest == definition.Operations[0].Digest {
		t.Fatalf("projected adapter = %#v", projected)
	}
}
