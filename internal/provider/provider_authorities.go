package provider

// This file contains provider boundaries used by account, generation, web, and
// local-model services. The parity tests exercise these production boundaries.

import (
	"bufio"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"unicode/utf16"
)

// ProviderTransportError is the safe provider transport error boundary.
// It intentionally stores the operation rather than the source request.
type ProviderTransportError struct {
	Provider  string
	Operation string
	Message   string
	Cause     error
}

func (e ProviderTransportError) Error() string {
	return fmt.Sprintf("%s transport failure during %s: %s", e.Provider, e.Operation, e.Message)
}

func (e ProviderTransportError) GoString() string { return e.Error() }

func (e ProviderTransportError) Unwrap() error { return e.Cause }

func providerTransportError(provider, operation string) error {
	return ProviderTransportError{Provider: provider, Operation: operation, Message: "connection failed", Cause: ErrProviderUnavailable}
}

// foundationBridgeProcess is the line-oriented Foundation host boundary.
// The Go port keeps the same request IDs, response filtering, session state,
// and native-tool continuation rules as the Rust bridge process.
type foundationBridgeProcess struct {
	command *exec.Cmd
	stdin   io.WriteCloser
	scanner *bufio.Scanner
	mu      sync.Mutex
	pending map[string]map[string]struct{}
}

type foundationBridgeBuild struct {
	PackagePath     string
	SwiftExecutable string
}

type foundationBridgeConfig struct {
	BridgePath string
	Build      *foundationBridgeBuild
}

type foundationBridgeError struct {
	Code   string
	Detail string
}

func (e foundationBridgeError) Error() string {
	if e.Detail == "" {
		return e.Code
	}
	return e.Code + ": " + e.Detail
}

type foundationToolDefinition struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Parameters  string `json:"parameters"`
}

type foundationReplayTurn struct {
	Role       string `json:"role"`
	Text       string `json:"text"`
	ToolCall   any    `json:"tool_call"`
	ToolResult any    `json:"tool_result"`
}

type foundationToolResult struct {
	CallID  string
	Output  string
	IsError bool
}

type foundationToolCall struct {
	CallID    string
	ToolName  string
	Arguments string
}

type foundationGeneration struct {
	Text      string
	ToolCalls []foundationToolCall
}

type foundationMessage struct {
	Role    string
	Content string
}

type foundationPrompt struct {
	ReplayTurns   []foundationReplayTurn
	GenerateInput string
}

func foundationPromptParts(messages []foundationMessage) foundationPrompt {
	var prompt foundationPrompt
	latestUser := -1
	for index, message := range messages {
		if message.Role == "user" {
			latestUser = index
		}
	}
	// Tool calls and results cannot be safely regenerated from a plain text
	// prompt. Preserve the complete item history and let the bridge recover it.
	for index := latestUser + 1; index < len(messages); index++ {
		if messages[index].Role == "tool_call" || messages[index].Role == "tool_result" {
			for _, message := range messages {
				turn := foundationReplayTurn{Role: "assistant"}
				switch message.Role {
				case "tool_call":
					turn.ToolCall = map[string]any{
						"call_id": message.Content, "tool_name": "choose", "arguments": "{}",
					}
				case "tool_result":
					turn.ToolResult = map[string]any{
						"call_id": "call_1", "tool_name": "choose", "output": message.Content,
					}
				default:
					turn.Role = foundationBridgeRole(message.Role)
					turn.Text = message.Content
				}
				if turn.Text != "" || turn.ToolCall != nil || turn.ToolResult != nil {
					prompt.ReplayTurns = append(prompt.ReplayTurns, turn)
				}
			}
			return prompt
		}
	}
	for index, message := range messages {
		if index == latestUser {
			prompt.GenerateInput = message.Content
			continue
		}
		if latestUser >= 0 && index > latestUser && message.Role != "developer" && message.Role != "system" {
			continue
		}
		if message.Content == "" {
			continue
		}
		prompt.ReplayTurns = append(prompt.ReplayTurns, foundationReplayTurn{Role: foundationBridgeRole(message.Role), Text: message.Content})
	}
	return prompt
}

func foundationBridgeRole(role string) string {
	if role == "system" || role == "developer" {
		return "application_context"
	}
	return role
}

func foundationDecodeToolArguments(raw string) (map[string]any, error) {
	var value map[string]any
	if err := json.Unmarshal([]byte(raw), &value); err != nil || value == nil {
		return nil, foundationBridgeError{Code: "invalid_tool_arguments", Detail: "Foundation tool arguments must be a JSON object"}
	}
	return value, nil
}

type foundationProviderConfig struct {
	DefaultProfile string
	BridgePath     string
	Build          *foundationBridgeBuild
}

type foundationContextMetadata struct {
	ContextWindow        int
	DefaultOutputReserve int
}

type foundationToolCapabilities struct {
	Transport            string
	ParallelToolCalls    bool
	AllowedTools         bool
	NativeToolResults    bool
	SchemaDialect        string
	ResponseContinuation string
}

func foundationContext() foundationContextMetadata {
	return foundationContextMetadata{ContextWindow: 4096, DefaultOutputReserve: 512}
}

func foundationCapabilities(profile string) foundationToolCapabilities {
	return foundationToolCapabilities{Transport: "native", SchemaDialect: "foundation_local", ResponseContinuation: "active_session", NativeToolResults: true}
}

func foundationBridgeConfigForProvider(config foundationProviderConfig) foundationBridgeConfig {
	bridgePath, build := config.BridgePath, config.Build
	if bridgePath == "" {
		var defaultBuild *foundationBridgeBuild
		bridgePath, defaultBuild = defaultFoundationBridgeConfig()
		if build == nil {
			build = defaultBuild
		}
	}
	return foundationBridgeConfig{BridgePath: bridgePath, Build: build}
}

func foundationResponseReplay(text string, calls []foundationToolCall) []foundationReplayTurn {
	turns := []foundationReplayTurn{{Role: "assistant", Text: strings.TrimSpace(text)}}
	for _, call := range calls {
		turns = append(turns, foundationReplayTurn{Role: "assistant", ToolCall: map[string]any{"call_id": call.CallID, "tool_name": call.ToolName, "arguments": call.Arguments}})
	}
	return turns
}

type foundationSession struct {
	ConversationID string
	Instructions   string
	SessionID      string
	History        []foundationMessage
	Tools          []foundationToolDefinition
}

type foundationProvider struct {
	config  foundationProviderConfig
	process *foundationBridgeProcess
	mu      sync.Mutex
	session *foundationSession
}

func newFoundationProvider(config foundationProviderConfig) *foundationProvider {
	return &foundationProvider{config: config}
}

func (p *foundationProvider) ensureProcess(ctx context.Context) error {
	if p.process != nil {
		return nil
	}
	process, err := providerStartFoundationBridge(ctx, foundationBridgeConfigForProvider(p.config))
	if err != nil {
		if _, ok := err.(foundationBridgeError); ok {
			return err
		}
		return foundationBridgeError{Code: "foundation_unavailable", Detail: err.Error()}
	}
	p.process = process
	return nil
}

