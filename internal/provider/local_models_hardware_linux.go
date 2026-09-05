//go:build linux

package provider

import (
	"context"
	"errors"
	"os"
)

var linuxVulkanLibraryPaths = [...]string{
	"/usr/lib/libvulkan.so.1",
	"/usr/lib64/libvulkan.so.1",
	"/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
	"/usr/lib/aarch64-linux-gnu/libvulkan.so.1",
}

func detectLocalHardware(context.Context) ([]LocalHardwareProfile, error) {
	value, err := localProbeFile("/proc/meminfo")
	if err != nil {
		return nil, errors.New("physical memory is unavailable")
	}
	ram, ok := parseLinuxRAMGB(value)
	if !ok {
		return nil, errors.New("physical memory is invalid")
	}
	profiles := make([]LocalHardwareProfile, 0, 2)
	for _, path := range linuxVulkanLibraryPaths {
		if info, probeErr := os.Stat(path); probeErr == nil && info.Mode().IsRegular() {
			profiles = append(profiles, LocalHardwareProfile{Backend: LocalModelVulkan, RAMGB: ram})
			break
		}
	}
	profiles = append(profiles, LocalHardwareProfile{Backend: LocalModelCPU, RAMGB: ram})
	return profiles, nil
}
