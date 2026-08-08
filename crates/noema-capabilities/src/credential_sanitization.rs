//! Small protocol-owned credential cleanup for model-visible and persisted values.

use serde_json::Value;
use std::collections::BTreeSet;
use url::Url;

const REDACTED: &str = "[REDACTED]";

const STANDARD_CREDENTIAL_FIELDS: &[&str] = &[
    "access_token",
    "api-key",
    "api_key",
    "apikey",
    "authorization",
    "client_assertion",
    "client_secret",
    "code_verifier",
    "cookie",
    "device_code",
    "id_token",
    "password",
    "proxy-authorization",
    "proxy_authorization",
    "refresh_token",
    "set-cookie",
    "set_cookie",
    "user_code",
    "x-api-key",
    "x-auth-token",
    "x_api_key",
    "x_auth_token",
];

const STANDARD_CREDENTIAL_QUERY_PARAMETERS: &[&str] = &[
    "access_token",
    "api_key",
    "apikey",
    "client_assertion",
    "client_secret",
    "code_verifier",
    "device_code",
    "id_token",
    "password",
    "refresh_token",
    "sig",
    "user_code",
    "x-amz-security-token",
    "x-amz-signature",
    "x-goog-signature",
];

/// Remove standard credential fields while preserving every other value exactly.
#[must_use]
pub fn sanitize_standard_credentials(value: &Value) -> Value {
    sanitize_standard_credentials_with_additional_names(value, &BTreeSet::new(), &BTreeSet::new())
}

