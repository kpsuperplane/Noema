package runtime

import (
	"context"
	"encoding/json"
	"strings"

	"github.com/kpsuperplane/noema/internal/adapter"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/webtool"
)

var (
	readMemoryPageSchema = json.RawMessage(`{
  "type":"object",
  "properties":{"page":{"type":"string"}},
  "required":["page"],
  "additionalProperties":false
}`)
	searchMemorySchema = json.RawMessage(`{
  "type":"object",
  "properties":{"query":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":16}},
  "required":["query"],
  "additionalProperties":false
}`)
	connectMCPServiceSchema = json.RawMessage(`{"type":"object","properties":{"service_url":{"type":"string","maxLength":4096}},"required":["service_url"],"additionalProperties":false}`)
)

func localChatTools() []provider.GenerationTool {
	result := []provider.GenerationTool{
		{
			Name:        noemamemory.ReadPageToolName,
			Description: "Read one Memory page by an exact path or ID from the root or page hierarchy.",
			InputSchema: append(json.RawMessage(nil), readMemoryPageSchema...),
		},
		{
			Name:        noemamemory.SearchToolName,
			Description: "Search Memory when no clear page exists or the question spans pages.",
			InputSchema: append(json.RawMessage(nil), searchMemorySchema...),
		},
		fileParseTool(),
		updateOwnNameTool(),
		{Name: artifactCreateLocalName, Description: "Create a durable local file artifact owned by the current conversation.", InputSchema: append(json.RawMessage(nil), artifactCreateLocalSchema...)},
		luaRunTool(),
		fileDownloadTool(),
		presentMultipleChoiceTool(),
		presentA2UITool(),
	}
	for _, tool := range taskToolSpecs {
		result = append(result, tool)
	}
	return append(result, projectToolSpecs...)
}

func supportsLocalChatTool(name string) bool {
	return isPrimaryTaskTool(name) || isProjectTool(name) || name == updateOwnNameToolName || name == luaRunName || name == artifactCreateLocalName || name == presentMultipleChoiceName || name == presentA2UIName || name == fileDownloadName || name == fileParseName ||
		name == noemamemory.ReadPageToolName || name == noemamemory.SearchToolName
}

func (c *Chat) chatTools(ctx context.Context) ([]provider.GenerationTool, error) {
	result := localChatTools()
	if c.web != nil && c.web.Explicit(ctx) {
		result = append(result, webtool.Tools...)
	}
	if c.web != nil && c.web.BrowserAvailable(ctx) {
		result = append(result, webtool.BrowserTools...)
	}
	if c.adapters != nil {
		result = append(result, c.adapters.SetupTools()...)
		bindings, err := c.adapters.Bindings()
		if err != nil {
			return nil, err
		}
		result = append(result, adapter.GenerationTools(bindings)...)
	}
	if c.mcp == nil {
		return result, nil
	}
	bindings, err := c.modelMCPBindings(ctx)
	if err != nil {
		return nil, err
	}
	for _, binding := range bindings {
		result = append(result, provider.GenerationTool{Name: binding.ModelName, Description: binding.Binding.Description,
			InputSchema: append(json.RawMessage(nil), binding.Binding.InputSchema...)})
	}
	return result, nil
}

func (c *Chat) supportsChatTool(ctx context.Context, name string) bool {
	if c.web != nil && c.web.Explicit(ctx) && (name == webtool.SearchName || name == webtool.FetchName) {
		return true
	}
	if c.web != nil && c.web.BrowserAvailable(ctx) && webtool.IsBrowserTool(name) {
		return true
	}
	if supportsLocalChatTool(name) {
		return true
	}
	if c.adapters != nil {
		if name == adapter.DefinitionTemplateTool || name == adapter.ProposeDefinitionTool {
			return true
		}
		if _, err := c.adapters.Binding(name); err == nil {
			return true
		}
	}
	if c.mcp == nil {
		return false
	}
	_, err := c.modelMCPBinding(ctx, name)
	return err == nil
}

