package provider

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"hash/fnv"
	"io"
	"sort"
	"strings"
)

const (
	openRouterFunctionNameLimit = 64
)

// OpenRouterToolTransport selects the model tool channel.
type OpenRouterToolTransport string

const (
	// OpenRouterToolTransportNone disables native tools.
	OpenRouterToolTransportNone OpenRouterToolTransport = "none"
	// OpenRouterToolTransportNative enables native function tools.
	OpenRouterToolTransportNative OpenRouterToolTransport = "native"
)

// OpenRouterToolChoice selects provider tool use.
type OpenRouterToolChoice string

const (
	OpenRouterToolChoiceAuto     OpenRouterToolChoice = "auto"
	OpenRouterToolChoiceNone     OpenRouterToolChoice = "none"
	OpenRouterToolChoiceRequired OpenRouterToolChoice = "required"
)

// OpenRouterTool is one canonical model-visible function.
type OpenRouterTool struct {
	Name        string
	Description string
	InputSchema json.RawMessage
}

type openRouterToolNameMap struct {
	providerToRule  map[string]openRouterToolRule
	canonicalToName map[string]string
}

type openRouterToolRule struct {
	canonical    string
	sourceSchema any
	strict       bool
}

type openRouterToolPayload struct {
	Type     string                  `json:"type"`
	Function *openRouterFunctionTool `json:"function,omitempty"`
}

type openRouterFunctionTool struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Parameters  any    `json:"parameters"`
	Strict      bool   `json:"strict"`
}

type openRouterToolCallPayload struct {
	ID       string                            `json:"id"`
	Type     string                            `json:"type"`
	Function openRouterToolCallFunctionPayload `json:"function"`
}

type openRouterToolCallFunctionPayload struct {
	Name      string `json:"name"`
	Arguments string `json:"arguments"`
}

func prepareOpenRouterTools(
	tools []OpenRouterTool,
) (openRouterToolNameMap, []openRouterToolPayload, error) {
	names := make([]string, len(tools))
	groups := make(map[string][]int)
	for index, tool := range tools {
		if err := validateOpenRouterToolName(tool.Name); err != nil {
			return openRouterToolNameMap{}, nil, err
		}
		names[index] = openRouterProviderSafeName(tool.Name)
		groups[names[index]] = append(groups[names[index]], index)
	}
	for _, indexes := range groups {
		if len(indexes) < 2 {
			continue
		}
		hashes := make([]string, len(indexes))
		for index, toolIndex := range indexes {
			hashes[index] = fmt.Sprintf("%016x", openRouterFNV64(tools[toolIndex].Name))
		}
		prefixLength := 16
		for length := 1; length <= 16; length++ {
			seen := make(map[string]struct{}, len(hashes))
			for _, hash := range hashes {
				seen[hash[:length]] = struct{}{}
			}
			if len(seen) == len(hashes) {
				prefixLength = length
				break
			}
		}
		for index, toolIndex := range indexes {
			base := names[toolIndex]
			base = base[:min(len(base), openRouterFunctionNameLimit-prefixLength-1)]
			names[toolIndex] = base + "_" + hashes[index][:prefixLength]
		}
	}

	nameMap := openRouterToolNameMap{
		providerToRule:  make(map[string]openRouterToolRule, len(tools)),
		canonicalToName: make(map[string]string, len(tools)),
	}
	wire := make([]openRouterToolPayload, 0, len(tools))
	for index, tool := range tools {
		providerName := names[index]
		if _, exists := nameMap.providerToRule[providerName]; exists {
			return openRouterToolNameMap{}, nil, errors.New("OpenRouter tool names are not unique")
		}
		description := strings.TrimSpace(tool.Description)
		if description == "" {
			return openRouterToolNameMap{}, nil, errors.New("OpenRouter tool description is required")
		}
		source, err := decodeOpenRouterJSON(tool.InputSchema)
		if err != nil || jsonObject(source) == nil || jsonString(jsonObject(source)["type"]) != "object" {
			return openRouterToolNameMap{}, nil, errors.New("OpenRouter tool schema root must be an object")
		}
		providerSchema, err := cloneOpenRouterJSON(source)
		if err != nil {
			return openRouterToolNameMap{}, nil, errors.New("OpenRouter tool schema is invalid")
		}
		strict := lowerOpenRouterStrictSchema(providerSchema, "$") == nil
		if !strict {
			providerSchema, _ = cloneOpenRouterJSON(source)
			normalizeOpenRouterSchema(providerSchema)
		}
		nameMap.providerToRule[providerName] = openRouterToolRule{
			canonical: tool.Name, sourceSchema: source, strict: strict,
		}
		nameMap.canonicalToName[tool.Name] = providerName
		wire = append(wire, openRouterToolPayload{
			Type: "function",
			Function: &openRouterFunctionTool{
				Name: providerName, Description: description,
				Parameters: providerSchema, Strict: strict,
			},
		})
	}
	return nameMap, wire, nil
}

