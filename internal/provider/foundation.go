package provider

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"hash/fnv"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"time"
)

const (
	foundationAccountID       = "provider_account:foundation_local:default"
	foundationProtocol        = 3
	foundationMessageLimit    = 8 << 20
	foundationControlTimeout  = 5 * time.Second
	foundationGenerateTimeout = 120 * time.Second
	foundationBuildTimeout    = 120 * time.Second
	foundationContextWindow   = 4096
	foundationOutputReserve   = 512
	foundationContextSafety   = 128
)

// FoundationGenerator uses the shipped Apple Foundation Models bridge.
type FoundationGenerator struct {
	accounts           *AccountService
	bridgePath         string
	developmentPackage string
	supported          bool
}

// NewFoundationGenerator creates the local Apple Foundation Models transport.
func NewFoundationGenerator(accounts *AccountService) (*FoundationGenerator, error) {
	if accounts == nil {
		return nil, errors.New("Foundation Models account service is required")
	}
	bridgePath, developmentPackage := discoverFoundationBridge()
	return &FoundationGenerator{accounts: accounts, bridgePath: bridgePath,
		developmentPackage: developmentPackage, supported: runtime.GOOS == "darwin"}, nil
}

func newFoundationGenerator(accounts *AccountService, path string, supported bool) *FoundationGenerator {
	return &FoundationGenerator{accounts: accounts, bridgePath: path, supported: supported}
}

// OpenGenerationSession opens one bridge process for one runtime turn or run.
func (g *FoundationGenerator) OpenGenerationSession() GenerationSession {
	return &foundationSession{generator: g}
}

// Generate performs one bounded generation without retaining a bridge process.
func (g *FoundationGenerator) Generate(
	ctx context.Context, request GenerateRequest, onEvent func(StreamEvent),
) (GenerationResult, error) {
	session := g.OpenGenerationSession()
	defer session.Close()
	return session.Generate(ctx, request, onEvent)
}

// CheckAvailability verifies that the bridge and Apple model runtime are ready.
func (g *FoundationGenerator) CheckAvailability(ctx context.Context) error {
	bridge, err := g.startBridge(ctx)
	if err != nil {
		return err
	}
	return bridge.stop()
}

// RefreshAccount runs one live availability check and records its result.
func (g *FoundationGenerator) RefreshAccount(ctx context.Context, now time.Time) (Account, error) {
	status := StatusAuthenticated
	var code, message string
	if err := g.CheckAvailability(ctx); err != nil {
		status, code, message = StatusUnavailable, FoundationErrorCode(err), err.Error()
	}
	if err := g.accounts.persistence.UpdateFoundationAvailability(ctx, status, code, message, now); err != nil {
		return Account{}, err
	}
	return g.accounts.LoadAccount(ctx, foundationAccountID)
}

// CountTokens asks Apple Foundation Models to count one prompt.
func (g *FoundationGenerator) CountTokens(
	ctx context.Context, instructions *string, input string,
) (uint32, error) {
	bridge, err := g.startBridge(ctx)
	if err != nil {
		return 0, err
	}
	defer bridge.stop()
	response, err := bridge.request(ctx, "count_tokens", map[string]any{
		"type": "count_tokens", "instructions": instructions, "input": input,
	}, foundationGenerateTimeout)
	if err != nil {
		return 0, err
	}
	if response.Type != "token_count" || response.Tokens == nil {
		return 0, foundationProtocolError("unexpected token count response")
	}
	return *response.Tokens, nil
}

// FoundationErrorCode returns the stable availability code for an error.
func FoundationErrorCode(err error) string {
	var bridgeError *foundationError
	if errors.As(err, &bridgeError) {
		return bridgeError.code
	}
	return "bridge_launch_failed"
}

