//! Durable transcript sink for non-interactive task execution rounds.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use tokio_util::sync::CancellationToken;

use crate::{
    GenerateRequest, GenerateResponse,
    capability::gateway::is_mcp_shaped_tool_name,
    daemon::protocol::DaemonError,
    graphql::{ConversationSubscriptionRegistry, TaskLiveEvent},
    provider::GenerateStreamEvent,
    store::NewAgentRunItem,
    web_fetch::tool::{WEB_FETCH_TOOL, sanitize_web_fetch_payload_for_storage},
};

use super::{
    actor::CodexRuntimeActor, background_task::BackgroundTaskGenerateRequest,
    handle::RuntimeModelProvider, tool_lifecycle::LocalToolCall,
};

impl CodexRuntimeActor {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn generate_task_provider_round(
        &self,
        provider: &Arc<dyn RuntimeModelProvider>,
        request: GenerateRequest,
        run_id: &str,
        task_id: &str,
        lease_token: &str,
        round_index: i64,
        deadline: tokio::time::Instant,
        cancellation: &CancellationToken,
        subscriptions: &ConversationSubscriptionRegistry,
    ) -> Result<GenerateResponse, DaemonError> {
        self.persist_task_run_item(
            task_id,
            subscriptions,
            NewAgentRunItem {
                item_id: None,
                run_id: run_id.to_string(),
                round_index,
                kind: "model_input".to_string(),
                status: crate::store::AgentRunItemStatus::Completed,
                correlation_id: None,
                parent_item_id: None,
                content_text: Some(task_model_input_projection(&request.input)),
                payload: serde_json::json!({
                    "conversation_id": request.conversation_id.clone(),
                    "model": request.model.clone(),
                    "has_system_instructions": request.instructions.as_ref().is_some_and(|value| !value.trim().is_empty()),
                    "previous_response_id": request.options.previous_response_id.clone(),
                    "context_mode": if request.options.previous_response_id.is_some() {
                        "response_chain"
                    } else if round_index > 0 && matches!(&request.input, crate::GenerateInput::Items(_)) {
                        "cumulative_replay"
                    } else {
                        "input"
                    },
                }),
            },
            lease_token,
        )
        .await;
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let store = self.store.clone();
        let run_id_for_writer = run_id.to_string();
        let task_id_for_writer = task_id.to_string();
        let lease_token_for_writer = lease_token.to_string();
        let subscriptions_for_writer = subscriptions.clone();
        let saw_assistant_delta = Arc::new(AtomicBool::new(false));
        let saw_assistant_delta_for_emit = Arc::clone(&saw_assistant_delta);
        let writer = tokio::spawn(async move {
            let mut assistant_text = BTreeMap::<usize, String>::new();
            let mut dirty = BTreeSet::<usize>::new();
            let mut flush = tokio::time::interval_at(
                tokio::time::Instant::now() + Duration::from_millis(100),
                Duration::from_millis(100),
            );
            loop {
                tokio::select! {
                    event = event_rx.recv() => {
                        let Some(event) = event else { break; };
                        if let GenerateStreamEvent::AssistantTextDelta { response_index, delta } = event
                            && !delta.is_empty()
                        {
                            assistant_text.entry(response_index).or_default().push_str(&delta);
                            dirty.insert(response_index);
                        }
                    }
                    _ = flush.tick(), if !dirty.is_empty() => {
                        for response_index in std::mem::take(&mut dirty) {
                            let Some(text) = assistant_text.get(&response_index) else { continue; };
                            let item = assistant_run_item(
                                &run_id_for_writer,
                                round_index,
                                response_index,
                                text.clone(),
                                crate::store::AgentRunItemStatus::Running,
                            );
                            if store
                                .upsert_agent_run_item(item, &lease_token_for_writer)
                                .await
                                .is_ok()
                            {
                                subscriptions_for_writer.publish_task(TaskLiveEvent::Changed {
                                    task_id: task_id_for_writer.clone(),
                                });
                            }
                        }
                    }
                }
            }
            for (response_index, text) in assistant_text {
                let item = assistant_run_item(
                    &run_id_for_writer,
                    round_index,
                    response_index,
                    text,
                    crate::store::AgentRunItemStatus::Completed,
                );
                if store
                    .upsert_agent_run_item(item, &lease_token_for_writer)
                    .await
                    .is_ok()
                {
                    subscriptions_for_writer.publish_task(TaskLiveEvent::Changed {
                        task_id: task_id_for_writer.clone(),
                    });
                }
            }
        });
        let mut emit = |event| {
            if matches!(&event, GenerateStreamEvent::AssistantTextDelta { .. }) {
                saw_assistant_delta_for_emit.store(true, Ordering::Relaxed);
            }
            let _ = event_tx.send(event);
        };
        let provider_started_at = Instant::now();
        let result = tokio::select! {
            _ = cancellation.cancelled() => Err(DaemonError::Protocol("task execution cancelled".to_string())),
            _ = tokio::time::sleep_until(deadline) => Err(DaemonError::Protocol("task active wall-time safety ceiling reached".to_string())),
            result = provider.generate_streaming(request, &mut emit) => result.map_err(DaemonError::Provider),
        };
        drop(event_tx);
        let _ = writer.await;
        if let Ok(response) = result.as_ref() {
            self.store
                .record_agent_run_observation(
                    run_id,
                    lease_token,
                    &response.provider,
                    &response.model,
                    response.usage.as_ref(),
                )
                .await?;
            self.store
                .record_agent_run_progress(
                    run_id,
                    lease_token,
                    0,
                    i64::try_from(provider_started_at.elapsed().as_millis()).unwrap_or(i64::MAX),
                )
                .await?;
            if !saw_assistant_delta.load(Ordering::Relaxed) {
                let assistant_text = response.assistant_text();
                if !assistant_text.is_empty() {
                    self.persist_task_run_item(
                        task_id,
                        subscriptions,
                        NewAgentRunItem {
                            item_id: None,
                            run_id: run_id.to_string(),
                            round_index,
                            kind: "assistant_output".to_string(),
                            status: crate::store::AgentRunItemStatus::Completed,
                            correlation_id: Some(format!("assistant:{round_index}")),
                            parent_item_id: None,
                            content_text: Some(assistant_text),
                            payload: serde_json::json!({"source": "response"}),
                        },
                        lease_token,
                    )
                    .await;
                }
            }
            for (output_index, call) in response.tool_calls.iter().enumerate() {
                let arguments = sanitize_task_tool_payload(&call.name, &call.payload);
                self.persist_task_run_item(
                    task_id,
                    subscriptions,
                    NewAgentRunItem {
                        item_id: Some(format!(
                            "run_item:tool_call:{run_id}:{round_index}:{}",
                            call.provider_call_id
                                .as_deref()
                                .or(call.id.as_deref())
                                .map_or_else(|| format!("output-{output_index}"), str::to_string)
                        )),
                        run_id: run_id.to_string(),
                        round_index,
                        kind: "tool_call".to_string(),
                        status: crate::store::AgentRunItemStatus::Running,
                        correlation_id: call.provider_call_id.clone().or(call.id.clone()),
                        parent_item_id: None,
                        content_text: Some(call.name.clone()),
                        payload: serde_json::json!({
                            "output_index": output_index,
                            "id": call.id,
                            "call_id": call.provider_call_id,
                            "provider_name": call.provider_name,
                            "arguments": arguments,
                        }),
                    },
                    lease_token,
                )
                .await;
            }
        }
        result
    }

