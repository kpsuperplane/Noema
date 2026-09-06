//go:build unix

package main

import (
	"bufio"
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
)

func TestSupervisorStopsWatcherDescendants(t *testing.T) {
	dir := t.TempDir()
	ready := filepath.Join(dir, "ready")
	if err := syscall.Mkfifo(ready, 0600); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(t.Context())
	done := make(chan error, 1)
	go func() { done <- watch(ctx, dir, []string{"sh", "-c", `sleep 30 & echo $! > ready; wait`}) }()
	started := make(chan int, 1)
	go func() {
		file, err := os.Open(ready)
		if err != nil {
			started <- 0
			return
		}
		defer file.Close()
		line, _ := bufio.NewReader(file).ReadString('\n')
		pid, _ := strconv.Atoi(strings.TrimSpace(line))
		started <- pid
	}()
	var pid int
	select {
	case pid = <-started:
		if pid == 0 {
			cancel()
			t.Fatal("watcher did not start")
		}
	case <-time.After(5 * time.Second):
		cancel()
		t.Fatal("watcher did not start")
	}
	cancel()
	if err := <-done; err != nil {
		t.Fatal(err)
	}
	// A terminated orphan can remain a zombie until the container's init reaps it.
	output, err := exec.Command("ps", "-o", "stat=", "-p", strconv.Itoa(pid)).Output()
	if err == nil && !strings.HasPrefix(strings.TrimSpace(string(output)), "Z") {
		_ = syscall.Kill(pid, syscall.SIGKILL)
		t.Fatalf("watcher descendant remains active: %s", output)
	}
}
