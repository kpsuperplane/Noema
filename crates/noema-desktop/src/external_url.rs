//! Native external URL command and validation.

const BROWSER_ERROR: &str = "Noema could not open your browser.";

#[tauri::command]
pub(crate) async fn open_external_url(url: String) -> Result<(), String> {
    if !is_external_web_url(&url) {
        return Err(BROWSER_ERROR.to_string());
    }
    open::that_detached(&url).map_err(|_| BROWSER_ERROR.to_string())?;
    Ok(())
}

fn is_external_web_url(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_url_policy_accepts_web_and_rejects_other_schemes() {
        assert!(is_external_web_url("https://example.com"));
        assert!(is_external_web_url("http://example.com"));
        assert!(!is_external_web_url("file:///etc/passwd"));
        assert!(!is_external_web_url("not a url"));
    }
}
