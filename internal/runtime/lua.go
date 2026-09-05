package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"math"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/arnodel/golua/lib/base"
	"github.com/arnodel/golua/lib/mathlib"
	"github.com/arnodel/golua/lib/packagelib"
	"github.com/arnodel/golua/lib/stringlib"
	"github.com/arnodel/golua/lib/tablelib"
	"github.com/arnodel/golua/lib/utf8lib"
	lua "github.com/arnodel/golua/runtime"
	"github.com/kpsuperplane/noema/internal/provider"
)

const (
	luaRunName            = "code.run_lua"
	luaMaximumSourceChars = 50_000
	luaMaximumSourceBytes = 64 << 10
	luaMemoryLimit        = 16 << 20
	luaOutputLimit        = 1 << 20
	luaCPULimit           = 1_000_000
	luaTimeLimitMillis    = 250
	luaJSONMaximumDepth   = 64
	luaJSONMaximumNodes   = 16_384
	luaJSONMaximumItems   = 1_024
	luaJSONMaximumString  = 256 << 10
	luaHostTableCharge    = 384
)

var luaRunSchema = json.RawMessage(`{"type":"object","properties":{"source":{"type":"string","minLength":1,"maxLength":50000},"input":{"type":"object","additionalProperties":true}},"required":["source"],"additionalProperties":false}`)

type luaTableKind uint8

const (
	luaArray luaTableKind = iota + 1
	luaObject
)

type luaTableDescription struct {
	kind   luaTableKind
	values *lua.Table
}

type luaSandbox struct {
	runtime *lua.Runtime
	tables  map[*lua.Table]luaTableDescription
	null    *struct{}
}

func luaRunTool() provider.GenerationTool {
	return provider.GenerationTool{
		Name:        luaRunName,
		Description: "Run bounded sandboxed Lua 5.4 over a read-only JSON object and return one JSON value. Use input as the global input object. Use json.object() or json.array() for empty tables.",
		InputSchema: append(json.RawMessage(nil), luaRunSchema...),
	}
}

func executeLuaTool(ctx context.Context, raw json.RawMessage) (json.RawMessage, bool) {
	if err := ctx.Err(); err != nil {
		return toolFailure("unavailable", "Lua execution was canceled"), false
	}
	decoded, err := decodeUniqueLuaJSON(raw)
	fields, ok := decoded.(map[string]any)
	if err != nil || !ok || len(fields) < 1 || len(fields) > 2 {
		return toolFailure("invalid_input", "code.run_lua arguments are invalid"), false
	}
	source, sourceExists := fields["source"].(string)
	if !sourceExists ||
		strings.TrimSpace(source) == "" || utf8.RuneCountInString(source) > luaMaximumSourceChars || len(source) > luaMaximumSourceBytes {
		return toolFailure("invalid_input", "Lua source is invalid or too large"), false
	}
	for name := range fields {
		if name != "source" && name != "input" {
			return toolFailure("invalid_input", "code.run_lua arguments are invalid"), false
		}
	}
	input := any(map[string]any{})
	if value, exists := fields["input"]; exists {
		if _, ok := value.(map[string]any); !ok || !validLuaJSON(value, 0, new(int)) {
			return toolFailure("invalid_input", "Lua input must be a bounded JSON object"), false
		}
		input = value
	}
	value, err := runLua(source, input)
	if err != nil {
		return toolFailure("execution_failed", "Sandboxed Lua execution failed"), false
	}
	payload, err := json.Marshal(map[string]any{"value": value})
	if err != nil || len(payload) > luaOutputLimit+32 {
		return toolFailure("execution_failed", "Lua output is invalid or too large"), false
	}
	return payload, true
}

