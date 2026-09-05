//go:build windows

package provider

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/sys/windows"
)

func detectLocalHardware(ctx context.Context) ([]LocalHardwareProfile, error) {
	windowsDirectory, systemDirectory, err := trustedWindowsDirectories()
	if err != nil {
		return nil, errors.New("Windows system directory is unavailable")
	}
	powershell := filepath.Join(systemDirectory, "WindowsPowerShell", "v1.0", "powershell.exe")
	environment := []string{"SystemRoot=" + windowsDirectory, "WINDIR=" + windowsDirectory}
	value, err := localProbeOutput(ctx, powershell, environment, "-NoProfile", "-NonInteractive", "-Command",
		"(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory")
	if err != nil {
		return nil, errors.New("physical memory is unavailable")
	}
	ram, ok := parseBytesRAMGB(value)
	if !ok {
		return nil, errors.New("physical memory is invalid")
	}
	profiles := make([]LocalHardwareProfile, 0, 3)
	nvidia := filepath.Join(systemDirectory, "nvidia-smi.exe")
	if info, probeErr := os.Stat(nvidia); probeErr == nil && info.Mode().IsRegular() {
		profiles = append(profiles, LocalHardwareProfile{Backend: LocalModelCUDA, RAMGB: ram})
	}
	vulkan := filepath.Join(systemDirectory, "vulkan-1.dll")
	if info, probeErr := os.Stat(vulkan); probeErr == nil && info.Mode().IsRegular() {
		profiles = append(profiles, LocalHardwareProfile{Backend: LocalModelVulkan, RAMGB: ram})
	}
	profiles = append(profiles, LocalHardwareProfile{Backend: LocalModelCPU, RAMGB: ram})
	return profiles, nil
}

func trustedWindowsDirectories() (string, string, error) {
	windowsDirectory, err := windows.GetWindowsDirectory()
	if err != nil {
		return "", "", err
	}
	systemDirectory, err := windows.GetSystemDirectory()
	if err != nil {
		return "", "", err
	}
	windowsDirectory, systemDirectory = filepath.Clean(windowsDirectory), filepath.Clean(systemDirectory)
	if !filepath.IsAbs(windowsDirectory) || !filepath.IsAbs(systemDirectory) ||
		filepath.VolumeName(windowsDirectory) == "" ||
		!strings.EqualFold(filepath.Dir(systemDirectory), windowsDirectory) ||
		!strings.EqualFold(filepath.Base(systemDirectory), "System32") {
		return "", "", errors.New("Windows system directory is invalid")
	}
	return windowsDirectory, systemDirectory, nil
}
