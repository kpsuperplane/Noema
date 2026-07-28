//! Deterministic MCP service identity discovery from public server cards.

use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use url::{Host, Url};

use crate::{
    McpStreamableHttpSetupConfig, McpTransportKind,
    client::{McpClientError, McpRequestContext, run_with_context},
    connection_url::parse_https_or_loopback,
    http::restricted_http_client,
    http_body::{BodyReadError, bounded_response_body},
};

const SERVER_CARD_PATH: &str = "/.well-known/mcp.json";
const SERVER_CARD_MAX_BYTES: usize = 64 * 1024;
const SERVER_CARD_TIMEOUT: Duration = Duration::from_secs(2);
const SERVICE_DESCRIPTION_MAX_CHARS: usize = 192;

#[derive(Debug, Deserialize)]
struct ServerCard {
    title: Option<String>,
    description: Option<String>,
    #[serde(default)]
    endpoints: Vec<ServerCardEndpoint>,
}

#[derive(Debug, Deserialize)]
struct ServerCardEndpoint {
    url: String,
}

/// Discover one model-facing service description from the public MCP server card.
pub(crate) async fn discover_service_description(
    transport_kind: McpTransportKind,
    safe_config: &Value,
    context: &McpRequestContext,
) -> Option<String> {
    if transport_kind != McpTransportKind::StreamableHttp {
        return None;
    }
    let config: McpStreamableHttpSetupConfig = serde_json::from_value(safe_config.clone()).ok()?;
    let endpoint = parse_https_or_loopback(&config.url)?;
    for candidate in server_card_candidates(&endpoint) {
        let description = fetch_server_card(&candidate, &endpoint, context).await;
        if description.is_some() {
            return description;
        }
    }
    None
}

async fn fetch_server_card(
    manifest_url: &Url,
    endpoint: &Url,
    context: &McpRequestContext,
) -> Option<String> {
    let fetch = async {
        tokio::time::timeout(SERVER_CARD_TIMEOUT, async {
            let client = restricted_http_client(manifest_url.as_str()).await?;
            let response = client.get(manifest_url.clone()).send().await.map_err(|_| {
                McpClientError::Unavailable("MCP server card is unavailable".to_string())
            })?;
            if response.status() != reqwest::StatusCode::OK {
                return Ok(None);
            }
            let body = bounded_response_body(response, SERVER_CARD_MAX_BYTES)
                .await
                .map_err(|error| match error {
                    BodyReadError::Request(_) => {
                        McpClientError::Unavailable("MCP server card is unavailable".to_string())
                    }
                    BodyReadError::Limit => McpClientError::Malformed(
                        "MCP server card exceeded the supported size".to_string(),
                    ),
                })?;
            Ok(server_card_description(&body, manifest_url, endpoint))
        })
        .await
        .map_err(|_| McpClientError::Timeout {
            operation: "server_card",
        })?
    };
    run_with_context(context, "server_card", fetch)
        .await
        .ok()
        .flatten()
}

fn server_card_candidates(endpoint: &Url) -> Vec<Url> {
    let mut candidates = Vec::with_capacity(2);
    if let Some(parent) = endpoint.host().and_then(|host| match host {
        Host::Domain(domain) => psl::domain_str(domain),
        Host::Ipv4(_) | Host::Ipv6(_) => None,
    }) {
        let mut parent_url = endpoint.clone();
        if parent_url.set_host(Some(parent)).is_ok() {
            let _ = parent_url.set_port(None);
            set_server_card_path(&mut parent_url);
            candidates.push(parent_url);
        }
    }
    let mut origin_url = endpoint.clone();
    set_server_card_path(&mut origin_url);
    if !candidates.contains(&origin_url) {
        candidates.push(origin_url);
    }
    candidates
}

fn set_server_card_path(url: &mut Url) {
    url.set_path(SERVER_CARD_PATH);
    url.set_query(None);
    url.set_fragment(None);
}

fn server_card_description(body: &[u8], manifest_url: &Url, endpoint: &Url) -> Option<String> {
    let card: ServerCard = serde_json::from_slice(body).ok()?;
    if !card.endpoints.iter().any(|candidate| {
        manifest_url
            .join(&candidate.url)
            .is_ok_and(|candidate| candidate == *endpoint)
    }) {
        return None;
    }
    combine_description(card.title.as_deref(), card.description.as_deref())
}

fn combine_description(title: Option<&str>, description: Option<&str>) -> Option<String> {
    let title = title.and_then(normalize_field);
    let description = description.and_then(normalize_field);
    let combined = match (title, description) {
        (Some(title), Some(description)) => format!("{title}: {description}"),
        (Some(title), None) => title,
        (None, Some(description)) => description,
        (None, None) => return None,
    };
    Some(bound_chars(combined, SERVICE_DESCRIPTION_MAX_CHARS))
}

fn normalize_field(value: &str) -> Option<String> {
    let normalized = value
        .chars()
        .filter(|character| !character.is_control() || character.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!normalized.is_empty()).then_some(normalized)
}

fn bound_chars(value: String, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value;
    }
    let mut bounded = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    bounded.push_str("...");
    bounded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_first_candidates_and_optional_fields_are_deterministic() {
        let endpoint = Url::parse("https://mcp.getdex.com/mcp").expect("endpoint");
        let candidates = server_card_candidates(&endpoint);
        assert_eq!(
            candidates.iter().map(Url::as_str).collect::<Vec<_>>(),
            vec![
                "https://getdex.com/.well-known/mcp.json",
                "https://mcp.getdex.com/.well-known/mcp.json",
            ]
        );
        assert_eq!(
            combine_description(Some(" Dex Personal CRM "), Some(" Search contacts. ")).as_deref(),
            Some("Dex Personal CRM: Search contacts.")
        );
        assert_eq!(
            combine_description(None, Some(" Search contacts. ")).as_deref(),
            Some("Search contacts.")
        );
        assert_eq!(
            combine_description(Some("Dex"), None).as_deref(),
            Some("Dex")
        );
        assert_eq!(combine_description(Some("  "), None), None);
    }

    #[test]
    fn manifest_requires_exact_endpoint_and_bounds_normalized_text() {
        let manifest = Url::parse("https://getdex.com/.well-known/mcp.json").expect("manifest");
        let endpoint = Url::parse("https://mcp.getdex.com/mcp").expect("endpoint");
        let body = serde_json::json!({
            "title": "Dex Personal CRM",
            "description": format!("Contacts {}", "x".repeat(300)),
            "endpoints": [{"url": endpoint.as_str()}]
        });
        let description = server_card_description(
            &serde_json::to_vec(&body).expect("json"),
            &manifest,
            &endpoint,
        )
        .expect("description");
        assert_eq!(description.chars().count(), SERVICE_DESCRIPTION_MAX_CHARS);
        assert!(description.starts_with("Dex Personal CRM: Contacts"));

        let mismatch = serde_json::json!({
            "title": "Wrong",
            "endpoints": [{"url": "https://other.example/mcp"}]
        });
        assert_eq!(
            server_card_description(
                &serde_json::to_vec(&mismatch).expect("json"),
                &manifest,
                &endpoint,
            ),
            None
        );
        assert_eq!(
            combine_description(Some("Dex\nPersonal\u{0} CRM"), None).as_deref(),
            Some("Dex Personal CRM")
        );
    }
}
