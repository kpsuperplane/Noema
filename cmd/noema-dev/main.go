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
	"sync"
	"time"
)

func main() {
	if len(os.Args) > 2 || len(os.Args) == 2 && os.Args[1] != "dev" {
		fmt.Fprintln(os.Stderr, "usage: noema-dev [dev]")
		os.Exit(2)
	}
	ctx, stop := signal.NotifyContext(context.Background(), shutdownSignals()...)
	defer stop()
	if err := run(ctx); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
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
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	commands := [][]string{
		{"go", "run", "github.com/air-verse/air@v1.67.4", "-c", ".air.toml"},
		{"bun", "run", "--cwd", "apps/web", "dev:assets"},
	}
	var group sync.WaitGroup
	failures := make(chan error, len(commands))
	for _, args := range commands {
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
		if err := child.Start(); err != nil {
			return fmt.Errorf("start %s: %w", args[0], err)
		}
		exited := make(chan error, 1)
		go func() { exited <- child.Wait() }()
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
		case <-time.After(250 * time.Millisecond):
		}
	}
	return nil
}
