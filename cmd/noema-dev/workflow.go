package main

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

const (
	gib                = uint64(1024 * 1024 * 1024)
	cacheCheckInterval = 6 * time.Hour
	cacheCheckSentinel = ".noema-cache-budget-checked"
	cacheTempDirectory = "tmp"
	cargoCacheTag      = "Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n# For information about cache directory tags see https://bford.info/cachedir/\n"
)

var (
	developmentCache = cacheTarget{label: "development server", directory: "noema-dev", maxBytes: 30 * gib}
	validationCache  = cacheTarget{label: "Rust validation", directory: "noema-validation", maxBytes: 20 * gib}
)

type cacheTarget struct {
	label, directory string
	maxBytes         uint64
}

func (target cacheTarget) path(repoRoot string) string {
	return filepath.Join(repoRoot, "target", target.directory)
}

func (target cacheTarget) tempPath(repoRoot string) string {
	return filepath.Join(target.path(repoRoot), cacheTempDirectory)
}

func cargoExe() string {
	if value := os.Getenv("CARGO"); value != "" {
		return value
	}
	return "cargo"
}

func runValidation(ctx context.Context, args []string) error {
	if len(args) == 0 {
		return errors.New("validation requires a Cargo command")
	}
	repoRoot, err := os.Getwd()
	if err != nil {
		return err
	}
	if err := enforceCacheBudget(ctx, repoRoot, validationCache); err != nil {
		return err
	}
	command := exec.CommandContext(ctx, cargoExe(), args...)
	command.Dir = repoRoot
	setCommandEnv(command, "CARGO_INCREMENTAL", "0")
	setCommandEnv(command, "CARGO_PROFILE_DEV_DEBUG", "0")
	setCommandEnv(command, "CARGO_PROFILE_TEST_DEBUG", "1")
	setCommandEnv(command, "CARGO_TARGET_DIR", validationCache.path(repoRoot))
	setCommandEnv(command, "TMPDIR", validationCache.tempPath(repoRoot))
	command.Stdin, command.Stdout, command.Stderr = os.Stdin, os.Stdout, os.Stderr
	stripCargoRunEnv(command)
	if err := command.Run(); err != nil {
		return fmt.Errorf("Cargo exited with status: %w", err)
	}
	return nil
}

func runDevelopmentServer(ctx context.Context) error {
	repoRoot, err := os.Getwd()
	if err != nil {
		return err
	}
	if err := enforceCacheBudget(ctx, repoRoot, developmentCache); err != nil {
		return err
	}
	prepared := developmentServerCommand(repoRoot, runningAsRoot())
	command := exec.CommandContext(ctx, prepared.Args[0], prepared.Args[1:]...)
	command.Dir = prepared.Dir
	command.Env = append([]string(nil), prepared.Env...)
	command.Stdin, command.Stdout, command.Stderr = prepared.Stdin, prepared.Stdout, prepared.Stderr
	if err := command.Run(); err != nil {
		return fmt.Errorf("Cargo exited with status: %w", err)
	}
	return nil
}

// commandEnv returns the prepared environment without changing the command.
// It keeps runDevelopmentServer from losing the bounded cache and socket settings
// when it attaches a context to the command.
func commandEnv(command *exec.Cmd) []string {
	if command.Env != nil {
		return command.Env
	}
	return os.Environ()
}

func setCommandEnv(command *exec.Cmd, key, value string) {
	environment := commandEnv(command)
	filtered := make([]string, 0, len(environment)+1)
	for _, entry := range environment {
		name, _, _ := strings.Cut(entry, "=")
		if name != key {
			filtered = append(filtered, entry)
		}
	}
	command.Env = append(filtered, key+"="+value)
}

