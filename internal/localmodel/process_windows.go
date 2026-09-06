//go:build windows

package localmodel

import "os/exec"

func configureProcess(*exec.Cmd) {}

func stopProcess(command *exec.Cmd) {
	if command.Process != nil {
		_ = command.Process.Kill()
	}
}
