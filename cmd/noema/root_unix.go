//go:build unix

package main

import (
	"errors"
)

func rejectReleaseRoot(desktop bool, uid int) error {
	if !desktop && uid == 0 {
		return errors.New("release Noema server must not run as root")
	}
	return nil
}
