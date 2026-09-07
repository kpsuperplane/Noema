package adapter

import (
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"sort"
	"strconv"
	"strings"

	"github.com/kpsuperplane/noema/internal/script"
)

// proposalError is the structured setup boundary used for direct proposal
// input. Its fields are safe to return to the caller and never contain the
// rejected value.
type proposalError struct {
	Reason       string
	ManifestPath string
	OperationID  string
	Details      map[string]any
	Message      string
}

func (e *proposalError) Error() string {
	if e == nil || e.Message == "" {
		return "adapter proposal is invalid"
	}
	return e.Message
}

func proposalIssue(reason string) error { return &proposalError{Reason: reason} }

func proposalIssueAt(reason, path string) error {
	return &proposalError{Reason: reason, ManifestPath: path}
}

func proposalFailure(err error) (json.RawMessage, bool) {
	var issue *proposalError
	if !errors.As(err, &issue) {
		return nil, false
	}
	payload := map[string]any{
		"status":    "invalid_proposal",
		"reason":    issue.Reason,
		"next_step": "Correct the reported value and retry the proposal.",
	}
	if issue.ManifestPath != "" {
		payload["manifest_path"] = issue.ManifestPath
	}
	if issue.OperationID != "" {
		payload["operation_id"] = issue.OperationID
	}
	if len(issue.Details) != 0 {
		payload["details"] = issue.Details
	}
	if issue.Message != "" {
		payload["message"] = issue.Message
	}
	raw, marshalErr := json.Marshal(payload)
	if marshalErr != nil {
		return nil, false
	}
	return raw, true
}

// validateProposalShape reports schema paths before typed decoding. Dynamic
// map keys use a wildcard so rejected input cannot enter the error payload.
func validateProposalShape(raw []byte) error {
	value, err := script.DecodeJSON(raw)
	if err != nil {
		return proposalIssue("proposal_input_invalid")
	}
	object, ok := value.(map[string]any)
	if !ok {
		return proposalIssue("proposal_input_invalid")
	}
	operations, present := object["upsert_operations"]
	if !present {
		return nil
	}
	list, ok := operations.([]any)
	if !ok {
		return proposalIssueAt("proposal_input_invalid", "upsert_operations")
	}
	for index, rawOperation := range list {
		operation, ok := rawOperation.(map[string]any)
		if !ok {
			return proposalIssueAt("proposal_input_invalid", "upsert_operations["+strconv.Itoa(index)+"]")
		}
		if rawPagination, exists := operation["pagination"]; exists {
			pagination, ok := rawPagination.(map[string]any)
			if !ok {
				return proposalIssueAt("proposal_input_invalid", "upsert_operations["+strconv.Itoa(index)+"].pagination")
			}
			kind, ok := pagination["kind"].(string)
			if !ok || kind != "none" && kind != "response_token" {
				return proposalIssueAt("proposal_input_invalid", "upsert_operations["+strconv.Itoa(index)+"].pagination.kind")
			}
		}
		if rawHeaders, exists := operation["fixed_headers"]; exists {
			headers, ok := rawHeaders.(map[string]any)
			if !ok {
				return proposalIssueAt("proposal_input_invalid", "upsert_operations["+strconv.Itoa(index)+"].fixed_headers")
			}
			for _, value := range headers {
				if _, ok := value.(string); !ok {
					return proposalIssueAt("proposal_input_invalid", "upsert_operations["+strconv.Itoa(index)+"].fixed_headers.*")
				}
			}
		}
	}
	return nil
}

type proposalInput struct {
	SourceReference    string              `json:"source_reference"`
	NewDefinition      *newDefinition      `json:"new_definition,omitempty"`
	BaseSemanticDigest string              `json:"base_semantic_digest,omitempty"`
	Revision           *revisionProposal   `json:"revision,omitempty"`
	UpsertOperations   []operationProposal `json:"upsert_operations,omitempty"`
	RemoveOperationIDs []string            `json:"remove_operation_ids,omitempty"`
}

type newDefinition struct {
	DefinitionID, AdapterID, DisplayName, DefinitionRevision, Origin string
	Authentication                                                   Authentication `json:"authentication"`
}

func (value *newDefinition) UnmarshalJSON(raw []byte) error {
	type wire struct {
		DefinitionID       string         `json:"definition_id"`
		AdapterID          string         `json:"adapter_id"`
		DisplayName        string         `json:"display_name"`
		DefinitionRevision string         `json:"definition_revision"`
		Origin             string         `json:"origin"`
		Authentication     Authentication `json:"authentication"`
	}
	var parsed wire
	if err := decodeExactJSON(raw, &parsed); err != nil {
		return err
	}
	value.DefinitionID, value.AdapterID, value.DisplayName = parsed.DefinitionID, parsed.AdapterID, parsed.DisplayName
	value.DefinitionRevision, value.Origin, value.Authentication = parsed.DefinitionRevision, parsed.Origin, parsed.Authentication
	return nil
}

