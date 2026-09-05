package acp

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"time"
)

var (
	// ErrExecutionFailed reports a safe ACP process or protocol failure.
	ErrExecutionFailed = errors.New("ACP execution failed")
	// ErrPermissionPending reports that one exact permission request awaits a human decision.
	ErrPermissionPending = errors.New("ACP permission awaits a decision")
	// ErrOutcomeUncertain reports process loss after Noema sent one effect approval.
	ErrOutcomeUncertain = errors.New("ACP operation outcome is uncertain")
)

// TerminalCall is one accepted Task terminal from the run-scoped MCP bridge.
type TerminalCall struct {
	Name      string
	Arguments json.RawMessage
}

// PermissionRequest is one exact ACP permission request.
type PermissionRequest struct {
	SessionID string             `json:"sessionId"`
	ToolCall  json.RawMessage    `json:"toolCall"`
	Options   []PermissionOption `json:"options"`
}

// PermissionOption is one ACP permission choice.
type PermissionOption struct {
	ID   string `json:"optionId"`
	Name string `json:"name"`
	Kind string `json:"kind"`
}

// PermissionDecision tells ACP whether one exact request can run.
type PermissionDecision struct {
	OptionID string
	Pending  bool
}

// ExecutorRequest contains one concrete ACP Task run.
type ExecutorRequest struct {
	Command    Command
	Cwd        string
	Prompt     string
	HelperPath string
	OnSession  func(string) error
	OnUpdate   func(json.RawMessage)
	Permission func(PermissionRequest) (PermissionDecision, error)
}

type newSessionResult struct {
	SessionID string `json:"sessionId"`
}

type executorEnvelope struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params"`
	Result  json.RawMessage `json:"result"`
	Error   json.RawMessage `json:"error"`
}

// Execute runs one ACP v1 Executor until a Task terminal, cancellation, or permission pause.
func Execute(ctx context.Context, request ExecutorRequest) (terminal TerminalCall, err error) {
	if !filepath.IsAbs(request.Cwd) || !filepath.IsAbs(request.HelperPath) || request.Prompt == "" {
		return TerminalCall{}, ErrExecutionFailed
	}
	bridge, err := startTerminalBridge(ctx)
	if err != nil {
		return TerminalCall{}, ErrExecutionFailed
	}
	defer bridge.close()
	session, err := start(request.Command)
	if err != nil {
		return TerminalCall{}, ErrExecutionFailed
	}
	defer func() { err = errors.Join(err, session.close()) }()

	handshake, cancelHandshake := context.WithTimeout(ctx, 30*time.Second)
	defer cancelHandshake()
	var initialized initializeResult
	if err := session.exchange(handshake, 1, "initialize", initializeParams{
		ProtocolVersion: protocolVersion, ClientCapabilities: map[string]any{},
		ClientInfo: implementation{Name: "noema", Version: "go-migration"},
	}, &initialized); err != nil || initialized.ProtocolVersion != protocolVersion {
		return TerminalCall{}, executionError(ctx)
	}
	var created newSessionResult
	if err := session.exchange(ctx, 2, "session/new", map[string]any{
		"cwd": request.Cwd,
		"mcpServers": []any{map[string]any{
			"name": "noema-task-terminal", "command": request.HelperPath,
			"args": []string{"--acp-task-mcp"},
			"env": []any{
				map[string]string{"name": terminalAddressEnvironment, "value": bridge.address},
				map[string]string{"name": terminalTokenEnvironment, "value": bridge.token},
			},
		}},
	}, &created); err != nil || created.SessionID == "" {
		return TerminalCall{}, executionError(ctx)
	}
	if request.OnSession != nil {
		if err := request.OnSession(created.SessionID); err != nil {
			return TerminalCall{}, err
		}
	}
	if err := session.send(ctx, rpcRequest{JSONRPC: "2.0", ID: 3, Method: "session/prompt", Params: map[string]any{
		"sessionId": created.SessionID,
		"prompt":    []any{map[string]string{"type": "text", "text": request.Prompt}},
	}}); err != nil {
		return TerminalCall{}, executionError(ctx)
	}
	return promptLoop(ctx, session, bridge, created.SessionID, request)
}

func promptLoop(ctx context.Context, session *session, bridge *terminalBridge, sessionID string, request ExecutorRequest) (TerminalCall, error) {
	approvedEffect := false
	for {
		lineResult := make(chan asyncResult[[]byte], 1)
		go func() {
			line, err := readMessage(session.reader)
			lineResult <- asyncResult[[]byte]{value: line, err: err}
		}()
		select {
		case <-ctx.Done():
			_ = session.sendNotification(ctx, "session/cancel", map[string]string{"sessionId": sessionID})
			if approvedEffect {
				return TerminalCall{}, ErrOutcomeUncertain
			}
			return TerminalCall{}, ctx.Err()
		case terminal := <-bridge.result:
			if terminal.err != nil {
				if approvedEffect {
					return TerminalCall{}, ErrOutcomeUncertain
				}
				return TerminalCall{}, ErrExecutionFailed
			}
			return terminal.call, nil
		case result := <-lineResult:
			if result.err != nil {
				if approvedEffect {
					return TerminalCall{}, ErrOutcomeUncertain
				}
				return TerminalCall{}, executionError(ctx)
			}
			var message executorEnvelope
			if json.Unmarshal(result.value, &message) != nil || message.JSONRPC != "2.0" {
				if approvedEffect {
					return TerminalCall{}, ErrOutcomeUncertain
				}
				return TerminalCall{}, ErrExecutionFailed
			}
			if message.Method == "session/update" && len(message.ID) == 0 {
				if request.OnUpdate != nil {
					request.OnUpdate(bytes.Clone(message.Params))
				}
				continue
			}
			if message.Method == "session/request_permission" && len(message.ID) != 0 {
				pending, selected, err := answerPermission(ctx, session, message, request.Permission)
				approvedEffect = approvedEffect || selected
				if err != nil {
					if approvedEffect {
						return TerminalCall{}, ErrOutcomeUncertain
					}
					return TerminalCall{}, err
				}
				if pending {
					return TerminalCall{}, ErrPermissionPending
				}
				continue
			}
			if isRPCID(message.ID, 3) {
				if approvedEffect {
					return TerminalCall{}, ErrOutcomeUncertain
				}
				return TerminalCall{}, ErrExecutionFailed
			}
		}
	}
}

