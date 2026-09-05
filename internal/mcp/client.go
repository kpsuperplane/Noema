package mcp

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/netip"
	"net/url"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/netpolicy"
	mcpauth "github.com/modelcontextprotocol/go-sdk/auth"
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

const (
	maxDiscoveredTools = 128
	maxSchemaBytes     = 256 << 10
	maxAnnotations     = 16 << 10
	maxToolResult      = 1 << 20
	maxWireBody        = 32 << 20
)

// ErrAuthenticationRequired reports one fixed authentication boundary failure.
var ErrAuthenticationRequired = errors.New("MCP authentication is required")

// SecretMaterial contains MCP credentials. Its formatting never exposes values.
type SecretMaterial struct {
	Environment map[string]string `json:"environment,omitempty"`
	Headers     map[string]string `json:"headers,omitempty"`
	OAuth       *OAuthCredentials `json:"oauth,omitempty"`
	Client      *OAuthClient      `json:"client,omitempty"`
	Revision    string            `json:"revision"`
}

// OAuthClient is one protected pre-registered OAuth client.
type OAuthClient struct {
	ClientID, ClientSecret string
	Scopes                 []string
}

// OAuthCredentials is one protected bearer token set.
type OAuthCredentials struct {
	AccessToken, RefreshToken, TokenType string
	Expiry                               time.Time
	ClientID, ClientSecret, TokenURL     string
	Scopes                               []string
}

func (SecretMaterial) String() string   { return "[REDACTED]" }
func (SecretMaterial) GoString() string { return "mcp.SecretMaterial{[REDACTED]}" }

// Config is one safe transport definition plus its protected binding.
type Config struct {
	TransportKind string
	SafeConfig    json.RawMessage
	Secrets       SecretMaterial
	OAuthHandler  mcpauth.OAuthHandler
}

// DiscoveredTool is one bounded source contract.
type DiscoveredTool struct {
	Name, Description, SourceRevision            string
	InputSchema, OutputSchema, Annotations       json.RawMessage
	ReadOnly, Idempotent, Destructive, OpenWorld *bool
}

// Discovery is one bounded remote catalog.
type Discovery struct {
	ServiceDescription string
	Tools              []DiscoveredTool
}

// Discover connects and returns the complete bounded tool catalog.
func Discover(ctx context.Context, config Config) (Discovery, error) {
	session, err := connect(ctx, config)
	if err != nil {
		return Discovery{}, safeTransportError("connect MCP server", err)
	}
	defer session.Close()
	tools, err := discoverTools(ctx, session)
	if err != nil {
		return Discovery{}, err
	}
	description := ""
	if initialized := session.InitializeResult(); initialized != nil && initialized.ServerInfo != nil {
		description = strings.TrimSpace(initialized.ServerInfo.Name)
	}
	return Discovery{ServiceDescription: description, Tools: tools}, nil
}

// Call rechecks the complete source catalog, then calls one exact tool revision.
func CallExact(ctx context.Context, config Config, name, revision string, arguments any) (json.RawMessage, bool, error) {
	session, err := connect(ctx, config)
	if err != nil {
		return nil, false, safeTransportError("connect MCP server", err)
	}
	defer session.Close()
	tools, err := discoverTools(ctx, session)
	if err != nil {
		return nil, false, err
	}
	found := false
	for _, tool := range tools {
		if tool.Name == name && tool.SourceRevision == revision {
			found = true
			break
		}
	}
	if !found {
		return nil, false, errors.New("MCP tool source revision changed")
	}
	result, err := session.CallTool(ctx, &mcpsdk.CallToolParams{Name: name, Arguments: arguments})
	if err != nil {
		return nil, false, safeTransportError("call MCP tool", err)
	}
	encoded, err := json.Marshal(result)
	if err != nil || len(encoded) > maxToolResult {
		return nil, false, errors.New("MCP tool result is invalid or too large")
	}
	return encoded, !result.IsError, nil
}

