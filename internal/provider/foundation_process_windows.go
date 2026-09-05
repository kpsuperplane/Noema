//go:build windows

package provider

import (
	"os/exec"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
)

type foundationProcessTree struct{ job windows.Handle }

func configureFoundationProcess(command *exec.Cmd) {
	command.SysProcAttr = &syscall.SysProcAttr{CreationFlags: windows.CREATE_NEW_PROCESS_GROUP}
}

func newFoundationProcessTree(command *exec.Cmd) (foundationProcessTree, error) {
	job, err := windows.CreateJobObject(nil, nil)
	if err != nil {
		return foundationProcessTree{}, err
	}
	info := windows.JOBOBJECT_EXTENDED_LIMIT_INFORMATION{}
	info.BasicLimitInformation.LimitFlags = windows.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
	if _, err = windows.SetInformationJobObject(job, windows.JobObjectExtendedLimitInformation,
		uintptr(unsafe.Pointer(&info)), uint32(unsafe.Sizeof(info))); err != nil {
		windows.CloseHandle(job)
		return foundationProcessTree{}, err
	}
	process, err := windows.OpenProcess(windows.PROCESS_SET_QUOTA|windows.PROCESS_TERMINATE, false, uint32(command.Process.Pid))
	if err != nil {
		windows.CloseHandle(job)
		return foundationProcessTree{}, err
	}
	defer windows.CloseHandle(process)
	if err = windows.AssignProcessToJobObject(job, process); err != nil {
		windows.CloseHandle(job)
		return foundationProcessTree{}, err
	}
	return foundationProcessTree{job: job}, nil
}

func (p foundationProcessTree) terminate() error { return windows.TerminateJobObject(p.job, 1) }
func (p foundationProcessTree) close()           { windows.CloseHandle(p.job) }
