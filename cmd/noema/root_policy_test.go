//go:build noema_release && unix

package main

import "testing"

func TestRejectReleaseRoot(t *testing.T) {
	if rejectReleaseRoot(false, 0) == nil {
		t.Fatal("release server accepted root")
	}
	if err := rejectReleaseRoot(true, 0); err != nil {
		t.Fatalf("desktop sidecar rejected root: %v", err)
	}
	if err := rejectReleaseRoot(false, 1000); err != nil {
		t.Fatalf("release server rejected unprivileged user: %v", err)
	}
}