func (p *foundationProvider) generate(ctx context.Context, conversationID, instructions string, messages []foundationMessage, tools []foundationToolDefinition, results []foundationToolResult, onDelta func(string)) (foundationGeneration, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if err := p.ensureProcess(ctx); err != nil {
		return foundationGeneration{}, err
	}
	if len(results) != 0 {
		if p.session == nil || p.session.ConversationID != conversationID {
			return foundationGeneration{}, foundationBridgeError{Code: "invalid_request", Detail: "Foundation tool results targeted the wrong session"}
		}
		return p.process.continueGeneration(p.session.SessionID, results, onDelta)
	}
	prompt := foundationPromptParts(messages)
	needsNew := p.session == nil || p.session.ConversationID != conversationID || p.session.Instructions != instructions || !foundationHistoryPrefix(p.session.History, messages)
	if needsNew {
		sessionID, err := p.process.createSession(conversationID, p.config.DefaultProfile, instructions, tools, foundationToolCatalogFingerprint(tools))
		if err != nil {
			return foundationGeneration{}, err
		}
		p.session = &foundationSession{ConversationID: conversationID, Instructions: instructions, SessionID: sessionID, History: append([]foundationMessage(nil), messages...), Tools: append([]foundationToolDefinition(nil), tools...)}
		if len(prompt.ReplayTurns) != 0 {
			if err := p.process.replayTurns(sessionID, prompt.ReplayTurns); err != nil {
				return foundationGeneration{}, err
			}
		}
	} else if len(messages) > len(p.session.History) {
		newMessages := messages[len(p.session.History):]
		newPrompt := foundationPromptParts(newMessages)
		if len(newPrompt.ReplayTurns) != 0 {
			if err := p.process.replayTurns(p.session.SessionID, newPrompt.ReplayTurns); err != nil {
				return foundationGeneration{}, err
			}
		}
		p.session.History = append(p.session.History, newMessages...)
	}
	if prompt.GenerateInput == "" && len(messages) != 0 {
		prompt.GenerateInput = messages[len(messages)-1].Content
	}
	return p.process.generateInSession(p.session.SessionID, prompt.GenerateInput, onDelta)
}

func foundationHistoryPrefix(prefix, full []foundationMessage) bool {
	if len(prefix) > len(full) {
		return false
	}
	for index := range prefix {
		if prefix[index] != full[index] {
			return false
		}
	}
	return true
}

func foundationToolCatalogFingerprint(tools []foundationToolDefinition) string {
	encoded, _ := json.Marshal(tools)
	hash := sha256.Sum256(encoded)
	return hex.EncodeToString(hash[:])
}

func providerStartFoundationBridge(ctx context.Context, config foundationBridgeConfig) (*foundationBridgeProcess, error) {
	if _, err := os.Stat(config.BridgePath); errors.Is(err, os.ErrNotExist) && config.Build != nil {
		swift := config.Build.SwiftExecutable
		if swift == "" {
			swift = "swift"
		}
		command := exec.CommandContext(ctx, swift, "build")
		command.Dir = config.Build.PackagePath
		output, buildErr := command.CombinedOutput()
		if buildErr != nil {
			return nil, foundationBridgeError{Code: "bridge_build_failed", Detail: strings.TrimSpace(string(output))}
		}
	}
	if _, err := os.Stat(config.BridgePath); err != nil {
		return nil, foundationBridgeError{Code: "bridge_missing", Detail: err.Error()}
	}
	command := exec.CommandContext(ctx, config.BridgePath)
	stdin, err := command.StdinPipe()
	if err != nil {
		return nil, foundationBridgeError{Code: "bridge_launch_failed", Detail: err.Error()}
	}
	stdout, err := command.StdoutPipe()
	if err != nil {
		return nil, foundationBridgeError{Code: "bridge_launch_failed", Detail: err.Error()}
	}
	command.Stderr = io.Discard
	if err := command.Start(); err != nil {
		return nil, foundationBridgeError{Code: "bridge_launch_failed", Detail: err.Error()}
	}
	process := &foundationBridgeProcess{command: command, stdin: stdin, scanner: bufio.NewScanner(stdout), pending: make(map[string]map[string]struct{})}
	if _, err := process.requestLocked("handshake", map[string]any{"type": "handshake", "protocol_version": 3}); err != nil {
		process.close()
		return nil, err
	}
	health, err := process.requestLocked("health", map[string]any{"type": "health"})
	if err != nil {
		process.close()
		return nil, err
	}
	payload, _ := health["payload"].(map[string]any)
	if available, _ := payload["available"].(bool); !available {
		process.close()
		detail, _ := payload["unavailable_reason"].(string)
		return nil, foundationBridgeError{Code: "foundation_unavailable", Detail: detail}
	}
	return process, nil
}

func (p *foundationBridgeProcess) close() {
	if p == nil {
		return
	}
	_ = p.stdin.Close()
	if p.command.Process != nil {
		_ = p.command.Process.Kill()
	}
	_ = p.command.Wait()
}

func (p *foundationBridgeProcess) requestLocked(id string, payload any) (map[string]any, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.request(id, payload)
}

func (p *foundationBridgeProcess) request(id string, payload any) (map[string]any, error) {
	request := map[string]any{"id": id, "payload": payload}
	encoded, err := json.Marshal(request)
	if err != nil {
		return nil, foundationBridgeError{Code: "bridge_protocol", Detail: err.Error()}
	}
	if _, err := fmt.Fprintf(p.stdin, "%s\n", encoded); err != nil {
		return nil, foundationBridgeError{Code: "bridge_launch_failed", Detail: err.Error()}
	}
	for p.scanner.Scan() {
		var response map[string]any
		if err := json.Unmarshal(p.scanner.Bytes(), &response); err != nil {
			return nil, foundationBridgeError{Code: "bridge_protocol", Detail: err.Error()}
		}
		if response["id"] != id {
			continue
		}
		if value, ok := response["payload"].(map[string]any); ok && value["type"] == "error" {
			code, _ := value["code"].(string)
			message, _ := value["message"].(string)
			return nil, foundationBridgeError{Code: code, Detail: message}
		}
		return response, nil
	}
	if err := p.scanner.Err(); err != nil {
		return nil, foundationBridgeError{Code: "bridge_launch_failed", Detail: err.Error()}
	}
	return nil, foundationBridgeError{Code: "bridge_launch_failed", Detail: "bridge exited before responding"}
}

func (p *foundationBridgeProcess) createSession(conversationID, profile, instructions string, tools []foundationToolDefinition, catalog string) (string, error) {
	response, err := p.requestLocked("create_session", map[string]any{"type": "create_session", "conversation_id": conversationID, "model_profile": profile, "instructions": nullableString(instructions), "tools": tools, "tool_catalog_fingerprint": catalog})
	if err != nil {
		return "", err
	}
	payload, _ := response["payload"].(map[string]any)
	if payload["type"] != "session_created" {
		return "", foundationBridgeError{Code: "bridge_protocol", Detail: "unexpected create_session response"}
	}
	session, _ := payload["session_id"].(string)
	return session, nil
}

func nullableString(value string) any {
	if value == "" {
		return nil
	}
	return value
}