func runLua(source string, input any) (any, error) {
	var output any
	_, err := lua.DoInContext(func(runtime *lua.Runtime) error {
		sandbox := luaSandbox{runtime: runtime, tables: make(map[*lua.Table]luaTableDescription), null: &struct{}{}}
		if err := sandbox.load(); err != nil {
			return err
		}
		inputValue, err := sandbox.jsonToLua(input, true)
		if err != nil {
			return err
		}
		runtime.SetEnv(runtime.GlobalEnv(), "input", inputValue)
		chunk, err := runtime.CompileAndLoadLuaChunk("agent_code", []byte(source), lua.TableValue(runtime.GlobalEnv()))
		if err != nil {
			return err
		}
		values := make([]lua.Value, 0, 2)
		termination := lua.NewTermination(nil, nil, &values)
		if err := lua.Call(runtime.MainThread(), lua.FunctionValue(chunk), nil, termination); err != nil {
			return err
		}
		if len(values) != 1 {
			return errors.New("Lua source must return exactly one value")
		}
		output, err = sandbox.luaToJSON(values[0], 0, new(int), make(map[*lua.Table]bool))
		if err != nil {
			return err
		}
		encoded, err := json.Marshal(output)
		if err != nil || len(encoded) > luaOutputLimit {
			return errors.New("Lua output is invalid or too large")
		}
		return nil
	}, lua.RuntimeContextDef{
		HardLimits:    lua.RuntimeResources{Cpu: luaCPULimit, Memory: luaMemoryLimit, Millis: luaTimeLimitMillis},
		RequiredFlags: lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe,
	}, io.Discard)
	return output, err
}

func (s *luaSandbox) load() error {
	base.LibLoader.Run(s.runtime)
	packagelib.LibLoader.Run(s.runtime)
	stringlib.LibLoader.Run(s.runtime)
	tablelib.LibLoader.Run(s.runtime)
	mathlib.LibLoader.Run(s.runtime)
	utf8lib.LibLoader.Run(s.runtime)
	environment := s.runtime.GlobalEnv()
	for _, name := range []string{
		"collectgarbage", "coroutine", "debug", "dofile", "io", "load", "loadfile", "os",
		"package", "print", "rawset", "require", "runtime", "setmetatable", "warn",
	} {
		s.runtime.SetEnv(environment, name, lua.NilValue)
	}
	mathValue := lua.RawGet(environment, lua.StringValue("math"))
	mathTable, ok := mathValue.TryTable()
	if !ok {
		return errors.New("Lua math library is unavailable")
	}
	s.runtime.SetEnv(mathTable, "random", lua.NilValue)
	s.runtime.SetEnv(mathTable, "randomseed", lua.NilValue)
	next := s.runtime.SetEnvGoFunc(environment, "next", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		if err := call.Check1Arg(); err != nil {
			return nil, err
		}
		table, ok := call.Arg(0).TryTable()
		if !ok {
			return nil, errors.New("next requires a table")
		}
		if description, exists := s.tables[table]; exists {
			table = description.values
		}
		key := lua.NilValue
		if call.NArgs() > 1 {
			key = call.Arg(1)
		}
		key, value, valid := table.Next(key)
		if !valid {
			return nil, errors.New("next received an invalid key")
		}
		return call.PushingNext(thread.Runtime, key, value), nil
	}, 2, false)
	next.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)
	rawget := s.runtime.SetEnvGoFunc(environment, "rawget", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		if err := call.CheckNArgs(2); err != nil {
			return nil, err
		}
		table, ok := call.Arg(0).TryTable()
		if !ok {
			return nil, errors.New("rawget requires a table")
		}
		if description, exists := s.tables[table]; exists {
			table = description.values
		}
		return call.PushingNext1(thread.Runtime, lua.RawGet(table, call.Arg(1))), nil
	}, 2, false)
	rawget.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)

	jsonTable := s.newTable()
	s.runtime.SetEnv(jsonTable, "null", s.runtime.NewUserDataValue(s.null, nil))
	decode := s.runtime.SetEnvGoFunc(jsonTable, "decode", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		if err := call.Check1Arg(); err != nil {
			return nil, err
		}
		raw, err := call.StringArg(0)
		if err != nil || len(raw) > luaMemoryLimit {
			return nil, errors.New("invalid JSON")
		}
		value, err := decodeLuaJSON([]byte(raw))
		if err != nil {
			return nil, errors.New("invalid JSON")
		}
		result, err := s.jsonToLua(value, false)
		if err != nil {
			return nil, errors.New("invalid JSON")
		}
		return call.PushingNext1(thread.Runtime, result), nil
	}, 1, false)
	decode.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)
	encode := s.runtime.SetEnvGoFunc(jsonTable, "encode", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		if err := call.Check1Arg(); err != nil {
			return nil, err
		}
		value, err := s.luaToJSON(call.Arg(0), 0, new(int), make(map[*lua.Table]bool))
		if err != nil {
			return nil, errors.New("invalid JSON value")
		}
		encoded, err := json.Marshal(value)
		if err != nil || len(encoded) > luaOutputLimit {
			return nil, errors.New("JSON output is too large")
		}
		thread.RequireBytes(len(encoded))
		return call.PushingNext1(thread.Runtime, lua.StringValue(string(encoded))), nil
	}, 1, false)
	encode.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)
	for name, kind := range map[string]luaTableKind{"array": luaArray, "object": luaObject} {
		kind := kind
		function := s.runtime.SetEnvGoFunc(jsonTable, name, func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
			table := s.newTable()
			s.tables[table] = luaTableDescription{kind: kind, values: table}
			return call.PushingNext1(thread.Runtime, lua.TableValue(table)), nil
		}, 0, false)
		function.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)
	}
	jsonProxy := s.readOnlyProxy(jsonTable)
	s.tables[jsonProxy] = luaTableDescription{kind: luaObject, values: jsonTable}
	s.runtime.SetEnv(environment, "json", lua.TableValue(jsonProxy))
	return nil
}

