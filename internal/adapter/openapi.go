package adapter

import (
	"encoding/json"
	"errors"
	"sort"
	"strings"

	"github.com/goccy/go-yaml"
	"github.com/kpsuperplane/noema/internal/script"
)

const openAPIMaxDepth = 16

var (
	ErrOpenAPIInvalidSource   = errors.New("openapi source is invalid")
	ErrOpenAPIInvalidDocument = errors.New("openapi document is invalid")
	ErrOpenAPIOversized       = errors.New("openapi source is oversized")
	ErrOpenAPIInvalidShape    = errors.New("openapi document shape is invalid")
	ErrOpenAPINotReviewed     = errors.New("openapi candidate is not reviewed")
)

type OpenAPISourceFormat string

const (
	OpenAPIJSON OpenAPISourceFormat = "json"
	OpenAPIYAML OpenAPISourceFormat = "yaml"
)

type OpenAPIDiagnostic struct {
	Code    string
	Message string
}

type OpenAPIOperation struct {
	OperationID       string
	Method            string
	Path              string
	SourceDescription string
	Arguments         []Argument
	FixedHeaders      map[string]string
}

type OpenAPICandidate struct {
	SourceReference string
	Title           string
	Version         string
	SourceFormat    OpenAPISourceFormat
	SourceDigest    string
	Operations      []OpenAPIOperation
	Diagnostics     []OpenAPIDiagnostic
	ReviewClaims    []string
	document        map[string]any
}

type OpenAPISelection struct{ OperationIDs []string }

type OpenAPIActivation struct {
	Compiled     Definition
	SourceDigest string
	OperationIDs []string
}

type OpenAPISemanticChange string

const OpenAPIDocumentationOnly OpenAPISemanticChange = "documentation_only"

func (candidate OpenAPICandidate) HasBlockingDiagnostics() bool {
	return len(candidate.Diagnostics) > 0
}

func (candidate OpenAPICandidate) SelectOperations(ids []string) (OpenAPISelection, error) {
	available := make(map[string]bool, len(candidate.Operations))
	for _, operation := range candidate.Operations {
		available[operation.OperationID] = true
	}
	seen := make(map[string]bool, len(ids))
	for _, id := range ids {
		if !available[id] || seen[id] {
			return OpenAPISelection{}, errors.New("openapi operation selection is invalid")
		}
		seen[id] = true
	}
	return OpenAPISelection{OperationIDs: append([]string(nil), ids...)}, nil
}

func (candidate OpenAPICandidate) Activate(selection OpenAPISelection, manifest Manifest) (OpenAPIActivation, error) {
	if !manifest.Reviewed {
		return OpenAPIActivation{}, ErrOpenAPINotReviewed
	}
	if _, err := Compile(manifest); err != nil {
		return OpenAPIActivation{}, err
	}
	return OpenAPIActivation{Compiled: mustCompile(manifest), SourceDigest: candidate.SourceDigest, OperationIDs: append([]string(nil), selection.OperationIDs...)}, nil
}

func (activation OpenAPIActivation) SemanticChangeFrom(previous OpenAPIActivation) OpenAPISemanticChange {
	if activation.Compiled.SemanticDigest == previous.Compiled.SemanticDigest {
		return OpenAPIDocumentationOnly
	}
	return "semantic"
}

func mustCompile(manifest Manifest) Definition {
	definition, _ := Compile(manifest)
	return definition
}

type OpenAPIImporter struct{}

func (OpenAPIImporter) ImportJSON(reference string, source []byte) (OpenAPICandidate, error) {
	if len(source) == 0 || len(source) > manifestLimit {
		return OpenAPICandidate{}, ErrOpenAPIOversized
	}
	value, err := script.DecodeJSON(source)
	if err != nil {
		return OpenAPICandidate{}, ErrOpenAPIInvalidSource
	}
	object, ok := value.(map[string]any)
	if !ok {
		return OpenAPICandidate{}, ErrOpenAPIInvalidDocument
	}
	return importOpenAPI(reference, source, OpenAPIJSON, object)
}

func (OpenAPIImporter) ImportYAML(reference string, source []byte) (OpenAPICandidate, error) {
	if len(source) == 0 || len(source) > manifestLimit {
		return OpenAPICandidate{}, ErrOpenAPIOversized
	}
	var value any
	if err := yaml.Unmarshal(source, &value); err != nil {
		return OpenAPICandidate{}, ErrOpenAPIInvalidSource
	}
	encoded, err := json.Marshal(value)
	if err != nil {
		return OpenAPICandidate{}, ErrOpenAPIInvalidSource
	}
	var object map[string]any
	if json.Unmarshal(encoded, &object) != nil {
		return OpenAPICandidate{}, ErrOpenAPIInvalidDocument
	}
	return importOpenAPI(reference, source, OpenAPIYAML, object)
}

