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
	"log/slog"
	"os/exec"
	"slices"
	"strings"
	"sync"
	"time"

	sdk "github.com/coder/acp-go-sdk"
	"github.com/kpsuperplane/noema/internal/childenv"
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
	session, err := start(command, nil)
	if err != nil {
		return ProbeResult{}, fmt.Errorf("%w: %v", ErrInitializationFailed, err)
	}
	defer func() { err = errors.Join(err, session.close()) }()

	var initialized initializeResult
	err = session.exchange(probeCtx, "initialize", initializeParams{
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
	session, err := start(command, nil)
	if err != nil {
		return fmt.Errorf("%w: %v", ErrAuthenticationFailed, err)
	}
	defer func() { err = errors.Join(err, session.close()) }()

	var initialized initializeResult
	if err := session.exchange(ctx, "initialize", initializeParams{
		ProtocolVersion:    protocolVersion,
		ClientCapabilities: map[string]any{},
		ClientInfo:         implementation{Name: "noema", Version: "go-migration"},
	}, &initialized); err != nil || initialized.ProtocolVersion != protocolVersion {
		return authenticationError(ctx)
	}
	var authenticated map[string]any
	if err := session.exchange(ctx, "authenticate", map[string]string{"methodId": methodID}, &authenticated); err != nil {
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

type session struct {
	stdin      io.WriteCloser
	stdout     io.ReadCloser
	connection *sdk.Connection
	process    *managedProcess
	once       sync.Once
}

func start(command Command, handler sdk.MethodHandler) (*session, error) {
	if strings.TrimSpace(command.Path) == "" {
		return nil, errors.New("ACP command is empty")
	}
	cmd := exec.Command(command.Path, slices.Clone(command.Args)...)
	cmd.Env = childenv.ExternalProcess()
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
	reader := &protocolReader{reader: bufio.NewReader(stdout), ready: make(chan struct{})}
	connection := sdk.NewConnection(handler, stdin, reader)
	// SDK diagnostics can contain peer data. Install the logger before reading.
	connection.SetLogger(slog.New(slog.NewTextHandler(io.Discard, nil)))
	close(reader.ready)
	return &session{stdin: stdin, stdout: stdout, connection: connection,
		process: newManagedProcess(cmd, tree)}, nil
}

func (s *session) exchange(ctx context.Context, method string, params any, target any) error {
	// Closing stdin also interrupts an SDK write blocked by an unresponsive peer.
	stop := context.AfterFunc(ctx, func() { _ = s.stdin.Close(); _ = s.stdout.Close() })
	defer stop()
	result, err := sdk.SendRequest[json.RawMessage](s.connection, ctx, method, params)
	if err != nil {
		return err
	}
	return json.Unmarshal(result, target)
}

// protocolReader enforces Noema's message bound and fails closed on invalid JSON-RPC.
type protocolReader struct {
	reader  *bufio.Reader
	ready   chan struct{}
	pending []byte
}

func (r *protocolReader) Read(p []byte) (int, error) {
	<-r.ready
	if len(r.pending) == 0 {
		line, err := readMessage(r.reader)
		if err != nil {
			return 0, err
		}
		var envelope struct {
			JSONRPC string `json:"jsonrpc"`
		}
		if json.Unmarshal(line, &envelope) != nil || envelope.JSONRPC != "2.0" {
			return 0, errors.New("invalid ACP JSON-RPC message")
		}
		r.pending = line
	}
	n := copy(p, r.pending)
	r.pending = r.pending[n:]
	return n, nil
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

func (s *session) close() (err error) {
	s.once.Do(func() {
		_ = s.stdin.Close()
		_ = s.stdout.Close()
		err = s.process.stop()
	})
	return err
}
