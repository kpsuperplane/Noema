package schedule

import "testing"

// Rust source: crates/noema-tasks/src/schedule.rs:255::recurrence_start_is_inclusive_and_preview_uses_iana_zone
func TestRustTasks_recurrence_start_is_inclusive_and_preview_uses_iana_zone(t *testing.T) {
	start, err := ParseInstant("2026-08-03T12:00:00Z")
	if err != nil {
		t.Fatal(err)
	}
	preview, err := Preview("0 8 * * *", "America/New_York", start)
	if err != nil {
		t.Fatal(err)
	}
	if len(preview) == 0 || !preview[0].Equal(start) {
		t.Fatalf("first recurrence = %v, want %s", preview, start)
	}
	if len(preview) != 5 {
		t.Fatalf("preview length = %d, want 5", len(preview))
	}
}

// Rust source: crates/noema-tasks/src/schedule.rs:263::invalid_cron_and_timezone_fail_closed
func TestRustTasks_invalid_cron_and_timezone_fail_closed(t *testing.T) {
	start, err := ParseInstant("1970-01-01T00:00:00Z")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Preview("0 8 * *", "UTC", start); err == nil {
		t.Fatal("invalid cron was accepted")
	}
	if _, err := Preview("0 8 * * *", "Not/AZone", start); err == nil {
		t.Fatal("invalid time zone was accepted")
	}
}

// Rust source: crates/noema-tasks/src/schedule.rs:269::daylight_saving_gap_is_skipped_and_repeated_minute_has_one_slot_identity
func TestRustTasks_daylight_saving_gap_is_skipped_and_repeated_minute_has_one_slot_identity(t *testing.T) {
	spring, err := ParseInstant("2026-03-08T00:00:00-05:00")
	if err != nil {
		t.Fatal(err)
	}
	next, err := NextAtOrAfter("30 2 * * *", "America/New_York", spring)
	if err != nil {
		t.Fatal(err)
	}
	expected, err := ParseInstant("2026-03-09T02:30:00-04:00")
	if err != nil {
		t.Fatal(err)
	}
	if !next.Equal(expected) {
		t.Fatalf("next recurrence = %s, want %s", next, expected)
	}

	first, err := ParseInstant("2026-11-01T01:30:00-04:00")
	if err != nil {
		t.Fatal(err)
	}
	repeated, err := ParseInstant("2026-11-01T01:30:00-05:00")
	if err != nil {
		t.Fatal(err)
	}
	firstSlot, err := LocalSlot(first, "America/New_York")
	if err != nil {
		t.Fatal(err)
	}
	repeatedSlot, err := LocalSlot(repeated, "America/New_York")
	if err != nil {
		t.Fatal(err)
	}
	if firstSlot != repeatedSlot {
		t.Fatalf("repeated local slots differ: %q and %q", firstSlot, repeatedSlot)
	}
}
