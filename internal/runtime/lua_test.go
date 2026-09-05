package runtime

import (
	"context"
	"encoding/json"
	"strings"
	"testing"
	"time"
)

func runLuaToolForTest(t *testing.T, source, input string) (map[string]any, bool) {
	t.Helper()
	arguments := `{"source":` + string(mustJSON(source))
	if input != "" {
		arguments += `,"input":` + input
	}
	arguments += `}`
	payload, success := executeLuaTool(context.Background(), json.RawMessage(arguments))
	var decoded map[string]any
	if err := json.Unmarshal(payload, &decoded); err != nil {
		t.Fatalf("decode Lua result: %v", err)
	}
	return decoded, success
}

func TestLuaRunsStandardSyntaxForChatAndTaskRoles(t *testing.T) {
	payload, success := runLuaToolForTest(t,
		`local total = 0 for _, value in ipairs(input.values) do total = total + value end local index, first = next(input.values) return { total = total, count = #input.values, index = index, first = first, raw = rawget(input.values, 1) }`,
		`{"values":[125,250,375]}`)
	if !success {
		t.Fatalf("Lua failed: %#v", payload)
	}
	want := map[string]any{"value": map[string]any{"total": float64(750), "count": float64(3), "index": float64(1), "first": float64(125), "raw": float64(125)}}
	if string(mustJSON(payload)) != string(mustJSON(want)) {
		t.Fatalf("Lua result = %#v", payload)
	}
	if !supportsLocalChatTool(luaRunName) {
		t.Fatal("primary Chat does not support code.run_lua")
	}
	for _, role := range []string{"planner", "executor", "reviewer"} {
		if !taskToolAllowed(role, luaRunName) {
			t.Fatalf("%s cannot run code.run_lua", role)
		}
		found := false
		for _, tool := range taskExecutionTools(role) {
			found = found || tool.Name == luaRunName
		}
		if !found {
			t.Fatalf("%s catalog omits code.run_lua", role)
		}
	}
}

func TestLuaPreservesJSONShapesAndReadOnlyInput(t *testing.T) {
	source := `local decoded = json.decode(json.encode(input))
decoded.copy = true
return { original = input, decoded = decoded, null = json.null, array = json.array(), object = json.object() }`
	payload, success := runLuaToolForTest(t, source, `{"name":"ordinary","items":[],"details":{},"optional":null}`)
	if !success {
		t.Fatalf("Lua failed: %#v", payload)
	}
	want := `{"value":{"array":[],"decoded":{"copy":true,"details":{},"items":[],"name":"ordinary","optional":null},"null":null,"object":{},"original":{"details":{},"items":[],"name":"ordinary","optional":null}}}`
	if string(mustJSON(payload)) != want {
		t.Fatalf("JSON shapes changed: %s", mustJSON(payload))
	}
	for _, mutation := range []string{
		`input.name = "changed" return input`,
		`input.items[1] = "changed" return input`,
		`table.insert(input.items, "changed") return input`,
		`local _, backing = pairs(input.items) backing[1] = "changed" return input`,
	} {
		if result, ok := runLuaToolForTest(t, mutation, `{"name":"ordinary","items":[]}`); ok {
			t.Fatalf("read-only input mutation succeeded: %#v", result)
		}
	}
}

func TestLuaRejectsOutOfRangeIntegralInput(t *testing.T) {
	payload, success := runLuaToolForTest(t, `return input.value`, `{"value":9223372036854775809}`)
	if success {
		t.Fatalf("out-of-range integer was rounded: %#v", payload)
	}
}

func TestLuaRejectsInvalidUTF8Output(t *testing.T) {
	for name, source := range map[string]string{
		"value": `return string.char(255)`,
		"keys":  `local value = json.object() value[string.char(255)] = 1 value[string.char(254)] = 2 return value`,
	} {
		t.Run(name, func(t *testing.T) {
			payload, success := runLuaToolForTest(t, source, "")
			if success {
				t.Fatalf("invalid UTF-8 output succeeded: %#v", payload)
			}
		})
	}
}

func TestLuaOutputLimitUsesUnescapedJSON(t *testing.T) {
	for name, source := range map[string]string{
		"result": `return string.rep("<", 256 * 1024)`,
		"encode": `return json.encode(string.rep("<", 200 * 1024))`,
	} {
		t.Run(name, func(t *testing.T) {
			payload, success := runLuaToolForTest(t, source, "")
			if !success {
				t.Fatalf("bounded output failed: %#v", payload)
			}
			value, ok := payload["value"].(string)
			if !ok || !strings.Contains(value, "<") {
				t.Fatalf("bounded output changed: %#v", payload)
			}
		})
	}
}

func TestLuaExposesNoAmbientAuthority(t *testing.T) {
	source := `return {
os = os ~= nil, io = io ~= nil, package = package ~= nil, require = require ~= nil,
debug = debug ~= nil, coroutine = coroutine ~= nil, load = load ~= nil,
collectgarbage = collectgarbage ~= nil, print = print ~= nil, rawset = rawset ~= nil,
setmetatable = setmetatable ~= nil, random = math.random ~= nil,
json_mutable = pcall(function() json.null = nil end),
standard = string.upper("lua") .. table.concat({"5", "4"}, ".") .. ":" .. utf8.len("✓")
}`
	payload, success := runLuaToolForTest(t, source, "")
	if !success {
		t.Fatalf("Lua failed: %#v", payload)
	}
	encoded := string(mustJSON(payload))
	if strings.Contains(encoded, `:true`) || !strings.Contains(encoded, `"standard":"LUA5.4:1"`) {
		t.Fatalf("sandbox globals = %s", encoded)
	}
}

func TestLuaTerminatesTimeAndMemoryExhaustion(t *testing.T) {
	for name, source := range map[string]string{
		"time":   `while true do end`,
		"memory": `local values = {} while true do values[#values + 1] = string.rep("x", 4096) end`,
	} {
		t.Run(name, func(t *testing.T) {
			started := time.Now()
			payload, success := runLuaToolForTest(t, source, "")
			if success || time.Since(started) > 2*time.Second {
				t.Fatalf("unbounded Lua result after %s: %#v", time.Since(started), payload)
			}
		})
	}
}
