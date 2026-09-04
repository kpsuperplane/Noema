//go:build !windows

package home

import (
	"fmt"
	"os"
)

func syncDirectory(root *os.Root, path string) error {
	directory, err := root.Open(path)
	if err != nil {
		return fmt.Errorf("open Task directory for sync: %w", err)
	}
	defer directory.Close()
	if err := directory.Sync(); err != nil {
		return fmt.Errorf("sync Task directory: %w", err)
	}
	return nil
}
