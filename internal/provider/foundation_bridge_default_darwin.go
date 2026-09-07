//go:build darwin && !noema_release

package provider

import "path/filepath"

func defaultFoundationBridgeConfig() (string, *foundationBridgeBuild) {
	packagePath := filepath.Join("crates", "noema-providers", "apple-foundation-bridge")
	return filepath.Join(packagePath, ".build", "debug", "noema-foundation-bridge"), &foundationBridgeBuild{
		PackagePath:     packagePath,
		SwiftExecutable: "swift",
	}
}

func foundationDebugBuildExpected() bool { return true }
