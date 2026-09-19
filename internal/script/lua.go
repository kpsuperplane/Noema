// Package script owns bounded Lua execution shared by tools and adapters.
package script

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"math"
	"sort"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/arnodel/golua/lib/base"
	"github.com/arnodel/golua/lib/mathlib"
	"github.com/arnodel/golua/lib/packagelib"
	"github.com/arnodel/golua/lib/stringlib"
	"github.com/arnodel/golua/lib/tablelib"
	"github.com/arnodel/golua/lib/utf8lib"
	lua "github.com/arnodel/golua/runtime"
)

const (
	MaximumSourceChars   = 50_000
	MaximumSourceBytes   = 64 << 10
	luaMemoryLimit       = 16 << 20
	OutputLimit          = 1 << 20
	luaTextLimit         = 32 << 10
	luaCPULimit          = 1_000_000
	luaTimeLimitMillis   = 250
	luaJSONMaximumDepth  = 64
	luaJSONMaximumNodes  = 16_384
	luaJSONMaximumItems  = 1_024
	luaJSONMaximumString = 256 << 10
	luaHostTableCharge   = 384
)

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

// SandboxProfile selects the reviewed helper surface exposed to one script.
type SandboxProfile uint8

const (
	ProfileAgent SandboxProfile = iota
	ProfileResponse
	ProfileCredential
	ProfileRequestAuth
)

// Run executes one bounded Lua chunk over a read-only JSON input.
func Run(source string, input any) (any, error) {
	return run(source, input, false, ProfileAgent)
}

// RunFunction executes source which returns one function, then calls it with input.
func RunFunction(source string, input any) (any, error) {
	return run(source, input, true, ProfileAgent)
}

// RunFunctionWithProfile executes one transform with the helper authority
// allowed by its production boundary.
func RunFunctionWithProfile(source string, input any, profile SandboxProfile) (any, error) {
	return run(source, input, true, profile)
}

// ValidateFunction checks one bounded Lua function chunk without executing it.
func ValidateFunction(source string) error {
	return ValidateFunctionWithProfile(source, ProfileAgent)
}

// ValidateFunctionWithProfile checks one transform against its reviewed
// helper authority without executing it.
func ValidateFunctionWithProfile(source string, profile SandboxProfile) error {
	if strings.TrimSpace(source) == "" || len(source) > MaximumSourceBytes || utf8.RuneCountInString(source) > MaximumSourceChars ||
		!strings.HasPrefix(strings.TrimLeftFunc(source, unicode.IsSpace), "return function(") {
		return errors.New("Lua function source is invalid")
	}
	_, err := lua.DoInContext(func(runtime *lua.Runtime) error {
		sandbox := luaSandbox{runtime: runtime, tables: make(map[*lua.Table]luaTableDescription), null: &struct{}{}}
		if err := sandbox.load(profile); err != nil {
			return err
		}
		_, err := runtime.CompileAndLoadLuaChunk("validated_function", []byte(source), lua.TableValue(runtime.GlobalEnv()))
		return err
	}, lua.RuntimeContextDef{
		HardLimits: lua.RuntimeResources{Cpu: luaCPULimit, Memory: luaMemoryLimit, Millis: luaTimeLimitMillis}, RequiredFlags: lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe,
	}, io.Discard)
	return err
}

