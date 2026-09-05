package localmodel

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os/exec"
	"path/filepath"
	goruntime "runtime"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/kpsuperplane/noema/internal/childenv"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	startupTimeout    = 180 * time.Second
	generationTimeout = 600 * time.Second
	stderrLines       = 32
)

type runtimeProcess struct {
	status   RuntimeStatus
	sequence uint64
	command  *exec.Cmd
	cancel   context.CancelFunc
	endpoint string
	modelID  string
	backend  string
	stderr   *lineHistory
}

type lineHistory struct {
	mu      sync.Mutex
	lines   []string
	partial string
}

func (h *lineHistory) String() string {
	h.mu.Lock()
	defer h.mu.Unlock()
	lines := append([]string(nil), h.lines...)
	if strings.TrimSpace(h.partial) != "" {
		lines = append(lines, strings.TrimSpace(h.partial))
	}
	if len(lines) > stderrLines {
		lines = lines[len(lines)-stderrLines:]
	}
	return strings.Join(lines, "\n")
}

func (h *lineHistory) Write(data []byte) (int, error) {
	h.mu.Lock()
	defer h.mu.Unlock()
	value := h.partial + string(data)
	parts := strings.Split(value, "\n")
	h.partial = parts[len(parts)-1]
	if strings.HasSuffix(value, "\n") {
		h.partial = ""
	}
	for _, line := range parts[:len(parts)-1] {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		h.lines = append(h.lines, line)
		if len(h.lines) > stderrLines {
			h.lines = h.lines[len(h.lines)-stderrLines:]
		}
	}
	return len(data), nil
}

func (s *Service) startRuntime(ctx context.Context, installation store.LocalModelInstallation) error {
	s.runtimeOpMu.Lock()
	defer s.runtimeOpMu.Unlock()

	if installation.Status != "installed" || !validDigest(installation.SHA256) {
		return errors.New("local model installation is not ready")
	}
	expected := filepath.ToSlash(filepath.Join("models", "blobs", installation.SHA256+".gguf"))
	if installation.BlobPath != expected {
		return errors.New("local model blob path does not match its digest")
	}
	blob := filepath.Join(s.home, filepath.FromSlash(expected))
	digest, _, err := hashGGUF(ctx, blob)
	if err != nil || digest != installation.SHA256 {
		return errors.New("installed local model blob is unavailable or changed")
	}
	backends := runtimeBackends(installation.Backend)
	var lastErr error
	for _, backend := range backends {
		lastErr = s.startRuntimeCandidate(ctx, installation, blob, backend)
		if lastErr == nil {
			return nil
		}
	}
	return lastErr
}

func runtimeBackends(preferred string) []string {
	backends := []string{preferred}
	if goruntime.GOOS == "windows" && preferred == "cuda" {
		backends = append(backends, "vulkan")
	}
	if preferred != "cpu" {
		backends = append(backends, "cpu")
	}
	return backends
}

func (s *Service) startRuntimeCandidate(
	ctx context.Context,
	installation store.LocalModelInstallation,
	blob, backend string,
) error {
	executable, err := s.ensureRuntimeAssets(ctx, backend)
	if err != nil {
		s.runtimeFailure(err)
		return err
	}

	s.stopRuntimeLocked()
	s.setRuntimeStatus(RuntimeStarting)
	listener, err := net.Listen("tcp4", "127.0.0.1:0")
	if err != nil {
		s.runtimeFailure(err)
		return err
	}
	port := listener.Addr().(*net.TCPAddr).Port
	_ = listener.Close()

	processContext, cancel := context.WithCancel(context.Background())
	args := runtimeArguments(blob, installation.ModelID, port, backend, s.checkpointCacheMiB(ctx))
	command := runtimeCommand(processContext, executable, args...)
	history := &lineHistory{}
	command.Stderr = history
	if err := command.Start(); err != nil {
		cancel()
		s.runtimeFailure(err)
		return err
	}
	endpoint := "http://127.0.0.1:" + strconv.Itoa(port)
	s.runtimeMu.Lock()
	s.runtime.command = command
	s.runtime.cancel = cancel
	s.runtime.endpoint = endpoint
	s.runtime.modelID = installation.ModelID
	s.runtime.backend = backend
	s.runtime.stderr = history
	s.runtimeMu.Unlock()
	go s.waitForRuntime(command)

	if err := s.waitForHealth(ctx, endpoint); err != nil {
		s.stopRuntimeLocked()
		s.runtimeFailure(err)
		return err
	}
	if err := s.qualifyNativeTools(ctx, endpoint, installation.ModelID); err != nil {
		s.stopRuntimeLocked()
		s.runtimeFailure(err)
		return err
	}
	s.setRuntimeStatus(RuntimeRunning)
	_ = s.database.SetLocalModelAccountStatus(
		context.Background(), provider.StatusAuthenticated, "", "", time.Now(),
	)
	return nil
}

