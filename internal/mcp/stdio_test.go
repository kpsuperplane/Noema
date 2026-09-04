package mcp

import (
	"bytes"
	"context"
	"errors"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"testing"
	"time"

	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

type echoInput struct {
	Text string `json:"text"`
}

type echoOutput struct {
	Text      string `json:"text"`
	Allowed   string `json:"allowed"`
	Inherited string `json:"inherited"`
}

func TestStdioCall(t *testing.T) {
	if os.Getenv("NOEMA_MCP_HELPER") == "echo" {
		server := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "test", Version: "1"}, nil)
		mcpsdk.AddTool(server, &mcpsdk.Tool{Name: "echo"}, func(
			_ context.Context,
			_ *mcpsdk.CallToolRequest,
			input echoInput,
		) (*mcpsdk.CallToolResult, echoOutput, error) {
			return nil, echoOutput{
				Text:      input.Text,
				Allowed:   os.Getenv("NOEMA_ALLOWED"),
				Inherited: os.Getenv("NOEMA_INHERITED_TEST"),
			}, nil
		})
		if err := server.Run(context.Background(), &mcpsdk.StdioTransport{}); err != nil {
			t.Fatal(err)
		}
		return
	}

	t.Setenv("NOEMA_INHERITED_TEST", "must-not-cross")
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	result, err := Call(context.Background(), Command{
		Path: executable,
		Args: []string{"-test.run=^TestStdioCall$"},
		Env:  []string{"NOEMA_MCP_HELPER=echo", "NOEMA_ALLOWED=yes"},
	}, "echo", echoInput{Text: "hello"})
	if err != nil {
		t.Fatal(err)
	}
	output, ok := result.StructuredContent.(map[string]any)
	if !ok {
		t.Fatalf("structured output has type %T", result.StructuredContent)
	}
	if output["text"] != "hello" || output["allowed"] != "yes" || output["inherited"] != "" {
		t.Fatalf("unexpected output: %#v", output)
	}
}

func TestBoundsAndCancellationStopProcess(t *testing.T) {
	mode := os.Getenv("NOEMA_MCP_HELPER")
	if mode == "descendant" {
		writePID(os.Getenv("NOEMA_DESCENDANT_PID_FILE"))
		for {
			time.Sleep(time.Hour)
		}
	}
	if mode == "oversize" {
		writePID(os.Getenv("NOEMA_PID_FILE"))
		_, _ = os.Stdout.WriteString(strings.Repeat("x", 513) + "\n")
		for {
			time.Sleep(time.Hour)
		}
	}
	if mode == "cancel" || mode == "leader-exit" {
		writePID(os.Getenv("NOEMA_PID_FILE"))
		if mode == "leader-exit" {
			descendant := exec.Command(os.Args[0], "-test.run=^TestBoundsAndCancellationStopProcess$")
			descendant.Env = []string{
				"NOEMA_MCP_HELPER=descendant",
				"NOEMA_DESCENDANT_PID_FILE=" + os.Getenv("NOEMA_DESCENDANT_PID_FILE"),
			}
			if err := descendant.Start(); err != nil {
				t.Fatal(err)
			}
			for range 100 {
				if _, err := os.Stat(os.Getenv("NOEMA_DESCENDANT_PID_FILE")); err == nil {
					return
				}
				time.Sleep(10 * time.Millisecond)
			}
			t.Fatal("descendant did not start")
		}
		server := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "test", Version: "1"}, nil)
		server.AddTool(&mcpsdk.Tool{Name: "wait", InputSchema: map[string]any{"type": "object"}}, func(
			ctx context.Context,
			_ *mcpsdk.CallToolRequest,
		) (*mcpsdk.CallToolResult, error) {
			<-ctx.Done()
			return nil, ctx.Err()
		})
		if err := server.Run(context.Background(), &mcpsdk.StdioTransport{}); err != nil {
			t.Fatal(err)
		}
		return
	}

	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	writer := &testWriteCloser{}
	bounded := &boundedWriter{writer: writer, limit: 5}
	if _, err := bounded.Write([]byte("123")); err != nil {
		t.Fatal(err)
	}
	if _, err := bounded.Write([]byte("456\n")); !errors.Is(err, ErrMessageTooLarge) {
		t.Fatalf("split oversized write returned %v", err)
	}
	if writer.String() != "123" {
		t.Fatalf("oversized write reached child: %q", writer.String())
	}
	for _, test := range []struct {
		name  string
		mode  string
		limit int
		ctx   func() (context.Context, context.CancelFunc)
		check func(error) bool
		desc  bool
	}{
		{
			name:  "oversize input",
			mode:  "oversize",
			limit: 512,
			ctx: func() (context.Context, context.CancelFunc) {
				return context.WithTimeout(context.Background(), 3*time.Second)
			},
			check: func(err error) bool { return errors.Is(err, ErrMessageTooLarge) },
		},
		{
			name: "cancel call",
			mode: "cancel",
			ctx: func() (context.Context, context.CancelFunc) {
				return context.WithTimeout(context.Background(), 100*time.Millisecond)
			},
			check: func(err error) bool {
				return errors.Is(err, context.DeadlineExceeded)
			},
		},
		{
			name: "leader exits first",
			mode: "leader-exit",
			ctx: func() (context.Context, context.CancelFunc) {
				return context.WithTimeout(context.Background(), 3*time.Second)
			},
			check: func(err error) bool { return err != nil },
			desc:  true,
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			pidFile := t.TempDir() + string(os.PathSeparator) + "pid"
			descendantPIDFile := pidFile + "-descendant"
			ctx, cancel := test.ctx()
			defer cancel()
			_, err := Call(ctx, Command{
				Path: executable,
				Args: []string{"-test.run=^TestBoundsAndCancellationStopProcess$"},
				Env: []string{
					"NOEMA_MCP_HELPER=" + test.mode,
					"NOEMA_PID_FILE=" + pidFile,
					"NOEMA_DESCENDANT_PID_FILE=" + descendantPIDFile,
				},
				MaxMessageBytes: test.limit,
			}, "wait", map[string]any{})
			if !test.check(err) {
				t.Fatalf("unexpected error: %v", err)
			}
			pid := readPID(t, pidFile)
			if !waitProcessExit(pid) {
				t.Fatalf("MCP process %d is still alive", pid)
			}
			if test.desc {
				descendantPID := readPID(t, descendantPIDFile)
				if !waitProcessExit(descendantPID) {
					t.Fatalf("MCP descendant %d is still alive", descendantPID)
				}
			}
		})
	}
}

func waitProcessExit(pid int) bool {
	for range 100 {
		if !processAlive(pid) {
			return true
		}
		time.Sleep(10 * time.Millisecond)
	}
	return false
}

func writePID(path string) {
	_ = os.WriteFile(path, []byte(strconv.Itoa(os.Getpid())), 0o600)
}

func readPID(t *testing.T, path string) int {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	pid, err := strconv.Atoi(strings.TrimSpace(string(data)))
	if err != nil {
		t.Fatal(err)
	}
	return pid
}

type testWriteCloser struct{ bytes.Buffer }

func (*testWriteCloser) Close() error { return nil }
