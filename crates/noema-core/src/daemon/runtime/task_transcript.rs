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
    daemon::protocol::DaemonError,
    graphql::{ConversationSubscriptionRegistry, TaskLiveEvent},
    store::NewAgentRunItem,
};
use noema_capabilities::CapabilityCatalogSnapshot;
use noema_providers::{
    GenerateRequest, GenerateResponse, GenerateStreamEvent, GenerationPriority, ProviderHandle,
};

use super::{
    actor::CodexRuntimeActor, background_task::BackgroundTaskGenerateRequest,
    tool_lifecycle::LocalToolCall,
};

impl CodexRuntimeActor {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn generate_task_provider_round(
        &self,
        provider: &ProviderHandle,
        mut request: GenerateRequest,
        bindings: &CapabilityCatalogSnapshot,
        run_id: &str,
        task_id: &str,
        lease_token: &str,
        round_index: i64,
        deadline: tokio::time::Instant,
        cancellation: &CancellationToken,
        subscriptions: &ConversationSubscriptionRegistry,
    ) -> Result<GenerateResponse, DaemonError> {
        request.options.generation_priority = GenerationPriority::Background;
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
                let arguments = persisted_capability_arguments(bindings, &call.name, &call.payload);
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

pub(super) fn persisted_capability_arguments(
    bindings: &CapabilityCatalogSnapshot,
    name: &str,
    payload: &serde_json::Value,
) -> serde_json::Value {
    bindings
        .resolve(name)
        .and_then(|binding| binding.persist_arguments(payload))
        .unwrap_or_else(omitted_capability_payload)
}

pub(super) fn omitted_capability_payload() -> serde_json::Value {
    serde_json::json!({
        "redacted": true,
        "reason": "capability_persistence_policy",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_capabilities::{
        CapabilityAccess, CapabilityBinding, CapabilityCatalogBuilder, CapabilityEffect,
        CapabilityScope, CapabilityTarget, InvokerKey, OmitPayloadSanitizer, OperationToken,
        RedactingPayloadSanitizer, ToolSpec,
    };
    use std::sync::Arc;

    fn bindings(name: &str, omit: bool) -> CapabilityCatalogSnapshot {
        let spec =
            ToolSpec::new(name, "Test", serde_json::json!({"type": "object"})).expect("spec");
        let sanitizer: Arc<dyn noema_capabilities::PayloadSanitizer> = if omit {
            Arc::new(OmitPayloadSanitizer)
        } else {
            Arc::new(RedactingPayloadSanitizer)
        };
        let binding = CapabilityBinding::new(
            spec,
            CapabilityTarget::new(InvokerKey::new("test"), OperationToken::new("test")),
            CapabilityAccess {
                effect: CapabilityEffect::ReadOnly,
                scope: CapabilityScope::Global,
            },
            sanitizer,
        );
        let mut builder = CapabilityCatalogBuilder::new();
        builder.add(binding).expect("unique binding");
        builder.build()
    }

    #[test]
    fn task_transcript_redacts_secret_fields_recursively() {
        let bindings = bindings("web.search", false);
        let sanitized = persisted_capability_arguments(
            &bindings,
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
    fn task_transcript_uses_binding_policy_after_mcp_rename() {
        let bindings = bindings("workspace.lookup", true);
        let sanitized = persisted_capability_arguments(
            &bindings,
            "workspace.lookup",
            &serde_json::json!({"query": "private workspace query"}),
        );
        assert_eq!(sanitized["redacted"], true);
        assert_eq!(sanitized["reason"], "capability_persistence_policy");
        assert!(!sanitized.to_string().contains("workspace"));
    }

    #[test]
    fn task_transcript_omits_artifact_file_contents() {
        let spec = ToolSpec::new(
            "artifact.create_local_file",
            "Create an artifact.",
            serde_json::json!({"type": "object"}),
        )
        .expect("spec");
        let binding = CapabilityBinding::new(
            spec,
            CapabilityTarget::new(
                InvokerKey::new("test"),
                OperationToken::new("artifact.create_local_file"),
            ),
            CapabilityAccess {
                effect: CapabilityEffect::Mutating,
                scope: CapabilityScope::ConversationOwned,
            },
            Arc::new(noema_capabilities::ArtifactPayloadSanitizer),
        );
        let mut builder = CapabilityCatalogBuilder::new();
        builder.add(binding).expect("unique binding");
        let bindings = builder.build();
        let sanitized = persisted_capability_arguments(
            &bindings,
            "artifact.create_local_file",
            &serde_json::json!({
                "arguments": {
                    "filename": "private.md",
                    "title": "Safe title",
                    "api_key": "private secret",
                    "versions": [{"title": "Draft", "content": "private artifact body"}]
                }
            }),
        );

        assert_eq!(sanitized["arguments"]["filename"], "private.md");
        assert_eq!(sanitized["arguments"]["title"], "Safe title");
        assert_eq!(sanitized["arguments"]["versions"][0]["title"], "Draft");
        assert_eq!(
            sanitized["arguments"]["versions"][0]["content"],
            serde_json::json!({"omitted": true, "character_count": 21})
        );
        assert_eq!(sanitized["arguments"]["api_key"], "[REDACTED]");
        assert!(!sanitized.to_string().contains("private artifact body"));
        assert!(!sanitized.to_string().contains("private secret"));
    }

    #[test]
    fn unknown_tool_arguments_are_omitted_without_inspection() {
        let sanitized = persisted_capability_arguments(
            &CapabilityCatalogSnapshot::default(),
            "forged.tool",
            &serde_json::json!({"private": "must not persist"}),
        );
        assert_eq!(sanitized, omitted_capability_payload());
        assert!(!sanitized.to_string().contains("must not persist"));
    }
}
