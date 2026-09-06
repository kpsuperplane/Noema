package runtime

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	presentA2UIName     = "noema.present_a2ui"
	a2uiVersion         = "v0.9.1"
	a2uiCatalogID       = "com.noema.a2ui/catalog/v0.9.1"
	a2uiInputLimit      = 256 * 1024
	a2uiActionJSONLimit = 64 * 1024
	a2uiStringLimit     = 8 * 1024
	a2uiCollectionLimit = 256
	a2uiDepthLimit      = 32
	a2uiMessageLimit    = 128
)

var (
	presentA2UISchema = json.RawMessage(`{"type":"object","properties":{"jsonl":{"type":"string","minLength":1}},"required":["jsonl"],"additionalProperties":false}`)
	a2uiComponents    = []string{"Text", "Row", "Column", "Card", "Divider", "Button", "TextField", "CheckBox", "ChoicePicker"}
)

type a2uiAction struct {
	SourceComponentID string `json:"source_component_id"`
	Name              string `json:"name"`
	Context           any    `json:"context,omitempty"`
}

type a2uiSurface struct {
	SurfaceID           string         `json:"surface_id"`
	NamespacedSurfaceID string         `json:"namespaced_surface_id"`
	Version             string         `json:"version"`
	CatalogID           string         `json:"catalog_id"`
	SendDataModel       bool           `json:"send_data_model"`
	Revision            int            `json:"revision"`
	Components          map[string]any `json:"components"`
	DataModel           any            `json:"data_model"`
	Actions             []a2uiAction   `json:"actions"`
}

type a2uiBatch struct {
	Messages          []any                   `json:"messages"`
	Surfaces          map[string]*a2uiSurface `json:"surfaces"`
	DeletedSurfaceIDs []string                `json:"deleted_surface_ids"`
}

type a2uiRepair struct {
	Status  string `json:"status"`
	Code    string `json:"code"`
	Line    int    `json:"line,omitempty"`
	Path    string `json:"path,omitempty"`
	Message string `json:"message"`
}

func a2uiError(code string, line int, path, message string) *a2uiRepair {
	return &a2uiRepair{Status: "VALIDATION_FAILED", Code: code, Line: line, Path: path, Message: message}
}

func presentA2UITool() provider.GenerationTool {
	return provider.GenerationTool{Name: presentA2UIName,
		Description: "Present one or more A2UI v0.9.1 messages encoded as JSONL to the human.",
		InputSchema: append(json.RawMessage(nil), presentA2UISchema...)}
}

