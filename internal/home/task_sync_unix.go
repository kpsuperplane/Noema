//go:build !windows

package home

import (
	"errors"
	"os"
	"path/filepath"
)

func syncDirectory(root *os.Root, path string) error {
	rootedBefore, err := root.Stat(path)
	if err != nil {
		return err
	}
	absolute := filepath.Join(root.Name(), path)
	ambientBefore, err := os.Stat(absolute)
	if err != nil || !os.SameFile(rootedBefore, ambientBefore) {
		return errors.New("directory authority changed before sync")
	}
	if err := syncAbsoluteDirectory(absolute); err != nil {
		return err
	}
	rootedAfter, rootedErr := root.Stat(path)
	ambientAfter, ambientErr := os.Stat(absolute)
	if rootedErr != nil || ambientErr != nil || !os.SameFile(rootedAfter, ambientAfter) {
		return errors.New("directory authority changed during sync")
	}
	return nil
}
