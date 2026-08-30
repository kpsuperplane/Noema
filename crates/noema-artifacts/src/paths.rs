//! Artifact-owned safe names, routes, and owner-relative filesystem layout.

use std::path::{Component, Path, PathBuf};

use noema_home::sanitize_path_segment;

use crate::{ArtifactDomainError, ArtifactOwnerRef};

const ARTIFACT_VERSION_ID_PREFIX: &str = "artifact_version:";

/// Validate an artifact filename as exactly one safe path component.
///
/// # Errors
///
/// Returns [`ArtifactDomainError::UnsafeFilename`] for empty values, path
/// traversal, nested paths, control characters, Windows-forbidden characters,
/// trailing aliases, or reserved device basenames.
pub fn safe_artifact_filename(value: &str) -> Result<&str, ArtifactDomainError> {
    if value.is_empty()
        || value.ends_with(['.', ' '])
        || value
            .chars()
            .any(|character| matches!(character, '\\' | '"' | ':' | '<' | '>' | '|' | '?' | '*'))
        || value.chars().any(char::is_control)
        || windows_device_basename(value)
    {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: value.to_string(),
        });
    }

    let mut components = Path::new(value).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(value),
        _ => Err(ArtifactDomainError::UnsafeFilename {
            value: value.to_string(),
        }),
    }
}

fn windows_device_basename(value: &str) -> bool {
    let basename = value
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    matches!(basename.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || basename
            .strip_prefix("COM")
            .or_else(|| basename.strip_prefix("LPT"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}

/// Return the root-owned directory for one artifact version.
///
/// # Errors
///
/// Returns [`ArtifactDomainError`] when the owner is unsupported or the
/// version index is not positive.
pub fn artifact_version_dir(
    root: &Path,
    owner: &ArtifactOwnerRef,
    artifact_id: &str,
    version_index: i64,
) -> Result<PathBuf, ArtifactDomainError> {
    if version_index < 1 {
        return Err(ArtifactDomainError::InvalidVersionIndex { version_index });
    }
    let artifact_segment = sanitize_path_segment(artifact_id);
    Ok(owner_artifacts_dir(root, owner)?
        .join(artifact_segment)
        .join("versions")
        .join(version_index.to_string()))
}

/// Return the artifact root for one supported concrete owner.
///
/// # Errors
///
/// Returns [`ArtifactDomainError`] when the owner is unsupported.
pub fn owner_artifacts_dir(
    root: &Path,
    owner: &ArtifactOwnerRef,
) -> Result<PathBuf, ArtifactDomainError> {
    owner.validate()?;
    let owner_segment = sanitize_path_segment(&owner.object_id);
    let base = match owner.object_type.as_str() {
        "conversation" => root.join("conversations").join(owner_segment),
        "task" => root.join("tasks").join(owner_segment),
        _ => {
            return Err(ArtifactDomainError::UnsupportedOwner {
                owner_object_type: owner.object_type.clone(),
                owner_object_id: owner.object_id.clone(),
            });
        }
    };
    Ok(base.join("artifacts"))
}

/// Build the local download route for an artifact version.
#[must_use]
pub fn artifact_download_url(artifact_version_id: &str) -> String {
    let slug = artifact_version_id
        .strip_prefix(ARTIFACT_VERSION_ID_PREFIX)
        .unwrap_or(artifact_version_id);
    format!("/artifacts/versions/{slug}/download")
}

/// Build the local inline-preview route for one artifact version.
#[must_use]
pub fn artifact_preview_url(artifact_version_id: &str) -> String {
    let slug = artifact_version_id
        .strip_prefix(ARTIFACT_VERSION_ID_PREFIX)
        .unwrap_or(artifact_version_id);
    format!("/artifacts/versions/{slug}/preview")
}

/// Convert a public artifact-version download slug back into a canonical id.
#[must_use]
pub fn artifact_version_id_from_download_slug(slug: &str) -> Option<String> {
    if slug.is_empty() || slug.contains('/') || slug.contains(':') {
        return None;
    }
    Some(format!("{ARTIFACT_VERSION_ID_PREFIX}{slug}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_artifact_filename_rejects_unsafe_cross_platform_path_and_header_components() {
        assert!(safe_artifact_filename("report.md").is_ok());
        for filename in [
            "",
            "../report.md",
            "nested/report.md",
            "report\".md",
            "report\r.md",
            "report\n.md",
        ] {
            assert!(safe_artifact_filename(filename).is_err(), "{filename:?}");
        }
        for filename in [
            "report:stream.txt",
            "report<draft>.txt",
            "report|draft.txt",
            "report?.txt",
            "report*.txt",
            "report.txt.",
            "report.txt ",
            "CON",
            "con.txt",
            "PRN.md",
            "AUX",
            "nul.json",
            "COM1.log",
            "com9",
            "LPT1.csv",
            "lpt9.txt",
        ] {
            assert!(safe_artifact_filename(filename).is_err(), "{filename}");
        }
        assert!(safe_artifact_filename("computer.txt").is_ok());
        assert!(safe_artifact_filename("com10.txt").is_ok());
        assert!(safe_artifact_filename("lpt0.txt").is_ok());
    }

    #[test]
    fn artifact_version_dirs_stay_below_supported_owner_roots() {
        for (owner, expected) in [
            (
                ArtifactOwnerRef::conversation("conversation:abc"),
                "/tmp/noema/conversations/conversation_abc/artifacts/artifact_def/versions/2",
            ),
            (
                ArtifactOwnerRef::task("task:abc"),
                "/tmp/noema/tasks/task_abc/artifacts/artifact_def/versions/2",
            ),
        ] {
            assert_eq!(
                artifact_version_dir(Path::new("/tmp/noema"), &owner, "artifact:def", 2)
                    .expect("path"),
                PathBuf::from(expected)
            );
        }
    }

    #[test]
    fn artifact_download_route_uses_and_validates_public_version_slugs() {
        assert_eq!(
            artifact_download_url("artifact_version:18c0aa78b3e7c5e86"),
            "/artifacts/versions/18c0aa78b3e7c5e86/download"
        );
        assert_eq!(
            artifact_preview_url("artifact_version:18c0aa78b3e7c5e86"),
            "/artifacts/versions/18c0aa78b3e7c5e86/preview"
        );
        assert_eq!(
            artifact_version_id_from_download_slug("18c0aa78b3e7c5e86"),
            Some("artifact_version:18c0aa78b3e7c5e86".to_string())
        );
        assert_eq!(artifact_version_id_from_download_slug(""), None);
        assert_eq!(
            artifact_version_id_from_download_slug("artifact_version:18c0aa78b3e7c5e86"),
            None
        );
        assert_eq!(artifact_version_id_from_download_slug("nested/path"), None);
    }
}