func developmentServerCommand(repoRoot string, dropRoot bool) *exec.Cmd {
	var command *exec.Cmd
	if dropRoot {
		command = exec.Command(filepath.Join(repoRoot, "scripts", "run-noema-dev-server"))
	} else {
		command = exec.Command(cargoExe(), "run", "-p", "noema-server", "--bin", "noema_web")
	}
	command.Dir = repoRoot
	setCommandEnv(command, "CARGO_TARGET_DIR", developmentCache.path(repoRoot))
	setCommandEnv(command, "TMPDIR", developmentCache.tempPath(repoRoot))
	setCommandEnv(command, "NOEMA_WEB__HOST", "127.0.0.1")
	setCommandEnv(command, "NOEMA_WEB__LOCAL_GRAPHQL_SOCKET", "true")
	command.Stdin, command.Stdout, command.Stderr = os.Stdin, os.Stdout, os.Stderr
	stripCargoRunEnv(command)
	return command
}

func enforceCacheBudget(ctx context.Context, repoRoot string, target cacheTarget) error {
	targetPath := target.path(repoRoot)
	if !cacheTargetIsSafe(repoRoot, targetPath) {
		return fmt.Errorf("refusing to clean unsafe Cargo target: %s", targetPath)
	}
	if err := prepareCacheTarget(targetPath); err != nil {
		return fmt.Errorf("failed to prepare %s Cargo cache: %w", target.label, err)
	}
	if cacheCheckIsCurrent(targetPath) {
		return nil
	}
	sizeBytes, err := directorySizeBytes(targetPath)
	if err != nil {
		return fmt.Errorf("failed to inspect %s Cargo cache: %w", target.label, err)
	}
	if shouldCleanCache(sizeBytes, target.maxBytes) {
		if err := cleanCache(ctx, repoRoot, targetPath); err != nil {
			return err
		}
		if err := prepareCacheTarget(targetPath); err != nil {
			return fmt.Errorf("failed to prepare %s Cargo cache: %w", target.label, err)
		}
	}
	if err := os.WriteFile(filepath.Join(targetPath, cacheCheckSentinel), nil, 0o600); err != nil {
		return fmt.Errorf("failed to prepare %s Cargo cache: %w", target.label, err)
	}
	return nil
}

func prepareCacheTarget(targetPath string) error {
	if err := os.MkdirAll(filepath.Join(targetPath, cacheTempDirectory), 0o700); err != nil {
		return err
	}
	tagPath := filepath.Join(targetPath, "CACHEDIR.TAG")
	contents, err := os.ReadFile(tagPath)
	if err != nil || string(contents) != cargoCacheTag {
		return os.WriteFile(tagPath, []byte(cargoCacheTag), 0o600)
	}
	return nil
}

func cacheCheckIsCurrent(targetPath string) bool {
	info, err := os.Stat(filepath.Join(targetPath, cacheCheckSentinel))
	return err == nil && time.Since(info.ModTime()) >= 0 && time.Since(info.ModTime()) < cacheCheckInterval
}

func directorySizeBytes(targetPath string) (uint64, error) {
	var total uint64
	err := filepath.WalkDir(targetPath, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			if errors.Is(err, fs.ErrNotExist) {
				return nil
			}
			return err
		}
		if entry.IsDir() {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		total += uint64(info.Size())
		return nil
	})
	return total, err
}

func cleanCache(ctx context.Context, repoRoot, targetPath string) error {
	command := exec.CommandContext(ctx, cargoExe(), "clean", "--target-dir", targetPath)
	command.Dir = repoRoot
	command.Stdin, command.Stdout, command.Stderr = nil, os.Stdout, os.Stderr
	if err := command.Run(); err != nil {
		return fmt.Errorf("Cargo exited with status: %w", err)
	}
	return nil
}

func cacheTargetIsSafe(repoRoot, targetPath string) bool {
	relative, err := filepath.Rel(repoRoot, targetPath)
	if err != nil || relative == "." || filepath.IsAbs(relative) {
		return false
	}
	parts := strings.Split(filepath.ToSlash(relative), "/")
	return len(parts) == 2 && parts[0] == "target" && (parts[1] == "noema-dev" || parts[1] == "noema-validation")
}

func shouldCleanCache(sizeBytes, maxBytes uint64) bool {
	return sizeBytes > maxBytes
}
