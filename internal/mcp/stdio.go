package mcp

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"sync"
	"time"

	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

const defaultMessageLimit = 1 << 20

var ErrMessageTooLarge = errors.New("MCP message exceeds the size limit")

// Command identifies one MCP server process. Environment entries are the only
// variables provided to the child.
type Command struct {
	Path            string
	Args            []string
	Env             []string
	MaxMessageBytes int
}

// Call starts a server, initializes MCP, lists its tools, and calls one listed
// tool. Closing the session stops the complete child process tree.
func Call(ctx context.Context, command Command, tool string, arguments any) (result *mcpsdk.CallToolResult, err error) {
	transport, err := newCommandTransport(command)
	if err != nil {
		return nil, err
	}
	client := mcpsdk.NewClient(&mcpsdk.Implementation{Name: "noema", Version: "go-migration"}, nil)
	session, err := client.Connect(ctx, transport, nil)
	if err != nil {
		return nil, transportError("initialize MCP server", err)
	}
	defer func() { err = errors.Join(err, session.Close()) }()

	listed, err := session.ListTools(ctx, nil)
	if err != nil {
		return nil, transportError("list MCP tools", err)
	}
	found := false
	for _, candidate := range listed.Tools {
		if candidate.Name == tool {
			found = true
			break
		}
	}
	if !found {
		return nil, fmt.Errorf("MCP tool %q is not listed", tool)
	}

	result, err = session.CallTool(ctx, &mcpsdk.CallToolParams{Name: tool, Arguments: arguments})
	if err != nil {
		return nil, transportError(fmt.Sprintf("call MCP tool %q", tool), err)
	}
	return result, nil
}

func transportError(action string, err error) error {
	if strings.Contains(err.Error(), ErrMessageTooLarge.Error()) {
		return fmt.Errorf("%s: %w: %v", action, ErrMessageTooLarge, err)
	}
	return fmt.Errorf("%s: %w", action, err)
}

type commandTransport struct {
	command Command
	limit   int
}

func newCommandTransport(command Command) (*commandTransport, error) {
	if command.Path == "" || !filepath.IsAbs(command.Path) {
		return nil, errors.New("MCP command path must be absolute")
	}
	limit := command.MaxMessageBytes
	if limit == 0 {
		limit = defaultMessageLimit
	}
	if limit < 1 {
		return nil, errors.New("MCP message limit must be positive")
	}
	return &commandTransport{command: command, limit: limit}, nil
}

func (t *commandTransport) Connect(ctx context.Context) (mcpsdk.Connection, error) {
	cmd := exec.Command(t.command.Path, t.command.Args...)
	cmd.Env = slices.Clone(t.command.Env)
	if cmd.Env == nil {
		cmd.Env = []string{}
	}
	configureProcess(cmd)

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, fmt.Errorf("open MCP stdout: %w", err)
	}
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, fmt.Errorf("open MCP stdin: %w", err)
	}
	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("start MCP server: %w", err)
	}
	tree, err := attachProcess(cmd)
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		return nil, fmt.Errorf("isolate MCP process tree: %w", err)
	}
	process := newManagedProcess(cmd, tree)
	inner, err := (&mcpsdk.IOTransport{
		Reader: &boundedReader{reader: bufio.NewReader(stdout), closer: stdout, limit: t.limit},
		Writer: &boundedWriter{writer: stdin, limit: t.limit},
	}).Connect(ctx)
	if err != nil {
		_ = process.stop()
		return nil, err
	}
	return &managedConnection{Connection: inner, process: process}, nil
}

type boundedReader struct {
	reader  *bufio.Reader
	closer  io.Closer
	limit   int
	pending []byte
}

func (r *boundedReader) Read(p []byte) (int, error) {
	if len(r.pending) == 0 {
		for {
			part, err := r.reader.ReadSlice('\n')
			if len(r.pending)+len(part) > r.limit {
				return 0, ErrMessageTooLarge
			}
			r.pending = append(r.pending, part...)
			if err == nil || !errors.Is(err, bufio.ErrBufferFull) {
				if len(r.pending) == 0 {
					return 0, err
				}
				break
			}
		}
	}
	n := copy(p, r.pending)
	r.pending = r.pending[n:]
	return n, nil
}

func (r *boundedReader) Close() error { return r.closer.Close() }

type boundedWriter struct {
	writer io.WriteCloser
	limit  int
	mu     sync.Mutex
	line   int
}

func (w *boundedWriter) Write(p []byte) (int, error) {
	w.mu.Lock()
	defer w.mu.Unlock()
	line := w.line
	for _, b := range p {
		line++
		if line > w.limit {
			return 0, ErrMessageTooLarge
		}
		if b == '\n' {
			line = 0
		}
	}
	n, err := w.writer.Write(p)
	for _, b := range p[:n] {
		w.line++
		if b == '\n' {
			w.line = 0
		}
	}
	return n, err
}

func (w *boundedWriter) Close() error { return w.writer.Close() }

type managedConnection struct {
	mcpsdk.Connection
	process *managedProcess
	once    sync.Once
	err     error
}

func (c *managedConnection) Close() error {
	c.once.Do(func() {
		c.err = errors.Join(c.Connection.Close(), c.process.stop())
	})
	return c.err
}

type managedProcess struct {
	tree processTree
	done chan error
	once sync.Once
	err  error
}

func newManagedProcess(cmd *exec.Cmd, tree processTree) *managedProcess {
	p := &managedProcess{tree: tree, done: make(chan error, 1)}
	go func() { p.done <- cmd.Wait() }()
	return p
}

func (p *managedProcess) stop() error {
	p.once.Do(func() {
		defer p.tree.close()
		if p.wait(200 * time.Millisecond) {
			return
		}
		_ = p.tree.terminate(false)
		if p.wait(500 * time.Millisecond) {
			return
		}
		_ = p.tree.terminate(true)
		if !p.wait(2 * time.Second) {
			p.err = errors.New("MCP process tree did not stop")
		}
	})
	return p.err
}

func (p *managedProcess) wait(timeout time.Duration) bool {
	select {
	case <-p.done:
		return true
	case <-time.After(timeout):
		return false
	}
}
