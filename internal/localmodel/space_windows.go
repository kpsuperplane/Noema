package localmodel

import (
	"errors"

	"golang.org/x/sys/windows"
)

func ensureFreeSpace(path string, needed int64) error {
	if needed <= 0 {
		return nil
	}
	directory, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return err
	}
	var available uint64
	if err = windows.GetDiskFreeSpaceEx(directory, &available, nil, nil); err != nil {
		return err
	}
	if uint64(needed) > available {
		return errors.New("not enough free space for the local model")
	}
	return nil
}
