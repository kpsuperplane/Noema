//go:build !windows

package webtool

import (
	"os/exec"
	"syscall"
)

type browserProcessTree struct{}

func browserWorkerEnvironment() []string { return []string{"LANG=C", "LC_ALL=C"} }

func configureBrowserCommand(command *exec.Cmd) {
	command.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

func attachBrowserCommand(command *exec.Cmd) (browserProcessTree, error) {
	return browserProcessTree{}, nil
}

func terminateBrowserCommand(_ browserProcessTree, command *exec.Cmd) {
	if command.Process != nil {
		_ = syscall.Kill(-command.Process.Pid, syscall.SIGKILL)
	}
}

func (browserProcessTree) close() {}