type revisionProposal struct {
	DefinitionRevision string          `json:"definition_revision"`
	DisplayName        *string         `json:"display_name,omitempty"`
	Origin             *string         `json:"origin,omitempty"`
	Authentication     *Authentication `json:"authentication,omitempty"`
}

type operationProposal struct {
	OperationID, Description, SourceDescription, Method, Path string
	Authorization                                             Authorization     `json:"authorization"`
	FixedHeaders                                              map[string]string `json:"fixed_headers,omitempty"`
	FixedQuery                                                map[string]string `json:"fixed_query,omitempty"`
	Arguments                                                 []Argument        `json:"arguments,omitempty"`
	JSONBodyTemplate                                          any               `json:"json_body_template,omitempty"`
	ReadOnly, Idempotent, Destructive, OpenWorld              bool
	Pagination                                                Pagination      `json:"pagination,omitempty"`
	Response                                                  json.RawMessage `json:"response"`
}

func (value *operationProposal) UnmarshalJSON(raw []byte) error {
	type wire struct {
		OperationID       string            `json:"operation_id"`
		Description       string            `json:"description"`
		SourceDescription string            `json:"source_description,omitempty"`
		Method            string            `json:"method"`
		Path              string            `json:"path"`
		Authorization     Authorization     `json:"authorization"`
		FixedHeaders      map[string]string `json:"fixed_headers,omitempty"`
		FixedQuery        map[string]string `json:"fixed_query,omitempty"`
		Arguments         []Argument        `json:"arguments,omitempty"`
		JSONBodyTemplate  any               `json:"json_body_template,omitempty"`
		ReadOnly          bool              `json:"read_only"`
		Idempotent        bool              `json:"idempotent"`
		Destructive       bool              `json:"destructive"`
		OpenWorld         bool              `json:"open_world"`
		Pagination        Pagination        `json:"pagination,omitempty"`
		Response          json.RawMessage   `json:"response"`
	}
	var parsed wire
	if err := decodeExactJSON(raw, &parsed); err != nil {
		return err
	}
	*value = operationProposal(parsed)
	return nil
}

func buildManifest(input proposalInput, base *Manifest) (Manifest, error) {
	if err := validateSourceReference(input.SourceReference); err != nil {
		return Manifest{}, proposalIssue("source_reference_invalid")
	}
	var manifest Manifest
	switch {
	case input.NewDefinition != nil && input.BaseSemanticDigest == "" && input.Revision == nil && base == nil:
		value := input.NewDefinition
		manifest = Manifest{SchemaVersion: 9, DefinitionID: value.DefinitionID, AdapterID: value.AdapterID, DisplayName: value.DisplayName,
			DefinitionRevision: value.DefinitionRevision, Origin: value.Origin, Authentication: value.Authentication}
	case input.NewDefinition == nil && input.BaseSemanticDigest != "" && input.Revision != nil && base != nil:
		manifest = *base
		manifest.Operations = append([]Operation(nil), base.Operations...)
		manifest.DefinitionRevision = input.Revision.DefinitionRevision
		if input.Revision.DisplayName != nil {
			manifest.DisplayName = *input.Revision.DisplayName
		}
		if input.Revision.Origin != nil {
			manifest.Origin = *input.Revision.Origin
		}
		if input.Revision.Authentication != nil {
			manifest.Authentication = *input.Revision.Authentication
		}
		manifest.Reviewed = false
	default:
		return Manifest{}, proposalIssue("proposal_input_invalid")
	}
	if len(input.UpsertOperations)+len(input.RemoveOperationIDs) == 0 || len(input.UpsertOperations) > 128 || len(input.RemoveOperationIDs) > 128 {
		return Manifest{}, proposalIssue("proposal_changes")
	}
	changed := make(map[string]bool)
	for _, id := range input.RemoveOperationIDs {
		if !validID(id) || changed[id] {
			return Manifest{}, proposalIssue("proposal_changes")
		}
		changed[id] = true
		found := false
		for index := range manifest.Operations {
			if manifest.Operations[index].OperationID == id {
				manifest.Operations = append(manifest.Operations[:index], manifest.Operations[index+1:]...)
				found = true
				break
			}
		}
		if !found {
			return Manifest{}, proposalIssue("proposal_changes")
		}
	}
	for _, proposed := range input.UpsertOperations {
		if !validID(proposed.OperationID) || changed[proposed.OperationID] {
			return Manifest{}, proposalIssue("proposal_changes")
		}
		changed[proposed.OperationID] = true
		operation, err := proposed.operation()
		if err != nil {
			return Manifest{}, proposalIssue("proposal_input_invalid")
		}
		found := false
		for index := range manifest.Operations {
			if manifest.Operations[index].OperationID == operation.OperationID {
				manifest.Operations[index] = operation
				found = true
				break
			}
		}
		if !found {
			manifest.Operations = append(manifest.Operations, operation)
		}
	}
	manifest.Reviewed = false
	return manifest, nil
}

