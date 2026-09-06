package adapter

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"net/url"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/script"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	manifestLimit     = 1 << 20
	modelResultLimit  = 32 << 10
	argumentLimit     = 256 << 10
	maximumOperations = 256
	maximumArguments  = 128
)

func CompileJSON(raw []byte) (Definition, error) {
	if len(raw) == 0 || len(raw) > manifestLimit {
		return Definition{}, errors.New("adapter manifest is invalid")
	}
	var manifest Manifest
	if err := decodeExactJSON(raw, &manifest); err != nil {
		return Definition{}, errors.New("adapter manifest is invalid")
	}
	return Compile(manifest)
}

func Compile(manifest Manifest) (Definition, error) {
	if err := validateManifest(&manifest); err != nil {
		return Definition{}, err
	}
	digest, err := manifestDigest(manifest)
	if err != nil {
		return Definition{}, err
	}
	operations := make([]CompiledOperation, len(manifest.Operations))
	for index, operation := range manifest.Operations {
		encoded, _ := normalizedJSON(operation)
		opDigest := sha256Hex(append([]byte(digest+":"), encoded...))
		behavior := storeBehavior(operation.Behavior)
		schema, _ := json.Marshal(inputSchema(operation))
		operations[index] = CompiledOperation{Operation: operation, Digest: opDigest, InputSchema: schema, Behavior: behavior}
	}
	sort.Slice(operations, func(i, j int) bool { return operations[i].OperationID < operations[j].OperationID })
	return Definition{Manifest: manifest, SemanticDigest: digest, Operations: operations}, nil
}

func validateManifest(manifest *Manifest) error {
	if manifest.SchemaVersion != 9 || validateAuthentication(manifest.Authentication) != nil {
		return errors.New("adapter authentication is invalid")
	}
	if !validID(manifest.DefinitionID) || !validID(manifest.AdapterID) || !validID(manifest.DefinitionRevision) {
		return errors.New("adapter identity is invalid")
	}
	if manifest.DisplayName != "" && !boundedText(manifest.DisplayName, 256, false) {
		return errors.New("adapter display name is invalid")
	}
	parsed, err := url.Parse(manifest.Origin)
	if err != nil || parsed.Scheme != "https" || parsed.Hostname() == "" || parsed.User != nil ||
		parsed.Path != "/" || parsed.RawQuery != "" || parsed.Fragment != "" || strings.ContainsAny(manifest.Origin, "{}") {
		return errors.New("adapter origin is invalid")
	}
	if len(manifest.Operations) == 0 || len(manifest.Operations) > maximumOperations {
		return errors.New("adapter operations are invalid")
	}
	seen := make(map[string]bool, len(manifest.Operations))
	for index := range manifest.Operations {
		operation := &manifest.Operations[index]
		if seen[operation.OperationID] {
			return fmt.Errorf("operations[%d].operation_id is duplicated", index)
		}
		if err := validateOperation(operation); err != nil {
			return fmt.Errorf("operations[%d]: %w", index, err)
		}
		if (manifest.Authentication.Kind == "oauth2_authorization_code_pkce") != (operation.Authorization.Kind == "oauth_scopes") {
			return errors.New("adapter operation authorization is invalid")
		}
		seen[operation.OperationID] = true
	}
	return nil
}

func sortedUniqueScopes(values []string) bool {
	for i := range values {
		if i > 0 && values[i-1] >= values[i] {
			return false
		}
	}
	return true
}
func scopeSubset(candidate, required []string) bool {
	wanted := make(map[string]bool, len(candidate))
	for _, value := range candidate {
		wanted[value] = true
	}
	for _, value := range required {
		if !wanted[value] {
			return false
		}
	}
	return true
}

