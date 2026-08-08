//! Opaque local-model evaluation lifecycle.

mod materialize;

use std::{fmt, path::PathBuf};

use thiserror::Error;

use crate::{LocalModelBackend, LocalModelsProviderConfig, ProviderHandle, erase_model_provider};

use super::{LLAMA_CPP_COMMIT, LLAMA_CPP_RELEASE_TAG, LlamaServerSupervisor, LocalModelsProvider};

pub use materialize::{
    MaterializeVerifiedEvalModelRequest, VerifiedEvalModelSource, materialize_verified_eval_model,
};

/// Explicit runtime inputs for one isolated local-model evaluation session.
#[derive(Clone)]
pub struct LocalModelEvalSessionConfig {
    /// Provider-facing model id used by evaluation requests.
    pub model_id: String,
    /// Already verified GGUF file.
    pub model_path: PathBuf,
    /// Prepared root containing target/backend-specific llama.cpp binaries.
    pub runtime_root: PathBuf,
    /// Context window exposed to the evaluation harness.
    pub context_window_tokens: u32,
    /// Per-generation timeout.
    pub timeout_seconds: u64,
    /// Runtime readiness timeout.
    pub startup_timeout_seconds: u64,
}

impl fmt::Debug for LocalModelEvalSessionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalModelEvalSessionConfig")
            .field("model_id", &self.model_id)
            .field("model_path", &self.model_path)
            .field("runtime_root", &self.runtime_root)
            .field("context_window_tokens", &self.context_window_tokens)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("startup_timeout_seconds", &self.startup_timeout_seconds)
            .finish()
    }
}

/// Pinned llama.cpp source identity used by evaluation reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalModelEvalRuntimeVersion {
    /// Upstream release tag.
    pub release_tag: &'static str,
    /// Upstream commit.
    pub commit: &'static str,
}

/// Return the pinned runtime identity without exposing packaging internals.
#[must_use]
pub const fn local_model_eval_runtime_version() -> LocalModelEvalRuntimeVersion {
    LocalModelEvalRuntimeVersion {
        release_tag: LLAMA_CPP_RELEASE_TAG,
        commit: LLAMA_CPP_COMMIT,
    }
}

/// One ready local-model process exposed through provider-neutral operations.
pub struct LocalModelEvalSession {
    provider: ProviderHandle,
    runtime: LlamaServerSupervisor,
    selected_backend: LocalModelBackend,
    process_id: Option<u32>,
}

impl LocalModelEvalSession {
    /// Start and health-check one verified model against an explicit runtime root.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelEvalError`] when configuration is invalid or the
    /// local runtime cannot reach readiness.
    pub async fn start(config: LocalModelEvalSessionConfig) -> Result<Self, LocalModelEvalError> {
        let provider = LocalModelsProvider::new(LocalModelsProviderConfig {
            default_model: config.model_id,
            model_path: Some(config.model_path),
            preferred_backend: None,
            runtime_root: Some(config.runtime_root),
            context_window_tokens: config.context_window_tokens,
            timeout_seconds: config.timeout_seconds,
            startup_timeout_seconds: config.startup_timeout_seconds,
            system_errors: None,
        })
        .map_err(|error| LocalModelEvalError::Runtime(error.to_string()))?;
        let endpoint = provider
            .runtime()
            .ensure_ready()
            .await
            .map_err(|error| LocalModelEvalError::Runtime(error.to_string()))?;
        let runtime = provider.runtime().clone();
        let process_id = runtime.process_id();
        Ok(Self {
            provider: erase_model_provider(provider),
            runtime,
            selected_backend: endpoint.backend,
            process_id,
        })
    }

    /// Clone the provider-neutral generation handle for this session.
    #[must_use]
    pub fn provider(&self) -> ProviderHandle {
        self.provider.clone()
    }

    /// Return the backend that reached readiness.
    #[must_use]
    pub const fn selected_backend(&self) -> LocalModelBackend {
        self.selected_backend
    }

