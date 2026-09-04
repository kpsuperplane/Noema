//go:build windows

package home

import (
	"errors"
	"os"
)

func syncDirectory(root *os.Root, path string) error {
	info, err := root.Stat(path)
	if err != nil {
		return err
	}
	if !info.IsDir() {
		return errors.New("publication path is not a directory")
	}
	// Windows has no supported directory flush. Rooted replacement follows a file Sync.
	return nil
}
