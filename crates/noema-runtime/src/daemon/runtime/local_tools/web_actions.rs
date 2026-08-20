//! Web-action execution and structured URL observation.

use super::*;
use crate::{
    file_tools::execute_file_download,
    search::tool::{WebSearchToolResult, execute_web_search},
    web_fetch::tool::{WebFetchToolResult, execute_web_fetch},
};
use noema_store::ObservedUrlSource;

pub(super) fn insert_web_tool_fallback_metadata(
    payload: &mut Value,
    fallback_from: Option<&str>,
    fallback_reason: Option<&str>,
) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    if let Some(value) = fallback_from
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        object.insert(
            "fallback_from".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = fallback_reason
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        object.insert(
            "fallback_reason".to_string(),
            Value::String(value.to_string()),
        );
    }
}

pub(super) fn is_provider_account_unauthenticated_payload(payload: &Value) -> bool {
    payload
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(|message| message == PROVIDER_ACCOUNT_UNAUTHENTICATED)
}

impl RuntimeActor {
    pub(in crate::daemon::runtime) async fn validate_browser_call(
        &self,
        owner_key: &str,
        name: &str,
        arguments: &Value,
    ) -> Result<(), WebBrowseError> {
        let command = match parse_command(name, arguments) {
            Ok(BrowseCommand::Interact(request)) => BrowseCommand::Interact(request),
            Ok(BrowseCommand::History(request)) => BrowseCommand::History(request),
            _ => return Ok(()),
        };
        let (provider, _) = self
            .web_browse_runtime_provider_resolution()
            .await
            .map_err(|_| WebBrowseError::Unavailable)?;
        if !provider.has_session(&WebBrowseOwner::new(owner_key)).await {
            return Err(WebBrowseError::SessionNotFound);
        }
        self.validate_browser_snapshot_call(owner_key, command)
    }

    pub(super) fn validate_browser_snapshot_call(
        &self,
        owner_key: &str,
        command: BrowseCommand,
    ) -> Result<(), WebBrowseError> {
        let contexts = self
            .browser_snapshot_contexts
            .lock()
            .expect("browser snapshot context lock");
        let context = contexts
            .get(owner_key)
            .ok_or(WebBrowseError::SessionNotFound)?;
        match command {
            BrowseCommand::Interact(request) => {
                if request.snapshot_revision != context.revision {
                    return Err(WebBrowseError::StaleSnapshot);
                }
                if !context.elements.contains_key(&request.reference) {
                    return Err(WebBrowseError::ElementNotFound);
                }
            }
            BrowseCommand::History(request) if request.snapshot_revision != context.revision => {
                return Err(WebBrowseError::StaleSnapshot);
            }
            BrowseCommand::History(_) => {}
            _ => unreachable!("browser action was filtered above"),
        }
        Ok(())
    }

    pub(super) async fn execute_web_search_action(
        &self,
        call_id: Option<String>,
        arguments: &Value,
        source: &str,
    ) -> CapabilityOutput {
        let result = match self.web_search_runtime_provider_resolution().await {
            Ok((provider, fallback_from, fallback_reason, auth_failure_target)) => {
                let mut result = execute_web_search(&provider, call_id, arguments).await;
                if let Some(target) = auth_failure_target
                    && is_provider_account_unauthenticated_payload(&result.payload)
                {
                    self.mark_provider_account_unauthenticated(&target).await;
                }
                insert_web_tool_fallback_metadata(
                    &mut result.payload,
                    fallback_from.as_deref(),
                    fallback_reason.as_deref(),
                );
                result
            }
            Err(message) => WebSearchToolResult {
                call_id,
                name: noema_capabilities::web::search::WEB_SEARCH_TOOL.to_string(),
                success: false,
                payload: json!({ "error": message }),
            },
        };
        if result.success {
            self.record_search_result_urls(source, &result.payload)
                .await;
            CapabilityOutput::success(result.payload)
        } else {
            CapabilityOutput::failed(result.payload)
        }
    }