func validateProposalMetadata(manifest Manifest) error {
	for index := range manifest.Operations {
		operation := &manifest.Operations[index]
		operationPath := "operations[" + strconv.Itoa(index) + "]"
		if strings.TrimSpace(operation.Description) == "" {
			return &proposalError{Reason: "description", ManifestPath: operationPath + ".description", OperationID: operation.OperationID}
		}
		for argumentIndex, argument := range operation.Arguments {
			if strings.TrimSpace(argument.Description) == "" {
				return &proposalError{Reason: "description", ManifestPath: operationPath + ".arguments[" + strconv.Itoa(argumentIndex) + "].description", OperationID: operation.OperationID}
			}
		}
		readOnly := operation.Behavior.ReadOnly.Value != nil && *operation.Behavior.ReadOnly.Value
		if !readOnly && operation.Response.Transform == nil {
			return &proposalError{Reason: "mutation_response_transform", ManifestPath: operationPath + ".response.transform", OperationID: operation.OperationID}
		}
	}
	return nil
}

func proposalOperationIndex(manifest Manifest, compileErr *CompileError) int {
	if compileErr == nil {
		return -1
	}
	message := compileErr.Error()
	marker := strings.Index(message, "operations[")
	if marker >= 0 {
		start := marker + len("operations[")
		end := strings.IndexByte(message[start:], ']')
		if end >= 0 {
			if index, err := strconv.Atoi(message[start : start+end]); err == nil && index >= 0 && index < len(manifest.Operations) {
				return index
			}
		}
	}
	for index := range manifest.Operations {
		if err := validateOperation(&manifest.Operations[index]); err != nil {
			return index
		}
	}
	if len(manifest.Operations) == 1 {
		return 0
	}
	return -1
}

func proposalCompileFailure(manifest Manifest, err error) error {
	var compileErr *CompileError
	if !errors.As(err, &compileErr) {
		return proposalIssue("proposal_input_invalid")
	}
	reason := compileErr.Field
	if reason == "" {
		if compileErr.Kind == "unsupported" {
			reason = "unsupported"
		} else {
			reason = "manifest_invalid"
		}
	}
	index := proposalOperationIndex(manifest, compileErr)
	path := ""
	operationID := ""
	if index >= 0 {
		operationID = manifest.Operations[index].OperationID
		field := map[string]string{
			"operation_id":                  "operation_id",
			"description":                   "description",
			"source_description":            "source_description",
			"operation_path":                "path",
			"path_arguments":                "path",
			"fixed_query":                   "fixed_query",
			"authority_header":              "fixed_headers",
			"unsafe_retry":                  "retry",
			"pagination":                    "pagination",
			"paginated_response":            "response",
			"response_size":                 "response.output_schema",
			"response_schema":               "response.output_schema",
			"reserved_response_field":       "response.output_schema",
			"response_content_type":         "response.accepted_content_types",
			"response_transform":            "response.transform",
			"authorization":                 "authorization",
			"authentication":                "authorization",
			"ambiguous_operation_scope_set": "authorization.accepted_scope_sets",
		}
		if value, ok := field[reason]; ok {
			path = "operations[" + strconv.Itoa(index) + "]." + value
		}
	}
	issue := &proposalError{Reason: reason, ManifestPath: path, OperationID: operationID, Message: compileErr.Error()}
	if reason == "response_size" && index >= 0 {
		maximum, finite := maximumOutputBytes(manifest.Operations[index].Response.OutputSchema)
		if finite {
			issue.Details = map[string]any{"maximum_serialized_bytes": maximum, "limit_bytes": modelResultLimit}
			issue.Message = fmt.Sprintf("response output permits %d serialized bytes; limit is %d bytes; this is a computed constraint, not a manifest field", maximum, modelResultLimit)
		}
	}
	return issue
}

