package acp

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestExecutorRunsV1SessionUpdatesPermissionsAndTerminals(t *testing.T) {
	for _, terminal := range []string{"task.finish_execution", "task.continue_execution", "task.report_blocked"} {
		t.Run(terminal, func(t *testing.T) {
			cwd := t.TempDir()
			command := executorHelperCommand(t, "terminal", terminal, "")
			var sessionID, updateText string
			result, err := Execute(context.Background(), ExecutorRequest{
				Command: command, Cwd: cwd, Prompt: "current Task data", HelperPath: command.Path,
				OnSession: func(value string) error { sessionID = value; return nil },
				OnUpdate:  func(raw json.RawMessage) { _, _, _, updateText = UpdateIdentity(raw) },
				Permission: func(request PermissionRequest) (PermissionDecision, error) {
					if request.SessionID != "session:test" || len(request.Options) != 1 {
						t.Fatalf("permission = %#v", request)
					}
					return PermissionDecision{OptionID: "allow:once"}, nil
				},
			})
			if err != nil || result.Name != terminal || sessionID != "session:test" || updateText != "Working" {
				t.Fatalf("result=%#v session=%q update=%q err=%v", result, sessionID, updateText, err)
			}
			if terminal == "task.report_blocked" && !bytes.Contains(result.Arguments, []byte("Which target")) {
				t.Fatalf("blocked arguments = %s", result.Arguments)
			}
		})
	}
	kind, status, correlation, content := UpdateIdentity(json.RawMessage(`{"update":{"sessionUpdate":"tool_call","toolCallId":"tool:test","title":"Inspect workspace","status":"pending"}}`))
	if kind != "tool_call" || status != "pending" || correlation != "tool:test" || content != "Inspect workspace" {
		t.Fatalf("tool update identity = %q, %q, %q, %q", kind, status, correlation, content)
	}
}

func TestExecutorCancellationStopsDescendants(t *testing.T) {
	marker := filepath.Join(t.TempDir(), "survived")
	command := executorHelperCommand(t, "hang", "", marker)
	ctx, cancel := context.WithTimeout(context.Background(), 150*time.Millisecond)
	defer cancel()
	_, err := Execute(ctx, ExecutorRequest{Command: command, Cwd: t.TempDir(), Prompt: "Task", HelperPath: command.Path})
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("cancellation error = %v", err)
	}
	time.Sleep(600 * time.Millisecond)
	if _, err := os.Stat(marker); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("ACP descendant survived: %v", err)
	}
}

func TestExecutorCancellationStopsBlockedPromptWrite(t *testing.T) {
	command := executorHelperCommand(t, "stalled_prompt", "", "")
	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()
	_, err := Execute(ctx, ExecutorRequest{
		Command: command, Cwd: t.TempDir(), Prompt: strings.Repeat("x", 512<<10), HelperPath: command.Path,
	})
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("blocked prompt cancellation error = %v", err)
	}
}

func TestExecutorProtocolFailureAfterApprovalIsUncertain(t *testing.T) {
	command := executorHelperCommand(t, "malformed_after_approval", "", "")
	_, err := Execute(context.Background(), ExecutorRequest{
		Command: command, Cwd: t.TempDir(), Prompt: "current Task", HelperPath: command.Path,
		Permission: func(PermissionRequest) (PermissionDecision, error) {
			return PermissionDecision{OptionID: "allow:once"}, nil
		},
	})
	if !errors.Is(err, ErrOutcomeUncertain) {
		t.Fatalf("post-approval protocol error = %v", err)
	}
}

func TestTaskMCPListsAndForwardsOnlyTerminalTools(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	t.Setenv(terminalAddressEnvironment, listener.Addr().String())
	t.Setenv(terminalTokenEnvironment, "test-token")
	received := make(chan map[string]any, 1)
	go func() {
		connection, acceptErr := listener.Accept()
		if acceptErr != nil {
			return
		}
		defer connection.Close()
		line, _ := bufio.NewReader(connection).ReadBytes('\n')
		var request map[string]any
		_ = json.Unmarshal(line, &request)
		received <- request
		_, _ = fmt.Fprintln(connection, `{"ok":true}`)
	}()
	input := strings.NewReader(
		`{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}` + "\n" +
			`{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}` + "\n" +
			`{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"task.finish_execution","arguments":{}}}` + "\n")
	var output bytes.Buffer
	if err := runTaskMCP(input, &output); err != nil {
		t.Fatal(err)
	}
	lines := strings.Split(strings.TrimSpace(output.String()), "\n")
	if len(lines) != 3 || !strings.Contains(lines[0], `"protocolVersion":"2025-06-18"`) || !strings.Contains(lines[1], "task.continue_execution") {
		t.Fatalf("MCP output = %q", output.String())
	}
	request := <-received
	if request["token"] != "test-token" || request["tool"] != "task.finish_execution" {
		t.Fatalf("forwarded request = %#v", request)
	}
}

