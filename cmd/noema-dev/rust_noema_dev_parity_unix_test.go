//go:build unix

package main

import (
	"fmt"
	"os/exec"
	"strconv"
	"strings"
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

func spawnRustDevWatcher(t *testing.T) *exec.Cmd {
	t.Helper()
	command := exec.Command("sh", "-c", "trap 'exit 0' TERM; while :; do sleep 1; done")
	command.Stdin, command.Stdout, command.Stderr = nil, nil, nil
	configureProcess(command)
	if err := command.Start(); err != nil {
		t.Fatal(err)
	}
	return command
}

// Rust source: crates/noema-dev/src/main.rs::supervisor_stops_watchers_when_shutdown_signal_arrives (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustDev_supervisor_stops_watchers_when_shutdown_signal_arrives(t *testing.T) {
	web := spawnRustDevWatcher(t)
	server := spawnRustDevWatcher(t)
	bridge := spawnRustDevWatcher(t)
	watchers := []*exec.Cmd{web, server, bridge}
	for _, watcher := range watchers {
		watcher := watcher
		t.Cleanup(func() {
			if watcher.ProcessState == nil {
				stopProcess(watcher)
				_ = watcher.Wait()
			}
		})
	}
	for _, watcher := range watchers {
		stopProcess(watcher)
	}
	for _, watcher := range watchers {
		_ = watcher.Wait()
		if watcher.ProcessState == nil {
			t.Fatal("watcher did not exit")
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