func (proposal operationProposal) operation() (Operation, error) {
	pagination := proposal.Pagination
	if pagination.Kind == "" {
		pagination.Kind = "none"
	}
	retry := "never"
	if proposal.Method == "GET" && proposal.ReadOnly && proposal.Idempotent {
		retry = "transport_safe_read"
	}
	response, err := proposalResponse(proposal.Response)
	if err != nil {
		return Operation{}, err
	}
	model := "model"
	hint := func(value bool) Hint { copy := value; return Hint{Value: &copy, Source: &model} }
	return Operation{OperationID: proposal.OperationID, Description: proposal.Description, SourceDescription: proposal.SourceDescription,
		Method: proposal.Method, Path: proposal.Path, Authorization: proposal.Authorization, FixedHeaders: proposal.FixedHeaders,
		FixedQuery: proposal.FixedQuery, Arguments: proposal.Arguments, JSONBodyTemplate: proposal.JSONBodyTemplate,
		Behavior: BehaviorHints{hint(proposal.ReadOnly), hint(proposal.Idempotent), hint(proposal.Destructive), hint(proposal.OpenWorld)},
		Retry:    retry, Pagination: pagination, Response: response}, nil
}

type responseField struct {
	Name, SourcePointer, Type string
	MaxBytes                  *int
	Truncate, Required        bool
}

func proposalResponse(raw json.RawMessage) (Response, error) {
	var header struct {
		Kind string `json:"kind"`
	}
	if len(raw) == 0 || json.Unmarshal(raw, &header) != nil {
		return Response{}, errors.New("adapter response proposal is invalid")
	}
	if header.Kind == "custom" {
		var value struct {
			Kind      string       `json:"kind"`
			Accepted  []string     `json:"accepted_content_types"`
			Transform *Transform   `json:"transform,omitempty"`
			Schema    OutputSchema `json:"output_schema"`
		}
		if decodeExactJSON(raw, &value) != nil {
			return Response{}, errors.New("adapter response proposal is invalid")
		}
		return Response{AcceptedContentTypes: value.Accepted, Transform: value.Transform, OutputSchema: value.Schema}, nil
	}
	var value struct {
		Kind   string `json:"kind"`
		Fields []struct {
			Name          string `json:"name"`
			SourcePointer string `json:"source_pointer"`
			Type          string `json:"type"`
			MaxBytes      *int   `json:"max_bytes,omitempty"`
			Truncate      bool   `json:"truncate,omitempty"`
			Required      bool   `json:"required,omitempty"`
		} `json:"fields,omitempty"`
		SourcePointer string `json:"source_pointer,omitempty"`
		OutputName    string `json:"output_name,omitempty"`
		MaxItems      int    `json:"max_items,omitempty"`
		Item          *struct {
			Type     string `json:"type"`
			MaxBytes *int   `json:"max_bytes,omitempty"`
			Truncate bool   `json:"truncate,omitempty"`
		} `json:"item,omitempty"`
	}
	if decodeExactJSON(raw, &value) != nil {
		return Response{}, errors.New("adapter response proposal is invalid")
	}
	fields := make([]responseField, len(value.Fields))
	for i, field := range value.Fields {
		fields[i] = responseField{field.Name, field.SourcePointer, field.Type, field.MaxBytes, field.Truncate, field.Required}
	}
	return generatedResponse(value.Kind, value.SourcePointer, value.OutputName, value.MaxItems, fields, value.Item)
}

