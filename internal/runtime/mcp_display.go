package runtime

import (
	"encoding/json"
	"errors"
	"strings"
	"unicode/utf8"

	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
)

func mcpHasDisplayLabel(name string) bool {
	return strings.HasPrefix(name, "mcp.") && name != noemamcp.ConnectServiceToolName
}

func mcpDisplayTool(tool provider.GenerationTool) (provider.GenerationTool, error) {
	if !mcpHasDisplayLabel(tool.Name) {
		return tool, nil
	}
	var arguments map[string]any
	if json.Unmarshal(tool.InputSchema, &arguments) != nil || arguments == nil {
		return tool, errors.New("invalid MCP display input schema")
	}
	moveMCPArgumentReferences(arguments)
	tool.InputSchema, _ = json.Marshal(map[string]any{
		"type": "object", "additionalProperties": false,
		"properties": map[string]any{
			"display_label": map[string]any{"type": "string", "minLength": 1, "maxLength": 160,
				"description": "Short, single-line label for this action, including the service and a recognizable target from the current conversation. Example: Notion · Update Quarterly plan. Use the user's language. If no recognizable target is known, use only the service and action. Describe the intended action, not success. Do not include opaque IDs or credentials. This label is for Noema's interface and is not sent to the tool."},
			"arguments": arguments,
		},
		"required": []string{"display_label", "arguments"},
	})
	return tool, nil
}

// Local pointers keep their original target when the schema moves into arguments.
// A schema with its own resource ID already retains its local reference scope.
func moveMCPArgumentReferences(value any) {
	switch schema := value.(type) {
	case []any:
		for _, child := range schema {
			moveMCPArgumentReferences(child)
		}
	case map[string]any:
		if _, exists := schema["$id"]; exists {
			return
		}
		for _, keyword := range []string{"$ref", "$dynamicRef", "$recursiveRef"} {
			if ref, ok := schema[keyword].(string); ok && (ref == "#" || strings.HasPrefix(ref, "#/")) {
				schema[keyword] = "#/properties/arguments" + strings.TrimPrefix(ref, "#")
			}
		}
		for _, keyword := range []string{"properties", "patternProperties", "$defs", "definitions", "dependentSchemas", "dependencies"} {
			children, _ := schema[keyword].(map[string]any)
			for _, child := range children {
				moveMCPArgumentReferences(child)
			}
		}
		for _, keyword := range []string{"additionalProperties", "additionalItems", "contains", "items", "not", "if", "then", "else", "propertyNames", "unevaluatedItems", "unevaluatedProperties", "allOf", "anyOf", "oneOf", "prefixItems"} {
			moveMCPArgumentReferences(schema[keyword])
		}
	}
}

func splitMCPDisplayCall(payload json.RawMessage) (string, json.RawMessage, error) {
	var envelope map[string]json.RawMessage
	if json.Unmarshal(payload, &envelope) != nil || len(envelope) != 2 {
		return "", nil, errors.New("MCP call requires display_label and arguments")
	}
	var label string
	var arguments map[string]json.RawMessage
	if json.Unmarshal(envelope["display_label"], &label) != nil ||
		json.Unmarshal(envelope["arguments"], &arguments) != nil || arguments == nil ||
		strings.TrimSpace(label) == "" || utf8.RuneCountInString(label) > 160 || strings.ContainsAny(label, "\r\n") {
		return "", nil, errors.New("invalid MCP display label or arguments")
	}
	return label, envelope["arguments"], nil
}

func mcpReplayArguments(arguments json.RawMessage, label string) json.RawMessage {
	if label == "" {
		return arguments
	}
	return mustJSON(map[string]any{"display_label": label, "arguments": arguments})
}
