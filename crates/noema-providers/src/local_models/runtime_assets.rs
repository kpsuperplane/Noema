//! Immutable llama.cpp release manifest and desktop sidecar resolution.

use std::path::Path;
#[cfg(any(test, debug_assertions))]
use std::path::PathBuf;

use super::LlamaServerCandidate;
use crate::LocalModelBackend;

/// Immutable upstream llama.cpp release bundled with this Noema runtime.
#[cfg(feature = "local-model-evals")]
pub const LLAMA_CPP_RELEASE_TAG: &str = "b10015";
/// Upstream commit referenced by [`LLAMA_CPP_RELEASE_TAG`].
#[cfg(feature = "local-model-evals")]
pub const LLAMA_CPP_COMMIT: &str = "12127defda4f41b7679cb2477a4b0d65ee6a0c8f";
/// Development/test-only override for the `llama-server` executable.
#[cfg(any(test, debug_assertions))]
const NOEMA_LLAMA_SERVER_PATH_ENV: &str = "NOEMA_LLAMA_SERVER_PATH";
/// Base name used for Tauri external sidecars and installed runtime binaries.
const LLAMA_SERVER_SIDECAR_BASENAME: &str = "noema-llama-server";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeTargetOs {
    MacOs,
    Linux,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeTargetPlatform<'a> {
    target_triple: &'a str,
    os: RuntimeTargetOs,
}

impl<'a> RuntimeTargetPlatform<'a> {
    fn from_target_triple(target_triple: &'a str) -> Option<Self> {
        let os = if target_triple.ends_with("-apple-darwin") {
            RuntimeTargetOs::MacOs
        } else if target_triple.ends_with("-unknown-linux-gnu") {
            RuntimeTargetOs::Linux
        } else if target_triple.ends_with("-pc-windows-msvc") {
            RuntimeTargetOs::Windows
        } else {
            return None;
        };
        Some(Self { target_triple, os })
    }

    #[cfg(any(test, debug_assertions))]
    const fn default_backend(self) -> LocalModelBackend {
        match self.os {
            RuntimeTargetOs::MacOs => LocalModelBackend::Metal,
            RuntimeTargetOs::Linux => LocalModelBackend::Vulkan,
            RuntimeTargetOs::Windows => LocalModelBackend::Cuda,
        }
    }

    const fn upstream_server_name(self) -> &'static str {
        match self.os {
            RuntimeTargetOs::Windows => "llama-server.exe",
            RuntimeTargetOs::MacOs | RuntimeTargetOs::Linux => "llama-server",
        }
    }

    const fn installed_sidecar_extension(self) -> &'static str {
        match self.os {
            RuntimeTargetOs::Windows => ".exe",
            RuntimeTargetOs::MacOs | RuntimeTargetOs::Linux => "",
        }
    }

    const fn supported_backends(self) -> &'static [LocalModelBackend] {
        match self.os {
            RuntimeTargetOs::MacOs => &[LocalModelBackend::Metal, LocalModelBackend::Cpu],
            RuntimeTargetOs::Linux => &[LocalModelBackend::Vulkan, LocalModelBackend::Cpu],
            RuntimeTargetOs::Windows => &[
                LocalModelBackend::Cuda,
                LocalModelBackend::Vulkan,
                LocalModelBackend::Cpu,
            ],
        }
    }

    const fn sidecar_backend(self, backend: LocalModelBackend) -> LocalModelBackend {
        if matches!(self.os, RuntimeTargetOs::MacOs) && matches!(backend, LocalModelBackend::Cpu) {
            LocalModelBackend::Metal
        } else {
            backend
        }
    }
}

