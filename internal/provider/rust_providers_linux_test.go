//go:build linux

package provider

import "testing"

// Rust source: crates/noema-providers/src/local_models/hardware.rs::parses_linux_memory_in_whole_gibibytes (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ParsesLinuxMemoryInWholeGibibytes(t *testing.T) {
	if value, ok := parseLinuxRAMGB("MemTotal:       16777216 kB\nMemFree: 1 kB\n"); !ok || value != 16 {
		t.Fatalf("Linux memory parse = %d, %t; want 16, true", value, ok)
	}
	if value, ok := parseLinuxRAMGB("MemFree: 1 kB\n"); ok || value != 0 {
		t.Fatalf("missing Linux memory parse = %d, %t; want 0, false", value, ok)
	}
}