func discoverFoundationBridge() (string, string) {
	var sibling string
	if executable, err := os.Executable(); err == nil {
		sibling = filepath.Join(filepath.Dir(executable), "noema-foundation-bridge")
		if _, err := os.Stat(sibling); err == nil {
			return sibling, ""
		}
	}
	_, source, _, _ := runtime.Caller(0)
	packagePath := filepath.Clean(filepath.Join(filepath.Dir(source), "../../crates/noema-providers/apple-foundation-bridge"))
	development := filepath.Join(packagePath, ".build/debug/noema-foundation-bridge")
	if _, err := os.Stat(filepath.Join(packagePath, "Package.swift")); err == nil {
		return development, packagePath
	}
	if sibling != "" {
		return sibling, ""
	}
	return development, ""
}

func (g *FoundationGenerator) startBridge(ctx context.Context) (*foundationBridge, error) {
	if !g.supported {
		return nil, &foundationError{code: "unsupported_platform", message: "Apple Foundation Models requires macOS"}
	}
	if _, err := os.Stat(g.bridgePath); errors.Is(err, os.ErrNotExist) && g.developmentPackage != "" {
		if err := g.buildDevelopmentBridge(ctx); err != nil {
			return nil, err
		}
	}
	if _, err := os.Stat(g.bridgePath); err != nil {
		return nil, &foundationError{code: "bridge_missing", message: "Apple Foundation Models bridge is missing"}
	}
	command := exec.Command(g.bridgePath)
	stdin, err := command.StdinPipe()
	if err != nil {
		return nil, foundationLaunchError(err)
	}
	stdout, err := command.StdoutPipe()
	if err != nil {
		return nil, foundationLaunchError(err)
	}
	command.Stderr = io.Discard
	if err := command.Start(); err != nil {
		return nil, foundationLaunchError(err)
	}
	bridge := &foundationBridge{
		stdin: stdin, scanner: bufio.NewScanner(stdout), process: newFoundationProcess(command),
	}
	bridge.scanner.Buffer(make([]byte, 4096), foundationMessageLimit)
	response, err := bridge.request(ctx, "handshake", map[string]any{
		"type": "handshake", "protocol_version": foundationProtocol,
	}, foundationControlTimeout)
	if err != nil || response.Type != "handshake_ok" || response.ProtocolVersion != foundationProtocol {
		_ = bridge.stop()
		if err != nil {
			return nil, err
		}
		return nil, foundationProtocolError("unsupported bridge protocol version")
	}
	response, err = bridge.request(ctx, "health", map[string]any{"type": "health"}, foundationControlTimeout)
	if err != nil || response.Type != "health" || !response.Available {
		_ = bridge.stop()
		if err != nil {
			return nil, err
		}
		if response.Type != "health" {
			return nil, foundationProtocolError("unexpected health response")
		}
		reason := response.UnavailableReason
		if reason == "" {
			reason = "Apple Foundation Models is unavailable"
		}
		return nil, &foundationError{code: "foundation_models_unavailable", message: reason}
	}
	return bridge, nil
}

func (g *FoundationGenerator) buildDevelopmentBridge(ctx context.Context) error {
	buildContext, cancel := context.WithTimeout(ctx, foundationBuildTimeout)
	defer cancel()
	command := exec.CommandContext(buildContext, "swift", "build")
	command.Dir = g.developmentPackage
	command.Stdin = nil
	output, err := command.CombinedOutput()
	if err == nil {
		if _, statErr := os.Stat(g.bridgePath); statErr == nil {
			return nil
		}
		err = errors.New("swift build did not create the Foundation Models bridge")
	}
	detail := strings.TrimSpace(string(output))
	if len(detail) > 4096 {
		detail = detail[:4096]
	}
	if detail != "" {
		return &foundationError{code: "bridge_build_failed", message: "Apple Foundation Models bridge build failed: " + detail}
	}
	return &foundationError{code: "bridge_build_failed", message: "Apple Foundation Models bridge build failed: " + err.Error()}
}

type foundationSession struct {
	generator  *FoundationGenerator
	mu         sync.Mutex
	bridge     *foundationBridge
	sessionID  string
	sessionKey string
	pending    map[string]struct{}
	tools      openRouterToolNameMap
	closed     bool
}

