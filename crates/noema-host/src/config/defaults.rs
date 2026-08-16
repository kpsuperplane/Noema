//! Host-owned product default configuration content.

/// Default config written during Noema home initialization.
pub const DEFAULT_NOEMA_CONFIG_YAML: &str = r"# Noema configuration
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
";
