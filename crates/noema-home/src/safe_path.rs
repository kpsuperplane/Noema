//! Safe identifier sanitization.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitization_preserves_safe_characters_and_replaces_others() {
        assert_eq!(
            sanitize_path_segment("mcp:GitHub/Default"),
            "mcp_GitHub_Default"
        );
        assert_eq!(sanitize_path_segment(""), "_");
    }
}
