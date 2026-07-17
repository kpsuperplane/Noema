//! Reviewer-only access to governed artifacts linked to a task submission.

use serde::Deserialize;
use serde_json::{Value, json};

use noema_capabilities::ToolSpec;
use noema_store::NoemaStore;

pub(crate) const TASK_READ_ARTIFACT_TOOL: &str = "task.read_artifact";
const MAX_ARTIFACT_TEXT_CHARS: usize = 120_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskArtifactReadContext {
    pub task_id: String,
    pub run_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArtifactArguments {
    artifact_id: String,
}

#[must_use]
pub(crate) fn is_task_read_artifact_tool(name: &str) -> bool {
    name == TASK_READ_ARTIFACT_TOOL
}

pub(crate) fn task_read_artifact_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_READ_ARTIFACT_TOOL,
        "Read bounded UTF-8 content from an artifact linked to the submission under review. Binary, oversized, external, foreign, and unlinked artifacts fail closed.",
        json!({
            "type": "object",
            "properties": {
                "artifact_id": {"type": "string", "minLength": 1, "maxLength": 200}
            },
            "required": ["artifact_id"],
            "additionalProperties": false
        }),
    )
}

pub(crate) async fn execute_task_read_artifact(
    store: &NoemaStore,
    artifact_operations: &noema_artifacts::ArtifactOperationsHandle,
    context: &TaskArtifactReadContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments: ReadArtifactArguments = serde_json::from_value(payload.clone())
        .map_err(|error| format!("invalid task artifact arguments: {error}"))?;
    let run = store
        .get_agent_run(&context.run_id)
        .await
        .map_err(|error| error.to_string())?
        .filter(|run| {
            run.task_id == context.task_id
                && run.run_kind == noema_tasks::RunKind::Reviewer
                && run.status == noema_tasks::RunStatus::Running
                && !run.cancellation_requested
        })
        .ok_or_else(|| "reviewer task context is unavailable".to_string())?;
    let submission_id = run
        .triggering_submission_id
        .as_deref()
        .ok_or_else(|| "reviewer run has no submission".to_string())?;
    let submission = store
        .get_task_submission(submission_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "submission is unavailable".to_string())?;
    let linked = submission
        .artifacts
        .into_iter()
        .find(|linked| linked.artifact.artifact_id == arguments.artifact_id)
        .ok_or_else(|| "artifact is not linked to the submission under review".to_string())?;

    if !matches!(
        linked.version.storage,
        noema_artifacts::ArtifactVersionStorage::LocalFile { .. }
    ) {
        return Err("external artifact content is unavailable to the reviewer".to_string());
    }
    let file = artifact_operations
        .read_local_file(noema_artifacts::ReadLocalArtifactRequest {
            artifact: linked.artifact.clone(),
            version: linked.version.clone(),
        })
        .await
        .map_err(|_| "artifact content is unavailable".to_string())?;
    let text = String::from_utf8(file.bytes)
        .map_err(|_| "artifact is not valid UTF-8 text".to_string())?;
    if text.chars().count() > MAX_ARTIFACT_TEXT_CHARS {
        return Err("artifact exceeds the reviewer text limit".to_string());
    }
    Ok(json!({
        "artifact_id": linked.artifact.artifact_id,
        "artifact_version_id": linked.version.artifact_version_id,
        "title": linked.artifact.title,
        "artifact_kind": linked.artifact.artifact_kind,
        "media_type": linked.version.media_type,
        "content": text,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reviewer_reads_only_the_linked_task_artifact_snapshot() {
        let store = crate::test_support::test_store().await;
        let artifact_operations =
            crate::test_support::artifact_operations(&store).expect("artifact operations");
        let (task, executor) = crate::test_support::seed_task(&store, "Artifact review").await;
        store
            .claim_next_agent_run("worker:test", "lease:executor", 120)
            .await
            .expect("claim")
            .expect("executor");
        store
            .transition_agent_run(
                &executor.run_id,
                noema_tasks::RunStatus::Running,
                Some("lease:executor"),
                None,
            )
            .await
            .expect("running");
        store
            .transition_task(&task.task_id, noema_tasks::TaskStatus::Executing, None)
            .await
            .expect("executing");
        let artifact = artifact_operations
            .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
                owner: noema_artifacts::ArtifactOwnerRef::task(&task.task_id),
                title: "Evidence".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                filename: "evidence.txt".to_string(),
                bytes: b"reviewable evidence".to_vec(),
                media_type: Some("text/plain".to_string()),
                created_by_actor_id: noema_tasks::TASK_EXECUTOR_AGENT_ID.to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: json!({}),
            })
            .await
            .expect("artifact");
        assert!(
            store
                .get_local_artifact_version_for_human(
                    &artifact.current_version.artifact_version_id,
                    "human:local",
                )
                .await
                .expect("authorized artifact")
                .is_some()
        );
        let criterion_id = store
            .list_task_validation_criteria(&task.task_id)
            .await
            .expect("criteria")[0]
            .criterion_id
            .clone();
        let provider_registry = crate::test_support::ready_test_provider_registry();
        let (_, reviewer) = store
            .create_task_submission_with_readiness(
                noema_tasks::NewTaskSubmission {
                    submission_id: None,
                    task_id: task.task_id.clone(),
                    executor_run_id: executor.run_id,
                    revision_index: 0,
                    summary: "Done".to_string(),
                    result_markdown: "See the artifact.".to_string(),
                    criteria: vec![noema_tasks::SubmissionCriterionEvidence {
                        criterion_id,
                        evidence_markdown: "Artifact contains evidence".to_string(),
                    }],
                    artifact_ids: vec![artifact.artifact.artifact_id.clone()],
                },
                "lease:executor",
                provider_registry.as_ref(),
            )
            .await
            .expect("submission");
        store
            .claim_next_agent_run("worker:test", "lease:reviewer", 120)
            .await
            .expect("claim")
            .expect("reviewer");
        store
            .transition_agent_run(
                &reviewer.run_id,
                noema_tasks::RunStatus::Running,
                Some("lease:reviewer"),
                None,
            )
            .await
            .expect("reviewer running");

        let result = execute_task_read_artifact(
            &store,
            &artifact_operations,
            &TaskArtifactReadContext {
                task_id: task.task_id.clone(),
                run_id: reviewer.run_id.clone(),
            },
            &json!({"artifact_id": artifact.artifact.artifact_id}),
        )
        .await
        .expect("read artifact");

        assert_eq!(result["content"], "reviewable evidence");

        let unlinked = artifact_operations
            .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
                owner: noema_artifacts::ArtifactOwnerRef::task(&task.task_id),
                title: "Unlinked".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                filename: "unlinked.txt".to_string(),
                bytes: b"not submitted".to_vec(),
                media_type: Some("text/plain".to_string()),
                created_by_actor_id: noema_tasks::TASK_EXECUTOR_AGENT_ID.to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: json!({}),
            })
            .await
            .expect("unlinked artifact");
        let denied = execute_task_read_artifact(
            &store,
            &artifact_operations,
            &TaskArtifactReadContext {
                task_id: task.task_id,
                run_id: reviewer.run_id,
            },
            &json!({"artifact_id": unlinked.artifact.artifact_id}),
        )
        .await;
        assert!(denied.is_err());
    }
}
