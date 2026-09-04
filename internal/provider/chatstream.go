// Package provider owns model provider protocols.
package provider

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/url"
	"sort"
	"strings"
)

const (
	maxSSELine  = 1 << 20
	maxSSEEvent = 2 << 20
	maxResult   = 16 << 20
	maxItems    = 4096
)

var errStreamDone = errors.New("provider stream completed")

// StreamEventKind identifies one live provider event.
type StreamEventKind string

const (
	// TextDelta contains new assistant text.
	TextDelta StreamEventKind = "text_delta"
	// ToolCallStarted identifies a complete provider tool call header.
	ToolCallStarted StreamEventKind = "tool_call_started"
	// HostedSearchStarted identifies one provider-hosted search.
	HostedSearchStarted StreamEventKind = "hosted_search_started"
)

// StreamEvent is one normalized live provider event.
type StreamEvent struct {
	Kind  StreamEventKind
	Index int
	ID    string
	Name  string
	Delta string
}

// ToolCall is one assembled native tool call.
type ToolCall struct {
	Index     int
	ID        string
	Name      string
	Arguments string
}

// Citation is one provider citation.
type Citation struct {
	Title      string
	URL        string
	StartIndex *int
	EndIndex   *int
}

// HostedSearch is one provider-hosted web search.
type HostedSearch struct {
	Index     int
	ID        string
	Name      string
	Status    string
	Arguments json.RawMessage
	Result    json.RawMessage
}

// Usage contains normalized provider token counts.
type Usage struct {
	InputTokens       int
	CachedInputTokens int
	OutputTokens      int
	TotalTokens       int
	WebSearchRequests int
}

// ChatStreamResult is one complete normalized chat stream.
type ChatStreamResult struct {
	ID        string
	Model     string
	Text      string
	ToolCalls []ToolCall
	Reasoning []json.RawMessage
	Citations []Citation
	Searches  []HostedSearch
	Usage     Usage
}

// ParseChatStream parses and closes one bounded OpenAI-compatible SSE response.
func ParseChatStream(
	ctx context.Context,
	reader io.ReadCloser,
	onEvent func(StreamEvent),
) (ChatStreamResult, error) {
	if onEvent == nil {
		onEvent = func(StreamEvent) {}
	}
	stopClose := context.AfterFunc(ctx, func() { _ = reader.Close() })
	defer stopClose()
	defer reader.Close()

	accumulator := chatAccumulator{tools: make(map[int]*toolAccumulator)}
	scanner := bufio.NewScanner(&contextReader{ctx: ctx, reader: reader})
	scanner.Buffer(make([]byte, 4096), maxSSELine)

	var eventType string
	var data strings.Builder
	dispatch := func() error {
		defer func() {
			eventType = ""
			data.Reset()
		}()
		if data.Len() == 0 {
			return nil
		}
		payload := strings.TrimSuffix(data.String(), "\n")
		if payload == "[DONE]" {
			return errStreamDone
		}
		if len(payload) > maxSSEEvent {
			return errors.New("provider SSE event is too large")
		}
		return accumulator.consume(eventType, []byte(payload), onEvent)
	}

	for scanner.Scan() {
		if err := ctx.Err(); err != nil {
			return ChatStreamResult{}, err
		}
		line := strings.TrimSuffix(scanner.Text(), "\r")
		if line == "" {
			if err := dispatch(); err != nil {
				if errors.Is(err, errStreamDone) {
					return accumulator.result(), nil
				}
				return ChatStreamResult{}, err
			}
			continue
		}
		if strings.HasPrefix(line, ":") {
			continue
		}
		field, value, _ := strings.Cut(line, ":")
		value = strings.TrimPrefix(value, " ")
		switch field {
		case "event":
			eventType = value
		case "data":
			if data.Len()+len(value)+1 > maxSSEEvent {
				return ChatStreamResult{}, errors.New("provider SSE event is too large")
			}
			data.WriteString(value)
			data.WriteByte('\n')
		}
	}
	if err := scanner.Err(); err != nil {
		if ctx.Err() != nil {
			return ChatStreamResult{}, ctx.Err()
		}
		return ChatStreamResult{}, fmt.Errorf("read provider SSE: %w", err)
	}
	if err := dispatch(); err != nil {
		if errors.Is(err, errStreamDone) {
			return accumulator.result(), nil
		}
		return ChatStreamResult{}, err
	}
	return accumulator.result(), nil
}

