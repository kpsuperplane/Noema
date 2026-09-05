//go:build !windows

package provider

import (
	"os/exec"
	"syscall"
)

type foundationProcessTree struct{ group int }

func configureFoundationProcess(command *exec.Cmd) {
	command.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

func newFoundationProcessTree(command *exec.Cmd) (foundationProcessTree, error) {
	return foundationProcessTree{group: command.Process.Pid}, nil
}

func (p foundationProcessTree) terminate() error { return syscall.Kill(-p.group, syscall.SIGKILL) }
func (p foundationProcessTree) close()           { _ = syscall.Kill(-p.group, syscall.SIGKILL) }