func (s *luaSandbox) newTable() *lua.Table {
	s.runtime.RequireMem(luaHostTableCharge)
	return lua.NewTable()
}

func decodeLuaJSON(raw []byte) (any, error) {
	value, err := decodeUniqueLuaJSON(raw)
	if err != nil {
		return nil, err
	}
	if !validLuaJSON(value, 0, new(int)) {
		return nil, errors.New("JSON exceeds its limits")
	}
	return value, nil
}

func decodeUniqueLuaJSON(raw []byte) (any, error) {
	if len(raw) > luaMemoryLimit {
		return nil, errors.New("JSON exceeds its byte limit")
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	value, err := readLuaJSON(decoder, 0, new(int))
	if err != nil {
		return nil, err
	}
	if token, err := decoder.Token(); err != io.EOF {
		_ = token
		return nil, errors.New("JSON has trailing content")
	}
	return value, nil
}

func readLuaJSON(decoder *json.Decoder, depth int, nodes *int) (any, error) {
	*nodes++
	if depth > luaJSONMaximumDepth+2 || *nodes > luaJSONMaximumNodes+4 {
		return nil, errors.New("JSON exceeds its limits")
	}
	token, err := decoder.Token()
	if err != nil {
		return nil, err
	}
	delimiter, nested := token.(json.Delim)
	if !nested {
		return token, nil
	}
	switch delimiter {
	case '{':
		object := make(map[string]any)
		for decoder.More() {
			if len(object) >= luaJSONMaximumItems {
				return nil, errors.New("JSON object exceeds its limits")
			}
			key, err := decoder.Token()
			text, ok := key.(string)
			if err != nil || !ok {
				return nil, errors.New("JSON object key is invalid")
			}
			if _, exists := object[text]; exists {
				return nil, errors.New("JSON object has a duplicate key")
			}
			value, err := readLuaJSON(decoder, depth+1, nodes)
			if err != nil {
				return nil, err
			}
			object[text] = value
		}
		if _, err := decoder.Token(); err != nil {
			return nil, err
		}
		return object, nil
	case '[':
		array := make([]any, 0)
		for decoder.More() {
			if len(array) >= luaJSONMaximumItems {
				return nil, errors.New("JSON array exceeds its limits")
			}
			value, err := readLuaJSON(decoder, depth+1, nodes)
			if err != nil {
				return nil, err
			}
			array = append(array, value)
		}
		if _, err := decoder.Token(); err != nil {
			return nil, err
		}
		return array, nil
	default:
		return nil, errors.New("JSON delimiter is invalid")
	}
}

func validLuaJSON(value any, depth int, nodes *int) bool {
	*nodes++
	if depth > luaJSONMaximumDepth || *nodes > luaJSONMaximumNodes {
		return false
	}
	switch value := value.(type) {
	case nil, bool, json.Number:
		return true
	case string:
		return len(value) <= luaJSONMaximumString
	case []any:
		if len(value) > luaJSONMaximumItems {
			return false
		}
		for _, item := range value {
			if !validLuaJSON(item, depth+1, nodes) {
				return false
			}
		}
	case map[string]any:
		if len(value) > luaJSONMaximumItems {
			return false
		}
		for key, item := range value {
			if len(key) > luaJSONMaximumString || !validLuaJSON(item, depth+1, nodes) {
				return false
			}
		}
	default:
		return false
	}
	return true
}

func (s *luaSandbox) jsonToLua(value any, readOnly bool) (lua.Value, error) {
	switch value := value.(type) {
	case nil:
		return s.runtime.NewUserDataValue(s.null, nil), nil
	case bool:
		return lua.BoolValue(value), nil
	case string:
		s.runtime.RequireBytes(len(value))
		return lua.StringValue(value), nil
	case json.Number:
		if integer, err := strconv.ParseInt(string(value), 10, 64); err == nil {
			return lua.IntValue(integer), nil
		}
		number, err := strconv.ParseFloat(string(value), 64)
		if err != nil || math.IsInf(number, 0) || math.IsNaN(number) {
			return lua.NilValue, errors.New("JSON number is invalid")
		}
		return lua.FloatValue(number), nil
	case []any:
		return s.jsonCollection(luaArray, value, nil, readOnly)
	case map[string]any:
		return s.jsonCollection(luaObject, nil, value, readOnly)
	default:
		return lua.NilValue, errors.New("JSON value is invalid")
	}
}

func (s *luaSandbox) jsonCollection(kind luaTableKind, array []any, object map[string]any, readOnly bool) (lua.Value, error) {
	values := s.newTable()
	if kind == luaArray {
		for index, item := range array {
			value, err := s.jsonToLua(item, readOnly)
			if err != nil {
				return lua.NilValue, err
			}
			s.runtime.SetTable(values, lua.IntValue(int64(index+1)), value)
		}
	} else {
		keys := make([]string, 0, len(object))
		for key := range object {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			item := object[key]
			value, err := s.jsonToLua(item, readOnly)
			if err != nil {
				return lua.NilValue, err
			}
			s.runtime.RequireBytes(len(key))
			s.runtime.SetTable(values, lua.StringValue(key), value)
		}
	}
	if !readOnly {
		s.tables[values] = luaTableDescription{kind: kind, values: values}
		return lua.TableValue(values), nil
	}
	proxy := s.readOnlyProxy(values)
	s.tables[proxy] = luaTableDescription{kind: kind, values: values}
	return lua.TableValue(proxy), nil
}

func (s *luaSandbox) readOnlyProxy(values *lua.Table) *lua.Table {
	proxy := s.newTable()
	meta := s.newTable()
	s.runtime.SetEnv(meta, "__index", lua.TableValue(values))
	s.runtime.SetEnv(meta, "__metatable", lua.BoolValue(false))
	deny := lua.NewGoFunction(func(_ *lua.Thread, _ *lua.GoCont) (lua.Cont, error) {
		return nil, errors.New("attempt to modify read-only input")
	}, "readonly", 3, false)
	deny.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)
	s.runtime.SetEnv(meta, "__newindex", lua.FunctionValue(deny))
	length := lua.NewGoFunction(func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		return call.PushingNext1(thread.Runtime, lua.IntValue(values.Len())), nil
	}, "readonly_len", 1, false)
	length.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)
	s.runtime.SetEnv(meta, "__len", lua.FunctionValue(length))
	next := lua.RawGet(s.runtime.GlobalEnv(), lua.StringValue("next"))
	pairs := lua.NewGoFunction(func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		return call.PushingNext(thread.Runtime, next, lua.TableValue(values), lua.NilValue), nil
	}, "readonly_pairs", 1, false)
	pairs.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)
	s.runtime.SetEnv(meta, "__pairs", lua.FunctionValue(pairs))
	proxy.SetMetatable(meta)
	return proxy
}