func validateAuthentication(value Authentication) error {
	if value.Kind == "none" {
		if value.Setup != nil || value.RequestAuth != nil {
			return errors.New("adapter authentication is invalid")
		}
		return nil
	}
	if value.Kind == "oauth2_authorization_code_pkce" {
		if !validDigest(value.ProfileDigest) || value.Setup != nil || value.RequestAuth != nil {
			return errors.New("adapter authentication is invalid")
		}
		return nil
	}
	if value.Kind != "credential" || value.Setup == nil || value.RequestAuth == nil || !validTransform(value.RequestAuth) {
		return errors.New("adapter authentication is invalid")
	}
	setup := value.Setup
	parsed, err := url.Parse(setup.SetupURL)
	if !boundedText(setup.CredentialType, 128, false) || len(setup.SetupURL) > 4096 || strings.TrimSpace(setup.SetupURL) != setup.SetupURL || err != nil || parsed.Scheme != "https" || parsed.Hostname() == "" || parsed.User != nil || parsed.RawQuery != "" || parsed.Fragment != "" || len(setup.Instructions) < 1 || len(setup.Instructions) > 8 {
		return errors.New("adapter credential setup is invalid")
	}
	for _, instruction := range setup.Instructions {
		if !boundedText(instruction, 512, false) {
			return errors.New("adapter credential setup is invalid")
		}
	}
	input := setup.Input
	if len(input.Fields) < 1 || len(input.Fields) > 16 {
		return errors.New("adapter credential setup is invalid")
	}
	seen := map[string]bool{}
	for _, field := range input.Fields {
		if !validID(field.ID) || seen[field.ID] || !boundedText(field.Label, 128, false) {
			return errors.New("adapter credential field is invalid")
		}
		seen[field.ID] = true
	}
	if input.Kind == "fields" {
		if input.MediaType != "" || input.Normalize != nil {
			return errors.New("adapter credential input is invalid")
		}
		return nil
	}
	if input.Kind != "document" || input.MediaType != "application/json" || !validTransform(input.Normalize) {
		return errors.New("adapter credential input is invalid")
	}
	return nil
}

func validTransform(value *Transform) bool {
	return value != nil && value.Language == "lua" && len(value.Source) > 0 && len(value.Source) <= 32<<10 &&
		strings.IndexFunc(value.Source, func(r rune) bool { return r < ' ' && r != '\n' && r != '\t' }) < 0 && script.ValidateFunction(value.Source) == nil
}

func validateOperation(operation *Operation) error {
	if !validID(operation.OperationID) || !boundedText(operation.Description, 1024, false) ||
		operation.SourceDescription != "" && !boundedText(operation.SourceDescription, 4096, false) {
		return errors.New("operation metadata is invalid")
	}
	switch operation.Method {
	case "GET", "POST", "PUT", "PATCH", "DELETE":
	default:
		return errors.New("operation method is invalid")
	}
	lowerPath := strings.ToLower(operation.Path)
	if len(operation.Path) > 2048 || !strings.HasPrefix(operation.Path, "/") || strings.HasPrefix(operation.Path, "//") ||
		strings.Contains(operation.Path, "://") || strings.ContainsAny(operation.Path, "?#\\") ||
		strings.Contains(lowerPath, "%2e") || strings.Contains(lowerPath, "%2f") || strings.Contains(lowerPath, "%5c") {
		return errors.New("operation path is invalid")
	}
	for _, part := range strings.Split(operation.Path, "/") {
		if part == "." || part == ".." || strings.IndexFunc(part, func(r rune) bool { return r < ' ' }) >= 0 {
			return errors.New("operation path is invalid")
		}
	}
	if operation.Authorization.Kind == "oauth_scopes" {
		if len(operation.Authorization.AcceptedScopeSets) < 1 || len(operation.Authorization.AcceptedScopeSets) > 16 {
			return errors.New("operation authorization is invalid")
		}
		for i, scopes := range operation.Authorization.AcceptedScopeSets {
			if len(scopes) < 1 || len(scopes) > 32 || !sortedUniqueScopes(scopes) {
				return errors.New("operation authorization is invalid")
			}
			for _, scope := range scopes {
				if !boundedText(scope, 512, false) {
					return errors.New("operation authorization is invalid")
				}
			}
			for _, other := range operation.Authorization.AcceptedScopeSets[:i] {
				if scopeSubset(scopes, other) || scopeSubset(other, scopes) {
					return errors.New("operation authorization is ambiguous")
				}
			}
		}
	} else if operation.Authorization.Kind != "none" || len(operation.Authorization.AcceptedScopeSets) != 0 {
		return errors.New("operation authorization is invalid")
	}
	if len(operation.Arguments) > maximumArguments || validateArguments(operation) != nil || validateFixedValues(operation) != nil {
		return errors.New("operation arguments are invalid")
	}
	values := []*Hint{&operation.Behavior.ReadOnly, &operation.Behavior.Idempotent, &operation.Behavior.Destructive, &operation.Behavior.OpenWorld}
	for _, hint := range values {
		if hint.Value == nil || hint.Source == nil || *hint.Source != "model" && *hint.Source != "safe_default" {
			return errors.New("operation behavior is invalid")
		}
	}
	if operation.Retry != "never" && operation.Retry != "transport_safe_read" ||
		operation.Retry == "transport_safe_read" && (operation.Method != "GET" || !*operation.Behavior.Idempotent.Value) {
		return errors.New("operation retry is invalid")
	}
	if err := validatePagination(operation); err != nil {
		return err
	}
	if err := validateResponse(&operation.Response, operation.Pagination.Kind == "response_token"); err != nil {
		return err
	}
	return nil
}

