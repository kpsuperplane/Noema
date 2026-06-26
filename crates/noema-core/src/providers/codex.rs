//! Provider adapter that shells out to `codex exec`.

use crate::provider::{
    GenerateInput, GenerateRequest, GenerateResponse, ModelProvider, ProviderError,
    output_items_from_text, required_output_items_from_text,
};
use std::process::Output;
use std::{io::ErrorKind, process::Stdio, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command, time};

/// Default time to wait for Codex process startup.
pub const DEFAULT_CODEX_STARTUP_TIMEOUT_SECONDS: u64 = 60;
/// Default time to wait for a Codex turn.
pub const DEFAULT_CODEX_TURN_TIMEOUT_SECONDS: u64 = 300;

/// Configuration for the Codex CLI provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexProviderConfig {
    /// Command used to invoke Codex.
    pub command: String,
    /// Optional default Codex model.
    pub default_model: Option<String>,
    /// Sandbox mode passed to Codex.
    pub sandbox: String,
    /// Whether Codex runs with an ephemeral session.
    pub ephemeral: bool,
    /// Whether Codex ignores repository rule files.
    pub ignore_rules: bool,
    /// Whether Codex ignores user-level config.
    pub ignore_user_config: bool,
    /// Startup timeout in seconds for app-server mode.
    pub startup_timeout_seconds: u64,
    /// Turn timeout in seconds.
    pub turn_timeout_seconds: u64,
    /// Optional `CODEX_HOME` override.
    pub codex_home: Option<String>,
}

impl Default for CodexProviderConfig {
    fn default() -> Self {
        Self {
            command: "codex".to_string(),
            default_model: None,
            sandbox: "read-only".to_string(),
            ephemeral: true,
            ignore_rules: true,
            ignore_user_config: false,
            startup_timeout_seconds: DEFAULT_CODEX_STARTUP_TIMEOUT_SECONDS,
            turn_timeout_seconds: DEFAULT_CODEX_TURN_TIMEOUT_SECONDS,
            codex_home: None,
        }
    }
}

/// Provider implementation backed by `codex exec`.
#[derive(Debug)]
pub struct CodexProvider {
    config: CodexProviderConfig,
}

impl CodexProvider {
    /// Build a Codex provider from validated configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] when required configuration
    /// values are empty or timeout values are zero.
    pub fn new(mut config: CodexProviderConfig) -> Result<Self, ProviderError> {
        if config.command.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "codex command cannot be empty".to_string(),
            });
        }

        if config.sandbox.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "codex sandbox cannot be empty".to_string(),
            });
        }

        if config.startup_timeout_seconds == 0 {
            return Err(ProviderError::InvalidRequest {
                message: "codex startup timeout must be greater than zero seconds".to_string(),
            });
        }

        if config.turn_timeout_seconds == 0 {
            return Err(ProviderError::InvalidRequest {
                message: "codex turn timeout must be greater than zero seconds".to_string(),
            });
        }

        config.command = config.command.trim().to_string();
        config.sandbox = config.sandbox.trim().to_string();
        config.default_model = config.default_model.and_then(|model| {
            let model = model.trim().to_string();
            (!model.is_empty()).then_some(model)
        });
        config.codex_home = config.codex_home.and_then(|codex_home| {
            let codex_home = codex_home.trim().to_string();
            (!codex_home.is_empty()).then_some(codex_home)
        });

        Ok(Self { config })
    }

    fn command(&self, model: Option<&str>) -> Command {
        let mut command = Command::new(&self.config.command);
        command
            .arg("exec")
            .arg("--sandbox")
            .arg(&self.config.sandbox)
            .arg("--color")
            .arg("never");

        if self.config.ephemeral {
            command.arg("--ephemeral");
        }

        if self.config.ignore_rules {
            command.arg("--ignore-rules");
        }

        if self.config.ignore_user_config {
            command.arg("--ignore-user-config");
        }

        if let Some(model) = model.filter(|model| !model.trim().is_empty()) {
            command.arg("--model").arg(model);
        }

        if let Some(codex_home) = self.config.codex_home.as_deref() {
            command.env("CODEX_HOME", codex_home);
        }

        command
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.kill_on_drop(true);
        command
    }

    async fn run_command(
        &self,
        mut command: Command,
        prompt: &str,
    ) -> Result<Output, ProviderError> {
        let mut child = command.spawn().map_err(|source| match source.kind() {
            ErrorKind::NotFound => ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: format!("command not found: {}", self.config.command),
            },
            _ => ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: source.to_string(),
            },
        })?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: "failed to open codex stdin".to_string(),
            })?;

        stdin.write_all(prompt.as_bytes()).await.map_err(|source| {
            ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: format!("failed to write prompt to codex stdin: {source}"),
            }
        })?;
        drop(stdin);

        time::timeout(
            Duration::from_secs(self.config.turn_timeout_seconds),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| ProviderError::ProviderUnavailable {
            provider: "codex".to_string(),
            message: format!(
                "codex exec timed out after {} seconds",
                self.config.turn_timeout_seconds
            ),
        })?
        .map_err(|source| ProviderError::ProviderUnavailable {
            provider: "codex".to_string(),
            message: source.to_string(),
        })
    }
}

