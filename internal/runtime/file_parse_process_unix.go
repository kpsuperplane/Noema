//go:build !windows

package runtime

import (
	"os/exec"
	"syscall"
)

type fileParseProcessTree struct{ group int }

func configureFileParseProcess(command *exec.Cmd) {
	command.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

func attachFileParseProcess(command *exec.Cmd) (fileParseProcessTree, error) {
	return fileParseProcessTree{group: command.Process.Pid}, nil
}

func (tree fileParseProcessTree) terminate() error {
	return syscall.Kill(-tree.group, syscall.SIGKILL)
}

func (tree fileParseProcessTree) close() {
	_ = syscall.Kill(-tree.group, syscall.SIGKILL)
}