type contextReader struct {
	ctx    context.Context
	reader io.Reader
}

func (r *contextReader) Read(buffer []byte) (int, error) {
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}
	return r.reader.Read(buffer)
}

type chatAccumulator struct {
	id         string
	model      string
	text       strings.Builder
	tools      map[int]*toolAccumulator
	reasoning  []json.RawMessage
	citations  []Citation
	searches   []HostedSearch
	usage      Usage
	resultSize int
}

type toolAccumulator struct {
	ToolCall
	started bool
}

type chatEnvelope struct {
	ID      string          `json:"id"`
	Model   string          `json:"model"`
	Type    string          `json:"type"`
	Choices []chatChoice    `json:"choices"`
	Usage   *chatUsage      `json:"usage"`
	Error   json.RawMessage `json:"error"`
}

type chatChoice struct {
	Index   int       `json:"index"`
	Delta   chatDelta `json:"delta"`
	Message chatDelta `json:"message"`
}

type chatDelta struct {
	Content          string             `json:"content"`
	ToolCalls        []chatToolFragment `json:"tool_calls"`
	ReasoningDetails []json.RawMessage  `json:"reasoning_details"`
	Annotations      []chatAnnotation   `json:"annotations"`
}

type chatToolFragment struct {
	Index    *int   `json:"index"`
	ID       string `json:"id"`
	Function struct {
		Name      string `json:"name"`
		Arguments string `json:"arguments"`
	} `json:"function"`
}

type chatAnnotation struct {
	Type        string `json:"type"`
	URLCitation struct {
		Title      string `json:"title"`
		URL        string `json:"url"`
		StartIndex *int   `json:"start_index"`
		EndIndex   *int   `json:"end_index"`
	} `json:"url_citation"`
}

type chatUsage struct {
	PromptTokens     int `json:"prompt_tokens"`
	CompletionTokens int `json:"completion_tokens"`
	TotalTokens      int `json:"total_tokens"`
	PromptDetails    struct {
		CachedTokens int `json:"cached_tokens"`
	} `json:"prompt_tokens_details"`
	ServerToolUse struct {
		WebSearchRequests int `json:"web_search_requests"`
	} `json:"server_tool_use"`
}

func (a *chatAccumulator) consume(
	eventType string,
	payload []byte,
	onEvent func(StreamEvent),
) error {
	var envelope chatEnvelope
	if err := decodeUniqueJSON(payload, &envelope); err != nil {
		return fmt.Errorf("decode provider SSE JSON: %w", err)
	}
	if (len(envelope.Error) != 0 && string(envelope.Error) != "null") ||
		eventType == "error" || envelope.Type == "error" {
		return errors.New("provider returned a stream error")
	}
	if a.id == "" {
		a.id = envelope.ID
	}
	if a.model == "" {
		a.model = envelope.Model
	}
	if envelope.Usage != nil {
		a.usage = Usage{
			InputTokens:       envelope.Usage.PromptTokens,
			CachedInputTokens: envelope.Usage.PromptDetails.CachedTokens,
			OutputTokens:      envelope.Usage.CompletionTokens,
			TotalTokens:       envelope.Usage.TotalTokens,
			WebSearchRequests: envelope.Usage.ServerToolUse.WebSearchRequests,
		}
	}
	for _, choice := range envelope.Choices {
		if choice.Index != 0 {
			continue
		}
		if err := a.consumeDelta(choice.Delta, onEvent); err != nil {
			return err
		}
		if err := a.consumeDelta(choice.Message, onEvent); err != nil {
			return err
		}
	}
	return nil
}

