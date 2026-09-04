//go:build !windows

package mcp

import (
	"os/exec"
	"syscall"
)

type processTree struct{ group int }

func configureProcess(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

func attachProcess(cmd *exec.Cmd) (processTree, error) {
	return processTree{group: cmd.Process.Pid}, nil
}

func (p processTree) terminate(force bool) error {
	signal := syscall.SIGTERM
	if force {
		signal = syscall.SIGKILL
	}
	return syscall.Kill(-p.group, signal)
}

func (p processTree) close() { _ = syscall.Kill(-p.group, syscall.SIGKILL) }
