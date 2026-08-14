use std::{collections::BTreeMap, time::Instant};

use noema_providers::{GenerateStreamEvent, ProviderTimingMilestone};
use noema_store::{
    NewRuntimeDebugSpan, NoemaStore, RuntimeDebugChildSpan, RuntimeDebugMetadata,
    RuntimeDebugScope, RuntimeDebugSpanCategory, RuntimeDebugSpanStatus,
};

pub(super) struct RuntimeDebugSpan {
    store: NoemaStore,
    span_id: Option<String>,
    started_at: Instant,
    metadata: RuntimeDebugMetadata,
}

impl RuntimeDebugSpan {
    pub(super) async fn begin(
        store: &NoemaStore,
        scope: RuntimeDebugScope,
        category: RuntimeDebugSpanCategory,
        name: impl Into<String>,
        metadata: RuntimeDebugMetadata,
    ) -> Self {
        let span_id = store
            .begin_runtime_debug_span(NewRuntimeDebugSpan {
                scope,
                category,
                name: name.into(),
                metadata: metadata.clone(),
            })
            .await
            .map_err(|error| eprintln!("runtime debug span start failed: {error}"))
            .ok();
        Self {
            store: store.clone(),
            span_id,
            started_at: Instant::now(),
            metadata,
        }
    }

    pub(super) async fn finish(
        self,
        status: RuntimeDebugSpanStatus,
        metadata: Option<RuntimeDebugMetadata>,
    ) {
        self.finish_with_children(status, metadata, &[]).await;
    }

    pub(super) async fn finish_with_children(
        self,
        status: RuntimeDebugSpanStatus,
        metadata: Option<RuntimeDebugMetadata>,
        children: &[RuntimeDebugChildSpan],
    ) {
        let Some(span_id) = self.span_id else {
            return;
        };
        if let Err(error) = self
            .store
            .finish_runtime_debug_span_with_children(
                &span_id,
                status,
                self.started_at
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
                metadata.unwrap_or(self.metadata),
                children,
            )
            .await
        {
            eprintln!("runtime debug span finish failed: {error}");
        }
    }
}

#[derive(Debug, Default)]
struct HostedSearchTimeline {
    started: Option<u64>,
    searching: Option<u64>,
    completed: Option<u64>,
    correlation_id: Option<String>,
}

pub(super) struct ProviderDebugTimeline {
    started_at: Instant,
    response_headers: Option<u64>,
    response_body: Option<u64>,
    first_assistant_text: Option<u64>,
    searches: BTreeMap<usize, HostedSearchTimeline>,
}

impl ProviderDebugTimeline {
    pub(super) fn begin() -> Self {
        Self {
            started_at: Instant::now(),
            response_headers: None,
            response_body: None,
            first_assistant_text: None,
            searches: BTreeMap::new(),
        }
    }

    pub(super) fn observe(&mut self, event: &GenerateStreamEvent) {
        let elapsed = elapsed_milliseconds(self.started_at);
        match event {
            GenerateStreamEvent::AssistantTextDelta { .. } => {
                self.first_assistant_text.get_or_insert(elapsed);
            }
            GenerateStreamEvent::HostedWebSearchStarted { output_index, id } => {
                let search = self.searches.entry(*output_index).or_default();
                search.started.get_or_insert(elapsed);
                if search.correlation_id.is_none() {
                    search.correlation_id.clone_from(id);
                }
            }
            GenerateStreamEvent::ProviderTiming {
                milestone,
                output_index,
            } => match milestone {
                ProviderTimingMilestone::ResponseHeaders => {
                    self.response_headers.get_or_insert(elapsed);
                }
                ProviderTimingMilestone::ResponseBodyStarted => {
                    self.response_body.get_or_insert(elapsed);
                }
                ProviderTimingMilestone::HostedWebSearchSearching => {
                    if let Some(output_index) = output_index {
                        self.searches
                            .entry(*output_index)
                            .or_default()
                            .searching
                            .get_or_insert(elapsed);
                    }
                }
                ProviderTimingMilestone::HostedWebSearchCompleted => {
                    if let Some(output_index) = output_index {
                        self.searches
                            .entry(*output_index)
                            .or_default()
                            .completed
                            .get_or_insert(elapsed);
                    }
                }
            },
            GenerateStreamEvent::ToolCallStarted { .. } => {}
        }
    }

