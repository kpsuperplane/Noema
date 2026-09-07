//go:build !unix

package main

func releaseRootError(bool) error {
	return nil
}