func validateArguments(operation *Operation) error {
	seen := make(map[string]bool)
	pathNames := make(map[string]bool)
	for _, argument := range operation.Arguments {
		if !validID(argument.Name) || seen[argument.Name] || !boundedText(argument.Description, 1024, true) {
			return errors.New("argument is invalid")
		}
		seen[argument.Name] = true
		switch argument.Location {
		case "path":
			if !argument.Required {
				return errors.New("path argument is optional")
			}
			pathNames[argument.Name] = true
		case "query", "json_body":
		default:
			return errors.New("argument location is invalid")
		}
		switch argument.Type {
		case "string":
		case "integer", "number", "boolean", "string_array":
			if len(argument.EnumValues) != 0 {
				return errors.New("argument enum is invalid")
			}
		default:
			return errors.New("argument type is invalid")
		}
		if len(argument.EnumValues) > 128 {
			return errors.New("argument enum is invalid")
		}
		for _, value := range argument.EnumValues {
			if !boundedText(value, 256, false) {
				return errors.New("argument enum is invalid")
			}
		}
	}
	placeholders := make(map[string]bool)
	rest := operation.Path
	for {
		start := strings.IndexByte(rest, '{')
		if start < 0 {
			break
		}
		end := strings.IndexByte(rest[start+1:], '}')
		if end < 0 {
			return errors.New("path placeholder is invalid")
		}
		name := rest[start+1 : start+1+end]
		if !validID(name) || placeholders[name] {
			return errors.New("path placeholder is invalid")
		}
		placeholders[name] = true
		rest = rest[start+end+2:]
	}
	if strings.ContainsRune(rest, '}') || len(placeholders) != len(pathNames) {
		return errors.New("path placeholder is invalid")
	}
	for name := range placeholders {
		if !pathNames[name] {
			return errors.New("path placeholder is invalid")
		}
	}
	return validateBodyTemplate(operation)
}

func validateBodyTemplate(operation *Operation) error {
	if operation.JSONBodyTemplate == nil {
		return nil
	}
	raw, err := json.Marshal(operation.JSONBodyTemplate)
	if err != nil || len(raw) > argumentLimit {
		return errors.New("body template is invalid")
	}
	root, ok := operation.JSONBodyTemplate.(map[string]any)
	if !ok {
		return errors.New("body template is invalid")
	}
	body := make(map[string]bool)
	for _, argument := range operation.Arguments {
		if argument.Location == "json_body" {
			body[argument.Name] = false
		}
	}
	nodes := 0
	var visit func(any, int) error
	visit = func(value any, depth int) error {
		nodes++
		if depth > 12 || nodes > 256 {
			return errors.New("body template is too complex")
		}
		switch value := value.(type) {
		case map[string]any:
			if name, exists := value["$argument"]; exists {
				text, ok := name.(string)
				used, declared := body[text]
				if len(value) != 1 || !ok || !declared || used {
					return errors.New("body placeholder is invalid")
				}
				body[text] = true
				return nil
			}
			for _, child := range value {
				if err := visit(child, depth+1); err != nil {
					return err
				}
			}
		case []any:
			for _, child := range value {
				if err := visit(child, depth+1); err != nil {
					return err
				}
			}
		}
		return nil
	}
	if err := visit(root, 0); err != nil {
		return err
	}
	for _, found := range body {
		if !found {
			return errors.New("body argument is unused")
		}
	}
	return nil
}