func (s *foundationSession) Generate(
	ctx context.Context, request GenerateRequest, onEvent func(StreamEvent),
) (GenerationResult, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return GenerationResult{}, errors.New("Foundation Models session is closed")
	}
	if err := s.validateAccount(ctx, request); err != nil {
		return GenerationResult{}, err
	}
	if s.bridge == nil {
		bridge, err := s.generator.startBridge(ctx)
		if err != nil {
			return GenerationResult{}, err
		}
		s.bridge = bridge
	}
	history := request.Messages
	if len(request.ReplayMessages) != 0 {
		history = request.ReplayMessages
	}
	tools, definitions, err := foundationTools(request.Tools)
	if err != nil {
		return GenerationResult{}, err
	}
	if len(definitions) != 0 && request.ToolTransport != ToolTransportNative {
		return GenerationResult{}, errors.New("Foundation Models tool transport is disabled")
	}
	model := strings.TrimSpace(request.Model)
	if model == "" {
		model = "default"
	}
	maxOutputTokens := foundationMaxOutputTokens(request.MaxOutputTokens)
	if err := s.admitContext(ctx, history, definitions, *maxOutputTokens); err != nil {
		return GenerationResult{}, err
	}
	if len(s.pending) != 0 {
		return s.continueGeneration(ctx, request.Messages, model, onEvent)
	}
	conversationID := strings.TrimSpace(request.ConversationID)
	if conversationID == "" {
		conversationID = fmt.Sprintf("conversation:foundation-local-%d", time.Now().UnixNano())
	}
	sessionKey := conversationID + "\x00" + model + "\x00" + foundationToolFingerprint(definitions)
	if s.sessionID != "" && s.sessionKey != sessionKey {
		response, closeErr := s.bridge.request(ctx, "close_session", map[string]any{
			"type": "close_session", "session_id": s.sessionID,
		}, foundationControlTimeout)
		if closeErr != nil || response.Type != "replay_complete" {
			if closeErr != nil {
				return GenerationResult{}, closeErr
			}
			return GenerationResult{}, foundationProtocolError("unexpected close session response")
		}
		s.sessionID = ""
	}
	replay, input, err := foundationPrompt(history)
	if err != nil {
		return GenerationResult{}, err
	}
	if s.sessionID == "" {
		response, err := s.bridge.request(ctx, "create_session", map[string]any{
			"type": "create_session", "conversation_id": conversationID,
			"model_profile": model, "instructions": nil, "tools": definitions,
			"tool_catalog_fingerprint": foundationToolFingerprint(definitions),
		}, foundationControlTimeout)
		if err != nil || response.Type != "session_created" || strings.TrimSpace(response.SessionID) == "" {
			if err != nil {
				return GenerationResult{}, err
			}
			return GenerationResult{}, foundationProtocolError("unexpected session creation response")
		}
		s.sessionID = response.SessionID
		s.sessionKey = sessionKey
		if len(replay) != 0 {
			response, err = s.bridge.request(ctx, "replay_turns", map[string]any{
				"type": "replay_turns", "session_id": s.sessionID, "turns": replay,
			}, foundationControlTimeout)
			if err != nil || response.Type != "replay_complete" {
				if err != nil {
					return GenerationResult{}, err
				}
				return GenerationResult{}, foundationProtocolError("unexpected replay response")
			}
		}
	}
	s.tools = tools
	return s.generate(ctx, input, maxOutputTokens, model, onEvent)
}

func foundationMaxOutputTokens(requested *uint32) *uint32 {
	value := uint32(foundationOutputReserve)
	if requested != nil && *requested > 0 && *requested < value {
		value = *requested
	}
	return &value
}

