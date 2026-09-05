// Package acp implements the bounded ACP stdio operations used by Settings.
package acp

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"slices"
	"strings"
	"sync"
	"time"
)

const (
	protocolVersion = 1
	probeTimeout    = 30 * time.Second
	messageLimit    = 1 << 20
)

var (
	ErrInitializationFailed   = errors.New("ACP initialization failed")
	ErrInitializationTimedOut = errors.New("ACP initialization timed out")
	ErrAuthenticationFailed   = errors.New("ACP authentication failed")
)

// Command identifies one ACP process and its exact arguments.
type Command struct {
	Path string
	Args []string
}

// ProbeResult contains the safe metadata advertised during initialization.
type ProbeResult struct {
	ProtocolVersion       int
	ImplementationName    *string
	ImplementationVersion *string
	Capabilities          map[string]any
}

// Probe initializes one ACP process. It stops the process after 30 seconds.
func Probe(ctx context.Context, command Command) (ProbeResult, error) {
	return probeWithTimeout(ctx, command, probeTimeout)
}

func probeWithTimeout(ctx context.Context, command Command, timeout time.Duration) (result ProbeResult, err error) {
	if err := ctx.Err(); err != nil {
		return ProbeResult{}, fmt.Errorf("probe ACP: %w", err)
	}
	probeCtx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()
	session, err := start(command)
	if err != nil {
		return ProbeResult{}, fmt.Errorf("%w: %v", ErrInitializationFailed, err)
	}
	defer func() { err = errors.Join(err, session.close()) }()

	var initialized initializeResult
	err = session.exchange(probeCtx, 1, "initialize", initializeParams{
		ProtocolVersion:    protocolVersion,
		ClientCapabilities: map[string]any{},
		ClientInfo:         implementation{Name: "noema", Version: "go-migration"},
	}, &initialized)
	if err != nil {
		if errors.Is(probeCtx.Err(), context.DeadlineExceeded) && ctx.Err() == nil {
			return ProbeResult{}, ErrInitializationTimedOut
		}
		if ctx.Err() != nil {
			return ProbeResult{}, fmt.Errorf("probe ACP: %w", ctx.Err())
		}
		return ProbeResult{}, ErrInitializationFailed
	}
	if initialized.ProtocolVersion != protocolVersion {
		return ProbeResult{}, ErrInitializationFailed
	}
	result = ProbeResult{
		ProtocolVersion: protocolVersion,
		Capabilities:    probeCapabilities(initialized),
	}
	if initialized.AgentInfo != nil {
		result.ImplementationName = &initialized.AgentInfo.Name
		result.ImplementationVersion = &initialized.AgentInfo.Version
	}
	return result, nil
}

// Authenticate initializes one ACP process and calls its named auth method.
func Authenticate(ctx context.Context, command Command, methodID string) (err error) {
	if err := ctx.Err(); err != nil {
		return fmt.Errorf("authenticate ACP: %w", err)
	}
	if strings.TrimSpace(methodID) == "" {
		return ErrAuthenticationFailed
	}
	session, err := start(command)
	if err != nil {
		return fmt.Errorf("%w: %v", ErrAuthenticationFailed, err)
	}
	defer func() { err = errors.Join(err, session.close()) }()

	var initialized initializeResult
	if err := session.exchange(ctx, 1, "initialize", initializeParams{
		ProtocolVersion:    protocolVersion,
		ClientCapabilities: map[string]any{},
		ClientInfo:         implementation{Name: "noema", Version: "go-migration"},
	}, &initialized); err != nil || initialized.ProtocolVersion != protocolVersion {
		return authenticationError(ctx)
	}
	var authenticated map[string]any
	if err := session.exchange(ctx, 2, "authenticate", map[string]string{"methodId": methodID}, &authenticated); err != nil {
		return authenticationError(ctx)
	}
	return nil
}

func authenticationError(ctx context.Context) error {
	if ctx.Err() != nil {
		return fmt.Errorf("authenticate ACP: %w", ctx.Err())
	}
	return ErrAuthenticationFailed
}

type implementation struct {
	Name    string `json:"name"`
	Version string `json:"version"`
}

type initializeParams struct {
	ProtocolVersion    int            `json:"protocolVersion"`
	ClientCapabilities map[string]any `json:"clientCapabilities"`
	ClientInfo         implementation `json:"clientInfo"`
}

type initializeResult struct {
	ProtocolVersion   int             `json:"protocolVersion"`
	AgentCapabilities json.RawMessage `json:"agentCapabilities"`
	AuthMethods       json.RawMessage `json:"authMethods"`
	AgentInfo         *implementation `json:"agentInfo"`
}