func validateFixedValues(operation *Operation) error {
	if len(operation.FixedHeaders) > 32 || len(operation.FixedQuery) > 32 {
		return errors.New("fixed request values are invalid")
	}
	headers := make(map[string]bool)
	for name, value := range operation.FixedHeaders {
		lower := strings.ToLower(name)
		if !validHeaderName(name) || headers[lower] || len(value) > 4096 || strings.ContainsAny(value, "\r\n") ||
			(lower != "accept" && lower != "content-type") || value != "application/json" {
			return errors.New("fixed header is invalid")
		}
		headers[lower] = true
	}
	queryArguments := make(map[string]bool)
	for _, argument := range operation.Arguments {
		if argument.Location == "query" {
			queryArguments[argument.Name] = true
		}
	}
	for name, value := range operation.FixedQuery {
		if !validID(name) || queryArguments[name] || len(value) > 4096 || strings.IndexFunc(value, func(r rune) bool { return r < ' ' }) >= 0 {
			return errors.New("fixed query is invalid")
		}
	}
	return nil
}

func validatePagination(operation *Operation) error {
	pagination := operation.Pagination
	if pagination.Kind == "none" {
		if pagination.ResponsePointer != "" || pagination.RequestArgument != "" || pagination.PageSize != nil {
			return errors.New("pagination is invalid")
		}
		return nil
	}
	if pagination.Kind != "response_token" || !validPointer(pagination.ResponsePointer) || !validID(pagination.RequestArgument) {
		return errors.New("pagination is invalid")
	}
	if operation.FixedQuery[pagination.RequestArgument] != "" {
		return errors.New("pagination query conflicts")
	}
	if pagination.PageSize != nil {
		if !validID(pagination.PageSize.RequestArgument) || pagination.PageSize.Value < 1 || pagination.PageSize.Value > 1000 ||
			pagination.PageSize.RequestArgument == pagination.RequestArgument || operation.FixedQuery[pagination.PageSize.RequestArgument] != "" {
			return errors.New("pagination page size is invalid")
		}
	}
	return nil
}

func validateResponse(response *Response, paginated bool) error {
	if len(response.AcceptedContentTypes) == 0 || len(response.AcceptedContentTypes) > 16 {
		return errors.New("response content types are invalid")
	}
	seen := make(map[string]bool)
	for _, media := range response.AcceptedContentTypes {
		parts := strings.Split(media, "/")
		if len(parts) != 2 || media != strings.ToLower(media) || len(media) > 128 || strings.ContainsAny(media, "*;") || seen[media] {
			return errors.New("response content type is invalid")
		}
		seen[media] = true
		if response.Transform == nil && media != "application/json" && !strings.HasSuffix(media, "+json") {
			return errors.New("response transform is required")
		}
	}
	if response.Transform != nil && !validTransform(response.Transform) {
		return errors.New("response transform is invalid")
	}
	if !validateOutputSchema(response.OutputSchema, 0, new(int)) {
		return errors.New("response schema is invalid")
	}
	maximum, ok := maximumOutputBytes(response.OutputSchema)
	if !ok || maximum > modelResultLimit {
		return fmt.Errorf("response.output_schema permits %d bytes; limit is %d bytes (finite bound: %t). Reduce maxItems or maxBytes and enforce the same bounds in the transform", maximum, modelResultLimit, ok)
	}
	_, reserved := response.OutputSchema.Properties["continuation"]
	if reserved || paginated && (response.Transform == nil || response.OutputSchema.Type != "object" || maximum > modelResultLimit-320) {
		return errors.New("paginated response is invalid")
	}
	return nil
}