func parseA2UIArguments(raw json.RawMessage) (string, error) {
	if !utf8.Valid(raw) || len(raw) > 6*a2uiInputLimit+1024 {
		return "", errors.New("A2UI arguments are invalid")
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var value struct {
		JSONL string `json:"jsonl"`
	}
	if decoder.Decode(&value) != nil {
		return "", errors.New("A2UI arguments are invalid")
	}
	var trailing any
	if decoder.Decode(&trailing) != io.EOF {
		return "", errors.New("A2UI arguments are invalid")
	}
	value.JSONL = strings.TrimSpace(value.JSONL)
	if value.JSONL == "" {
		return "", errors.New("A2UI arguments are invalid")
	}
	return value.JSONL, nil
}

func reduceA2UI(conversationID, jsonl string) (*a2uiBatch, *a2uiRepair) {
	if strings.TrimSpace(conversationID) == "" {
		return nil, a2uiError("invalid_protocol", 0, "", "empty namespace")
	}
	if len(jsonl) > a2uiInputLimit {
		return nil, a2uiError("bounds_exceeded", 0, "", "input bound")
	}
	lines := strings.Split(jsonl, "\n")
	if len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	if len(lines) == 0 {
		return nil, a2uiError("invalid_jsonl", 0, "", "empty JSONL")
	}
	batch := &a2uiBatch{Messages: make([]any, 0, len(lines)), Surfaces: make(map[string]*a2uiSurface)}
	deleted := make(map[string]bool)
	for index, raw := range lines {
		line := index + 1
		if strings.TrimSpace(raw) == "" || len(batch.Messages) >= a2uiMessageLimit {
			return nil, a2uiError("invalid_jsonl", line, "", "invalid JSONL line")
		}
		raw = strings.TrimSuffix(raw, "\r")
		if !utf8.ValidString(raw) {
			return nil, a2uiError("invalid_jsonl", line, "", "invalid JSON")
		}
		var message any
		decoder := json.NewDecoder(strings.NewReader(raw))
		if decoder.Decode(&message) != nil {
			return nil, a2uiError("invalid_jsonl", line, "", "invalid JSON")
		}
		var extra any
		if decoder.Decode(&extra) != io.EOF {
			return nil, a2uiError("invalid_jsonl", line, "", "invalid JSON")
		}
		if failure := validateA2UIBounds(message, 0, line); failure != nil {
			return nil, failure
		}
		if failure := validateA2UISafety(message, line); failure != nil {
			return nil, failure
		}
		if failure := applyA2UIMessage(batch, deleted, conversationID, message, line); failure != nil {
			return nil, failure
		}
		batch.Messages = append(batch.Messages, message)
	}
	for _, surface := range batch.Surfaces {
		if failure := validateA2UIIntegrity(surface, 0, true); failure != nil {
			return nil, failure
		}
		refreshA2UIActions(surface)
		if len(surface.Actions) != 0 {
			if failure := validateA2UIBindings(surface); failure != nil {
				return nil, failure
			}
		}
	}
	for id := range deleted {
		batch.DeletedSurfaceIDs = append(batch.DeletedSurfaceIDs, id)
	}
	sort.Strings(batch.DeletedSurfaceIDs)
	return batch, nil
}

func applyA2UIMessage(batch *a2uiBatch, deleted map[string]bool, conversationID string, raw any, line int) *a2uiRepair {
	message, ok := raw.(map[string]any)
	if !ok {
		return a2uiError("invalid_jsonl", line, "", "expected object")
	}
	if message["version"] != a2uiVersion {
		return a2uiError("unsupported_version", line, "version", "unsupported version")
	}
	names := []string{"createSurface", "updateComponents", "updateDataModel", "deleteSurface"}
	found := ""
	for key := range message {
		if key == "version" {
			continue
		}
		if !stringIn(key, names) || found != "" {
			return a2uiError("invalid_protocol", line, "", "invalid message")
		}
		found = key
	}
	if found == "" || len(message) != 2 {
		return a2uiError("invalid_protocol", line, "", "invalid message")
	}
	body, ok := message[found].(map[string]any)
	if !ok {
		return a2uiError("invalid_protocol", line, "", "expected object")
	}
	surfaceID, failure := a2uiIdentifier(body["surfaceId"], line)
	if failure != nil {
		return failure
	}
	key := "a2ui/" + escapeA2UISegment(conversationID) + "/" + escapeA2UISegment(surfaceID)
	switch found {
	case "createSurface":
		if failure = onlyA2UIFields(body, line, "surfaceId", "catalogId", "sendDataModel"); failure != nil {
			return failure
		}
		if body["catalogId"] != a2uiCatalogID {
			return a2uiError("unsupported_catalog", line, "catalogId", "unsupported catalog")
		}
		send := false
		if value, exists := body["sendDataModel"]; exists {
			var valid bool
			send, valid = value.(bool)
			if !valid {
				return a2uiError("invalid_protocol", line, "sendDataModel", "expected boolean")
			}
		}
		if len(batch.Surfaces) >= 1 {
			return a2uiError("bounds_exceeded", line, "surfaceId", "surface bound")
		}
		if _, exists := batch.Surfaces[key]; exists {
			return a2uiError("invalid_lifecycle", line, "surfaceId", "surface exists")
		}
		batch.Surfaces[key] = &a2uiSurface{SurfaceID: surfaceID, NamespacedSurfaceID: key,
			Version: a2uiVersion, CatalogID: a2uiCatalogID, SendDataModel: send, Revision: 1,
			Components: make(map[string]any), DataModel: map[string]any{}, Actions: []a2uiAction{}}
		delete(deleted, key)
	case "updateComponents":
		if failure = onlyA2UIFields(body, line, "surfaceId", "components"); failure != nil {
			return failure
		}
		surface, failure := activeA2UISurface(batch, deleted, key, line)
		if failure != nil {
			return failure
		}
		values, ok := body["components"].([]any)
		if !ok {
			return a2uiError("invalid_component", line, "components", "expected array")
		}
		if len(values) == 0 || len(values) > a2uiCollectionLimit {
			return a2uiError("bounds_exceeded", line, "components", "component bound")
		}
		seen := make(map[string]bool)
		for _, value := range values {
			id, failed := validateA2UIComponent(value, line)
			if failed != nil {
				return failed
			}
			if seen[id] {
				return a2uiError("invalid_component", line, "id", "duplicate id")
			}
			seen[id] = true
			surface.Components[id] = value
		}
		if len(surface.Components) > a2uiCollectionLimit {
			return a2uiError("bounds_exceeded", line, "components", "component bound")
		}
		surface.Revision++
		return validateA2UIIntegrity(surface, line, false)
	case "updateDataModel":
		if failure = onlyA2UIFields(body, line, "surfaceId", "path", "value"); failure != nil {
			return failure
		}
		surface, failure := activeA2UISurface(batch, deleted, key, line)
		if failure != nil {
			return failure
		}
		path := "/"
		if value, exists := body["path"]; exists {
			path, ok = value.(string)
			if !ok {
				path = ""
			}
		}
		parts, failure := parseA2UIPointer(path, line)
		if failure != nil {
			return failure
		}
		if value, exists := body["value"]; exists {
			model, failed := setA2UIValue(surface.DataModel, parts, value, line)
			if failed != nil {
				return failed
			}
			surface.DataModel = model
		} else {
			surface.DataModel = removeA2UIValue(surface.DataModel, parts)
		}
		encoded, _ := json.Marshal(surface.DataModel)
		if len(encoded) > a2uiActionJSONLimit {
			return a2uiError("bounds_exceeded", line, "value", "data bound")
		}
		surface.Revision++
	case "deleteSurface":
		if failure = onlyA2UIFields(body, line, "surfaceId"); failure != nil {
			return failure
		}
		if _, exists := batch.Surfaces[key]; !exists {
			return a2uiError("invalid_lifecycle", line, "surfaceId", "surface inactive")
		}
		delete(batch.Surfaces, key)
		deleted[key] = true
	}
	return nil
}

func validateA2UIComponent(raw any, line int) (string, *a2uiRepair) {
	value, ok := raw.(map[string]any)
	if !ok {
		return "", a2uiError("invalid_component", line, "", "expected object")
	}
	id, failure := a2uiIdentifier(value["id"], line)
	if failure != nil {
		return "", failure
	}
	kind, ok := value["component"].(string)
	if !ok {
		return "", a2uiError("invalid_protocol", line, "", "expected string")
	}
	if !stringIn(kind, a2uiComponents) {
		return "", a2uiError("unsupported_catalog", line, "component", "unsupported component")
	}
	var allowed []string
	switch kind {
	case "Text":
		allowed = []string{"id", "component", "text", "variant"}
		failure = dynamicA2UI(value["text"], "string", line)
		if failure == nil {
			failure = enumA2UI(value, "variant", line, "h1", "h2", "h3", "h4", "h5", "caption", "body")
		}
	case "Row", "Column":
		allowed = []string{"id", "component", "children", "justify", "align"}
		children, valid := value["children"].([]any)
		if !valid {
			failure = a2uiError("invalid_component", line, "children", "expected array")
		}
		for _, child := range children {
			if failure == nil {
				_, failure = a2uiIdentifier(child, line)
			}
		}
		if failure == nil {
			failure = enumA2UI(value, "justify", line, "start", "center", "end", "spaceBetween", "spaceAround", "spaceEvenly")
		}
		if failure == nil {
			failure = enumA2UI(value, "align", line, "start", "center", "end", "stretch")
		}
	case "Card":
		allowed = []string{"id", "component", "child"}
		_, failure = a2uiIdentifier(value["child"], line)
	case "Divider":
		allowed = []string{"id", "component", "axis"}
		failure = enumA2UI(value, "axis", line, "horizontal", "vertical")
	case "Button":
		allowed = []string{"id", "component", "child", "action", "variant"}
		_, failure = a2uiIdentifier(value["child"], line)
		if failure == nil {
			failure = validateA2UIEvent(value["action"], line)
		}
		if failure == nil {
			failure = enumA2UI(value, "variant", line, "default", "primary", "borderless")
		}
	case "TextField":
		allowed = []string{"id", "component", "label", "value", "variant"}
		failure = dynamicA2UI(value["label"], "string", line)
		if _, exists := value["value"]; failure == nil && exists {
			failure = dynamicA2UI(value["value"], "string", line)
		}
		if failure == nil {
			failure = enumA2UI(value, "variant", line, "longText", "shortText", "obscured")
		}
	case "CheckBox":
		allowed = []string{"id", "component", "label", "value"}
		failure = dynamicA2UI(value["label"], "string", line)
		if failure == nil {
			failure = dynamicA2UI(value["value"], "bool", line)
		}
	case "ChoicePicker":
		allowed = []string{"id", "component", "label", "options", "value", "variant", "displayStyle", "filterable"}
		if _, exists := value["label"]; exists {
			failure = dynamicA2UI(value["label"], "string", line)
		}
		if failure == nil {
			failure = choiceOptionsA2UI(value["options"], line)
		}
		if failure == nil {
			failure = dynamicA2UI(value["value"], "strings", line)
		}
		if failure == nil {
			failure = enumA2UI(value, "variant", line, "multipleSelection", "mutuallyExclusive")
		}
		if failure == nil {
			failure = enumA2UI(value, "displayStyle", line, "checkbox")
		}
		if filterable, exists := value["filterable"]; failure == nil && exists && filterable != false {
			failure = a2uiError("invalid_component", line, "filterable", "unsupported value")
		}
	}
	if failure != nil {
		return "", failure
	}
	if failure = onlyA2UIFields(value, line, allowed...); failure != nil {
		return "", failure
	}
	return id, nil
}

func dynamicA2UI(value any, kind string, line int) *a2uiRepair {
	literal := kind == "string" && func() bool { _, ok := value.(string); return ok }() ||
		kind == "bool" && func() bool { _, ok := value.(bool); return ok }()
	if kind == "strings" {
		values, ok := value.([]any)
		literal = ok && len(values) <= a2uiCollectionLimit
		for _, item := range values {
			if _, ok := item.(string); !ok {
				literal = false
			}
		}
	}
	if literal {
		return nil
	}
	binding, ok := value.(map[string]any)
	if !ok {
		return a2uiError("invalid_component", line, "", "expected object")
	}
	if failure := onlyA2UIFields(binding, line, "path"); failure != nil {
		return failure
	}
	path, ok := binding["path"].(string)
	if !ok {
		return a2uiError("invalid_protocol", line, "", "expected string")
	}
	_, failure := parseA2UIPointer(path, line)
	return failure
}

func choiceOptionsA2UI(raw any, line int) *a2uiRepair {
	values, ok := raw.([]any)
	if !ok || len(values) == 0 || len(values) > a2uiCollectionLimit {
		return a2uiError("invalid_component", line, "options", "invalid option count")
	}
	for _, raw := range values {
		value, ok := raw.(map[string]any)
		if !ok {
			return a2uiError("invalid_component", line, "options", "expected object")
		}
		if failure := onlyA2UIFields(value, line, "label", "value"); failure != nil {
			return failure
		}
		if failure := dynamicA2UI(value["label"], "string", line); failure != nil {
			return failure
		}
		if _, failure := a2uiIdentifier(value["value"], line); failure != nil {
			return failure
		}
	}
	return nil
}

func validateA2UIEvent(raw any, line int) *a2uiRepair {
	action, ok := raw.(map[string]any)
	if !ok {
		return a2uiError("invalid_action", line, "", "expected object")
	}
	if failure := onlyA2UIFields(action, line, "event"); failure != nil {
		return failure
	}
	event, ok := action["event"].(map[string]any)
	if !ok {
		return a2uiError("invalid_action", line, "", "expected object")
	}
	if failure := onlyA2UIFields(event, line, "name", "context"); failure != nil {
		return failure
	}
	if _, failure := a2uiIdentifier(event["name"], line); failure != nil {
		return failure
	}
	if context, exists := event["context"]; exists {
		if _, ok := context.(map[string]any); !ok {
			return a2uiError("invalid_action", line, "context", "expected object")
		}
	}
	return nil
}

func validateA2UIIntegrity(surface *a2uiSurface, line int, requireRoot bool) *a2uiRepair {
	root, exists := surface.Components["root"]
	if !exists {
		if requireRoot {
			return a2uiError("missing_root", line, "root", "missing root")
		}
		return nil
	}
	rootObject, _ := root.(map[string]any)
	if rootObject["id"] != "root" {
		return a2uiError("missing_root", line, "root", "invalid root")
	}
	visiting, visited := map[string]bool{}, map[string]bool{}
	var visit func(string, int) *a2uiRepair
	visit = func(id string, depth int) *a2uiRepair {
		if depth > a2uiDepthLimit {
			return a2uiError("bounds_exceeded", line, id, "component depth")
		}
		if visited[id] {
			return nil
		}
		if visiting[id] {
			return a2uiError("invalid_reference", line, id, "component cycle")
		}
		component, exists := surface.Components[id]
		if !exists {
			return a2uiError("invalid_reference", line, id, "missing reference")
		}
		visiting[id] = true
		for _, child := range a2uiReferences(component) {
			if failure := visit(child, depth+1); failure != nil {
				return failure
			}
		}
		delete(visiting, id)
		visited[id] = true
		return nil
	}
	if failure := visit("root", 0); failure != nil {
		return failure
	}
	if len(visited) != len(surface.Components) {
		ids := make([]string, 0, len(surface.Components))
		for id := range surface.Components {
			ids = append(ids, id)
		}
		sort.Strings(ids)
		for _, id := range ids {
			if !visited[id] {
				return a2uiError("invalid_reference", line, id, "unreachable component")
			}
		}
	}
	return nil
}

func a2uiReferences(raw any) []string {
	value, _ := raw.(map[string]any)
	switch value["component"] {
	case "Row", "Column":
		children, _ := value["children"].([]any)
		result := make([]string, 0, len(children))
		for _, child := range children {
			if id, ok := child.(string); ok {
				result = append(result, id)
			}
		}
		return result
	case "Card", "Button":
		if id, ok := value["child"].(string); ok {
			return []string{id}
		}
	}
	return nil
}

func refreshA2UIActions(surface *a2uiSurface) {
	for id, raw := range surface.Components {
		value, _ := raw.(map[string]any)
		if value["component"] != "Button" {
			continue
		}
		action, _ := value["action"].(map[string]any)
		event, _ := action["event"].(map[string]any)
		name, _ := event["name"].(string)
		if name == "" {
			continue
		}
		surface.Actions = append(surface.Actions, a2uiAction{SourceComponentID: id, Name: name, Context: event["context"]})
	}
	sort.Slice(surface.Actions, func(i, j int) bool {
		if surface.Actions[i].SourceComponentID == surface.Actions[j].SourceComponentID {
			return surface.Actions[i].Name < surface.Actions[j].Name
		}
		return surface.Actions[i].SourceComponentID < surface.Actions[j].SourceComponentID
	})
}

func validateA2UIBindings(surface *a2uiSurface) *a2uiRepair {
	ids := make([]string, 0, len(surface.Components))
	for id := range surface.Components {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		value, _ := surface.Components[id].(map[string]any)
		if !stringIn(fmt.Sprint(value["component"]), []string{"TextField", "CheckBox", "ChoicePicker"}) {
			continue
		}
		if !surface.SendDataModel {
			return a2uiError("invalid_protocol", 0, id, "interactive input requires synchronized data model actions")
		}
		binding, ok := value["value"].(map[string]any)
		if !ok || len(binding) != 1 {
			return a2uiError("invalid_data_path", 0, id, "interactive input requires a data binding")
		}
		if _, ok := binding["path"].(string); !ok {
			return a2uiError("invalid_data_path", 0, id, "interactive input requires a data binding")
		}
	}
	return nil
}

func validateA2UIBounds(value any, depth, line int) *a2uiRepair {
	if depth > a2uiDepthLimit {
		return a2uiError("bounds_exceeded", line, "", "JSON depth")
	}
	switch value := value.(type) {
	case string:
		if len(value) > a2uiStringLimit {
			return a2uiError("bounds_exceeded", line, "", "string bound")
		}
	case []any:
		if len(value) > a2uiCollectionLimit {
			return a2uiError("bounds_exceeded", line, "", "array bound")
		}
		for _, child := range value {
			if failure := validateA2UIBounds(child, depth+1, line); failure != nil {
				return failure
			}
		}
	case map[string]any:
		if len(value) > a2uiCollectionLimit {
			return a2uiError("bounds_exceeded", line, "", "object bound")
		}
		for _, child := range value {
			if failure := validateA2UIBounds(child, depth+1, line); failure != nil {
				return failure
			}
		}
	}
	return nil
}

func validateA2UISafety(value any, line int) *a2uiRepair {
	switch value := value.(type) {
	case string:
		for index := 0; index+1 < len(value); index++ {
			if value[index] == '<' && (isASCIIAlpha(value[index+1]) || strings.ContainsRune("/!?", rune(value[index+1]))) {
				return a2uiError("unsafe_content", line, "", "raw HTML rejected")
			}
		}
	case []any:
		for _, child := range value {
			if failure := validateA2UISafety(child, line); failure != nil {
				return failure
			}
		}
	case map[string]any:
		for key, child := range value {
			lower := strings.ToLower(key)
			if stringIn(lower, []string{"html", "script", "functioncall", "clientfunction", "call"}) {
				return a2uiError("unsafe_content", line, key, "unsafe field")
			}
			if lower == "url" {
				if text, ok := child.(string); ok {
					scheme := strings.ToLower(strings.TrimSpace(text))
					for _, prefix := range []string{"javascript:", "data:", "vbscript:", "file:", "blob:"} {
						if strings.HasPrefix(scheme, prefix) {
							return a2uiError("unsafe_content", line, key, "unsafe URL")
						}
					}
				}
			}
			if failure := validateA2UISafety(child, line); failure != nil {
				return failure
			}
		}
	}
	return nil
}

func parseA2UIPointer(path string, line int) ([]string, *a2uiRepair) {
	if !strings.HasPrefix(path, "/") {
		return nil, a2uiError("invalid_data_path", line, "path", "expected pointer")
	}
	if path == "/" {
		return nil, nil
	}
	parts := strings.Split(path[1:], "/")
	for index, part := range parts {
		if part == "" {
			return nil, a2uiError("invalid_data_path", line, "path", "empty segment")
		}
		var output strings.Builder
		for offset := 0; offset < len(part); offset++ {
			if part[offset] != '~' {
				output.WriteByte(part[offset])
				continue
			}
			offset++
			if offset >= len(part) || part[offset] != '0' && part[offset] != '1' {
				return nil, a2uiError("invalid_data_path", line, "path", "invalid escape")
			}
			if part[offset] == '0' {
				output.WriteByte('~')
			} else {
				output.WriteByte('/')
			}
		}
		parts[index] = output.String()
	}
	return parts, nil
}

func setA2UIValue(root any, path []string, value any, line int) (any, *a2uiRepair) {
	if len(path) == 0 {
		return value, nil
	}
	object, ok := root.(map[string]any)
	if !ok {
		return nil, a2uiError("invalid_data_path", line, "path", "cannot descend")
	}
	if len(path) == 1 {
		object[path[0]] = value
		return root, nil
	}
	child, exists := object[path[0]]
	if !exists {
		child = map[string]any{}
		object[path[0]] = child
	}
	updated, failure := setA2UIValue(child, path[1:], value, line)
	if failure != nil {
		return nil, failure
	}
	object[path[0]] = updated
	return root, nil
}

func removeA2UIValue(root any, path []string) any {
	if len(path) == 0 {
		return map[string]any{}
	}
	object, ok := root.(map[string]any)
	if !ok {
		return root
	}
	if len(path) == 1 {
		delete(object, path[0])
		return root
	}
	if child, exists := object[path[0]]; exists {
		object[path[0]] = removeA2UIValue(child, path[1:])
	}
	return root
}

func activeA2UISurface(batch *a2uiBatch, deleted map[string]bool, key string, line int) (*a2uiSurface, *a2uiRepair) {
	if surface := batch.Surfaces[key]; surface != nil {
		return surface, nil
	}
	code := "invalid_ordering"
	if deleted[key] {
		code = "invalid_lifecycle"
	}
	return nil, a2uiError(code, line, "surfaceId", "surface unavailable")
}

func a2uiIdentifier(raw any, line int) (string, *a2uiRepair) {
	value, ok := raw.(string)
	if !ok {
		return "", a2uiError("invalid_protocol", line, "", "expected string")
	}
	if value == "" || strings.TrimSpace(value) != value || len(value) > a2uiStringLimit || strings.IndexFunc(value, unicode.IsControl) >= 0 {
		return "", a2uiError("invalid_protocol", line, "", "invalid identifier")
	}
	return value, nil
}

func onlyA2UIFields(value map[string]any, line int, allowed ...string) *a2uiRepair {
	for key := range value {
		if !stringIn(key, allowed) {
			return a2uiError("invalid_protocol", line, key, "unsupported field")
		}
	}
	return nil
}

func enumA2UI(value map[string]any, key string, line int, allowed ...string) *a2uiRepair {
	raw, exists := value[key]
	if !exists {
		return nil
	}
	text, ok := raw.(string)
	if !ok {
		return a2uiError("invalid_protocol", line, "", "expected string")
	}
	if !stringIn(text, allowed) {
		return a2uiError("invalid_component", line, key, "unsupported value")
	}
	return nil
}
func escapeA2UISegment(value string) string {
	const hex = "0123456789ABCDEF"
	var result strings.Builder
	for _, value := range []byte(value) {
		if isASCIIAlpha(value) || value >= '0' && value <= '9' || strings.ContainsRune("-_.~", rune(value)) {
			result.WriteByte(value)
			continue
		}
		result.WriteByte('%')
		result.WriteByte(hex[value>>4])
		result.WriteByte(hex[value&15])
	}
	return result.String()
}
func stringIn(value string, allowed []string) bool {
	for _, item := range allowed {
		if value == item {
			return true
		}
	}
	return false
}
func isASCIIAlpha(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z'
}
func a2uiProjection(batch *a2uiBatch) map[string]any {
	encoded, _ := json.Marshal(batch)
	var value map[string]any
	_ = json.Unmarshal(encoded, &value)
	value["protocol_version"] = a2uiVersion
	value["catalog"] = map[string]any{"catalog_id": a2uiCatalogID, "protocol_version": a2uiVersion, "components": append([]string(nil), a2uiComponents...)}
	return value
}

func storedA2UIInput(batch *a2uiBatch, hasActions bool, assignment store.ModelAssignment,
	responseID string, hostedState bool, credentialRevision uint64, toolCatalogDigest string,
) *store.ConversationA2UIInput {
	if batch == nil || len(batch.Surfaces) == 0 {
		return nil
	}
	return &store.ConversationA2UIInput{Projection: a2uiProjection(batch), HasActions: hasActions,
		ProviderSelection: modelAssignmentValue(assignment), ResponseID: responseID, HostedState: hostedState,
		CredentialRevision: credentialRevision, ToolCatalogDigest: toolCatalogDigest}
}
