//go:build noema_release && unix

package main

import (
	"errors"
	"os"
)

func releaseRootError(desktop bool) error {
	return rejectReleaseRoot(desktop, os.Geteuid())
}

func rejectReleaseRoot(desktop bool, uid int) error {
	if !desktop && uid == 0 {
		return errors.New("release Noema server must not run as root")
	}
	return nil
}
