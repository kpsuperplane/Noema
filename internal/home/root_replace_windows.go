//go:build windows

package home

import (
	"os"
	"path/filepath"
)

func replaceRootFile(root *os.Root, from, to string) error {
	return replacePrivateFile(
		filepath.Join(root.Name(), filepath.FromSlash(from)),
		filepath.Join(root.Name(), filepath.FromSlash(to)),
	)
}
