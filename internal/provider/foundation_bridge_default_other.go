//go:build !darwin || noema_release

package provider

import (
	"os"
	"path/filepath"
)

func defaultFoundationBridgeConfig() (string, *foundationBridgeBuild) {
	if executable, err := os.Executable(); err == nil {
		if sibling := filepath.Join(filepath.Dir(executable), "noema-foundation-bridge"); fileExists(sibling) {
			return sibling, nil
		}
	}
	return filepath.Join("crates", "noema-providers", "apple-foundation-bridge", ".build", "debug", "noema-foundation-bridge"), nil
}

func fileExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir()
}

func foundationDebugBuildExpected() bool { return false }