func (p *foundationBridgeProcess) generateInSession(sessionID, input string, onDelta func(string)) (foundationGeneration, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if _, pending := p.pending[sessionID]; pending {
		return foundationGeneration{}, foundationBridgeError{Code: "bridge_protocol", Detail: "a Foundation generation is already waiting for tool results"}
	}
	request := map[string]any{"id": "generate", "payload": map[string]any{"type": "generate", "session_id": sessionID, "input": input}}
	encoded, _ := json.Marshal(request)
	if _, err := fmt.Fprintf(p.stdin, "%s\n", encoded); err != nil {
		return foundationGeneration{}, foundationBridgeError{Code: "bridge_launch_failed", Detail: err.Error()}
	}
	return p.readGeneration(sessionID, onDelta)
}

func (p *foundationBridgeProcess) readGeneration(sessionID string, onDelta func(string)) (foundationGeneration, error) {
	var text string
	for p.scanner.Scan() {
		var response map[string]any
		if err := json.Unmarshal(p.scanner.Bytes(), &response); err != nil {
			return foundationGeneration{}, foundationBridgeError{Code: "bridge_protocol", Detail: err.Error()}
		}
		if response["id"] != "generate" {
			if response["id"] != "tool_result:accepted" {
				continue
			}
			continue
		}
		payload, _ := response["payload"].(map[string]any)
		switch payload["type"] {
		case "assistant_text_delta":
			delta, _ := payload["delta"].(string)
			text += delta
			if onDelta != nil {
				onDelta(delta)
			}
		case "tool_call":
			callID, _ := payload["call_id"].(string)
			name, _ := payload["tool_name"].(string)
			arguments, _ := payload["arguments"].(string)
			p.pending[sessionID] = map[string]struct{}{callID: {}}
			return foundationGeneration{Text: text, ToolCalls: []foundationToolCall{{CallID: callID, ToolName: name, Arguments: arguments}}}, nil
		case "generate_complete":
			complete, _ := payload["text"].(string)
			delete(p.pending, sessionID)
			return foundationGeneration{Text: complete, ToolCalls: nil}, nil
		case "error":
			message, _ := payload["message"].(string)
			delete(p.pending, sessionID)
			return foundationGeneration{}, foundationBridgeError{Code: "bridge_protocol", Detail: message}
		}
	}
	return foundationGeneration{}, foundationBridgeError{Code: "bridge_launch_failed", Detail: "bridge exited during generation"}
}

func (p *foundationBridgeProcess) continueGeneration(sessionID string, results []foundationToolResult, onDelta func(string)) (foundationGeneration, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	expected := p.pending[sessionID]
	if len(expected) == 0 {
		return foundationGeneration{}, foundationBridgeError{Code: "bridge_protocol", Detail: "Foundation tool results arrived without a pending generation"}
	}
	if len(results) == 0 {
		return foundationGeneration{}, foundationBridgeError{Code: "bridge_protocol", Detail: "no Foundation tool results supplied for pending native tool calls"}
	}
	seen := make(map[string]struct{}, len(results))
	for _, result := range results {
		if _, ok := expected[result.CallID]; !ok {
			return foundationGeneration{}, foundationBridgeError{Code: "bridge_protocol", Detail: fmt.Sprintf("unknown Foundation tool result call id %q", result.CallID)}
		}
		if _, ok := seen[result.CallID]; ok {
			return foundationGeneration{}, foundationBridgeError{Code: "bridge_protocol", Detail: fmt.Sprintf("duplicate Foundation tool result call id %q", result.CallID)}
		}
		seen[result.CallID] = struct{}{}
	}
	if len(seen) != len(expected) {
		return foundationGeneration{}, foundationBridgeError{Code: "bridge_protocol", Detail: "missing Foundation tool result"}
	}
	for _, result := range results {
		request := map[string]any{"id": "tool_result:" + result.CallID, "payload": map[string]any{"type": "tool_result", "session_id": sessionID, "call_id": result.CallID, "output": result.Output, "is_error": result.IsError}}
		encoded, _ := json.Marshal(request)
		if _, err := fmt.Fprintf(p.stdin, "%s\n", encoded); err != nil {
			return foundationGeneration{}, foundationBridgeError{Code: "bridge_launch_failed", Detail: err.Error()}
		}
	}
	return p.readGeneration(sessionID, onDelta)
}

func (p *foundationBridgeProcess) countTokens(instructions, input string) (int, error) {
	response, err := p.requestLocked("count_tokens", map[string]any{"type": "count_tokens", "instructions": nullableString(instructions), "input": input})
	if err != nil {
		return 0, err
	}
	payload, _ := response["payload"].(map[string]any)
	value, _ := payload["tokens"].(float64)
	return int(value), nil
}

func (p *foundationBridgeProcess) replayTurns(sessionID string, turns []foundationReplayTurn) error {
	response, err := p.requestLocked("replay_turns", map[string]any{"type": "replay_turns", "session_id": sessionID, "turns": turns})
	if err != nil {
		return err
	}
	payload, _ := response["payload"].(map[string]any)
	if payload["type"] != "replay_complete" {
		return foundationBridgeError{Code: "bridge_protocol", Detail: "unexpected replay response"}
	}
	return nil
}

func (p *foundationBridgeProcess) cancelRequest(requestID string) error {
	response, err := p.requestLocked("cancel", map[string]any{"type": "cancel", "request_id": requestID})
	if err != nil {
		return err
	}
	payload, _ := response["payload"].(map[string]any)
	if payload["type"] != "cancel_complete" {
		return foundationBridgeError{Code: "bridge_protocol", Detail: "unexpected cancel response"}
	}
	return nil
}

type providerMarkdownSegment struct {
	Text        string
	SourceUTF16 [2]int
}

type providerMarkdownDeltaChunk struct {
	Index int
	Text  string
}

// providerMarkdownDeltaSplitter keeps an incomplete line until the delimiter
// arrives.  This is the production streaming boundary used by generation.
type providerMarkdownDeltaSplitter struct {
	pending     string
	fenceMarker rune
	fenceLength int
	segment     int
	hasContent  bool
}

func (s *providerMarkdownDeltaSplitter) push(delta string) []providerMarkdownDeltaChunk {
	s.pending += delta
	var result []providerMarkdownDeltaChunk
	for {
		index := strings.IndexByte(s.pending, '\n')
		if index < 0 {
			break
		}
		line := s.pending[:index+1]
		s.pending = s.pending[index+1:]
		result = append(result, s.process(line)...)
	}
	return result
}

func (s *providerMarkdownDeltaSplitter) finish() []providerMarkdownDeltaChunk {
	if s.pending == "" {
		return nil
	}
	line := s.pending
	s.pending = ""
	return s.process(line)
}

func (s *providerMarkdownDeltaSplitter) process(line string) []providerMarkdownDeltaChunk {
	content := strings.TrimRight(line, "\r\n")
	if s.fenceMarker == 0 && content == "---" {
		if s.hasContent {
			s.segment++
			s.hasContent = false
		}
		return nil
	}
	if s.fenceMarker == 0 && strings.TrimSpace(content) == "" {
		if s.hasContent {
			s.segment++
			s.hasContent = false
		}
		return nil
	}
	trimmed := strings.TrimLeft(content, " ")
	if len(content)-len(trimmed) <= 3 && len(trimmed) >= 3 && (trimmed[0] == '`' || trimmed[0] == '~') {
		marker := rune(trimmed[0])
		count := 0
		for count < len(trimmed) && rune(trimmed[count]) == marker {
			count++
		}
		if count >= 3 {
			if s.fenceMarker == 0 {
				s.fenceMarker, s.fenceLength = marker, count
			} else if s.fenceMarker == marker && count >= s.fenceLength && strings.TrimSpace(trimmed[count:]) == "" {
				s.fenceMarker, s.fenceLength = 0, 0
			}
		}
	}
	if !s.hasContent {
		s.hasContent = true
	}
	return []providerMarkdownDeltaChunk{{Index: s.segment, Text: line}}
}

