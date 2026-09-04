//go:build !windows

package home

import "os"

func protectDirectory(path string) error {
	return os.Chmod(path, 0o700)
}

func protectPath(path string, _ bool) error {
	return os.Chmod(path, 0o600)
}
