package mcp

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/store"
	"golang.org/x/net/publicsuffix"
)

// ConnectServiceToolName is the primary Chat hosted setup tool.
const ConnectServiceToolName = "mcp.connect_service"

const connectServiceOperationToken = "mcp-setup-v1:connect-service"

func connectServiceBinding() Binding {
	schema := json.RawMessage(`{"type":"object","properties":{"service_url":{"type":"string","maxLength":4096,"description":"Official public website URL for the service, such as https://notion.com/."}},"required":["service_url"],"additionalProperties":false}`)
	return Binding{
		Name:        ConnectServiceToolName,
		Description: "Discover and start chat-first setup for an official hosted MCP service. When the human asks to connect a service, first use web search to identify the service's official HTTPS website, then pass that website URL here. Noema fetches the site's /.well-known/mcp.json server card, verifies the advertised Streamable HTTP endpoint, and starts connection discovery. Do not guess an MCP endpoint or pass a third-party directory, documentation mirror, API endpoint, token, cookie, or other credential.",
		InputSchema: schema,
		InvokerKey:  "mcp", OperationToken: connectServiceOperationToken,
		Behavior:          store.ActionBehavior{OpenWorld: true},
		PersistencePolicy: BindingPersistenceRedacted,
		InputCheck: func(value any) bool {
			encoded, err := json.Marshal(value)
			return err == nil && ValidateArguments(schema, encoded) == nil
		},
	}
}

// ServiceCardResult is the safe result of one official-site setup request.
type ServiceCardResult struct {
	Status, ServiceURL, CardURL, DisplayName, Description, EndpointURL string
	Setup                                                              SetupResult
}

func (result ServiceCardResult) capabilityPayload() map[string]any {
	payload := map[string]any{
		"status":          result.Status,
		"service_url":     result.ServiceURL,
		"server_card_url": result.CardURL,
		"display_name":    result.DisplayName,
		"description":     result.Description,
		"endpoint_url":    result.EndpointURL,
		"next_step":       serviceCardNextStep(result.Status),
		"setup_input": map[string]any{
			"displayName":   result.DisplayName,
			"transportKind": "streamable_http",
			"stdio":         nil,
			"http": map[string]any{
				"url": result.EndpointURL, "headers": map[string]any{},
				"secretHeaders": map[string]any{}, "oauthClientCredentials": nil,
			},
		},
	}
	setup := map[string]any{
		"setup_status":          result.Setup.Status,
		"discovered_tool_count": result.Setup.Discovered,
		"setup_error":           nil,
		"auth":                  nil,
		"server":                nil,
	}
	if result.Setup.Error != "" {
		setup["setup_error"] = result.Setup.Error
	}
	if result.Setup.Server != nil {
		setup["server"] = map[string]any{
			"mcp_server_id":       result.Setup.Server.ID,
			"display_name":        result.Setup.Server.DisplayName,
			"connection_revision": result.Setup.Server.ConnectionRevision,
			"policy_revision":     result.Setup.Server.PolicyRevision,
			"tool_count":          result.Setup.Server.ToolCount,
		}
	}
	payload["setup_result"] = setup
	return payload
}

func serviceCardNextStep(status string) string {
	switch status {
	case "needs_auth", "authentication_available", "ready_for_policy":
		return "Do not narrate setup status or send the human to Settings. The human-intervention surface now owns authentication and policy setup."
	case "unavailable":
		return "Tell the human that Noema found the official MCP endpoint but could not connect to it, and that they can retry."
	case "invalid_card", "not_found":
		return "Tell the human that the official website did not publish a supported MCP server card. Do not guess an endpoint. If appropriate, investigate the service's public HTTP API instead."
	default:
		return ""
	}
}

type serverCard struct {
	Name, Title, Description, Endpoint string
	Endpoints                          []struct {
		URL string `json:"url"`
	}
	Transport *struct {
		Kind     string `json:"type"`
		Endpoint string `json:"endpoint"`
	} `json:"transport"`
	ServerInfo *struct{ Name, Title string } `json:"serverInfo"`
}