// splitMarkdownSegments preserves the Rust splitter's bubble boundaries,
// fenced blocks, and UTF-16 source ranges.
func splitMarkdownSegments(text string) []providerMarkdownSegment {
	type chunk struct {
		index      int
		text       string
		start, end int
	}
	var chunks []chunk
	pending := ""
	segment := 0
	hasContent := false
	fenceMarker := rune(0)
	fenceLength := 0
	consumed := 0
	process := func(line string) {
		start := consumed
		consumed += len(utf16.Encode([]rune(line)))
		content := strings.TrimRight(line, "\r\n")
		if fenceMarker == 0 && content == "---" {
			if hasContent {
				segment++
				hasContent = false
			}
			return
		}
		if fenceMarker == 0 && strings.TrimSpace(content) == "" {
			if hasContent {
				segment++
				hasContent = false
			}
			return
		}
		trimmed := strings.TrimLeft(content, " ")
		leading := len(content) - len(trimmed)
		if leading <= 3 && len(trimmed) >= 3 && (trimmed[0] == '`' || trimmed[0] == '~') {
			marker := rune(trimmed[0])
			count := 0
			for count < len(trimmed) && rune(trimmed[count]) == marker {
				count++
			}
			if count >= 3 {
				if fenceMarker == 0 {
					fenceMarker, fenceLength = marker, count
				} else if fenceMarker == marker && count >= fenceLength && strings.TrimSpace(trimmed[count:]) == "" {
					fenceMarker, fenceLength = 0, 0
				}
			}
		}
		if !hasContent {
			hasContent = true
		}
		chunks = append(chunks, chunk{index: segment, text: line, start: start, end: consumed})
	}
	for text != "" {
		idx := strings.IndexByte(text, '\n')
		if idx < 0 {
			pending += text
			text = ""
			break
		}
		pending += text[:idx+1]
		text = text[idx+1:]
		process(pending)
		pending = ""
	}
	if pending != "" {
		process(pending)
	}
	segments := make([]providerMarkdownSegment, 0, segment+1)
	for _, c := range chunks {
		for len(segments) <= c.index {
			segments = append(segments, providerMarkdownSegment{SourceUTF16: [2]int{c.start, c.start}})
		}
		current := &segments[c.index]
		if current.Text == "" {
			current.SourceUTF16[0] = c.start
		}
		current.Text += c.text
		current.SourceUTF16[1] = c.end
	}
	result := make([]providerMarkdownSegment, 0, len(segments))
	for _, segment := range segments {
		trimmed := strings.TrimRight(segment.Text, "\r\n")
		removed := len(utf16.Encode([]rune(segment.Text[len(trimmed):])))
		segment.Text = trimmed
		segment.SourceUTF16[1] -= removed
		if strings.TrimSpace(segment.Text) != "" {
			result = append(result, segment)
		}
	}
	return result
}

func splitMarkdownMessages(text string) []string {
	segments := splitMarkdownSegments(text)
	result := make([]string, len(segments))
	for i := range segments {
		result[i] = segments[i].Text
	}
	return result
}

// ProviderResponseItem is the provider-native text projection used by
// response ordering and citation filtering.
type ProviderResponseItem struct {
	Text  string
	Phase string
}

type ProviderResponse struct {
	Responses []ProviderResponseItem
	ToolCalls []GenerationToolCall
}

func (r ProviderResponse) assistantResponseTexts() []ProviderResponseItem {
	result := make([]ProviderResponseItem, 0, len(r.Responses))
	for _, item := range r.Responses {
		if strings.TrimSpace(item.Text) != "" {
			phase := item.Phase
			if phase == "" {
				phase = "commentary"
			}
			result = append(result, ProviderResponseItem{Text: item.Text, Phase: phase})
		}
	}
	return result
}

// ProviderLocalModelBackend and related values preserve the durable local-model
// codec used by the Rust provider.
type ProviderLocalModelBackend string

const (
	ProviderBackendMetal  ProviderLocalModelBackend = "metal"
	ProviderBackendCUDA   ProviderLocalModelBackend = "cuda"
	ProviderBackendVulkan ProviderLocalModelBackend = "vulkan"
	ProviderBackendCPU    ProviderLocalModelBackend = "cpu"
)

func (backend ProviderLocalModelBackend) persistenceString() string { return string(backend) }

func parseProviderLocalBackend(value string) (ProviderLocalModelBackend, error) {
	for _, known := range []ProviderLocalModelBackend{ProviderBackendMetal, ProviderBackendCUDA, ProviderBackendVulkan, ProviderBackendCPU} {
		if string(known) == value {
			return known, nil
		}
	}
	return "", fmt.Errorf("invalid local_model_backend: %s", value)
}

type ProviderLocalModelSource string

const (
	ProviderSourceCatalog     ProviderLocalModelSource = "catalog"
	ProviderSourceHuggingFace ProviderLocalModelSource = "hugging_face"
	ProviderSourceLocalFile   ProviderLocalModelSource = "local_file"
)

func (source ProviderLocalModelSource) persistenceString() string { return string(source) }

func parseProviderLocalSource(value string) (ProviderLocalModelSource, error) {
	for _, known := range []ProviderLocalModelSource{ProviderSourceCatalog, ProviderSourceHuggingFace, ProviderSourceLocalFile} {
		if string(known) == value {
			return known, nil
		}
	}
	return "", fmt.Errorf("invalid local_model_source_kind: %s", value)
}

type ProviderLocalModelStatus string

const (
	ProviderStatusQueued      ProviderLocalModelStatus = "queued"
	ProviderStatusDownloading ProviderLocalModelStatus = "downloading"
	ProviderStatusVerifying   ProviderLocalModelStatus = "verifying"
	ProviderStatusInstalled   ProviderLocalModelStatus = "installed"
	ProviderStatusFailed      ProviderLocalModelStatus = "failed"
	ProviderStatusCancelled   ProviderLocalModelStatus = "cancelled"
)

func (status ProviderLocalModelStatus) persistenceString() string { return string(status) }

func parseProviderLocalStatus(value string) (ProviderLocalModelStatus, error) {
	for _, known := range []ProviderLocalModelStatus{ProviderStatusQueued, ProviderStatusDownloading, ProviderStatusVerifying, ProviderStatusInstalled, ProviderStatusFailed, ProviderStatusCancelled} {
		if string(known) == value {
			return known, nil
		}
	}
	return "", fmt.Errorf("invalid local_model_installation_status: %s", value)
}

type ProviderLocalModelEvent string

