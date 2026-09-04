// Package schedule validates Task timing and resolves recurring UTC instants.
package schedule

import (
	"errors"
	"fmt"
	"strings"
	"time"
	_ "time/tzdata"

	"github.com/robfig/cron/v3"
)

const previewCount = 5

// ErrInvalid identifies invalid schedule input.
var ErrInvalid = errors.New("invalid schedule")

// MissedRunPolicy controls recovery when a scheduled instant has elapsed.
type MissedRunPolicy string

const (
	// MissedRunSkip records a missed instant without running it.
	MissedRunSkip MissedRunPolicy = "skip"
	// MissedRunOnce runs one representative occurrence after recovery.
	MissedRunOnce MissedRunPolicy = "run_once"
)

// Recurrence defines the continuing timing for a Task.
type Recurrence struct {
	StartsAt       time.Time
	CronExpression string
}

// Schedule defines the optional future timing attached to a Task.
type Schedule struct {
	ScheduledFor    time.Time
	TimeZone        string
	MissedRunPolicy MissedRunPolicy
	Recurrence      *Recurrence
}

// ParseInstant converts an RFC3339 instant to UTC second precision.
func ParseInstant(value string) (time.Time, error) {
	instant, err := time.Parse(time.RFC3339Nano, value)
	if err != nil {
		return time.Time{}, invalid("instant", "expected an RFC3339 instant")
	}
	return utcSecond(instant), nil
}

// ParseMissedRunPolicy validates a stored missed-run policy.
func ParseMissedRunPolicy(value string) (MissedRunPolicy, error) {
	policy := MissedRunPolicy(value)
	if policy != MissedRunSkip && policy != MissedRunOnce {
		return "", invalid("missed run policy", "expected skip or run_once")
	}
	return policy, nil
}

// Normalize validates a Task schedule and returns its stable stored form.
// One-time schedules must be later than now. Recurring schedules can begin in
// the past so that the missed-run policy can resolve elapsed occurrences.
func Normalize(value Schedule, now time.Time) (Schedule, error) {
	zone := strings.TrimSpace(value.TimeZone)
	if _, err := loadLocation(zone); err != nil {
		return Schedule{}, err
	}
	policy, err := ParseMissedRunPolicy(string(value.MissedRunPolicy))
	if err != nil {
		return Schedule{}, err
	}

	value.ScheduledFor = utcSecond(value.ScheduledFor)
	value.TimeZone = zone
	value.MissedRunPolicy = policy
	if value.Recurrence == nil {
		if !value.ScheduledFor.After(utcSecond(now)) {
			return Schedule{}, invalid("scheduled instant", "must be in the future")
		}
		return value, nil
	}

	recurrence := *value.Recurrence
	recurrence.StartsAt = utcSecond(recurrence.StartsAt)
	recurrence.CronExpression, err = normalizeCron(recurrence.CronExpression)
	if err != nil {
		return Schedule{}, err
	}
	first, err := NextAtOrAfter(recurrence.CronExpression, zone, recurrence.StartsAt)
	if err != nil {
		return Schedule{}, err
	}
	if !value.ScheduledFor.Equal(first) {
		return Schedule{}, invalid(
			"scheduled instant",
			"must equal the first cron match at or after the recurrence start",
		)
	}
	value.Recurrence = &recurrence
	return value, nil
}

// NextAtOrAfter returns the first cron match at or after start.
func NextAtOrAfter(expression, zone string, start time.Time) (time.Time, error) {
	parsed, err := parseCron(expression, zone)
	if err != nil {
		return time.Time{}, err
	}
	start = utcSecond(start)
	next := parsed.Next(start.Add(-time.Second))
	if next.IsZero() {
		return time.Time{}, invalid("cron expression", "has no future match")
	}
	return utcSecond(next), nil
}

// Preview returns five cron matches, including a matching start instant.
func Preview(expression, zone string, start time.Time) ([]time.Time, error) {
	parsed, err := parseCron(expression, zone)
	if err != nil {
		return nil, err
	}
	start = utcSecond(start)
	values := make([]time.Time, 0, previewCount)
	cursor := start.Add(-time.Second)
	for range previewCount {
		cursor = parsed.Next(cursor)
		if cursor.IsZero() {
			return nil, invalid("cron expression", "has no future match")
		}
		cursor = utcSecond(cursor)
		values = append(values, cursor)
	}
	return values, nil
}

// LocalSlot returns the stable wall-clock minute for recurrence deduplication.
func LocalSlot(instant time.Time, zone string) (string, error) {
	location, err := loadLocation(strings.TrimSpace(zone))
	if err != nil {
		return "", err
	}
	return utcSecond(instant).In(location).Format("2006-01-02T15:04"), nil
}

func parseCron(expression, zone string) (cron.Schedule, error) {
	expression, err := normalizeCron(expression)
	if err != nil {
		return nil, err
	}
	location, err := loadLocation(strings.TrimSpace(zone))
	if err != nil {
		return nil, err
	}
	parser := cron.NewParser(cron.Minute | cron.Hour | cron.Dom | cron.Month | cron.Dow)
	parsed, err := parser.Parse(expression)
	if err != nil {
		return nil, invalid("cron expression", err.Error())
	}
	parsed.(*cron.SpecSchedule).Location = location
	return parsed, nil
}

func normalizeCron(value string) (string, error) {
	fields := strings.Fields(value)
	if len(fields) != 5 {
		return "", invalid("cron expression", "expected five fields")
	}
	return strings.Join(fields, " "), nil
}

func loadLocation(zone string) (*time.Location, error) {
	if zone == "" || zone == "Local" {
		return nil, invalid("time zone", "expected an IANA time zone")
	}
	location, err := time.LoadLocation(zone)
	if err != nil {
		return nil, invalid("time zone", "expected an IANA time zone")
	}
	return location, nil
}

func utcSecond(value time.Time) time.Time {
	return time.Unix(value.Unix(), 0).UTC()
}

func invalid(field, reason string) error {
	return fmt.Errorf("%w: %s: %s", ErrInvalid, field, reason)
}
