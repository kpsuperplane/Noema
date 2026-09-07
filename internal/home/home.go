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

// MissingHomeError means that no explicit or fallback home directory exists.
type MissingHomeError struct{}

func (MissingHomeError) Error() string {
	return "could not determine Noema directory; set NOEMA_HOME or a platform home directory"
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

// LocalModelImportPartialPath returns the path for one model import transfer.
func (p Paths) LocalModelImportPartialPath(installationID string) string {
	return filepath.Join(p.root, "models", "downloads", "import-"+sanitizePathSegment(installationID)+".part")
}

// CapabilityAuthArgumentsDir returns the protected capability-auth argument directory.
func (p Paths) CapabilityAuthArgumentsDir() string {
	return filepath.Join(p.root, "run", "capability-auth")
}

// AdapterDefinitionDir returns one content-addressed adapter definition directory.
func (p Paths) AdapterDefinitionDir(digest string) (string, error) {
	if !isLowerHex(digest, 64) {
		return "", errors.New("adapter digest must be 64 lowercase hexadecimal characters")
	}
	return filepath.Join(p.root, "adapters", "definitions", digest), nil
}

// AdapterSourcePath returns one imported adapter source path.
func (p Paths) AdapterSourcePath(digest, extension string) (string, error) {
	if !isLowerHex(digest, 64) {
		return "", errors.New("adapter digest must be 64 lowercase hexadecimal characters")
	}
	if extension != "json" && extension != "yaml" && extension != "yml" {
		return "", errors.New("unsupported adapter source extension")
	}
	return filepath.Join(p.root, "adapters", "sources", digest+"."+extension), nil
}

// AdapterConnectionDir returns one stable adapter connection directory.
func (p Paths) AdapterConnectionDir(connectionID string) (string, error) {
	if !isLowerHex(connectionID, 32) {
		return "", errors.New("adapter connection id must be 32 lowercase hexadecimal characters")
	}
	return filepath.Join(p.root, "adapters", "connections", connectionID), nil
}

// AdapterOAuthProfileDir returns one reviewed OAuth profile directory.
func (p Paths) AdapterOAuthProfileDir(digest string) (string, error) {
	if !isLowerHex(digest, 64) {
		return "", errors.New("adapter digest must be 64 lowercase hexadecimal characters")
	}
	return filepath.Join(p.root, "adapters", "oauth-profiles", digest), nil
}

// AdapterOAuthApplicationDir returns one OAuth application directory.
func (p Paths) AdapterOAuthApplicationDir(id string) (string, error) {
	if !isLowerHex(id, 32) {
		return "", errors.New("adapter object id must be 32 lowercase hexadecimal characters")
	}
	return filepath.Join(p.root, "adapters", "oauth-applications", id), nil
}

// AdapterExternalAccountDir returns one external account directory.
func (p Paths) AdapterExternalAccountDir(id string) (string, error) {
	if !isLowerHex(id, 32) {
		return "", errors.New("adapter object id must be 32 lowercase hexadecimal characters")
	}
	return filepath.Join(p.root, "adapters", "external-accounts", id), nil
}

// AdapterOAuthGrantDir returns one OAuth grant directory.
func (p Paths) AdapterOAuthGrantDir(id string) (string, error) {
	if !isLowerHex(id, 32) {
		return "", errors.New("adapter object id must be 32 lowercase hexadecimal characters")
	}
	return filepath.Join(p.root, "adapters", "oauth-grants", id), nil
}

// LocalModelBlobPath returns one verified content-addressed model blob path.
func (p Paths) LocalModelBlobPath(digest string) (string, error) {
	if !isLowerHex(digest, 64) {
		return "", errors.New("model SHA-256 digest must be 64 lowercase hexadecimal characters")
	}
	return filepath.Join(p.root, "models", "blobs", digest+".gguf"), nil
}

// LocalModelPartialPath returns one resumable model download path.
func (p Paths) LocalModelPartialPath(digest string) (string, error) {
	if !isLowerHex(digest, 64) {
		return "", errors.New("model SHA-256 digest must be 64 lowercase hexadecimal characters")
	}
	return filepath.Join(p.root, "models", "downloads", digest+".part"), nil
}

// ProviderAccountHome returns one sanitized provider account directory.
func (p Paths) ProviderAccountHome(providerKind, accountKey string) string {
	return filepath.Join(p.root, "providers", sanitizePathSegment(providerKind), sanitizePathSegment(accountKey))
}

// MCPDir returns the MCP server configuration root.
func (p Paths) MCPDir() string {
	return filepath.Join(p.root, "mcp")
}

// MCPServerHome returns one sanitized MCP server configuration directory.
func (p Paths) MCPServerHome(serverID string) string {
	return filepath.Join(p.MCPDir(), sanitizePathSegment(serverID))
}

// Config returns the protected startup configuration path.
func (p Paths) Config() string {
	return filepath.Join(p.root, "config.yaml")
}

// ErrorsLog returns the durable developer error log path.
func (p Paths) ErrorsLog() string {
	return filepath.Join(p.root, "errors.log")
}

// BrowserSessionKey returns the protected browser cookie key path.
func (p Paths) BrowserSessionKey() string {
	return filepath.Join(p.root, "run", "browser-session.key")
}

// NativeOAuthRetries returns the protected native refresh recovery path.
func (p Paths) NativeOAuthRetries() string {
	return filepath.Join(p.root, "run", "native-oauth-retries.json")
}

// FaviconCacheDir returns the rebuildable public-site favicon cache.
func (p Paths) FaviconCacheDir() string {
	return filepath.Join(p.root, "system", "cache", "favicons")
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
	if _, err := os.Lstat(filepath.Join(p.root, "db", "noema.sqlite3")); err == nil {
		return nil, errors.New("Rust-created NOEMA_HOME is unsupported")
	} else if !errors.Is(err, os.ErrNotExist) {
		return nil, fmt.Errorf("inspect NOEMA_HOME: %w", err)
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
		return Paths{}, MissingHomeError{}
	}
	return FromRoot(filepath.Join(root, defaultName))
}

func isLowerHex(value string, length int) bool {
	if len(value) != length {
		return false
	}
	for _, character := range value {
		if character >= '0' && character <= '9' || character >= 'a' && character <= 'f' {
			continue
		}
		return false
	}
	return true
}

func sanitizePathSegment(value string) string {
	var sanitized []rune
	for _, character := range value {
		if character >= 'a' && character <= 'z' || character >= 'A' && character <= 'Z' ||
			character >= '0' && character <= '9' || character == '-' || character == '_' {
			sanitized = append(sanitized, character)
		} else {
			sanitized = append(sanitized, '_')
		}
	}
	if len(sanitized) == 0 {
		return "_"
	}
	return string(sanitized)
}