    pub(super) async fn persist_task_run_item(
        &self,
        task_id: &str,
        subscriptions: &ConversationSubscriptionRegistry,
        item: NewAgentRunItem,
        lease_token: &str,
    ) {
        if self
            .store
            .append_agent_run_item(item, lease_token)
            .await
            .is_ok()
        {
            subscriptions.publish_task(TaskLiveEvent::Changed {
                task_id: task_id.to_string(),
            });
        }
    }

    pub(super) async fn mark_task_calls_skipped(
        &self,
        request: &BackgroundTaskGenerateRequest,
        round_index: i64,
        calls: &[LocalToolCall],
        reason: &str,
    ) {
        for call in calls {
            let correlation_id = call
                .provider_call_id
                .clone()
                .or_else(|| call.call_id.clone())
                .unwrap_or_else(|| format!("output-{}", call.output_index));
            self.persist_task_run_item(
                &request.task_id,
                &request.task_subscriptions,
                NewAgentRunItem {
                    item_id: Some(format!(
                        "run_item:tool_call:{}:{round_index}:{correlation_id}",
                        request.run_id
                    )),
                    run_id: request.run_id.clone(),
                    round_index,
                    kind: "tool_call".to_string(),
                    status: crate::store::AgentRunItemStatus::Skipped,
                    correlation_id: Some(correlation_id),
                    parent_item_id: None,
                    content_text: Some(call.name.clone()),
                    payload: serde_json::json!({
                        "output_index": call.output_index,
                        "reason": reason,
                    }),
                },
                &request.lease_token,
            )
            .await;
        }
    }