func generatedResponse(kind, sourcePointer, outputName string, maxItems int, fields []responseField, item any) (Response, error) {
	falseValue := false
	object := func(properties map[string]OutputSchema, required []string) OutputSchema {
		sort.Strings(required)
		return OutputSchema{Type: "object", Properties: properties, Required: required, AdditionalProperties: &falseValue}
	}
	fieldSchema := func(field responseField) (OutputSchema, error) {
		switch field.Type {
		case "string":
			if field.MaxBytes == nil || *field.MaxBytes < 0 {
				return OutputSchema{}, errors.New("response string bound is required")
			}
			return OutputSchema{Type: "string", MaxBytes: field.MaxBytes}, nil
		case "integer", "number", "boolean":
			return OutputSchema{Type: field.Type}, nil
		}
		return OutputSchema{}, errors.New("response field type is invalid")
	}
	base := "return function(response)\n local body=json.decode(response.body)\n local function at(v,p) for _,k in ipairs(p) do if type(v)~='table' then return nil end v=v[k] end return v end\n local output=json.object()\n"
	properties, required, assignments := map[string]OutputSchema{}, []string{}, ""
	for _, field := range fields {
		if !validOutputName(field.Name) || !validPointer(field.SourcePointer) {
			return Response{}, errors.New("response recipe is invalid")
		}
		schema, err := fieldSchema(field)
		if err != nil {
			return Response{}, err
		}
		properties[field.Name] = schema
		if field.Required {
			required = append(required, field.Name)
		}
		expression := luaValue("at(input,"+luaPath(field.SourcePointer)+")", field.Type, field.MaxBytes, field.Truncate)
		assignments += " local value=" + expression + "\n if value~=nil then item[" + luaString(field.Name) + "]=value end\n"
	}
	var schema OutputSchema
	switch kind {
	case "flat_object":
		body := strings.ReplaceAll(assignments, "input", "body")
		body = strings.ReplaceAll(body, "item[", "output[")
		schema, base = object(properties, required), base+body+" return output\nend"
	case "object_list":
		if maxItems < 1 || !validOutputName(outputName) || !validPointer(sourcePointer) || len(fields) == 0 {
			return Response{}, errors.New("response recipe is invalid")
		}
		itemSchema := object(properties, required)
		schema = object(map[string]OutputSchema{outputName: {Type: "array", Items: &itemSchema, MaxItems: &maxItems}}, []string{outputName})
		base += " local source=at(body," + luaPath(sourcePointer) + ") local values=json.array()\n if type(source)=='table' then for index=1,math.min(#source," + fmt.Sprint(maxItems) + ") do local input=source[index] local item=json.object()\n" + assignments + " values[#values+1]=item end end\n output[" + luaString(outputName) + "]=values return output\nend"
	case "scalar_list":
		var scalar struct {
			Type     string `json:"type"`
			MaxBytes *int   `json:"max_bytes,omitempty"`
			Truncate bool   `json:"truncate,omitempty"`
		}
		raw, _ := json.Marshal(item)
		if json.Unmarshal(raw, &scalar) != nil || maxItems < 1 || !validOutputName(outputName) || !validPointer(sourcePointer) {
			return Response{}, errors.New("response recipe is invalid")
		}
		itemSchema, err := fieldSchema(responseField{Type: scalar.Type, MaxBytes: scalar.MaxBytes})
		if err != nil {
			return Response{}, err
		}
		schema = object(map[string]OutputSchema{outputName: {Type: "array", Items: &itemSchema, MaxItems: &maxItems}}, []string{outputName})
		base += " local source=at(body," + luaPath(sourcePointer) + ") local values=json.array()\n if type(source)=='table' then for index=1,math.min(#source," + fmt.Sprint(maxItems) + ") do local value=" + luaValue("source[index]", scalar.Type, scalar.MaxBytes, scalar.Truncate) + " if value~=nil then values[#values+1]=value end end end\n output[" + luaString(outputName) + "]=values return output\nend"
	default:
		return Response{}, errors.New("response recipe kind is invalid")
	}
	return Response{AcceptedContentTypes: []string{"application/json"}, Transform: &Transform{Language: "lua", Source: base}, OutputSchema: schema}, nil
}

func luaValue(source, kind string, max *int, truncate bool) string {
	switch kind {
	case "string":
		if truncate {
			return "(type(" + source + ")==\"string\" and text.truncate_utf8(" + source + "," + fmt.Sprint(*max) + ") or nil)"
		}
		return "(type(" + source + ")==\"string\" and #" + source + "<=" + fmt.Sprint(*max) + " and " + source + " or nil)"
	case "integer":
		return "(type(" + source + ")==\"number\" and math.type(" + source + ")==\"integer\" and " + source + " or nil)"
	case "number":
		return "(type(" + source + ")==\"number\" and " + source + " or nil)"
	case "boolean":
		return "(type(" + source + ")==\"boolean\" and " + source + " or nil)"
	}
	return "nil"
}

func luaPath(pointer string) string {
	parts := strings.Split(strings.TrimPrefix(pointer, "/"), "/")
	values := make([]string, 0, len(parts))
	for _, part := range parts {
		part = strings.ReplaceAll(strings.ReplaceAll(part, "~1", "/"), "~0", "~")
		values = append(values, luaString(part))
	}
	return "{" + strings.Join(values, ",") + "}"
}
func luaString(value string) string { raw, _ := json.Marshal(value); return string(raw) }
func validateSourceReference(raw string) error {
	if len(raw) == 0 || len(raw) > 4096 || strings.TrimSpace(raw) != raw {
		return errors.New("adapter source reference is invalid")
	}
	value, err := url.Parse(raw)
	if err != nil || value.Scheme != "https" || value.Hostname() == "" || value.User != nil {
		return errors.New("adapter source reference is invalid")
	}
	return nil
}
