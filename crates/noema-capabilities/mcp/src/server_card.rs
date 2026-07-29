//! Deterministic MCP service identity discovery from public server cards.

use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
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
    name: Option<String>,
    title: Option<String>,
    description: Option<String>,
    endpoint: Option<String>,
    #[serde(default)]
    endpoints: Vec<ServerCardEndpoint>,
    transport: Option<ServerCardTransport>,
    #[serde(rename = "serverInfo")]
    server_info: Option<ServerCardInfo>,
}

#[derive(Debug, Deserialize)]
struct ServerCardEndpoint {
    url: String,
}

#[derive(Debug, Deserialize)]
struct ServerCardTransport {
    #[serde(rename = "type")]
    kind: String,
    endpoint: String,
}

#[derive(Debug, Deserialize)]
struct ServerCardInfo {
    name: String,
    title: Option<String>,
}

/// Public metadata resolved from a service-owned MCP server card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiscoveredMcpService {
    pub(crate) service_url: String,
    pub(crate) server_card_url: String,
    pub(crate) display_name: String,
    pub(crate) description: Option<String>,
    pub(crate) endpoint_url: String,
}

/// Safe failure classes for service-owned server-card discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum McpServiceCardError {
    #[error("the service URL is invalid")]
    InvalidInput,
    #[error("the MCP server card is unavailable")]
    Unavailable,
    #[error("the MCP server card is unsupported")]
    Unsupported,
}

/// Discover a hosted MCP endpoint from a service's public website.
pub(crate) async fn discover_mcp_service(
    service_url: &str,
    context: &McpRequestContext,
) -> Result<Option<DiscoveredMcpService>, McpServiceCardError> {
    let service = normalized_service_url(service_url).ok_or(McpServiceCardError::InvalidInput)?;
    let mut unavailable = false;
    let mut unsupported = false;
    for candidate in server_card_candidates(&service) {
        match fetch_server_card_body(&candidate, context).await {
            Ok(Some(body)) => {
                if let Some(discovered) = parse_discovered_service(&body, &service, &candidate) {
                    return Ok(Some(discovered));
                }
                unsupported = true;
            }
            Ok(None) => {}
            Err(_) => unavailable = true,
        }
    }
    if unsupported {
        Err(McpServiceCardError::Unsupported)
    } else if unavailable {
        Err(McpServiceCardError::Unavailable)
    } else {
        Ok(None)
    }
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
    fetch_server_card_body(manifest_url, context)
        .await
        .ok()
        .flatten()
        .and_then(|body| server_card_description(&body, manifest_url, endpoint))
}

async fn fetch_server_card_body(
    manifest_url: &Url,
    context: &McpRequestContext,
) -> Result<Option<Vec<u8>>, McpClientError> {
    let fetch = async {
        tokio::time::timeout(SERVER_CARD_TIMEOUT, async {
            let client = restricted_http_client(manifest_url.as_str()).await?;
            let response = client.get(manifest_url.clone()).send().await.map_err(|_| {
                McpClientError::Unavailable("MCP server card is unavailable".to_string())
            })?;
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if response.status() != reqwest::StatusCode::OK {
                return Err(McpClientError::Unavailable(
                    "MCP server card is unavailable".to_string(),
                ));
            }
            bounded_response_body(response, SERVER_CARD_MAX_BYTES)
                .await
                .map(Some)
                .map_err(|error| match error {
                    BodyReadError::Request(_) => {
                        McpClientError::Unavailable("MCP server card is unavailable".to_string())
                    }
                    BodyReadError::Limit => McpClientError::Malformed(
                        "MCP server card exceeded the supported size".to_string(),
                    ),
                })
        })
        .await
        .map_err(|_| McpClientError::Timeout {
            operation: "server_card",
        })?
    };
    run_with_context(context, "server_card", fetch).await
}