    pub(super) async fn execute_web_fetch_action(
        &self,
        priority: noema_providers::GenerationPriority,
        call_id: Option<String>,
        arguments: &Value,
        source: &str,
    ) -> CapabilityOutput {
        let result = match self.web_fetch_runtime_execution_context(priority).await {
            Ok((provider, context, fallback_from, fallback_reason, auth_failure_target)) => {
                let mut result = execute_web_fetch(&provider, &context, call_id, arguments).await;
                if let Some(target) = auth_failure_target
                    && is_provider_account_unauthenticated_payload(&result.payload)
                {
                    self.mark_provider_account_unauthenticated(&target).await;
                }
                insert_web_tool_fallback_metadata(
                    &mut result.payload,
                    fallback_from.as_deref(),
                    fallback_reason.as_deref(),
                );
                result
            }
            Err(message) => WebFetchToolResult {
                call_id,
                name: WEB_FETCH_TOOL.to_string(),
                success: false,
                payload: json!({ "error": message }),
            },
        };
        if result.success {
            self.record_fetched_link_urls(source, &result.payload).await;
            CapabilityOutput::success(result.payload)
        } else {
            CapabilityOutput::failed(result.payload)
        }
    }

    pub(super) async fn execute_web_browse_action(
        &self,
        owner_key: String,
        name: &str,
        arguments: &Value,
        source: &str,
    ) -> CapabilityOutput {
        let result = if name == noema_capabilities::web::browse::WEB_BROWSE_CLOSE_TOOL {
            self.close_browser_session(&owner_key).await
        } else {
            self.execute_web_browse(WebBrowseOwner::new(owner_key.clone()), name, arguments)
                .await
        };
        match result {
            Ok(payload) => {
                self.record_browser_urls(source, &payload).await;
                if name != noema_capabilities::web::browse::WEB_BROWSE_CLOSE_TOOL {
                    self.remember_browser_snapshot(&owner_key, &payload);
                }
                browser_capability_output(payload)
            }
            Err(message) => CapabilityOutput::failed(json!({"error": message})),
        }
    }

    pub(super) async fn record_search_result_urls(&self, source: &str, payload: &Value) {
        let Ok(response) = serde_json::from_value::<noema_capabilities::web::search::SearchResponse>(
            payload.clone(),
        ) else {
            return;
        };
        let urls = response
            .results
            .iter()
            .filter_map(|result| {
                noema_capabilities::web::url_policy::normalize_observed_url(&result.url).ok()
            })
            .collect::<Vec<_>>();
        let _ = self
            .store
            .record_observed_urls(ObservedUrlSource::SearchResult, source, &urls)
            .await;
    }

    pub(in crate::daemon::runtime) async fn record_hosted_web_search_urls(
        &self,
        source: &str,
        searches: &[noema_providers::GenerateHostedWebSearch],
    ) {
        let urls = hosted_web_search_urls(searches);
        let _ = self
            .store
            .record_observed_urls(ObservedUrlSource::SearchResult, source, &urls)
            .await;
    }

    pub(super) async fn record_fetched_link_urls(&self, source: &str, payload: &Value) {
        let Ok(response) = serde_json::from_value::<noema_capabilities::web::fetch::FetchResponse>(
            payload.clone(),
        ) else {
            return;
        };
        let urls = response
            .links
            .iter()
            .filter_map(|url| noema_capabilities::web::url_policy::normalize_observed_url(url).ok())
            .collect::<Vec<_>>();
        let _ = self
            .store
            .record_observed_urls(ObservedUrlSource::FetchedLink, source, &urls)
            .await;
    }

    pub(super) async fn record_browser_urls(&self, source: &str, payload: &Value) {
        let Ok(response) = serde_json::from_value::<noema_capabilities::web::browse::BrowseResponse>(
            payload.clone(),
        ) else {
            return;
        };
        let Some(snapshot) = response.snapshot else {
            return;
        };
        let urls = std::iter::once(snapshot.url)
            .chain(
                snapshot
                    .elements
                    .into_iter()
                    .filter_map(|element| element.href),
            )
            .filter_map(|url| {
                noema_capabilities::web::url_policy::normalize_observed_url(&url).ok()
            })
            .collect::<Vec<_>>();
        let _ = self
            .store
            .record_observed_urls(ObservedUrlSource::BrowserLink, source, &urls)
            .await;
    }