    pub(super) async fn persist_context_checkpoint(
        &self,
        request: &BackgroundTaskGenerateRequest,
        round_index: i64,
        summary: &str,
        progress: serde_json::Value,
    ) {
        self.persist_task_run_item(
            &request.task_id,
            &request.task_subscriptions,
            NewAgentRunItem {
                item_id: Some(format!(
                    "run_item:checkpoint:{}:{round_index}",
                    request.run_id
                )),
                run_id: request.run_id.clone(),
                round_index,
                kind: "context_checkpoint".to_string(),
                status: crate::store::AgentRunItemStatus::Completed,
                correlation_id: None,
                parent_item_id: None,
                content_text: Some(summary.to_string()),
                payload: serde_json::json!({"progress": progress}),
            },
            &request.lease_token,
        )
        .await;
    }
}

fn assistant_run_item(
    run_id: &str,
    round_index: i64,
    response_index: usize,
    text: String,
    status: crate::store::AgentRunItemStatus,
) -> NewAgentRunItem {
    NewAgentRunItem {
        item_id: Some(format!(
            "run_item:assistant:{run_id}:{round_index}:{response_index}"
        )),
        run_id: run_id.to_string(),
        round_index,
        kind: "assistant_output".to_string(),
        status,
        correlation_id: Some(format!("assistant:{round_index}:{response_index}")),
        parent_item_id: None,
        content_text: Some(text),
        payload: serde_json::json!({"response_index": response_index}),
    }
}

fn task_model_input_projection(input: &crate::GenerateInput) -> String {
    match input {
        crate::GenerateInput::Text(text) => serde_json::from_str::<serde_json::Value>(text)
            .map(|value| sanitize_evidence_envelope(&value).to_string())
            .unwrap_or_else(|_| text.clone()),
        crate::GenerateInput::Messages(messages) => messages
            .iter()
            .map(|message| format!("{}: {}", message.role.as_str(), message.content))
            .collect::<Vec<_>>()
            .join("\n"),
        crate::GenerateInput::Items(items) => items
            .iter()
            .map(|item| match item {
                crate::provider::GenerateInputItem::Message(message) => {
                    format!("{}: {}", message.role.as_str(), message.content)
                }
                crate::provider::GenerateInputItem::Reasoning(_) => {
                    "[encrypted reasoning omitted]".to_string()
                }
                crate::provider::GenerateInputItem::ToolCall(call) => serde_json::json!({
                    "type": "tool_call",
                    "call_id": call.call_id,
                    "name": call.name,
                    "arguments": sanitize_task_tool_payload(&call.name, &call.arguments),
                })
                .to_string(),
                crate::provider::GenerateInputItem::ToolResult(result) => serde_json::json!({
                    "type": "tool_result",
                    "call_id": result.call_id,
                    "name": result.name,
                    "success": result.success,
                    "arguments": sanitize_task_tool_payload(&result.name, &result.arguments),
                    "payload": sanitize_task_tool_payload(&result.name, &result.payload),
                })
                .to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        crate::GenerateInput::NativeToolResults(results) => serde_json::Value::Array(
            results
                .iter()
                .map(|result| {
                    serde_json::json!({
                        "call_id": result.call_id,
                        "name": result.name,
                        "success": result.success,
                        "arguments": sanitize_task_tool_payload(&result.name, &result.arguments),
                        "payload": sanitize_task_tool_payload(&result.name, &result.payload),
                    })
                })
                .collect(),
        )
        .to_string(),
    }
}

fn sanitize_evidence_envelope(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(name) = object.get("name").and_then(serde_json::Value::as_str) {
                let mut sanitized = object.clone();
                for key in ["arguments", "payload"] {
                    if let Some(value) = object.get(key) {
                        sanitized.insert(key.to_string(), sanitize_task_tool_payload(name, value));
                    }
                }
                return serde_json::Value::Object(sanitized);
            }
            serde_json::Value::Object(
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), sanitize_evidence_envelope(value)))
                    .collect(),
            )
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(sanitize_evidence_envelope).collect())
        }
        _ => value.clone(),
    }
}