func (a *chatAccumulator) consumeDelta(delta chatDelta, onEvent func(StreamEvent)) error {
	if delta.Content != "" {
		if err := a.addSize(len(delta.Content)); err != nil {
			return err
		}
		a.text.WriteString(delta.Content)
		onEvent(StreamEvent{Kind: TextDelta, Delta: delta.Content})
	}
	for position, fragment := range delta.ToolCalls {
		index := position
		if fragment.Index != nil {
			index = *fragment.Index
		}
		tool := a.tools[index]
		if tool == nil {
			if len(a.tools) >= maxItems {
				return errors.New("provider stream contains too many items")
			}
			tool = &toolAccumulator{ToolCall: ToolCall{Index: index}}
			a.tools[index] = tool
		}
		if fragment.ID != "" && tool.ID == "" {
			if err := a.addSize(len(fragment.ID)); err != nil {
				return err
			}
			tool.ID = fragment.ID
		}
		if fragment.Function.Name != "" && tool.Name == "" {
			if err := a.addSize(len(fragment.Function.Name)); err != nil {
				return err
			}
			tool.Name = fragment.Function.Name
		}
		if err := a.addSize(len(fragment.Function.Arguments)); err != nil {
			return err
		}
		tool.Arguments += fragment.Function.Arguments
		if !tool.started && tool.ID != "" && tool.Name != "" {
			tool.started = true
			onEvent(StreamEvent{Kind: ToolCallStarted, Index: tool.Index, ID: tool.ID, Name: tool.Name})
		}
	}
	for _, detail := range delta.ReasoningDetails {
		if err := a.appendReasoning(detail); err != nil {
			return err
		}
		var value struct {
			Type      string          `json:"type"`
			ID        string          `json:"id"`
			Name      string          `json:"name"`
			Status    string          `json:"status"`
			Arguments json.RawMessage `json:"arguments"`
			Result    json.RawMessage `json:"result"`
		}
		if json.Unmarshal(detail, &value) == nil && value.Type == "reasoning.server_tool_call" {
			if len(a.searches) >= maxItems {
				return errors.New("provider stream contains too many items")
			}
			if err := a.addSize(len(value.ID) + len(value.Name) + len(value.Status) + len(value.Arguments) + len(value.Result)); err != nil {
				return err
			}
			search := HostedSearch{
				Index: len(a.searches), ID: value.ID, Name: value.Name,
				Status: value.Status, Arguments: value.Arguments, Result: value.Result,
			}
			a.searches = append(a.searches, search)
			onEvent(StreamEvent{Kind: HostedSearchStarted, Index: search.Index, ID: search.ID, Name: search.Name})
		}
	}
	for _, annotation := range delta.Annotations {
		if annotation.Type != "url_citation" || annotation.URLCitation.URL == "" {
			continue
		}
		citationURL, err := safeCitationURL(annotation.URLCitation.URL)
		if err != nil {
			return err
		}
		if len(a.citations) >= maxItems {
			return errors.New("provider stream contains too many items")
		}
		if err := a.addSize(len(annotation.URLCitation.Title) + len(citationURL)); err != nil {
			return err
		}
		a.citations = append(a.citations, Citation{
			Title: annotation.URLCitation.Title, URL: citationURL,
			StartIndex: annotation.URLCitation.StartIndex, EndIndex: annotation.URLCitation.EndIndex,
		})
	}
	return nil
}

func (a *chatAccumulator) addSize(size int) error {
	if size < 0 || a.resultSize > maxResult-size {
		return errors.New("provider stream result is too large")
	}
	a.resultSize += size
	return nil
}

