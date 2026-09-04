//go:build windows

package home

import "golang.org/x/sys/windows"

func replacePrivateFile(from string, to string) error {
	fromName, err := windows.UTF16PtrFromString(from)
	if err != nil {
		return err
	}
	toName, err := windows.UTF16PtrFromString(to)
	if err != nil {
		return err
	}
	return windows.MoveFileEx(
		fromName,
		toName,
		windows.MOVEFILE_REPLACE_EXISTING|windows.MOVEFILE_WRITE_THROUGH,
	)
}
