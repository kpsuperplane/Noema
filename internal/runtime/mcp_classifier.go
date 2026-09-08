package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	"sort"
	"strings"
	"unicode"
)

// MCPToolClassifier fills only behavior hints absent from source annotations.
func (c *Chat) MCPToolClassifier() func(context.Context, store.MCPTool) ([4]bool, error) {
	return func(ctx context.Context, tool store.MCPTool) ([4]bool, error) {
		assignment, err := c.actionReviewerAssignment(ctx)
		if err != nil {
			return [4]bool{}, err
		}
		generator, err := c.generatorFor(assignment.ProviderKind)
		if err != nil {
			return [4]bool{}, err
		}
		limit := uint32(512)
		result, err := generator.Generate(ctx, provider.GenerateRequest{
			AccountID: assignment.ProviderAccountID, Model: assignment.ModelProfile,
			Messages:        []provider.GenerationMessage{{Role: "user", Content: mcpClassificationPrompt(tool)}},
			ReasoningEffort: string(assignment.ReasoningEffort), MaxOutputTokens: &limit,
			ToolTransport: provider.ToolTransportNone, ToolChoice: provider.ToolChoiceNone, FastMode: assignment.FastMode,
		}, func(provider.StreamEvent) {})
		if err != nil || len(result.ToolCalls) != 0 {
			return [4]bool{}, errors.New("MCP tool classification is unavailable")
		}
		completion, err := mcp.ParseToolClassificationResponse(result.Text, tool)
		if err != nil {
			return [4]bool{}, err
		}
		tool = mcp.ApplyToolClassification(tool, completion)
		return [4]bool{*tool.ReadOnly.Value, *tool.Idempotent.Value, *tool.Destructive.Value, *tool.OpenWorld.Value}, nil
	}
}

func mcpClassificationPrompt(tool store.MCPTool) string {
	missing := []string{}
	for i, hint := range []store.MCPHint{tool.ReadOnly, tool.Idempotent, tool.Destructive, tool.OpenWorld} {
		if hint.Value == nil {
			missing = append(missing, []string{"readOnly", "idempotent", "destructive", "openWorld"}[i])
		}
	}
	description := tool.Description
	if description == "" {
		description = "-"
	}
	return fmt.Sprintf("Classify only these missing tool behavior hints: %s.\nReturn one strict JSON object using only the corresponding camelCase keys and boolean values.\nreadOnly=no environment mutation; idempotent=repeating identical arguments adds no effect; destructive=may overwrite/delete; openWorld=may interact with external entities.\ntool=%s\ndescription=%s\ninput_fields=%s\noutput_fields=%s", strings.Join(missing, ","), sanitizeClassificationText(tool.Name, 128), sanitizeClassificationText(description, 256), classificationSchemaFields(tool.InputSchema), classificationSchemaFields(tool.OutputSchema))
}

func sanitizeClassificationText(value string, limit int) string {
	runes := []rune{}
	for _, r := range value {
		if !unicode.IsControl(r) {
			runes = append(runes, r)
			if len(runes) == limit {
				break
			}
		}
	}
	return strings.Join(strings.Fields(string(runes)), " ")
}

func classificationSchemaFields(raw json.RawMessage) string {
	var schema struct {
		Properties map[string]json.RawMessage `json:"properties"`
	}
	if json.Unmarshal(raw, &schema) != nil || len(schema.Properties) == 0 {
		return "-"
	}
	keys := make([]string, 0, len(schema.Properties))
	for key := range schema.Properties {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for i, key := range keys {
		keys[i] = sanitizeClassificationText(key, 64)
	}
	fields := strings.Join(keys, ",")
	if fields == "" {
		return "-"
	}
	return fields
}
