//go:build !darwin && !linux && !windows

package provider

import (
	"context"
	"errors"
)

// Other platforms do not expose a stable physical-memory probe. Keep the
// provider available so account and capability discovery can report the
// absence of a selectable local build instead of failing to compile.
func detectLocalHardware(context.Context) ([]LocalHardwareProfile, error) {
	return nil, errors.New("physical memory is unavailable")
}
