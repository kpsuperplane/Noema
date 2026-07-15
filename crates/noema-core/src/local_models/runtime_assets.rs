//! Immutable llama.cpp release manifest and desktop sidecar resolution.

use std::path::{Path, PathBuf};

use super::{LlamaServerCandidate, LocalModelBackend};

/// Immutable upstream llama.cpp release bundled with this Noema runtime.
pub const LLAMA_CPP_RELEASE_TAG: &str = "b10015";
/// Upstream commit referenced by [`LLAMA_CPP_RELEASE_TAG`].
pub const LLAMA_CPP_COMMIT: &str = "12127defda4f41b7679cb2477a4b0d65ee6a0c8f";
/// Development/test-only override for the `llama-server` executable.
pub const NOEMA_LLAMA_SERVER_PATH_ENV: &str = "NOEMA_LLAMA_SERVER_PATH";
/// Base name used for Tauri external sidecars and installed runtime binaries.
pub const LLAMA_SERVER_SIDECAR_BASENAME: &str = "noema-llama-server";

/// Role of one archive in a bundled llama.cpp runtime candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LlamaCppRuntimeAssetRole {
    /// Archive containing `llama-server` and backend libraries.
    ServerBundle,
    /// Additional vendor runtime libraries required by the server bundle.
    RuntimeLibraries,
}

/// Immutable upstream archive required to materialize a bundled sidecar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LlamaCppRuntimeAsset {
    /// Rust target triple for the packaged application.
    pub target_triple: &'static str,
    /// Inference backend supplied by this archive.
    pub backend: LocalModelBackend,
    /// Upstream GitHub release asset filename.
    pub archive_name: &'static str,
    /// Verified lowercase SHA-256 digest for the complete archive.
    pub sha256: &'static str,
    /// Whether the archive supplies the server or companion runtime libraries.
    pub role: LlamaCppRuntimeAssetRole,
}

/// Complete pinned upstream runtime asset manifest for V1 desktop targets.
pub const LLAMA_CPP_RUNTIME_ASSETS: &[LlamaCppRuntimeAsset] = &[
    LlamaCppRuntimeAsset {
        target_triple: "aarch64-apple-darwin",
        backend: LocalModelBackend::Metal,
        archive_name: "llama-b10015-bin-macos-arm64.tar.gz",
        sha256: "8d3144eb71a4b9b5b9ed50512f659d1cbcd5772e30aca75e6f9a0d7d68c311a7",
        role: LlamaCppRuntimeAssetRole::ServerBundle,
    },
    LlamaCppRuntimeAsset {
        target_triple: "x86_64-apple-darwin",
        backend: LocalModelBackend::Metal,
        archive_name: "llama-b10015-bin-macos-x64.tar.gz",
        sha256: "295a51dad3feafaafed8aa8fba9d9429fcaca8a2e7e66c73d839076fdf3fec6f",
        role: LlamaCppRuntimeAssetRole::ServerBundle,
    },
    LlamaCppRuntimeAsset {
        target_triple: "x86_64-unknown-linux-gnu",
        backend: LocalModelBackend::Vulkan,
        archive_name: "llama-b10015-bin-ubuntu-vulkan-x64.tar.gz",
        sha256: "cdd2fed8d96dcb584f6f7907df67c3787454e1f44e176580ae2da34fc79d63a0",
        role: LlamaCppRuntimeAssetRole::ServerBundle,
    },
    LlamaCppRuntimeAsset {
        target_triple: "x86_64-unknown-linux-gnu",
        backend: LocalModelBackend::Cpu,
        archive_name: "llama-b10015-bin-ubuntu-x64.tar.gz",
        sha256: "fc9c641c5ab5ce74b01d3a95123ff76ad488e1d46122d602f20604100ceae834",
        role: LlamaCppRuntimeAssetRole::ServerBundle,
    },
    LlamaCppRuntimeAsset {
        target_triple: "x86_64-pc-windows-msvc",
        backend: LocalModelBackend::Cuda,
        archive_name: "llama-b10015-bin-win-cuda-12.4-x64.zip",
        sha256: "336104b92b9b6a53a39eb7a373003d6256c1f20af3fc8213f65c2c3c11716469",
        role: LlamaCppRuntimeAssetRole::ServerBundle,
    },
    LlamaCppRuntimeAsset {
        target_triple: "x86_64-pc-windows-msvc",
        backend: LocalModelBackend::Cuda,
        archive_name: "cudart-llama-bin-win-cuda-12.4-x64.zip",
        sha256: "8c79a9b226de4b3cacfd1f83d24f962d0773be79f1e7b75c6af4ded7e32ae1d6",
        role: LlamaCppRuntimeAssetRole::RuntimeLibraries,
    },
    LlamaCppRuntimeAsset {
        target_triple: "x86_64-pc-windows-msvc",
        backend: LocalModelBackend::Vulkan,
        archive_name: "llama-b10015-bin-win-vulkan-x64.zip",
        sha256: "e2b63eba0fb124e51c93159540e11a861236f3701636d417b92504209a315210",
        role: LlamaCppRuntimeAssetRole::ServerBundle,
    },
    LlamaCppRuntimeAsset {
        target_triple: "x86_64-pc-windows-msvc",
        backend: LocalModelBackend::Cpu,
        archive_name: "llama-b10015-bin-win-cpu-x64.zip",
        sha256: "b850b461695b4c3e3811757fe20ae218bce06018a7f274c6c56485cf23adf980",
        role: LlamaCppRuntimeAssetRole::ServerBundle,
    },
];

