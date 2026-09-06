package adapter

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/netpolicy"
	"github.com/kpsuperplane/noema/internal/script"
)

var (
	errOutcomeUncertain = errors.New("adapter request outcome is uncertain")
	errResponseInvalid  = errors.New("adapter response is invalid")
)

type encodedRequest struct {
	method, rawURL string
	headers        map[string]string
	body           []byte
}
type httpResponse struct {
	status      int
	contentType string
	body        []byte
}

func encodeRequest(definition Definition, operation CompiledOperation, raw json.RawMessage, cursor string) (encodedRequest, map[string]any, error) {
	if len(raw) > argumentLimit {
		return encodedRequest{}, nil, errors.New("adapter arguments are too large")
	}
	value, err := script.DecodeJSON(raw)
	if err != nil {
		return encodedRequest{}, nil, errors.New("adapter arguments are invalid")
	}
	arguments, ok := value.(map[string]any)
	if !ok {
		return encodedRequest{}, nil, errors.New("adapter arguments are invalid")
	}
	expected := make(map[string]Argument, len(operation.Arguments))
	for _, argument := range operation.Arguments {
		expected[argument.Name] = argument
	}
	continuation := ""
	for name, value := range arguments {
		if name == "continuation" && operation.Pagination.Kind == "response_token" {
			continuation, ok = value.(string)
			if !ok || continuation == "" || len(continuation) > 128 {
				return encodedRequest{}, nil, errors.New("adapter continuation is invalid")
			}
			continue
		}
		argument, exists := expected[name]
		if !exists || !validArgumentValue(argument, value) {
			return encodedRequest{}, nil, errors.New("adapter arguments are invalid")
		}
	}
	for _, argument := range operation.Arguments {
		if argument.Required {
			if _, exists := arguments[argument.Name]; !exists {
				return encodedRequest{}, nil, errors.New("adapter argument is required")
			}
		}
	}
	if cursor != "" && continuation == "" {
		return encodedRequest{}, nil, errors.New("adapter continuation is invalid")
	}
	modelArguments := make(map[string]any, len(arguments))
	for name, value := range arguments {
		if name != "continuation" {
			modelArguments[name] = value
		}
	}
	path := operation.Path
	query := url.Values{}
	for name, value := range operation.FixedQuery {
		query.Add(name, value)
	}
	body := map[string]any{}
	for _, argument := range operation.Arguments {
		value, exists := modelArguments[argument.Name]
		if !exists || value == nil {
			continue
		}
		switch argument.Location {
		case "path":
			text := scalarText(value)
			if text == "." || text == ".." {
				return encodedRequest{}, nil, errors.New("adapter path argument is invalid")
			}
			path = strings.ReplaceAll(path, "{"+argument.Name+"}", url.PathEscape(text))
		case "query":
			for _, text := range queryTexts(value) {
				query.Add(argument.Name, text)
			}
		case "json_body":
			if operation.JSONBodyTemplate == nil {
				body[argument.Name] = value
			}
		}
	}
	if strings.ContainsAny(path, "{}") {
		return encodedRequest{}, nil, errors.New("adapter path is invalid")
	}
	if operation.Pagination.PageSize != nil && !query.Has(operation.Pagination.PageSize.RequestArgument) {
		query.Add(operation.Pagination.PageSize.RequestArgument, strconv.Itoa(operation.Pagination.PageSize.Value))
	}
	if cursor != "" {
		if query.Has(operation.Pagination.RequestArgument) {
			return encodedRequest{}, nil, errors.New("adapter continuation query conflicts")
		}
		query.Add(operation.Pagination.RequestArgument, cursor)
	}
	parsed, err := url.Parse(strings.TrimSuffix(definition.Manifest.Origin, "/") + path)
	if err != nil {
		return encodedRequest{}, nil, errors.New("adapter URL is invalid")
	}
	parsed.RawQuery = query.Encode()
	origin, _ := url.Parse(definition.Manifest.Origin)
	if len(parsed.String()) > 8192 || parsed.Scheme != "https" || parsed.Host != origin.Host || parsed.User != nil {
		return encodedRequest{}, nil, errors.New("adapter URL changed origin")
	}
	var bodyRaw []byte
	if operation.JSONBodyTemplate != nil {
		rendered, renderErr := renderTemplate(operation.JSONBodyTemplate, modelArguments)
		if renderErr != nil {
			return encodedRequest{}, nil, renderErr
		}
		bodyRaw, err = json.Marshal(rendered)
	} else if len(body) != 0 {
		bodyRaw, err = json.Marshal(body)
	}
	if err != nil || len(bodyRaw) > argumentLimit {
		return encodedRequest{}, nil, errors.New("adapter body is invalid")
	}
	headers := make(map[string]string, len(operation.FixedHeaders))
	for name, value := range operation.FixedHeaders {
		headers[name] = value
	}
	return encodedRequest{method: operation.Method, rawURL: parsed.String(), headers: headers, body: bodyRaw}, modelArguments, nil
}

