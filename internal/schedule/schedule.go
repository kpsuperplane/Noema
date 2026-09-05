// Package schedule validates Task timing and resolves recurring UTC instants.
package schedule

import (
	"errors"
	"fmt"
	"sort"
	"strings"
	"time"
	_ "time/tzdata"

	"github.com/adhocore/gronx"
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

// OverlapPolicy controls a due occurrence while another remains active.
type OverlapPolicy string

const (
	// OverlapSkip records the due instant without another Task.
	OverlapSkip OverlapPolicy = "skip"
	// OverlapQueueOne keeps one coalesced instant until the active Task ends.
	OverlapQueueOne OverlapPolicy = "queue_one"
	// OverlapAllow materializes every due occurrence.
	OverlapAllow OverlapPolicy = "allow"
)

// Recurrence defines the continuing timing for a Task.
type Recurrence struct {
	StartsAt       time.Time
	CronExpression string
	OverlapPolicy  OverlapPolicy
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

// ParseOverlapPolicy validates a stored overlap policy.
func ParseOverlapPolicy(value string) (OverlapPolicy, error) {
	policy := OverlapPolicy(value)
	if policy != OverlapSkip && policy != OverlapQueueOne && policy != OverlapAllow {
		return "", invalid("overlap policy", "expected skip, queue_one, or allow")
	}
	return policy, nil
}

// Normalize validates a Task schedule and returns its stable stored form.
// Elapsed schedules remain valid so that missed-run policy can resolve them.
func Normalize(value Schedule, _ time.Time) (Schedule, error) {
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
		return value, nil
	}

	recurrence := *value.Recurrence
	recurrence.StartsAt = utcSecond(recurrence.StartsAt)
	recurrence.CronExpression, err = normalizeCron(recurrence.CronExpression)
	if err != nil {
		return Schedule{}, err
	}
	if recurrence.OverlapPolicy == "" {
		recurrence.OverlapPolicy = OverlapSkip
	} else if recurrence.OverlapPolicy, err = ParseOverlapPolicy(string(recurrence.OverlapPolicy)); err != nil {
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
	next := parsed.NextAtOrAfter(start)
	if next.IsZero() {
		return time.Time{}, invalid("cron expression", "has no future match")
	}
	return utcSecond(next), nil
}

// Preview returns five distinct local-minute matches, including a matching start.
func Preview(expression, zone string, start time.Time) ([]time.Time, error) {
	parsed, err := parseCron(expression, zone)
	if err != nil {
		return nil, err
	}
	start = utcSecond(start)
	values := make([]time.Time, 0, previewCount)
	seen := make(map[string]bool, previewCount)
	cursor := start
	for len(values) < previewCount {
		cursor = parsed.NextAtOrAfter(cursor)
		if cursor.IsZero() {
			return nil, invalid("cron expression", "has no future match")
		}
		cursor = utcSecond(cursor)
		slot, _ := LocalSlot(cursor, zone) // parseCron already validated the zone.
		if !seen[slot] {
			values = append(values, cursor)
			seen[slot] = true
		}
		cursor = cursor.Add(time.Second)
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

type parsedCron struct {
	expression string
	location   *time.Location
}

func (value parsedCron) NextAtOrAfter(start time.Time) time.Time {
	start = utcSecond(start)
	local := start.In(value.location)
	wallCursor := time.Date(
		local.Year(), local.Month(), local.Day(), local.Hour(), local.Minute(), 0, 0, time.UTC,
	).Add(-time.Second)
	for {
		wallMatch, err := gronx.NextTickAfter(value.expression, wallCursor, false)
		if err != nil || wallMatch.IsZero() {
			return time.Time{}
		}
		for _, instant := range resolveWallTime(wallMatch, value.location) {
			if !instant.Before(start) {
				return instant
			}
		}
		wallCursor = wallMatch
	}
}

// resolveWallTime returns each real instant for one local wall-clock value.
// A DST gap has no result. A repeated minute has two ordered results.
func resolveWallTime(wall time.Time, location *time.Location) []time.Time {
	normalized := time.Date(
		wall.Year(), wall.Month(), wall.Day(), wall.Hour(), wall.Minute(), wall.Second(), 0, location,
	)
	offsets := make(map[int]struct{})
	for hours := -72; hours <= 72; hours += 6 {
		_, offset := normalized.Add(time.Duration(hours) * time.Hour).Zone()
		offsets[offset] = struct{}{}
	}

	instants := make([]time.Time, 0, 2)
	seen := make(map[int64]struct{})
	for offset := range offsets {
		instant := time.Unix(wall.Unix()-int64(offset), 0).UTC()
		local := instant.In(location)
		if local.Year() != wall.Year() || local.Month() != wall.Month() ||
			local.Day() != wall.Day() || local.Hour() != wall.Hour() ||
			local.Minute() != wall.Minute() || local.Second() != wall.Second() {
			continue
		}
		if _, exists := seen[instant.Unix()]; !exists {
			seen[instant.Unix()] = struct{}{}
			instants = append(instants, instant)
		}
	}
	sort.Slice(instants, func(left, right int) bool {
		return instants[left].Before(instants[right])
	})
	return instants
}

func parseCron(expression, zone string) (parsedCron, error) {
	expression, err := normalizeCron(expression)
	if err != nil {
		return parsedCron{}, err
	}
	location, err := loadLocation(strings.TrimSpace(zone))
	if err != nil {
		return parsedCron{}, err
	}
	_, err = gronx.New().IsDue(expression, time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC))
	if err != nil {
		return parsedCron{}, invalid("cron expression", err.Error())
	}
	return parsedCron{expression: expression, location: location}, nil
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
