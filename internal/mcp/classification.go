package mcp

import (
	"encoding/json"
	"errors"
	"io"
	"strings"

	"github.com/kpsuperplane/noema/internal/store"
)

// ToolHintCompletion is the strict model response for missing MCP behavior hints.
type ToolHintCompletion struct {
	ReadOnly    *bool `json:"readOnly"`
	Idempotent  *bool `json:"idempotent"`
	Destructive *bool `json:"destructive"`
	OpenWorld   *bool `json:"openWorld"`
}

var (
	// ErrToolClassificationFieldMismatch means the model returned a field that
	// was already source-owned or omitted one that still required classification.
	ErrToolClassificationFieldMismatch = errors.New("tool-hint response did not match the missing fields")
	ErrToolClassificationJSON          = errors.New("invalid tool-hint JSON")
)

// ParseToolClassificationResponse parses only the fields still marked as safe
// defaults for one exact stored tool.
func ParseToolClassificationResponse(raw string, tool store.MCPTool) (ToolHintCompletion, error) {
	var completion ToolHintCompletion
	decoder := json.NewDecoder(strings.NewReader(strings.TrimSpace(raw)))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&completion); err != nil {
		return ToolHintCompletion{}, errors.Join(ErrToolClassificationJSON, err)
	}
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		if err == nil {
			err = errors.New("trailing JSON")
		}
		return ToolHintCompletion{}, errors.Join(ErrToolClassificationJSON, err)
	}
	expected := [4]bool{
		tool.ReadOnly.Value == nil,
		tool.Idempotent.Value == nil,
		tool.Destructive.Value == nil,
		tool.OpenWorld.Value == nil,
	}
	actual := [4]bool{
		completion.ReadOnly != nil,
		completion.Idempotent != nil,
		completion.Destructive != nil,
		completion.OpenWorld != nil,
	}
	if expected != actual {
		return ToolHintCompletion{}, ErrToolClassificationFieldMismatch
	}
	return completion, nil
}

// ApplyToolClassification merges a validated completion without replacing
// source-owned annotations.
func ApplyToolClassification(tool store.MCPTool, completion ToolHintCompletion) store.MCPTool {
	apply := func(hint *store.MCPHint, value *bool) {
		if hint.Value == nil && value != nil {
			copy := *value
			hint.Value, hint.Source = &copy, "model"
		}
	}
	apply(&tool.ReadOnly, completion.ReadOnly)
	apply(&tool.Idempotent, completion.Idempotent)
	apply(&tool.Destructive, completion.Destructive)
	apply(&tool.OpenWorld, completion.OpenWorld)
	tool.Status = "ready"
	return tool
}

// ApplyToolSafeDefaults fills any missing values with the conservative source
// defaults used before a model classification is available.
func ApplyToolSafeDefaults(tool store.MCPTool) store.MCPTool {
	fill := func(hint *store.MCPHint, value bool) {
		if hint.Value == nil {
			copy := value
			hint.Value, hint.Source = &copy, "safe_default"
		}
	}
	fill(&tool.ReadOnly, false)
	fill(&tool.Idempotent, false)
	fill(&tool.Destructive, true)
	fill(&tool.OpenWorld, true)
	tool.Status = "defaulted"
	return tool
}