func validArgumentValue(argument Argument, value any) bool {
	if value == nil {
		return !argument.Required
	}
	switch argument.Type {
	case "string":
		text, ok := value.(string)
		if !ok {
			return false
		}
		if len(argument.EnumValues) > 0 {
			for _, allowed := range argument.EnumValues {
				if text == allowed {
					return true
				}
			}
			return false
		}
		return true
	case "integer":
		number, ok := value.(json.Number)
		if !ok {
			return false
		}
		_, err := number.Int64()
		return err == nil
	case "number":
		number, ok := value.(json.Number)
		if !ok {
			return false
		}
		parsed, err := number.Float64()
		return err == nil && !math.IsInf(parsed, 0) && !math.IsNaN(parsed)
	case "boolean":
		_, ok := value.(bool)
		return ok
	case "string_array":
		array, ok := value.([]any)
		if !ok {
			return false
		}
		for _, item := range array {
			if _, ok := item.(string); !ok {
				return false
			}
		}
		return true
	}
	return false
}
func scalarText(value any) string {
	switch value := value.(type) {
	case string:
		return value
	case json.Number:
		return value.String()
	case bool:
		return strconv.FormatBool(value)
	}
	return ""
}
func queryTexts(value any) []string {
	if array, ok := value.([]any); ok {
		result := make([]string, len(array))
		for i, item := range array {
			result[i] = scalarText(item)
		}
		return result
	}
	return []string{scalarText(value)}
}
func renderTemplate(value any, arguments map[string]any) (any, error) {
	switch value := value.(type) {
	case map[string]any:
		if name, ok := value["$argument"].(string); ok && len(value) == 1 {
			return arguments[name], nil
		}
		result := map[string]any{}
		for name, child := range value {
			rendered, err := renderTemplate(child, arguments)
			if err != nil {
				return nil, err
			}
			if rendered != nil {
				result[name] = rendered
			}
		}
		return result, nil
	case []any:
		result := make([]any, 0, len(value))
		for _, child := range value {
			rendered, err := renderTemplate(child, arguments)
			if err != nil {
				return nil, err
			}
			if rendered != nil {
				result = append(result, rendered)
			}
		}
		return result, nil
	default:
		return value, nil
	}
}

func executeHTTP(ctx context.Context, request encodedRequest, retry bool) (httpResponse, error) {
	attempts := 1
	if retry && request.method == "GET" {
		attempts = 2
	}
	var last error
	for range attempts {
		response, err := executeHTTPOnce(ctx, request)
		if err == nil {
			return response, nil
		}
		last = err
		if errors.Is(err, errResponseInvalid) || errors.Is(err, context.Canceled) {
			return httpResponse{}, err
		}
		if !errors.Is(err, context.DeadlineExceeded) && request.method != "GET" {
			return httpResponse{}, err
		}
	}
	return httpResponse{}, last
}