/// Resolves backend-specific bundled runtime sidecars in fallback order.
///
/// Installed builds only resolve sidecars adjacent to the Noema executable.
/// Debug and test builds may opt into [`NOEMA_LLAMA_SERVER_PATH_ENV`] to use a
/// developer-built runtime; ambient `PATH` is never consulted.
#[must_use]
pub fn bundled_llama_server_candidates(
    preferred_backend: Option<LocalModelBackend>,
) -> Vec<LlamaServerCandidate> {
    bundled_llama_server_candidates_in(preferred_backend, None)
}

/// Resolves backend-specific runtime candidates from a packaged resource root.
///
/// The root contains one target-triple directory whose backend directories
/// preserve the verified upstream `llama-server` and its adjacent libraries.
#[must_use]
pub fn bundled_llama_server_candidates_in(
    preferred_backend: Option<LocalModelBackend>,
    runtime_root: Option<&Path>,
) -> Vec<LlamaServerCandidate> {
    #[cfg(any(test, debug_assertions))]
    if let Some(path) = std::env::var_os(NOEMA_LLAMA_SERVER_PATH_ENV)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
    {
        let preferred = preferred_backend.unwrap_or(default_platform_backend());
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
    platform_backends(preferred_backend)
        .into_iter()
        .map(|backend| {
            let sidecar_backend = if cfg!(target_os = "macos") && backend == LocalModelBackend::Cpu
            {
                LocalModelBackend::Metal
            } else {
                backend
            };
            let executable_path = runtime_root.map_or_else(
                || {
                    adjacent_directory
                        .as_ref()
                        .expect("adjacent runtime directory")
                        .join(installed_sidecar_name(sidecar_backend))
                },
                |root| {
                    root.join(current_target_triple())
                        .join(backend_slug(sidecar_backend))
                        .join(upstream_server_name())
                },
            );
            LlamaServerCandidate::new(backend, executable_path)
        })
        .collect()
}

/// Returns the Tauri build-input filename for one target/backend pair.
///
/// Packaging places this file under `crates/noema-desktop/binaries/` and
/// configures `binaries/noema-llama-server-<backend>` in `externalBin`.
#[must_use]
pub fn tauri_sidecar_input_name(backend: LocalModelBackend, target_triple: &str) -> String {
    let extension = if target_triple.contains("windows") {
        ".exe"
    } else {
        ""
    };
    format!(
        "{LLAMA_SERVER_SIDECAR_BASENAME}-{}-{target_triple}{extension}",
        backend_slug(backend)
    )
}

fn installed_sidecar_name(backend: LocalModelBackend) -> String {
    let extension = if cfg!(windows) { ".exe" } else { "" };
    format!(
        "{LLAMA_SERVER_SIDECAR_BASENAME}-{}{extension}",
        backend_slug(backend)
    )
}

const fn upstream_server_name() -> &'static str {
    if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    }
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