func runtimeCommand(ctx context.Context, executable string, args ...string) *exec.Cmd {
	command := exec.CommandContext(ctx, executable, args...)
	command.Env = childenv.ExternalProcess()
	configureProcess(command)
	return command
}

func runtimeArguments(blob, model string, port int, backend string, cacheMiB int) []string {
	return []string{
		"--model", blob,
		"--alias", model,
		"--host", "127.0.0.1",
		"--port", strconv.Itoa(port),
		"--ctx-size", strconv.FormatUint(uint64(contextTokens), 10),
		"--parallel", "1",
		"--cache-ram", strconv.Itoa(cacheMiB),
		"--n-gpu-layers", gpuLayers(backend),
		"--no-webui",
		"--jinja",
	}
}

func (s *Service) stopRuntime() {
	s.runtimeOpMu.Lock()
	defer s.runtimeOpMu.Unlock()
	s.stopRuntimeLocked()
}

func (s *Service) stopRuntimeLocked() {
	s.runtimeMu.Lock()
	command := s.runtime.command
	cancel := s.runtime.cancel
	s.runtime.command = nil
	s.runtime.cancel = nil
	s.runtime.endpoint = ""
	s.runtime.modelID = ""
	s.runtimeMu.Unlock()
	if command == nil {
		return
	}
	s.setRuntimeStatus(RuntimeStopping)
	if cancel != nil {
		cancel()
	}
	stopProcess(command)
	s.setRuntimeStatus(RuntimeInactive)
}

func (s *Service) waitForRuntime(command *exec.Cmd) {
	err := command.Wait()
	s.runtimeMu.Lock()
	current := s.runtime.command == command
	if current {
		s.runtime.command = nil
		s.runtime.cancel = nil
		s.runtime.endpoint = ""
		s.runtime.modelID = ""
	}
	s.runtimeMu.Unlock()
	if current {
		if err == nil {
			err = errors.New("llama-server stopped unexpectedly")
		}
		s.runtimeFailure(err)
	}
}

func (s *Service) waitForHealth(ctx context.Context, endpoint string) error {
	deadline := time.Now().Add(startupTimeout)
	for {
		requestContext, cancel := context.WithTimeout(ctx, time.Second)
		request, err := http.NewRequestWithContext(requestContext, http.MethodGet, endpoint+"/health", nil)
		if err == nil {
			response, requestErr := s.client.Do(request)
			if requestErr == nil {
				_ = response.Body.Close()
				if response.StatusCode >= 200 && response.StatusCode < 300 {
					cancel()
					return nil
				}
			}
		}
		cancel()
		s.runtimeMu.Lock()
		processExited := s.runtime.endpoint != endpoint || s.runtime.command == nil
		s.runtimeMu.Unlock()
		if processExited {
			return errors.New("llama-server exited during startup")
		}
		if ctx.Err() != nil {
			return ctx.Err()
		}
		if time.Now().After(deadline) {
			return errors.New("llama-server did not become healthy within 180 seconds")
		}
		time.Sleep(100 * time.Millisecond)
	}
}

