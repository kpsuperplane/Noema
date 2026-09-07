package main

import "testing"

// Rust source: crates/noema-server/src/bin/noema_web.rs::release_service_identity_rejects_root.
func TestRustServer_release_service_identity_rejects_root(t *testing.T) {
	if rejectRoot(true) == nil {
		t.Fatal("release server accepted root")
	}
	if err := rejectRoot(false); err != nil {
		t.Fatalf("unprivileged release server rejected: %v", err)
	}
}