func (s *foundationSession) admitContext(
	ctx context.Context, messages []GenerationMessage, tools []foundationToolDefinition, reserve uint32,
) error {
	turns, err := foundationReplay(messages)
	if err != nil {
		return err
	}
	input, err := json.Marshal(map[string]any{"turns": turns, "tools": tools})
	if err != nil {
		return errors.New("Foundation Models context is invalid")
	}
	response, err := s.bridge.request(ctx, "count_tokens", map[string]any{
		"type": "count_tokens", "instructions": nil, "input": string(input),
	}, foundationGenerateTimeout)
	if err != nil {
		return err
	}
	if response.Type != "token_count" || response.Tokens == nil {
		return foundationProtocolError("unexpected token count response")
	}
	available := uint32(foundationContextWindow-foundationContextSafety) - reserve
	if *response.Tokens > available {
		return ErrGenerationRequestTooLarge
	}
	return nil
}

func (s *foundationSession) validateAccount(ctx context.Context, request GenerateRequest) error {
	if request.AccountID != foundationAccountID {
		return errors.New("Foundation Models generation requires the default account")
	}
	account, err := s.generator.accounts.LoadAccount(ctx, request.AccountID)
	if err != nil {
		return err
	}
	if account.ProviderKind == "foundation_local" && account.IsActive && account.Status != StatusAuthenticated {
		account, err = s.generator.RefreshAccount(ctx, time.Now())
		if err != nil {
			return err
		}
	}
	if account.ProviderKind != "foundation_local" || !account.IsActive || account.Status != StatusAuthenticated {
		return ErrProviderUnavailable
	}
	return nil
}

func (s *foundationSession) generate(
	ctx context.Context, input string, maxTokens *uint32, model string, onEvent func(StreamEvent),
) (GenerationResult, error) {
	payload := map[string]any{"type": "generate", "session_id": s.sessionID, "input": input}
	if maxTokens != nil {
		payload["max_output_tokens"] = *maxTokens
	}
	if err := s.bridge.write("generate", payload); err != nil {
		s.poison()
		return GenerationResult{}, err
	}
	return s.readGeneration(ctx, model, onEvent)
}

func (s *foundationSession) continueGeneration(
	ctx context.Context, messages []GenerationMessage, model string, onEvent func(StreamEvent),
) (GenerationResult, error) {
	results := make(map[string]*ReplayToolResult)
	for _, message := range messages {
		if message.ToolResult != nil {
			if _, exists := results[message.ToolResult.ProviderCallID]; exists {
				return GenerationResult{}, errors.New("Foundation Models tool result is duplicated")
			}
			results[message.ToolResult.ProviderCallID] = message.ToolResult
		}
	}
	if len(results) != len(s.pending) {
		return GenerationResult{}, errors.New("Foundation Models tool results do not match pending calls")
	}
	for callID := range s.pending {
		result := results[callID]
		if result == nil {
			return GenerationResult{}, errors.New("Foundation Models tool result is missing")
		}
		output, err := json.Marshal(map[string]any{"success": result.Success, "payload": result.Payload})
		if err != nil {
			return GenerationResult{}, errors.New("Foundation Models tool result is invalid")
		}
		if err := s.bridge.write("tool_result:"+callID, map[string]any{
			"type": "tool_result", "session_id": s.sessionID, "call_id": callID,
			"output": string(output), "is_error": !result.Success,
		}); err != nil {
			s.poison()
			return GenerationResult{}, err
		}
	}
	return s.readGeneration(ctx, model, onEvent)
}

