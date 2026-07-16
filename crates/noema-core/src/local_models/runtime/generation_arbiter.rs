use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::sync::oneshot;

use noema_providers::GenerationPriority;

#[derive(Clone, Debug, Default)]
pub(super) struct GenerationArbiter {
    inner: Arc<GenerationArbiterInner>,
}

#[derive(Debug, Default)]
struct GenerationArbiterInner {
    state: Mutex<GenerationArbiterState>,
}

#[derive(Debug, Default)]
struct GenerationArbiterState {
    active: bool,
    closed: bool,
    foreground: VecDeque<oneshot::Sender<GenerationPermit>>,
    background: VecDeque<oneshot::Sender<GenerationPermit>>,
}

#[derive(Debug)]
pub(crate) struct GenerationPermit {
    inner: Arc<GenerationArbiterInner>,
    armed: bool,
}

impl GenerationArbiter {
    pub(super) async fn acquire(
        &self,
        priority: GenerationPriority,
    ) -> Result<GenerationPermit, GenerationArbiterClosed> {
        let receiver = {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if state.closed {
                return Err(GenerationArbiterClosed);
            }
            if !state.active && state.foreground.is_empty() && state.background.is_empty() {
                state.active = true;
                None
            } else {
                let (sender, receiver) = oneshot::channel();
                match priority {
                    GenerationPriority::Foreground => state.foreground.push_back(sender),
                    GenerationPriority::Background => state.background.push_back(sender),
                }
                Some(receiver)
            }
        };

        match receiver {
            Some(receiver) => receiver.await.map_err(|_| GenerationArbiterClosed),
            None => Ok(GenerationPermit::new(Arc::clone(&self.inner))),
        }
    }

    fn release(inner: &Arc<GenerationArbiterInner>) {
        let mut state = inner.state.lock().unwrap_or_else(PoisonError::into_inner);
        if !state.active {
            debug_assert!(state.active, "generation permit released while idle");
            return;
        }
        state.active = false;
        if state.closed {
            return;
        }

        loop {
            let Some(waiter) = state
                .foreground
                .pop_front()
                .or_else(|| state.background.pop_front())
            else {
                return;
            };
            state.active = true;
            match waiter.send(GenerationPermit::new(Arc::clone(inner))) {
                Ok(()) => return,
                Err(mut permit) => {
                    permit.disarm();
                    state.active = false;
                }
            }
        }
    }

    pub(super) fn close(&self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        state.closed = true;
        state.foreground.clear();
        state.background.clear();
    }

    pub(super) fn is_closed(&self) -> bool {
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .closed
    }

    #[cfg(test)]
    pub(super) fn queued(&self, priority: GenerationPriority) -> usize {
        let state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match priority {
            GenerationPriority::Foreground => state.foreground.len(),
            GenerationPriority::Background => state.background.len(),
        }
    }
}

