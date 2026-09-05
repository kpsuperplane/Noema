// Package home owns access to one Go-created Noema home.
package home

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

const (
	// EnvironmentName selects the Noema home directory.
	EnvironmentName = "NOEMA_HOME"
	defaultName     = ".noema"
)

// Paths contains resolved paths for one Noema home.
type Paths struct {
	root string
}

// Resolve reads the current process environment.
func Resolve() (Paths, error) {
	return resolve(os.LookupEnv, os.UserHomeDir)
}

// FromRoot uses an explicit Noema home directory.
func FromRoot(root string) (Paths, error) {
	if root == "" {
		return Paths{}, errors.New("NOEMA_HOME cannot be empty")
	}

	abs, err := filepath.Abs(root)
	if err != nil {
		return Paths{}, fmt.Errorf("resolve NOEMA_HOME: %w", err)
	}

	return Paths{root: filepath.Clean(abs)}, nil
}

// Root returns the absolute Noema home directory.
func (p Paths) Root() string {
	return p.root
}

// Database returns the Go server database path.
func (p Paths) Database() string {
	return filepath.Join(p.root, "noema.sqlite3")
}

// Config returns the protected startup configuration path.
func (p Paths) Config() string {
	return filepath.Join(p.root, "config.yaml")
}

// BrowserSessionKey returns the protected browser cookie key path.
func (p Paths) BrowserSessionKey() string {
	return filepath.Join(p.root, "run", "browser-session.key")
}

// NativeOAuthRetries returns the protected native refresh recovery path.
func (p Paths) NativeOAuthRetries() string {
	return filepath.Join(p.root, "run", "native-oauth-retries.json")
}

// WebPushVAPID returns the protected browser Push signing-key path.
func (p Paths) WebPushVAPID() string {
	return filepath.Join(p.root, "notifications", "web-push-vapid.json")
}

// APNSProvider returns the protected Apple Push provider configuration path.
func (p Paths) APNSProvider() string {
	return filepath.Join(p.root, "notifications", "apns-provider.json")
}

// Open creates the home and returns rooted filesystem access.
func (p Paths) Open() (*os.Root, error) {
	if p.root == "" {
		return nil, errors.New("home is not resolved")
	}
	if err := os.MkdirAll(p.root, 0o700); err != nil {
		return nil, fmt.Errorf("create Noema home: %w", err)
	}
	if err := protectDirectory(p.root); err != nil {
		return nil, fmt.Errorf("protect Noema home: %w", err)
	}

	root, err := os.OpenRoot(p.root)
	if err != nil {
		return nil, fmt.Errorf("open Noema home: %w", err)
	}
	return root, nil
}

// ProtectFile restricts one file to the current operating-system user.
func ProtectFile(path string) error {
	return protectPath(path, false)
}

func resolve(
	lookup func(string) (string, bool),
	userHome func() (string, error),
) (Paths, error) {
	if root, exists := lookup(EnvironmentName); exists {
		return FromRoot(root)
	}

	root, err := userHome()
	if err != nil {
		return Paths{}, fmt.Errorf("resolve user home: %w", err)
	}
	if root == "" {
		return Paths{}, errors.New("user home cannot be empty")
	}
	return FromRoot(filepath.Join(root, defaultName))
}