/// Resolves backend-specific bundled runtime sidecars in fallback order.
///
/// Installed builds only resolve sidecars adjacent to the Noema executable.
/// Debug and test builds may opt into `NOEMA_LLAMA_SERVER_PATH` to use a
/// developer-built runtime; ambient `PATH` is never consulted.
/// Resolves backend-specific runtime candidates from a packaged resource root.
///
/// The root contains one target-triple directory whose backend directories
/// preserve the verified upstream `llama-server` and its adjacent libraries.
#[must_use]
pub fn bundled_llama_server_candidates_in(
    preferred_backend: Option<LocalModelBackend>,
    runtime_root: Option<&Path>,
) -> Vec<LlamaServerCandidate> {
    let Some(target) = RuntimeTargetPlatform::from_target_triple(current_target_triple()) else {
        return Vec::new();
    };

    #[cfg(any(test, debug_assertions))]
    if let Some(path) = runtime_root
        .is_none()
        .then(|| std::env::var_os(NOEMA_LLAMA_SERVER_PATH_ENV))
        .flatten()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
    {
        let preferred = preferred_backend.unwrap_or(target.default_backend());
        let mut candidates = vec![LlamaServerCandidate::new(preferred, path.clone())];
        if preferred != LocalModelBackend::Cpu {
            candidates.push(LlamaServerCandidate::new(LocalModelBackend::Cpu, path));
        }
        return candidates;
    }

    let adjacent_directory = runtime_root.is_none().then(|| {
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(ToOwned::to_owned))
            .unwrap_or_default()
    });

    resolve_llama_server_candidates(
        preferred_backend,
        target,
        runtime_root,
        adjacent_directory.as_deref(),
    )
}

fn resolve_llama_server_candidates(
    preferred_backend: Option<LocalModelBackend>,
    target: RuntimeTargetPlatform<'_>,
    runtime_root: Option<&Path>,
    adjacent_directory: Option<&Path>,
) -> Vec<LlamaServerCandidate> {
    platform_backends(preferred_backend, target)
        .into_iter()
        .map(|backend| {
            let sidecar_backend = target.sidecar_backend(backend);
            let executable_path = runtime_root.map_or_else(
                || {
                    adjacent_directory
                        .as_ref()
                        .expect("adjacent runtime directory")
                        .join(installed_sidecar_name(sidecar_backend, target))
                },
                |root| {
                    root.join(target.target_triple)
                        .join(backend_slug(sidecar_backend))
                        .join(target.upstream_server_name())
                },
            );
            LlamaServerCandidate::new(backend, executable_path)
        })
        .collect()
}

fn installed_sidecar_name(backend: LocalModelBackend, target: RuntimeTargetPlatform<'_>) -> String {
    let extension = target.installed_sidecar_extension();
    format!(
        "{LLAMA_SERVER_SIDECAR_BASENAME}-{}{extension}",
        backend_slug(backend)
    )
}

const fn current_target_triple() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else {
        "unsupported-target"
    }
}

const fn backend_slug(backend: LocalModelBackend) -> &'static str {
    match backend {
        LocalModelBackend::Metal => "metal",
        LocalModelBackend::Cuda => "cuda",
        LocalModelBackend::Vulkan => "vulkan",
        LocalModelBackend::Cpu => "cpu",
    }
}

fn platform_backends(
    preferred: Option<LocalModelBackend>,
    target: RuntimeTargetPlatform<'_>,
) -> Vec<LocalModelBackend> {
    let supported = target.supported_backends();
    let mut backends = Vec::with_capacity(supported.len());
    if let Some(preferred) = preferred.filter(|backend| supported.contains(backend)) {
        backends.push(preferred);
    }
    for backend in supported.iter().copied() {
        if !backends.contains(&backend) {
            backends.push(backend);
        }
    }
    backends
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pinned_runtime_asset_resolves_for_its_injected_target_platform() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!(
            "../../resources/local-models/runtime-assets.json"
        ))
        .expect("runtime manifest");
        let root = Path::new("/app/resources/binaries/runtime");

        for asset in manifest["assets"].as_array().expect("assets") {
            let target_triple = asset["target_triple"].as_str().expect("target");
            let backend = match asset["backend"].as_str().expect("backend") {
                "metal" => LocalModelBackend::Metal,
                "cuda" => LocalModelBackend::Cuda,
                "vulkan" => LocalModelBackend::Vulkan,
                "cpu" => LocalModelBackend::Cpu,
                value => panic!("unknown manifest backend {value}"),
            };
            let target =
                RuntimeTargetPlatform::from_target_triple(target_triple).expect("supported target");
            let candidate =
                resolve_llama_server_candidates(Some(backend), target, Some(root), None)
                    .into_iter()
                    .find(|candidate| candidate.backend == backend)
                    .expect("asset backend resolves");
            assert_eq!(
                candidate.executable_path,
                root.join(target_triple)
                    .join(backend_slug(target.sidecar_backend(backend)))
                    .join(target.upstream_server_name()),
                "{target_triple}/{backend:?}"
            );
        }
    }
}