impl GenerationPermit {
    fn new(inner: Arc<GenerationArbiterInner>) -> Self {
        Self { inner, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for GenerationPermit {
    fn drop(&mut self) {
        if self.armed {
            self.armed = false;
            GenerationArbiter::release(&self.inner);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct GenerationArbiterClosed;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::task::JoinHandle;

    use super::*;

    async fn wait_for_queue(
        arbiter: &GenerationArbiter,
        priority: GenerationPriority,
        expected: usize,
    ) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while arbiter.queued(priority) != expected {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("waiter should enter the generation queue");
    }

    async fn join_permit(
        task: JoinHandle<Result<GenerationPermit, GenerationArbiterClosed>>,
        message: &'static str,
    ) -> GenerationPermit {
        join_result(task, message).await.expect("generation permit")
    }

    async fn join_result(
        task: JoinHandle<Result<GenerationPermit, GenerationArbiterClosed>>,
        message: &'static str,
    ) -> Result<GenerationPermit, GenerationArbiterClosed> {
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect(message)
            .expect("generation task")
    }

    #[tokio::test]
    async fn serializes_active_generations() {
        let arbiter = GenerationArbiter::default();
        let first = arbiter
            .acquire(GenerationPriority::Foreground)
            .await
            .expect("first permit");
        let queued_arbiter = arbiter.clone();
        let second =
            tokio::spawn(
                async move { queued_arbiter.acquire(GenerationPriority::Foreground).await },
            );
        wait_for_queue(&arbiter, GenerationPriority::Foreground, 1).await;

        assert!(!second.is_finished());
        drop(first);
        let second = join_permit(second, "second generation should be released").await;
        drop(second);
    }

    #[tokio::test]
    async fn foreground_overtakes_queued_background() {
        let arbiter = GenerationArbiter::default();
        let active = arbiter
            .acquire(GenerationPriority::Background)
            .await
            .expect("active permit");
        let background_arbiter = arbiter.clone();
        let background = tokio::spawn(async move {
            background_arbiter
                .acquire(GenerationPriority::Background)
                .await
        });
        wait_for_queue(&arbiter, GenerationPriority::Background, 1).await;
        let foreground_arbiter = arbiter.clone();
        let foreground = tokio::spawn(async move {
            foreground_arbiter
                .acquire(GenerationPriority::Foreground)
                .await
        });
        wait_for_queue(&arbiter, GenerationPriority::Foreground, 1).await;

        drop(active);
        let foreground = join_permit(foreground, "foreground generation should overtake").await;
        assert!(!background.is_finished());

        background.abort();
        let _ = background.await;
        drop(foreground);
    }

    #[tokio::test]
    async fn same_priority_waiters_run_fifo() {
        let arbiter = GenerationArbiter::default();
        let active = arbiter
            .acquire(GenerationPriority::Foreground)
            .await
            .expect("active permit");
        let first_arbiter = arbiter.clone();
        let first =
            tokio::spawn(
                async move { first_arbiter.acquire(GenerationPriority::Foreground).await },
            );
        wait_for_queue(&arbiter, GenerationPriority::Foreground, 1).await;
        let second_arbiter = arbiter.clone();
        let second =
            tokio::spawn(
                async move { second_arbiter.acquire(GenerationPriority::Foreground).await },
            );
        wait_for_queue(&arbiter, GenerationPriority::Foreground, 2).await;

        drop(active);
        let first = join_permit(first, "first same-priority waiter should run first").await;
        assert!(!second.is_finished());
        drop(first);
        let second = join_permit(second, "second same-priority waiter should run second").await;
        drop(second);
    }

    #[tokio::test]
    async fn background_runs_after_foreground_queue_drains() {
        let arbiter = GenerationArbiter::default();
        let active = arbiter
            .acquire(GenerationPriority::Foreground)
            .await
            .expect("active permit");
        let background_arbiter = arbiter.clone();
        let background = tokio::spawn(async move {
            background_arbiter
                .acquire(GenerationPriority::Background)
                .await
        });
        wait_for_queue(&arbiter, GenerationPriority::Background, 1).await;
        let first_foreground_arbiter = arbiter.clone();
        let first_foreground = tokio::spawn(async move {
            first_foreground_arbiter
                .acquire(GenerationPriority::Foreground)
                .await
        });
        wait_for_queue(&arbiter, GenerationPriority::Foreground, 1).await;
        let second_foreground_arbiter = arbiter.clone();
        let second_foreground = tokio::spawn(async move {
            second_foreground_arbiter
                .acquire(GenerationPriority::Foreground)
                .await
        });
        wait_for_queue(&arbiter, GenerationPriority::Foreground, 2).await;

        drop(active);
        let first_foreground = join_permit(first_foreground, "first foreground should run").await;
        assert!(!second_foreground.is_finished());
        assert!(!background.is_finished());
        drop(first_foreground);
        let second_foreground =
            join_permit(second_foreground, "second foreground should run").await;
        assert!(!background.is_finished());
        drop(second_foreground);
        let background =
            join_permit(background, "background should run after foreground drains").await;
        drop(background);
    }

    #[tokio::test]
    async fn cancelled_waiters_are_skipped() {
        let arbiter = GenerationArbiter::default();
        let active = arbiter
            .acquire(GenerationPriority::Foreground)
            .await
            .expect("active permit");
        let cancelled_arbiter = arbiter.clone();
        let cancelled = tokio::spawn(async move {
            cancelled_arbiter
                .acquire(GenerationPriority::Foreground)
                .await
        });
        wait_for_queue(&arbiter, GenerationPriority::Foreground, 1).await;
        cancelled.abort();
        let _ = cancelled.await;

        let granted_then_cancelled_arbiter = arbiter.clone();
        let granted_then_cancelled = tokio::spawn(async move {
            granted_then_cancelled_arbiter
                .acquire(GenerationPriority::Foreground)
                .await
        });
        wait_for_queue(&arbiter, GenerationPriority::Foreground, 2).await;
        let live_arbiter = arbiter.clone();
        let live =
            tokio::spawn(async move { live_arbiter.acquire(GenerationPriority::Background).await });
        wait_for_queue(&arbiter, GenerationPriority::Background, 1).await;

        drop(active);
        granted_then_cancelled.abort();
        let _ = granted_then_cancelled.await;

        let live = join_permit(live, "live waiter should survive cancelled waiters").await;
        drop(live);
    }

    #[tokio::test]
    async fn aborting_active_holder_releases_capacity() {
        let arbiter = GenerationArbiter::default();
        let holder_arbiter = arbiter.clone();
        let (acquired_tx, acquired_rx) = oneshot::channel();
        let holder = tokio::spawn(async move {
            let _permit = holder_arbiter
                .acquire(GenerationPriority::Foreground)
                .await
                .expect("holder permit");
            let _ = acquired_tx.send(());
            std::future::pending::<()>().await;
        });
        tokio::time::timeout(Duration::from_secs(1), acquired_rx)
            .await
            .expect("holder should acquire capacity")
            .expect("holder acquisition signal");
        let follower_arbiter = arbiter.clone();
        let follower = tokio::spawn(async move {
            follower_arbiter
                .acquire(GenerationPriority::Background)
                .await
        });
        wait_for_queue(&arbiter, GenerationPriority::Background, 1).await;

        holder.abort();
        let _ = holder.await;

        let follower = join_permit(follower, "follower should run after holder aborts").await;
        drop(follower);
    }

    #[tokio::test]
    async fn close_drains_waiters_and_permanently_rejects_acquisition() {
        let arbiter = GenerationArbiter::default();
        let active = arbiter
            .acquire(GenerationPriority::Foreground)
            .await
            .expect("active permit");
        let queued_arbiter = arbiter.clone();
        let queued =
            tokio::spawn(
                async move { queued_arbiter.acquire(GenerationPriority::Background).await },
            );
        wait_for_queue(&arbiter, GenerationPriority::Background, 1).await;

        arbiter.close();

        assert!(arbiter.is_closed());
        assert!(
            join_result(queued, "queued waiter should be drained")
                .await
                .is_err()
        );
        assert!(
            arbiter
                .acquire(GenerationPriority::Foreground)
                .await
                .is_err()
        );
        drop(active);
        assert!(
            arbiter
                .acquire(GenerationPriority::Background)
                .await
                .is_err()
        );
    }
}
