//go:build windows

package home

import (
	"fmt"
	"os"
	"path/filepath"

	"golang.org/x/sys/windows"
)

func syncDirectory(root *os.Root, path string) error {
	name, err := windows.UTF16PtrFromString(filepath.Join(root.Name(), path))
	if err != nil {
		return fmt.Errorf("encode Task directory for sync: %w", err)
	}
	handle, err := windows.CreateFile(
		name,
		windows.GENERIC_WRITE,
		windows.FILE_SHARE_READ|windows.FILE_SHARE_WRITE|windows.FILE_SHARE_DELETE,
		nil,
		windows.OPEN_EXISTING,
		windows.FILE_FLAG_BACKUP_SEMANTICS,
		0,
	)
	if err != nil {
		return fmt.Errorf("open Task directory for sync: %w", err)
	}
	defer windows.CloseHandle(handle)
	if err := windows.FlushFileBuffers(handle); err != nil {
		return fmt.Errorf("sync Task directory: %w", err)
	}
	return nil
}
