//go:build linux

package provider

import (
	"slices"
	"testing"
)

func TestLinuxVulkanProbeIncludesARM64Library(t *testing.T) {
	const path = "/usr/lib/aarch64-linux-gnu/libvulkan.so.1"
	if !slices.Contains(linuxVulkanLibraryPaths[:], path) {
		t.Fatalf("Vulkan probe paths omit %q", path)
	}
}