// ConnectService discovers the official server card and starts its public setup.
func (s *Service) ConnectService(ctx context.Context, raw string) ServiceCardResult {
	rawURL, _ := url.Parse(strings.TrimSpace(raw))
	service, err := normalizeServiceURL(ctx, raw)
	if err != nil {
		return ServiceCardResult{Status: "invalid_card", ServiceURL: raw}
	}
	result := ServiceCardResult{Status: "not_found", ServiceURL: service.String()}
	candidates := serverCardCandidates(service)
	// Some hosted services are mounted below a reverse-proxy path. Keep the
	// standard origin check first, then try that explicit path's card location.
	// The normalized service URL remains origin-scoped for storage and display.
	if rawURL != nil && rawURL.Path != "" && rawURL.Path != "/" {
		pathCard := *service
		pathCard.Path = strings.TrimRight(rawURL.Path, "/") + "/.well-known/mcp.json"
		candidates = append(candidates, &pathCard)
	}
	for _, candidate := range candidates {
		card, state := fetchServerCard(ctx, candidate)
		if state == "not_found" {
			continue
		}
		if state != "ok" {
			if result.Status != "invalid_card" {
				result.Status = state
			}
			continue
		}
		discovered, ok := parseServerCard(ctx, card, service, candidate)
		if !ok {
			result.Status = "invalid_card"
			continue
		}
		result = discovered
		input := SetupInput{DisplayName: result.DisplayName, TransportKind: "streamable_http",
			URL: result.EndpointURL, AuthPreference: "PROMPT_IF_AVAILABLE"}
		definition, setupErr := definitionFromSetup(input)
		if setupErr != nil {
			result.Status = "invalid_card"
			return result
		}
		discoveryContext, cancel := context.WithTimeout(ctx, discoveryTimeout)
		public, publicErr := Discover(discoveryContext, configFromInput(input, definition.SafeConfig))
		cancel()
		if publicErr == nil && oauthAvailable(ctx, result.EndpointURL) {
			result.Status = "authentication_available"
			result.Setup = SetupResult{Status: result.Status, Discovered: len(public.Tools), OAuthSupported: true}
			return result
		}
		setup, setupErr := s.create(ctx, input, definition, false)
		if setupErr != nil {
			result.Status = "unavailable"
			return result
		}
		result.Status, result.Setup = setup.Status, setup
		return result
	}
	return result
}

func normalizeServiceURL(ctx context.Context, raw string) (*url.URL, error) {
	value, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || value.User != nil || value.Hostname() == "" || value.Fragment != "" || (value.Scheme != "https" && value.Scheme != "http") {
		return nil, errors.New("invalid service URL")
	}
	if value.Scheme == "http" && !loopbackHost(ctx, value.Hostname()) {
		return nil, errors.New("invalid service URL")
	}
	value.Path, value.RawQuery, value.Fragment = "/", "", ""
	return value, nil
}

func serverCardCandidates(service *url.URL) []*url.URL {
	result := make([]*url.URL, 0, 3)
	host := service.Hostname()
	if domain, err := publicsuffix.EffectiveTLDPlusOne(host); err == nil {
		parent := *service
		parent.Host = domain
		parent.Path = "/.well-known/mcp.json"
		result = append(result, &parent)
		if strings.EqualFold(host, domain) {
			www := parent
			www.Host = "www." + domain
			result = append(result, &www)
		}
	}
	origin := *service
	origin.Path = "/.well-known/mcp.json"
	for _, item := range result {
		if item.String() == origin.String() {
			return result
		}
	}
	return append(result, &origin)
}

