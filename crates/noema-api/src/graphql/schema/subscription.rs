/// Root GraphQL subscription object.
use super::*;

pub struct SubscriptionRoot;

/// Durable task subscription event category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, async_graphql::Enum)]
#[graphql(name = "TaskEventKind")]
pub enum GraphqlTaskEventKind {
    /// Task lifecycle/read-model state changed.
    TaskUpdated,
    /// One durable task run changed.
    RunUpdated,
    /// One task-run transcript item was inserted or updated.
    RunItemUpserted,
}

/// One durable owner-authorized task event.
#[derive(Clone, Debug, async_graphql::SimpleObject)]
#[graphql(name = "TaskEvent")]
pub struct GraphqlTaskEvent {
    /// Exclusive cursor for reconnect/backfill.
    pub cursor: String,
    /// Typed event category.
    pub kind: GraphqlTaskEventKind,
    /// Task whose durable projection changed.
    pub task_id: String,
    /// Run affected by the event, when applicable.
    pub run_id: Option<String>,
    /// Current task or run status, when applicable.
    pub status: Option<String>,
    /// Appended or updated transcript item, when applicable.
    pub item: Option<tasks::GraphqlTaskRunItem>,
    /// Current durable run projection, when the event is associated with a run.
    pub run: Option<tasks::GraphqlTaskRun>,
    /// Durable event creation timestamp.
    pub created_at: String,
}

#[Subscription]
impl SubscriptionRoot {
    /// Stream cursor-bearing local-model transfer, selection, and runtime events.
    async fn local_model_events(
        &self,
        _ctx: &Context<'_>,
        after: Option<String>,
    ) -> Result<impl Stream<Item = Result<GraphqlLocalModelEvent>>> {
        let state = _ctx.data_unchecked::<GraphqlState>();
        local_models::local_model_events(state, after).await
    }

    #[cfg(any(test, feature = "test-support"))]
    async fn test_request_principal(&self, ctx: &Context<'_>) -> impl Stream<Item = String> {
        futures_util::stream::once(std::future::ready(
            ctx.data_unchecked::<crate::graphql::RequestPrincipal>()
                .subject_id()
                .to_string(),
        ))
    }

    /// Stream conversation events.
    async fn conversation_events(
        &self,
        ctx: &Context<'_>,
        conversation_id: String,
    ) -> Result<impl Stream<Item = GraphqlConversationEvent>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        chat::conversation_events(state, principal, conversation_id).await
    }

    /// Stream owner-authorized durable task updates with reconnect backfill.
    async fn task_events(
        &self,
        ctx: &Context<'_>,
        task_id: String,
        after: Option<String>,
    ) -> Result<impl Stream<Item = Result<GraphqlTaskEvent>>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        let store = state.store()?.clone();
        let task_id = task_id.trim().to_string();
        let is_authorized = store
            .get_task(&task_id)
            .await
            .map_err(crate::graphql::errors::graphql_error)?
            .is_some_and(|task| task.owner_human_id == principal);
        if !is_authorized {
            return Err(async_graphql::Error::new("task is unavailable"));
        }
        let mut cursor = match after {
            Some(cursor) => parse_task_event_cursor(&cursor)?,
            None => store
                .latest_task_event_sequence(&task_id)
                .await
                .map_err(crate::graphql::errors::graphql_error)?,
        };
        let mut rx = state.subscriptions().subscribe_task(&task_id);
        Ok(async_stream::stream! {
            loop {
                let events = match store.list_task_events_after(&task_id, Some(cursor), 256).await {
                    Ok(events) => events,
                    Err(error) => {
                        yield Err(crate::graphql::errors::graphql_error(error));
                        break;
                    }
                };
                if !events.is_empty() {
                    for event in events {
                        cursor = event.sequence_number;
                        yield project_task_event(&store, event).await;
                    }
                    continue;
                }
                match rx.recv().await {
                    Ok(TaskRuntimeEvent::Changed { .. })
                    | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        })
    }
}

fn parse_task_event_cursor(cursor: &str) -> Result<i64> {
    let cursor = cursor.trim();
    let value = cursor
        .parse::<i64>()
        .map_err(|_| async_graphql::Error::new("task event cursor is invalid"))?;
    if value < 0 {
        return Err(async_graphql::Error::new("task event cursor is invalid"));
    }
    Ok(value)
}

async fn project_task_event(
    store: &noema_store::NoemaStore,
    event: noema_tasks::TaskEventRecord,
) -> Result<GraphqlTaskEvent> {
    let kind = if event.event_kind.as_str() == "run.item_upserted" {
        GraphqlTaskEventKind::RunItemUpserted
    } else if event.event_kind.as_str().starts_with("run.") {
        GraphqlTaskEventKind::RunUpdated
    } else {
        GraphqlTaskEventKind::TaskUpdated
    };
    let run_id = event
        .payload
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let run = if let Some(run_id) = run_id.as_deref() {
        store
            .get_agent_run(run_id)
            .await
            .map_err(crate::graphql::errors::graphql_error)?
    } else {
        None
    };
    let item = if kind == GraphqlTaskEventKind::RunItemUpserted {
        let item_id = event
            .payload
            .get("item_id")
            .and_then(serde_json::Value::as_str);
        let sequence_index = event
            .payload
            .get("sequence_index")
            .and_then(serde_json::Value::as_i64);
        if let (Some(run_id), Some(item_id), Some(sequence_index)) =
            (run_id.as_deref(), item_id, sequence_index)
        {
            store
                .list_agent_run_items_page(run_id, Some(sequence_index.saturating_sub(1)), 1)
                .await
                .map_err(crate::graphql::errors::graphql_error)?
                .into_iter()
                .find(|item| item.item_id == item_id)
                .map(Into::into)
        } else {
            None
        }
    } else {
        None
    };
    let event_status = event
        .payload
        .get("status")
        .or_else(|| event.payload.get("to"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let status = match kind {
        GraphqlTaskEventKind::TaskUpdated => match event_status {
            Some(status) => Some(status),
            None => store
                .get_task(&event.task_id)
                .await
                .map_err(crate::graphql::errors::graphql_error)?
                .map(|task| task.status.as_str().to_string()),
        },
        GraphqlTaskEventKind::RunUpdated | GraphqlTaskEventKind::RunItemUpserted => {
            if event_status.is_some() {
                event_status
            } else {
                run.as_ref().map(|run| run.status.as_str().to_string())
            }
        }
    };
    Ok(GraphqlTaskEvent {
        cursor: event.sequence_number.to_string(),
        kind,
        task_id: event.task_id,
        run_id,
        status,
        item,
        run: run.map(Into::into),
        created_at: event.created_at,
    })
}