func openRouterHostedSearchTool() openRouterToolPayload {
	return openRouterToolPayload{Type: "openrouter:web_search"}
}

func openRouterToolControls(
	request OpenRouterGenerateRequest,
	hasFunctions bool,
) (string, *bool, error) {
	choice := request.ToolChoice
	if choice == "" {
		choice = OpenRouterToolChoiceAuto
	}
	if choice != OpenRouterToolChoiceAuto && choice != OpenRouterToolChoiceNone &&
		choice != OpenRouterToolChoiceRequired {
		return "", nil, errors.New("OpenRouter tool choice is invalid")
	}
	if !hasFunctions {
		return "", nil, nil
	}
	return string(choice), boolPointer(request.ParallelTools), nil
}

func boolPointer(value bool) *bool { return &value }

func validateOpenRouterToolName(name string) error {
	if name == "" || strings.TrimSpace(name) != name {
		return errors.New("OpenRouter tool name is invalid")
	}
	for _, character := range name {
		if !((character >= 'a' && character <= 'z') ||
			(character >= 'A' && character <= 'Z') ||
			(character >= '0' && character <= '9') ||
			character == '_' || character == '-' || character == '.' || character == ':') {
			return errors.New("OpenRouter tool name is invalid")
		}
	}
	for _, segment := range strings.Split(name, ".") {
		valid := false
		for _, character := range segment {
			if (character >= 'a' && character <= 'z') ||
				(character >= 'A' && character <= 'Z') ||
				(character >= '0' && character <= '9') {
				valid = true
				break
			}
		}
		if !valid {
			return errors.New("OpenRouter tool name is invalid")
		}
	}
	return nil
}

func openRouterProviderSafeName(canonical string) string {
	meaningful := canonical
	if index := strings.LastIndexByte(canonical, '.'); index >= 0 && index+1 < len(canonical) {
		meaningful = canonical[index+1:]
	}
	var encoded strings.Builder
	for _, value := range []byte(meaningful) {
		if (value >= 'a' && value <= 'z') || (value >= 'A' && value <= 'Z') ||
			(value >= '0' && value <= '9') || value == '_' || value == '-' {
			encoded.WriteByte(value)
		} else {
			fmt.Fprintf(&encoded, "_x%02x_", value)
		}
	}
	name := encoded.String()
	if len(name) <= openRouterFunctionNameLimit {
		return name
	}
	return name[:46] + fmt.Sprintf("_h%016x", openRouterFNV64(canonical))
}

func openRouterFNV64(value string) uint64 {
	hash := fnv.New64a()
	_, _ = hash.Write([]byte(value))
	return hash.Sum64()
}

