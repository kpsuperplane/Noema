use serde::{Deserialize, Serialize};

/// Trusted identity selector type used for deterministic ownership matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustedIdentitySelectorKind {
    /// Email address selector.
    Email,
    /// Phone number selector.
    Phone,
    /// DNS domain selector.
    Domain,
}

impl TrustedIdentitySelectorKind {
    /// Return the persisted snake_case representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Phone => "phone",
            Self::Domain => "domain",
        }
    }
}

/// Normalize a trusted identity selector value before storage or matching.
#[must_use]
pub fn normalize_trusted_identity_value(
    kind: TrustedIdentitySelectorKind,
    value: &str,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    match kind {
        TrustedIdentitySelectorKind::Email => normalize_email_value(trimmed),
        TrustedIdentitySelectorKind::Domain => normalize_domain_value(trimmed),
        TrustedIdentitySelectorKind::Phone => normalize_phone_value(trimmed),
    }
}

fn normalize_email_value(trimmed: &str) -> Option<String> {
    if trimmed.chars().any(char::is_whitespace) {
        return None;
    }

    let (local, domain) = trimmed.split_once('@')?;
    if !is_valid_email_local_part(local) || domain.contains('@') {
        return None;
    }

    let normalized_domain = normalize_domain_value(domain)?;
    Some(format!(
        "{}@{}",
        local.to_ascii_lowercase(),
        normalized_domain
    ))
}

fn is_valid_email_local_part(local: &str) -> bool {
    !local.is_empty()
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && local
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-'))
}

fn normalize_domain_value(trimmed: &str) -> Option<String> {
    if trimmed.contains("://")
        || trimmed.contains('/')
        || trimmed.chars().any(char::is_whitespace)
        || !trimmed.contains('.')
    {
        return None;
    }

    let normalized = trimmed.to_ascii_lowercase();
    let labels_are_valid = normalized.split('.').all(is_valid_domain_label);

    labels_are_valid.then_some(normalized)
}

fn is_valid_domain_label(label: &str) -> bool {
    !label.is_empty()
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn normalize_phone_value(trimmed: &str) -> Option<String> {
    if !trimmed.starts_with('+') {
        return None;
    }

    let mut chars = trimmed.chars();
    chars.next();
    if !chars.all(is_allowed_phone_format_char) || trimmed[1..].contains('+') {
        return None;
    }

    let digits: String = trimmed[1..].chars().filter(char::is_ascii_digit).collect();
    let digit_count = digits.len();

    if (8..=15).contains(&digit_count) && !digits.starts_with('0') {
        Some(format!("+{digits}"))
    } else {
        None
    }
}

fn is_allowed_phone_format_char(ch: char) -> bool {
    ch.is_ascii_digit() || matches!(ch, ' ' | '-' | '.' | '(' | ')')
}