func run(source string, input any, callReturned bool, profile SandboxProfile) (any, error) {
	if strings.TrimSpace(source) == "" || len(source) > MaximumSourceBytes || utf8.RuneCountInString(source) > MaximumSourceChars || !validLuaJSON(input, 0, new(int)) {
		return nil, errors.New("Lua source or input exceeds its limits")
	}
	var output any
	_, err := lua.DoInContext(func(runtime *lua.Runtime) error {
		sandbox := luaSandbox{runtime: runtime, tables: make(map[*lua.Table]luaTableDescription), null: &struct{}{}}
		if err := sandbox.load(profile); err != nil {
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
		if callReturned {
			function := values[0]
			values = values[:0]
			termination = lua.NewTermination(nil, nil, &values)
			if err := lua.Call(runtime.MainThread(), function, []lua.Value{inputValue}, termination); err != nil {
				return err
			}
			if len(values) != 1 {
				return errors.New("Lua function must return exactly one value")
			}
		}
		output, err = sandbox.luaToJSON(values[0], 0, new(int), make(map[*lua.Table]bool))
		if err != nil {
			return err
		}
		encoded, err := marshalLuaJSON(output)
		if err != nil || len(encoded) > OutputLimit {
			return errors.New("Lua output is invalid or too large")
		}
		return nil
	}, lua.RuntimeContextDef{
		HardLimits:    lua.RuntimeResources{Cpu: luaCPULimit, Memory: luaMemoryLimit, Millis: luaTimeLimitMillis},
		RequiredFlags: lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe,
	}, io.Discard)
	return output, err
}

func (s *luaSandbox) load(profile SandboxProfile) error {
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
	if profile == ProfileAgent {
		s.loadAgentTimeAndRandom(mathTable)
	} else {
		s.runtime.SetEnv(mathTable, "random", lua.NilValue)
		s.runtime.SetEnv(mathTable, "randomseed", lua.NilValue)
	}
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
		encoded, err := marshalLuaJSON(value)
		if err != nil || len(encoded) > OutputLimit {
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
	textTable := s.newTable()
	truncate := s.runtime.SetEnvGoFunc(textTable, "truncate_utf8", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		if err := call.CheckNArgs(2); err != nil {
			return nil, err
		}
		value, valueErr := call.StringArg(0)
		limit, limitErr := call.IntArg(1)
		if valueErr != nil || limitErr != nil || limit < 0 || limit > luaTextLimit {
			return nil, errors.New("invalid UTF-8 truncation")
		}
		if int64(len(value)) > limit {
			value = value[:limit]
			for !utf8.ValidString(value) {
				value = value[:len(value)-1]
			}
		}
		return call.PushingNext1(thread.Runtime, lua.StringValue(value)), nil
	}, 2, false)
	truncate.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)
	if profile == ProfileResponse {
		decodeText := s.runtime.SetEnvGoFunc(textTable, "decode_base64url_utf8", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
			if err := call.CheckNArgs(2); err != nil {
				return nil, err
			}
			encoded, encodedErr := call.StringArg(0)
			limit, limitErr := call.IntArg(1)
			if encodedErr != nil || limitErr != nil || limit < 0 || limit > luaTextLimit {
				return nil, errors.New("invalid base64url text limit")
			}
			decoded, err := base64.RawURLEncoding.DecodeString(encoded)
			if err != nil || !utf8.Valid(decoded) {
				return nil, errors.New("invalid base64url UTF-8 text")
			}
			value := string(decoded)
			end := len(value)
			if int64(end) > limit {
				end = int(limit)
				for !utf8.ValidString(value[:end]) {
					end--
				}
			}
			return call.PushingNext1(thread.Runtime, lua.StringValue(value[:end])), nil
		}, 2, false)
		decodeText.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)
		textProxy := s.readOnlyProxy(textTable)
		s.tables[textProxy] = luaTableDescription{kind: luaObject, values: textTable}
		s.runtime.SetEnv(environment, "text", lua.TableValue(textProxy))
	}
	if profile == ProfileRequestAuth {
		encodingTable := s.newTable()
		encodeBase64 := s.runtime.SetEnvGoFunc(encodingTable, "base64", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
			if err := call.Check1Arg(); err != nil {
				return nil, err
			}
			value, err := call.StringArg(0)
			if err != nil {
				return nil, err
			}
			return call.PushingNext1(thread.Runtime, lua.StringValue(base64.StdEncoding.EncodeToString([]byte(value)))), nil
		}, 1, false)
		encodeBase64.SolemnlyDeclareCompliance(lua.ComplyMemSafe | lua.ComplyCpuSafe | lua.ComplyTimeSafe | lua.ComplyIoSafe)
		encodingProxy := s.readOnlyProxy(encodingTable)
		s.tables[encodingProxy] = luaTableDescription{kind: luaObject, values: encodingTable}
		s.runtime.SetEnv(environment, "encoding", lua.TableValue(encodingProxy))
	}
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

func marshalLuaJSON(value any) ([]byte, error) {
	var output bytes.Buffer
	encoder := json.NewEncoder(&output)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	return bytes.TrimSuffix(output.Bytes(), []byte("\n")), nil
}

// MarshalJSON encodes one bounded Lua-compatible JSON value.
func MarshalJSON(value any) ([]byte, error) { return marshalLuaJSON(value) }

// DecodeJSON decodes one duplicate-free bounded JSON value.
func DecodeJSON(raw []byte) (any, error) { return decodeLuaJSON(raw) }

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
		text := string(value)
		if !strings.ContainsAny(text, ".eE") {
			integer, err := strconv.ParseInt(text, 10, 64)
			if err != nil {
				return lua.NilValue, errors.New("JSON integer is out of range")
			}
			return lua.IntValue(integer), nil
		}
		number, err := strconv.ParseFloat(text, 64)
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
		return call.PushingNext(thread.Runtime, next, lua.TableValue(proxy), lua.NilValue), nil
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
		if !utf8.ValidString(text) || len(text) > luaJSONMaximumString {
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
		if !ok || !utf8.ValidString(key) || len(key) > luaJSONMaximumString {
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