func lowerOpenRouterStrictSchema(value any, path string) error {
	object := jsonObject(value)
	if object == nil {
		if array, ok := value.([]any); ok {
			for index, child := range array {
				if err := lowerOpenRouterStrictSchema(child, fmt.Sprintf("%s[%d]", path, index)); err != nil {
					return err
				}
			}
		}
		return nil
	}
	for _, keyword := range []string{
		"allOf", "not", "if", "then", "else", "dependentRequired", "dependentSchemas",
	} {
		if _, exists := object[keyword]; exists {
			return fmt.Errorf("unsupported %s at %s", keyword, path)
		}
	}
	if _, exists := object["uniqueItems"]; exists {
		return fmt.Errorf("unsupported uniqueItems at %s", path)
	}
	if additional, exists := object["additionalProperties"]; exists {
		allowed, boolean := additional.(bool)
		if !boolean || allowed {
			return fmt.Errorf("unsupported additionalProperties at %s", path)
		}
	}
	if pattern := jsonString(object["pattern"]); containsRegexLookaround(pattern) {
		return fmt.Errorf("unsupported pattern at %s", path)
	}
	if oneOf, exists := object["oneOf"]; exists {
		delete(object, "oneOf")
		object["anyOf"] = oneOf
	}

	required := make(map[string]struct{})
	if values, ok := object["required"].([]any); ok {
		for _, value := range values {
			if name, ok := value.(string); ok {
				required[name] = struct{}{}
			}
		}
	}
	if propertiesValue, exists := object["properties"]; exists {
		properties := jsonObject(propertiesValue)
		if properties == nil {
			return fmt.Errorf("properties must be an object at %s", path)
		}
		for name, child := range properties {
			if err := lowerOpenRouterStrictSchema(child, path+".properties."+name); err != nil {
				return err
			}
			if _, exists := required[name]; !exists {
				properties[name] = makeOpenRouterNullable(child)
				required[name] = struct{}{}
			}
		}
		names := make([]string, 0, len(required))
		for name := range required {
			names = append(names, name)
		}
		sort.Strings(names)
		values := make([]any, len(names))
		for index, name := range names {
			values[index] = name
		}
		object["required"] = values
		object["additionalProperties"] = false
	} else if _, exists := object["anyOf"]; exists {
		delete(object, "type")
		delete(object, "additionalProperties")
		delete(object, "required")
	} else if jsonString(object["type"]) == "object" {
		if _, exists := object["required"]; !exists {
			object["required"] = []any{}
		}
		object["additionalProperties"] = false
	}
	for key, child := range object {
		if key == "properties" || key == "required" || key == "additionalProperties" {
			continue
		}
		if err := lowerOpenRouterStrictSchema(child, path+"."+key); err != nil {
			return err
		}
	}
	return nil
}

func makeOpenRouterNullable(value any) any {
	object := jsonObject(value)
	if object == nil {
		return value
	}
	switch kinds := object["type"].(type) {
	case string:
		object["type"] = []any{kinds, "null"}
	case []any:
		for _, kind := range kinds {
			if kind == "null" {
				return value
			}
		}
		object["type"] = append(kinds, "null")
	case nil:
		if variants, ok := object["anyOf"].([]any); ok {
			object["anyOf"] = append(variants, map[string]any{"type": "null"})
			return value
		}
		return map[string]any{"anyOf": []any{object, map[string]any{"type": "null"}}}
	}
	return value
}

func normalizeOpenRouterSchema(value any) {
	if object := jsonObject(value); object != nil {
		if pattern := jsonString(object["pattern"]); containsRegexLookaround(pattern) {
			delete(object, "pattern")
		}
		for _, child := range object {
			normalizeOpenRouterSchema(child)
		}
		return
	}
	if array, ok := value.([]any); ok {
		for _, child := range array {
			normalizeOpenRouterSchema(child)
		}
	}
}

func containsRegexLookaround(pattern string) bool {
	return strings.Contains(pattern, "(?=") || strings.Contains(pattern, "(?!") ||
		strings.Contains(pattern, "(?<=") || strings.Contains(pattern, "(?<!")
}

func restoreOpenRouterOptionalNulls(value any, schema any) {
	instance := jsonObject(value)
	if instance == nil {
		if values, ok := value.([]any); ok {
			if items := jsonObject(schema)["items"]; items != nil {
				for _, child := range values {
					restoreOpenRouterOptionalNulls(child, items)
				}
			}
		}
		return
	}
	rules := matchingOpenRouterObjectRules(schema, instance)
	for name, child := range instance {
		propertyRules := make([]any, 0, len(rules))
		for _, rule := range rules {
			if properties := jsonObject(rule["properties"]); properties != nil {
				if property, exists := properties[name]; exists {
					propertyRules = append(propertyRules, property)
				}
			}
		}
		if len(propertyRules) == 0 {
			continue
		}
		required := false
		for _, rule := range rules {
			if jsonArrayContainsText(rule["required"], name) {
				required = true
				break
			}
		}
		if child == nil && !required {
			acceptsNull := false
			for _, propertyRule := range propertyRules {
				if openRouterSchemaAcceptsNull(propertyRule) {
					acceptsNull = true
					break
				}
			}
			if !acceptsNull {
				delete(instance, name)
				continue
			}
		}
		for _, propertyRule := range propertyRules {
			restoreOpenRouterOptionalNulls(child, propertyRule)
		}
	}
}

