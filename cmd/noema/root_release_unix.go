//go:build noema_release && unix

package main

import "os"

func releaseRootError(desktop bool) error {
	return rejectReleaseRoot(desktop, os.Geteuid())
}