    pub(super) fn completed_spans(&self) -> Vec<RuntimeDebugChildSpan> {
        let completed = elapsed_milliseconds(self.started_at);
        let mut spans = Vec::new();
        if let Some(headers) = self.response_headers {
            push_span(&mut spans, "Await response headers", 0, headers, None);
        }
        if let (Some(headers), Some(body)) = (self.response_headers, self.response_body) {
            push_span(&mut spans, "Await response body", headers, body, None);
        }
        if let (Some(body), Some(search_started)) = (
            self.response_body,
            self.searches
                .values()
                .filter_map(|search| search.started)
                .min(),
        ) {
            push_span(
                &mut spans,
                "Before hosted web search",
                body,
                search_started,
                None,
            );
        }
        for (output_index, search) in &self.searches {
            let metadata = Some(RuntimeDebugMetadata {
                tool_name: Some("web.search".to_string()),
                correlation_id: search.correlation_id.clone(),
                response_index: (*output_index).try_into().ok(),
                ..RuntimeDebugMetadata::default()
            });
            if let (Some(started), Some(searching)) = (search.started, search.searching) {
                push_span(
                    &mut spans,
                    "Prepare hosted web search",
                    started,
                    searching,
                    metadata.clone(),
                );
            }
            if let Some(completed) = search.completed {
                let started = search.searching.or(search.started).unwrap_or(completed);
                push_span(
                    &mut spans,
                    "Hosted web search",
                    started,
                    completed,
                    metadata,
                );
            }
        }
        if let Some(first_text) = self.first_assistant_text {
            let prior = self
                .searches
                .values()
                .filter_map(|search| search.completed)
                .max()
                .or(self.response_body)
                .or(self.response_headers)
                .unwrap_or(0);
            push_span(
                &mut spans,
                "Before first assistant text",
                prior,
                first_text,
                None,
            );
            push_span(
                &mut spans,
                "Assistant text stream",
                first_text,
                completed,
                None,
            );
        }
        spans
    }
}

fn elapsed_milliseconds(started_at: Instant) -> u64 {
    started_at
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn push_span(
    spans: &mut Vec<RuntimeDebugChildSpan>,
    name: &str,
    started: u64,
    ended: u64,
    metadata: Option<RuntimeDebugMetadata>,
) {
    if ended < started {
        return;
    }
    spans.push(RuntimeDebugChildSpan {
        name: name.to_string(),
        start_offset_milliseconds: started,
        duration_milliseconds: ended - started,
        metadata: metadata.unwrap_or_default(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_timeline_projects_search_boundaries_without_content() {
        let mut timeline = ProviderDebugTimeline::begin();
        for event in [
            GenerateStreamEvent::ProviderTiming {
                milestone: ProviderTimingMilestone::ResponseHeaders,
                output_index: None,
            },
            GenerateStreamEvent::ProviderTiming {
                milestone: ProviderTimingMilestone::ResponseBodyStarted,
                output_index: None,
            },
            GenerateStreamEvent::HostedWebSearchStarted {
                output_index: 2,
                id: Some("search-item".to_string()),
            },
            GenerateStreamEvent::ProviderTiming {
                milestone: ProviderTimingMilestone::HostedWebSearchSearching,
                output_index: Some(2),
            },
            GenerateStreamEvent::ProviderTiming {
                milestone: ProviderTimingMilestone::HostedWebSearchCompleted,
                output_index: Some(2),
            },
            GenerateStreamEvent::AssistantTextDelta {
                response_index: 0,
                delta: "private response text".to_string(),
            },
        ] {
            timeline.observe(&event);
        }
        let spans = timeline.completed_spans();
        assert!(spans.iter().any(|span| span.name == "Hosted web search"));
        let serialized =
            serde_json::to_string(&spans.iter().map(|span| &span.metadata).collect::<Vec<_>>())
                .expect("metadata JSON");
        assert!(!serialized.contains("private response text"));
    }
}
