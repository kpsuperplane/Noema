//! Safe single-component path validation and identifier sanitization.

use std::path::{Component, Path};

use crate::NoemaPathError;

/// Convert an opaque identifier into one non-empty portable path component.
#[must_use]
pub fn sanitize_path_segment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "_".to_string()
    } else {
        sanitized
    }
}

/// Validate an artifact filename as exactly one safe path component.
///
/// # Errors
///
/// Returns [`NoemaPathError::UnsafeArtifactFilename`] for empty values, path
/// traversal, nested paths, control characters, backslashes, or quotes.
pub fn safe_artifact_filename(value: &str) -> Result<&str, NoemaPathError> {
    if value.is_empty()
        || value.contains('\\')
        || value.contains('"')
        || value.chars().any(char::is_control)
    {
        return Err(NoemaPathError::UnsafeArtifactFilename {
            value: value.to_string(),
        });
    }

    let mut components = Path::new(value).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(value),
        _ => Err(NoemaPathError::UnsafeArtifactFilename {
            value: value.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_artifact_filename_rejects_path_traversal() {
        assert!(safe_artifact_filename("report.md").is_ok());
        assert!(safe_artifact_filename("../report.md").is_err());
        assert!(safe_artifact_filename("nested/report.md").is_err());
        assert!(safe_artifact_filename("").is_err());
    }

    #[test]
    fn safe_artifact_filename_rejects_header_unsafe_characters() {
        assert!(safe_artifact_filename("report\".md").is_err());
        assert!(safe_artifact_filename("report\r.md").is_err());
        assert!(safe_artifact_filename("report\n.md").is_err());
    }

    #[test]
    fn sanitization_preserves_safe_characters_and_replaces_others() {
        assert_eq!(
            sanitize_path_segment("mcp:GitHub/Default"),
            "mcp_GitHub_Default"
        );
        assert_eq!(sanitize_path_segment(""), "_");
    }
}