func probeCapabilities(initialized initializeResult) map[string]any {
	agent := map[string]any{}
	candidates := []any{}
	decodeOptional(initialized.AgentCapabilities, &agent)
	decodeOptional(initialized.AuthMethods, &candidates)
	methods := make([]any, 0, len(candidates))
	for _, candidate := range candidates {
		method, ok := candidate.(map[string]any)
		if !ok || !nonEmptyString(method["id"]) || !nonEmptyString(method["name"]) {
			continue
		}
		methods = append(methods, method)
	}
	return map[string]any{"agent": agent, "authMethods": methods}
}

func decodeOptional(raw json.RawMessage, target any) {
	if len(raw) == 0 || bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
		return
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	_ = decoder.Decode(target)
}

func nonEmptyString(value any) bool {
	text, ok := value.(string)
	return ok && strings.TrimSpace(text) != ""
}

type rpcRequest struct {
	JSONRPC string `json:"jsonrpc"`
	ID      int    `json:"id"`
	Method  string `json:"method"`
	Params  any    `json:"params"`
}

type rpcResponse struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      *int            `json:"id"`
	Result  json.RawMessage `json:"result"`
	Error   *struct {
		Code int `json:"code"`
	} `json:"error"`
}

type session struct {
	stdin   io.WriteCloser
	stdout  io.ReadCloser
	reader  *bufio.Reader
	process *managedProcess
	once    sync.Once
}

func start(command Command) (*session, error) {
	if strings.TrimSpace(command.Path) == "" {
		return nil, errors.New("ACP command is empty")
	}
	cmd := exec.Command(command.Path, slices.Clone(command.Args)...)
	configureProcess(cmd)
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, err
	}
	stdin, err := cmd.StdinPipe()
	if err != nil {
		_ = stdout.Close()
		return nil, err
	}
	if err := cmd.Start(); err != nil {
		_ = stdin.Close()
		_ = stdout.Close()
		return nil, err
	}
	tree, err := attachProcess(cmd)
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		return nil, err
	}
	return &session{
		stdin: stdin, stdout: stdout, reader: bufio.NewReader(stdout),
		process: newManagedProcess(cmd, tree),
	}, nil
}

func (s *session) exchange(ctx context.Context, id int, method string, params any, target any) error {
	message, err := json.Marshal(rpcRequest{JSONRPC: "2.0", ID: id, Method: method, Params: params})
	if err != nil {
		return err
	}
	message = append(message, '\n')
	if err := s.write(ctx, message); err != nil {
		return err
	}
	for {
		line, err := await(ctx, func() ([]byte, error) { return readMessage(s.reader) })
		if err != nil {
			return err
		}
		var response rpcResponse
		if err := json.Unmarshal(line, &response); err != nil {
			return err
		}
		if response.ID == nil {
			continue
		}
		if response.JSONRPC != "2.0" || *response.ID != id || response.Error != nil || len(response.Result) == 0 {
			return errors.New("invalid ACP JSON-RPC response")
		}
		return json.Unmarshal(response.Result, target)
	}
}

func writeMessage(writer io.Writer, message []byte) error {
	for len(message) > 0 {
		written, err := writer.Write(message)
		if err != nil {
			return err
		}
		if written == 0 {
			return io.ErrShortWrite
		}
		message = message[written:]
	}
	return nil
}

func readMessage(reader *bufio.Reader) ([]byte, error) {
	var message []byte
	for {
		part, err := reader.ReadSlice('\n')
		if len(message)+len(part) > messageLimit {
			return nil, errors.New("ACP message exceeds the size limit")
		}
		message = append(message, part...)
		if err == nil {
			return message, nil
		}
		if !errors.Is(err, bufio.ErrBufferFull) {
			return nil, err
		}
	}
}

type asyncResult[T any] struct {
	value T
	err   error
}

func await[T any](ctx context.Context, call func() (T, error)) (T, error) {
	result := make(chan asyncResult[T], 1)
	go func() {
		value, err := call()
		result <- asyncResult[T]{value: value, err: err}
	}()
	select {
	case <-ctx.Done():
		var zero T
		return zero, ctx.Err()
	case result := <-result:
		return result.value, result.err
	}
}

func (s *session) close() (err error) {
	s.once.Do(func() {
		_ = s.stdin.Close()
		_ = s.stdout.Close()
		err = s.process.stop()
	})
	return err
}
