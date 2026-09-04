package home

import (
	"errors"
	"io/fs"
	"os"
	"strings"
)

// SyncRootDirectory durably records changes made through one rooted directory.
func SyncRootDirectory(root *os.Root, path string) error {
	return syncDirectory(root, path)
}

// ReplaceRootFile atomically replaces one file below a rooted directory.
func ReplaceRootFile(root *os.Root, from, to string) error {
	if root == nil || !fs.ValidPath(from) || !fs.ValidPath(to) ||
		strings.Contains(from, `\`) || strings.Contains(to, `\`) {
		return errors.New("rooted replacement path is invalid")
	}
	return replaceRootFile(root, from, to)
}