func executeHTTPOnce(ctx context.Context, request encodedRequest) (httpResponse, error) {
	resolveContext, cancelResolve := context.WithTimeout(ctx, 10*time.Second)
	defer cancelResolve()
	checked, err := netpolicy.CheckURL(resolveContext, request.rawURL)
	if err != nil || checked.URL.Scheme != "https" || checked.URL.User != nil {
		return httpResponse{}, errors.New("adapter target is unavailable")
	}
	client := netpolicy.PinnedClient(checked, 10*time.Second)
	if transport, ok := client.Transport.(*http.Transport); ok {
		transport.DisableKeepAlives = true
		transport.DisableCompression = true
		transport.MaxIdleConns = 0
		transport.ResponseHeaderTimeout = 30 * time.Second
	}
	client.Timeout = 30 * time.Second
	var body io.Reader
	if len(request.body) != 0 {
		body = bytes.NewReader(request.body)
	}
	httpRequest, err := http.NewRequestWithContext(ctx, request.method, request.rawURL, body)
	if err != nil {
		return httpResponse{}, errors.New("adapter request is invalid")
	}
	httpRequest.Header.Set("Accept-Encoding", "identity")
	for name, value := range request.headers {
		httpRequest.Header.Set(name, value)
	}
	if len(request.body) != 0 && httpRequest.Header.Get("Content-Type") == "" {
		httpRequest.Header.Set("Content-Type", "application/json")
	}
	response, err := client.Do(httpRequest)
	if err != nil {
		return httpResponse{}, errOutcomeUncertain
	}
	defer response.Body.Close()
	headerBytes := 0
	for name, values := range response.Header {
		for _, value := range values {
			headerBytes += len(name) + len(value)
		}
	}
	if headerBytes > 64<<10 {
		return httpResponse{}, errResponseInvalid
	}
	if encoding := response.Header.Get("Content-Encoding"); encoding != "" && strings.ToLower(strings.TrimSpace(encoding)) != "identity" {
		return httpResponse{}, errResponseInvalid
	}
	if response.ContentLength > 1<<20 {
		return httpResponse{}, errResponseInvalid
	}
	bodyRaw, err := io.ReadAll(io.LimitReader(response.Body, (1<<20)+1))
	if err != nil {
		return httpResponse{}, errOutcomeUncertain
	}
	if len(bodyRaw) > 1<<20 {
		return httpResponse{}, errResponseInvalid
	}
	contentType := strings.ToLower(strings.TrimSpace(strings.Split(response.Header.Get("Content-Type"), ";")[0]))
	return httpResponse{status: response.StatusCode, contentType: contentType, body: bodyRaw}, nil
}

func decodeResponse(response httpResponse, contract Response) (any, error) {
	if response.status == 204 {
		if contract.OutputSchema.Type != "null" {
			return nil, errors.New("adapter empty response is invalid")
		}
		return nil, nil
	}
	accepted := false
	for _, value := range contract.AcceptedContentTypes {
		if value == response.contentType {
			accepted = true
			break
		}
	}
	if !accepted {
		return nil, errors.New("adapter response content type is invalid")
	}
	var result any
	var err error
	if contract.Transform != nil {
		result, err = script.RunFunction(contract.Transform.Source, map[string]any{"status": json.Number(strconv.Itoa(response.status)), "body": string(response.body), "content_type": response.contentType})
	} else {
		result, err = script.DecodeJSON(response.body)
	}
	if err != nil || !matchesOutput(contract.OutputSchema, result) {
		return nil, errors.New("adapter response does not match its reviewed contract")
	}
	raw, err := script.MarshalJSON(result)
	if err != nil || len(raw) > modelResultLimit {
		return nil, errors.New("adapter response is too large")
	}
	return result, nil
}

func responseFailure(response httpResponse, sensitive map[string]bool, secretValues []string) json.RawMessage {
	value := map[string]any{"error": "remote_request_failed", "status": response.status}
	if len(response.body) != 0 && len(response.body) <= 4096 && (response.contentType == "application/json" || strings.HasSuffix(response.contentType, "+json")) {
		if body, err := script.DecodeJSON(response.body); err == nil {
			value["body"] = sanitizeSensitiveOutput(body, sensitive, secretValues)
		}
	}
	raw, _ := json.Marshal(value)
	return raw
}

func sanitizeSensitiveOutput(value any, sensitive map[string]bool, secretValues []string) any {
	value = sanitizeOutput(value)
	if len(sensitive) == 0 && len(secretValues) == 0 {
		return value
	}
	redactText := func(text string) string {
		for _, secret := range secretValues {
			text = strings.ReplaceAll(text, secret, "[REDACTED]")
		}
		return text
	}
	if text, ok := value.(string); ok {
		return redactText(text)
	}
	var scrub func(any)
	scrub = func(current any) {
		switch current := current.(type) {
		case map[string]any:
			for key, child := range current {
				if sensitive[strings.ToLower(key)] {
					current[key] = "[REDACTED]"
					continue
				}
				if text, ok := child.(string); ok && (strings.EqualFold(key, "url") || strings.EqualFold(key, "location")) {
					if parsed, err := url.Parse(text); err == nil {
						query := parsed.Query()
						for name := range query {
							if sensitive[strings.ToLower(name)] {
								query.Del(name)
							}
						}
						parsed.RawQuery = query.Encode()
						if fragment, err := url.ParseQuery(parsed.Fragment); err == nil {
							for name := range fragment {
								if sensitive[strings.ToLower(name)] {
									fragment.Del(name)
								}
							}
							parsed.Fragment = fragment.Encode()
						}
						current[key] = parsed.String()
					}
				}
				if text, ok := current[key].(string); ok {
					current[key] = redactText(text)
				}
				scrub(current[key])
			}
		case []any:
			for index, child := range current {
				if text, ok := child.(string); ok {
					current[index] = redactText(text)
				}
				scrub(current[index])
			}
		}
	}
	scrub(value)
	return value
}

