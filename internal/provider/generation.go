package provider

import (
	"context"
	"encoding/json"
	"errors"
	"strings"
)

// ErrGenerationRequestTooLarge means local replay exceeded a provider request bound.
var ErrGenerationRequestTooLarge = errors.New("provider generation request is too large")

var (
	// ErrGenerationToolNameInvalid identifies a non-canonical tool name.
	ErrGenerationToolNameInvalid = errors.New("tool name is invalid")
	// ErrGenerationToolDescriptionInvalid identifies a missing model description.
	ErrGenerationToolDescriptionInvalid = errors.New("tool description is required")
	// ErrGenerationToolSchemaInvalid identifies a non-object input schema.
	ErrGenerationToolSchemaInvalid = errors.New("tool input schema root must be an object")
)

// ToolName is one validated canonical Noema tool name.
type ToolName string

// NewToolName validates and constructs one canonical tool name.
func NewToolName(value string) (ToolName, error) {
	if err := validateGenerationToolName(value); err != nil {
		return "", err
	}
	return ToolName(value), nil
}

// String returns the canonical tool name.
func (name ToolName) String() string { return string(name) }

// UnmarshalJSON keeps the validation boundary active for decoded names.
func (name *ToolName) UnmarshalJSON(raw []byte) error {
	var value string
	if err := json.Unmarshal(raw, &value); err != nil {
		return ErrGenerationToolNameInvalid
	}
	validated, err := NewToolName(value)
	if err != nil {
		return err
	}
	*name = validated
	return nil
}

// Generator performs one provider-neutral model generation.
type Generator interface {
	Generate(context.Context, GenerateRequest, func(StreamEvent)) (GenerationResult, error)
}

// GenerationSession reuses one provider connection for related generations.
type GenerationSession interface {
	Generator
	Close() error
}

// SessionGenerator opens a bounded provider generation session.
type SessionGenerator interface {
	OpenGenerationSession() GenerationSession
}

// ContinuationSession reports whether one live session can accept incremental input.
type ContinuationSession interface {
	ContinuationReady(string) bool
}

// GenerationMessage is one provider-neutral history message.
type GenerationMessage struct {
	Role, Content                   string
	ToolCalls                       []ReplayToolCall
	ToolResult                      *ReplayToolResult
	HostedSearch                    *HostedSearch
	ToolCallID                      string
	ReasoningDetails                []json.RawMessage
	ReasoningID, EncryptedReasoning string
}

// ReplayToolCall is one native tool call in replay history.
type ReplayToolCall struct {
	ProviderItemID, ProviderCallID string
	Name, ProviderName             string
	Arguments                      json.RawMessage
}

// ReplayToolResult is one native tool result in replay history.
type ReplayToolResult struct {
	ProviderCallID, Name, ProviderName string
	Arguments                          json.RawMessage
	Success                            bool
	Payload                            json.RawMessage
}

// GenerateRequest is one provider-neutral generation request.
type GenerateRequest struct {
	AccountID, Model string
	Messages         []GenerationMessage
	// ReplayMessages supplies full local replay when a session sends incremental input.
	ReplayMessages             []GenerationMessage
	PreviousResponseID         string
	StoreResponse              bool
	ExpectedCredentialRevision *uint64
	ReasoningEffort            string
	MaxOutputTokens            *uint32
	Temperature                *float32
	ConversationID             string
	Tools                      []GenerationTool
	ToolTransport              ToolTransport
	ToolChoice                 ToolChoice
	ParallelTools              bool
	HostedWebSearch            bool
	FastMode                   bool
}

func generationPriority(request GenerateRequest) int {
	if request.FastMode {
		return 1
	}
	return 0
}

// GenerationResult is one completed provider response.
type GenerationResult struct {
	ID           string
	Model        string
	Text         string
	FinishReason string
	Usage        Usage
	ToolCalls    []GenerationToolCall
	Reasoning    []GenerationReasoning
	Citations    []Citation
	Searches     []HostedSearch
}

