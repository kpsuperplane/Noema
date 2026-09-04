//go:build windows

package home

import (
	"errors"
	"fmt"
	"os"
)

func syncAbsoluteDirectory(path string) error {
	info, err := os.Stat(path)
	if err != nil {
		return fmt.Errorf("inspect publication directory: %w", err)
	}
	if !info.IsDir() {
		return errors.New("publication path is not a directory")
	}
	// Windows does not support FlushFileBuffers on directory handles.
	// The caller syncs file data before atomic replacement.
	return nil
}
