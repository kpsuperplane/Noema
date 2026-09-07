// Command noema-dev supervises Go and web development processes.
package main

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

const (
	webAssetWatchScript = "dev:assets"
	devAssetDirEnv      = "NOEMA_DEV_ASSET_DIR"
	rootDevAssetDir     = "/run/noema-dev/web-assets"
	rootWebAssetShell   = `umask 022; exec "$@"`
	watcherRestartDelay = 250 * time.Millisecond
)

var webServerWatchIgnoreGlobs = []string{
	"apps/web/**",
	"crates/noema-server/target/web-assets/**",
}

type devErrorKind string

const (
	devProcessExited devErrorKind = "process_exited"
	devUnknownMode   devErrorKind = "unknown_mode"
)

type devError struct {
	kind         devErrorKind
	label        string
	status       error
	processGroup *int
	unknownMode  string
}

func (e *devError) Error() string {
	switch e.kind {
	case devProcessExited:
		return fmt.Sprintf("%s exited with status %v", e.label, e.status)
	case devUnknownMode:
		return fmt.Sprintf("unknown Noema development mode: %q", e.unknownMode)
	default:
		return "Noema development error"
	}
}

func main() {
	if len(os.Args) > 1 && os.Args[1] != "dev" && os.Args[1] != "serve" && os.Args[1] != "validate" {
		fmt.Fprintln(os.Stderr, "usage: noema-dev [dev|serve|validate]")
		os.Exit(2)
	}
	ctx, stop := signal.NotifyContext(context.Background(), shutdownSignals()...)
	defer stop()
	if err := runMode(ctx, os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func runMode(ctx context.Context, args []string) error {
	if len(args) == 0 || args[0] == "dev" {
		return run(ctx)
	}
	switch args[0] {
	case "serve":
		return runDevelopmentServer(ctx)
	case "validate":
		return runValidation(ctx, args[1:])
	default:
		return &devError{kind: devUnknownMode, unknownMode: args[0]}
	}
}

func run(ctx context.Context) error {
	if err := preparePlatform(); err != nil {
		return err
	}
	root, err := os.Getwd()
	if err != nil {
		return err
	}
	if _, err := os.Stat(filepath.Join(root, "go.mod")); err != nil {
		return errors.New("run noema-dev from the repository root")
	}
	commands := [][]string{
		{"go", "run", "github.com/air-verse/air@v1.67.4", "-c", ".air.toml"},
		{"bun", "run", "--cwd", "apps/web", "dev:assets"},
	}
	return superviseDevProcesses(ctx, root, commands)
}

func superviseDevProcesses(ctx context.Context, root string, commands [][]string) error {
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	var group sync.WaitGroup
	failures := make(chan error, len(commands))
	for _, args := range commands {
		args := append([]string(nil), args...)
		group.Go(func() {
			if err := watch(ctx, root, args); err != nil {
				failures <- err
				cancel()
			}
		})
	}
	group.Wait()
	close(failures)
	for err := range failures {
		return err
	}
	return nil
}

func watch(ctx context.Context, root string, args []string) error {
	for ctx.Err() == nil {
		child := exec.Command(args[0], args[1:]...)
		child.Dir = root
		child.Stdout, child.Stderr = os.Stdout, os.Stderr
		configureProcess(child)
		stripCargoRunEnv(child)
		if err := child.Start(); err != nil {
			return fmt.Errorf("start %s: %w", args[0], err)
		}
		exited := make(chan error, 1)
		go func() { exited <- waitForChild(args[0], child) }()
		select {
		case <-ctx.Done():
			stopProcess(child)
			<-exited
			return nil
		case err := <-exited:
			stopProcess(child)
			if ctx.Err() != nil {
				return nil
			}
			fmt.Fprintf(os.Stderr, "%s exited (%v); restarting\n", args[0], err)
		}
		select {
		case <-ctx.Done():
			return nil
		case <-time.After(watcherRestartDelay):
		}
	}
	return nil
}

func waitForChild(label string, child *exec.Cmd) error {
	processGroup := 0
	if child.Process != nil {
		processGroup = child.Process.Pid
	}
	status := child.Wait()
	return &devError{kind: devProcessExited, label: label, status: status, processGroup: &processGroup}
}

func watcherExit(err error) (string, *int) {
	var exited *devError
	if !errors.As(err, &exited) || exited.kind != devProcessExited {
		return "", nil
	}
	return exited.label, exited.processGroup
}

func configureWebServerWatcher(command *exec.Cmd, executable string) {
	command.Args = append(command.Args,
		"watch", "--delay", "1.5",
		"-E", "CARGO_PROFILE_DEV_INCREMENTAL=true",
		"-E", "CARGO_PROFILE_DEV_DEBUG=0",
		"-w", "crates",
		"-w", "Cargo.toml",
		"-w", "Cargo.lock",
	)
	for _, glob := range webServerWatchIgnoreGlobs {
		command.Args = append(command.Args, "--ignore", glob)
	}
	command.Args = append(command.Args, "--", executable, "serve")
}

func stripCargoRunEnv(command *exec.Cmd) {
	environment := command.Env
	if environment == nil {
		environment = os.Environ()
	}
	filtered := make([]string, 0, len(environment))
	for _, entry := range environment {
		key, _, _ := strings.Cut(entry, "=")
		if key == "CC" || key == "CXX" || key == "CARGO_MANIFEST_DIR" ||
			key == "CARGO_MANIFEST_PATH" || key == "CARGO_CRATE_NAME" ||
			key == "CARGO_BIN_NAME" || key == "CARGO_PRIMARY_PACKAGE" ||
			strings.HasPrefix(key, "CARGO_PKG_") {
			continue
		}
		filtered = append(filtered, entry)
	}
	command.Env = filtered
}