// GenerationToolCall is one validated native tool call.
type GenerationToolCall struct {
	Index                          int
	ProviderItemID, ProviderCallID string
	ProviderName, Name             string
	Payload                        json.RawMessage
}

// GenerationReasoning holds provider reasoning for replay.
type GenerationReasoning struct {
	ID               string
	EncryptedContent string
	Summary          []string
	ProviderDetails  []json.RawMessage
}

// ToolTransport selects the model tool channel.
type ToolTransport string

const (
	// ToolTransportNone disables native tools.
	ToolTransportNone ToolTransport = "none"
	// ToolTransportNative enables native function tools.
	ToolTransportNative ToolTransport = "native"
)

// ToolChoice selects provider tool use.
type ToolChoice string

const (
	ToolChoiceAuto     ToolChoice = "auto"
	ToolChoiceNone     ToolChoice = "none"
	ToolChoiceRequired ToolChoice = "required"
)

// GenerationTool is one source model-visible function.
type GenerationTool struct {
	Name        string          `json:"name"`
	Description string          `json:"description"`
	InputSchema json.RawMessage `json:"input_schema"`
}

// ToolSchema preserves one provider-neutral JSON schema without mapping it
// through a Go object model.
type ToolSchema struct {
	raw json.RawMessage
}

// NewInputToolSchema validates one model input schema and keeps its exact JSON.
func NewInputToolSchema(toolName string, raw json.RawMessage) (ToolSchema, error) {
	if err := validateGenerationToolName(toolName); err != nil {
		return ToolSchema{}, err
	}
	var object map[string]any
	if json.Unmarshal(raw, &object) != nil || object["type"] != "object" {
		return ToolSchema{}, ErrGenerationToolSchemaInvalid
	}
	return ToolSchema{raw: append(json.RawMessage(nil), raw...)}, nil
}

// MarshalJSON writes the schema as its raw object value.
func (schema ToolSchema) MarshalJSON() ([]byte, error) {
	if len(schema.raw) == 0 {
		return []byte("null"), nil
	}
	return schema.raw, nil
}

// UnmarshalJSON validates and preserves one raw object schema.
func (schema *ToolSchema) UnmarshalJSON(raw []byte) error {
	var object map[string]any
	if json.Unmarshal(raw, &object) != nil || object["type"] != "object" {
		return ErrGenerationToolSchemaInvalid
	}
	schema.raw = append(schema.raw[:0], raw...)
	return nil
}

// Validate checks the provider-neutral tool contract before a provider maps it
// into its own request format.
func (tool GenerationTool) Validate() error {
	if _, err := NewToolName(tool.Name); err != nil {
		return err
	}
	if strings.TrimSpace(tool.Description) == "" {
		return ErrGenerationToolDescriptionInvalid
	}
	var schema map[string]any
	if json.Unmarshal(tool.InputSchema, &schema) != nil || schema["type"] != "object" {
		return ErrGenerationToolSchemaInvalid
	}
	return nil
}

func validateGenerationToolName(name string) error {
	if name == "" || strings.TrimSpace(name) != name {
		return ErrGenerationToolNameInvalid
	}
	for _, character := range name {
		if !((character >= 'a' && character <= 'z') ||
			(character >= 'A' && character <= 'Z') ||
			(character >= '0' && character <= '9') ||
			character == '_' || character == '-' || character == '.' || character == ':') {
			return ErrGenerationToolNameInvalid
		}
	}
	for _, segment := range strings.Split(name, ".") {
		valid := false
		for _, character := range segment {
			if (character >= 'a' && character <= 'z') ||
				(character >= 'A' && character <= 'Z') ||
				(character >= '0' && character <= '9') {
				valid = true
				break
			}
		}
		if !valid {
			return ErrGenerationToolNameInvalid
		}
	}
	return nil
}