    pub(in crate::daemon::runtime) async fn execute_approved_native_action(
        &self,
        action: &noema_store::GovernedActionRecord,
    ) -> Option<CapabilityOutput> {
        if action.capability_name == noema_capabilities::web::search::WEB_SEARCH_TOOL {
            Some(
                self.execute_web_search_action(None, &action.arguments, &action.action_id)
                    .await,
            )
        } else if action.capability_name == WEB_FETCH_TOOL {
            let priority = if action.task_id.is_some() {
                noema_providers::GenerationPriority::Background
            } else {
                noema_providers::GenerationPriority::Foreground
            };
            Some(
                self.execute_web_fetch_action(priority, None, &action.arguments, &action.action_id)
                    .await,
            )
        } else if action.capability_name == noema_capabilities::file::FILE_DOWNLOAD_TOOL {
            let cwd = match action.conversation_id.as_deref() {
                Some(conversation_id) => self
                    .store
                    .conversation_working_directory(conversation_id, None)
                    .await
                    .ok()
                    .map(|path| path.to_string_lossy().into_owned()),
                None => None,
            };
            let result = execute_file_download(
                &self.store,
                action.task_id.as_deref(),
                cwd.as_deref(),
                &action.arguments,
            )
            .await;
            Some(match result {
                Ok(payload) => CapabilityOutput::success(payload),
                Err(error) => CapabilityOutput::failed(json!({"error": error})),
            })
        } else if action.capability_name.starts_with("web.browse.") {
            let Some(owner_key) = super::browse_owner_key_for_action(action) else {
                return Some(CapabilityOutput::failed(
                    json!({"error":"browser execution authority is unavailable"}),
                ));
            };
            Some(
                self.execute_web_browse_action(
                    owner_key,
                    &action.capability_name,
                    &action.arguments,
                    &action.action_id,
                )
                .await,
            )
        } else {
            None
        }
    }
}

fn browser_capability_output(payload: Value) -> CapabilityOutput {
    if payload.get("screenshot").is_none() {
        return CapabilityOutput::success(payload);
    }
    CapabilityOutput::success(super::browser_model_visible_payload(payload.clone()))
        .with_persisted_output_source(payload)
}

fn hosted_web_search_urls(searches: &[noema_providers::GenerateHostedWebSearch]) -> Vec<String> {
    searches
        .iter()
        .flat_map(|search| &search.sources)
        .filter_map(|source| {
            noema_capabilities::web::url_policy::normalize_observed_url(&source.url).ok()
        })
        .take(256)
        .collect()
}

#[cfg(test)]
mod hosted_search_tests {
    use noema_providers::{GenerateHostedWebSearch, GenerateWebSource};

    use super::*;

    #[test]
    fn browser_screenshot_is_persisted_but_not_model_visible() {
        let output = browser_capability_output(json!({
            "state":"open",
            "screenshot":{"media_type":"image/png","data":"cG5n","width":1280,"height":720}
        }));

        assert_eq!(output.payload, json!({"state":"open"}));
        assert_eq!(
            output.persisted_output_source()["screenshot"]["data"],
            "cG5n"
        );
    }

    #[test]
    fn hosted_search_sources_become_observed_urls() {
        let searches = [GenerateHostedWebSearch {
            output_index: 0,
            id: None,
            tool_name: "web.search".to_string(),
            arguments: json!({"query": "hotel"}),
            result: json!({}),
            status: "completed".to_string(),
            sources: vec![GenerateWebSource {
                title: Some("Hotel".to_string()),
                url: "HTTPS://Example.com:443/booking#rooms".to_string(),
            }],
        }];

        assert_eq!(
            hosted_web_search_urls(&searches),
            ["https://example.com/booking".to_string()]
        );
    }
}