func importOpenAPI(reference string, source []byte, format OpenAPISourceFormat, document map[string]any) (OpenAPICandidate, error) {
	if depthExceeded(document, 0) {
		return OpenAPICandidate{}, ErrOpenAPIInvalidShape
	}
	version, _ := document["openapi"].(string)
	info, _ := document["info"].(map[string]any)
	title, _ := info["title"].(string)
	if version == "" || title == "" || !strings.HasPrefix(version, "3.") {
		return OpenAPICandidate{}, ErrOpenAPIInvalidDocument
	}
	if strings.HasPrefix(version, "3.1") {
		dialect, _ := document["jsonSchemaDialect"].(string)
		if dialect != "" && dialect != "https://spec.openapis.org/oas/3.1/dialect/base" && dialect != "https://json-schema.org/draft/2020-12/schema" || hasKey(document, "webhooks") || hasUnion(document) || externalRefs(document) {
			return OpenAPICandidate{}, ErrOpenAPIInvalidDocument
		}
	}
	candidate := OpenAPICandidate{SourceReference: reference, Title: boundedOpenAPIText(title), Version: version, SourceFormat: format, SourceDigest: sha256Hex(source), ReviewClaims: []string{"effects", "authentication", "authorization", "behavior", "response"}, document: document}
	if refs := externalRefs(document); refs {
		candidate.Diagnostics = append(candidate.Diagnostics, OpenAPIDiagnostic{Code: "external_ref_unsupported", Message: "external references require review"})
	}
	if servers, ok := document["servers"].([]any); ok {
		for _, server := range servers {
			if object, ok := server.(map[string]any); ok {
				if value, _ := object["url"].(string); strings.Contains(value, "{") {
					candidate.Diagnostics = append(candidate.Diagnostics, OpenAPIDiagnostic{Code: "server_variables_unsupported", Message: "server variables require review"})
				}
			}
		}
	}
	paths, _ := document["paths"].(map[string]any)
	pathNames := make([]string, 0, len(paths))
	for path := range paths {
		pathNames = append(pathNames, path)
	}
	sort.Strings(pathNames)
	for _, path := range pathNames {
		rawPath := paths[path]
		pathObject, _ := rawPath.(map[string]any)
		for _, method := range []string{"get", "post", "put", "patch", "delete"} {
			rawOperation, exists := pathObject[method]
			if !exists {
				continue
			}
			operation, ok := rawOperation.(map[string]any)
			if !ok {
				return OpenAPICandidate{}, ErrOpenAPIInvalidDocument
			}
			id, _ := operation["operationId"].(string)
			id = normalizedOpenAPIOperationID(id, method, path)
			if id == "" {
				return OpenAPICandidate{}, ErrOpenAPIInvalidDocument
			}
			description, _ := operation["summary"].(string)
			candidate.Operations = append(candidate.Operations, OpenAPIOperation{OperationID: id, Method: strings.ToUpper(method), Path: path, SourceDescription: boundedOpenAPIText(description), Arguments: openAPIArguments(document, pathObject, operation)})
			if _, ok := operation["callbacks"]; ok {
				candidate.Diagnostics = append(candidate.Diagnostics, OpenAPIDiagnostic{Code: "callbacks_unsupported", Message: "callbacks require review"})
			}
			if body, ok := operation["requestBody"].(map[string]any); ok {
				content, _ := body["content"].(map[string]any)
				for media := range content {
					if strings.HasPrefix(media, "multipart/") {
						candidate.Diagnostics = append(candidate.Diagnostics, OpenAPIDiagnostic{Code: "request_media_type_unsupported", Message: "multipart requests require review"})
					}
				}
			}
		}
	}
	return candidate, nil
}

func boundedOpenAPIText(value string) string {
	value = strings.Map(func(r rune) rune {
		if r < ' ' {
			return -1
		}
		return r
	}, value)
	if len(value) > 4096 {
		return value[:4096]
	}
	return value
}

func hasKey(value map[string]any, name string) bool { _, ok := value[name]; return ok }

func depthExceeded(value any, depth int) bool {
	if depth > openAPIMaxDepth {
		return true
	}
	switch value := value.(type) {
	case map[string]any:
		for _, child := range value {
			if depthExceeded(child, depth+1) {
				return true
			}
		}
	case []any:
		for _, child := range value {
			if depthExceeded(child, depth+1) {
				return true
			}
		}
	}
	return false
}

