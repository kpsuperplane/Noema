package provider

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestLocalModelCatalogSelectionPreservesPinnedContract(t *testing.T) {
	if ram, ok := parseLinuxRAMGB("MemTotal:       16777216 kB\nMemFree: 1 kB\n"); !ok || ram != 16 {
		t.Fatalf("Linux RAM = %d, %t", ram, ok)
	}
	if ram, ok := parseBytesRAMGB("17179869184\n"); !ok || ram != 16 {
		t.Fatalf("byte RAM = %d, %t", ram, ok)
	}
	if vram := parseMacVRAMGB("VRAM (Total): 6 GB\n"); vram == nil || *vram != 6 {
		t.Fatalf("macOS VRAM = %v", vram)
	}

	tests := []struct {
		name     string
		profile  LocalHardwareProfile
		selected bool
	}{
		{"unified boundary", LocalHardwareProfile{Backend: LocalModelMetal, RAMGB: 16, UnifiedMemory: true}, true},
		{"insufficient RAM", LocalHardwareProfile{Backend: LocalModelMetal, RAMGB: 15, UnifiedMemory: true}, false},
		{"discrete boundary", LocalHardwareProfile{Backend: LocalModelMetal, RAMGB: 16, VRAMGB: intPointer(6)}, true},
		{"insufficient VRAM", LocalHardwareProfile{Backend: LocalModelMetal, RAMGB: 16, VRAMGB: intPointer(5)}, false},
		{"unsupported backend", LocalHardwareProfile{Backend: LocalModelCPU, RAMGB: 64}, false},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			catalog := localModelCatalogForHardware([]LocalHardwareProfile{test.profile})
			if len(catalog) != 1 {
				t.Fatalf("catalog length = %d", len(catalog))
			}
			item := catalog[0]
			if item.ID != "gemma-4-e4b-it" || item.Revision != "2714b5519c6c3516b1000e7c5e1eba998dfe1fe8" {
				t.Fatalf("pinned catalog item = %#v", item)
			}
			if (item.SelectedBuild != nil) != test.selected || item.Recommended != test.selected {
				t.Fatalf("selection = %#v", item)
			}
			if test.selected && item.SelectedBuild.SHA256 != "90ce98129eb3e8cc57e62433d500c97c624b1e3af1fcc85dd3b55ad7e0313e9f" {
				t.Fatalf("selected build = %#v", item.SelectedBuild)
			}
			if test.selected && item.Explanation == "" {
				t.Fatal("selected model has no explanation")
			}
		})
	}
}

func TestLocalProbeIsBoundedAndPrivate(t *testing.T) {
	if mode := localProbeTestMode(); mode != "" {
		if mode == "timeout" {
			time.Sleep(10 * time.Second)
			return
		}
		_, _ = fmt.Fprint(os.Stdout, strings.Repeat("private-hardware-value", localProbeLimit))
		return
	}

	executable, err := filepath.Abs(os.Args[0])
	if err != nil {
		t.Fatal(err)
	}
	_, err = localProbeOutput(context.Background(), executable, nil,
		"-test.run=^TestLocalProbeIsBoundedAndPrivate$", "--", "noema-local-probe=output")
	if err != errLocalProbeTooLarge || strings.Contains(fmt.Sprint(err), "private-hardware-value") {
		t.Fatalf("large probe error = %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 25*time.Millisecond)
	defer cancel()
	started := time.Now()
	_, err = localProbeOutput(ctx, executable, nil,
		"-test.run=^TestLocalProbeIsBoundedAndPrivate$", "--", "noema-local-probe=timeout")
	if err == nil || time.Since(started) > time.Second {
		t.Fatalf("timed probe = %v after %s", err, time.Since(started))
	}
}

func TestLocalProbeUsesFixedPathAndMinimalEnvironment(t *testing.T) {
	if localProbeTestMode() == "environment" {
		_, _ = fmt.Fprintf(os.Stdout, "%s|%s", os.Getenv("NOEMA_PROVIDER_SECRET"), os.Getenv("LC_ALL"))
		os.Exit(0)
	}
	if _, err := localProbeOutput(context.Background(), "probe-from-path", nil); err != errLocalProbePath {
		t.Fatalf("relative probe path error = %v", err)
	}
	executable, err := filepath.Abs(os.Args[0])
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv("NOEMA_PROVIDER_SECRET", "must-not-reach-child")
	output, err := localProbeOutput(context.Background(), executable, []string{"LANG=C", "LC_ALL=C"},
		"-test.run=^TestLocalProbeUsesFixedPathAndMinimalEnvironment$", "--", "noema-local-probe=environment")
	if err != nil {
		t.Fatal(err)
	}
	if output != "|C" {
		t.Fatalf("probe environment = %q", output)
	}
}

func localProbeTestMode() string {
	for _, argument := range os.Args {
		if value, found := strings.CutPrefix(argument, "noema-local-probe="); found {
			return value
		}
	}
	return ""
}