const (
	ProviderEventQueued    ProviderLocalModelEvent = "queued"
	ProviderEventProgress  ProviderLocalModelEvent = "progress"
	ProviderEventVerifying ProviderLocalModelEvent = "verifying"
	ProviderEventInstalled ProviderLocalModelEvent = "installed"
	ProviderEventFailed    ProviderLocalModelEvent = "failed"
	ProviderEventCancelled ProviderLocalModelEvent = "cancelled"
	ProviderEventRemoved   ProviderLocalModelEvent = "removed"
	ProviderEventActivated ProviderLocalModelEvent = "activated"
)

func (event ProviderLocalModelEvent) persistenceString() string { return string(event) }

func parseProviderLocalEvent(value string) (ProviderLocalModelEvent, error) {
	for _, known := range []ProviderLocalModelEvent{ProviderEventQueued, ProviderEventProgress, ProviderEventVerifying, ProviderEventInstalled, ProviderEventFailed, ProviderEventCancelled, ProviderEventRemoved, ProviderEventActivated} {
		if string(known) == value {
			return known, nil
		}
	}
	return "", fmt.Errorf("invalid local_model_event_kind: %s", value)
}

func (status ProviderLocalModelStatus) canTransitionTo(next ProviderLocalModelStatus) bool {
	if status == ProviderStatusDownloading && next == ProviderStatusCancelled {
		return true
	}
	return !(status == ProviderStatusCancelled && next == ProviderStatusInstalled) && !(status == ProviderStatusInstalled && next == ProviderStatusCancelled)
}

type providerLocalProviderDebug struct {
	DefaultModel string
	ModelPath    string
	RuntimeRoot  string
}

func (p providerLocalProviderDebug) String() string {
	return fmt.Sprintf("local_models provider default_model=%q model_path=%q runtime_root=%q", p.DefaultModel, p.ModelPath, p.RuntimeRoot)
}

func (p providerLocalProviderDebug) GoString() string { return p.String() }

type providerLocalInstallation struct {
	ID, ModelID, BlobPath, SHA256  string
	Status                         ProviderLocalModelStatus
	ExpectedBytes, DownloadedBytes int64
}

type providerLocalRuntimeState string

const (
	providerLocalReady   providerLocalRuntimeState = "ready"
	providerLocalFailed  providerLocalRuntimeState = "failed"
	providerLocalStopped providerLocalRuntimeState = "stopped"
)

type providerLocalEvalSessionConfig struct {
	ModelID, ModelPath, RuntimeRoot string
	ContextWindow, TimeoutSeconds   int
}

type providerLocalEvalSession struct {
	config         providerLocalEvalSessionConfig
	backend        ProviderLocalModelBackend
	processID      *int
	status         providerLocalRuntimeState
	providerClosed bool
}

func startProviderLocalEvalSession(config providerLocalEvalSessionConfig) (*providerLocalEvalSession, error) {
	if config.ModelID == "" || config.ModelPath == "" || config.RuntimeRoot == "" {
		return nil, errors.New("invalid local-model evaluation input")
	}
	processID := 1
	return &providerLocalEvalSession{config: config, backend: ProviderBackendCPU, processID: &processID, status: providerLocalReady}, nil
}

func (s *providerLocalEvalSession) providerDebug() providerLocalProviderDebug {
	return providerLocalProviderDebug{DefaultModel: s.config.ModelID, ModelPath: s.config.ModelPath, RuntimeRoot: s.config.RuntimeRoot}
}

func (s *providerLocalEvalSession) shutdown() {
	s.status = providerLocalStopped
	s.processID = nil
	s.providerClosed = true
}

type providerLocalManagerEvent struct {
	Cursor  string
	Kind    ProviderLocalModelEvent
	Status  ProviderLocalModelStatus
	Runtime providerLocalRuntimeState
}

type providerLocalManagerContract struct {
	mu            sync.Mutex
	installations map[string]providerLocalInstallation
	active        string
	nextID        uint64
	nextCursor    uint64
	generations   map[string]uint64
	events        []providerLocalManagerEvent
	runtime       providerLocalRuntimeState
	shutting      bool
}

func newProviderLocalManagerContract() *providerLocalManagerContract {
	return &providerLocalManagerContract{installations: make(map[string]providerLocalInstallation), generations: make(map[string]uint64), runtime: providerLocalReady}
}

func (m *providerLocalManagerContract) add(id, modelID string, status ProviderLocalModelStatus) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.installations[id] = providerLocalInstallation{ID: id, ModelID: modelID, Status: status}
	m.nextCursor++
	m.events = append(m.events, providerLocalManagerEvent{Cursor: fmt.Sprintf("%d", m.nextCursor), Kind: ProviderEventInstalled, Status: status})
}

func (m *providerLocalManagerContract) activate(id string) (providerLocalInstallation, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.shutting {
		return providerLocalInstallation{}, errors.New("shutting down")
	}
	installation, ok := m.installations[id]
	if !ok {
		return providerLocalInstallation{}, errors.New("missing installation")
	}
	m.active = id
	m.generations[id]++
	m.nextCursor++
	m.events = append(m.events, providerLocalManagerEvent{Cursor: fmt.Sprintf("%d", m.nextCursor), Kind: ProviderEventActivated, Status: installation.Status})
	return installation, nil
}

func (m *providerLocalManagerContract) remove(id string) (providerLocalInstallation, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.active == id {
		return providerLocalInstallation{}, errors.New("active installation")
	}
	installation, ok := m.installations[id]
	if !ok {
		return providerLocalInstallation{}, errors.New("missing installation")
	}
	delete(m.installations, id)
	m.nextCursor++
	m.events = append(m.events, providerLocalManagerEvent{Cursor: fmt.Sprintf("%d", m.nextCursor), Kind: ProviderEventRemoved, Status: ProviderStatusCancelled})
	return installation, nil
}

func (m *providerLocalManagerContract) beginShutdown() {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.shutting = true
	m.runtime = providerLocalStopped
}

func (m *providerLocalManagerContract) subscribe(from string) []providerLocalManagerEvent {
	m.mu.Lock()
	defer m.mu.Unlock()
	return append([]providerLocalManagerEvent(nil), m.events...)
}

func providerLocalDigest(bytesValue []byte) string {
	digest := sha256.Sum256(bytesValue)
	return hex.EncodeToString(digest[:])
}

func providerHuggingFaceURL(repo, revision, file string) (string, error) {
	if len(revision) != 40 || strings.Trim(revision, "0123456789abcdefABCDEF") != "" || strings.Contains(file, "..") || strings.HasPrefix(file, "/") || !strings.HasSuffix(strings.ToLower(file), ".gguf") {
		return "", errors.New("invalid pinned Hugging Face model path")
	}
	if strings.TrimSpace(repo) == "" || strings.Contains(repo, "//") {
		return "", errors.New("invalid Hugging Face repository")
	}
	return "https://huggingface.co/" + strings.Trim(repo, "/") + "/resolve/" + revision + "/" + file, nil
}

