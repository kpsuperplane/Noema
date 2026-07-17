//! Durable-event and active-runtime-status stream merging.

use std::{collections::VecDeque, time::Duration};

use futures_util::stream;

use super::{
    LIFECYCLE_STOPPED, LocalModelManagerError, LocalModelManagerEvent,
    LocalModelManagerEventRecord, LocalModelManagerEventStream, LocalModelManagerService,
};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(250);
const EVENT_BATCH_LIMIT: u32 = 256;

impl LocalModelManagerService {
    /// Subscribes to merged durable and active-runtime events.
    ///
    /// A supplied cursor backfills durable events strictly after its numeric
    /// prefix. With no cursor, the stream begins after the latest durable event.
    /// Runtime events carry that durable prefix plus an ephemeral sequence and
    /// are intentionally not replayed after reconnect.
    ///
    /// # Errors
    ///
    /// Returns [`LocalModelManagerError::InvalidEventCursor`] for malformed
    /// cursors or [`LocalModelManagerError::Persistence`] when initial cursor
    /// discovery fails.
    pub async fn subscribe_events(
        &self,
        after: Option<&str>,
    ) -> Result<LocalModelManagerEventStream, LocalModelManagerError> {
        self.ensure_accepting_work()?;
        let cursor = match after {
            Some(cursor) => parse_cursor(cursor)?,
            None => latest_durable_cursor(self).await?,
        };
        let state = EventStreamState {
            manager: self.clone(),
            runtime_status: self.subscribe_runtime_status(),
            lifecycle: self.inner.lifecycle_tx.subscribe(),
            durable_cursor: cursor,
            runtime_sequence: 0,
            pending: VecDeque::new(),
            last_runtime_event: None,
            terminate_after_pending: false,
        };
        Ok(Box::pin(stream::unfold(state, next_event)))
    }
}

struct EventStreamState {
    manager: LocalModelManagerService,
    runtime_status: tokio::sync::watch::Receiver<crate::LocalModelRuntimeStatus>,
    lifecycle: tokio::sync::watch::Receiver<u8>,
    durable_cursor: u64,
    runtime_sequence: u64,
    pending: VecDeque<LocalModelManagerEventRecord>,
    last_runtime_event: Option<crate::LocalModelRuntimeStatus>,
    terminate_after_pending: bool,
}

async fn next_event(
    mut state: EventStreamState,
) -> Option<(
    Result<LocalModelManagerEventRecord, LocalModelManagerError>,
    EventStreamState,
)> {
    loop {
        if let Some(event) = state.pending.pop_front() {
            return Some((Ok(event), state));
        }
        if state.terminate_after_pending {
            return None;
        }

        if let Err(error) = enqueue_durable_events(&mut state).await {
            state.terminate_after_pending = true;
            return Some((Err(error), state));
        }
        if let Some(event) = state.pending.pop_front() {
            return Some((Ok(event), state));
        }
        if *state.lifecycle.borrow() == LIFECYCLE_STOPPED {
            enqueue_runtime_event(&mut state);
            state.terminate_after_pending = true;
            continue;
        }

        tokio::select! {
            changed = state.runtime_status.changed() => {
                if changed.is_ok() {
                    if let Err(error) = enqueue_durable_events(&mut state).await {
                        state.terminate_after_pending = true;
                        return Some((Err(error), state));
                    }
                    enqueue_runtime_event(&mut state);
                }
            }
            _ = state.lifecycle.changed() => {}
            () = tokio::time::sleep(EVENT_POLL_INTERVAL) => {}
        }
    }
}

fn enqueue_runtime_event(state: &mut EventStreamState) {
    let status = state.runtime_status.borrow().clone();
    if state.last_runtime_event.as_ref() == Some(&status) {
        return;
    }
    state.last_runtime_event = Some(status.clone());
    state.runtime_sequence = state.runtime_sequence.saturating_add(1);
    state.pending.push_back(LocalModelManagerEventRecord {
        cursor: format!(
            "{}:runtime:{}",
            state.durable_cursor, state.runtime_sequence
        ),
        payload: LocalModelManagerEvent::RuntimeChanged { status },
    });
}

async fn enqueue_durable_events(
    state: &mut EventStreamState,
) -> Result<(), LocalModelManagerError> {
    let events = state
        .manager
        .inner
        .installations
        .local_model_events(Some(state.durable_cursor), EVENT_BATCH_LIMIT)
        .await?;
    for event in events {
        state.durable_cursor = event.cursor;
        let installation = state
            .manager
            .inner
            .installations
            .local_model_installation(&event.installation_id)
            .await?;
        state.pending.push_back(LocalModelManagerEventRecord {
            cursor: event.cursor.to_string(),
            payload: LocalModelManagerEvent::Durable {
                event,
                installation: installation.map(Box::new),
            },
        });
    }
    Ok(())
}

fn parse_cursor(cursor: &str) -> Result<u64, LocalModelManagerError> {
    cursor
        .trim()
        .split(':')
        .next()
        .unwrap_or_default()
        .parse()
        .map_err(|_| LocalModelManagerError::InvalidEventCursor)
}

async fn latest_durable_cursor(
    manager: &LocalModelManagerService,
) -> Result<u64, LocalModelManagerError> {
    let mut cursor = 0;
    loop {
        let events = manager
            .inner
            .installations
            .local_model_events(Some(cursor), 1_000)
            .await?;
        let count = events.len();
        if let Some(last) = events.last() {
            cursor = last.cursor;
        }
        if count < 1_000 {
            return Ok(cursor);
        }
    }
}
