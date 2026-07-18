//! Human-controlled executor model pools.

mod defaults;
mod entries;
mod rows;
mod selection;

#[cfg(test)]
mod tests;

use noema_tasks::TaskComplexity;

use super::{NoemaStore, StoreError};

const TASK_MODEL_POOL_SETTING_PREFIX: &str = "task_pool:setting:";

fn global_task_model_pool_setting_id(complexity: TaskComplexity) -> String {
    format!("{TASK_MODEL_POOL_SETTING_PREFIX}{}", complexity.as_str())
}
