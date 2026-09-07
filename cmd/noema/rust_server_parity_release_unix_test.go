//go:build unix

package main

import "testing"

// Rust source: crates/noema-server/src/bin/noema_web.rs::release_service_identity_rejects_root.
func TestRustServer_release_service_identity_rejects_root(t *testing.T) {
	if rejectReleaseRoot(false, 0) == nil {
		t.Fatal("release server accepted root")
	}
	if err := rejectReleaseRoot(true, 0); err != nil {
		t.Fatalf("desktop sidecar rejected root: %v", err)
	}
}
