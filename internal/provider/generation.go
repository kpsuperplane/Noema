package provider

import (
	"context"
	"encoding/json"
	"errors"
)

// ErrGenerationRequestTooLarge means local replay exceeded a provider request bound.
var ErrGenerationRequestTooLarge = errors.New("provider generation request is too large")

// Generator performs one provider-neutral model generation.
type Generator interface {
	Generate(context.Context, GenerateRequest, func(StreamEvent)) (GenerationResult, error)
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
	AccountID, Model   string
	Messages           []GenerationMessage
	PreviousResponseID string
	StoreResponse      bool
	ReasoningEffort    string
	MaxOutputTokens    *uint32
	Temperature        *float32
	ConversationID     string
	Tools              []GenerationTool
	ToolTransport      ToolTransport
	ToolChoice         ToolChoice
	ParallelTools      bool
	HostedWebSearch    bool
	FastMode           bool
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
	Name        string
	Description string
	InputSchema json.RawMessage
}
