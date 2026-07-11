use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;

use crate::daemon::TurnStreamEvent;

/// Live conversation event sent to GraphQL subscribers.
#[derive(Clone, Debug)]
pub(crate) enum ConversationLiveEvent {
    /// One runtime turn stream event.
    Turn {
        /// Frontend-generated id for optimistic UI correlation.
        client_message_id: Option<String>,
        /// Runtime event.
        event: Box<TurnStreamEvent>,
    },
    /// Turn completion event.
    Completed {
        /// Durable Noema conversation id.
        conversation_id: String,
        /// Frontend-generated id for optimistic UI correlation.
        client_message_id: Option<String>,
    },
}

/// Live task detail event used to invalidate/refill the task rail.
#[derive(Clone, Debug)]
pub(crate) enum TaskLiveEvent {
    /// Durable task/run state or safe run activity changed.
    Changed { task_id: String },
}

impl ConversationLiveEvent {
    /// Return the durable conversation id for this event.
    #[must_use]
    pub(crate) fn conversation_id(&self) -> &str {
        match self {
            Self::Turn { event, .. } => event.conversation_id(),
            Self::Completed {
                conversation_id, ..
            } => conversation_id,
        }
    }
}

/// In-process conversation event registry for GraphQL subscriptions.
#[derive(Clone, Debug, Default)]
pub(crate) struct ConversationSubscriptionRegistry {
    inner: Arc<Mutex<HashMap<String, broadcast::Sender<ConversationLiveEvent>>>>,
    task_inner: Arc<Mutex<HashMap<String, broadcast::Sender<TaskLiveEvent>>>>,
}

impl ConversationSubscriptionRegistry {
    /// Subscribe to one conversation's live turn events.
    pub(crate) fn subscribe(
        &self,
        conversation_id: &str,
    ) -> broadcast::Receiver<ConversationLiveEvent> {
        self.sender(conversation_id).subscribe()
    }

    /// Publish a live event to subscribers.
    pub(crate) fn publish(&self, event: ConversationLiveEvent) {
        let sender = self.sender(event.conversation_id());
        let _ = sender.send(event);
    }

    /// Subscribe to one task's durable/live detail updates.
    pub(crate) fn subscribe_task(&self, task_id: &str) -> broadcast::Receiver<TaskLiveEvent> {
        self.task_sender(task_id).subscribe()
    }

    /// Publish one task detail update.
    pub(crate) fn publish_task(&self, event: TaskLiveEvent) {
        let task_id = match &event {
            TaskLiveEvent::Changed { task_id } => task_id,
        };
        let _ = self.task_sender(task_id).send(event);
    }

    fn sender(&self, conversation_id: &str) -> broadcast::Sender<ConversationLiveEvent> {
        let mut inner = self.inner.lock().expect("subscription registry poisoned");
        inner
            .entry(conversation_id.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }

    fn task_sender(&self, task_id: &str) -> broadcast::Sender<TaskLiveEvent> {
        let mut inner = self
            .task_inner
            .lock()
            .expect("task subscription registry poisoned");
        inner
            .entry(task_id.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::{AgentStatus, daemon::TurnStreamEvent};

    use super::*;

    #[tokio::test]
    async fn registry_delivers_events_to_subscriber() {
        let registry = ConversationSubscriptionRegistry::default();
        let mut rx = registry.subscribe("conversation_1");

        registry.publish(ConversationLiveEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::AgentStatusChanged {
                conversation_id: "conversation_1".to_string(),
                status: AgentStatus::Thinking,
            }),
        });

        let event = rx.recv().await.expect("event should be delivered");
        assert_eq!(event.conversation_id(), "conversation_1");
    }

    #[tokio::test]
    async fn registry_delivers_task_events_to_subscriber() {
        let registry = ConversationSubscriptionRegistry::default();
        let mut rx = registry.subscribe_task("task_1");

        registry.publish_task(TaskLiveEvent::Changed {
            task_id: "task_1".to_string(),
        });

        assert!(
            matches!(rx.recv().await, Ok(TaskLiveEvent::Changed { task_id }) if task_id == "task_1")
        );
    }
}
