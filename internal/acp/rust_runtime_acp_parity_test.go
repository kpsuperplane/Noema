package acp

import (
	"context"
	"errors"
	"net"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestRustRuntime_fake_stdio_acp_covers_probe_auth_malformed_crash_and_timeout(t *testing.T) {
	// Rust source: crates/noema-runtime/src/acp.rs::fake_stdio_acp_covers_probe_auth_malformed_crash_and_timeout.
	command, pidFile := helperCommand(t, "probe")
	probe, err := Probe(context.Background(), command)
	if err != nil || stringValue(probe.ImplementationName) != "fake-acp" || stringValue(probe.ImplementationVersion) != "1.2.3" {
		t.Fatalf("ACP probe = %#v, %v", probe, err)
	}
	if err := Authenticate(context.Background(), command, "browser"); err != nil {
		t.Fatal(err)
	}
	assertProcessStopped(t, pidFile)
	for _, mode := range []string{"rpc-error", "wrong-version", "oversize"} {
		command, pidFile := helperCommand(t, mode)
		_, err := probeWithTimeout(context.Background(), command, 100*time.Millisecond)
		if !errors.Is(err, ErrInitializationFailed) {
			t.Fatalf("mode %s error = %v", mode, err)
		}
		assertProcessStopped(t, pidFile)
	}
	command, pidFile = helperCommand(t, "hang-probe")
	_, err = probeWithTimeout(context.Background(), command, 50*time.Millisecond)
	if !errors.Is(err, ErrInitializationTimedOut) {
		t.Fatalf("timeout error = %v", err)
	}
	assertProcessStopped(t, pidFile)
}

func TestRustRuntime_fake_stdio_acp_covers_streaming_terminal_launch_and_cancellation(t *testing.T) {
	// Rust source: crates/noema-runtime/src/acp.rs::fake_stdio_acp_covers_streaming_terminal_launch_and_cancellation.
	for _, terminal := range []string{"task.finish_execution", "task.continue_execution", "task.report_blocked"} {
		command := executorHelperCommand(t, "terminal", terminal, "")
		result, err := Execute(context.Background(), ExecutorRequest{Command: command, Cwd: t.TempDir(), Prompt: "current Task data", HelperPath: command.Path})
		if err != nil || result.Name != terminal {
			t.Fatalf("terminal %s result = %#v, %v", terminal, result, err)
		}
	}
	marker := filepath.Join(t.TempDir(), "survived")
	command := executorHelperCommand(t, "hang", "", marker)
	ctx, cancel := context.WithTimeout(context.Background(), 150*time.Millisecond)
	defer cancel()
	_, err := Execute(ctx, ExecutorRequest{Command: command, Cwd: t.TempDir(), Prompt: "Task", HelperPath: command.Path})
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("cancellation error = %v", err)
	}
	time.Sleep(600 * time.Millisecond)
	if _, statErr := os.Stat(marker); !errors.Is(statErr, os.ErrNotExist) {
		t.Fatalf("ACP descendant survived: %v", statErr)
	}
}

func TestRustRuntime_terminal_tokens_are_scoped_expiring_and_one_use(t *testing.T) {
	// Rust source: crates/noema-runtime/src/acp_terminal_bridge.rs::terminal_tokens_are_scoped_expiring_and_one_use.
	first, err := startTerminalBridge(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	second, err := startTerminalBridge(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if first.token == second.token || first.address == second.address {
		t.Fatal("terminal bridge tokens or addresses were reused")
	}
	first.close()
	second.close()
	valid, err := startTerminalBridge(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	defer valid.close()
	connection, err := net.Dial("tcp", valid.address)
	if err != nil {
		t.Fatal(err)
	}
	_, _ = connection.Write([]byte(`{"token":"` + valid.token + `","tool":"task.finish_execution","arguments":{}}` + "\n"))
	if response := make([]byte, 256); func() bool {
		n, _ := connection.Read(response)
		return strings.Contains(string(response[:n]), `"ok":true`)
	}() == false {
		t.Fatal("valid terminal token was rejected")
	}
	_ = connection.Close()
	if result := <-valid.result; result.err != nil || result.call.Name != "task.finish_execution" {
		t.Fatalf("terminal result = %#v", result)
	}
}
