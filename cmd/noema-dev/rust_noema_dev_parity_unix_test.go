//go:build unix

package main

import (
	"bufio"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
)

// Rust source: crates/noema-dev/src/main.rs::root_web_assets_use_a_public_read_umask (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_root_web_assets_use_a_public_read_umask(t *testing.T) {
	output, err := exec.Command("sh", "-c", rootWebAssetShell, "noema-web-assets-test", "sh", "-c", "umask").Output()
	if err != nil {
		t.Fatal(err)
	}
	if strings.TrimSpace(string(output)) != "0022" {
		t.Fatalf("web asset umask = %q", output)
	}
}

// Rust source: crates/noema-dev/src/main.rs::supervisor_stops_watchers_when_shutdown_signal_arrives (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_supervisor_stops_watchers_when_shutdown_signal_arrives(t *testing.T) {
	dir := t.TempDir()
	ready := filepath.Join(dir, "ready")
	if err := syscall.Mkfifo(ready, 0600); err != nil {
		t.Fatal(err)
	}
	watcherScript := `trap 'printf exited > "$3"; exit 0' TERM; printf '%s\n' "$1" > "$2"; while :; do :; done`
	names := []string{"web", "server", "bridge"}
	commands := make([][]string, 0, len(names))
	for _, name := range names {
		commands = append(commands, []string{"sh", "-c", watcherScript, "noema-dev-test-watcher", name, ready, filepath.Join(dir, name+"-exited")})
	}
	ctx, stop := signal.NotifyContext(t.Context(), shutdownSignals()...)
	defer stop()
	done := make(chan error, 1)
	go func() { done <- superviseDevProcesses(ctx, t.TempDir(), commands) }()

	readyNames := make(chan string, len(names))
	go func() {
		file, err := os.OpenFile(ready, os.O_RDWR, 0600)
		if err != nil {
			return
		}
		defer file.Close()
		scanner := bufio.NewScanner(file)
		for range names {
			if !scanner.Scan() {
				return
			}
			readyNames <- scanner.Text()
		}
	}()
	seen := make(map[string]bool, len(names))
	deadline := time.NewTimer(5 * time.Second)
	defer deadline.Stop()
	for len(seen) < len(names) {
		select {
		case name := <-readyNames:
			seen[name] = true
		case <-deadline.C:
			t.Fatal("watchers did not start")
		}
	}
	if err := syscall.Kill(os.Getpid(), syscall.SIGINT); err != nil {
		t.Fatal(err)
	}
	select {
	case err := <-done:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("supervisor did not stop watchers")
	}
	for _, name := range names {
		path := filepath.Join(dir, name+"-exited")
		contents, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("%s watcher did not exit: %v", name, err)
		}
		if string(contents) != "exited" {
			t.Fatalf("%s watcher exit marker = %q", name, contents)
		}
	}
}

// Rust source: crates/noema-dev/src/main.rs::only_watcher_exit_returns_restart_information (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_only_watcher_exit_returns_restart_information(t *testing.T) {
	processGroup := 42
	exited := &devError{kind: devProcessExited, label: "web asset watcher", status: fmt.Errorf("exit status 1"), processGroup: &processGroup}
	label, group := watcherExit(exited)
	if label != "web asset watcher" || group == nil || *group != 42 {
		t.Fatalf("watcher restart information = %q/%v", label, group)
	}
	unknown := &devError{kind: devUnknownMode, unknownMode: "unknown"}
	if label, group := watcherExit(unknown); label != "" || group != nil {
		t.Fatalf("unknown error was treated as watcher exit = %q/%v", label, group)
	}
}

// Rust source: crates/noema-dev/src/main.rs::watcher_exit_retains_process_group_after_reaping_leader (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_watcher_exit_retains_process_group_after_reaping_leader(t *testing.T) {
	command := exec.Command("sh", "-c", "exit 7")
	configureProcess(command)
	if err := command.Start(); err != nil {
		t.Fatal(err)
	}
	processGroup := command.Process.Pid
	err := waitForChild("test watcher", command)
	label, group := watcherExit(err)
	if label != "test watcher" || group == nil || *group != processGroup {
		t.Fatalf("reaped watcher process group = %q/%v, want %d", label, group, processGroup)
	}
}

// Rust source: crates/noema-dev/src/main.rs::cleanup_stops_group_after_watcher_leader_exits (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_cleanup_stops_group_after_watcher_leader_exits(t *testing.T) {
	command := exec.Command("sh", "-c", "trap '' HUP; sleep 30 &")
	configureProcess(command)
	if err := command.Start(); err != nil {
		t.Fatal(err)
	}
	processGroup := command.Process.Pid
	time.Sleep(50 * time.Millisecond)
	stopProcess(command)
	_ = command.Wait()
	status := exec.Command("kill", "-0", "--", "-"+strconv.Itoa(processGroup)).Run()
	if status == nil {
		t.Fatalf("watcher process group %d survived cleanup", processGroup)
	}
}
