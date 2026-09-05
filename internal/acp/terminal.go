package acp

import (
	"bufio"
	"context"
	"crypto/rand"
	"crypto/subtle"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"net"
	"time"
)

const (
	terminalAddressEnvironment = "NOEMA_ACP_TASK_BRIDGE_ADDR"
	terminalTokenEnvironment   = "NOEMA_ACP_TASK_TOKEN"
	terminalRequestLimit       = 1 << 20
)

type terminalResult struct {
	call TerminalCall
	err  error
}

type terminalBridge struct {
	address string
	token   string
	result  chan terminalResult
	close   context.CancelFunc
}

func startTerminalBridge(parent context.Context) (*terminalBridge, error) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, err
	}
	bytes := make([]byte, 32)
	if _, err := rand.Read(bytes); err != nil {
		_ = listener.Close()
		return nil, err
	}
	ctx, cancel := context.WithCancel(parent)
	bridge := &terminalBridge{
		address: listener.Addr().String(), token: hex.EncodeToString(bytes),
		result: make(chan terminalResult, 1), close: cancel,
	}
	go func() {
		defer listener.Close()
		accepted := make(chan struct {
			connection net.Conn
			err        error
		}, 1)
		go func() {
			connection, err := listener.Accept()
			accepted <- struct {
				connection net.Conn
				err        error
			}{connection, err}
		}()
		select {
		case <-ctx.Done():
			bridge.result <- terminalResult{err: ctx.Err()}
		case value := <-accepted:
			if value.err != nil {
				bridge.result <- terminalResult{err: value.err}
				return
			}
			defer value.connection.Close()
			bridge.result <- readTerminal(value.connection, bridge.token)
		}
	}()
	return bridge, nil
}

func readTerminal(connection net.Conn, expected string) terminalResult {
	_ = connection.SetDeadline(time.Now().Add(30 * time.Second))
	limited := io.LimitReader(connection, terminalRequestLimit+1)
	line, err := bufio.NewReader(limited).ReadBytes('\n')
	if err != nil || len(line) > terminalRequestLimit {
		return terminalResult{err: errors.New("ACP terminal request is invalid")}
	}
	var request struct {
		Token     string          `json:"token"`
		Tool      string          `json:"tool"`
		Arguments json.RawMessage `json:"arguments"`
	}
	if json.Unmarshal(line, &request) != nil || len(request.Arguments) == 0 ||
		len(request.Token) != len(expected) || subtle.ConstantTimeCompare([]byte(request.Token), []byte(expected)) != 1 {
		_, _ = io.WriteString(connection, `{"ok":false,"error":"invalid_or_expired_token"}`+"\n")
		return terminalResult{err: errors.New("ACP terminal token is invalid")}
	}
	if request.Tool != "task.finish_execution" && request.Tool != "task.continue_execution" && request.Tool != "task.report_blocked" {
		_, _ = io.WriteString(connection, `{"ok":false,"error":"tool_not_scoped"}`+"\n")
		return terminalResult{err: errors.New("ACP terminal tool is not scoped")}
	}
	if _, err := io.WriteString(connection, `{"ok":true}`+"\n"); err != nil {
		return terminalResult{err: err}
	}
	return terminalResult{call: TerminalCall{Name: request.Tool, Arguments: request.Arguments}}
}
