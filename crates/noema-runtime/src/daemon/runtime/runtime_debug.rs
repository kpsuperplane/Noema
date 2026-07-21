use std::time::Instant;

use noema_store::{
    NewRuntimeDebugSpan, NoemaStore, RuntimeDebugMetadata, RuntimeDebugScope,
    RuntimeDebugSpanCategory, RuntimeDebugSpanStatus,
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
        let Some(span_id) = self.span_id else {
            return;
        };
        if let Err(error) = self
            .store
            .finish_runtime_debug_span(
                &span_id,
                status,
                self.started_at
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
                metadata.unwrap_or(self.metadata),
            )
            .await
        {
            eprintln!("runtime debug span finish failed: {error}");
        }
    }
}
