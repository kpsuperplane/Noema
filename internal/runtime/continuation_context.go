package runtime

import (
	"encoding/json"
	"fmt"

	"github.com/kpsuperplane/noema/internal/provider"
)

// ContinuationToolResult is one completed local tool result that can be sent
// back to the provider on the next continuation.
type ContinuationToolResult struct {
	CallID, ProviderCallID string
	Name, ProviderName     string
	Arguments              json.RawMessage
	Success                bool
	Payload                json.RawMessage
}

type continuationInputItem struct {
	kind    string
	message provider.GenerationMessage
}

// ContinuationContext is the ordered provider context for one tool-using
// execution. It keeps provider response items separate until the provider
// boundary maps them to generation messages.
type ContinuationContext struct {
	items                  []continuationInputItem
	pendingCallIDs         []string
	continuationDeltaStart int
	checkpoint             string
	requiresReplay         bool
	roundEnds              []int
	awaitingProvider       bool
	nextSyntheticCall      int
}

// NewContinuationContext starts one context from the exact provider messages
// used for the initial request.
func NewContinuationContext(input []provider.GenerationMessage) *ContinuationContext {
	items := make([]continuationInputItem, 0, len(input))
	for _, message := range input {
		items = append(items, continuationInputItem{kind: continuationKind(message), message: message})
	}
	return &ContinuationContext{items: items, continuationDeltaStart: len(items)}
}

func continuationKind(message provider.GenerationMessage) string {
	switch message.Role {
	case "hosted_web_search":
		return "hosted_web_search"
	case "tool":
		return "tool_result"
	case "assistant":
		if len(message.ToolCalls) != 0 {
			return "tool_call"
		}
		if len(message.ReasoningDetails) != 0 || message.ReasoningID != "" || message.EncryptedReasoning != "" {
			return "reasoning"
		}
		return "assistant_text"
	default:
		return "message"
	}
}

// AppendResponse records one provider response in provider-visible order.
func (c *ContinuationContext) AppendResponse(response provider.GenerationResult) {
	c.awaitingProvider = false
	c.requiresReplay = false
	for _, reasoning := range response.Reasoning {
		if reasoning.EncryptedContent == "" && len(reasoning.ProviderDetails) == 0 && reasoning.ID == "" {
			continue
		}
		c.items = append(c.items, continuationInputItem{kind: "reasoning", message: provider.GenerationMessage{
			Role: "assistant", ReasoningID: reasoning.ID, EncryptedReasoning: reasoning.EncryptedContent,
			ReasoningDetails: append([]json.RawMessage(nil), reasoning.ProviderDetails...),
		}})
	}
	for _, search := range response.Searches {
		searchCopy := search
		c.items = append(c.items, continuationInputItem{kind: "hosted_web_search", message: provider.GenerationMessage{Role: "hosted_web_search", HostedSearch: &searchCopy}})
	}
	if response.Text != "" || len(response.ToolCalls) == 0 {
		c.items = append(c.items, continuationInputItem{kind: "assistant_text", message: provider.GenerationMessage{
			Role: "assistant", Content: response.Text, ReasoningDetails: generationReasoning(response),
		}})
	}
	for _, call := range response.ToolCalls {
		callID := call.ProviderCallID
		if callID == "" {
			callID = call.ProviderItemID
		}
		if callID == "" {
			callID = c.syntheticCallID()
		}
		c.pendingCallIDs = append(c.pendingCallIDs, callID)
		c.items = append(c.items, continuationInputItem{kind: "tool_call", message: provider.GenerationMessage{
			Role: "assistant", ToolCalls: []provider.ReplayToolCall{{
				ProviderItemID: call.ProviderItemID, ProviderCallID: callID, ProviderName: call.ProviderName,
				Name: call.Name, Arguments: append(json.RawMessage(nil), call.Payload...),
			}},
		}})
	}
	c.continuationDeltaStart = len(c.items)
}

