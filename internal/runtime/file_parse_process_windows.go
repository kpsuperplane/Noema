//go:build windows

package runtime

import (
	"fmt"
	"os/exec"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
)

type fileParseProcessTree struct{ job windows.Handle }

func configureFileParseProcess(command *exec.Cmd) {
	command.SysProcAttr = &syscall.SysProcAttr{
		CreationFlags: windows.CREATE_NEW_PROCESS_GROUP | windows.CREATE_SUSPENDED,
	}
}

func attachFileParseProcess(command *exec.Cmd) (fileParseProcessTree, error) {
	job, err := windows.CreateJobObject(nil, nil)
	if err != nil {
		return fileParseProcessTree{}, err
	}
	info := windows.JOBOBJECT_EXTENDED_LIMIT_INFORMATION{}
	info.BasicLimitInformation.LimitFlags = windows.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
	if _, err = windows.SetInformationJobObject(
		job,
		windows.JobObjectExtendedLimitInformation,
		uintptr(unsafe.Pointer(&info)),
		uint32(unsafe.Sizeof(info)),
	); err != nil {
		windows.CloseHandle(job)
		return fileParseProcessTree{}, err
	}
	process, err := windows.OpenProcess(
		windows.PROCESS_SET_QUOTA|windows.PROCESS_TERMINATE,
		false,
		uint32(command.Process.Pid),
	)
	if err != nil {
		windows.CloseHandle(job)
		return fileParseProcessTree{}, err
	}
	defer windows.CloseHandle(process)
	if err = windows.AssignProcessToJobObject(job, process); err != nil {
		windows.CloseHandle(job)
		return fileParseProcessTree{}, err
	}
	if err = resumeFileParseProcess(uint32(command.Process.Pid)); err != nil {
		windows.CloseHandle(job)
		return fileParseProcessTree{}, err
	}
	return fileParseProcessTree{job: job}, nil
}

func resumeFileParseProcess(pid uint32) error {
	snapshot, err := windows.CreateToolhelp32Snapshot(windows.TH32CS_SNAPTHREAD, 0)
	if err != nil {
		return fmt.Errorf("list suspended file parse threads: %w", err)
	}
	defer windows.CloseHandle(snapshot)
	entry := windows.ThreadEntry32{Size: uint32(unsafe.Sizeof(windows.ThreadEntry32{}))}
	if err = windows.Thread32First(snapshot, &entry); err != nil {
		return fmt.Errorf("read suspended file parse threads: %w", err)
	}
	for entry.OwnerProcessID != pid {
		if err = windows.Thread32Next(snapshot, &entry); err != nil {
			return fmt.Errorf("find suspended file parse thread: %w", err)
		}
	}
	thread, err := windows.OpenThread(windows.THREAD_SUSPEND_RESUME, false, entry.ThreadID)
	if err != nil {
		return fmt.Errorf("open suspended file parse thread: %w", err)
	}
	defer windows.CloseHandle(thread)
	if _, err = windows.ResumeThread(thread); err != nil {
		return fmt.Errorf("resume suspended file parse thread: %w", err)
	}
	return nil
}

func (tree fileParseProcessTree) terminate() error {
	return windows.TerminateJobObject(tree.job, 1)
}

func (tree fileParseProcessTree) close() {
	windows.CloseHandle(tree.job)
}
