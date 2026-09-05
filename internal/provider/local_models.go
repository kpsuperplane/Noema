package provider

import (
	"context"
	"errors"
	"fmt"
)

// LocalModelBackend identifies one local inference backend.
type LocalModelBackend string

const (
	LocalModelMetal  LocalModelBackend = "metal"
	LocalModelCUDA   LocalModelBackend = "cuda"
	LocalModelVulkan LocalModelBackend = "vulkan"
	LocalModelCPU    LocalModelBackend = "cpu"
)

// LocalModelBuild is one pinned GGUF artifact.
type LocalModelBuild struct {
	File, SHA256 string
	DownloadGB   float64
	Backends     []LocalModelBackend
	MinRAMGB     int
	MinVRAMGB    *int
}

// LocalHardwareProfile contains the machine values used for build selection.
type LocalHardwareProfile struct {
	Backend       LocalModelBackend
	RAMGB         int
	VRAMGB        *int
	UnifiedMemory bool
}

// LocalModelCatalogItem is one curated model with its machine-specific selection.
type LocalModelCatalogItem struct {
	ID, Name, License, Repo, Revision string
	Priority                          int
	SelectedBuild                     *LocalModelBuild
	Hardware                          *LocalHardwareProfile
	Explanation                       string
	Recommended                       bool
}

type localModelCatalogEntry struct {
	ID, Name, License, Repo, Revision string
	Priority                          int
	Builds                            []LocalModelBuild
}

var bundledLocalModels = []localModelCatalogEntry{{
	ID: "gemma-4-e4b-it", Name: "Gemma 4 E4B IT", License: "Apache-2.0", Priority: 100,
	Repo: "ggml-org/gemma-4-E4B-it-GGUF", Revision: "2714b5519c6c3516b1000e7c5e1eba998dfe1fe8",
	Builds: []LocalModelBuild{{
		File:       "gemma-4-E4B-it-Q4_K_M.gguf",
		SHA256:     "90ce98129eb3e8cc57e62433d500c97c624b1e3af1fcc85dd3b55ad7e0313e9f",
		DownloadGB: 5.3, Backends: []LocalModelBackend{LocalModelMetal}, MinRAMGB: 16,
		MinVRAMGB: intPointer(6),
	}},
}}

// DiscoverLocalModelCatalog returns the bundled catalog with current hardware matches.
func DiscoverLocalModelCatalog(ctx context.Context) ([]LocalModelCatalogItem, error) {
	hardware, err := detectLocalHardware(ctx)
	if err != nil {
		return nil, fmt.Errorf("detect local model hardware: %w", err)
	}
	return localModelCatalogForHardware(hardware), nil
}

// LocalModelHardwareProfiles returns usable backends in preference order.
func LocalModelHardwareProfiles(ctx context.Context) ([]LocalHardwareProfile, error) {
	profiles, err := detectLocalHardware(ctx)
	if err != nil {
		return nil, err
	}
	if len(profiles) == 0 {
		return nil, errors.New("local model hardware is unavailable")
	}
	return profiles, nil
}

func localModelCatalogForHardware(hardware []LocalHardwareProfile) []LocalModelCatalogItem {
	result := make([]LocalModelCatalogItem, len(bundledLocalModels))
	best := -1
	for index, source := range bundledLocalModels {
		item := LocalModelCatalogItem{
			ID: source.ID, Name: source.Name, License: source.License,
			Repo: source.Repo, Revision: source.Revision, Priority: source.Priority,
		}
		for _, profile := range hardware {
			for buildIndex := range source.Builds {
				build := &source.Builds[buildIndex]
				if !localModelBuildFits(*build, profile) {
					continue
				}
				selected, matched := *build, profile
				selected.Backends = append([]LocalModelBackend(nil), build.Backends...)
				item.SelectedBuild, item.Hardware = &selected, &matched
				item.Explanation = localModelExplanation(item.Name, matched)
				break
			}
			if item.SelectedBuild != nil {
				break
			}
		}
		result[index] = item
		if item.SelectedBuild != nil && (best < 0 || item.Priority > result[best].Priority) {
			best = index
		}
	}
	if best >= 0 {
		result[best].Recommended = true
	}
	return result
}

func localModelBuildFits(build LocalModelBuild, hardware LocalHardwareProfile) bool {
	if hardware.RAMGB < build.MinRAMGB || !containsLocalBackend(build.Backends, hardware.Backend) {
		return false
	}
	if build.MinVRAMGB == nil {
		return true
	}
	available := hardware.VRAMGB
	if hardware.UnifiedMemory {
		available = &hardware.RAMGB
	}
	return available != nil && *available >= *build.MinVRAMGB
}

func containsLocalBackend(backends []LocalModelBackend, expected LocalModelBackend) bool {
	for _, backend := range backends {
		if backend == expected {
			return true
		}
	}
	return false
}

func localModelExplanation(name string, hardware LocalHardwareProfile) string {
	memory := fmt.Sprintf("%d GB of accelerator memory", valueOrZero(hardware.VRAMGB))
	if hardware.UnifiedMemory {
		memory = fmt.Sprintf("%d GB of unified memory", hardware.RAMGB)
	} else if hardware.Backend == LocalModelCPU {
		memory = fmt.Sprintf("%d GB of system memory", hardware.RAMGB)
	}
	return fmt.Sprintf("Recommended because %s fits your %s backend and %s.", name, localBackendName(hardware.Backend), memory)
}

func localBackendName(backend LocalModelBackend) string {
	switch backend {
	case LocalModelMetal:
		return "Metal"
	case LocalModelCUDA:
		return "CUDA"
	case LocalModelVulkan:
		return "Vulkan"
	default:
		return "CPU"
	}
}

func valueOrZero(value *int) int {
	if value == nil {
		return 0
	}
	return *value
}

func intPointer(value int) *int { return &value }
