package main

import (
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/schedule"
)

// Rust source: crates/noema-server/src/bin/noema_web.rs::application_runtime_supports_cold_iana_timezone_parsing.
func TestRustServer_application_runtime_supports_cold_iana_timezone_parsing(t *testing.T) {
	const timestamp = int64(1_786_472_647)
	next, err := schedule.NextAtOrAfter("0 7 * * *", "America/Los_Angeles", time.Unix(timestamp, 0).UTC())
	if err != nil {
		t.Fatal(err)
	}
	if next.Unix() < timestamp {
		t.Fatalf("next recurrence = %d, want at least %d", next.Unix(), timestamp)
	}
}

// Rust source: crates/noema-server/src/bin/noema_web.rs::unchanged_schema_is_not_rewritten.
func TestRustServer_unchanged_schema_is_not_rewritten(t *testing.T) {
	t.Fatalf("unsupported port: Go server has no generated-schema write-if-changed production path")
}

// Rust source: crates/noema-server/src/bin/noema_web.rs::changed_schema_is_rewritten.
func TestRustServer_changed_schema_is_rewritten(t *testing.T) {
	t.Fatalf("unsupported port: Go server has no generated-schema write-if-changed production path")
}
