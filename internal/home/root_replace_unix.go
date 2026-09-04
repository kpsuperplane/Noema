//go:build !windows

package home

import "os"

func replaceRootFile(root *os.Root, from, to string) error {
	return root.Rename(from, to)
}