func (s *Service) runtimeFailure(cause error) {
	message := cause.Error()
	if goruntime.GOOS == "linux" {
		message += ". The pinned runtime requires GLIBC 2.34 and GLIBCXX 3.4.30 support"
	}
	s.runtimeMu.Lock()
	if history := s.runtime.stderr; history != nil {
		if stderr := history.String(); stderr != "" {
			message += ": " + stderr
		}
	}
	s.runtimeMu.Unlock()
	s.setRuntimeStatus(RuntimeFailed)
	_ = s.database.SetLocalModelAccountStatus(
		context.Background(), provider.StatusUnavailable, "runtime_unavailable", message, time.Now(),
	)
}

func gpuLayers(backend string) string {
	if backend == "cpu" {
		return "0"
	}
	return "999"
}

func (s *Service) runtimeEndpoint(model string) (string, bool) {
	s.runtimeMu.Lock()
	defer s.runtimeMu.Unlock()
	return s.runtime.endpoint, s.runtime.status == RuntimeRunning && s.runtime.modelID == model
}

func (s *Service) activeInstallation(ctx context.Context, model string) (store.LocalModelInstallation, error) {
	installations, err := s.database.LocalModelInstallations(ctx)
	if err != nil {
		return store.LocalModelInstallation{}, err
	}
	for _, installation := range installations {
		if installation.Active {
			if model != "" && model != installation.ModelID {
				return installation, fmt.Errorf(
					"local model %q is not loaded; active model is %q",
					model, installation.ModelID,
				)
			}
			return installation, nil
		}
	}
	return store.LocalModelInstallation{}, errors.New("no installed local model is active")
}

// Generate runs one OpenAI-compatible generation against the active local model.
func (s *Service) Generate(
	ctx context.Context,
	request provider.GenerateRequest,
	onEvent func(provider.StreamEvent),
) (provider.GenerationResult, error) {
	if request.AccountID != accountID {
		return provider.GenerationResult{}, errors.New("local generation requires the local model account")
	}
	s.generationMu.Lock()
	defer s.generationMu.Unlock()
	if err := ctx.Err(); err != nil {
		return provider.GenerationResult{}, err
	}
	installation, err := s.activeInstallation(ctx, request.Model)
	if err != nil {
		return provider.GenerationResult{}, err
	}
	endpoint, ready := s.runtimeEndpoint(installation.ModelID)
	if !ready {
		if err := s.startRuntime(ctx, installation); err != nil {
			return provider.GenerationResult{}, err
		}
		endpoint, _ = s.runtimeEndpoint(installation.ModelID)
	}
	return s.generateAt(ctx, endpoint, installation.ModelID, request, onEvent)
}

func (s *Service) generateAt(
	ctx context.Context,
	endpoint, model string,
	request provider.GenerateRequest,
	onEvent func(provider.StreamEvent),
) (provider.GenerationResult, error) {
	body, names, err := localGenerationBody(model, request)
	if err != nil {
		return provider.GenerationResult{}, err
	}
	requestContext, cancel := context.WithTimeout(ctx, generationTimeout)
	defer cancel()
	httpRequest, err := http.NewRequestWithContext(
		requestContext, http.MethodPost, endpoint+"/v1/chat/completions", bytes.NewReader(body),
	)
	if err != nil {
		return provider.GenerationResult{}, err
	}
	httpRequest.Header.Set("Content-Type", "application/json")
	response, err := s.client.Do(httpRequest)
	if err != nil {
		return provider.GenerationResult{}, err
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		defer response.Body.Close()
		body, _ := io.ReadAll(io.LimitReader(response.Body, 64<<10))
		message := strings.TrimSpace(string(body))
		if message == "" {
			return provider.GenerationResult{}, fmt.Errorf("local runtime returned HTTP %d", response.StatusCode)
		}
		return provider.GenerationResult{}, fmt.Errorf(
			"local runtime returned HTTP %d: %s", response.StatusCode, message,
		)
	}
	streamEvent := onEvent
	if onEvent != nil {
		streamEvent = func(event provider.StreamEvent) {
			if event.Kind == provider.ToolCallStarted {
				if canonical, ok := names.providerToCanonical[event.Name]; ok {
					event.Name = canonical
				}
			}
			onEvent(event)
		}
	}
	parsed, err := provider.ParseChatStream(requestContext, response.Body, streamEvent)
	if err != nil {
		return provider.GenerationResult{}, err
	}
	result := provider.GenerationResult{
		ID: parsed.ID, Model: model, Text: parsed.Text, Usage: parsed.Usage,
		FinishReason: "stop", Citations: parsed.Citations, Searches: parsed.Searches,
	}
	for _, call := range parsed.ToolCalls {
		canonical, ok := names.providerToCanonical[call.Name]
		if !ok {
			return provider.GenerationResult{}, errors.New("local runtime returned an unknown tool")
		}
		var object map[string]json.RawMessage
		if err := json.Unmarshal([]byte(call.Arguments), &object); err != nil || object == nil {
			return provider.GenerationResult{}, errors.New("local runtime returned invalid tool arguments")
		}
		result.ToolCalls = append(result.ToolCalls, provider.GenerationToolCall{
			Index: call.Index, ProviderCallID: call.ID, ProviderName: call.Name,
			Name: canonical, Payload: json.RawMessage(call.Arguments),
		})
	}
	if len(result.ToolCalls) != 0 {
		result.FinishReason = "tool_calls"
	}
	if result.Text == "" && len(result.ToolCalls) == 0 {
		return provider.GenerationResult{}, errors.New("local runtime returned empty content")
	}
	return result, nil
}

