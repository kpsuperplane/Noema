//go:build linux || darwin

package localmodel

import (
	"errors"

	"golang.org/x/sys/unix"
)

func ensureFreeSpace(path string, needed int64) error {
	if needed <= 0 {
		return nil
	}
	var status unix.Statfs_t
	if err := unix.Statfs(path, &status); err != nil {
		return err
	}
	if uint64(needed) > status.Bavail*uint64(status.Bsize) {
		return errors.New("not enough free space for the local model")
	}
	return nil
}