/// Remove standard and connection-declared credential fields and URL parameters.
#[must_use]
pub fn sanitize_standard_credentials_with_additional_names(
    value: &Value,
    additional_field_names: &BTreeSet<String>,
    additional_query_names: &BTreeSet<String>,
) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let normalized = key.to_ascii_lowercase();
                    let sanitized = if is_credential_name(&normalized, additional_field_names) {
                        Value::String(REDACTED.to_string())
                    } else if is_url_field(&normalized) {
                        value
                            .as_str()
                            .and_then(|raw| sanitize_url_credentials(raw, additional_query_names))
                            .map_or_else(
                                || {
                                    sanitize_standard_credentials_with_additional_names(
                                        value,
                                        additional_field_names,
                                        additional_query_names,
                                    )
                                },
                                |(url, _)| Value::String(url),
                            )
                    } else {
                        sanitize_standard_credentials_with_additional_names(
                            value,
                            additional_field_names,
                            additional_query_names,
                        )
                    };
                    (key.clone(), sanitized)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| {
                    sanitize_standard_credentials_with_additional_names(
                        item,
                        additional_field_names,
                        additional_query_names,
                    )
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

/// Remove URL userinfo and recognized credential query or fragment parameters.
///
/// Returns the cleaned URL and whether any credential-bearing component was
/// removed. Malformed values return `None` rather than being guessed about.
#[must_use]
pub fn sanitize_url_credentials(
    raw_url: &str,
    additional_query_names: &BTreeSet<String>,
) -> Option<(String, bool)> {
    let trimmed = raw_url.trim();
    let mut url = Url::parse(trimmed).ok()?;
    let mut removed = false;

    if !url.username().is_empty() || url.password().is_some() {
        url.set_password(None).ok()?;
        url.set_username("").ok()?;
        removed = true;
    }

    let pairs = url
        .query_pairs()
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let retained = pairs
        .iter()
        .filter(|(name, _)| {
            !is_credential_query_parameter(&name.to_ascii_lowercase(), additional_query_names)
        })
        .cloned()
        .collect::<Vec<_>>();
    if retained.len() != pairs.len() {
        url.set_query(None);
        if !retained.is_empty() {
            url.query_pairs_mut().extend_pairs(retained);
        }
        removed = true;
    }

    if let Some(fragment) = url.fragment().filter(|fragment| fragment.contains('=')) {
        let pairs = url::form_urlencoded::parse(fragment.as_bytes())
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        let retained = pairs
            .iter()
            .filter(|(name, _)| {
                !is_credential_query_parameter(&name.to_ascii_lowercase(), additional_query_names)
            })
            .cloned()
            .collect::<Vec<_>>();
        if retained.len() != pairs.len() {
            let fragment = url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(retained)
                .finish();
            url.set_fragment((!fragment.is_empty()).then_some(fragment.as_str()));
            removed = true;
        }
    }

    Some((
        if removed {
            url.to_string()
        } else {
            trimmed.to_string()
        },
        removed,
    ))
}

fn is_credential_name(name: &str, additional_names: &BTreeSet<String>) -> bool {
    STANDARD_CREDENTIAL_FIELDS.contains(&name) || additional_names.contains(name)
}

fn is_credential_query_parameter(name: &str, additional_query_names: &BTreeSet<String>) -> bool {
    STANDARD_CREDENTIAL_QUERY_PARAMETERS.contains(&name) || additional_query_names.contains(name)
}

fn is_url_field(name: &str) -> bool {
    matches!(name, "final_url" | "location" | "referer" | "url")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exact_standard_fields_are_removed_without_damaging_lookalikes() {
        let sanitized = sanitize_standard_credentials(&json!({
            "Authorization": "Bearer private",
            "access_token": "private",
            "nested": {"PASSWORD": "private"},
            "oauth_authorization_supported": true,
            "authorization_url": "https://example.com/authorize",
            "token_count": 12,
            "cookie_policy": "strict",
            "secret_rotation_status": "current",
            "client_id": "ordinary-client",
            "opaque_id": "p8xQ2zL7wN4vR9mK"
        }));

        assert_eq!(sanitized["Authorization"], REDACTED);
        assert_eq!(sanitized["access_token"], REDACTED);
        assert_eq!(sanitized["nested"]["PASSWORD"], REDACTED);
        assert_eq!(sanitized["oauth_authorization_supported"], true);
        assert_eq!(
            sanitized["authorization_url"],
            "https://example.com/authorize"
        );
        assert_eq!(sanitized["token_count"], 12);
        assert_eq!(sanitized["cookie_policy"], "strict");
        assert_eq!(sanitized["secret_rotation_status"], "current");
        assert_eq!(sanitized["client_id"], "ordinary-client");
        assert_eq!(sanitized["opaque_id"], "p8xQ2zL7wN4vR9mK");
    }

    #[test]
    fn urls_lose_only_standard_credentials() {
        let (url, removed) = sanitize_url_credentials(
            "https://user:password@example.com/path?view=full&access_token=private#section",
            &BTreeSet::new(),
        )
        .expect("URL");

        assert!(removed);
        assert_eq!(url, "https://example.com/path?view=full#section");
        assert_eq!(
            sanitize_url_credentials(
                "https://example.com/callback#access_token=private&state=ordinary",
                &BTreeSet::new(),
            ),
            Some((
                "https://example.com/callback#state=ordinary".to_string(),
                true
            ))
        );
        assert_eq!(
            sanitize_url_credentials(
                "https://example.com/path?token_count=5&code=sample#section",
                &BTreeSet::new(),
            ),
            Some((
                "https://example.com/path?token_count=5&code=sample#section".to_string(),
                false
            ))
        );
    }

    #[test]
    fn connection_declared_names_extend_exact_field_and_url_cleanup() {
        let field_names = BTreeSet::from(["authz".to_string(), "x-custom-credential".to_string()]);
        let query_names = BTreeSet::from(["authz".to_string()]);
        let sanitized = sanitize_standard_credentials_with_additional_names(
            &json!({
                "X-Custom-Credential": "private",
                "x_custom_credential_count": 2,
                "url": "https://example.com/path?view=full&x-custom-credential=ordinary&authz=private#section"
            }),
            &field_names,
            &query_names,
        );

        assert_eq!(sanitized["X-Custom-Credential"], REDACTED);
        assert_eq!(sanitized["x_custom_credential_count"], 2);
        assert_eq!(
            sanitized["url"],
            "https://example.com/path?view=full&x-custom-credential=ordinary#section"
        );
    }
}
