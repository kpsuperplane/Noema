//go:build darwin

package localmodel

import (
	"context"
	"errors"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
	"time"
)

func checkRuntimePlatform() error {
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, "/usr/bin/sw_vers", "-productVersion")
	command.Env = []string{"LANG=C", "LC_ALL=C"}
	output, err := command.Output()
	if err != nil {
		return errors.New("macOS version is unavailable")
	}
	parts := strings.Split(strings.TrimSpace(string(output)), ".")
	major, majorErr := strconv.Atoi(parts[0])
	minor := 0
	if len(parts) > 1 {
		minor, _ = strconv.Atoi(parts[1])
	}
	if majorErr != nil {
		return errors.New("macOS version is invalid")
	}
	if runtime.GOARCH == "arm64" && major < 26 {
		return errors.New("the pinned llama.cpp arm64 runtime requires macOS 26.0 or later")
	}
	if runtime.GOARCH == "amd64" && (major < 13 || major == 13 && minor < 3) {
		return errors.New("the pinned llama.cpp x64 runtime requires macOS 13.3 or later")
	}
	return nil
}