func connect(ctx context.Context, config Config) (*mcpsdk.ClientSession, error) {
	client := mcpsdk.NewClient(&mcpsdk.Implementation{Name: "noema", Version: "go-migration"}, nil)
	var transport mcpsdk.Transport
	switch config.TransportKind {
	case "stdio":
		var safe struct {
			Command string            `json:"command"`
			Args    []string          `json:"args"`
			Cwd     string            `json:"cwd"`
			Env     map[string]string `json:"env"`
		}
		if json.Unmarshal(config.SafeConfig, &safe) != nil || strings.TrimSpace(safe.Command) == "" {
			return nil, errors.New("MCP stdio configuration is invalid")
		}
		path, err := exec.LookPath(safe.Command)
		if err != nil {
			return nil, errors.New("MCP stdio executable is unavailable")
		}
		path, err = filepath.Abs(path)
		if err != nil {
			return nil, errors.New("MCP stdio executable is unavailable")
		}
		environment := make([]string, 0, len(safe.Env)+len(config.Secrets.Environment))
		for key, value := range safe.Env {
			environment = append(environment, key+"="+value)
		}
		for key, value := range config.Secrets.Environment {
			environment = append(environment, key+"="+value)
		}
		sort.Strings(environment)
		transport, err = newCommandTransport(Command{Path: path, Args: safe.Args, Env: environment, Cwd: safe.Cwd, MaxMessageBytes: 32 << 20})
		if err != nil {
			return nil, err
		}
	case "streamable_http":
		var safe struct {
			URL     string            `json:"url"`
			Headers map[string]string `json:"headers"`
		}
		if json.Unmarshal(config.SafeConfig, &safe) != nil {
			return nil, errors.New("MCP HTTP configuration is invalid")
		}
		httpClient, err := mcpHTTPClient(ctx, safe.URL, safe.Headers, config.Secrets)
		if err != nil {
			return nil, err
		}
		transport = &mcpsdk.StreamableClientTransport{Endpoint: safe.URL, HTTPClient: httpClient,
			MaxRetries: -1, DisableStandaloneSSE: true, OAuthHandler: config.OAuthHandler}
	default:
		return nil, errors.New("MCP transport is unsupported")
	}
	return client.Connect(ctx, transport, nil)
}

func discoverTools(ctx context.Context, session *mcpsdk.ClientSession) ([]DiscoveredTool, error) {
	result := make([]DiscoveredTool, 0)
	cursor := ""
	seenCursors, names := map[string]struct{}{}, map[string]struct{}{}
	for {
		page, err := session.ListTools(ctx, &mcpsdk.ListToolsParams{Cursor: cursor})
		if err != nil {
			return nil, safeTransportError("list MCP tools", err)
		}
		for _, source := range page.Tools {
			if len(result) >= maxDiscoveredTools {
				return nil, errors.New("MCP tool catalog exceeds its limit")
			}
			tool, err := normalizeTool(source)
			if err != nil {
				return nil, err
			}
			if _, exists := names[tool.Name]; exists {
				return nil, errors.New("MCP tool catalog has duplicate names")
			}
			names[tool.Name] = struct{}{}
			result = append(result, tool)
		}
		if page.NextCursor == "" {
			break
		}
		if len(page.NextCursor) > 8192 {
			return nil, errors.New("MCP pagination cursor exceeds its limit")
		}
		if _, exists := seenCursors[page.NextCursor]; exists {
			return nil, errors.New("MCP pagination did not advance")
		}
		seenCursors[page.NextCursor] = struct{}{}
		cursor = page.NextCursor
	}
	sort.Slice(result, func(i, j int) bool { return result[i].Name < result[j].Name })
	return result, nil
}

func normalizeTool(source *mcpsdk.Tool) (DiscoveredTool, error) {
	if source == nil || source.Name == "" || len(source.Name) > 256 || len(source.Description) > 8192 {
		return DiscoveredTool{}, errors.New("MCP tool metadata is invalid")
	}
	input, err := boundedObject(source.InputSchema, maxSchemaBytes)
	if err != nil {
		return DiscoveredTool{}, errors.New("MCP input schema is invalid")
	}
	var output json.RawMessage
	if source.OutputSchema != nil {
		output, err = boundedObject(source.OutputSchema, maxSchemaBytes)
		if err != nil {
			return DiscoveredTool{}, errors.New("MCP output schema is invalid")
		}
	}
	annotations, err := boundedJSONObject(source.Annotations, maxAnnotations)
	if source.Annotations == nil {
		annotations = json.RawMessage(`{}`)
		err = nil
	}
	if err != nil {
		return DiscoveredTool{}, errors.New("MCP annotations are invalid")
	}
	contract, _ := json.Marshal([]any{source.Name, source.Description, json.RawMessage(input), json.RawMessage(output), json.RawMessage(annotations)})
	digest := sha256.Sum256(contract)
	tool := DiscoveredTool{Name: source.Name, Description: source.Description, InputSchema: input,
		OutputSchema: output, Annotations: annotations, SourceRevision: hex.EncodeToString(digest[:])}
	if source.Annotations != nil {
		readOnly, idempotent := source.Annotations.ReadOnlyHint, source.Annotations.IdempotentHint
		tool.ReadOnly = &readOnly
		tool.Idempotent = &idempotent
		tool.Destructive = source.Annotations.DestructiveHint
		tool.OpenWorld = source.Annotations.OpenWorldHint
	}
	return tool, nil
}

func boundedObject(value any, limit int) (json.RawMessage, error) {
	encoded, err := boundedJSONObject(value, limit)
	if err != nil {
		return nil, err
	}
	var object map[string]any
	if json.Unmarshal(encoded, &object) != nil || object["type"] != "object" {
		return nil, errors.New("JSON schema must describe an object")
	}
	return encoded, nil
}

