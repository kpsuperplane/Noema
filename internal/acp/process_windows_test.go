//go:build windows

package acp

import (
	"os/exec"
	"testing"

	"golang.org/x/sys/windows"
)

const stillActive = 259

func processAlive(pid int) bool {
	handle, err := windows.OpenProcess(windows.PROCESS_QUERY_LIMITED_INFORMATION, false, uint32(pid))
	if err != nil {
		return false
	}
	defer windows.CloseHandle(handle)
	var code uint32
	return windows.GetExitCodeProcess(handle, &code) == nil && code == stillActive
}

func TestConfigureProcessStartsSuspendedInNewGroup(t *testing.T) {
	cmd := exec.Command("cmd.exe")
	configureProcess(cmd)
	want := uint32(windows.CREATE_NEW_PROCESS_GROUP | windows.CREATE_SUSPENDED)
	var got uint32
	if cmd.SysProcAttr != nil {
		got = cmd.SysProcAttr.CreationFlags
	}
	if got&want != want {
		t.Fatalf("creation flags = %#x", got)
	}
}
