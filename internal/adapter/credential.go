package adapter

import (
	"encoding/json"
	"errors"
	"net/url"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/script"
)

const (
	credentialDocumentLimit   = 128 << 10
	credentialGenerationLimit = 512 << 10
	credentialValueLimit      = 16 << 10
)

type credentialGeneration struct {
	SchemaVersion int               `json:"schema_version"`
	GenerationID  string            `json:"generation_id"`
	Fields        map[string]string `json:"fields"`
}

func normalizeCredential(setup CredentialSetup, input CredentialInputValue) (map[string]string, error) {
	wanted := make(map[string]bool, len(setup.Input.Fields))
	for _, field := range setup.Input.Fields {
		wanted[field.ID] = true
	}
	var values map[string]string
	switch setup.Input.Kind {
	case "fields":
		if len(input.Document) != 0 || len(input.FieldValues) != len(wanted) {
			return nil, errors.New("adapter credential input is invalid")
		}
		values = input.FieldValues
	case "document":
		if len(input.FieldValues) != 0 || len(input.Document) == 0 || len(input.Document) > credentialDocumentLimit || !utf8.Valid(input.Document) || !json.Valid(input.Document) {
			return nil, errors.New("adapter credential document is invalid")
		}
		result, err := script.RunFunction(setup.Input.Normalize.Source, map[string]any{"document": string(input.Document)})
		object, ok := result.(map[string]any)
		if err != nil || !ok || len(object) != len(wanted) {
			return nil, errors.New("adapter credential normalization failed")
		}
		values = make(map[string]string, len(object))
		for name, raw := range object {
			text, ok := raw.(string)
			if !ok {
				return nil, errors.New("adapter credential normalization failed")
			}
			values[name] = text
		}
	default:
		return nil, errors.New("adapter credential input is invalid")
	}
	if !validCredentialFields(setup, values) {
		return nil, errors.New("adapter credential input is invalid")
	}
	return values, nil
}

func validCredentialFields(setup CredentialSetup, values map[string]string) bool {
	if len(values) != len(setup.Input.Fields) {
		return false
	}
	wanted := make(map[string]bool, len(setup.Input.Fields))
	for _, field := range setup.Input.Fields {
		wanted[field.ID] = true
	}
	for name, value := range values {
		if !wanted[name] || value == "" || len(value) > credentialValueLimit || strings.IndexFunc(value, controlRune) >= 0 {
			return false
		}
	}
	return true
}

var forbiddenCredentialHeaders = map[string]bool{
	"host": true, "content-length": true, "content-type": true, "content-encoding": true,
	"content-range": true, "transfer-encoding": true, "connection": true, "keep-alive": true,
	"proxy-connection": true, "proxy-authenticate": true, "proxy-authorization": true,
	"te": true, "trailer": true, "upgrade": true, "http2-settings": true, "expect": true,
	"via": true, "cookie": true, "set-cookie": true, "accept-encoding": true, "forwarded": true,
}

func applyCredentialAuth(auth Authentication, fields map[string]string, operation CompiledOperation, request *encodedRequest) (map[string]bool, error) {
	parsed, _ := url.Parse(request.rawURL)
	credentials := make(map[string]any, len(fields))
	for name, value := range fields {
		credentials[name] = value
	}
	headerText := make([]string, 0, len(request.headers))
	for name := range request.headers {
		headerText = append(headerText, name)
	}
	sort.Strings(headerText)
	headerNames := make([]any, len(headerText))
	for i, name := range headerText {
		headerNames[i] = name
	}
	query := parsed.Query()
	queryText := make([]string, 0, len(query))
	for name := range query {
		queryText = append(queryText, name)
	}
	sort.Strings(queryText)
	queryNames := make([]any, len(queryText))
	for i, name := range queryText {
		queryNames[i] = name
	}
	input := map[string]any{"credentials": credentials, "request": map[string]any{
		"operation_id": operation.OperationID, "method": request.method, "path": parsed.Path,
		"header_names": headerNames, "query_names": queryNames,
	}}
	result, err := script.RunFunction(auth.RequestAuth.Source, input)
	object, ok := result.(map[string]any)
	if err != nil || !ok || len(object) > 2 {
		return nil, errors.New("adapter request authentication failed")
	}
	sensitive := map[string]bool{}
	for key := range object {
		if key != "headers" && key != "query" {
			return nil, errors.New("adapter request authentication failed")
		}
	}
	if err := applyCredentialMap(object["headers"], 16, func(name, value string) error {
		lower := strings.ToLower(name)
		if !validHeaderName(name) || len(name) > 128 || forbiddenCredentialHeaders[lower] || strings.HasPrefix(lower, "x-forwarded-") || requestHeaderExists(request.headers, lower) || strings.ContainsAny(value, "\r\n") {
			return errors.New("adapter credential header is invalid")
		}
		request.headers[name] = value
		sensitive[lower] = true
		return nil
	}); err != nil {
		return nil, err
	}
	if err := applyCredentialMap(object["query"], 16, func(name, value string) error {
		if name == "" || len(name) > 128 || strings.IndexFunc(name, controlRune) >= 0 || query.Has(name) {
			return errors.New("adapter credential query is invalid")
		}
		query.Add(name, value)
		sensitive[strings.ToLower(name)] = true
		return nil
	}); err != nil {
		return nil, err
	}
	parsed.RawQuery = query.Encode()
	if len(parsed.String()) > 8192 {
		return nil, errors.New("adapter credential URL is too large")
	}
	request.rawURL = parsed.String()
	return sensitive, nil
}

func applyCredentialMap(raw any, limit int, apply func(string, string) error) error {
	if raw == nil {
		return nil
	}
	values, ok := raw.(map[string]any)
	if !ok || len(values) > limit {
		return errors.New("adapter request authentication failed")
	}
	for name, value := range values {
		text, ok := value.(string)
		if !ok || text == "" || len(text) > credentialValueLimit || strings.IndexFunc(text, controlRune) >= 0 || apply(name, text) != nil {
			return errors.New("adapter request authentication failed")
		}
	}
	return nil
}

func requestHeaderExists(values map[string]string, lower string) bool {
	for name := range values {
		if strings.EqualFold(name, lower) {
			return true
		}
	}
	return false
}
func controlRune(r rune) bool { return r < ' ' || r == 0x7f }
