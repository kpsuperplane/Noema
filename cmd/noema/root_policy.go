package main

import "errors"

// rejectRoot is the platform-independent release identity policy.
//
// The Rust entrypoint tests this decision with a synthetic root flag. Keep
// that same boundary available on every Go target so the parity test does not
// disappear merely because the host has no Unix geteuid call.
func rejectRoot(isRoot bool) error {
	if isRoot {
		return errors.New("release Noema server must not run as root")
	}
	return nil
}