func (s *Service) qualifyNativeTools(ctx context.Context, endpoint, model string) error {
	maxTokens := uint32(64)
	result, err := s.generateAt(ctx, endpoint, model, provider.GenerateRequest{
		AccountID: accountID,
		Model:     model,
		Messages: []provider.GenerationMessage{
			{Role: "system", Content: "This is a local runtime qualification request. Emit the requested native function call and stop."},
			{Role: "user", Content: "Call the noema_local_qualification function exactly once with an empty JSON object. Do not answer in prose."},
		},
		MaxOutputTokens: &maxTokens,
		Tools: []provider.GenerationTool{{
			Name:        "noema_local_qualification",
			Description: "Qualification-only function. Call it once with an empty object.",
			InputSchema: json.RawMessage(`{"type":"object","properties":{},"required":[],"additionalProperties":false}`),
		}},
		ToolTransport: provider.ToolTransportNative,
		ToolChoice:    provider.ToolChoiceRequired,
	}, nil)
	if err != nil {
		return err
	}
	var arguments map[string]json.RawMessage
	if len(result.ToolCalls) == 1 {
		_ = json.Unmarshal(result.ToolCalls[0].Payload, &arguments)
	}
	if len(result.ToolCalls) != 1 || result.ToolCalls[0].Name != "noema_local_qualification" ||
		arguments == nil || len(arguments) != 0 {
		return errors.New("local runtime failed native tool qualification")
	}
	return nil
}

// CountTokens asks the active llama.cpp runtime to tokenize one prompt.
func (s *Service) CountTokens(ctx context.Context, instructions *string, input string) (uint32, error) {
	installation, err := s.activeInstallation(ctx, "")
	if err != nil {
		return 0, err
	}
	endpoint, ready := s.runtimeEndpoint(installation.ModelID)
	if !ready {
		if err := s.startRuntime(ctx, installation); err != nil {
			return 0, err
		}
		endpoint, _ = s.runtimeEndpoint(installation.ModelID)
	}
	content := input
	if instructions != nil && strings.TrimSpace(*instructions) != "" {
		content = *instructions + "\n\n" + input
	}
	body, _ := json.Marshal(map[string]string{"content": content})
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint+"/tokenize", bytes.NewReader(body))
	if err != nil {
		return 0, err
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := s.client.Do(request)
	if err != nil {
		return 0, err
	}
	defer response.Body.Close()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return 0, fmt.Errorf("local tokenizer returned HTTP %d", response.StatusCode)
	}
	var value struct {
		Count  *uint32           `json:"count"`
		Tokens []json.RawMessage `json:"tokens"`
	}
	decoder := json.NewDecoder(io.LimitReader(response.Body, 16<<20))
	if err := decoder.Decode(&value); err != nil {
		return 0, errors.New("local tokenizer returned invalid JSON")
	}
	if value.Count != nil {
		return *value.Count, nil
	}
	return uint32(len(value.Tokens)), nil
}

