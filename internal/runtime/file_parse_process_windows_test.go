//go:build windows

package runtime

import (
	"testing"

	"golang.org/x/sys/windows"
)

func TestFileParseJobLimitsMemory(t *testing.T) {
	limits := fileParseJobLimits()
	wantFlags := uint32(windows.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | windows.JOB_OBJECT_LIMIT_JOB_MEMORY)
	if limits.BasicLimitInformation.LimitFlags&wantFlags != wantFlags {
		t.Fatalf("file parse Job Object flags = %#x", limits.BasicLimitInformation.LimitFlags)
	}
	if limits.JobMemoryLimit != uintptr(fileParseWorkerMemory) {
		t.Fatalf("file parse Job Object memory = %d", limits.JobMemoryLimit)
	}
}
