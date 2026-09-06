//go:build (linux || darwin) && !race

package runtime

import (
	"runtime"

	"golang.org/x/sys/unix"
)

func limitFileParseWorkerMemory(bytes int) bool {
	limit := uint64(bytes)
	resource := unix.RLIMIT_AS
	if runtime.GOOS == "linux" {
		// Linux counts writable mappings without Go's unused address reservations.
		resource = unix.RLIMIT_DATA
	}
	return unix.Setrlimit(resource, &unix.Rlimit{Cur: limit, Max: limit}) == nil
}