// AppendResults records local tool results and bounds their model-facing
// payload before any provider request can use them.
func (c *ContinuationContext) AppendResults(results []ContinuationToolResult) {
	for _, result := range results {
		callID := result.ProviderCallID
		if callID == "" {
			callID = result.CallID
		}
		if callID == "" && len(c.pendingCallIDs) != 0 {
			callID = c.pendingCallIDs[0]
		}
		if len(c.pendingCallIDs) != 0 && c.pendingCallIDs[0] == callID {
			c.pendingCallIDs = c.pendingCallIDs[1:]
		}
		if callID == "" {
			callID = c.syntheticCallID()
		}
		c.items = append(c.items, continuationInputItem{kind: "tool_result", message: provider.GenerationMessage{
			Role: "tool", ToolResult: &provider.ReplayToolResult{
				ProviderCallID: callID, Name: result.Name, ProviderName: result.ProviderName,
				Arguments: append(json.RawMessage(nil), result.Arguments...), Success: result.Success,
				Payload: boundedModelToolPayload(stripContinuationScreenshots(result.Payload), modelToolResultLimit),
			},
		}})
	}
}

func stripContinuationScreenshots(payload json.RawMessage) json.RawMessage {
	var value any
	if json.Unmarshal(payload, &value) != nil {
		return append(json.RawMessage(nil), payload...)
	}
	var strip func(any)
	strip = func(current any) {
		switch object := current.(type) {
		case map[string]any:
			delete(object, "screenshot")
			delete(object, "screenshots")
			for _, child := range object {
				strip(child)
			}
		case []any:
			for _, child := range object {
				strip(child)
			}
		}
	}
	strip(value)
	encoded, err := json.Marshal(value)
	if err != nil {
		return append(json.RawMessage(nil), payload...)
	}
	return encoded
}

// AppendDeveloperMessage adds trusted runtime state after a local state change.
func (c *ContinuationContext) AppendDeveloperMessage(content string) {
	if content == "" {
		return
	}
	c.items = append(c.items, continuationInputItem{kind: "message", message: provider.GenerationMessage{Role: "developer", Content: content}})
}

// FinishRound closes one provider response and clears unresolved call matching.
func (c *ContinuationContext) FinishRound() {
	c.roundEnds = append(c.roundEnds, len(c.items))
	c.awaitingProvider = true
	c.pendingCallIDs = nil
}

// ProviderInput returns the ordered production input items before provider
// specific message mapping. nativeHistory selects the same structured input
// authority used by native provider sessions; both modes retain item order.
func (c *ContinuationContext) ProviderInput(nativeHistory bool) []continuationInputItem {
	_ = nativeHistory
	items := make([]continuationInputItem, 0, len(c.items)+1)
	if c.checkpoint != "" {
		items = append(items, continuationInputItem{kind: "message", message: provider.GenerationMessage{Role: "system", Content: "Noema execution context checkpoint:\n" + c.checkpoint}})
	}
	items = append(items, c.items...)
	return items
}

// providerInput is the package-local spelling used by runtime admission code.
func (c *ContinuationContext) providerInput(nativeHistory bool) []continuationInputItem {
	return c.ProviderInput(nativeHistory)
}

// ProviderMessages maps ordered continuation input to provider-neutral messages.
func (c *ContinuationContext) ProviderMessages(nativeHistory bool) []provider.GenerationMessage {
	items := c.ProviderInput(nativeHistory)
	messages := make([]provider.GenerationMessage, 0, len(items))
	for _, item := range items {
		messages = append(messages, item.message)
	}
	return messages
}

// IncrementalInput returns only the post-response items when no replay is
// required. It preserves native tool results as a provider-neutral message set.
func (c *ContinuationContext) IncrementalInput() []provider.GenerationMessage {
	if c.requiresReplay || c.continuationDeltaStart > len(c.items) {
		return nil
	}
	items := c.items[c.continuationDeltaStart:]
	result := make([]provider.GenerationMessage, 0, len(items))
	for _, item := range items {
		result = append(result, item.message)
	}
	return result
}

// AdmissionMessages returns the bounded provider-visible context. It exists
// as the single runtime boundary used before context sizing and dispatch.
func (c *ContinuationContext) AdmissionMessages() ([]provider.GenerationMessage, error) {
	items := c.providerInput(true)
	for _, item := range items {
		if item.kind == "tool_result" && item.message.ToolResult == nil {
			return nil, fmt.Errorf("continuation tool result is invalid")
		}
	}
	return c.ProviderMessages(true), nil
}

func (c *ContinuationContext) syntheticCallID() string {
	callID := fmt.Sprintf("noema_continuation_call_%d", c.nextSyntheticCall)
	c.nextSyntheticCall++
	return callID
}