fn server_card_candidates(endpoint: &Url) -> Vec<Url> {
    let mut candidates = Vec::with_capacity(3);
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
        if endpoint.host_str() == Some(parent) {
            let mut www_url = endpoint.clone();
            if www_url.set_host(Some(&format!("www.{parent}"))).is_ok() {
                let _ = www_url.set_port(None);
                set_server_card_path(&mut www_url);
                candidates.push(www_url);
            }
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
    if advertised_endpoint(&card, manifest_url).as_ref() != Some(endpoint) {
        return None;
    }
    combine_description(card_title(&card), card.description.as_deref())
}

fn parse_discovered_service(
    body: &[u8],
    service_url: &Url,
    manifest_url: &Url,
) -> Option<DiscoveredMcpService> {
    let card: ServerCard = serde_json::from_slice(body).ok()?;
    let endpoint = advertised_endpoint(&card, manifest_url)?;
    let display_name = card_title(&card)
        .and_then(normalize_field)
        .or_else(|| service_url.host_str().and_then(normalize_field))?;
    let description = card.description.as_deref().and_then(normalize_field);
    Some(DiscoveredMcpService {
        service_url: service_url.as_str().to_string(),
        server_card_url: manifest_url.as_str().to_string(),
        display_name,
        description,
        endpoint_url: endpoint.as_str().to_string(),
    })
}

fn advertised_endpoint(card: &ServerCard, manifest_url: &Url) -> Option<Url> {
    if let Some(endpoint) = card.endpoints.iter().find_map(|candidate| {
        manifest_url
            .join(&candidate.url)
            .ok()
            .filter(|url| parse_https_or_loopback(url.as_str()).is_some())
    }) {
        return Some(endpoint);
    }
    if let Some(endpoint) = card.endpoint.as_deref().and_then(|candidate| {
        manifest_url
            .join(candidate)
            .ok()
            .filter(|url| parse_https_or_loopback(url.as_str()).is_some())
    }) {
        return Some(endpoint);
    }
    let transport = card.transport.as_ref()?;
    if !matches!(
        transport.kind.as_str(),
        "streamable-http" | "streamable_http"
    ) {
        return None;
    }
    manifest_url
        .join(&transport.endpoint)
        .ok()
        .filter(|url| parse_https_or_loopback(url.as_str()).is_some())
}

fn card_title(card: &ServerCard) -> Option<&str> {
    card.title.as_deref().or(card.name.as_deref()).or_else(|| {
        card.server_info
            .as_ref()
            .map(|info| info.title.as_deref().unwrap_or(&info.name))
    })
}

fn normalized_service_url(value: &str) -> Option<Url> {
    let mut url = parse_https_or_loopback(value.trim())?;
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
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
        assert_eq!(
            server_card_candidates(&Url::parse("https://notion.com/").expect("apex"))[1].as_str(),
            "https://www.notion.com/.well-known/mcp.json"
        );
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

    #[test]
    fn discovery_accepts_deployed_and_draft_server_cards() {
        let service = Url::parse("https://notion.com/").expect("service");
        let manifest = Url::parse("https://notion.com/.well-known/mcp.json").expect("manifest");
        let cases = [
            serde_json::json!({
                "name": "Notion",
                "description": "Workspace tools",
                "endpoint": "https://mcp.notion.com/mcp"
            }),
            serde_json::json!({
                "serverInfo": {"name": "notion", "title": "Notion", "version": "1"},
                "transport": {"type": "streamable-http", "endpoint": "https://mcp.notion.com/mcp"},
                "description": "Workspace tools"
            }),
        ];
        for card in cases {
            let discovered = parse_discovered_service(
                &serde_json::to_vec(&card).expect("json"),
                &service,
                &manifest,
            )
            .expect("supported card");
            assert_eq!(discovered.display_name, "Notion");
            assert_eq!(discovered.endpoint_url, "https://mcp.notion.com/mcp");
        }
    }

    #[test]
    fn discovery_rejects_unsupported_or_non_http_transports() {
        let service = Url::parse("https://example.com/").expect("service");
        let manifest = Url::parse("https://example.com/.well-known/mcp.json").expect("manifest");
        for card in [
            serde_json::json!({
                "serverInfo": {"name": "local", "version": "1"},
                "transport": {"type": "stdio", "endpoint": "npx server"}
            }),
            serde_json::json!({
                "title": "Files",
                "endpoints": [{"url": "file:///tmp/mcp.sock"}]
            }),
        ] {
            assert_eq!(
                parse_discovered_service(
                    &serde_json::to_vec(&card).expect("json"),
                    &service,
                    &manifest,
                ),
                None
            );
        }
    }
}