func (s *luaSandbox) luaToJSON(value lua.Value, depth int, nodes *int, active map[*lua.Table]bool) (any, error) {
	*nodes++
	if depth > luaJSONMaximumDepth || *nodes > luaJSONMaximumNodes {
		return nil, errors.New("Lua output exceeds JSON limits")
	}
	if value.IsNil() {
		return nil, errors.New("Lua nil is not JSON null")
	}
	if boolean, ok := value.TryBool(); ok {
		return boolean, nil
	}
	if integer, ok := value.TryInt(); ok {
		return integer, nil
	}
	if number, ok := value.TryFloat(); ok {
		if math.IsInf(number, 0) || math.IsNaN(number) {
			return nil, errors.New("Lua number is not finite")
		}
		return number, nil
	}
	if text, ok := value.TryString(); ok {
		if len(text) > luaJSONMaximumString {
			return nil, errors.New("Lua string exceeds JSON limits")
		}
		return text, nil
	}
	if user, ok := value.TryUserData(); ok && user.Value() == s.null {
		return nil, nil
	}
	table, ok := value.TryTable()
	if !ok || active[table] {
		return nil, errors.New("Lua output is not one acyclic JSON value")
	}
	active[table] = true
	defer delete(active, table)
	description, tagged := s.tables[table]
	values := table
	if tagged {
		values = description.values
	}
	entries := make([][2]lua.Value, 0, values.Len())
	for key := lua.NilValue; ; {
		next, item, valid := values.Next(key)
		if !valid {
			return nil, errors.New("Lua table iteration failed")
		}
		if next.IsNil() {
			break
		}
		entries = append(entries, [2]lua.Value{next, item})
		if len(entries) > luaJSONMaximumItems {
			return nil, errors.New("Lua table exceeds JSON limits")
		}
		key = next
	}
	kind := description.kind
	if !tagged {
		if len(entries) == 0 {
			return nil, errors.New("empty Lua tables need an explicit JSON shape")
		}
		kind = luaArray
		for _, entry := range entries {
			if _, ok := entry[0].TryInt(); !ok {
				kind = luaObject
				break
			}
		}
	}
	if kind == luaArray {
		result := make([]any, len(entries))
		seen := make([]bool, len(entries))
		for _, entry := range entries {
			index, ok := entry[0].TryInt()
			if !ok || index < 1 || index > int64(len(entries)) || seen[index-1] {
				return nil, errors.New("Lua array keys are invalid")
			}
			item, err := s.luaToJSON(entry[1], depth+1, nodes, active)
			if err != nil {
				return nil, err
			}
			seen[index-1], result[index-1] = true, item
		}
		return result, nil
	}
	result := make(map[string]any, len(entries))
	for _, entry := range entries {
		key, ok := entry[0].TryString()
		if !ok || len(key) > luaJSONMaximumString {
			return nil, errors.New("Lua object keys are invalid")
		}
		item, err := s.luaToJSON(entry[1], depth+1, nodes, active)
		if err != nil {
			return nil, err
		}
		result[key] = item
	}
	return result, nil
}
