package runtime

import (
	"encoding/json"
	"strings"
	"testing"

	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
)

func TestMCPDisplaySchemaPreservesReferencesAndSourceFields(t *testing.T) {
	source := json.RawMessage(`{"type":"object","$defs":{"value":{"type":"string","enum":["café 日本語"]}},"properties":{"display_label":{"$ref":"#/$defs/value"},"arguments":{"type":"object","properties":{"$ref":{"const":"#/ordinary-data"}},"required":["$ref"],"additionalProperties":false}},"required":["display_label","arguments"],"additionalProperties":false}`)
	for _, resourceID := range []string{"", "https://example.test/tool-schema"} {
		var schema map[string]any
		_ = json.Unmarshal(source, &schema)
		if resourceID != "" {
			schema["$id"] = resourceID
		}
		encoded, _ := json.Marshal(schema)
		tool, err := mcpDisplayTool(provider.GenerationTool{Name: "mcp.docs.update", Description: "Update a document", InputSchema: encoded})
		if err != nil {
			t.Fatal(err)
		}
		envelope := json.RawMessage(`{"display_label":"Docs · Update quarterly plan","arguments":{"display_label":"café 日本語","arguments":{"$ref":"#/ordinary-data"}}}`)
		if err := noemamcp.ValidateArguments(tool.InputSchema, envelope); err != nil {
			t.Fatalf("wrapped schema (%s): %v; %s", resourceID, err, tool.InputSchema)
		}
		label, arguments, err := splitMCPDisplayCall(envelope)
		if err != nil || label != "Docs · Update quarterly plan" || noemamcp.ValidateArguments(encoded, arguments) != nil {
			t.Fatalf("source arguments changed: %s, %v", arguments, err)
		}
		var replay map[string]any
		_ = json.Unmarshal(mcpReplayArguments(arguments, label), &replay)
		if replay["display_label"] != label || replay["arguments"].(map[string]any)["display_label"] != "café 日本語" {
			t.Fatalf("replay lost label or original field: %#v", replay)
		}
		invalid := json.RawMessage(strings.Replace(string(envelope), "café 日本語", "invalid", 1))
		if noemamcp.ValidateArguments(tool.InputSchema, invalid) == nil {
			t.Fatal("wrapped schema lost source enum")
		}
	}
}

func TestMCPDisplayCallRejectsInvalidEnvelope(t *testing.T) {
	for _, payload := range []string{
		`{}`, `{"display_label":"Read","arguments":null}`, `{"display_label":" ","arguments":{}}`,
		`{"display_label":"Read\nsecret","arguments":{}}`, `{"display_label":"Read","arguments":{},"extra":true}`,
		`{"display_label":"` + strings.Repeat("x", 161) + `","arguments":{}}`,
	} {
		if _, _, err := splitMCPDisplayCall(json.RawMessage(payload)); err == nil {
			t.Fatalf("accepted invalid envelope: %s", payload)
		}
	}
}
