//! Native external URL command and validation.

/// Validate an external URL before opening it in a browser.
///
/// # Errors
///
/// Returns a user-facing error when the URL is invalid or uses an unsupported
/// scheme.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    validate_external_url(&url).map_err(|error| error.to_string())?;
    Ok(())
}

fn validate_external_url(url: &str) -> Result<(), ExternalUrlError> {
    let parsed = url::Url::parse(url).map_err(|_| ExternalUrlError::InvalidUrl)?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        _ => Err(ExternalUrlError::UnsupportedScheme),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ExternalUrlError {
    InvalidUrl,
    UnsupportedScheme,
}

impl std::fmt::Display for ExternalUrlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Noema could not open your browser.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_urls() {
        assert_eq!(validate_external_url("https://example.com"), Ok(()));
        assert_eq!(validate_external_url("http://example.com"), Ok(()));
    }

    #[test]
    fn rejects_non_web_urls() {
        assert_eq!(
            validate_external_url("file:///etc/passwd"),
            Err(ExternalUrlError::UnsupportedScheme)
        );
        assert_eq!(
            validate_external_url("not a url"),
            Err(ExternalUrlError::InvalidUrl)
        );
    }
}
