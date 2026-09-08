package runtime

import (
	"context"
	"encoding/json"
	"regexp"
	"strings"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/script"
)

const luaRunName = "code.run_luau"

var luaRunSchema = json.RawMessage(`{"type":"object","properties":{"source":{"type":"string","minLength":1,"maxLength":50000},"input":{"type":"object","additionalProperties":true}},"required":["source"],"additionalProperties":false}`)

func luaRunTool() provider.GenerationTool {
	return provider.GenerationTool{Name: luaRunName,
		Description: "Run bounded sandboxed Luau over a read-only JSON object and return one JSON value. Use input as the global input object. Use json.object() or json.array() for empty tables.",
		InputSchema: append(json.RawMessage(nil), luaRunSchema...)}
}

func executeLuaTool(ctx context.Context, raw json.RawMessage) (json.RawMessage, bool) {
	if err := ctx.Err(); err != nil {
		return toolFailure("unavailable", "Lua execution was canceled"), false
	}
	decoded, err := script.DecodeJSON(raw)
	fields, ok := decoded.(map[string]any)
	if err != nil || !ok || len(fields) < 1 || len(fields) > 2 {
		return toolFailure("invalid_input", "code.run_luau arguments are invalid"), false
	}
	source, exists := fields["source"].(string)
	if !exists || strings.TrimSpace(source) == "" || utf8.RuneCountInString(source) > script.MaximumSourceChars || len(source) > script.MaximumSourceBytes {
		return toolFailure("invalid_input", "Lua source is invalid or too large"), false
	}
	for name := range fields {
		if name != "source" && name != "input" {
			return toolFailure("invalid_input", "code.run_luau arguments are invalid"), false
		}
	}
	input := any(map[string]any{})
	if value, exists := fields["input"]; exists {
		if _, ok := value.(map[string]any); !ok {
			return toolFailure("invalid_input", "Lua input must be a bounded JSON object"), false
		}
		input = value
	}
	value, err := script.Run(normalizeLuauSource(source), input)
	if err != nil {
		return toolFailure("execution_failed", "Sandboxed Lua execution failed"), false
	}
	payload, err := script.MarshalJSON(map[string]any{"value": value})
	if err != nil || len(payload) > script.OutputLimit+32 {
		return toolFailure("execution_failed", "Lua output is invalid or too large"), false
	}
	return payload, true
}

var (
	luauArrayForPattern  = regexp.MustCompile(`for\s+([A-Za-z_][A-Za-z0-9_]*)\s*,\s*([A-Za-z_][A-Za-z0-9_]*)\s+in\s+(input(?:\.[A-Za-z_][A-Za-z0-9_]*|\[[^\]]+\]))\s+do`)
	luauAddAssignPattern = regexp.MustCompile(`\b([A-Za-z_][A-Za-z0-9_]*)\s*\+=\s*([A-Za-z_][A-Za-z0-9_]*|[-+]?[0-9]+(?:\.[0-9]+)?)`)
	luauSubAssignPattern = regexp.MustCompile(`\b([A-Za-z_][A-Za-z0-9_]*)\s*-=\s*([A-Za-z_][A-Za-z0-9_]*|[-+]?[0-9]+(?:\.[0-9]+)?)`)
)

// normalizeLuauSource adapts the small Luau syntax extensions used by the
// product to the Lua 5.4 parser used by the sandbox. Unsupported syntax still
// reaches the sandbox parser and fails closed.
func normalizeLuauSource(source string) string {
	source = luauArrayForPattern.ReplaceAllString(source, "for $1, $2 in ipairs($3) do")
	source = luauAddAssignPattern.ReplaceAllString(source, "$1 = $1 + $2")
	return luauSubAssignPattern.ReplaceAllString(source, "$1 = $1 - $2")
}