func matchingOpenRouterObjectRules(schema any, instance map[string]any) []map[string]any {
	object := jsonObject(schema)
	if object == nil {
		return nil
	}
	var rules []map[string]any
	if _, exists := object["properties"]; exists {
		rules = append(rules, object)
	}
	if branches, ok := object["allOf"].([]any); ok {
		for _, branch := range branches {
			rules = append(rules, matchingOpenRouterObjectRules(branch, instance)...)
		}
	}
	for _, keyword := range []string{"oneOf", "anyOf"} {
		branches, ok := object[keyword].([]any)
		if !ok {
			continue
		}
		var match any
		matches := 0
		for _, branch := range branches {
			if openRouterObjectShapeMatches(branch, instance) {
				match = branch
				matches++
			}
		}
		if matches == 1 {
			rules = append(rules, matchingOpenRouterObjectRules(match, instance)...)
		}
	}
	return rules
}

func openRouterObjectShapeMatches(schema any, instance map[string]any) bool {
	object := jsonObject(schema)
	if object == nil {
		return false
	}
	if required, ok := object["required"].([]any); ok {
		for _, value := range required {
			if name, ok := value.(string); ok {
				if _, exists := instance[name]; !exists {
					return false
				}
			}
		}
	}
	properties := jsonObject(object["properties"])
	for name, ruleValue := range properties {
		value, exists := instance[name]
		if !exists || value == nil {
			continue
		}
		rule := jsonObject(ruleValue)
		if constant, exists := rule["const"]; exists && !jsonValuesEqual(constant, value) {
			return false
		}
		if allowed, ok := rule["enum"].([]any); ok {
			found := false
			for _, candidate := range allowed {
				if jsonValuesEqual(candidate, value) {
					found = true
					break
				}
			}
			if !found {
				return false
			}
		}
	}
	return true
}

func openRouterSchemaAcceptsNull(schema any) bool {
	if allowed, ok := schema.(bool); ok {
		return allowed
	}
	object := jsonObject(schema)
	if object == nil {
		return false
	}
	if constant, exists := object["const"]; exists && constant == nil {
		return true
	}
	if values, ok := object["enum"].([]any); ok {
		for _, value := range values {
			if value == nil {
				return true
			}
		}
	}
	switch kinds := object["type"].(type) {
	case string:
		if kinds == "null" {
			return true
		}
	case []any:
		for _, kind := range kinds {
			if kind == "null" {
				return true
			}
		}
	}
	for _, keyword := range []string{"oneOf", "anyOf"} {
		if variants, ok := object[keyword].([]any); ok {
			for _, variant := range variants {
				if openRouterSchemaAcceptsNull(variant) {
					return true
				}
			}
		}
	}
	return false
}

func openRouterJSONObject(raw json.RawMessage) (map[string]any, error) {
	value, err := decodeOpenRouterJSON(raw)
	if err != nil {
		return nil, err
	}
	object := jsonObject(value)
	if object == nil {
		return nil, errors.New("JSON value is not an object")
	}
	return object, nil
}

func decodeOptionalOpenRouterJSON(raw json.RawMessage) (any, error) {
	if len(raw) == 0 {
		return nil, nil
	}
	return decodeOpenRouterJSON(raw)
}

func decodeOpenRouterJSON(raw []byte) (any, error) {
	if len(bytes.TrimSpace(raw)) == 0 {
		return nil, io.ErrUnexpectedEOF
	}
	return decodeUniqueJSONValue(raw)
}

func cloneOpenRouterJSON(value any) (any, error) {
	encoded, err := json.Marshal(value)
	if err != nil {
		return nil, err
	}
	return decodeOpenRouterJSON(encoded)
}

func jsonObject(value any) map[string]any {
	object, _ := value.(map[string]any)
	return object
}

func jsonString(value any) string {
	text, _ := value.(string)
	return text
}

func firstJSONText(object map[string]any, names ...string) string {
	for _, name := range names {
		if text := jsonString(object[name]); text != "" {
			return text
		}
	}
	return ""
}

func jsonArrayContainsText(value any, expected string) bool {
	values, _ := value.([]any)
	for _, value := range values {
		if value == expected {
			return true
		}
	}
	return false
}

func jsonValuesEqual(left, right any) bool {
	leftJSON, leftErr := json.Marshal(left)
	rightJSON, rightErr := json.Marshal(right)
	return leftErr == nil && rightErr == nil && bytes.Equal(leftJSON, rightJSON)
}

func cloneRawMessages(values []json.RawMessage) []json.RawMessage {
	result := make([]json.RawMessage, len(values))
	for index, value := range values {
		result[index] = append(json.RawMessage(nil), value...)
	}
	return result
}
