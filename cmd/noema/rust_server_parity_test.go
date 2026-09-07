package main

import (
	"os"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/schedule"
)

// Rust source: crates/noema-server/src/bin/noema_web.rs::application_runtime_supports_cold_iana_timezone_parsing.
func TestRustServer_application_runtime_supports_cold_iana_timezone_parsing(t *testing.T) {
	const timestamp = int64(1_786_472_647)
	result := make(chan struct {
		next time.Time
		err  error
	}, 1)
	// The Go worker goroutine is the application-runtime equivalent of Rust's
	// spawn_blocking call for this synchronous timezone parser.
	go func() {
		next, err := schedule.NextAtOrAfter("0 7 * * *", "America/Los_Angeles", time.Unix(timestamp, 0).UTC())
		result <- struct {
			next time.Time
			err  error
		}{next: next, err: err}
	}()
	var value struct {
		next time.Time
		err  error
	}
	timer := time.NewTimer(time.Second)
	defer timer.Stop()
	select {
	case value = <-result:
	case <-timer.C:
		t.Fatal("timezone parser task did not complete")
	}
	next, err := value.next, value.err
	if err != nil {
		t.Fatal(err)
	}
	if next.Unix() < timestamp {
		t.Fatalf("next recurrence = %d, want at least %d", next.Unix(), timestamp)
	}
}

// Rust source: crates/noema-server/src/bin/noema_web.rs::unchanged_schema_is_not_rewritten.
func TestRustServer_unchanged_schema_is_not_rewritten(t *testing.T) {
	path := t.TempDir() + "/schema.graphql"
	if err := os.WriteFile(path, []byte("type Query"), 0o600); err != nil {
		t.Fatal(err)
	}
	changed, err := writeGraphQLSchemaIfChanged(path, []byte("type Query"))
	if err != nil {
		t.Fatal(err)
	}
	if changed {
		t.Fatal("unchanged schema was rewritten")
	}
	first, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	changed, err = writeGraphQLSchemaIfChanged(path, []byte("type Query"))
	if err != nil {
		t.Fatal(err)
	}
	if changed {
		t.Fatal("unchanged schema was rewritten on the second comparison")
	}
	second, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(first) != string(second) {
		t.Fatal("unchanged schema bytes changed")
	}
}

// Rust source: crates/noema-server/src/bin/noema_web.rs::changed_schema_is_rewritten.
func TestRustServer_changed_schema_is_rewritten(t *testing.T) {
	path := t.TempDir() + "/schema.graphql"
	if err := os.WriteFile(path, []byte("type Query"), 0o600); err != nil {
		t.Fatal(err)
	}
	changed, err := writeGraphQLSchemaIfChanged(path, []byte("type Mutation"))
	if err != nil {
		t.Fatal(err)
	}
	if !changed {
		t.Fatal("changed schema was not rewritten")
	}
	contents, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(contents) != "type Mutation" {
		t.Fatalf("rewritten schema = %q", contents)
	}
}