func validateOutputSchema(schema OutputSchema, depth int, nodes *int) bool {
	*nodes++
	if depth > 16 || *nodes > 512 {
		return false
	}
	objectFields := len(schema.Properties) != 0 || len(schema.Required) != 0 || schema.AdditionalProperties != nil
	switch schema.Type {
	case "object":
		if schema.AdditionalProperties == nil || *schema.AdditionalProperties || schema.Items != nil || schema.MaxBytes != nil || schema.MaxItems != nil {
			return false
		}
		required := make(map[string]bool)
		for _, name := range schema.Required {
			_, declared := schema.Properties[name]
			if required[name] || !declared || !validOutputName(name) {
				return false
			}
			required[name] = true
		}
		for name, child := range schema.Properties {
			if !validOutputName(name) || !validateOutputSchema(child, depth+1, nodes) {
				return false
			}
		}
		return true
	case "array":
		return !objectFields && schema.Items != nil && schema.MaxItems != nil && *schema.MaxItems >= 0 && validateOutputSchema(*schema.Items, depth+1, nodes)
	case "string":
		return !objectFields && schema.Items == nil && schema.MaxBytes != nil && *schema.MaxBytes >= 0 && schema.MaxItems == nil
	case "integer", "number", "boolean", "null":
		return !objectFields && schema.Items == nil && schema.MaxBytes == nil && schema.MaxItems == nil
	}
	return false
}

func maximumOutputBytes(schema OutputSchema) (int, bool) {
	switch schema.Type {
	case "object":
		total := 2
		for name, child := range schema.Properties {
			size, ok := maximumOutputBytes(child)
			if !ok {
				return 0, false
			}
			total += len(name)*6 + 4 + size
		}
		return total, total >= 0
	case "array":
		if schema.Items == nil || schema.MaxItems == nil {
			return 0, false
		}
		size, ok := maximumOutputBytes(*schema.Items)
		if !ok || *schema.MaxItems < 0 || size+1 > 0 && *schema.MaxItems > (math.MaxInt-2)/(size+1) {
			return 0, false
		}
		return 2 + *schema.MaxItems*(size+1), true
	case "string":
		if schema.MaxBytes == nil || *schema.MaxBytes > math.MaxInt/6 {
			return 0, false
		}
		return *schema.MaxBytes*6 + 2, true
	case "integer":
		return 20, true
	case "number":
		return 24, true
	case "boolean":
		return 5, true
	case "null":
		return 4, true
	}
	return 0, false
}

func matchesOutput(schema OutputSchema, value any) bool {
	switch schema.Type {
	case "object":
		object, ok := value.(map[string]any)
		if !ok {
			return false
		}
		for name := range object {
			if _, ok := schema.Properties[name]; !ok {
				return false
			}
		}
		for _, name := range schema.Required {
			if _, ok := object[name]; !ok {
				return false
			}
		}
		for name, child := range object {
			if !matchesOutput(schema.Properties[name], child) {
				return false
			}
		}
		return true
	case "array":
		array, ok := value.([]any)
		if !ok || schema.MaxItems == nil || len(array) > *schema.MaxItems || schema.Items == nil {
			return false
		}
		for _, child := range array {
			if !matchesOutput(*schema.Items, child) {
				return false
			}
		}
		return true
	case "string":
		text, ok := value.(string)
		return ok && schema.MaxBytes != nil && len(text) <= *schema.MaxBytes
	case "boolean":
		_, ok := value.(bool)
		return ok
	case "integer":
		switch number := value.(type) {
		case int, int64:
			return true
		case json.Number:
			_, err := number.Int64()
			return err == nil
		}
		return false
	case "number":
		switch value.(type) {
		case int, int64, float64, json.Number:
			return true
		}
		return false
	case "null":
		return value == nil
	}
	return false
}

