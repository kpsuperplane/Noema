//go:build windows

package artifact

import (
	"fmt"
	"os"

	"golang.org/x/sys/windows"
)

func syncDirectory(directory *os.Root) error {
	name, err := windows.UTF16PtrFromString(directory.Name())
	if err != nil {
		return fmt.Errorf("encode Artifact directory for sync: %w", err)
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
		return fmt.Errorf("open Artifact directory for sync: %w", err)
	}
	defer windows.CloseHandle(handle)
	if err := windows.FlushFileBuffers(handle); err != nil {
		return fmt.Errorf("sync Artifact directory: %w", err)
	}
	return nil
}
