package acp

import (
	"bufio"
	"encoding/json"
	"io"
	"net"
	"os"
)

// RunTaskMCPIfRequested runs the internal terminal MCP helper.
func RunTaskMCPIfRequested(args []string, input io.Reader, output io.Writer) (bool, int) {
	if len(args) != 2 || args[1] != "--acp-task-mcp" {
		return false, 0
	}
	if runTaskMCP(input, output) != nil {
		return true, 1
	}
	return true, 0
}

func runTaskMCP(input io.Reader, output io.Writer) error {
	reader := bufio.NewReader(input)
	for {
		line, err := readMessage(reader)
		if err != nil {
			if err == io.EOF {
				return nil
			}
			return err
		}
		var request struct {
			JSONRPC string          `json:"jsonrpc"`
			ID      json.RawMessage `json:"id"`
			Method  string          `json:"method"`
			Params  json.RawMessage `json:"params"`
		}
		if json.Unmarshal(line, &request) != nil || request.JSONRPC != "2.0" {
			return io.ErrUnexpectedEOF
		}
		if len(request.ID) == 0 {
			continue
		}
		var result any
		switch request.Method {
		case "initialize":
			var params struct {
				ProtocolVersion string `json:"protocolVersion"`
			}
			_ = json.Unmarshal(request.Params, &params)
			result = map[string]any{
				"protocolVersion": terminalMCPVersion(params.ProtocolVersion), "capabilities": map[string]any{"tools": map[string]any{}},
				"serverInfo": map[string]string{"name": "noema-task-terminal", "version": "go-migration"},
			}
		case "tools/list":
			result = map[string]any{"tools": taskTerminalTools()}
		case "tools/call":
			var call struct {
				Name      string          `json:"name"`
				Arguments json.RawMessage `json:"arguments"`
			}
			if json.Unmarshal(request.Params, &call) != nil {
				return writeMCPError(output, request.ID, -32602, "invalid terminal call")
			}
			if err := forwardTerminal(call.Name, call.Arguments); err != nil {
				return writeMCPError(output, request.ID, -32603, "runtime rejected the terminal submission")
			}
			result = map[string]any{"content": []any{map[string]string{"type": "text", "text": "Noema accepted the terminal submission."}}}
		default:
			return writeMCPError(output, request.ID, -32601, "method not found")
		}
		if err := writeMCPResult(output, request.ID, result); err != nil {
			return err
		}
	}
}

func terminalMCPVersion(requested string) string {
	switch requested {
	case "2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25":
		return requested
	default:
		return "2025-11-25"
	}
}

func taskTerminalTools() []any {
	empty := map[string]any{"type": "object", "properties": map[string]any{}, "additionalProperties": false}
	blocked := map[string]any{
		"type": "object", "additionalProperties": false,
		"properties": map[string]any{
			"gate_kind":         map[string]any{"type": "string", "enum": []string{"clarification", "approval"}},
			"question":          map[string]any{"type": "string", "minLength": 1, "maxLength": 4000},
			"context_markdown":  map[string]any{"type": "string", "maxLength": 20000},
			"suggested_answers": map[string]any{"type": "array", "maxItems": 8, "items": map[string]any{"type": "string", "minLength": 1, "maxLength": 1000}},
		}, "required": []string{"gate_kind", "question"},
	}
	return []any{
		map[string]any{"name": "task.finish_execution", "description": "Finish after RESULT.md satisfies the Task.", "inputSchema": empty},
		map[string]any{"name": "task.continue_execution", "description": "Continue in another run from current Task files.", "inputSchema": empty},
		map[string]any{"name": "task.report_blocked", "description": "Request one specific human decision or missing input.", "inputSchema": blocked},
	}
}

func forwardTerminal(name string, arguments json.RawMessage) error {
	address, token := os.Getenv(terminalAddressEnvironment), os.Getenv(terminalTokenEnvironment)
	connection, err := net.Dial("tcp", address)
	if err != nil {
		return err
	}
	defer connection.Close()
	request, err := json.Marshal(map[string]any{"token": token, "tool": name, "arguments": arguments})
	if err != nil {
		return err
	}
	if _, err := connection.Write(append(request, '\n')); err != nil {
		return err
	}
	response, err := readMessage(bufio.NewReader(connection))
	if err != nil {
		return err
	}
	var accepted struct {
		OK bool `json:"ok"`
	}
	if json.Unmarshal(response, &accepted) != nil || !accepted.OK {
		return io.ErrUnexpectedEOF
	}
	return nil
}

func writeMCPResult(output io.Writer, id json.RawMessage, result any) error {
	return writeMCPMessage(output, id, "result", result)
}

func writeMCPError(output io.Writer, id json.RawMessage, code int, message string) error {
	return writeMCPMessage(output, id, "error", map[string]any{"code": code, "message": message})
}

func writeMCPMessage(output io.Writer, id json.RawMessage, field string, value any) error {
	encoded, err := json.Marshal(value)
	if err != nil {
		return err
	}
	message := append([]byte(`{"jsonrpc":"2.0","id":`), id...)
	message = append(message, []byte(`,"`+field+`":`)...)
	message = append(message, encoded...)
	message = append(message, '}', '\n')
	_, err = output.Write(message)
	return err
}
