use std::time::Duration;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::{
    sync::oneshot,
    task::JoinHandle,
    time::{MissedTickBehavior, interval},
};

use super::types::ModelEvalRuntimeMemory;

const SAMPLE_INTERVAL: Duration = Duration::from_millis(50);

pub(super) struct RuntimeMemorySampler {
    stop_tx: oneshot::Sender<()>,
    task: JoinHandle<Option<ModelEvalRuntimeMemory>>,
}

impl RuntimeMemorySampler {
    pub(super) fn start(process_id: u32) -> Self {
        let (stop_tx, mut stop_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut system = System::new();
            let initial = sample_process_memory(&mut system, process_id);
            let mut peak_bytes = initial.as_ref().map_or(0, |sample| sample.bytes);
            let mut ticker = interval(SAMPLE_INTERVAL);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        if let Some(sample) = sample_process_memory(&mut system, process_id) {
                            peak_bytes = peak_bytes.max(sample.bytes);
                        }
                    }
                    _ = &mut stop_rx => break,
                }
            }
            initial.map(|sample| ModelEvalRuntimeMemory {
                metric: sample.metric.to_string(),
                ready_bytes: sample.bytes,
                peak_bytes,
            })
        });
        Self { stop_tx, task }
    }

    pub(super) async fn finish(self) -> Option<ModelEvalRuntimeMemory> {
        let _ = self.stop_tx.send(());
        self.task.await.ok().flatten()
    }
}

struct ProcessMemorySample {
    metric: &'static str,
    bytes: u64,
}

fn sample_process_memory(system: &mut System, process_id: u32) -> Option<ProcessMemorySample> {
    let process_id = Pid::from_u32(process_id);
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[process_id]),
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );
    let process = system.process(process_id)?;
    Some(ProcessMemorySample {
        metric: "resident_set",
        bytes: process.memory(),
    })
}

pub(super) fn resident_bytes(process_id: u32) -> Option<u64> {
    let mut system = System::new();
    sample_process_memory(&mut system, process_id).map(|sample| sample.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_the_current_process() {
        let mut system = System::new();
        let sample = sample_process_memory(&mut system, std::process::id()).expect("memory sample");

        assert!(sample.bytes > 0);
        assert!(!sample.metric.is_empty());
    }
}