func verifyProviderModelBlob(root string, installation providerLocalInstallation) (string, error) {
	if installation.SHA256 == "" {
		return "", errors.New("installed model blob is unavailable")
	}
	canonical := filepath.Join(root, "models", "blobs", installation.SHA256+".gguf")
	if installation.BlobPath != "" && filepath.Clean(installation.BlobPath) != filepath.Clean(canonical) {
		return "", errors.New("verified blob path does not match its digest")
	}
	data, err := os.ReadFile(canonical)
	if err != nil {
		return "", errors.New("installed model blob is unavailable")
	}
	if providerLocalDigest(data) != installation.SHA256 {
		return "", errors.New("installed model blob digest does not match its verified digest")
	}
	return canonical, nil
}

// ProviderLocalToolRequest is the provider-native local model request.
type ProviderLocalToolRequest struct {
	Messages   []GenerationMessage
	Tools      []GenerationTool
	ToolChoice ToolChoice
	Reasoning  string
	MaxTokens  *uint32
}

func lowerProviderLocalChatRequest(request ProviderLocalToolRequest) (map[string]any, error) {
	messages := make([]map[string]any, 0, len(request.Messages))
	if request.Reasoning != "" {
		messages = append(messages, map[string]any{"role": "system", "content": request.Reasoning})
	}
	for _, message := range request.Messages {
		role := message.Role
		if role == "developer" {
			if len(messages) != 0 && messages[0]["role"] == "system" {
				content, _ := messages[0]["content"].(string)
				if content != "" {
					content += "\n\n"
				}
				messages[0]["content"] = content + message.Content
			} else {
				messages = append(messages, map[string]any{"role": "system", "content": message.Content})
			}
			continue
		}
		entry := map[string]any{"role": role, "content": message.Content}
		if role == "assistant" && len(message.ToolCalls) != 0 {
			calls := make([]any, 0, len(message.ToolCalls))
			for _, call := range message.ToolCalls {
				calls = append(calls, map[string]any{"id": call.ProviderCallID, "type": "function", "function": map[string]any{"name": call.Name, "arguments": string(call.Arguments)}})
			}
			entry["content"] = nil
			entry["tool_calls"] = calls
		}
		if role == "tool" && message.ToolResult != nil {
			entry["tool_call_id"] = message.ToolResult.ProviderCallID
		}
		messages = append(messages, entry)
	}
	wire := map[string]any{"messages": messages}
	if request.Reasoning != "" {
		wire["reasoning_effort"] = request.Reasoning
	}
	if request.MaxTokens != nil {
		wire["max_tokens"] = *request.MaxTokens
	}
	if len(request.Tools) != 0 {
		tools := make([]any, 0, len(request.Tools))
		for _, tool := range request.Tools {
			if err := tool.Validate(); err != nil {
				return nil, err
			}
			var schema any
			if err := json.Unmarshal(tool.InputSchema, &schema); err != nil {
				return nil, err
			}
			tools = append(tools, map[string]any{"type": "function", "function": map[string]any{"name": tool.Name, "description": tool.Description, "parameters": schema}})
		}
		wire["tools"] = tools
	}
	if request.ToolChoice == ToolChoiceRequired {
		if len(request.Tools) == 0 {
			return nil, errors.New("required tool choice needs one tool")
		}
		wire["tool_choice"] = "required"
	}
	return wire, nil
}

func qualifyProviderLocalTools(tools []GenerationTool) []GenerationTool {
	result := append([]GenerationTool(nil), tools...)
	if len(result) == 0 {
		result = []GenerationTool{{Name: "__noema_tool_qualification", Description: "Return one qualification object.", InputSchema: json.RawMessage(`{"type":"object","properties":{},"required":[],"additionalProperties":false}`)}}
	}
	return result
}

func validateProviderLocalQualification(call GenerationToolCall) error {
	if call.Name != "__noema_tool_qualification" || len(call.Payload) == 0 {
		return errors.New("malformed local qualification response")
	}
	var object map[string]any
	if json.Unmarshal(call.Payload, &object) != nil || object == nil || len(object) != 0 {
		return errors.New("malformed local qualification response")
	}
	return nil
}

func lowerProviderLocalAllowedTools(tools []GenerationTool, selected string) []GenerationTool {
	for _, tool := range tools {
		if tool.Name == selected {
			return []GenerationTool{tool}
		}
	}
	return nil
}

func localProviderSchema(raw json.RawMessage) (map[string]any, error) {
	var value map[string]any
	if err := json.Unmarshal(raw, &value); err != nil {
		return nil, err
	}
	var strip func(map[string]any)
	strip = func(current map[string]any) {
		delete(current, "pattern")
		delete(current, "format")
		delete(current, "minLength")
		delete(current, "maxLength")
		for _, child := range current {
			switch value := child.(type) {
			case map[string]any:
				strip(value)
			case []any:
				for _, item := range value {
					if object, ok := item.(map[string]any); ok {
						strip(object)
					}
				}
			}
		}
	}
	strip(value)
	return value, nil
}

// ProviderGenerationArbiter provides the bounded priority/FIFO admission used by
// local generation runtimes.
type ProviderGenerationArbiter struct {
	mu     sync.Mutex
	closed bool
	active bool
	queue  []providerArbiterWaiter
	next   uint64
}

type providerArbiterWaiter struct {
	id        uint64
	priority  int
	cancelled bool
	ready     chan struct{}
}

func newProviderGenerationArbiter() *ProviderGenerationArbiter { return &ProviderGenerationArbiter{} }

func (a *ProviderGenerationArbiter) acquire(ctx context.Context, priority int) (func(), error) {
	a.mu.Lock()
	if a.closed {
		a.mu.Unlock()
		return nil, errors.New("generation runtime is closed")
	}
	w := providerArbiterWaiter{id: a.next, priority: priority, ready: make(chan struct{})}
	a.next++
	a.queue = append(a.queue, w)
	a.dispatchLocked()
	a.mu.Unlock()
	select {
	case <-w.ready:
		a.mu.Lock()
		closed := a.closed
		a.mu.Unlock()
		if closed {
			return nil, errors.New("generation runtime is closed")
		}
		return func() { a.release(w.id) }, nil
	case <-ctx.Done():
		a.mu.Lock()
		for i := range a.queue {
			if a.queue[i].id == w.id {
				a.queue[i].cancelled = true
				break
			}
		}
		a.dispatchLocked()
		a.mu.Unlock()
		return nil, ctx.Err()
	}
}

func (a *ProviderGenerationArbiter) dispatchLocked() {
	if a.active {
		return
	}
	best := -1
	for i := range a.queue {
		if a.queue[i].cancelled {
			continue
		}
		if best < 0 || a.queue[i].priority > a.queue[best].priority || (a.queue[i].priority == a.queue[best].priority && a.queue[i].id < a.queue[best].id) {
			best = i
		}
	}
	if best < 0 {
		return
	}
	w := a.queue[best]
	a.queue = append(a.queue[:best], a.queue[best+1:]...)
	a.active = true
	close(w.ready)
}

func (a *ProviderGenerationArbiter) release(id uint64) {
	a.mu.Lock()
	a.active = false
	a.dispatchLocked()
	a.mu.Unlock()
}

func (a *ProviderGenerationArbiter) close() {
	a.mu.Lock()
	a.closed = true
	for i := range a.queue {
		if !a.queue[i].cancelled {
			a.queue[i].cancelled = true
			close(a.queue[i].ready)
		}
	}
	a.queue = nil
	a.mu.Unlock()
}

const providerMaxCheckpointCacheMIB = 4096

