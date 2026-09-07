package home

import (
	"errors"
	"fmt"
	"os"
)

// DefaultConfigYAML is the host's first-run configuration document.
const DefaultConfigYAML = `# Noema configuration
provider: codex

codex:
  base_url: https://chatgpt.com/backend-api/codex
  # Explicit model overrides must also set reasoning_effort when supported.
  # model: <model-id>
  # reasoning_effort: medium
  # tool_classification_model defaults to gpt-5.4-mini when unset.
  # tool_classification_model: gpt-5.4-mini
  timeout_seconds: 300

# The daemon opens the embedded Noema store under this home directory.

browser:
  max_sessions: 2
  max_old_space_mb: 1024

web:
  host: 127.0.0.1
  port: 3737
  rp_id: localhost
  dev_no_auth: false
  local_graphql_socket: false
  graphiql: false
  # Set this to the exact HTTPS origin exposed by your reverse proxy.
  # For deployment, set both values to the stable public domain.
  # rp_id: noema.example.com
  # public_origin: https://noema.example.com

mcp:
  stdio_enabled: false
`

// Initialize creates the host home layout and writes initial configuration
// only when the configuration file does not exist.
func Initialize(paths Paths, initialConfig []byte) error {
	if paths.Root() == "" {
		return errors.New("home is not resolved")
	}
	if err := os.MkdirAll(paths.Root(), 0o700); err != nil {
		return fmt.Errorf("create Noema home: %w", err)
	}
	if err := protectDirectory(paths.Root()); err != nil {
		return fmt.Errorf("protect Noema home: %w", err)
	}
	runPath := paths.Root() + string(os.PathSeparator) + "run"
	if err := os.MkdirAll(runPath, 0o700); err != nil {
		return fmt.Errorf("create Noema run directory: %w", err)
	}
	if err := protectDirectory(runPath); err != nil {
		return fmt.Errorf("protect Noema run directory: %w", err)
	}
	_, err := os.Lstat(paths.Config())
	switch {
	case errors.Is(err, os.ErrNotExist):
		if initialConfig != nil {
			if err := AtomicWritePrivate(paths.Config(), initialConfig); err != nil {
				return fmt.Errorf("write initial config: %w", err)
			}
		}
	case err == nil:
		if err := ProtectFile(paths.Config()); err != nil {
			return fmt.Errorf("protect config: %w", err)
		}
	case err != nil:
		return fmt.Errorf("inspect config: %w", err)
	}
	return nil
}