    /// Return the supervised process id for sampling, when available.
    #[must_use]
    pub const fn process_id(&self) -> Option<u32> {
        self.process_id
    }

    /// Stop the supervised process and permanently close generation admission.
    pub async fn shutdown(&self) {
        self.runtime.shutdown().await;
    }
}

impl fmt::Debug for LocalModelEvalSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalModelEvalSession")
            .field("selected_backend", &self.selected_backend)
            .field("process_id", &self.process_id)
            .finish_non_exhaustive()
    }
}

/// Failures from the narrow evaluation-only local-model API.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LocalModelEvalError {
    /// Evaluation input did not satisfy the immutable-source contract.
    #[error("invalid local-model evaluation input: {0}")]
    InvalidInput(String),
    /// The caller cancelled materialization.
    #[error("local-model evaluation materialization was cancelled")]
    Cancelled,
    /// Model bytes could not be downloaded or published.
    #[error("local-model evaluation materialization failed: {0}")]
    Materialization(String),
    /// The isolated runtime could not start.
    #[error("local-model evaluation runtime failed: {0}")]
    Runtime(String),
}

#[cfg(all(test, unix))]
mod tests {
    use std::{collections::HashSet, os::unix::fs::PermissionsExt};

    use super::*;
    use crate::{LocalModelRuntimeStatus, local_models::bundled_llama_server_candidates_in};

    const FAKE_SERVER: &str = r#"#!/bin/sh
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--port" ]; then port="$2"; break; fi
  shift
done
exec python3 - "$port" <<'PY'
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200); self.end_headers(); self.wfile.write(b"{}")
    def log_message(self, *_args): pass
HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
PY
"#;

    #[tokio::test]
    async fn eval_session_exposes_only_ready_provider_metadata_and_owned_shutdown() {
        let directory = tempfile::tempdir().expect("fixture");
        let runtime_root = directory.path().join("runtime");
        let candidates = bundled_llama_server_candidates_in(None, Some(&runtime_root));
        let expected_backend = candidates.first().expect("platform candidate").backend;
        let mut prepared = HashSet::new();
        for path in candidates
            .into_iter()
            .map(|candidate| candidate.executable_path)
            .filter(|path| prepared.insert(path.clone()))
        {
            tokio::fs::create_dir_all(path.parent().expect("runtime directory"))
                .await
                .expect("create runtime directory");
            tokio::fs::write(&path, FAKE_SERVER)
                .await
                .expect("fake runtime");
            let mut permissions = tokio::fs::metadata(&path)
                .await
                .expect("metadata")
                .permissions();
            permissions.set_mode(0o700);
            tokio::fs::set_permissions(path, permissions)
                .await
                .expect("permissions");
        }
        let model_path = directory.path().join("model.gguf");
        tokio::fs::write(&model_path, b"GGUF eval session fixture")
            .await
            .expect("model");

        let config = LocalModelEvalSessionConfig {
            model_id: "eval-model".to_string(),
            model_path: model_path.clone(),
            runtime_root: runtime_root.clone(),
            context_window_tokens: 4_096,
            timeout_seconds: 5,
            startup_timeout_seconds: 3,
        };
        let debug = format!("{config:?}");
        assert!(debug.contains(&model_path.display().to_string()));
        assert!(debug.contains(&runtime_root.display().to_string()));

        let session = LocalModelEvalSession::start(config)
            .await
            .expect("ready session");
        assert_eq!(session.selected_backend(), expected_backend);
        assert!(session.process_id().is_some());
        let provider = session.provider();
        assert_eq!(
            provider.default_tool_classification_model().as_deref(),
            Some("eval-model")
        );
        assert_eq!(
            provider.context_metadata(None).await.context_window_tokens,
            Some(4_096)
        );

        session.shutdown().await;
        assert_eq!(session.runtime.status(), LocalModelRuntimeStatus::Stopped);
        assert_eq!(session.runtime.process_id(), None);
    }
}
