//go:build windows

package webtool

import (
	"os"
	"os/exec"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
)

type browserProcessTree struct{ job windows.Handle }

func browserWorkerEnvironment() []string {
	result := []string{}
	for _, name := range []string{"SystemRoot", "TEMP", "TMP", "USERPROFILE"} {
		if value := os.Getenv(name); value != "" {
			result = append(result, name+"="+value)
		}
	}
	return result
}

func configureBrowserCommand(command *exec.Cmd) {
	command.SysProcAttr = &syscall.SysProcAttr{CreationFlags: windows.CREATE_NEW_PROCESS_GROUP}
}

func attachBrowserCommand(command *exec.Cmd) (browserProcessTree, error) {
	job, err := windows.CreateJobObject(nil, nil)
	if err != nil {
		return browserProcessTree{}, err
	}
	info := windows.JOBOBJECT_EXTENDED_LIMIT_INFORMATION{}
	info.BasicLimitInformation.LimitFlags = windows.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
	if _, err = windows.SetInformationJobObject(job, windows.JobObjectExtendedLimitInformation,
		uintptr(unsafe.Pointer(&info)), uint32(unsafe.Sizeof(info))); err != nil {
		windows.CloseHandle(job)
		return browserProcessTree{}, err
	}
	process, err := windows.OpenProcess(windows.PROCESS_SET_QUOTA|windows.PROCESS_TERMINATE, false, uint32(command.Process.Pid))
	if err != nil {
		windows.CloseHandle(job)
		return browserProcessTree{}, err
	}
	defer windows.CloseHandle(process)
	if err = windows.AssignProcessToJobObject(job, process); err != nil {
		windows.CloseHandle(job)
		return browserProcessTree{}, err
	}
	return browserProcessTree{job: job}, nil
}

func terminateBrowserCommand(tree browserProcessTree, command *exec.Cmd) {
	if tree.job != 0 {
		_ = windows.TerminateJobObject(tree.job, 1)
		return
	}
	if command.Process != nil {
		_ = command.Process.Kill()
	}
}

func (tree browserProcessTree) close() {
	if tree.job != 0 {
		windows.CloseHandle(tree.job)
	}
}
