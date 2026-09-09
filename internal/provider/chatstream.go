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
	"strconv"
	"strings"
)

const (
	maxSSELine        = 1 << 20
	maxSSEEvent       = 2 << 20
	maxResult         = 16 << 20
	maxItems          = 4096
	maxHostedSearches = 64
)

const (
	providerCitationMarkerStart = "\ue200cite\ue202"
	providerCitationMarkerEnd   = '\ue201'
)

// providerTextDeltaFilter removes private citation markers from live text
// events while leaving the completed provider response unchanged.
//
// A marker may be split at any byte boundary. Whitespace terminates a
// malformed marker so the visible stream cannot be held open indefinitely.
type providerTextDeltaFilter struct {
	pending      string
	insideMarker bool
	lastEvent    StreamEvent
	hasLastEvent bool
}

func (filter *providerTextDeltaFilter) push(delta string) string {
	filter.pending += delta
	var visible strings.Builder
	for {
		if filter.insideMarker {
			boundary := strings.IndexRune(filter.pending, providerCitationMarkerEnd)
			whitespace := strings.IndexFunc(filter.pending, func(r rune) bool { return r == ' ' || r == '\t' || r == '\n' || r == '\r' })
			if boundary < 0 || (whitespace >= 0 && whitespace < boundary) {
				if whitespace < 0 {
					filter.pending = ""
					break
				}
				filter.pending = filter.pending[whitespace:]
				filter.insideMarker = false
				continue
			}
			filter.pending = filter.pending[boundary+len(string(providerCitationMarkerEnd)):]
			filter.insideMarker = false
			continue
		}

		index := strings.Index(filter.pending, providerCitationMarkerStart)
		if index >= 0 {
			visible.WriteString(filter.pending[:index])
			filter.pending = filter.pending[index+len(providerCitationMarkerStart):]
			filter.insideMarker = true
			continue
		}

		keep := 0
		for size := 1; size < len(providerCitationMarkerStart) && size <= len(filter.pending); size++ {
			if strings.HasSuffix(filter.pending, providerCitationMarkerStart[:size]) {
				keep = size
			}
		}
		flush := len(filter.pending) - keep
		visible.WriteString(filter.pending[:flush])
		filter.pending = filter.pending[flush:]
		break
	}
	return visible.String()
}

func (filter *providerTextDeltaFilter) finish() string {
	if filter.insideMarker {
		filter.pending = ""
		return ""
	}
	value := filter.pending
	filter.pending = ""
	return value
}

var errStreamDone = errors.New("provider stream completed")

// StreamEventKind identifies one live provider event.
type StreamEventKind string

const (
	// TextDelta contains new assistant text.
	TextDelta          StreamEventKind = "text_delta"
	MessageStarted     StreamEventKind = "message_started"
	MessageCompleted   StreamEventKind = "message_completed"
	ReasoningDelta     StreamEventKind = "reasoning_delta"
	ReasoningCompleted StreamEventKind = "reasoning_completed"
	// ToolCallStarted identifies a complete provider tool call header.
	ToolCallStarted StreamEventKind = "tool_call_started"
	// HostedSearchStarted identifies one provider-hosted search.
	HostedSearchStarted StreamEventKind = "hosted_search_started"
)

// StreamEvent is one normalized live provider event.
type StreamEvent struct {
	Kind         StreamEventKind
	Index        int
	ID           string
	Name         string
	Delta        string
	Phase        string
	SectionIndex int
	Text         string
}

// ToolCall is one assembled native tool call.
type ToolCall struct {
	OutputIndex int
	Index       int
	ID          string
	Name        string
	Arguments   string
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
	Index          int
	ID             string
	Name           string
	Status         string
	Arguments      json.RawMessage
	Result         json.RawMessage
	Sources        []WebSource
	ProviderAction json.RawMessage
}