func (s *foundationSession) readGeneration(
	ctx context.Context, model string, onEvent func(StreamEvent),
) (GenerationResult, error) {
	var streamed strings.Builder
	for {
		response, err := s.bridge.read(ctx, foundationGenerateTimeout)
		if err != nil {
			s.poison()
			return GenerationResult{}, err
		}
		if response.ID != "generate" {
			if response.Type == "tool_result_accepted" {
				continue
			}
			s.poison()
			return GenerationResult{}, foundationProtocolError("unexpected interleaved bridge response")
		}
		switch response.Type {
		case "error":
			s.poison()
			return GenerationResult{}, &foundationError{code: "bridge_protocol", message: response.Code + ": " + response.Message}
		case "assistant_text_delta":
			streamed.WriteString(response.Delta)
			if onEvent != nil {
				onEvent(StreamEvent{Kind: TextDelta, Delta: response.Delta})
			}
		case "tool_call":
			call, err := foundationToolCall(response, s.tools)
			if err != nil {
				s.poison()
				return GenerationResult{}, err
			}
			s.pending = map[string]struct{}{call.ProviderCallID: {}}
			return GenerationResult{ID: s.sessionID, Model: model, Text: streamed.String(), FinishReason: "tool_calls", ToolCalls: []GenerationToolCall{call}}, nil
		case "generate_complete":
			s.pending = nil
			return GenerationResult{ID: s.sessionID, Model: model, Text: response.Text, FinishReason: "stop"}, nil
		default:
			s.poison()
			return GenerationResult{}, foundationProtocolError("unexpected generation response")
		}
	}
}

func (s *foundationSession) poison() {
	_ = s.bridge.stop()
	s.bridge = nil
	s.sessionID = ""
	s.sessionKey = ""
	s.pending = nil
}

// Close closes the model session and its bridge process.
func (s *foundationSession) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return nil
	}
	s.closed = true
	if s.bridge == nil {
		return nil
	}
	if s.sessionID != "" {
		ctx, cancel := context.WithTimeout(context.Background(), foundationControlTimeout)
		_, _ = s.bridge.request(ctx, "close_session", map[string]any{
			"type": "close_session", "session_id": s.sessionID,
		}, foundationControlTimeout)
		cancel()
	}
	return s.bridge.stop()
}

type foundationBridge struct {
	stdin    io.WriteCloser
	scanner  *bufio.Scanner
	process  *foundationProcess
	stopOnce sync.Once
	stopErr  error
}

type foundationResponse struct {
	ID                string  `json:"id"`
	Type              string  `json:"type"`
	ProtocolVersion   int     `json:"protocol_version"`
	Available         bool    `json:"available"`
	UnavailableReason string  `json:"unavailable_reason"`
	SessionID         string  `json:"session_id"`
	Delta             string  `json:"delta"`
	Text              string  `json:"text"`
	CallID            string  `json:"call_id"`
	ToolName          string  `json:"tool_name"`
	Arguments         string  `json:"arguments"`
	Tokens            *uint32 `json:"tokens"`
	Code              string  `json:"code"`
	Message           string  `json:"message"`
}

func (b *foundationBridge) write(id string, payload map[string]any) error {
	encoded, err := json.Marshal(map[string]any{"id": id, "payload": payload})
	if err != nil || len(encoded)+1 > foundationMessageLimit {
		return ErrGenerationRequestTooLarge
	}
	encoded = append(encoded, '\n')
	if _, err := b.stdin.Write(encoded); err != nil {
		return foundationLaunchError(err)
	}
	return nil
}

func (b *foundationBridge) request(
	ctx context.Context, id string, payload map[string]any, timeout time.Duration,
) (foundationResponse, error) {
	if err := b.write(id, payload); err != nil {
		return foundationResponse{}, err
	}
	deadline := time.Now().Add(timeout)
	for {
		remaining := time.Until(deadline)
		if remaining <= 0 {
			_ = b.stop()
			return foundationResponse{}, &foundationError{code: "bridge_launch_failed", message: "Apple Foundation Models bridge response timed out"}
		}
		response, err := b.read(ctx, remaining)
		if err != nil {
			return foundationResponse{}, err
		}
		if response.ID != id {
			continue
		}
		if response.Type == "error" {
			return foundationResponse{}, &foundationError{code: "bridge_protocol", message: response.Code + ": " + response.Message}
		}
		return response, nil
	}
}

