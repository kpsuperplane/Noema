//go:build linux

package provider

import (
	"context"
	"errors"
)

func detectLocalHardware(context.Context) ([]LocalHardwareProfile, error) {
	value, err := localProbeFile("/proc/meminfo")
	if err != nil {
		return nil, errors.New("physical memory is unavailable")
	}
	ram, ok := parseLinuxRAMGB(value)
	if !ok {
		return nil, errors.New("physical memory is invalid")
	}
	return []LocalHardwareProfile{{Backend: LocalModelCPU, RAMGB: ram}}, nil
}