type modelMCPBinding struct {
	ModelName string
	Binding   noemamcp.Binding
}

// modelMCPBindings maps durable MCP authorities to the short, stable names
// exposed to the model. The service keeps its canonical binding names for
// action records, review, and stale-authority checks.
func (c *Chat) modelMCPBindings(ctx context.Context) ([]modelMCPBinding, error) {
	if c.mcp == nil {
		return nil, nil
	}
	bindings, err := c.mcp.Bindings(ctx)
	if err != nil {
		return nil, err
	}
	discovered := false
	for _, binding := range bindings {
		if binding.Name != noemamcp.ConnectServiceToolName {
			discovered = true
			break
		}
	}
	result := make([]modelMCPBinding, 0, len(bindings))
	used := make(map[string]int)
	for _, binding := range bindings {
		if binding.Name == noemamcp.ConnectServiceToolName {
			if discovered {
				continue
			}
			result = append(result, modelMCPBinding{ModelName: binding.Name, Binding: binding})
			continue
		}
		server, serverErr := c.database.MCPServer(ctx, binding.ServerID)
		if serverErr != nil {
			return nil, serverErr
		}
		name := modelMCPBindingName(binding, server.DisplayName)
		used[name]++
		if used[name] > 1 {
			name += "-" + itoa(used[name])
		}
		result = append(result, modelMCPBinding{ModelName: name, Binding: binding})
	}
	return result, nil
}

func (c *Chat) modelMCPBinding(ctx context.Context, name string) (noemamcp.Binding, error) {
	bindings, err := c.modelMCPBindings(ctx)
	if err != nil {
		return noemamcp.Binding{}, err
	}
	for _, binding := range bindings {
		if binding.ModelName == name {
			return binding.Binding, nil
		}
	}
	return noemamcp.Binding{}, noemamcp.ErrUnknownOperation
}

func modelMCPToolName(displayName, operation string) string {
	slug := strings.ToLower(strings.TrimSpace(displayName))
	var builder strings.Builder
	separator := false
	for _, runeValue := range slug {
		if runeValue >= 'a' && runeValue <= 'z' || runeValue >= '0' && runeValue <= '9' {
			builder.WriteRune(runeValue)
			separator = false
			continue
		}
		if builder.Len() != 0 && !separator {
			builder.WriteByte('-')
			separator = true
		}
	}
	slug = strings.TrimSuffix(builder.String(), "-")
	if slug == "" {
		slug = "service"
	}
	return "mcp.mcp:" + slug + "." + operation
}

func modelMCPBindingName(binding noemamcp.Binding, displayName string) string {
	name := modelMCPToolName(displayName, binding.OperationToken)
	if strings.HasPrefix(binding.Name, "enable.") {
		return "enable." + name
	}
	return name
}

