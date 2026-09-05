//go:build darwin

package provider

import (
	"context"
	"errors"
	"runtime"
)

func detectLocalHardware(ctx context.Context) ([]LocalHardwareProfile, error) {
	environment := []string{"LANG=C", "LC_ALL=C"}
	value, err := localProbeOutput(ctx, "/usr/sbin/sysctl", environment, "-n", "hw.memsize")
	if err != nil {
		return nil, errors.New("physical memory is unavailable")
	}
	ram, ok := parseBytesRAMGB(value)
	if !ok {
		return nil, errors.New("physical memory is invalid")
	}
	unified := runtime.GOARCH == "arm64"
	var vram *int
	if !unified {
		if value, probeErr := localProbeOutput(ctx, "/usr/sbin/system_profiler", environment, "SPDisplaysDataType"); probeErr == nil {
			vram = parseMacVRAMGB(value)
		}
	}
	return []LocalHardwareProfile{
		{Backend: LocalModelMetal, RAMGB: ram, VRAMGB: vram, UnifiedMemory: unified},
		{Backend: LocalModelCPU, RAMGB: ram},
	}, nil
}