func inputSchema(operation Operation) map[string]any {
	properties := make(map[string]any)
	required := make([]string, 0)
	for _, argument := range operation.Arguments {
		schema := map[string]any{"description": argument.Description}
		switch argument.Type {
		case "string":
			schema["type"] = "string"
		case "integer":
			schema["type"] = "integer"
		case "number":
			schema["type"] = "number"
		case "boolean":
			schema["type"] = "boolean"
		case "string_array":
			schema["type"], schema["items"] = "array", map[string]any{"type": "string"}
		}
		if len(argument.EnumValues) != 0 {
			schema["enum"] = argument.EnumValues
		}
		properties[argument.Name] = schema
		if argument.Required {
			required = append(required, argument.Name)
		}
	}
	if operation.Pagination.Kind == "response_token" {
		properties["continuation"] = map[string]any{"type": "string", "maxLength": 128}
	}
	sort.Strings(required)
	return map[string]any{"type": "object", "properties": properties, "required": required, "additionalProperties": false}
}

func manifestDigest(manifest Manifest) (string, error) {
	manifest.DisplayName = ""
	for i := range manifest.Operations {
		manifest.Operations[i].SourceDescription = ""
		sort.Strings(manifest.Operations[i].Response.AcceptedContentTypes)
		sort.Strings(manifest.Operations[i].Response.OutputSchema.Required)
		sort.Slice(manifest.Operations[i].Arguments, func(a, b int) bool {
			return manifest.Operations[i].Arguments[a].Name < manifest.Operations[i].Arguments[b].Name
		})
		for j := range manifest.Operations[i].Arguments {
			sort.Strings(manifest.Operations[i].Arguments[j].EnumValues)
		}
	}
	sort.Slice(manifest.Operations, func(i, j int) bool { return manifest.Operations[i].OperationID < manifest.Operations[j].OperationID })
	raw, err := normalizedJSON(manifest)
	if err != nil {
		return "", err
	}
	return sha256Hex(raw), nil
}

func normalizedJSON(value any) ([]byte, error) { return json.Marshal(value) }
func sha256Hex(raw []byte) string              { sum := sha256.Sum256(raw); return hex.EncodeToString(sum[:]) }
func validID(value string) bool {
	if value == "" || len(value) > 96 || strings.TrimSpace(value) != value {
		return false
	}
	for _, r := range value {
		if !(r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9' || strings.ContainsRune("_-.:", r)) {
			return false
		}
	}
	return true
}
func boundedText(value string, limit int, empty bool) bool {
	return (empty || value != "") && len(value) <= limit && strings.TrimSpace(value) == value && utf8.ValidString(value) && strings.IndexFunc(value, func(r rune) bool { return r < ' ' }) < 0
}
func validHeaderName(value string) bool {
	if value == "" || len(value) > 128 {
		return false
	}
	for _, r := range value {
		if !(r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9' || r == '-') {
			return false
		}
	}
	return true
}
func validOutputName(value string) bool {
	return value != "" && len(value) <= 128 && strings.IndexFunc(value, func(r rune) bool { return r < ' ' }) < 0
}
func validPointer(value string) bool {
	return strings.HasPrefix(value, "/") && len(value) <= 2048 && !strings.Contains(value, "~2")
}
func storeBehavior(value BehaviorHints) store.ActionBehavior {
	return store.ActionBehavior{ReadOnly: *value.ReadOnly.Value, RepeatSafe: *value.Idempotent.Value, Destructive: *value.Destructive.Value, OpenWorld: *value.OpenWorld.Value}
}

func decodeExactJSON(raw []byte, target any) error {
	if !utf8.Valid(raw) {
		return errors.New("JSON is not UTF-8")
	}
	value, err := script.DecodeJSON(raw)
	if err != nil {
		return err
	}
	normalized, err := script.MarshalJSON(value)
	if err != nil {
		return err
	}
	decoder := json.NewDecoder(bytes.NewReader(normalized))
	decoder.DisallowUnknownFields()
	decoder.UseNumber()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	if err := decoder.Decode(new(any)); err != io.EOF {
		return fmt.Errorf("JSON has trailing content")
	}
	return nil
}
