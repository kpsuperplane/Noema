//go:build !windows

package home

import "os"

func replacePrivateFile(from string, to string) error {
	return os.Rename(from, to)
}
