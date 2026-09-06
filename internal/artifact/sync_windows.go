//go:build windows

package artifact

import (
	"errors"
	"fmt"
	"os"
)

func syncDirectory(directory *os.Root) error {
	info, err := directory.Stat(".")
	if err != nil {
		return fmt.Errorf("inspect Artifact publication directory: %w", err)
	}
	if !info.IsDir() {
		return errors.New("Artifact publication path is not a directory")
	}
	// Windows does not support FlushFileBuffers on directory handles.
	// Artifact publication syncs file data before rooted renames.
	return nil
}