func answerPermission(ctx context.Context, session *session, message executorEnvelope, decide func(PermissionRequest) (PermissionDecision, error)) (bool, bool, error) {
	var request PermissionRequest
	if json.Unmarshal(message.Params, &request) != nil || request.SessionID == "" || decide == nil {
		return false, false, session.sendRawResponse(ctx, message.ID, map[string]any{"outcome": map[string]string{"outcome": "cancelled"}})
	}
	decision, err := decide(request)
	if err != nil {
		return false, false, err
	}
	outcome := map[string]any{"outcome": "cancelled"}
	if decision.OptionID != "" {
		outcome = map[string]any{"outcome": "selected", "optionId": decision.OptionID}
	}
	selected := decision.OptionID != ""
	if err := session.sendRawResponse(ctx, message.ID, map[string]any{"outcome": outcome}); err != nil {
		return false, selected, ErrExecutionFailed
	}
	return decision.Pending, selected, nil
}

func (s *session) send(ctx context.Context, request rpcRequest) error {
	message, err := json.Marshal(request)
	if err != nil {
		return err
	}
	return s.write(ctx, append(message, '\n'))
}

func (s *session) sendNotification(ctx context.Context, method string, params any) error {
	message, err := json.Marshal(map[string]any{"jsonrpc": "2.0", "method": method, "params": params})
	if err != nil {
		return err
	}
	return s.write(ctx, append(message, '\n'))
}

func (s *session) sendRawResponse(ctx context.Context, id json.RawMessage, result any) error {
	resultJSON, err := json.Marshal(result)
	if err != nil {
		return err
	}
	message := append([]byte(`{"jsonrpc":"2.0","id":`), id...)
	message = append(message, []byte(`,"result":`)...)
	message = append(message, resultJSON...)
	message = append(message, '}', '\n')
	return s.write(ctx, message)
}

func (s *session) write(ctx context.Context, message []byte) error {
	_, err := await(ctx, func() (struct{}, error) {
		return struct{}{}, writeMessage(s.stdin, message)
	})
	return err
}

func isRPCID(raw json.RawMessage, want int) bool {
	var value int
	return json.Unmarshal(raw, &value) == nil && value == want
}

func executionError(ctx context.Context) error {
	if ctx.Err() != nil {
		return ctx.Err()
	}
	return ErrExecutionFailed
}

func safeUpdateText(params json.RawMessage) string {
	var value any
	decoder := json.NewDecoder(bytes.NewReader(params))
	decoder.UseNumber()
	if decoder.Decode(&value) != nil {
		return ""
	}
	return firstUpdateText(value)
}

func firstUpdateText(value any) string {
	switch typed := value.(type) {
	case map[string]any:
		if text, ok := typed["text"].(string); ok {
			return text
		}
		for _, child := range typed {
			if text := firstUpdateText(child); text != "" {
				return text
			}
		}
	case []any:
		for _, child := range typed {
			if text := firstUpdateText(child); text != "" {
				return text
			}
		}
	}
	return ""
}

// UpdateIdentity maps one ACP session update to the existing Task run-item vocabulary.
func UpdateIdentity(params json.RawMessage) (kind, status, correlation, content string) {
	var envelope struct {
		Update map[string]any `json:"update"`
	}
	if json.Unmarshal(params, &envelope) != nil {
		return "progress_notice", "completed", "", ""
	}
	typeName, _ := envelope.Update["sessionUpdate"].(string)
	correlation, _ = envelope.Update["toolCallId"].(string)
	content = safeUpdateText(params)
	if typeName == "tool_call" || typeName == "tool_call_update" {
		if title, ok := envelope.Update["title"].(string); ok && title != "" {
			content = title
		}
	}
	if len([]rune(content)) > 20_000 {
		content = string([]rune(content)[:20_000])
	}
	switch typeName {
	case "agent_message_chunk", "agent_thought_chunk":
		return "assistant_output", "completed", correlation, content
	case "tool_call":
		return "tool_call", updateStatus(envelope.Update["status"]), correlation, content
	case "tool_call_update":
		return "tool_result", updateStatus(envelope.Update["status"]), correlation, content
	default:
		return "progress_notice", "completed", correlation, content
	}
}

func updateStatus(value any) string {
	status, _ := value.(string)
	switch status {
	case "pending":
		return "pending"
	case "completed":
		return "completed"
	case "failed":
		return "failed"
	default:
		return "running"
	}
}

// FormatExecutionPrompt adds the bounded ACP terminal contract to current Task data.
func FormatExecutionPrompt(prompt string) string {
	return fmt.Sprintf("%s\n\nYou are the selected ACP Task Executor. Work from current files in the Task directory.\n\nUse task.finish_execution, task.continue_execution, or task.report_blocked exactly once. Ordinary assistant text is not a terminal result.", prompt)
}
