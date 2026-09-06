//go:build windows

package home

import (
	"errors"
	"os"
)

func replaceRootFile(root *os.Root, from, to string) error {
	if err := root.Rename(from, to); err != nil {
		return err
	}
	file, err := root.OpenFile(to, os.O_RDWR, 0)
	if err != nil {
		return err
	}
	return errors.Join(file.Sync(), file.Close())
}