func (c *Chat) executeChatTool(
	ctx context.Context,
	conversation store.Conversation,
	name string,
	arguments json.RawMessage,
	requestID string,
	turnID string,
	details ...chatTaskToolDetails,
) (json.RawMessage, bool) {
	correlationID := ""
	if turnID != "" {
		correlationID = "correlation:turn:" + turnID
	}
	if isPrimaryTaskTool(name) {
		detail := chatTaskToolDetails{TimeZone: "UTC"}
		if len(details) != 0 {
			detail = details[0]
		}
		return c.executePrimaryTaskTool(ctx, name, requestID, correlationID, arguments,
			store.ArtifactSource{ConversationID: conversation.ID, TurnID: turnID, ItemID: detail.SourceItemID}, detail.TimeZone)
	}
	if isProjectTool(name) {
		return c.executeProjectTool(ctx, name, requestID, correlationID, arguments)
	}
	switch name {
	case webtool.SearchName, webtool.FetchName:
		if c.web != nil {
			return c.web.Execute(ctx, name, arguments, "conversation:"+conversation.ID+":"+requestID)
		}
		return toolFailure("unavailable", "web tool is unavailable"), false
	case webtool.BrowseOpenName, webtool.BrowseSnapshotName, webtool.BrowseInteractName, webtool.BrowseWaitName,
		webtool.BrowseHistoryName, webtool.BrowseSwitchName, webtool.BrowseCloseName:
		if c.web != nil {
			result := c.web.ExecuteBrowser(ctx, chatBrowserOwner(conversation.ID), name, arguments, "conversation:"+conversation.ID+":"+requestID)
			return result.Model, result.Success
		}
		return toolFailure("unavailable", "browser is unavailable"), false
	case updateOwnNameToolName:
		return c.updateOwnName(ctx, arguments)
	case luaRunName:
		return executeLuaTool(ctx, arguments)
	case fileParseName:
		return c.parseFileTool(ctx, conversation, arguments)
	case noemamemory.ReadPageToolName:
		return c.readMemoryPage(arguments)
	case noemamemory.SearchToolName:
		return c.searchMemory(arguments)
	case noemamcp.ConnectServiceToolName:
		return c.connectMCPService(ctx, arguments)
	case presentMultipleChoiceName:
		if _, err := parseMultipleChoiceArguments(arguments); err != nil {
			return toolFailure("invalid_arguments", err.Error()), false
		}
		return json.RawMessage(`{"status":"displayed"}`), true
	case presentA2UIName:
		jsonl, err := parseA2UIArguments(arguments)
		if err != nil {
			payload, _ := json.Marshal(map[string]any{"status": "VALIDATION_FAILED", "message": err.Error()})
			return payload, false
		}
		batch, repair := reduceA2UI(conversation.ID, jsonl)
		if repair != nil {
			payload, _ := json.Marshal(repair)
			return payload, false
		}
		for _, surface := range batch.Surfaces {
			if len(surface.Actions) != 0 {
				return toolFailure("invalid_input", "action-bearing A2UI call was not intercepted"), false
			}
		}
		payload, _ := json.Marshal(map[string]any{"status": "published", "surface_count": len(batch.Surfaces)})
		return payload, true
	case adapter.DefinitionTemplateTool, adapter.ProposeDefinitionTool:
		if c.adapters != nil {
			return c.adapters.ExecuteSetup(name, arguments)
		}
		return toolFailure("invalid_input", "adapter service is unavailable"), false
	default:
		return toolFailure("invalid_input", "tool is unavailable"), false
	}
}

type chatTaskToolDetails struct {
	SourceItemID string
	TimeZone     string
}

func taskToolDetails(request queuedTurn) chatTaskToolDetails {
	zone := "UTC"
	if request.location != nil {
		zone = request.location.String()
	}
	return chatTaskToolDetails{SourceItemID: request.sourceItemID, TimeZone: zone}
}

func (c *Chat) connectMCPService(ctx context.Context, arguments json.RawMessage) (json.RawMessage, bool) {
	var input struct {
		ServiceURL string `json:"service_url"`
	}
	if c.mcp == nil || decodeToolArguments(arguments, &input) != nil || input.ServiceURL == "" || len(input.ServiceURL) > 4096 {
		return toolFailure("invalid_input", "MCP service URL is invalid"), false
	}
	result := c.mcp.ConnectService(ctx, input.ServiceURL)
	payload := map[string]any{"status": result.Status, "service_url": result.ServiceURL}
	if result.DisplayName != "" {
		payload["server_card_url"], payload["display_name"] = result.CardURL, result.DisplayName
		payload["description"], payload["endpoint_url"] = result.Description, result.EndpointURL
		payload["setup_result"] = map[string]any{"setup_status": result.Setup.Status,
			"discovered_tool_count": result.Setup.Discovered, "server": setupServerValue(result.Setup.Server)}
	}
	encoded, _ := json.Marshal(payload)
	return encoded, true
}

