package runtime

import (
	"context"
	"encoding/json"
	"strings"

	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
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
)

func chatTools() []provider.GenerationTool {
	return []provider.GenerationTool{
		fileParseTool(),
		taskInspectTool(),
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
	}
}

func supportsChatTool(name string) bool {
	return name == fileParseName || name == taskInspectName ||
		name == noemamemory.ReadPageToolName || name == noemamemory.SearchToolName
}

func (c *Chat) executeChatTool(
	ctx context.Context,
	conversation store.Conversation,
	name string,
	arguments json.RawMessage,
) (json.RawMessage, bool) {
	switch name {
	case fileParseName:
		return c.parseFileTool(ctx, conversation, arguments)
	case taskInspectName:
		return c.inspectTask(ctx, arguments)
	case noemamemory.ReadPageToolName:
		return c.readMemoryPage(arguments)
	case noemamemory.SearchToolName:
		return c.searchMemory(arguments)
	default:
		return toolFailure("invalid_input", "tool is unavailable"), false
	}
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

func developerMessages(environment, memoryContext string, hostedWeb bool) []provider.GenerationMessage {
	messages := make([]provider.GenerationMessage, 0, 2)
	if strings.TrimSpace(memoryContext) != "" {
		messages = append(messages, provider.GenerationMessage{
			Role: "developer", Content: "Native local-human memory (source root page):\n" + memoryContext,
		})
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
