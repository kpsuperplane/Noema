//go:build windows

package provider

import (
	"context"
	"errors"
)

func detectLocalHardware(ctx context.Context) ([]LocalHardwareProfile, error) {
	value, err := localProbeOutput(ctx, "powershell.exe", "-NoProfile", "-NonInteractive", "-Command",
		"(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory")
	if err != nil {
		return nil, errors.New("physical memory is unavailable")
	}
	ram, ok := parseBytesRAMGB(value)
	if !ok {
		return nil, errors.New("physical memory is invalid")
	}
	return []LocalHardwareProfile{{Backend: LocalModelCPU, RAMGB: ram}}, nil
}