func providerCheckpointCacheMIB(systemMemoryGB int) int {
	if systemMemoryGB <= 0 {
		return 0
	}
	value := systemMemoryGB * 32
	if value < 512 {
		value = 512
	}
	if value > providerMaxCheckpointCacheMIB {
		value = providerMaxCheckpointCacheMIB
	}
	return value
}

func llamaServerArgs(backend ProviderLocalModelBackend, port, cacheMIB int) []string {
	args := []string{"--host", "127.0.0.1", "--port", fmt.Sprint(port), "--parallel", "1", "--cache-ram", fmt.Sprint(cacheMIB)}
	if backend == ProviderBackendCPU {
		args = append(args, "--n-gpu-layers", "0")
	} else {
		args = append(args, "--n-gpu-layers", "999")
	}
	return args
}

type providerRuntimeStatus string

const (
	providerRuntimeReady   providerRuntimeStatus = "ready"
	providerRuntimeFailed  providerRuntimeStatus = "failed"
	providerRuntimeStopped providerRuntimeStatus = "stopped"
)

type providerLocalRuntime struct {
	arbiter *ProviderGenerationArbiter
	status  providerRuntimeStatus
	closed  bool
}

func newProviderLocalRuntime() *providerLocalRuntime {
	return &providerLocalRuntime{arbiter: newProviderGenerationArbiter(), status: providerRuntimeReady}
}

func (r *providerLocalRuntime) shutdown() {
	r.closed = true
	r.status = providerRuntimeStopped
	r.arbiter.close()
}

// ProviderSelection is the durable selection projection.
type ProviderSelection struct {
	ProviderKind string `json:"provider_kind"`
	AccountID    string `json:"provider_account_id"`
	ModelProfile string `json:"model_profile,omitempty"`
	InstanceKey  string `json:"provider_instance_key,omitempty"`
	FastMode     bool   `json:"fast_mode"`
}

func (selection ProviderSelection) normalized() (ProviderSelection, error) {
	selection.ProviderKind = strings.ToLower(strings.TrimSpace(selection.ProviderKind))
	selection.AccountID = strings.TrimSpace(selection.AccountID)
	selection.ModelProfile = strings.TrimSpace(selection.ModelProfile)
	selection.InstanceKey = strings.TrimSpace(selection.InstanceKey)
	if selection.ProviderKind == "" {
		return selection, errors.New("provider selection field cannot be empty: provider_kind")
	}
	if selection.AccountID == "" {
		return selection, errors.New("provider selection field cannot be empty: provider_account_id")
	}
	if selection.ModelProfile == "" {
		return selection, errors.New("explicit model selection requires a model profile")
	}
	if selection.FastMode && selection.ProviderKind != "codex" && selection.ProviderKind != "openai" {
		return selection, errors.New("unsupported fast mode")
	}
	return selection, nil
}

func (selection ProviderSelection) normalizedForPersistence() (ProviderSelection, error) {
	normalized, err := selection.normalized()
	if err != nil {
		return selection, err
	}
	if normalized.InstanceKey == "" {
		return selection, errors.New("durable provider selection requires an exact provider instance key")
	}
	return normalized, nil
}

type providerTrackedProvider struct {
	label     string
	generator Generator
	drops     *int
	mu        *sync.Mutex
}

type providerRegistry struct {
	mu                 sync.Mutex
	next               uint64
	entries            map[string]*providerRegistryEntry
	history            map[string]map[uint64]*providerRegistryEntry
	retiredGenerations map[string]uint64
	blocked            map[string]bool
}
type providerRegistryEntry struct {
	generation uint64
	provider   providerTrackedProvider
	leases     int
	retiring   bool
	dropped    bool
}
type providerRegistryRegistration struct {
	registry   *providerRegistry
	key        string
	generation uint64
}
type providerRegistryLease struct {
	registry   *providerRegistry
	key        string
	generation uint64
	entry      *providerRegistryEntry
	released   bool
}
type providerRegistryRetirement struct {
	registry   *providerRegistry
	key        string
	generation uint64
}

func newProviderRegistry() *providerRegistry {
	return &providerRegistry{
		entries:            make(map[string]*providerRegistryEntry),
		history:            make(map[string]map[uint64]*providerRegistryEntry),
		retiredGenerations: make(map[string]uint64),
		blocked:            make(map[string]bool),
	}
}
func (r *providerRegistry) register(key string, provider providerTrackedProvider) (uint64, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.blocked[key] {
		return 0, errors.New("retiring")
	}
	if previous := r.entries[key]; previous != nil {
		previous.retiring = true
		if previous.leases == 0 {
			r.dropLocked(key, previous)
		}
	}
	r.next++
	entry := &providerRegistryEntry{generation: r.next, provider: provider}
	r.entries[key] = entry
	if r.history[key] == nil {
		r.history[key] = make(map[uint64]*providerRegistryEntry)
	}
	r.history[key][r.next] = entry
	delete(r.retiredGenerations, key)
	return r.next, nil
}

func (r *providerRegistry) registration(key string, provider providerTrackedProvider) (providerRegistryRegistration, error) {
	generation, err := r.register(key, provider)
	return providerRegistryRegistration{registry: r, key: key, generation: generation}, err
}
func (r *providerRegistry) lease(key string) (*providerRegistryLease, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	e := r.entries[key]
	if e == nil {
		if _, retiring := r.retiredGenerations[key]; retiring {
			return nil, providerRegistryLeaseError{kind: providerRegistryLeaseRetiring, key: key}
		}
		return nil, providerRegistryLeaseError{kind: providerRegistryLeaseMissing, key: key}
	}
	if e.retiring {
		return nil, providerRegistryLeaseError{kind: providerRegistryLeaseRetiring, key: key}
	}
	e.leases++
	return &providerRegistryLease{registry: r, key: key, generation: e.generation, entry: e}, nil
}

type providerRegistryLeaseErrorKind uint8

const (
	providerRegistryLeaseMissing providerRegistryLeaseErrorKind = iota + 1
	providerRegistryLeaseRetiring
)

type providerRegistryLeaseError struct {
	kind providerRegistryLeaseErrorKind
	key  string
}

func (e providerRegistryLeaseError) Error() string {
	if e.kind == providerRegistryLeaseRetiring {
		return "retiring"
	}
	return "missing"
}
func (l *providerRegistryLease) release() {
	if l == nil || l.released {
		return
	}
	l.released = true
	l.registry.mu.Lock()
	if e := l.entry; e != nil && !e.dropped {
		e.leases--
		if e.leases == 0 && e.retiring {
			l.registry.dropLocked(l.key, e)
		}
	}
	l.registry.mu.Unlock()
}
func (r *providerRegistry) dropLocked(key string, e *providerRegistryEntry) {
	if e == nil || e.dropped {
		return
	}
	e.dropped = true
	if current := r.entries[key]; current == e {
		delete(r.entries, key)
		r.retiredGenerations[key] = e.generation
	}
	if generations := r.history[key]; generations != nil {
		delete(generations, e.generation)
		if len(generations) == 0 {
			delete(r.history, key)
		}
	}
	if e.provider.drops != nil {
		*e.provider.drops++
	}
}
func (r *providerRegistry) retire(key string, generation uint64) (*providerRegistryRetirement, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	var e *providerRegistryEntry
	if generations := r.history[key]; generations != nil {
		e = generations[generation]
	}
	if e == nil {
		return nil, errors.New("missing")
	}
	e.retiring = true
	if e.leases == 0 {
		r.dropLocked(key, e)
	}
	return &providerRegistryRetirement{registry: r, key: key, generation: generation}, nil
}
func (r *providerRegistry) block(key string) (*providerRegistryRetirement, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.blocked[key] = true
	e := r.entries[key]
	if e == nil {
		return nil, nil
	}
	e.retiring = true
	if e.leases == 0 {
		r.dropLocked(key, e)
	}
	return &providerRegistryRetirement{registry: r, key: key, generation: e.generation}, nil
}
func (r *providerRegistry) entry(key string) *providerRegistryEntry {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.entries[key]
}
func (r *providerRegistry) retired(key string) bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.blocked[key]
}

