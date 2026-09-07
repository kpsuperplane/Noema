//go:build !(aix || android || darwin || dragonfly || freebsd || hurd || illumos || ios || linux || netbsd || openbsd || solaris)

package adapter

import "testing"

func assertSymlinkedDefinition(t *testing.T, authority *fileAuthority, manifestPath, digest string) {
	t.Helper()
}