type localToolNames struct {
	canonicalToProvider map[string]string
	providerToCanonical map[string]string
}

type localRequest struct {
	Model              string          `json:"model"`
	Messages           []localMessage  `json:"messages"`
	Stream             bool            `json:"stream"`
	StreamOptions      map[string]bool `json:"stream_options"`
	CachePrompt        bool            `json:"cache_prompt"`
	ChatTemplateKwargs map[string]bool `json:"chat_template_kwargs"`
	MaxTokens          *uint32         `json:"max_tokens,omitempty"`
	Temperature        *float32        `json:"temperature,omitempty"`
	Tools              []localTool     `json:"tools,omitempty"`
	ToolChoice         string          `json:"tool_choice,omitempty"`
	ParallelToolCalls  *bool           `json:"parallel_tool_calls,omitempty"`
}

type localMessage struct {
	Role       string          `json:"role"`
	Content    *string         `json:"content,omitempty"`
	ToolCalls  []localToolCall `json:"tool_calls,omitempty"`
	ToolCallID string          `json:"tool_call_id,omitempty"`
}

type localToolCall struct {
	ID       string            `json:"id"`
	Type     string            `json:"type"`
	Function localFunctionCall `json:"function"`
}

type localFunctionCall struct {
	Name      string `json:"name"`
	Arguments string `json:"arguments"`
}

type localTool struct {
	Type     string              `json:"type"`
	Function localToolDefinition `json:"function"`
}

type localToolDefinition struct {
	Name        string         `json:"name"`
	Description string         `json:"description"`
	Parameters  map[string]any `json:"parameters"`
}

func localGenerationBody(model string, request provider.GenerateRequest) ([]byte, localToolNames, error) {
	if request.HostedWebSearch {
		return nil, localToolNames{}, errors.New("local models do not provide hosted web search")
	}
	selectedTools := request.Tools
	if request.ToolChoice == provider.ToolChoiceNone {
		selectedTools = nil
	}
	names, tools, err := lowerLocalTools(selectedTools)
	if err != nil {
		return nil, names, err
	}
	if len(tools) != 0 && request.ToolTransport != provider.ToolTransportNative {
		return nil, names, errors.New("local model tool transport is disabled")
	}
	if request.ToolChoice == provider.ToolChoiceRequired && len(tools) == 0 {
		return nil, names, errors.New("required tool choice needs a non-empty catalog")
	}
	messages, err := lowerLocalMessages(request.Messages, names)
	if err != nil {
		return nil, names, err
	}
	if len(messages) == 0 {
		return nil, names, errors.New("local model request needs model-visible input")
	}
	payload := localRequest{
		Model: model, Messages: messages, Stream: true,
		StreamOptions: map[string]bool{"include_usage": true}, CachePrompt: true,
		ChatTemplateKwargs: map[string]bool{"enable_thinking": false},
		MaxTokens:          request.MaxOutputTokens, Temperature: request.Temperature, Tools: tools,
	}
	if len(tools) != 0 {
		choice := string(request.ToolChoice)
		if choice == "" {
			choice = string(provider.ToolChoiceAuto)
		}
		parallel := request.ParallelTools
		payload.ToolChoice = choice
		payload.ParallelToolCalls = &parallel
	}
	body, err := json.Marshal(payload)
	return body, names, err
}

func lowerLocalTools(tools []provider.GenerationTool) (localToolNames, []localTool, error) {
	names := localToolNames{
		canonicalToProvider: make(map[string]string, len(tools)),
		providerToCanonical: make(map[string]string, len(tools)),
	}
	result := make([]localTool, 0, len(tools))
	for _, tool := range tools {
		wireName := localToolName(tool.Name)
		if previous, exists := names.providerToCanonical[wireName]; exists && previous != tool.Name {
			sum := sha256.Sum256([]byte(tool.Name))
			wireName = strings.TrimRight(wireName[:min(len(wireName), 55)], "_") + "_" + hex.EncodeToString(sum[:4])
		}
		var schema map[string]any
		if err := json.Unmarshal(tool.InputSchema, &schema); err != nil || schema == nil {
			return names, nil, fmt.Errorf("local tool %q has an invalid schema", tool.Name)
		}
		normalizeLocalSchema(schema)
		names.canonicalToProvider[tool.Name] = wireName
		names.providerToCanonical[wireName] = tool.Name
		result = append(result, localTool{Type: "function", Function: localToolDefinition{
			Name: wireName, Description: tool.Description, Parameters: schema,
		}})
	}
	return names, result, nil
}