var credentialFields = map[string]bool{"access_token": true, "api-key": true, "api_key": true, "apikey": true, "authorization": true, "client_assertion": true, "client_secret": true, "code_verifier": true, "cookie": true, "device_code": true, "id_token": true, "password": true, "proxy-authorization": true, "proxy_authorization": true, "refresh_token": true, "set-cookie": true, "set_cookie": true, "user_code": true, "x-api-key": true, "x-auth-token": true, "x_api_key": true, "x_auth_token": true}
var credentialQueries = map[string]bool{"access_token": true, "api_key": true, "apikey": true, "client_assertion": true, "client_secret": true, "code_verifier": true, "device_code": true, "id_token": true, "password": true, "refresh_token": true, "sig": true, "user_code": true, "x-amz-security-token": true, "x-amz-signature": true, "x-goog-signature": true}

func sanitizeOutput(value any) any {
	switch value := value.(type) {
	case map[string]any:
		for key, child := range value {
			lower := strings.ToLower(key)
			if credentialFields[lower] {
				value[key] = "[REDACTED]"
				continue
			}
			if lower == "url" || lower == "location" || lower == "referer" || lower == "final_url" {
				if raw, ok := child.(string); ok {
					if clean, changed := sanitizeURL(raw); changed {
						value[key] = clean
						continue
					}
				}
			}
			value[key] = sanitizeOutput(child)
		}
	case []any:
		for i := range value {
			value[i] = sanitizeOutput(value[i])
		}
	}
	return value
}

func sanitizeURL(raw string) (string, bool) {
	parsed, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || parsed.Scheme == "" || parsed.Host == "" {
		return raw, false
	}
	changed := parsed.User != nil
	parsed.User = nil
	query := parsed.Query()
	for name := range query {
		if credentialQueries[strings.ToLower(name)] {
			query.Del(name)
			changed = true
		}
	}
	parsed.RawQuery = query.Encode()
	if parsed.Fragment != "" && strings.Contains(parsed.Fragment, "=") {
		fragment, err := url.ParseQuery(parsed.Fragment)
		if err == nil {
			for name := range fragment {
				if credentialQueries[strings.ToLower(name)] {
					fragment.Del(name)
					changed = true
				}
			}
			parsed.Fragment = fragment.Encode()
		}
	}
	if !changed {
		return raw, false
	}
	return parsed.String(), true
}

func removePointer(value any, pointer string) (string, bool) {
	parts := strings.Split(strings.TrimPrefix(pointer, "/"), "/")
	if len(parts) == 0 {
		return "", false
	}
	for i := range parts {
		parts[i] = strings.ReplaceAll(strings.ReplaceAll(parts[i], "~1", "/"), "~0", "~")
	}
	var remove func(any, int) (any, string, bool)
	remove = func(current any, depth int) (any, string, bool) {
		last := depth == len(parts)-1
		switch parent := current.(type) {
		case map[string]any:
			child, exists := parent[parts[depth]]
			if !exists {
				return current, "", true
			}
			if last {
				if child == nil {
					return current, "", true
				}
				token, ok := child.(string)
				if !ok || token == "" || len(token) > 4096 {
					return current, "", false
				}
				delete(parent, parts[depth])
				return current, token, true
			}
			replacement, token, ok := remove(child, depth+1)
			if ok {
				parent[parts[depth]] = replacement
			}
			return current, token, ok
		case []any:
			index, err := strconv.Atoi(parts[depth])
			if err != nil || index < 0 || index >= len(parent) {
				return current, "", false
			}
			if last {
				if parent[index] == nil {
					return current, "", true
				}
				token, ok := parent[index].(string)
				if !ok || token == "" || len(token) > 4096 {
					return current, "", false
				}
				return append(parent[:index], parent[index+1:]...), token, true
			}
			replacement, token, ok := remove(parent[index], depth+1)
			if ok {
				parent[index] = replacement
			}
			return current, token, ok
		}
		return current, "", false
	}
	_, token, ok := remove(value, 0)
	return token, ok
}

func argumentsDigest(value map[string]any) string {
	raw, _ := script.MarshalJSON(value)
	return sha256Hex(raw)
}
func wrapResult(value any) (json.RawMessage, error) {
	raw, err := script.MarshalJSON(value)
	if err != nil {
		return nil, fmt.Errorf("encode adapter result: %w", err)
	}
	return raw, nil
}