// WebSource is one exact public source used by hosted search.
type WebSource struct {
	Title string
	URL   string
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
	Output    []GenerationOutput
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

	accumulator := chatAccumulator{
		tools: make(map[int]*toolAccumulator), searchIndex: make(map[string]int),
		citationKeys: make(map[string]struct{}),
	}
	var deltaFilter providerTextDeltaFilter
	emit := func(event StreamEvent) {
		if event.Kind != TextDelta {
			onEvent(event)
			return
		}
		deltaFilter.lastEvent = event
		deltaFilter.hasLastEvent = true
		event.Delta = deltaFilter.push(event.Delta)
		if event.Delta != "" {
			onEvent(event)
		}
	}
	finishDeltas := func() {
		if delta := deltaFilter.finish(); delta != "" && deltaFilter.hasLastEvent {
			event := deltaFilter.lastEvent
			event.Delta = delta
			onEvent(event)
		}
	}
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
		return accumulator.consume(eventType, []byte(payload), emit)
	}

	for scanner.Scan() {
		if err := ctx.Err(); err != nil {
			return ChatStreamResult{}, err
		}
		line := strings.TrimSuffix(scanner.Text(), "\r")
		if line == "" {
			if err := dispatch(); err != nil {
				if errors.Is(err, errStreamDone) {
					finishDeltas()
					return accumulator.finish(onEvent), nil
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
			finishDeltas()
			return accumulator.finish(onEvent), nil
		}
		return ChatStreamResult{}, err
	}
	finishDeltas()
	return accumulator.finish(onEvent), nil
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
	nextOutputIndex int
	output          []GenerationOutput
	outputKeys      map[string]int
	id              string
	model           string
	text            strings.Builder
	tools           map[int]*toolAccumulator
	reasoning       []json.RawMessage
	plainReasoning  strings.Builder
	citations       []Citation
	searches        []HostedSearch
	searchFields    []hostedSearchFields
	searchIndex     map[string]int
	usage           Usage
	citationKeys    map[string]struct{}
	resultSize      int
}

type toolAccumulator struct {
	ToolCall
	started bool
}

type hostedSearchFields struct {
	name      bool
	status    bool
	arguments bool
	result    bool
}

type chatEnvelope struct {
	ID               string            `json:"id"`
	Model            string            `json:"model"`
	Type             string            `json:"type"`
	Choices          []chatChoice      `json:"choices"`
	Usage            json.RawMessage   `json:"usage"`
	Error            json.RawMessage   `json:"error"`
	ReasoningDetails []json.RawMessage `json:"reasoning_details"`
}

type chatChoice struct {
	Index   int       `json:"index"`
	Delta   chatDelta `json:"delta"`
	Message chatDelta `json:"message"`
}

type chatDelta struct {
	Content          string             `json:"content"`
	Reasoning        string             `json:"reasoning"`
	ReasoningContent string             `json:"reasoning_content"`
	ToolCalls        []chatToolFragment `json:"tool_calls"`
	ReasoningDetails []json.RawMessage  `json:"reasoning_details"`
	Annotations      json.RawMessage    `json:"annotations"`
}

type chatToolFragment struct {
	Index    *int   `json:"index"`
	ID       string `json:"id"`
	Function struct {
		Name      string `json:"name"`
		Arguments string `json:"arguments"`
	} `json:"function"`
}

type chatUsage struct {
	InputTokens       int
	CachedInputTokens int
	OutputTokens      int
	TotalTokens       int
	WebSearchRequests int
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
	if len(envelope.Usage) != 0 {
		a.usage = Usage{}
		if usage, ok := parseChatUsage(envelope.Usage); ok {
			a.usage = Usage(usage)
		}
	}
	if len(envelope.ReasoningDetails) != 0 {
		if err := a.consumeDelta(chatDelta{ReasoningDetails: envelope.ReasoningDetails}, onEvent); err != nil {
			return err
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

func parseChatUsage(raw json.RawMessage) (chatUsage, bool) {
	if len(raw) == 0 || bytes.Equal(raw, []byte("null")) {
		return chatUsage{}, false
	}
	var fields map[string]json.RawMessage
	if json.Unmarshal(raw, &fields) != nil || fields == nil {
		return chatUsage{}, false
	}
	input, ok := aliasedTokenCount(fields, "prompt_tokens", "input_tokens")
	if !ok {
		return chatUsage{}, false
	}
	output, ok := aliasedTokenCount(fields, "completion_tokens", "output_tokens")
	if !ok {
		return chatUsage{}, false
	}
	total, ok := tokenCount(fields, "total_tokens")
	if !ok {
		return chatUsage{}, false
	}
	cached, ok := nestedTokenCount(fields, "prompt_tokens_details", "cached_tokens")
	if !ok {
		return chatUsage{}, false
	}
	searches, ok := nestedTokenCount(fields, "server_tool_use", "web_search_requests")
	if !ok {
		return chatUsage{}, false
	}
	return chatUsage{
		InputTokens: input, CachedInputTokens: cached, OutputTokens: output,
		TotalTokens: total, WebSearchRequests: searches,
	}, true
}

func aliasedTokenCount(fields map[string]json.RawMessage, primary, alias string) (int, bool) {
	_, hasPrimary := fields[primary]
	_, hasAlias := fields[alias]
	if hasPrimary && hasAlias {
		return 0, false
	}
	if hasPrimary {
		return tokenCount(fields, primary)
	}
	return tokenCount(fields, alias)
}

func nestedTokenCount(fields map[string]json.RawMessage, object, field string) (int, bool) {
	raw, exists := fields[object]
	if !exists || bytes.Equal(raw, []byte("null")) {
		return 0, true
	}
	var nested map[string]json.RawMessage
	if json.Unmarshal(raw, &nested) != nil || nested == nil {
		return 0, false
	}
	return tokenCount(nested, field)
}

func tokenCount(fields map[string]json.RawMessage, name string) (int, bool) {
	raw, exists := fields[name]
	if !exists {
		return 0, true
	}
	var value uint64
	maxInt := uint64(^uint(0) >> 1)
	if json.Unmarshal(raw, &value) != nil || value > maxInt {
		return 0, false
	}
	return int(value), true
}

func (a *chatAccumulator) consumeDelta(delta chatDelta, onEvent func(StreamEvent)) error {
	plain := delta.Reasoning
	if plain == "" {
		plain = delta.ReasoningContent
	}
	if err := a.addSize(len(plain)); err != nil {
		return err
	}
	a.plainReasoning.WriteString(plain)
	if plain != "" && !hasReadableReasoning(delta.ReasoningDetails) {
		a.updateReadable("plain", "reasoning", "", a.plainReasoning.String(), onEvent)
	}
	for _, detail := range delta.ReasoningDetails {
		if err := a.appendReasoning(detail); err != nil {
			return err
		}
		for index, raw := range a.reasoning {
			var item map[string]any
			if json.Unmarshal(raw, &item) != nil {
				continue
			}
			if item["type"] != "reasoning.text" && item["type"] != "reasoning.summary" {
				continue
			}
			value := firstJSONText(item, "summary", "text")
			key := fmt.Sprintf("reasoning:%d", index)
			// A provider may repeat plain reasoning in structured details.
			if value == a.plainReasoning.String() && a.plainReasoning.Len() > 0 {
				if _, ok := a.outputKeys["plain"]; ok {
					key = "plain"
				}
			}
			a.updateReadable(key, "reasoning", jsonString(item["id"]), value, onEvent)
			if value == a.plainReasoning.String() && a.plainReasoning.Len() > 0 {
				a.outputKeys["plain"] = a.outputKeys[key]
			}
		}
		if err := a.captureHostedSearch(detail, onEvent); err != nil {
			return err
		}
	}
	if delta.Content != "" {
		if err := a.addSize(len(delta.Content)); err != nil {
			return err
		}
		a.text.WriteString(delta.Content)
		a.updateReadable("message", "message", "", a.text.String(), onEvent)
	}
	for position, fragment := range delta.ToolCalls {
		index := position
		if fragment.Index != nil && *fragment.Index >= 0 {
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
			tool.OutputIndex = a.nextOutputIndex
			a.nextOutputIndex++
			onEvent(StreamEvent{Kind: ToolCallStarted, Index: tool.OutputIndex, ID: tool.ID, Name: tool.Name})
		}
	}

	if len(delta.Annotations) != 0 && string(delta.Annotations) != "null" {
		annotations, err := decodeUniqueJSONValue(delta.Annotations)
		if err != nil {
			return errors.New("provider citation annotations are invalid")
		}
		if err := a.captureCitations(annotations); err != nil {
			return err
		}
	}
	return nil
}

func (a *chatAccumulator) captureHostedSearch(
	detail json.RawMessage,
	onEvent func(StreamEvent),
) error {
	decoded, err := decodeUniqueJSONValue(detail)
	value, ok := decoded.(map[string]any)
	if err != nil || !ok || value["type"] != "reasoning.server_tool_call" {
		return nil
	}
	id, _ := value["id"].(string)
	key := "id:" + id
	if id == "" {
		key = fmt.Sprintf("anonymous:%d", len(a.searches))
	}
	name, namePresent := stringField(value, "name", "tool_name")
	status, statusPresent := stringField(value, "status")
	arguments, argumentsPresent := encodedJSONField(value, "arguments", "input")
	result, resultPresent := encodedJSONField(value, "result")
	if index, exists := a.searchIndex[key]; exists {
		search := &a.searches[index]
		fields := &a.searchFields[index]
		before := len(search.Name) + len(search.Status) + len(search.Arguments) + len(search.Result)
		if namePresent {
			search.Name = name
			fields.name = true
		}
		if statusPresent {
			search.Status = status
			fields.status = true
		}
		if argumentsPresent {
			search.Arguments = arguments
			fields.arguments = true
		}
		if resultPresent {
			search.Result = result
			fields.result = true
		}
		after := len(search.Name) + len(search.Status) + len(search.Arguments) + len(search.Result)
		if after > before {
			if err := a.addSize(after - before); err != nil {
				return err
			}
		}
		return nil
	}
	if len(a.searches) >= maxHostedSearches {
		return errors.New("provider stream contains too many items")
	}
	if err := a.addSize(len(id) + len(name) + len(status) + len(arguments) + len(result)); err != nil {
		return err
	}
	search := HostedSearch{
		Index: a.nextOutputIndex, ID: id, Name: name, Status: status,
		Arguments: arguments, Result: result,
	}
	a.nextOutputIndex++
	a.searchIndex[key] = len(a.searches)
	a.searches = append(a.searches, search)
	a.searchFields = append(a.searchFields, hostedSearchFields{
		name: namePresent, status: statusPresent, arguments: argumentsPresent, result: resultPresent,
	})
	eventID := id
	if eventID == "" {
		eventID = fmt.Sprintf("anonymous:%d", len(a.searches)-1)
	}
	eventName := name
	if !namePresent {
		eventName = "web.search"
	}
	onEvent(StreamEvent{
		Kind: HostedSearchStarted, Index: search.Index, ID: eventID, Name: eventName,
	})
	return nil
}

func stringField(value map[string]any, names ...string) (string, bool) {
	for _, name := range names {
		field, exists := value[name]
		if !exists {
			continue
		}
		text, ok := field.(string)
		return text, ok
	}
	return "", false
}

func encodedJSONField(value map[string]any, names ...string) (json.RawMessage, bool) {
	for _, name := range names {
		field, exists := value[name]
		if !exists {
			continue
		}
		encoded, err := json.Marshal(field)
		if err == nil {
			return encoded, true
		}
	}
	return nil, false
}

func (a *chatAccumulator) addSize(size int) error {
	if size < 0 || a.resultSize > maxResult-size {
		return errors.New("provider stream result is too large")
	}
	a.resultSize += size
	return nil
}

func (a *chatAccumulator) appendReasoning(fragment json.RawMessage) error {
	value, err := decodeUniqueJSONValue(fragment)
	if err != nil {
		return fmt.Errorf("decode provider reasoning: %w", err)
	}
	if source, ok := value.(map[string]any); ok {
		fragmentType, _ := source["type"].(string)
		mergeable := fragmentType == "reasoning.text" ||
			fragmentType == "reasoning.encrypted" ||
			fragmentType == "reasoning.summary"
		if mergeable {
			for index, current := range a.reasoning {
				decoded, decodeErr := decodeUniqueJSONValue(current)
				target, object := decoded.(map[string]any)
				if decodeErr != nil || !object || !sameReasoningDetail(target, source) {
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

func (a *chatAccumulator) captureCitations(value any) error {
	switch current := value.(type) {
	case map[string]any:
		citation := current
		if nested, ok := current["url_citation"].(map[string]any); ok {
			citation = nested
		}
		_, hasNested := current["url_citation"]
		if current["type"] == "url_citation" || hasNested {
			if rawURL, ok := citation["url"].(string); ok {
				citationURL, err := safeCitationURL(rawURL)
				if err != nil {
					return err
				}
				start := citationIndex(citation["start_index"])
				end := citationIndex(citation["end_index"])
				key := citationURL + "\x00" + citationKeyIndex(start) + "\x00" + citationKeyIndex(end)
				if _, exists := a.citationKeys[key]; !exists {
					if len(a.citations) >= maxItems {
						return errors.New("provider stream contains too many items")
					}
					title, _ := citation["title"].(string)
					title = strings.TrimSpace(title)
					if title == "" {
						title = citationURL
					}
					if err := a.addSize(len(title) + len(citationURL)); err != nil {
						return err
					}
					a.citationKeys[key] = struct{}{}
					a.citations = append(a.citations, Citation{
						Title: title, URL: citationURL, StartIndex: start, EndIndex: end,
					})
				}
			}
		}
		for _, child := range current {
			if err := a.captureCitations(child); err != nil {
				return err
			}
		}
	case []any:
		for _, child := range current {
			if err := a.captureCitations(child); err != nil {
				return err
			}
		}
	}
	return nil
}

func citationKeyIndex(value *int) string {
	if value == nil {
		return ""
	}
	return strconv.Itoa(*value)
}

func citationIndex(value any) *int {
	number, ok := value.(json.Number)
	if !ok {
		return nil
	}
	parsed, err := strconv.ParseUint(string(number), 10, strconv.IntSize)
	if err != nil {
		return nil
	}
	index := int(parsed)
	return &index
}

func safeCitationURL(raw string) (string, error) {
	raw = strings.TrimSpace(raw)
	parsed, err := url.Parse(raw)
	if err != nil || parsed.Host == "" || parsed.User != nil ||
		(parsed.Scheme != "http" && parsed.Scheme != "https") {
		return "", errors.New("provider citation URL must use HTTP or HTTPS")
	}
	return parsed.String(), nil
}

func (a *chatAccumulator) result() ChatStreamResult {
	reasoning := a.reasoning
	// Structured details and plain reasoning can contain the same text.
	if a.plainReasoning.Len() != 0 && !containsReadableReasoning(reasoning, a.plainReasoning.String()) {
		raw, _ := json.Marshal(map[string]string{"type": "reasoning.text", "text": a.plainReasoning.String()})
		reasoning = append(reasoning, raw)
	}
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
		ID: a.id, Model: a.model, Text: a.text.String(), ToolCalls: tools, Output: append([]GenerationOutput(nil), a.output...),
		Reasoning: reasoning, Citations: a.citations,
		Searches: a.normalizedSearches(), Usage: a.usage,
	}
}

func (a *chatAccumulator) normalizedSearches() []HostedSearch {
	searches := append([]HostedSearch(nil), a.searches...)
	for index := range searches {
		fields := a.searchFields[index]
		if !fields.name {
			searches[index].Name = "web.search"
		}
		if !fields.status {
			searches[index].Status = "completed"
		}
		if !fields.arguments {
			searches[index].Arguments = json.RawMessage(`{}`)
		}
		if !fields.result {
			searches[index].Result, _ = json.Marshal(map[string]any{
				"status": searches[index].Status,
			})
		}
	}
	expected := a.usage.WebSearchRequests
	if expected < 0 {
		expected = 0
	}
	if expected > maxHostedSearches {
		expected = maxHostedSearches
	}
	if len(a.citations) != 0 && expected < 1 {
		expected = 1
	}
	sourceCount := len(a.citations)
	summary := "Web search completed"
	if sourceCount == 1 {
		summary = "Found 1 cited source"
	} else if sourceCount > 1 {
		summary = fmt.Sprintf("Found %d cited sources", sourceCount)
	}
	for len(searches) < expected {
		index := len(searches)
		id := ""
		if a.id != "" {
			id = fmt.Sprintf("%s:web_search:%d", a.id, index)
		}
		result, _ := json.Marshal(map[string]any{
			"provider": "openrouter", "source_count": sourceCount, "summary": summary,
		})
		searches = append(searches, HostedSearch{
			Index: a.nextOutputIndex + index - len(a.searches), ID: id, Name: "web.search", Status: "completed",
			Arguments: json.RawMessage(`{}`), Result: result,
		})
	}
	return searches
}

func decodeUniqueJSON(data []byte, target any) error {
	value, err := decodeUniqueJSONValue(data)
	if err != nil {
		return err
	}
	normalized, err := json.Marshal(value)
	if err != nil {
		return err
	}
	return json.Unmarshal(normalized, target)
}

func decodeUniqueJSONValue(data []byte) (any, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	value, err := readJSONValue(decoder)
	if err != nil {
		return nil, err
	}
	if token, err := decoder.Token(); err != io.EOF {
		if err != nil {
			return nil, err
		}
		return nil, fmt.Errorf("unexpected trailing JSON token %v", token)
	}
	return value, nil
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

func containsReadableReasoning(details []json.RawMessage, text string) bool {
	for _, item := range normalizeOpenRouterReasoning(details) {
		if strings.Join(item.Summary, "") == text {
			return true
		}
	}
	return false
}
