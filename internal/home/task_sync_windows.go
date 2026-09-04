//go:build windows

package home

import (
	"os"
	"path/filepath"
)

func syncDirectory(root *os.Root, path string) error {
	return syncAbsoluteDirectory(filepath.Join(root.Name(), path))
}