pub(super) fn sanitize_task_tool_payload(
    name: &str,
    payload: &serde_json::Value,
) -> serde_json::Value {
    if name == WEB_FETCH_TOOL {
        return sanitize_web_fetch_payload_for_storage(payload);
    }
    if name == "artifact.create_local_file" {
        let mut sanitized = payload.clone();
        if let Some(versions) = sanitized
            .get_mut("versions")
            .and_then(serde_json::Value::as_array_mut)
        {
            for version in versions {
                if let Some(object) = version.as_object_mut()
                    && let Some(content) = object.get_mut("content")
                {
                    let chars = content.as_str().map(str::chars).map(Iterator::count);
                    *content = serde_json::json!({
                        "omitted": true,
                        "character_count": chars,
                    });
                }
            }
        }
        return redact_secret_fields(&sanitized);
    }
    if is_mcp_shaped_tool_name(name) {
        return serde_json::json!({
            "redacted": true,
            "reason": "mcp_payload",
        });
    }
    redact_secret_fields(payload)
}

fn redact_secret_fields(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let normalized = key.to_ascii_lowercase();
                    let sensitive = [
                        "authorization",
                        "api_key",
                        "apikey",
                        "access_token",
                        "refresh_token",
                        "password",
                        "secret",
                        "cookie",
                    ]
                    .iter()
                    .any(|needle| normalized.contains(needle));
                    (
                        key.clone(),
                        if sensitive {
                            serde_json::Value::String("[REDACTED]".to_string())
                        } else {
                            redact_secret_fields(value)
                        },
                    )
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(redact_secret_fields).collect())
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_transcript_redacts_secret_fields_recursively() {
        let sanitized = sanitize_task_tool_payload(
            "web.search",
            &serde_json::json!({
                "query": "safe",
                "headers": {"Authorization": "Bearer private"},
                "nested": [{"api_key": "private"}],
            }),
        );
        assert_eq!(sanitized["query"], "safe");
        assert_eq!(sanitized["headers"]["Authorization"], "[REDACTED]");
        assert_eq!(sanitized["nested"][0]["api_key"], "[REDACTED]");
        assert!(!sanitized.to_string().contains("private"));
    }

    #[test]
    fn task_transcript_does_not_persist_mcp_payloads() {
        let sanitized = sanitize_task_tool_payload(
            "mcp.mcp:notion.search",
            &serde_json::json!({"query": "private workspace query"}),
        );
        assert_eq!(sanitized["redacted"], true);
        assert!(!sanitized.to_string().contains("workspace"));
    }

    #[test]
    fn task_transcript_omits_artifact_file_contents() {
        let sanitized = sanitize_task_tool_payload(
            "artifact.create_local_file",
            &serde_json::json!({
                "title": "Report",
                "versions": [{"content": "private report body"}],
            }),
        );
        assert_eq!(sanitized["title"], "Report");
        assert_eq!(sanitized["versions"][0]["content"]["omitted"], true);
        assert!(!sanitized.to_string().contains("private report body"));
    }

    #[test]
    fn task_model_input_projection_redacts_native_mcp_results() {
        let input = crate::GenerateInput::NativeToolResults(vec![
            crate::provider::GenerateToolResultInput {
                id: None,
                call_id: "call-1".to_string(),
                name: "mcp.mcp:notion.search".to_string(),
                provider_name: None,
                arguments: serde_json::json!({"query": "private workspace query"}),
                success: true,
                payload: serde_json::json!({"access_token": "private-token", "result": "private page"}),
            },
        ]);
        let projection = task_model_input_projection(&input);
        assert!(!projection.contains("private"));
        assert!(projection.contains("mcp_payload"));
    }
}
