package runtime

import (
	"encoding/json"
	"slices"
	"strings"

	"github.com/kpsuperplane/noema/internal/provider"
)

const loadToolsName = "tools.load"

// Selection comes from saved tool use, never from matching human message text.
func onDemandTools(catalog []provider.GenerationTool, history []provider.GenerationMessage, providerKind, model string) []provider.GenerationTool {
	loaded := make(map[string]bool)
	for _, message := range history {
		for _, call := range message.ToolCalls {
			loaded[call.Name] = true
		}
		if result := message.ToolResult; result != nil && result.Name == loadToolsName && result.Success {
			var value struct {
				Names []string `json:"names"`
			}
			if json.Unmarshal(result.Payload, &value) == nil {
				for _, name := range value.Names {
					loaded[name] = true
				}
			}
		}
	}
	native := provider.SupportsDeferredTools(providerKind, model)
	tools := make([]provider.GenerationTool, 0, len(catalog)+1)
	var deferred []provider.GenerationTool
	for _, tool := range catalog {
		if tool.ServiceConnectionID != "" && !loaded[tool.Name] {
			tool.Deferred = native
			deferred = append(deferred, tool)
			if !native {
				continue
			}
		}
		tools = append(tools, tool)
	}
	if len(deferred) != 0 && !native {
		tools = append(tools, loadToolsSpec(deferred))
	}
	return tools
}

func loadToolsSpec(tools []provider.GenerationTool) provider.GenerationTool {
	names, rows := make([]string, 0, len(tools)), make([]string, 0, len(tools))
	for _, tool := range tools {
		names = append(names, tool.Name)
		if tool.ServiceCatalogRow != "" {
			rows = append(rows, tool.ServiceCatalogRow)
		}
		rows = append(rows, "- "+tool.Name+"\tservice="+tool.ServiceConnectionID)
	}
	slices.Sort(names)
	slices.Sort(rows)
	rows = slices.Compact(rows)
	schema, _ := json.Marshal(map[string]any{"type": "object", "properties": map[string]any{"names": map[string]any{"type": "array", "minItems": 1, "maxItems": 4, "uniqueItems": true, "items": map[string]any{"type": "string", "enum": names}}}, "required": []string{"names"}, "additionalProperties": false})
	return provider.GenerationTool{Name: loadToolsName, Description: "Load definitions for up to four exact tool names from this connected-service directory. Their native definitions become available in the next response. Load only tools needed for the current request. Loading does not execute tools or grant permission.\n" + strings.Join(rows, "\n"), InputSchema: schema}
}

func loadToolDefinitions(catalog []provider.GenerationTool, raw json.RawMessage) (json.RawMessage, bool) {
	var fields map[string]json.RawMessage
	var names []string
	if json.Unmarshal(raw, &fields) != nil || len(fields) != 1 || json.Unmarshal(fields["names"], &names) != nil || len(names) == 0 || len(names) > 4 {
		return toolFailure("invalid_input", "Supply one to four exact tool names."), false
	}
	available := make(map[string]bool)
	for _, tool := range catalog {
		if tool.ServiceConnectionID != "" {
			available[tool.Name] = true
		}
	}
	selected := make(map[string]bool)
	for _, name := range names {
		if !available[name] || selected[name] {
			return toolFailure("unavailable", "A selected tool is no longer available or was selected twice."), false
		}
		selected[name] = true
	}
	result, _ := json.Marshal(map[string]any{"names": names, "status": "definitions_loaded"})
	return result, true
}
