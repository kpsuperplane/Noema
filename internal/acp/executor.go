package acp

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"sync"
	"time"

	sdk "github.com/coder/acp-go-sdk"
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
	var handlerMu sync.Mutex
	active, approvedEffect := false, false
	failures := make(chan error, 1)
	fail := func(err error) {
		select {
		case failures <- err:
		default:
		}
	}
	session, err := start(request.Command, func(_ context.Context, method string, params json.RawMessage) (any, *sdk.RequestError) {
		handlerMu.Lock()
		defer handlerMu.Unlock()
		switch method {
		case "session/update":
			if active && request.OnUpdate != nil {
				request.OnUpdate(bytes.Clone(params))
			}
			return nil, nil
		case "session/request_permission":
			outcome := map[string]string{"outcome": "cancelled"}
			var permission PermissionRequest
			if active && request.Permission != nil && json.Unmarshal(params, &permission) == nil && permission.SessionID != "" {
				decision, err := request.Permission(permission)
				if err != nil {
					fail(err)
					return nil, sdk.NewInternalError(nil)
				}
				if decision.OptionID != "" {
					outcome = map[string]string{"outcome": "selected", "optionId": decision.OptionID}
					approvedEffect = true
				}
				if decision.Pending {
					fail(ErrPermissionPending)
				}
			}
			return map[string]any{"outcome": outcome}, nil
		default:
			return nil, sdk.NewMethodNotFound(method)
		}
	})
	if err != nil {
		return TerminalCall{}, ErrExecutionFailed
	}
	defer func() {
		handlerMu.Lock()
		active = false
		if err != nil && approvedEffect && !errors.Is(err, ErrPermissionPending) {
			err = ErrOutcomeUncertain
		}
		handlerMu.Unlock()
		err = errors.Join(err, session.close())
	}()

	handshake, cancelHandshake := context.WithTimeout(ctx, 30*time.Second)
	defer cancelHandshake()
	var initialized initializeResult
	if err := session.exchange(handshake, "initialize", initializeParams{
		ProtocolVersion: protocolVersion, ClientCapabilities: map[string]any{},
		ClientInfo: implementation{Name: "noema", Version: "go-migration"},
	}, &initialized); err != nil || initialized.ProtocolVersion != protocolVersion {
		return TerminalCall{}, executionError(ctx)
	}
	var created newSessionResult
	if err := session.exchange(ctx, "session/new", map[string]any{
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
	handlerMu.Lock()
	active = true
	handlerMu.Unlock()
	promptCtx, cancelPrompt := context.WithCancel(ctx)
	defer cancelPrompt()
	finished := make(chan struct{})
	go func() {
		defer close(finished)
		var response json.RawMessage
		_ = session.exchange(promptCtx, "session/prompt", map[string]any{
			"sessionId": created.SessionID,
			"prompt":    []any{map[string]string{"type": "text", "text": request.Prompt}},
		}, &response)
	}()
	// Stop blocked writes before waiting for the request goroutine.
	defer func() { cancelPrompt(); _ = session.stdin.Close(); <-finished }()
	select {
	case <-ctx.Done():
		return TerminalCall{}, ctx.Err()
	case result := <-bridge.result:
		if result.err != nil {
			return TerminalCall{}, ErrExecutionFailed
		}
		return result.call, nil
	case err := <-failures:
		return TerminalCall{}, err
	case <-finished:
		return TerminalCall{}, executionError(ctx)
	}
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