func (a *chatAccumulator) appendReasoning(fragment json.RawMessage) error {
	var source map[string]any
	if err := json.Unmarshal(fragment, &source); err != nil {
		return fmt.Errorf("decode provider reasoning: %w", err)
	}
	fragmentType, _ := source["type"].(string)
	mergeable := fragmentType == "reasoning.text" ||
		fragmentType == "reasoning.encrypted" ||
		fragmentType == "reasoning.summary"
	if mergeable {
		for index, current := range a.reasoning {
			var target map[string]any
			if json.Unmarshal(current, &target) != nil || !sameReasoningDetail(target, source) {
				continue
			}
			mergeReasoningDetail(target, source)
			encoded, err := json.Marshal(target)
			if err != nil {
				return fmt.Errorf("encode provider reasoning: %w", err)
			}
			if growth := len(encoded) - len(current); growth > 0 {
				if err := a.addSize(growth); err != nil {
					return err
				}
			}
			a.reasoning[index] = encoded
			return nil
		}
	}
	if len(a.reasoning) >= maxItems {
		return errors.New("provider stream contains too many items")
	}
	if err := a.addSize(len(fragment)); err != nil {
		return err
	}
	a.reasoning = append(a.reasoning, append(json.RawMessage(nil), fragment...))
	return nil
}

func sameReasoningDetail(left, right map[string]any) bool {
	if left["type"] != right["type"] {
		return false
	}
	if rightID, ok := right["id"]; ok {
		return left["id"] == rightID
	}
	if rightIndex, ok := right["index"]; ok {
		return left["index"] == rightIndex
	}
	return false
}

func mergeReasoningDetail(target, fragment map[string]any) {
	for _, key := range []string{"text", "data", "summary"} {
		part, ok := fragment[key].(string)
		if !ok {
			continue
		}
		current, _ := target[key].(string)
		target[key] = current + part
	}
	for key, value := range fragment {
		if _, exists := target[key]; !exists {
			target[key] = value
		}
	}
}

func safeCitationURL(raw string) (string, error) {
	parsed, err := url.Parse(raw)
	if err != nil || parsed.Host == "" || parsed.User != nil ||
		(parsed.Scheme != "http" && parsed.Scheme != "https") {
		return "", errors.New("provider citation URL must use HTTP or HTTPS")
	}
	return parsed.String(), nil
}

func (a *chatAccumulator) result() ChatStreamResult {
	tools := make([]ToolCall, 0, len(a.tools))
	indices := make([]int, 0, len(a.tools))
	for index := range a.tools {
		indices = append(indices, index)
	}
	sort.Ints(indices)
	for _, index := range indices {
		tools = append(tools, a.tools[index].ToolCall)
	}
	return ChatStreamResult{
		ID: a.id, Model: a.model, Text: a.text.String(), ToolCalls: tools,
		Reasoning: a.reasoning, Citations: a.citations, Searches: a.searches, Usage: a.usage,
	}
}

func decodeUniqueJSON(data []byte, target any) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	value, err := readJSONValue(decoder)
	if err != nil {
		return err
	}
	if token, err := decoder.Token(); err != io.EOF {
		if err != nil {
			return err
		}
		return fmt.Errorf("unexpected trailing JSON token %v", token)
	}
	normalized, err := json.Marshal(value)
	if err != nil {
		return err
	}
	return json.Unmarshal(normalized, target)
}

func readJSONValue(decoder *json.Decoder) (any, error) {
	token, err := decoder.Token()
	if err != nil {
		return nil, err
	}
	delimiter, isDelimiter := token.(json.Delim)
	if !isDelimiter {
		return token, nil
	}
	switch delimiter {
	case '{':
		object := make(map[string]any)
		for decoder.More() {
			keyToken, err := decoder.Token()
			if err != nil {
				return nil, err
			}
			key, ok := keyToken.(string)
			if !ok {
				return nil, errors.New("JSON object key is not text")
			}
			if _, exists := object[key]; exists {
				return nil, fmt.Errorf("duplicate JSON key %q", key)
			}
			value, err := readJSONValue(decoder)
			if err != nil {
				return nil, err
			}
			object[key] = value
		}
		if _, err := decoder.Token(); err != nil {
			return nil, err
		}
		return object, nil
	case '[':
		array := make([]any, 0)
		for decoder.More() {
			value, err := readJSONValue(decoder)
			if err != nil {
				return nil, err
			}
			array = append(array, value)
		}
		if _, err := decoder.Token(); err != nil {
			return nil, err
		}
		return array, nil
	default:
		return nil, fmt.Errorf("unexpected JSON delimiter %q", delimiter)
	}
}