func TestTerminalBridgeChecksTokenSizeLifetimeAndSingleUse(t *testing.T) {
	send := func(t *testing.T, bridge *terminalBridge, token string, payload []byte) string {
		t.Helper()
		connection, err := net.Dial("tcp", bridge.address)
		if err != nil {
			t.Fatal(err)
		}
		defer connection.Close()
		if payload == nil {
			payload, _ = json.Marshal(map[string]any{"token": token, "tool": "task.finish_execution", "arguments": map[string]any{}})
		}
		_, _ = connection.Write(append(payload, '\n'))
		line, _ := bufio.NewReader(connection).ReadString('\n')
		return line
	}

	valid, err := startTerminalBridge(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if response := send(t, valid, valid.token, nil); !strings.Contains(response, `"ok":true`) {
		t.Fatalf("valid response = %q", response)
	}
	if result := <-valid.result; result.err != nil || result.call.Name != "task.finish_execution" {
		t.Fatalf("terminal result = %#v", result)
	}
	valid.close()
	if connection, err := net.DialTimeout("tcp", valid.address, 50*time.Millisecond); err == nil {
		_ = connection.Close()
		t.Fatal("one-use bridge accepted a second connection")
	}

	invalid, _ := startTerminalBridge(context.Background())
	if response := send(t, invalid, strings.Repeat("0", len(invalid.token)), nil); !strings.Contains(response, "invalid_or_expired_token") {
		t.Fatalf("invalid response = %q", response)
	}
	if result := <-invalid.result; result.err == nil {
		t.Fatal("invalid token reached the terminal")
	}
	invalid.close()

	oversized, _ := startTerminalBridge(context.Background())
	_ = send(t, oversized, oversized.token, bytes.Repeat([]byte("x"), terminalRequestLimit+1))
	if result := <-oversized.result; result.err == nil {
		t.Fatal("oversized terminal request was accepted")
	}
	oversized.close()

	ctx, cancel := context.WithCancel(context.Background())
	expired, _ := startTerminalBridge(ctx)
	cancel()
	if result := <-expired.result; result.err == nil {
		t.Fatal("cancelled terminal bridge stayed active")
	}
	expired.close()
}

func TestACPExecutorHelperProcess(t *testing.T) {
	marker := -1
	for index, argument := range os.Args {
		if argument == "NOEMA_ACP_EXECUTOR_HELPER" || argument == "NOEMA_ACP_EXECUTOR_CHILD" {
			marker = index
			break
		}
	}
	if marker < 0 {
		return
	}
	if os.Args[marker] == "NOEMA_ACP_EXECUTOR_CHILD" {
		time.Sleep(400 * time.Millisecond)
		if err := os.WriteFile(os.Args[marker+1], []byte("survived"), 0o600); err != nil {
			t.Fatal(err)
		}
		return
	}
	mode, terminal, childMarker := os.Args[marker+1], os.Args[marker+2], os.Args[marker+3]
	reader := bufio.NewReader(os.Stdin)
	request := readExecutorRequest(t, reader)
	if request.Method != "initialize" {
		t.Fatalf("first method = %q", request.Method)
	}
	writeExecutorResponse(t, request.ID, map[string]any{"protocolVersion": 1, "agentCapabilities": map[string]any{}, "authMethods": []any{}})
	request = readExecutorRequest(t, reader)
	if request.Method != "session/new" {
		t.Fatalf("second method = %q", request.Method)
	}
	var sessionParams struct {
		Cwd        string `json:"cwd"`
		MCPServers []struct {
			Command string `json:"command"`
			Args    []string
			Env     []struct{ Name, Value string }
		} `json:"mcpServers"`
	}
	if json.Unmarshal(request.Params, &sessionParams) != nil || !filepath.IsAbs(sessionParams.Cwd) || len(sessionParams.MCPServers) != 1 {
		t.Fatalf("session params = %s", request.Params)
	}
	environment := map[string]string{}
	for _, value := range sessionParams.MCPServers[0].Env {
		environment[value.Name] = value.Value
	}
	writeExecutorResponse(t, request.ID, map[string]any{"sessionId": "session:test"})
	if mode == "stalled_prompt" {
		for {
			time.Sleep(time.Hour)
		}
	}
	request = readExecutorRequest(t, reader)
	if request.Method != "session/prompt" || !bytes.Contains(request.Params, []byte("current Task")) && mode != "hang" {
		t.Fatalf("prompt = %s", request.Params)
	}
	if mode == "hang" {
		process, err := os.StartProcess(os.Args[0], []string{os.Args[0], "-test.run=^TestACPExecutorHelperProcess$", "--", "NOEMA_ACP_EXECUTOR_CHILD", childMarker}, &os.ProcAttr{Files: []*os.File{os.Stdin, os.Stdout, os.Stderr}})
		if err != nil {
			t.Fatal(err)
		}
		_ = process.Release()
		for {
			time.Sleep(time.Hour)
		}
	}
	writeExecutorNotification(t, "session/update", map[string]any{"sessionId": "session:test", "update": map[string]any{"sessionUpdate": "agent_message_chunk", "content": map[string]any{"type": "text", "text": "Working"}}})
	writeExecutorRequest(t, 88, "session/request_permission", map[string]any{
		"sessionId": "session:test", "toolCall": map[string]any{"toolCallId": "tool:write", "title": "Write file"},
		"options": []any{map[string]any{"optionId": "allow:once", "name": "Allow once", "kind": "allow_once"}},
	})
	permission := readExecutorRequest(t, reader)
	if permission.ID != 88 || !bytes.Contains(permission.Result, []byte(`"selected"`)) {
		t.Fatalf("permission response = %#v", permission)
	}
	if mode == "malformed_after_approval" {
		_, _ = fmt.Fprintln(os.Stdout, "not-json")
		for {
			time.Sleep(time.Hour)
		}
	}
	arguments := map[string]any{}
	if terminal == "task.report_blocked" {
		arguments = map[string]any{"gate_kind": "clarification", "question": "Which target?"}
	}
	connection, err := net.Dial("tcp", environment[terminalAddressEnvironment])
	if err != nil {
		t.Fatal(err)
	}
	defer connection.Close()
	encoded, _ := json.Marshal(map[string]any{"token": environment[terminalTokenEnvironment], "tool": terminal, "arguments": arguments})
	_, _ = connection.Write(append(encoded, '\n'))
	_, _ = bufio.NewReader(connection).ReadBytes('\n')
	for {
		time.Sleep(time.Hour)
	}
}

type executorHelperMessage struct {
	ID     int             `json:"id"`
	Method string          `json:"method"`
	Params json.RawMessage `json:"params"`
	Result json.RawMessage `json:"result"`
}

func readExecutorRequest(t *testing.T, reader *bufio.Reader) executorHelperMessage {
	t.Helper()
	line, err := reader.ReadBytes('\n')
	if err != nil {
		t.Fatal(err)
	}
	var request executorHelperMessage
	if json.Unmarshal(line, &request) != nil {
		t.Fatalf("invalid request: %s", line)
	}
	return request
}

func writeExecutorResponse(t *testing.T, id int, result any) {
	t.Helper()
	if err := json.NewEncoder(os.Stdout).Encode(map[string]any{"jsonrpc": "2.0", "id": id, "result": result}); err != nil {
		t.Fatal(err)
	}
}

func writeExecutorNotification(t *testing.T, method string, params any) {
	t.Helper()
	if err := json.NewEncoder(os.Stdout).Encode(map[string]any{"jsonrpc": "2.0", "method": method, "params": params}); err != nil {
		t.Fatal(err)
	}
}

func writeExecutorRequest(t *testing.T, id int, method string, params any) {
	t.Helper()
	if err := json.NewEncoder(os.Stdout).Encode(map[string]any{"jsonrpc": "2.0", "id": id, "method": method, "params": params}); err != nil {
		t.Fatal(err)
	}
}

func executorHelperCommand(t *testing.T, mode, terminal, marker string) Command {
	t.Helper()
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	return Command{Path: executable, Args: []string{"-test.run=^TestACPExecutorHelperProcess$", "--", "NOEMA_ACP_EXECUTOR_HELPER", mode, terminal, marker}}
}