func (b *foundationBridge) read(ctx context.Context, timeout time.Duration) (foundationResponse, error) {
	type readResult struct {
		response foundationResponse
		err      error
	}
	result := make(chan readResult, 1)
	go func() {
		if !b.scanner.Scan() {
			err := b.scanner.Err()
			if err == nil {
				err = io.EOF
			}
			result <- readResult{err: err}
			return
		}
		var envelope struct {
			ID      string          `json:"id"`
			Payload json.RawMessage `json:"payload"`
		}
		if err := json.Unmarshal(b.scanner.Bytes(), &envelope); err != nil {
			result <- readResult{err: foundationProtocolError("malformed bridge response")}
			return
		}
		var response foundationResponse
		response.ID = envelope.ID
		if err := json.Unmarshal(envelope.Payload, &response); err != nil || response.ID == "" || response.Type == "" {
			result <- readResult{err: foundationProtocolError("malformed bridge response")}
			return
		}
		result <- readResult{response: response}
	}()
	timer := time.NewTimer(timeout)
	defer timer.Stop()
	select {
	case value := <-result:
		if value.err != nil {
			return foundationResponse{}, foundationLaunchError(value.err)
		}
		return value.response, nil
	case <-ctx.Done():
		_ = b.stop()
		<-result
		return foundationResponse{}, ctx.Err()
	case <-timer.C:
		_ = b.stop()
		<-result
		return foundationResponse{}, &foundationError{code: "bridge_launch_failed", message: "Apple Foundation Models bridge response timed out"}
	}
}

func (b *foundationBridge) stop() error {
	b.stopOnce.Do(func() {
		_ = b.stdin.Close()
		b.stopErr = b.process.stop()
	})
	return b.stopErr
}

type foundationError struct{ code, message string }

func (e *foundationError) Error() string { return e.message }
func foundationProtocolError(message string) error {
	return &foundationError{code: "bridge_protocol", message: message}
}
func foundationLaunchError(err error) error {
	var typed *foundationError
	if errors.As(err, &typed) {
		return typed
	}
	return &foundationError{code: "bridge_launch_failed", message: "Apple Foundation Models bridge failed: " + err.Error()}
}

type foundationToolDefinition struct {
	Name, Description, Parameters string
}

func (d foundationToolDefinition) MarshalJSON() ([]byte, error) {
	return json.Marshal(map[string]string{"name": d.Name, "description": d.Description, "parameters": d.Parameters})
}

type foundationReplayTurn struct {
	Role       string                      `json:"role"`
	Text       string                      `json:"text"`
	ToolCall   *foundationReplayToolCall   `json:"tool_call,omitempty"`
	ToolResult *foundationReplayToolResult `json:"tool_result,omitempty"`
}

type foundationReplayToolCall struct {
	CallID, ToolName, Arguments string
}

func (v foundationReplayToolCall) MarshalJSON() ([]byte, error) {
	return json.Marshal(map[string]string{"call_id": v.CallID, "tool_name": v.ToolName, "arguments": v.Arguments})
}

type foundationReplayToolResult struct {
	CallID, ToolName, Output string
}

func (v foundationReplayToolResult) MarshalJSON() ([]byte, error) {
	return json.Marshal(map[string]string{"call_id": v.CallID, "tool_name": v.ToolName, "output": v.Output})
}

func foundationTools(tools []GenerationTool) (openRouterToolNameMap, []foundationToolDefinition, error) {
	names, _, err := prepareOpenRouterTools(tools)
	if err != nil {
		return openRouterToolNameMap{}, nil, err
	}
	definitions := make([]foundationToolDefinition, 0, len(tools))
	for _, tool := range tools {
		var schema any
		if json.Unmarshal(tool.InputSchema, &schema) != nil {
			return openRouterToolNameMap{}, nil, errors.New("Foundation Models tool schema is invalid")
		}
		parameters, _ := json.Marshal(schema)
		definitions = append(definitions, foundationToolDefinition{
			Name: names.canonicalToName[tool.Name], Description: strings.TrimSpace(tool.Description), Parameters: string(parameters),
		})
	}
	return names, definitions, nil
}

func foundationToolFingerprint(tools []foundationToolDefinition) string {
	encoded, _ := json.Marshal(tools)
	hash := fnv.New64a()
	_, _ = hash.Write(encoded)
	return fmt.Sprintf("%016x", hash.Sum64())
}