func externalRefs(value any) bool {
	var found bool
	var walk func(any)
	walk = func(current any) {
		if found {
			return
		}
		switch current := current.(type) {
		case map[string]any:
			if ref, _ := current["$ref"].(string); strings.HasPrefix(ref, "http://") || strings.HasPrefix(ref, "https://") {
				found = true
				return
			}
			for _, child := range current {
				walk(child)
			}
		case []any:
			for _, child := range current {
				walk(child)
			}
		}
	}
	walk(value)
	return found
}

func hasUnion(value any) bool {
	found := false
	var walk func(any)
	walk = func(current any) {
		if found {
			return
		}
		if object, ok := current.(map[string]any); ok {
			if _, ok := object["type"].([]any); ok {
				found = true
				return
			}
			for _, child := range object {
				walk(child)
			}
		} else if array, ok := current.([]any); ok {
			for _, child := range array {
				walk(child)
			}
		}
	}
	walk(value)
	return found
}

func openAPIArguments(document map[string]any, pathObject, operation map[string]any) []Argument {
	result := []Argument{}
	parameters := append([]any{}, asArray(pathObject["parameters"])...)
	parameters = append(parameters, asArray(operation["parameters"])...)
	for _, raw := range parameters {
		parameter, ok := resolveOpenAPIRef(document, raw, "parameters")
		if !ok {
			continue
		}
		name, _ := parameter["name"].(string)
		location, _ := parameter["in"].(string)
		required, _ := parameter["required"].(bool)
		if location != "path" && location != "query" {
			continue
		}
		if location == "path" {
			required = true
		}
		schema, _ := resolveOpenAPIRef(document, parameter["schema"], "schemas")
		typ, _ := schema["type"].(string)
		if typ == "" {
			typ = "string"
		}
		result = append(result, Argument{Name: name, Description: "Supply the documented value.", Location: location, Type: typ, Required: required})
	}
	if body, ok := operation["requestBody"].(map[string]any); ok {
		body, _ = resolveOpenAPIRef(document, body, "requestBodies")
		content, _ := body["content"].(map[string]any)
		if raw, ok := content["application/json"].(map[string]any); ok {
			schema, _ := resolveOpenAPIRef(document, raw["schema"], "schemas")
			properties, _ := schema["properties"].(map[string]any)
			required := map[string]bool{}
			for _, value := range asArray(schema["required"]) {
				if name, ok := value.(string); ok {
					required[name] = true
				}
			}
			names := make([]string, 0, len(properties))
			for name := range properties {
				names = append(names, name)
			}
			sort.Strings(names)
			for _, name := range names {
				rawProperty := properties[name]
				property, _ := resolveOpenAPIRef(document, rawProperty, "schemas")
				typ, _ := property["type"].(string)
				if typ == "" {
					typ = "string"
				}
				result = append(result, Argument{Name: name, Description: "Supply the documented value.", Location: "json_body", Type: typ, Required: required[name]})
			}
		}
	}
	sort.Slice(result, func(i, j int) bool {
		if result[i].Name == result[j].Name {
			return result[i].Location < result[j].Location
		}
		return result[i].Name < result[j].Name
	})
	return result
}

func normalizedOpenAPIOperationID(source, method, path string) string {
	if source == "" {
		source = method + "_" + path
	}
	var builder strings.Builder
	for _, character := range source {
		if character >= 'a' && character <= 'z' || character >= 'A' && character <= 'Z' || character >= '0' && character <= '9' || strings.ContainsRune("_-.:", character) {
			builder.WriteRune(character)
		} else {
			builder.WriteByte('_')
		}
		if builder.Len() >= 96 {
			break
		}
	}
	return strings.Trim(builder.String(), "_")
}

func asArray(value any) []any { array, _ := value.([]any); return array }

func resolveOpenAPIRef(document map[string]any, raw any, component string) (map[string]any, bool) {
	object, ok := raw.(map[string]any)
	if !ok {
		return nil, false
	}
	ref, _ := object["$ref"].(string)
	prefix := "#/components/" + component + "/"
	if !strings.HasPrefix(ref, prefix) {
		return object, true
	}
	components, _ := document["components"].(map[string]any)
	values, _ := components[component].(map[string]any)
	resolved, ok := values[strings.TrimPrefix(ref, prefix)].(map[string]any)
	return resolved, ok
}
