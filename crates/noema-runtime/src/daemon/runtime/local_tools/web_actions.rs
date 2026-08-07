//! Web-action execution and structured URL observation.

use super::*;
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

    pub(in crate::daemon::runtime) async fn execute_approved_web_action(
        &self,
        action: &noema_store::GovernedActionRecord,
    ) -> Option<CapabilityOutput> {
        if action.capability_name == noema_capabilities::web::search::WEB_SEARCH_TOOL {
            let result = match self.web_search_runtime_provider_resolution().await {
                Ok((provider, fallback_from, fallback_reason, auth_failure_target)) => {
                    let mut result = execute_web_search(&provider, None, &action.arguments).await;
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
                    call_id: None,
                    name: noema_capabilities::web::search::WEB_SEARCH_TOOL.to_string(),
                    success: false,
                    payload: json!({ "error": message }),
                },
            };
            if result.success {
                self.record_search_result_urls(&action.action_id, &result.payload)
                    .await;
                Some(CapabilityOutput::success(result.payload))
            } else {
                Some(CapabilityOutput::failed(result.payload))
            }
        } else if action.capability_name == WEB_FETCH_TOOL {
            let priority = if action.task_id.is_some() {
                noema_providers::GenerationPriority::Background
            } else {
                noema_providers::GenerationPriority::Foreground
            };
            let result = match self.web_fetch_runtime_execution_context(priority).await {
                Ok((provider, context, fallback_from, fallback_reason, auth_failure_target)) => {
                    let mut result =
                        execute_web_fetch(&provider, &context, None, &action.arguments).await;
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
                    call_id: None,
                    name: WEB_FETCH_TOOL.to_string(),
                    success: false,
                    payload: json!({ "error": message }),
                },
            };
            if result.success {
                self.record_fetched_link_urls(&action.action_id, &result.payload)
                    .await;
                Some(CapabilityOutput::success(result.payload))
            } else {
                Some(CapabilityOutput::failed(result.payload))
            }
        } else if action.capability_name.starts_with("web.browse.") {
            self.browser_action_previews
                .lock()
                .expect("browser action preview lock")
                .remove(&action.action_id);
            let arguments = action.arguments.clone();
            let owner_key = if let (Some(task_id), Some(generation)) = (
                action.task_id.as_deref(),
                action
                    .authorization_context
                    .get("task_generation")
                    .and_then(Value::as_u64),
            ) {
                format!("task:{task_id}:{generation}")
            } else if let Some(turn_id) = action.turn_id.as_deref() {
                format!("turn:{turn_id}")
            } else {
                return Some(CapabilityOutput::failed(
                    json!({"error":"browser execution authority is unavailable"}),
                ));
            };
            match self
                .execute_web_browse(
                    WebBrowseOwner::new(owner_key.clone()),
                    &action.capability_name,
                    &arguments,
                )
                .await
            {
                Ok(payload) => {
                    self.record_browser_urls(&action.action_id, &payload).await;
                    self.remember_browser_snapshot(&owner_key, &action.capability_name, &payload);
                    Some(CapabilityOutput::success(payload))
                }
                Err(message) => Some(CapabilityOutput::failed(json!({"error":message}))),
            }
        } else {
            None
        }
    }
}