func foundationToolCall(response foundationResponse, names openRouterToolNameMap) (GenerationToolCall, error) {
	rule, ok := names.providerToRule[response.ToolName]
	if !ok || strings.TrimSpace(response.CallID) == "" {
		return GenerationToolCall{}, foundationProtocolError("Foundation Models called an unknown native tool")
	}
	var arguments any
	if json.Unmarshal([]byte(response.Arguments), &arguments) != nil || jsonObject(arguments) == nil {
		return GenerationToolCall{}, foundationProtocolError("Foundation Models tool arguments must be a JSON object")
	}
	payload, _ := json.Marshal(arguments)
	return GenerationToolCall{
		ProviderCallID: response.CallID, ProviderName: response.ToolName,
		Name: rule.canonical, Payload: payload,
	}, nil
}

func foundationPrompt(messages []GenerationMessage) ([]foundationReplayTurn, string, error) {
	lastUser := -1
	for index := range messages {
		if messages[index].Role == "user" {
			lastUser = index
		}
	}
	trailingTool := false
	if lastUser >= 0 {
		for _, message := range messages[lastUser+1:] {
			trailingTool = trailingTool || len(message.ToolCalls) != 0 || message.ToolResult != nil
		}
	}
	if lastUser < 0 || trailingTool {
		turns, err := foundationReplay(messages)
		return turns, "", err
	}
	turns, err := foundationReplay(messages[:lastUser])
	if err != nil {
		return nil, "", err
	}
	for _, message := range messages[lastUser+1:] {
		if message.Role == "system" || message.Role == "developer" {
			more, lowerErr := foundationReplay([]GenerationMessage{message})
			if lowerErr != nil {
				return nil, "", lowerErr
			}
			turns = append(turns, more...)
		}
	}
	return turns, messages[lastUser].Content, nil
}

func foundationReplay(messages []GenerationMessage) ([]foundationReplayTurn, error) {
	var turns []foundationReplayTurn
	for _, message := range messages {
		role := map[string]string{"system": "application_context", "developer": "application_context", "user": "user", "assistant": "assistant"}[message.Role]
		if role != "" && strings.TrimSpace(message.Content) != "" {
			turns = append(turns, foundationReplayTurn{Role: role, Text: message.Content})
		}
		if message.EncryptedReasoning != "" {
			turns = append(turns, foundationReplayTurn{Role: "assistant", Text: message.EncryptedReasoning})
		}
		for _, call := range message.ToolCalls {
			name := call.ProviderName
			if name == "" {
				name = call.Name
			}
			arguments := call.Arguments
			if len(arguments) == 0 {
				arguments = json.RawMessage(`{}`)
			}
			if strings.TrimSpace(call.ProviderCallID) == "" || !json.Valid(arguments) {
				return nil, errors.New("Foundation Models replay tool call is invalid")
			}
			turns = append(turns, foundationReplayTurn{Role: "assistant", ToolCall: &foundationReplayToolCall{
				CallID: call.ProviderCallID, ToolName: name, Arguments: string(arguments),
			}})
		}
		if message.ToolResult != nil {
			result := message.ToolResult
			name := result.ProviderName
			if name == "" {
				name = result.Name
			}
			output, err := json.Marshal(map[string]any{"success": result.Success, "payload": result.Payload})
			if err != nil || strings.TrimSpace(result.ProviderCallID) == "" {
				return nil, errors.New("Foundation Models replay tool result is invalid")
			}
			turns = append(turns, foundationReplayTurn{Role: "assistant", ToolResult: &foundationReplayToolResult{
				CallID: result.ProviderCallID, ToolName: name, Output: string(output),
			}})
		}
		if message.HostedSearch != nil {
			content, _ := json.Marshal(message.HostedSearch)
			turns = append(turns, foundationReplayTurn{Role: "assistant", Text: string(content)})
		}
	}
	return turns, nil
}
