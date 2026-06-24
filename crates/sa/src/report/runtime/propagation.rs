use anyhow::Context;

use crate::TaskManager;
use crate::{AnalysisResult, PersistedTask};

use crate::task_manager::TaskRunParams;

pub(crate) struct Propagator;

impl Propagator {
    pub(crate) async fn prepare_result(
        manager: &TaskManager,
        task: &PersistedTask,
        params: &TaskRunParams,
    ) -> anyhow::Result<AnalysisResult> {
        let checkpoint = manager
            .checkpoint_store
            .load(&task.task_id, &task.symbol, &task.analysis_date)
            .await?;
        let mut result = if checkpoint.is_some() {
            let checkpoint = manager
                .checkpoint_store
                .load(&task.task_id, &task.symbol, &task.analysis_date)
                .await?
                .context("checkpoint payload missing after existence check")?;
            let mut result = checkpoint.result;
            result.artifacts.checkpoint_thread_id =
                crate::checkpoint::TaskCheckpointStore::thread_id(
                    &task.task_id,
                    &task.symbol,
                    &task.analysis_date,
                );
            result.artifacts.resumed_from_node = checkpoint.node;
            result.artifacts.resumed_from_step = checkpoint.step;
            result
        } else {
            let mut built = manager.build_initial_result(task, params);
            built.artifacts.checkpoint_thread_id =
                crate::checkpoint::TaskCheckpointStore::thread_id(
                    &task.task_id,
                    &task.symbol,
                    &task.analysis_date,
                );
            manager
                .analysis_store
                .save_result(&task.task_id, &built)
                .await?;
            manager
                .save_checkpoint(
                    &task.task_id,
                    &task.symbol,
                    &task.analysis_date,
                    "overview",
                    "overview",
                    &built,
                )
                .await?;
            built
        };
        result.artifacts.runtime_nodes = manager
            .checkpoint_store
            .load_writes(&task.task_id, &task.symbol, &task.analysis_date)
            .await?
            .into_iter()
            .map(|write| crate::RuntimeNodeTrace {
                stage: write.stage,
                node: write.node,
                step: write.step,
                timestamp: write.created_at,
            })
            .collect();
        Ok(result)
    }
}
