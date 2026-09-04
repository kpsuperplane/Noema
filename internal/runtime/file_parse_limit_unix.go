//go:build (linux || darwin) && !race

package runtime

import "golang.org/x/sys/unix"

func limitFileParseWorkerMemory(bytes int) bool {
	limit := uint64(bytes)
	return unix.Setrlimit(unix.RLIMIT_AS, &unix.Rlimit{Cur: limit, Max: limit}) == nil
}
