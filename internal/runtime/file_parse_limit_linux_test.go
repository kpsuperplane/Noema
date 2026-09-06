//go:build linux && !race

package runtime

import (
	"os"
	"os/exec"
	"testing"

	"golang.org/x/sys/unix"
)

func TestFileParseWorkerMemory(t *testing.T) {
	if os.Getenv("NOEMA_TEST_MEMORY_LIMIT") == "1" {
		if !limitFileParseWorkerMemory(fileParseWorkerMemory) {
			t.Fatal("set worker memory limit")
		}
		for _, size := range []int{1 << 20, fileParseWorkerMemory * 2} {
			data, err := unix.Mmap(-1, 0, size, unix.PROT_READ|unix.PROT_WRITE, unix.MAP_PRIVATE|unix.MAP_ANON)
			if err == nil {
				_ = unix.Munmap(data)
			}
			if size < fileParseWorkerMemory && err != nil || size > fileParseWorkerMemory && err != unix.ENOMEM {
				t.Fatalf("allocation of %d bytes: %v", size, err)
			}
		}
		return
	}
	command := exec.Command(os.Args[0], "-test.run=^TestFileParseWorkerMemory$")
	command.Env = []string{"NOEMA_TEST_MEMORY_LIMIT=1"}
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("worker memory limit: %v\n%s", err, output)
	}
}