func fetchServerCard(ctx context.Context, candidate *url.URL) ([]byte, string) {
	requestContext, cancel := context.WithTimeout(ctx, 2*time.Second)
	defer cancel()
	client, err := mcpHTTPClient(requestContext, candidate.String(), nil, SecretMaterial{})
	if err != nil {
		return nil, "unavailable"
	}
	request, _ := http.NewRequestWithContext(requestContext, http.MethodGet, candidate.String(), nil)
	response, err := client.Do(request)
	if err != nil {
		return nil, "unavailable"
	}
	defer response.Body.Close()
	if response.StatusCode == http.StatusNotFound {
		return nil, "not_found"
	}
	if response.StatusCode != http.StatusOK {
		return nil, "unavailable"
	}
	body, err := io.ReadAll(io.LimitReader(response.Body, (64<<10)+1))
	if err != nil || len(body) > 64<<10 {
		return nil, "invalid_card"
	}
	return body, "ok"
}

func parseServerCard(ctx context.Context, body []byte, service, source *url.URL) (ServiceCardResult, bool) {
	var card serverCard
	if json.Unmarshal(body, &card) != nil {
		return ServiceCardResult{}, false
	}
	rawEndpoints := make([]string, 0, len(card.Endpoints)+2)
	for _, candidate := range card.Endpoints {
		rawEndpoints = append(rawEndpoints, candidate.URL)
	}
	if card.Endpoint != "" {
		rawEndpoints = append(rawEndpoints, card.Endpoint)
	}
	if len(rawEndpoints) == 0 && card.Transport != nil &&
		(card.Transport.Kind == "streamable-http" || card.Transport.Kind == "streamable_http") {
		rawEndpoints = append(rawEndpoints, card.Transport.Endpoint)
	}
	var endpoint *url.URL
	for _, raw := range rawEndpoints {
		candidate, err := source.Parse(raw)
		if err == nil {
			if _, err = mcpHTTPClient(ctx, candidate.String(), nil, SecretMaterial{}); err == nil {
				endpoint = candidate
				break
			}
		}
	}
	if endpoint == nil {
		return ServiceCardResult{}, false
	}
	title := card.Title
	if title == "" {
		title = card.Name
	}
	if title == "" && card.ServerInfo != nil {
		title = card.ServerInfo.Title
		if title == "" {
			title = card.ServerInfo.Name
		}
	}
	title = normalizeCardText(title, 192)
	if title == "" {
		title = service.Hostname()
	}
	return ServiceCardResult{ServiceURL: service.String(), CardURL: source.String(), DisplayName: title,
		Description: normalizeCardText(card.Description, 192), EndpointURL: endpoint.String()}, true
}

func normalizeCardText(value string, limit int) string {
	value = strings.Join(strings.FieldsFunc(value, func(r rune) bool { return r < ' ' && !strings.ContainsRune("\t\r\n", r) }), " ")
	value = strings.Join(strings.Fields(value), " ")
	runes := []rune(value)
	if len(runes) > limit {
		value = string(runes[:limit-3]) + "..."
	}
	return value
}

func oauthAvailable(ctx context.Context, endpoint string) bool {
	base, err := url.Parse(endpoint)
	if err != nil {
		return false
	}
	paths := []string{"/.well-known/oauth-protected-resource"}
	if path := strings.Trim(base.Path, "/"); path != "" {
		paths = append([]string{"/.well-known/oauth-protected-resource/" + path}, paths...)
		segments := strings.Split(path, "/")
		for end := len(segments) - 1; end > 0; end-- {
			paths = append(paths, "/"+strings.Join(segments[:end], "/")+"/.well-known/oauth-protected-resource")
		}
	}
	seen := make(map[string]struct{}, len(paths))
	for _, path := range paths {
		if _, ok := seen[path]; ok {
			continue
		}
		seen[path] = struct{}{}
		candidate := *base
		candidate.Path, candidate.RawQuery, candidate.Fragment = path, "", ""
		body, state := fetchServerCard(ctx, &candidate)
		if state != "ok" {
			continue
		}
		var metadata struct {
			Resource             string   `json:"resource"`
			AuthorizationServer  string   `json:"authorization_server"`
			AuthorizationServers []string `json:"authorization_servers"`
		}
		if json.Unmarshal(body, &metadata) == nil && metadata.Resource != "" &&
			(metadata.AuthorizationServer != "" || len(metadata.AuthorizationServers) != 0) {
			return true
		}
	}
	return false
}
