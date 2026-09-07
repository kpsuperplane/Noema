package adapter

import (
	"errors"
	"strconv"
	"strings"
)

const maxRetryAfterSeconds uint64 = 24 * 60 * 60

var errRetryAfterInvalid = errors.New("adapter retry-after value is invalid")

// parseRetryAfter mirrors the Rust continuation parser. It accepts only a
// trimmed decimal number in the one-day range.
func parseRetryAfter(value string) (uint64, error) {
	seconds, err := strconv.ParseUint(strings.TrimSpace(value), 10, 64)
	if err != nil || seconds > maxRetryAfterSeconds {
		return 0, errRetryAfterInvalid
	}
	return seconds, nil
}