func setupServerValue(server *store.MCPServer) any {
	if server == nil {
		return nil
	}
	return map[string]any{"mcp_server_id": server.ID, "connection_revision": server.ConnectionRevision,
		"policy_revision": server.PolicyRevision, "tool_count": server.ToolCount}
}

func (c *Chat) readMemoryPage(arguments json.RawMessage) (json.RawMessage, bool) {
	var fields map[string]json.RawMessage
	if decodeToolArguments(arguments, &fields) != nil || len(fields) != 1 {
		return toolFailure("invalid_input", "read_memory_page arguments are invalid"), false
	}
	var selector string
	if raw, exists := fields["page"]; !exists || json.Unmarshal(raw, &selector) != nil {
		return toolFailure("invalid_input", "read_memory_page requires a string page"), false
	}
	page, err := c.memory.ReadPage(selector)
	if err != nil {
		payload, _ := json.Marshal(map[string]any{"error": err.Error()})
		return payload, false
	}
	payload, _ := json.Marshal(map[string]any{"page": memoryPageValue(page)})
	return payload, true
}

func (c *Chat) searchMemory(arguments json.RawMessage) (json.RawMessage, bool) {
	var fields map[string]json.RawMessage
	decodeErr := decodeToolArguments(arguments, &fields)
	_, hasLimit := fields["limit"]
	if decodeErr != nil || len(fields) < 1 || len(fields) > 2 || len(fields) == 2 && !hasLimit {
		return toolFailure("invalid_input", "search_memory arguments are invalid"), false
	}
	var query string
	if raw, exists := fields["query"]; !exists || json.Unmarshal(raw, &query) != nil {
		return toolFailure("invalid_input", "search_memory requires a string query"), false
	}
	limit := 8
	if raw, exists := fields["limit"]; exists {
		if json.Unmarshal(raw, &limit) != nil || limit < 1 || limit > 16 {
			return toolFailure("invalid_input", "search_memory limit must be from 1 through 16"), false
		}
	}
	results, err := c.memory.Search(query, limit)
	if err != nil {
		payload, _ := json.Marshal(map[string]any{"error": err.Error()})
		return payload, false
	}
	payload, _ := json.Marshal(map[string]any{"scope_id": noemamemory.Scope, "pages": results})
	return payload, true
}

func memoryPageValue(page noemamemory.Page) map[string]any {
	var parent any
	if page.Parent != "" {
		parent = page.Parent
	}
	return map[string]any{
		"id": page.ID, "path": page.Path, "title": page.Title, "icon": page.Icon,
		"body": page.Body, "hash": page.Hash, "citations": page.Citations,
		"parent": parent, "ancestors": page.Ancestors, "children": page.Children,
	}
}

func developerMessages(environment, memoryContext, projectContext string, hostedWeb bool) []provider.GenerationMessage {
	messages := make([]provider.GenerationMessage, 0, 3)
	if strings.TrimSpace(memoryContext) != "" {
		messages = append(messages, provider.GenerationMessage{
			Role: "developer", Content: "Native local-human memory (source root page):\n" + memoryContext,
		})
	}
	if projectContext != "" {
		messages = append(messages, provider.GenerationMessage{Role: "developer", Content: projectContext})
	}
	if hostedWeb {
		environment += "\n\nAvailable provider tool:\n- provider_native\tweb_search\tSearch the live public web through the active model provider."
	}
	return append(messages, provider.GenerationMessage{Role: "developer", Content: environment})
}

func (c *Chat) memoryRootContext() string {
	root, err := c.memory.ReadRoot()
	if err != nil {
		return ""
	}
	var rendered strings.Builder
	rendered.WriteString(root.Body)
	if len(root.Children) != 0 {
		rendered.WriteString("\n\nDirect child pages:\n")
		for _, child := range root.Children {
			_, _ = rendered.WriteString("- " + child.Title + " (" + child.Path + ", " + child.ID + ")\n")
		}
	}
	return rendered.String()
}