func localToolName(name string) string {
	var output strings.Builder
	for _, character := range name {
		if character >= 'a' && character <= 'z' || character >= 'A' && character <= 'Z' ||
			character >= '0' && character <= '9' || character == '_' || character == '-' {
			output.WriteRune(character)
		} else {
			output.WriteByte('_')
		}
	}
	value := strings.Trim(output.String(), "_")
	if value == "" {
		value = "tool"
	}
	return value[:min(len(value), 64)]
}

func normalizeLocalSchema(value any) {
	switch current := value.(type) {
	case map[string]any:
		delete(current, "minLength")
		delete(current, "maxLength")
		delete(current, "pattern")
		for _, child := range current {
			normalizeLocalSchema(child)
		}
	case []any:
		for _, child := range current {
			normalizeLocalSchema(child)
		}
	}
}

func lowerLocalMessages(messages []provider.GenerationMessage, names localToolNames) ([]localMessage, error) {
	result := make([]localMessage, 0, len(messages))
	for _, message := range messages {
		role := message.Role
		if role == "developer" {
			role = "system"
		}
		if role != "system" && role != "user" && role != "assistant" && role != "tool" {
			return nil, errors.New("local model message role is invalid")
		}
		if message.ToolResult != nil {
			content, _ := json.Marshal(map[string]any{
				"call_id":       message.ToolResult.ProviderCallID,
				"name":          message.ToolResult.Name,
				"provider_name": message.ToolResult.ProviderName,
				"success":       message.ToolResult.Success,
				"payload":       message.ToolResult.Payload,
			})
			text := string(content)
			result = append(result, localMessage{Role: "tool", Content: &text, ToolCallID: message.ToolResult.ProviderCallID})
			continue
		}
		text := message.Content
		if message.HostedSearch != nil && text == "" {
			encoded, _ := json.Marshal(message.HostedSearch)
			text = string(encoded)
		}
		lowered := localMessage{Role: role, Content: &text}
		for _, call := range message.ToolCalls {
			wireName := names.canonicalToProvider[call.Name]
			if wireName == "" {
				return nil, errors.New("local replay references an unknown tool")
			}
			callID := call.ProviderCallID
			if callID == "" {
				callID = call.ProviderItemID
			}
			lowered.ToolCalls = append(lowered.ToolCalls, localToolCall{
				ID: callID, Type: "function",
				Function: localFunctionCall{Name: wireName, Arguments: string(call.Arguments)},
			})
		}
		if len(lowered.ToolCalls) != 0 && text == "" {
			lowered.Content = nil
		}
		if role == "system" && len(lowered.ToolCalls) == 0 && len(result) != 0 &&
			result[len(result)-1].Role == "system" && result[len(result)-1].Content != nil {
			*result[len(result)-1].Content += "\n\n" + text
			continue
		}
		result = append(result, lowered)
	}
	return result, nil
}

func (s *Service) checkpointCacheMiB(ctx context.Context) int {
	profiles, err := provider.LocalModelHardwareProfiles(ctx)
	if err != nil || len(profiles) == 0 {
		return 0
	}
	cache := profiles[0].RAMGB * 1024 / 32
	if cache > 2048 {
		return 2048
	}
	return cache
}

var _ provider.Generator = (*Service)(nil)

// RuntimeProcess identifies the production runtime for resource qualification.
func (s *Service) RuntimeProcess() (pid int, backend, release string) {
	s.runtimeMu.Lock()
	defer s.runtimeMu.Unlock()
	if s.runtime.command != nil && s.runtime.command.Process != nil {
		pid = s.runtime.command.Process.Pid
	}
	return pid, s.runtime.backend, llamaRelease
}
