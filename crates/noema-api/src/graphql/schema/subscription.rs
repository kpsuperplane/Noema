//! Durable Work event subscriptions.
//!
//! Broadcast notifications are wakeups only. Every emitted item is read from
//! the durable `work_events` ledger, so lag and reconnects recover by scanning
//! from the last successfully projected cursor.

use super::*;
use async_graphql::{Context, ErrorExtensions, Result, Subscription};
use futures_util::{Stream, StreamExt};
use noema_store::{WorkEventCursor, WorkEventQuery, WorkOverviewQuery, WorkPageSize};
use noema_tasks::TaskId;
use noema_workspaces::WorkspaceId;

#[derive(Clone, async_graphql::SimpleObject)]
struct GraphqlTaskRuntimeEvent {
    task_id: String,
    run_id: Option<String>,
}

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Keep this exact browser client visible while its primary Chat has focus.
    async fn web_push_presence(
        &self,
        ctx: &Context<'_>,
        subscription_id: String,
    ) -> Result<impl Stream<Item = GraphqlWebPushPresenceEvent>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        state
            .web_push()
            .ok_or_else(|| async_graphql::Error::new("Web Push requires an HTTPS public origin"))?
            .presence(principal, subscription_id)
            .await
    }

    /// Stream pushed updates for one provider authentication attempt.
    async fn provider_auth_attempt_events(
        &self,
        ctx: &Context<'_>,
        attempt_id: String,
    ) -> Result<impl Stream<Item = Result<GraphqlProviderAuthAttempt>>> {
        let operations = ctx
            .data_unchecked::<GraphqlState>()
            .provider_account_operations()?
            .clone();
        let mut receiver = operations.subscribe_auth_attempts();
        let initial = operations
            .auth_attempt(&attempt_id)
            .await
            .map_err(onboarding::provider_operation_graphql_error)?;
        Ok(async_stream::stream! {
            if let Some(initial) = initial {
                let terminal = provider_auth_terminal(initial.status);
                yield Ok(initial.into());
                if terminal {
                    return;
                }
            }
            loop {
                match receiver.next().await {
                    Some(view) if view.attempt_id == attempt_id => {
                        let terminal = provider_auth_terminal(view.status);
                        yield Ok(view.into());
                        if terminal {
                            break;
                        }
                    }
                    Some(_) => continue,
                    None => break,
                }
            }
        })
    }

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

    /// Stream authoritative native-memory snapshots after each invalidation.
    async fn memory_events(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl Stream<Item = Result<GraphqlNativeMemoryTree>>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        native_memory::memory_events(state, principal).await
    }

    /// Replay and stream the workspace-scoped durable Work ledger.
    async fn work_events(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        after: Option<String>,
    ) -> Result<impl Stream<Item = Result<GraphqlWorkEvent>>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        require_work_owner(principal)?;
        let workspace_id =
            WorkspaceId::new(workspace_id.trim()).map_err(|_| work_unavailable_error())?;
        tasks::require_personal_workspace(&workspace_id)?;
        let store = state.store()?.clone();
        store
            .work_overview(WorkOverviewQuery {
                workspace_id: workspace_id.clone(),
                project_id: None,
                first: WorkPageSize::new(1).map_err(tasks::cursor_error)?,
            })
            .await
            .map_err(tasks::work_error)?;
        let after = decode_after(after.as_deref())?;
        let start_cursor = match after {
            Some(cursor) => Some(cursor),
            None => store
                .latest_work_event_cursor(&workspace_id)
                .await
                .map_err(tasks::work_error)?,
        };
        let receiver = state.subscriptions().subscribe_work(workspace_id.as_str());
        Ok(work_event_stream(
            store,
            receiver,
            workspace_id,
            None,
            start_cursor,
        ))
    }

    /// Stream the task-filtered projection of the same durable Work ledger.
    async fn task_events(
        &self,
        ctx: &Context<'_>,
        task_id: String,
        after: Option<String>,
    ) -> Result<impl Stream<Item = Result<GraphqlWorkEvent>>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        let store = state.store()?.clone();
        require_work_owner(principal)?;
        let task_id = TaskId::new(task_id.trim()).map_err(|_| work_unavailable_error())?;
        let detail = store
            .get_work_task(&task_id)
            .await
            .map_err(tasks::work_error)?
            .ok_or_else(work_unavailable_error)?;
        let workspace_id = detail.workspace.workspace_id;
        tasks::require_personal_workspace(&workspace_id)?;
        let after = decode_after(after.as_deref())?;
        let start_cursor = match after {
            Some(cursor) => Some(cursor),
            None => store
                .latest_task_work_event_cursor(&workspace_id, &task_id)
                .await
                .map_err(tasks::work_error)?,
        };
        let receiver = state.subscriptions().subscribe_work(workspace_id.as_str());
        Ok(work_event_stream(
            store,
            receiver,
            workspace_id,
            Some(task_id),
            start_cursor,
        ))
    }

    /// Wake subscribers when live task-run transcript items are persisted.
    async fn task_runtime_events(
        &self,
        ctx: &Context<'_>,
        task_id: String,
    ) -> Result<impl Stream<Item = Result<GraphqlTaskRuntimeEvent>>> {
        let state = ctx.data_unchecked::<GraphqlState>();
        let principal = crate::graphql::request_principal_subject(ctx)?;
        require_work_owner(principal)?;
        let task_id = TaskId::new(task_id.trim()).map_err(|_| work_unavailable_error())?;
        state
            .store()?
            .get_work_task(&task_id)
            .await
            .map_err(tasks::work_error)?
            .ok_or_else(work_unavailable_error)?;
        let mut receiver = state.subscriptions().subscribe_task(task_id.as_str());
        Ok(async_stream::stream! {
            loop {
                match receiver.recv().await {
                    Ok(noema_runtime::TaskRuntimeEvent::Changed { task_id, run_id }) => {
                        yield Ok(GraphqlTaskRuntimeEvent { task_id, run_id });
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        })
    }
}

fn provider_auth_terminal(status: noema_providers::ProviderAuthAttemptStatus) -> bool {
    matches!(
        status,
        noema_providers::ProviderAuthAttemptStatus::Completed
            | noema_providers::ProviderAuthAttemptStatus::Failed
            | noema_providers::ProviderAuthAttemptStatus::Expired
            | noema_providers::ProviderAuthAttemptStatus::Cancelled
    )
}

fn work_event_stream(
    store: noema_store::NoemaStore,
    mut receiver: tokio::sync::broadcast::Receiver<noema_runtime::WorkRuntimeEvent>,
    workspace_id: WorkspaceId,
    task_id: Option<TaskId>,
    start_cursor: Option<WorkEventCursor>,
) -> impl Stream<Item = Result<GraphqlWorkEvent>> {
    async_stream::stream! {
        // The caller's cursor, or the durable high-water cursor captured before
        // receiver registration, is the replay boundary. Any commit after that
        // read is recovered from the ledger before the stream waits on wakeups.
        let mut cursor = start_cursor;

        loop {
            let page = match store.list_work_events_after(WorkEventQuery {
                workspace_id: workspace_id.clone(),
                project_id: None,
                task_id: task_id.clone(),
                run_id: None,
                after: cursor,
                first: WorkPageSize::new(100).expect("the fixed subscription page size is valid"),
            }).await.map_err(tasks::work_error) {
                Ok(page) => page,
                Err(error) => { yield Err(error); break; }
            };
            if !page.edges.is_empty() {
                for edge in page.edges {
                    let projected = match project_event_edge_and_advance(&edge, &mut cursor) {
                        Ok(projected) => projected,
                        Err(error) => { yield Err(error); return; }
                    };
                    yield Ok(projected);
                }
                continue;
            }
            match receiver.recv().await {
                Ok(noema_runtime::WorkRuntimeEvent::Committed { .. }) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}

fn project_event_edge(edge: &noema_store::WorkEventEdge) -> Result<GraphqlWorkEvent> {
    let projected =
        GraphqlWorkEvent::try_from(edge.node.clone()).map_err(|_| work_unavailable_error())?;
    if projected.cursor == edge.cursor.encode() {
        Ok(projected)
    } else {
        Err(work_unavailable_error())
    }
}

fn project_event_edge_and_advance(
    edge: &noema_store::WorkEventEdge,
    cursor: &mut Option<WorkEventCursor>,
) -> Result<GraphqlWorkEvent> {
    let projected = project_event_edge(edge)?;
    *cursor = Some(edge.cursor);
    Ok(projected)
}

fn decode_after(value: Option<&str>) -> Result<Option<WorkEventCursor>> {
    value
        .map(WorkEventCursor::decode)
        .transpose()
        .map_err(tasks::cursor_error)
}

fn require_work_owner(principal: &str) -> Result<()> {
    if principal == "human:local" {
        Ok(())
    } else {
        Err(work_unavailable_error())
    }
}

fn work_unavailable_error() -> async_graphql::Error {
    async_graphql::Error::new("work is unavailable")
        .extend_with(|_, extensions| extensions.set("code", "work_unavailable"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_failure_does_not_advance_the_subscription_cursor() {
        let original = WorkEventCursor::new(1).expect("original cursor");
        let mut cursor = Some(original);
        let node = noema_tasks::WorkEventRecord::new(
            noema_tasks::WorkEventId::new("event:projection-overflow").expect("event id"),
            u64::try_from(i64::MAX).expect("i64 maximum fits u64") + 1,
            noema_tasks::WorkEventContext {
                workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
                project_id: None,
                task_id: None,
                run_id: None,
                actor_id: "actor:system".to_string(),
                causation_id: None,
                correlation_id: "correlation:projection-overflow".to_string(),
            },
            noema_tasks::WorkEventPayload::project_created(1).expect("event payload"),
            "2026-07-18T00:00:00.000Z".to_string(),
        )
        .expect("domain-valid event outside cursor range");
        let edge = noema_store::WorkEventEdge {
            cursor: WorkEventCursor::new(2).expect("edge cursor"),
            node,
        };

        assert!(project_event_edge_and_advance(&edge, &mut cursor).is_err());
        assert_eq!(cursor, Some(original));
    }
}
