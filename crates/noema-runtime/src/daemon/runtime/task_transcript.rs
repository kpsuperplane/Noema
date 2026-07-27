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

use noema_store::{
    RuntimeDebugMetadata, RuntimeDebugScope, RuntimeDebugSpanCategory, RuntimeDebugSpanStatus,
    WorkCommandService, WorkRunFence, WorkRunProgress,
};
use noema_tasks::NewAgentRunItem;

use crate::daemon::{RuntimeError, RuntimeEventRegistry, TaskRuntimeEvent};
use noema_capabilities::CapabilityCatalogSnapshot;
use noema_providers::{
    GenerateRequest, GenerateResponse, GenerateStreamEvent, GenerationPriority, ProviderOperations,
};

use super::{
    actor::RuntimeActor, background_task::BackgroundTaskGenerateRequest,
    runtime_debug::RuntimeDebugSpan, tool_lifecycle::LocalToolCall,
};

impl RuntimeActor {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn generate_task_provider_round(
        &self,
        provider: &dyn ProviderOperations,
        mut request: GenerateRequest,
        bindings: &CapabilityCatalogSnapshot,
        run_id: &str,
        task_id: &str,
        lease_token: &str,
        task_generation: u64,
        contract_id: Option<&noema_tasks::TaskContractId>,
        phase: &'static str,
        round_index: i64,
        deadline: tokio::time::Instant,
        cancellation: &CancellationToken,
        subscriptions: &RuntimeEventRegistry,
    ) -> Result<GenerateResponse, RuntimeError> {
        request.options.generation_priority = GenerationPriority::Background;
        let debug_span = RuntimeDebugSpan::begin(
            &self.store,
            RuntimeDebugScope::AgentRun(run_id.to_string()),
            RuntimeDebugSpanCategory::Provider,
            match phase {
                "initial" => "Initial provider round".to_string(),
                "finalization" => "Final provider round".to_string(),
                _ => format!("Provider continuation {round_index}"),
            },
            RuntimeDebugMetadata {
                phase: Some(phase.to_string()),
                round_index: u64::try_from(round_index).ok(),
                model: request.model.clone(),
                ..RuntimeDebugMetadata::default()
            },
        )
        .await;
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let store = self.store.clone();
        let run_id_for_writer = run_id.to_string();
        let task_id_for_writer = task_id.to_string();
        let fence = WorkRunFence {
            run_id: run_id.to_string(),
            lease_token: lease_token.to_string(),
            task_generation,
            contract_id: contract_id.cloned(),
        };
        let fence_for_writer = fence.clone();
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
                                noema_tasks::AgentRunItemStatus::Running,
                            );
                            if store
                                .upsert_agent_run_item(item, &fence_for_writer)
                                .await
                                .is_ok()
                            {
                                subscriptions_for_writer.publish_task(TaskRuntimeEvent::Changed {
                                    task_id: task_id_for_writer.clone(),
                                    run_id: Some(run_id_for_writer.clone()),
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
                    noema_tasks::AgentRunItemStatus::Completed,
                );
                if store
                    .upsert_agent_run_item(item, &fence_for_writer)
                    .await
                    .is_ok()
                {
                    subscriptions_for_writer.publish_task(TaskRuntimeEvent::Changed {
                        task_id: task_id_for_writer.clone(),
                        run_id: Some(run_id_for_writer.clone()),
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
            _ = cancellation.cancelled() => Err(RuntimeError::Protocol("task execution cancelled".to_string())),
            _ = tokio::time::sleep_until(deadline) => Err(RuntimeError::Protocol("task active wall-time safety ceiling reached".to_string())),
            result = provider.generate_streaming(request, &mut emit) => result.map_err(RuntimeError::Provider),
        };
        drop(event_tx);
        let _ = writer.await;
        let (debug_status, debug_metadata) = match result.as_ref() {
            Ok(response) => {
                let usage = response.usage.as_ref();
                (
                    RuntimeDebugSpanStatus::Completed,
                    RuntimeDebugMetadata {
                        provider: Some(response.provider.clone()),
                        model: Some(response.model.clone()),
                        phase: Some(phase.to_string()),
                        round_index: u64::try_from(round_index).ok(),
                        input_tokens: usage.map(|value| value.input_tokens),
                        cached_input_tokens: usage.and_then(|value| value.cached_input_tokens),
                        output_tokens: usage.map(|value| value.output_tokens),
                        total_tokens: usage.map(|value| value.total_tokens),
                        ..RuntimeDebugMetadata::default()
                    },
                )
            }
            Err(_) => (
                RuntimeDebugSpanStatus::Failed,
                RuntimeDebugMetadata {
                    phase: Some(phase.to_string()),
                    round_index: u64::try_from(round_index).ok(),
                    ..RuntimeDebugMetadata::default()
                },
            ),
        };
        debug_span.finish(debug_status, Some(debug_metadata)).await;
        if let Ok(response) = result.as_ref() {
            let persistence_debug = RuntimeDebugSpan::begin(
                &self.store,
                RuntimeDebugScope::AgentRun(run_id.to_string()),
                RuntimeDebugSpanCategory::Persistence,
                "Persist provider response",
                RuntimeDebugMetadata {
                    phase: Some(phase.to_string()),
                    round_index: u64::try_from(round_index).ok(),
                    ..RuntimeDebugMetadata::default()
                },
            )
            .await;
            let active_milliseconds =
                u64::try_from(provider_started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
            let tool_call_count_delta = u32::try_from(response.tool_calls.len()).map_err(|_| {
                RuntimeError::Protocol(
                    "provider tool-call count exceeds the supported range".to_string(),
                )
            })?;
            WorkCommandService::new(self.store.clone(), self.provider_registry.clone())
                .record_work_run_progress(WorkRunProgress {
                    fence: fence.clone(),
                    actual_provider_kind: Some(response.provider.clone()),
                    actual_model_profile: Some(response.model.clone()),
                    provider_call_count_delta: 1,
                    tool_call_count_delta,
                    input_tokens_delta: response
                        .usage
                        .as_ref()
                        .map_or(0, |usage| usage.input_tokens),
                    cached_input_tokens_delta: response
                        .usage
                        .as_ref()
                        .and_then(|usage| usage.cached_input_tokens)
                        .unwrap_or(0),
                    output_tokens_delta: response
                        .usage
                        .as_ref()
                        .map_or(0, |usage| usage.output_tokens),
                    active_milliseconds_delta: active_milliseconds,
                })
                .await?;
            let duration_ms = i64::try_from(active_milliseconds).unwrap_or(i64::MAX);
            self.store
                .append_agent_run_item(
                    NewAgentRunItem {
                        item_id: Some(format!(
                            "run_item:provider_observation:{run_id}:{round_index}"
                        )),
                        run_id: run_id.to_string(),
                        round_index,
                        kind: noema_tasks::AgentRunItemKind::ProgressNotice,
                        status: noema_tasks::AgentRunItemStatus::Completed,
                        correlation_id: Some(format!("provider:{round_index}")),
                        parent_item_id: None,
                        content_text: Some("Provider response received.".to_string()),
                        payload: serde_json::json!({
                            "phase": "provider_response",
                            "provider": response.provider,
                            "model": response.model,
                            "response_id": response.response_id,
                            "duration_ms": duration_ms,
                            "usage": response.usage.as_ref().map(|usage| serde_json::json!({
                                "input_tokens": usage.input_tokens,
                                "output_tokens": usage.output_tokens,
                                "total_tokens": usage.total_tokens,
                                "cached_input_tokens": usage.cached_input_tokens,
                            })),
                        }),
                    },
                    &fence,
                )
                .await?;
            subscriptions.publish_task(TaskRuntimeEvent::Changed {
                task_id: task_id.to_string(),
                run_id: Some(run_id.to_string()),
            });
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
                            kind: noema_tasks::AgentRunItemKind::AssistantOutput,
                            status: noema_tasks::AgentRunItemStatus::Completed,
                            correlation_id: Some(format!("assistant:{round_index}")),
                            parent_item_id: None,
                            content_text: Some(assistant_text),
                            payload: serde_json::json!({"source": "response"}),
                        },
                        &fence,
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
                        kind: noema_tasks::AgentRunItemKind::ToolCall,
                        status: noema_tasks::AgentRunItemStatus::Running,
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
                    &fence,
                )
                .await;
            }
            persistence_debug
                .finish(RuntimeDebugSpanStatus::Completed, None)
                .await;
        }
        result
    }

    pub(super) async fn persist_task_run_item(
        &self,
        task_id: &str,
        subscriptions: &RuntimeEventRegistry,
        item: NewAgentRunItem,
        fence: &WorkRunFence,
    ) {
        let run_id = item.run_id.clone();
        if self.store.append_agent_run_item(item, fence).await.is_ok() {
            subscriptions.publish_task(TaskRuntimeEvent::Changed {
                task_id: task_id.to_string(),
                run_id: Some(run_id),
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
        let fence = request.work_run_fence();
        for call in calls {
            let correlation_id = call
                .provider_call_id
                .clone()
                .or_else(|| call.call_id.clone())
                .unwrap_or_else(|| format!("output-{}", call.output_index));
            self.persist_task_run_item(
                &request.task_id,
                &request.runtime_events,
                NewAgentRunItem {
                    item_id: Some(format!(
                        "run_item:tool_call:{}:{round_index}:{correlation_id}",
                        request.run_id
                    )),
                    run_id: request.run_id.clone(),
                    round_index,
                    kind: noema_tasks::AgentRunItemKind::ToolCall,
                    status: noema_tasks::AgentRunItemStatus::Skipped,
                    correlation_id: Some(correlation_id),
                    parent_item_id: None,
                    content_text: Some(call.name.clone()),
                    payload: serde_json::json!({
                        "output_index": call.output_index,
                        "reason": reason,
                    }),
                },
                &fence,
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
    status: noema_tasks::AgentRunItemStatus,
) -> NewAgentRunItem {
    NewAgentRunItem {
        item_id: Some(format!(
            "run_item:assistant:{run_id}:{round_index}:{response_index}"
        )),
        run_id: run_id.to_string(),
        round_index,
        kind: noema_tasks::AgentRunItemKind::AssistantOutput,
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
        CapabilityBinding, CapabilityCatalogBuilder, CapabilityExecutionDecision, CapabilityScope,
        CapabilityTarget, CapabilityToolBehavior, InvokerKey, OmitPayloadSanitizer, OperationToken,
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
            CapabilityToolBehavior {
                read_only: true,
                idempotent: true,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            CapabilityScope::Global,
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
            CapabilityToolBehavior {
                read_only: false,
                idempotent: false,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            CapabilityScope::ConversationOwned,
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
