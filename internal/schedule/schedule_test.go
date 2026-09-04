package schedule

import (
	"errors"
	"testing"
	"time"
)

func TestInvalidInput(t *testing.T) {
	t.Parallel()
	start := mustInstant(t, "2026-08-03T12:00:00Z")
	tests := []struct {
		name       string
		expression string
		zone       string
	}{
		{name: "four cron fields", expression: "0 8 * *", zone: "UTC"},
		{name: "six cron fields", expression: "0 0 8 * * *", zone: "UTC"},
		{name: "invalid cron value", expression: "61 8 * * *", zone: "UTC"},
		{name: "unknown time zone", expression: "0 8 * * *", zone: "Not/AZone"},
		{name: "process local time zone", expression: "0 8 * * *", zone: "Local"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			_, err := Preview(test.expression, test.zone, start)
			if !errors.Is(err, ErrInvalid) {
				t.Fatalf("Preview() error = %v, want ErrInvalid", err)
			}
		})
	}

	for _, policy := range []string{"", "run_all", "SKIP"} {
		if _, err := ParseMissedRunPolicy(policy); !errors.Is(err, ErrInvalid) {
			t.Fatalf("ParseMissedRunPolicy(%q) error = %v, want ErrInvalid", policy, err)
		}
	}
}

func TestNormalizeOneTimeRequiresFutureInstant(t *testing.T) {
	t.Parallel()
	now := mustInstant(t, "2026-08-03T12:00:00Z")
	valid := Schedule{
		ScheduledFor:    now.Add(time.Minute),
		TimeZone:        " America/New_York ",
		MissedRunPolicy: MissedRunOnce,
	}
	normalized, err := Normalize(valid, now)
	if err != nil {
		t.Fatalf("Normalize() error = %v", err)
	}
	if normalized.TimeZone != "America/New_York" {
		t.Fatalf("Normalize() time zone = %q", normalized.TimeZone)
	}

	for _, scheduledFor := range []time.Time{now, now.Add(-time.Second)} {
		invalid := valid
		invalid.ScheduledFor = scheduledFor
		if _, err := Normalize(invalid, now); !errors.Is(err, ErrInvalid) {
			t.Fatalf("Normalize(%v) error = %v, want ErrInvalid", scheduledFor, err)
		}
	}
}

func TestCronSequenceIsInclusiveAndUsesIANAZone(t *testing.T) {
	t.Parallel()
	start := mustInstant(t, "2026-08-03T12:00:00Z")
	values, err := Preview(" 0  8 * * * ", "America/New_York", start)
	if err != nil {
		t.Fatalf("Preview() error = %v", err)
	}
	want := []string{
		"2026-08-03T12:00:00Z",
		"2026-08-04T12:00:00Z",
		"2026-08-05T12:00:00Z",
		"2026-08-06T12:00:00Z",
		"2026-08-07T12:00:00Z",
	}
	assertInstants(t, values, want)
}

func TestNormalizeRecurrenceRequiresFirstCronMatch(t *testing.T) {
	t.Parallel()
	start := mustInstant(t, "2026-08-03T11:30:00Z")
	value := Schedule{
		ScheduledFor:    mustInstant(t, "2026-08-03T12:00:00Z"),
		TimeZone:        "America/New_York",
		MissedRunPolicy: MissedRunSkip,
		Recurrence: &Recurrence{
			StartsAt:       start,
			CronExpression: " 0  8 * * * ",
		},
	}
	normalized, err := Normalize(value, mustInstant(t, "2026-08-04T00:00:00Z"))
	if err != nil {
		t.Fatalf("Normalize() error = %v", err)
	}
	if normalized.Recurrence.CronExpression != "0 8 * * *" {
		t.Fatalf("Normalize() cron = %q", normalized.Recurrence.CronExpression)
	}

	value.ScheduledFor = value.ScheduledFor.Add(time.Minute)
	if _, err := Normalize(value, time.Time{}); !errors.Is(err, ErrInvalid) {
		t.Fatalf("Normalize() error = %v, want ErrInvalid", err)
	}
}

func TestDSTGapIsSkipped(t *testing.T) {
	t.Parallel()
	start := mustInstant(t, "2026-03-08T00:00:00-05:00")
	next, err := NextAtOrAfter("30 2 * * *", "America/New_York", start)
	if err != nil {
		t.Fatalf("NextAtOrAfter() error = %v", err)
	}
	want := mustInstant(t, "2026-03-09T02:30:00-04:00")
	if !next.Equal(want) {
		t.Fatalf("NextAtOrAfter() = %s, want %s", next, want)
	}
}

func TestDSTRepeatedMinuteHasOneLocalSlot(t *testing.T) {
	t.Parallel()
	first := mustInstant(t, "2026-11-01T01:30:00-04:00")
	repeated := mustInstant(t, "2026-11-01T01:30:00-05:00")
	firstSlot, err := LocalSlot(first, "America/New_York")
	if err != nil {
		t.Fatalf("LocalSlot(first) error = %v", err)
	}
	repeatedSlot, err := LocalSlot(repeated, "America/New_York")
	if err != nil {
		t.Fatalf("LocalSlot(repeated) error = %v", err)
	}
	if firstSlot != "2026-11-01T01:30" || repeatedSlot != firstSlot {
		t.Fatalf("LocalSlot() values = %q and %q", firstSlot, repeatedSlot)
	}

	next, err := NextAtOrAfter("30 1 * * *", "America/New_York", first.Add(time.Second))
	if err != nil {
		t.Fatalf("NextAtOrAfter() error = %v", err)
	}
	if !next.Equal(repeated) {
		t.Fatalf("NextAtOrAfter() = %s, want repeated instant %s", next, repeated)
	}
}

func mustInstant(t *testing.T, value string) time.Time {
	t.Helper()
	instant, err := ParseInstant(value)
	if err != nil {
		t.Fatalf("ParseInstant(%q) error = %v", value, err)
	}
	return instant
}

func assertInstants(t *testing.T, got []time.Time, want []string) {
	t.Helper()
	if len(got) != len(want) {
		t.Fatalf("len(instants) = %d, want %d", len(got), len(want))
	}
	for index, value := range want {
		expected := mustInstant(t, value)
		if !got[index].Equal(expected) {
			t.Errorf("instant[%d] = %s, want %s", index, got[index], expected)
		}
	}
}
