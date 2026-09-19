package runtime

import (
	"context"
	"encoding/json"
	"strings"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/script"
)

const luaRunName = "code.run_lua"

var luaRunSchema = json.RawMessage(`{"type":"object","properties":{"source":{"type":"string","minLength":1,"maxLength":50000},"input":{"type":"object","additionalProperties":true}},"required":["source"],"additionalProperties":false}`)

func luaRunTool() provider.GenerationTool {
	return provider.GenerationTool{Name: luaRunName,
		Description: "Run bounded sandboxed Lua 5.4 over a read-only JSON object and return one JSON value. Use input as the global input object. Use json.object() or json.array() for empty tables. math.random and math.randomseed are available with a fresh generator per call. os.time, os.date, os.difftime, and os.clock are available; os.time leaves date tables unchanged. Files, network, processes, environment variables, and module loading are unavailable.",
		InputSchema: append(json.RawMessage(nil), luaRunSchema...)}
}

func executeLuaTool(ctx context.Context, raw json.RawMessage) (json.RawMessage, bool) {
	if err := ctx.Err(); err != nil {
		return toolFailure("unavailable", "Lua execution was canceled"), false
	}
	decoded, err := script.DecodeJSON(raw)
	fields, ok := decoded.(map[string]any)
	if err != nil || !ok || len(fields) < 1 || len(fields) > 2 {
		return toolFailure("invalid_input", "code.run_lua arguments are invalid"), false
	}
	source, exists := fields["source"].(string)
	if !exists || strings.TrimSpace(source) == "" || utf8.RuneCountInString(source) > script.MaximumSourceChars || len(source) > script.MaximumSourceBytes {
		return toolFailure("invalid_input", "Lua source is invalid or too large"), false
	}
	for name := range fields {
		if name != "source" && name != "input" {
			return toolFailure("invalid_input", "code.run_lua arguments are invalid"), false
		}
	}
	input := any(map[string]any{})
	if value, exists := fields["input"]; exists {
		if _, ok := value.(map[string]any); !ok {
			return toolFailure("invalid_input", "Lua input must be a bounded JSON object"), false
		}
		input = value
	}
	value, err := script.Run(source, input)
	if err != nil {
		return toolFailure("execution_failed", "Sandboxed Lua execution failed"), false
	}
	payload, err := script.MarshalJSON(map[string]any{"value": value})
	if err != nil || len(payload) > script.OutputLimit+32 {
		return toolFailure("execution_failed", "Lua output is invalid or too large"), false
	}
	return payload, true
}
