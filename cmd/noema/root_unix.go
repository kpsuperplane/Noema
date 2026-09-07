//go:build unix

package main

func rejectReleaseRoot(desktop bool, uid int) error {
	return rejectRoot(!desktop && uid == 0)
}
