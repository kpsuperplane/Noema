//! Reviewer-only access to governed artifacts linked to a task submission.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{NoemaStore, provider::NoemaToolSpec};

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
-> Result<NoemaToolSpec, crate::provider::ToolContractError> {
    NoemaToolSpec::new(
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
        crate::provider::NoemaToolExecution::LocalBuiltin,
    )
}

pub(crate) async fn execute_task_read_artifact(
    store: &NoemaStore,
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
                && run.run_kind == crate::RunKind::Reviewer
                && run.status == crate::RunStatus::Running
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
    let paths = store.noema_paths().map_err(|error| error.to_string())?;
    let (_, bytes) = crate::artifacts::read_validated_local_artifact_file(
        &paths,
        &linked.artifact,
        &linked.version,
    )
    .map_err(|_| "artifact content is unavailable".to_string())?;
    let text =
        String::from_utf8(bytes).map_err(|_| "artifact is not valid UTF-8 text".to_string())?;
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
        let store = crate::store::tests::test_store().await;
        let (task, executor) = crate::store::tests::seed_task(&store, "Artifact review").await;
        store
            .claim_next_agent_run("worker:test", "lease:executor", 120)
            .await
            .expect("claim")
            .expect("executor");
        store
            .transition_agent_run(
                &executor.run_id,
                crate::RunStatus::Running,
                Some("lease:executor"),
                None,
            )
            .await
            .expect("running");
        store
            .transition_task(&task.task_id, crate::TaskStatus::Executing, None)
            .await
            .expect("executing");
        let artifact = crate::create_task_local_file_artifact(
            &store,
            &store.noema_paths().expect("paths"),
            crate::NewTaskLocalFileArtifact {
                task_id: task.task_id.clone(),
                title: "Evidence".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                filename: "evidence.txt".to_string(),
                bytes: b"reviewable evidence".to_vec(),
                media_type: Some("text/plain".to_string()),
                created_by_actor_id: crate::TASK_EXECUTOR_AGENT_ID.to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: json!({}),
            },
        )
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
        let (_, reviewer) = store
            .create_task_submission(
                crate::NewTaskSubmission {
                    submission_id: None,
                    task_id: task.task_id.clone(),
                    executor_run_id: executor.run_id,
                    revision_index: 0,
                    summary: "Done".to_string(),
                    result_markdown: "See the artifact.".to_string(),
                    criteria: vec![crate::SubmissionCriterionEvidence {
                        criterion_id,
                        evidence_markdown: "Artifact contains evidence".to_string(),
                    }],
                    artifact_ids: vec![artifact.artifact.artifact_id.clone()],
                },
                "lease:executor",
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
                crate::RunStatus::Running,
                Some("lease:reviewer"),
                None,
            )
            .await
            .expect("reviewer running");

        let result = execute_task_read_artifact(
            &store,
            &TaskArtifactReadContext {
                task_id: task.task_id.clone(),
                run_id: reviewer.run_id.clone(),
            },
            &json!({"artifact_id": artifact.artifact.artifact_id}),
        )
        .await
        .expect("read artifact");

        assert_eq!(result["content"], "reviewable evidence");

        let unlinked = crate::create_task_local_file_artifact(
            &store,
            &store.noema_paths().expect("paths"),
            crate::NewTaskLocalFileArtifact {
                task_id: task.task_id.clone(),
                title: "Unlinked".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                filename: "unlinked.txt".to_string(),
                bytes: b"not submitted".to_vec(),
                media_type: Some("text/plain".to_string()),
                created_by_actor_id: crate::TASK_EXECUTOR_AGENT_ID.to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: json!({}),
            },
        )
        .await
        .expect("unlinked artifact");
        let denied = execute_task_read_artifact(
            &store,
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