fn platform_backends(preferred: Option<LocalModelBackend>) -> Vec<LocalModelBackend> {
    let supported: &[LocalModelBackend] = if cfg!(target_os = "macos") {
        &[LocalModelBackend::Metal, LocalModelBackend::Cpu]
    } else if cfg!(target_os = "windows") {
        &[
            LocalModelBackend::Cuda,
            LocalModelBackend::Vulkan,
            LocalModelBackend::Cpu,
        ]
    } else {
        &[LocalModelBackend::Vulkan, LocalModelBackend::Cpu]
    };
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

const fn default_platform_backend() -> LocalModelBackend {
    if cfg!(target_os = "macos") {
        LocalModelBackend::Metal
    } else if cfg!(target_os = "windows") {
        LocalModelBackend::Cuda
    } else {
        LocalModelBackend::Vulkan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tauri_input_names_follow_target_triple_contract() {
        assert_eq!(
            tauri_sidecar_input_name(LocalModelBackend::Metal, "aarch64-apple-darwin"),
            "noema-llama-server-metal-aarch64-apple-darwin"
        );
        assert_eq!(
            tauri_sidecar_input_name(LocalModelBackend::Cuda, "x86_64-pc-windows-msvc"),
            "noema-llama-server-cuda-x86_64-pc-windows-msvc.exe"
        );
    }

    #[test]
    fn packaged_runtime_candidates_use_target_and_backend_directories() {
        let candidates = bundled_llama_server_candidates_in(
            Some(default_platform_backend()),
            Some(Path::new("/app/resources/binaries/runtime")),
        );

        assert_eq!(
            candidates[0].executable_path,
            Path::new("/app/resources/binaries/runtime")
                .join(current_target_triple())
                .join(backend_slug(default_platform_backend()))
                .join(upstream_server_name())
        );
    }

    #[test]
    fn runtime_manifest_uses_the_pinned_release_and_valid_hashes() {
        assert_eq!(LLAMA_CPP_RELEASE_TAG, "b10015");
        assert!(LLAMA_CPP_RUNTIME_ASSETS.iter().all(|asset| {
            asset.archive_name.contains(LLAMA_CPP_RELEASE_TAG)
                || asset.role == LlamaCppRuntimeAssetRole::RuntimeLibraries
        }));
        assert!(LLAMA_CPP_RUNTIME_ASSETS.iter().all(|asset| {
            asset.sha256.len() == 64
                && asset
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        }));
    }

    #[test]
    fn desktop_packaging_manifest_matches_runtime_contract() {
        let packaging: serde_json::Value = serde_json::from_str(include_str!(
            "../../resources/local-models/runtime-assets.json"
        ))
        .expect("packaging manifest");
        assert_eq!(packaging["release_tag"], LLAMA_CPP_RELEASE_TAG);
        assert_eq!(packaging["commit"], LLAMA_CPP_COMMIT);
        let assets = packaging["assets"].as_array().expect("packaging assets");
        assert_eq!(assets.len(), LLAMA_CPP_RUNTIME_ASSETS.len());

        for runtime in LLAMA_CPP_RUNTIME_ASSETS {
            let role = match runtime.role {
                LlamaCppRuntimeAssetRole::ServerBundle => "server_bundle",
                LlamaCppRuntimeAssetRole::RuntimeLibraries => "runtime_libraries",
            };
            assert!(assets.iter().any(|asset| {
                asset["target_triple"] == runtime.target_triple
                    && asset["backend"] == backend_slug(runtime.backend)
                    && asset["archive_name"] == runtime.archive_name
                    && asset["sha256"] == runtime.sha256
                    && asset["role"] == role
            }));
        }
    }
}