func boundedJSONObject(value any, limit int) (json.RawMessage, error) {
	encoded, err := json.Marshal(value)
	if err != nil || len(encoded) > limit {
		return nil, errors.New("bounded JSON is invalid")
	}
	var object map[string]any
	if json.Unmarshal(encoded, &object) != nil {
		return nil, errors.New("JSON object is invalid")
	}
	nodes := 0
	if !boundedJSONValue(object, 0, &nodes) {
		return nil, errors.New("JSON object exceeds structural limits")
	}
	return encoded, nil
}

func boundedJSONValue(value any, depth int, nodes *int) bool {
	*nodes++
	if depth > 64 || *nodes > 16384 {
		return false
	}
	switch value := value.(type) {
	case map[string]any:
		for key, child := range value {
			if len(key) > 65536 || !boundedJSONValue(child, depth+1, nodes) {
				return false
			}
		}
	case []any:
		for _, child := range value {
			if !boundedJSONValue(child, depth+1, nodes) {
				return false
			}
		}
	case string:
		return len(value) <= 65536
	}
	return true
}

type headerTransport struct {
	base    http.RoundTripper
	headers map[string]string
	token   string
}

func (t headerTransport) RoundTrip(request *http.Request) (*http.Response, error) {
	copy := request.Clone(request.Context())
	copy.Header = request.Header.Clone()
	for key, value := range t.headers {
		copy.Header.Set(key, value)
	}
	if t.token != "" {
		copy.Header.Set("Authorization", "Bearer "+t.token)
	}
	response, err := t.base.RoundTrip(copy)
	if err == nil && response.Body != nil {
		response.Body = &boundedBody{ReadCloser: response.Body, remaining: maxWireBody}
	}
	return response, err
}

type boundedBody struct {
	io.ReadCloser
	remaining int64
}

func (r *boundedBody) Read(buffer []byte) (int, error) {
	if r.remaining < 0 {
		return 0, ErrMessageTooLarge
	}
	if int64(len(buffer)) > r.remaining+1 {
		buffer = buffer[:r.remaining+1]
	}
	n, err := r.ReadCloser.Read(buffer)
	r.remaining -= int64(n)
	if r.remaining < 0 {
		return n, ErrMessageTooLarge
	}
	return n, err
}

func mcpHTTPClient(ctx context.Context, rawURL string, safeHeaders map[string]string, secrets SecretMaterial) (*http.Client, error) {
	parsed, err := url.Parse(strings.TrimSpace(rawURL))
	if err != nil || parsed.User != nil || parsed.Hostname() == "" {
		return nil, errors.New("MCP endpoint is invalid")
	}
	var client *http.Client
	if parsed.Scheme == "http" && loopbackHost(ctx, parsed.Hostname()) {
		transport := http.DefaultTransport.(*http.Transport).Clone()
		transport.Proxy = nil
		transport.DialContext = func(callCtx context.Context, network, address string) (net.Conn, error) {
			host, _, splitErr := net.SplitHostPort(address)
			if splitErr != nil || !loopbackHost(callCtx, host) {
				return nil, errors.New("MCP loopback target changed")
			}
			return (&net.Dialer{Timeout: 10 * time.Second}).DialContext(callCtx, network, address)
		}
		client = &http.Client{Transport: transport, Timeout: 30 * time.Second}
	} else if parsed.Scheme == "https" {
		checked, checkErr := netpolicy.CheckURL(ctx, rawURL)
		if checkErr != nil {
			return nil, errors.New("MCP endpoint is unavailable")
		}
		client = netpolicy.PinnedClient(checked, 30*time.Second)
	} else {
		return nil, errors.New("MCP endpoint must use HTTPS or loopback HTTP")
	}
	headers := make(map[string]string, len(safeHeaders)+len(secrets.Headers))
	for key, value := range safeHeaders {
		headers[key] = value
	}
	for key, value := range secrets.Headers {
		headers[key] = value
	}
	token := ""
	if secrets.OAuth != nil {
		token = secrets.OAuth.AccessToken
	}
	client.Transport = headerTransport{base: client.Transport, headers: headers, token: token}
	client.CheckRedirect = func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }
	return client, nil
}

func loopbackHost(ctx context.Context, host string) bool {
	if strings.EqualFold(strings.TrimSuffix(host, "."), "localhost") {
		return true
	}
	if address, err := netip.ParseAddr(host); err == nil {
		return address.IsLoopback()
	}
	addresses, err := net.DefaultResolver.LookupNetIP(ctx, "ip", host)
	if err != nil || len(addresses) == 0 {
		return false
	}
	for _, address := range addresses {
		if !address.IsLoopback() {
			return false
		}
	}
	return true
}

func safeTransportError(action string, err error) error {
	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		return err
	}
	message := strings.ToLower(err.Error())
	if errors.Is(err, ErrMessageTooLarge) || strings.Contains(err.Error(), ErrMessageTooLarge.Error()) {
		return fmt.Errorf("%s: %w", action, ErrMessageTooLarge)
	}
	if strings.Contains(message, "401") || strings.Contains(message, "unauthorized") {
		return ErrAuthenticationRequired
	}
	return fmt.Errorf("%s failed", action)
}
