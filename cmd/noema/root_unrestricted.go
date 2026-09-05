//go:build !noema_release || !unix

package main

func releaseRootError(bool) error {
	return nil
}
