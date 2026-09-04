package home

import "os"

// SyncRootDirectory durably records changes made through one rooted directory.
func SyncRootDirectory(root *os.Root, path string) error {
	return syncDirectory(root, path)
}