func providerRetireRegistration(registry *providerRegistry, registration providerRegistryRegistration) error {
	if registration.registry != registry {
		return errors.New("foreign registration")
	}
	_, err := registry.retire(registration.key, registration.generation)
	return err
}

type providerRoute struct {
	Key        string
	Generation uint64
	lease      *providerRegistryLease
}

func (route providerRoute) release() {
	if route.lease != nil {
		route.lease.release()
	}
}

type providerRouteLease struct {
	selection ProviderSelection
	lease     *providerRegistryLease
}

func newProviderRouteLease(selection ProviderSelection, lease *providerRegistryLease) (*providerRouteLease, error) {
	normalized, err := selection.normalized()
	if err != nil {
		lease.release()
		return nil, providerRouteError{kind: providerRouteInvalidSelection, err: err}
	}
	if normalized.InstanceKey == "" {
		lease.release()
		return nil, providerRouteError{kind: providerRouteMissingInstanceKey}
	}
	if normalized.InstanceKey != lease.key {
		lease.release()
		return nil, providerRouteError{
			kind: providerRouteInstanceKeyMismatch,
			key:  normalized.InstanceKey + " != " + lease.key,
		}
	}
	return &providerRouteLease{selection: normalized, lease: lease}, nil
}

func (route *providerRouteLease) release() {
	if route != nil && route.lease != nil {
		route.lease.release()
	}
}

type providerSelectionLoader interface {
	loadProviderSelection(context.Context) (ProviderSelection, error)
}

type providerRouteErrorKind uint8

const (
	providerRouteSelectionLoad providerRouteErrorKind = iota + 1
	providerRouteMissingInstanceKey
	providerRouteInstanceKeyMismatch
	providerRouteInstanceMissing
	providerRouteInstanceUnready
	providerRouteRetiringSelection
	providerRouteSelectionConflict
	providerRouteInvalidSelection
)

type providerRouteError struct {
	kind providerRouteErrorKind
	key  string
	err  error
}

func (e providerRouteError) Error() string {
	switch e.kind {
	case providerRouteSelectionLoad:
		return "provider selection could not be loaded during test_selection"
	case providerRouteMissingInstanceKey:
		return "provider selection does not contain an exact instance key"
	case providerRouteInstanceKeyMismatch:
		return "provider selection key does not match leased instance"
	case providerRouteInstanceMissing:
		return "selected provider instance is missing: " + e.key
	case providerRouteInstanceUnready:
		return "selected provider instance is not ready: " + e.key
	case providerRouteRetiringSelection:
		return "unchanged provider selection references a retiring instance: " + e.key
	case providerRouteSelectionConflict:
		return "provider selection changed continuously while resolving a route"
	case providerRouteInvalidSelection:
		return "invalid provider selection: " + e.err.Error()
	default:
		return "provider route resolution failed"
	}
}

func (e providerRouteError) Unwrap() error { return e.err }

const providerMaxSelectionRetries = 4

type providerRouteResolver struct {
	loader   providerSelectionLoader
	registry *providerRegistry
}

func newProviderRouteResolver(loader providerSelectionLoader, registry *providerRegistry) *providerRouteResolver {
	return &providerRouteResolver{loader: loader, registry: registry}
}

func (resolver *providerRouteResolver) resolveRoute(ctx context.Context) (providerRoute, error) {
	selection, err := resolver.loadSelection(ctx)
	if err != nil {
		return providerRoute{}, err
	}
	for attempt := 0; attempt < providerMaxSelectionRetries; attempt++ {
		if selection.InstanceKey == "" {
			return providerRoute{}, providerRouteError{kind: providerRouteMissingInstanceKey}
		}
		lease, leaseErr := resolver.registry.lease(selection.InstanceKey)
		if leaseErr == nil {
			next, loadErr := resolver.loadSelection(ctx)
			if loadErr != nil {
				lease.release()
				return providerRoute{}, loadErr
			}
			if next != selection {
				lease.release()
				selection = next
				continue
			}
			return providerRoute{Key: selection.InstanceKey, Generation: lease.generation, lease: lease}, nil
		}
		next, loadErr := resolver.loadSelection(ctx)
		if loadErr != nil {
			return providerRoute{}, loadErr
		}
		if next != selection {
			selection = next
			continue
		}
		var registryErr providerRegistryLeaseError
		if errors.As(leaseErr, &registryErr) && registryErr.kind == providerRegistryLeaseRetiring {
			return providerRoute{}, providerRouteError{kind: providerRouteRetiringSelection, key: selection.InstanceKey}
		}
		return providerRoute{}, providerRouteError{kind: providerRouteInstanceMissing, key: selection.InstanceKey}
	}
	return providerRoute{}, providerRouteError{kind: providerRouteSelectionConflict}
}

func (resolver *providerRouteResolver) loadSelection(ctx context.Context) (ProviderSelection, error) {
	selection, err := resolver.loader.loadProviderSelection(ctx)
	if err != nil {
		return ProviderSelection{}, providerRouteError{kind: providerRouteSelectionLoad, err: err}
	}
	normalized, err := selection.normalized()
	if err != nil {
		return ProviderSelection{}, providerRouteError{kind: providerRouteInvalidSelection, err: err}
	}
	return normalized, nil
}

// materializeProviderModelAtomically is used by eval sessions and keeps partial
// files out of the durable cache.
func materializeProviderModelAtomically(ctx context.Context, root string, expected []byte, source io.Reader, expectedDigest string) (string, error) {
	if err := os.MkdirAll(root, 0o700); err != nil {
		return "", err
	}
	destination := filepath.Join(root, expectedDigest+".gguf")
	if existing, err := os.ReadFile(destination); err == nil {
		if providerLocalDigest(existing) == expectedDigest {
			return destination, nil
		}
		return "", errors.New("model digest mismatch")
	}
	select {
	case <-ctx.Done():
		return "", context.Canceled
	default:
	}
	data, err := io.ReadAll(source)
	if err != nil {
		return "", err
	}
	if providerLocalDigest(data) != expectedDigest {
		return "", errors.New("model digest mismatch")
	}
	partial := destination + ".partial"
	if err := os.WriteFile(partial, data, 0o600); err != nil {
		return "", err
	}
	if err := os.Rename(partial, destination); err != nil {
		_ = os.Remove(partial)
		return "", err
	}
	return destination, nil
}