impl ModelProvider for CodexProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let GenerateInput::Text(input) = request.input;
        if input.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }

        if request.options.max_output_tokens.is_some() {
            return Err(ProviderError::UnsupportedFeature {
                feature: "max_output_tokens for codex provider".to_string(),
            });
        }

        if request.options.temperature.is_some() {
            return Err(ProviderError::UnsupportedFeature {
                feature: "temperature for codex provider".to_string(),
            });
        }

        let model = request.model.or_else(|| self.config.default_model.clone());
        let prompt = compose_prompt(request.instructions.as_deref(), &input);
        let output = self
            .run_command(self.command(model.as_deref()), &prompt)
            .await?;

        if !output.status.success() {
            let status = output
                .status
                .code()
                .and_then(|code| u16::try_from(code).ok())
                .unwrap_or(1);
            let message = decode_output(&output.stderr)
                .or_else(|| decode_output(&output.stdout))
                .unwrap_or_else(|| "codex exec failed".to_string());

            return Err(ProviderError::ApiError {
                status,
                message,
                request_id: None,
            });
        }

        let text =
            decode_output(&output.stdout).ok_or_else(|| ProviderError::MalformedResponse {
                message: "codex exec produced empty stdout".to_string(),
            })?;

        let output = if request.options.require_noema_response {
            required_output_items_from_text(text)?
        } else {
            output_items_from_text(text)?
        };

        Ok(GenerateResponse {
            output,
            provider: "codex".to_string(),
            model: model.unwrap_or_else(|| "codex-default".to_string()),
            response_id: None,
            usage: None,
        })
    }
}

fn compose_prompt(instructions: Option<&str>, input: &str) -> String {
    match instructions.filter(|instructions| !instructions.trim().is_empty()) {
        Some(instructions) => format!("{}\n\n{}", instructions.trim(), input),
        None => input.to_string(),
    }
}

fn decode_output(output: &[u8]) -> Option<String> {
    let output = String::from_utf8_lossy(output)
        .trim_end_matches(['\r', '\n'])
        .to_string();

    if output.trim().is_empty() {
        None
    } else {
        Some(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{GenerateInput, GenerateOptions};
    use std::{fs, path::Path};

    #[tokio::test]
    async fn invokes_codex_exec_with_prompt_on_stdin() {
        let dir = tempfile::tempdir().expect("temp dir");
        let args_path = dir.path().join("args.txt");
        let stdin_path = dir.path().join("stdin.txt");
        let command_path = fake_codex_script(
            dir.path(),
            &format!(
                r#"
printf '%s\n' "$@" > '{}'
cat > '{}'
printf 'codex answer\n'
"#,
                args_path.display(),
                stdin_path.display()
            ),
        );

        let provider = CodexProvider::new(CodexProviderConfig {
            command: command_path.to_string_lossy().to_string(),
            default_model: Some("gpt-test".to_string()),
            sandbox: "read-only".to_string(),
            ephemeral: true,
            ignore_rules: true,
            ignore_user_config: false,
            startup_timeout_seconds: DEFAULT_CODEX_STARTUP_TIMEOUT_SECONDS,
            turn_timeout_seconds: DEFAULT_CODEX_TURN_TIMEOUT_SECONDS,
            codex_home: None,
        })
        .expect("provider");

        let response = provider
            .generate(GenerateRequest {
                model: None,
                input: GenerateInput::Text("hello".to_string()),
                instructions: Some("be brief".to_string()),
                options: GenerateOptions::default(),
            })
            .await
            .expect("response");

        assert_eq!(response.assistant_text(), "codex answer");
        assert_eq!(response.provider, "codex");
        assert_eq!(response.model, "gpt-test");

        let args = fs::read_to_string(args_path).expect("args");
        assert_eq!(
            args,
            "exec\n--sandbox\nread-only\n--color\nnever\n--ephemeral\n--ignore-rules\n--model\ngpt-test\n-\n"
        );

        let stdin = fs::read_to_string(stdin_path).expect("stdin");
        assert_eq!(stdin, "be brief\n\nhello");
    }

    #[tokio::test]
    async fn maps_nonzero_exit_to_api_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let command_path = fake_codex_script(
            dir.path(),
            r"
printf 'not logged in\n' >&2
exit 42
",
        );

        let provider = CodexProvider::new(CodexProviderConfig {
            command: command_path.to_string_lossy().to_string(),
            ..CodexProviderConfig::default()
        })
        .expect("provider");

        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::ApiError { status: 42, message, .. } if message == "not logged in"
        ));
    }

    #[tokio::test]
    async fn missing_command_is_provider_unavailable() {
        let provider = CodexProvider::new(CodexProviderConfig {
            command: "definitely-not-a-real-codex-command".to_string(),
            ..CodexProviderConfig::default()
        })
        .expect("provider");

        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::ProviderUnavailable { provider, .. } if provider == "codex"
        ));
    }

    #[test]
    fn rejects_zero_timeout() {
        let error = CodexProvider::new(CodexProviderConfig {
            turn_timeout_seconds: 0,
            ..CodexProviderConfig::default()
        })
        .unwrap_err();

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    fn fake_codex_script(dir: &Path, body: &str) -> std::path::PathBuf {
        let path = dir.join("fake-codex");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write script");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).expect("chmod");
        }

        path
    }
}
