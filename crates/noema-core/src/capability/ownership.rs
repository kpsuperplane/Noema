//! Deterministic MCP ownership extraction.

use crate::{OwnerExtractor, TrustedIdentitySelectorKind, normalize_trusted_identity_value};

/// Owner identity resolved from a deterministic extractor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOwner {
    /// Type of trusted identity that was resolved.
    pub selector_kind: TrustedIdentitySelectorKind,
    /// Normalized selector value used for trust matching.
    pub normalized_value: String,
}

/// Resolve an owner identity from a JSON document with a JSON Pointer extractor.
#[must_use]
pub fn resolve_owner_from_json(
    extractor: &OwnerExtractor,
    value: &serde_json::Value,
) -> Option<ResolvedOwner> {
    let raw = value.pointer(&extractor.path)?.as_str()?;
    let normalized = normalize_trusted_identity_value(extractor.selector_kind, raw)?;

    Some(ResolvedOwner {
        selector_kind: extractor.selector_kind,
        normalized_value: normalized,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OwnerExtractorSource, TrustedIdentitySelectorKind};

    #[test]
    fn json_pointer_owner_extractor_resolves_email_from_structured_content() {
        let extractor = OwnerExtractor {
            source: OwnerExtractorSource::StructuredContent,
            selector_kind: TrustedIdentitySelectorKind::Email,
            path: "/owner/email".to_string(),
        };

        let result = resolve_owner_from_json(
            &extractor,
            &serde_json::json!({"owner": {"email": "Kevin@Example.com"}}),
        )
        .expect("owner");

        assert_eq!(result.selector_kind, TrustedIdentitySelectorKind::Email);
        assert_eq!(result.normalized_value, "kevin@example.com");
    }
}
