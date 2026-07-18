use noema_store::{
    ReportRunFailure, ReportTaskBlocked, SubmitTaskResult, SubmitTaskReview, WorkCommandService,
    WorkRunFence, WorkRunTerminal,
};
use noema_tasks::{
    CriterionOutcome, NewTaskReview, NewTaskSubmission, RunStatus, SafeErrorCode,
    SubmissionCriterionEvidence, TaskGateKind, TaskGateState, TaskRecoveryReason,
    TaskReviewCriterion, TaskReviewVerdict,
};
use serde_json::json;

async fn start_seeded_executor(
    store: &noema_store::NoemaStore,
    service: &WorkCommandService,
    title: &str,
    worker: &str,
) -> (
    noema_tasks::TaskRecord,
    noema_tasks::TaskExecutionContract,
    WorkRunFence,
) {
    let (task, expected_run) = crate::test_support::seed_task(store, title).await;
    let detail = store
        .get_work_task(&task.task_id)
        .await
        .expect("seeded detail read")
        .expect("seeded detail");
    let contract = detail.current_contract.expect("delegated contract");
    let claim = service
        .claim_next_work_run(worker, 60, &[])
        .await
        .expect("claim executor")
        .expect("queued executor");
    assert_eq!(claim.run.run_id, expected_run.run_id);
    let fence = WorkRunFence {
        run_id: claim.run.run_id,
        lease_token: claim.lease_token,
        task_generation: task.generation,
        contract_id: Some(contract.contract_id.clone()),
    };
    service
        .start_work_run(
            &fence,
            "actor:agent:test",
            None,
            &format!("correlation:{worker}:start"),
        )
        .await
        .expect("start executor");
    (task, contract, fence)
}

#[tokio::test]
async fn recovery_mutations_cannot_bypass_advertised_actions() {
    let store = crate::test_support::test_store().await;
    let service = WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );

    let (unsafe_task, _, unsafe_fence) =
        start_seeded_executor(&store, &service, "Unsafe effect task", "unsafe-effect").await;
    service
        .report_work_run_failure(
            ReportRunFailure {
                fence: unsafe_fence,
                status: RunStatus::Failed,
                error_code: SafeErrorCode::new("unsafe_effect_uncertain").expect("safe error code"),
                error_message: Some("The external effect cannot be confirmed.".to_string()),
                retryable: true,
            },
            "actor:agent:test",
            None,
            "correlation:unsafe-effect:failure",
        )
        .await
        .expect("unsafe effect opens recovery instead of retrying");
    let unsafe_waiting = store
        .get_work_task(&unsafe_task.task_id)
        .await
        .expect("load unsafe recovery")
        .expect("unsafe recovery task");
    let unsafe_gate = unsafe_waiting
        .active_gate
        .as_ref()
        .expect("unsafe recovery gate");
    assert_eq!(
        unsafe_gate.recovery_reason,
        Some(TaskRecoveryReason::UnsafeEffectUncertain)
    );

    let (configuration_task, _, configuration_fence) = start_seeded_executor(
        &store,
        &service,
        "Configuration recovery task",
        "configuration-recovery",
    )
    .await;
    service
        .report_work_run_failure(
            ReportRunFailure {
                fence: configuration_fence,
                status: RunStatus::Failed,
                error_code: SafeErrorCode::new("configuration_unavailable")
                    .expect("safe error code"),
                error_message: Some("The configured route is unavailable.".to_string()),
                retryable: true,
            },
            "actor:agent:test",
            None,
            "correlation:configuration-recovery:failure",
        )
        .await
        .expect("configuration failure opens recovery instead of retrying");
    let configuration_waiting = store
        .get_work_task(&configuration_task.task_id)
        .await
        .expect("load configuration recovery")
        .expect("configuration recovery task");
    let configuration_gate = configuration_waiting
        .active_gate
        .as_ref()
        .expect("configuration recovery gate");
    assert_eq!(
        configuration_gate.recovery_reason,
        Some(TaskRecoveryReason::ConfigurationUnavailable)
    );

    let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
    let advertised = schema
        .execute(format!(
            r#"query {{
              unsafe: task(taskId: "{}") {{ validActions activeGate {{ recoveryReason }} }}
              configuration: task(taskId: "{}") {{ validActions activeGate {{ recoveryReason }} }}
            }}"#,
            unsafe_task.task_id, configuration_task.task_id,
        ))
        .await;
    assert!(advertised.errors.is_empty(), "{:?}", advertised.errors);
    let advertised = advertised.data.into_json().expect("recovery actions JSON");
    assert_eq!(
        advertised["unsafe"]["validActions"],
        json!(["ANSWER", "CANCEL"])
    );
    assert_eq!(
        advertised["unsafe"]["activeGate"]["recoveryReason"],
        "UNSAFE_EFFECT_UNCERTAIN"
    );
    assert_eq!(
        advertised["configuration"]["validActions"],
        json!(["RETRY", "CANCEL"])
    );
    assert_eq!(
        advertised["configuration"]["activeGate"]["recoveryReason"],
        "CONFIGURATION_UNAVAILABLE"
    );

    let retry_unsafe = schema
        .execute(format!(
            r#"mutation {{
              retryTask(input: {{
                taskId: "{}"
                gateId: "{}"
                expectedRevision: {}
                expectedGeneration: {}
                clientMutationId: "retry-unsafe-effect"
              }}) {{ eventCursor }}
            }}"#,
            unsafe_waiting.task.task_id,
            unsafe_gate.gate_id,
            unsafe_waiting.task.revision,
            unsafe_waiting.task.generation,
        ))
        .await;
    assert_single_graphql_error(&retry_unsafe, "requested work action is not valid now");
    assert_eq!(
        retry_unsafe.errors[0]
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code")),
        Some(&async_graphql::Value::from("invalid_transition"))
    );

    let answer_configuration = schema
        .execute(format!(
            r#"mutation {{
              answerTask(input: {{
                taskId: "{}"
                gateId: "{}"
                expectedRevision: {}
                expectedGeneration: {}
                answerMarkdown: "Use the restored route."
                clientMutationId: "answer-configuration-recovery"
              }}) {{ eventCursor }}
            }}"#,
            configuration_waiting.task.task_id,
            configuration_gate.gate_id,
            configuration_waiting.task.revision,
            configuration_waiting.task.generation,
        ))
        .await;
    assert_single_graphql_error(
        &answer_configuration,
        "requested work action is not valid now",
    );
    assert_eq!(
        answer_configuration.errors[0]
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code")),
        Some(&async_graphql::Value::from("invalid_transition"))
    );

    for (before, gate_id) in [
        (&unsafe_waiting, &unsafe_gate.gate_id),
        (&configuration_waiting, &configuration_gate.gate_id),
    ] {
        let after = store
            .get_work_task(&before.task.task_id)
            .await
            .expect("reload rejected recovery mutation")
            .expect("recovery task remains");
        assert_eq!(after.task.revision, before.task.revision);
        let gate = after.active_gate.expect("gate remains active");
        assert_eq!(&gate.gate_id, gate_id);
        assert_eq!(gate.state, TaskGateState::Open);
    }
}

#[tokio::test]
async fn populated_needs_you_and_accepted_artifact_projections_are_authoritative() {
    let store = crate::test_support::test_store().await;
    let service = WorkCommandService::new(
        store.clone(),
        crate::test_support::ready_test_provider_registry(),
    );

    let (gate_task, _, gate_fence) =
        start_seeded_executor(&store, &service, "Needs You gate task", "needs-you-gate").await;
    service
        .record_work_run_terminal(
            WorkRunTerminal::Blocked(ReportTaskBlocked {
                fence: gate_fence,
                gate_kind: TaskGateKind::Clarification,
                prompt_markdown: "Choose the exact option".to_string(),
                context_markdown: "The executor requires owner evidence".to_string(),
            }),
            "actor:agent:test",
            None,
            "correlation:needs-you-gate:blocked",
        )
        .await
        .expect("open clarification gate");

    let (review_task, contract, executor_fence) = start_seeded_executor(
        &store,
        &service,
        "Needs You review task",
        "needs-you-review",
    )
    .await;
    let executor = store
        .get_work_run_record(&executor_fence.run_id)
        .await
        .expect("load current Executor")
        .expect("current Executor exists");
    let artifact = store
        .create_artifact_with_initial_version(
            noema_artifacts::NewArtifact {
                artifact_id: Some("artifact:graphql-work-result".to_string()),
                owner: noema_artifacts::ArtifactOwnerRef::task(review_task.task_id.as_str()),
                title: "Reviewed result".to_string(),
                description: Some("Task-owned evidence".to_string()),
                artifact_kind: "document".to_string(),
                storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: executor.agent_id.clone(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({
                    "task_id": review_task.task_id.as_str(),
                    "task_run_id": executor.run_id,
                }),
            },
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: Some("artifact_version:graphql-work-result".to_string()),
                title: None,
                storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                    url: "https://example.com/reviewed-result".to_string(),
                },
                media_type: Some("text/markdown".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: executor.agent_id,
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("create task artifact");
    let executor_run_id = executor_fence.run_id.clone();
    service
        .record_work_run_terminal(
            WorkRunTerminal::TaskResult(SubmitTaskResult {
                fence: executor_fence,
                submission: NewTaskSubmission {
                    submission_id: Some("submission:graphql-work-result".to_string()),
                    task_id: review_task.task_id.clone(),
                    contract_id: contract.contract_id.clone(),
                    executor_run_id,
                    review_round: 1,
                    summary: "Reviewed result summary".to_string(),
                    result_markdown: "Complete reviewed result".to_string(),
                    criteria: contract
                        .criteria
                        .iter()
                        .map(|criterion| SubmissionCriterionEvidence {
                            criterion_id: criterion.criterion_id.clone(),
                            evidence_markdown: "Executor evidence".to_string(),
                        })
                        .collect(),
                    artifact_ids: vec![artifact.artifact.artifact_id.clone()],
                },
            }),
            "actor:agent:test",
            None,
            "correlation:needs-you-review:submission",
        )
        .await
        .expect("record task submission");

    let reviewer = service
        .claim_next_work_run("needs-you-reviewer", 60, &[])
        .await
        .expect("claim reviewer")
        .expect("queued reviewer");
    let reviewer_fence = WorkRunFence {
        run_id: reviewer.run.run_id.clone(),
        lease_token: reviewer.lease_token,
        task_generation: review_task.generation,
        contract_id: Some(contract.contract_id.clone()),
    };
    service
        .start_work_run(
            &reviewer_fence,
            "actor:agent:test",
            None,
            "correlation:needs-you-review:reviewer-start",
        )
        .await
        .expect("start reviewer");
    service
        .record_work_run_terminal(
            WorkRunTerminal::Review(SubmitTaskReview {
                fence: reviewer_fence,
                review: NewTaskReview {
                    review_id: Some("review:graphql-work-result".to_string()),
                    task_id: review_task.task_id.clone(),
                    contract_id: contract.contract_id.clone(),
                    reviewer_run_id: reviewer.run.run_id,
                    reviewed_submission_id: "submission:graphql-work-result".to_string(),
                    review_attempt_index: 1,
                    supersedes_review_id: None,
                    overall_verdict: TaskReviewVerdict::Approve,
                    human_gate_kind: None,
                    overall_feedback: "Approved with complete evidence".to_string(),
                    criteria: contract
                        .criteria
                        .iter()
                        .map(|criterion| TaskReviewCriterion {
                            criterion_id: criterion.criterion_id.clone(),
                            outcome: CriterionOutcome::Pass,
                            evidence_markdown: Some("Reviewer evidence".to_string()),
                            feedback: Some("Criterion passed".to_string()),
                        })
                        .collect(),
                },
            }),
            "actor:agent:test",
            None,
            "correlation:needs-you-review:approved",
        )
        .await
        .expect("record approved review");

    let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
    let needs_you = schema
        .execute(
            r#"query {
              needsYou(workspaceId: "workspace:personal", first: 10) {
                edges { node {
                  kind title summary validActions
                  gate { kind prompt contextMarkdown }
                  review { reviewId verdict feedback criteria { outcome evidenceMarkdown } }
                  task { taskId title revision generation stage { key } validActions }
                } }
              }
            }"#,
        )
        .await;
    assert!(needs_you.errors.is_empty(), "{:?}", needs_you.errors);
    let needs_you = needs_you.data.into_json().expect("Needs You JSON");
    let cards = needs_you["needsYou"]["edges"]
        .as_array()
        .expect("Needs You edges");
    let gate_card = cards
        .iter()
        .find(|edge| edge["node"]["task"]["taskId"] == gate_task.task_id.as_str())
        .expect("gate card");
    assert_eq!(gate_card["node"]["kind"], "CLARIFICATION_REQUIRED");
    assert_eq!(gate_card["node"]["task"]["title"], "Needs You gate task");
    assert_eq!(
        gate_card["node"]["gate"]["prompt"],
        "Choose the exact option"
    );
    assert!(gate_card["node"]["review"].is_null());
    let review_card = cards
        .iter()
        .find(|edge| edge["node"]["task"]["taskId"] == review_task.task_id.as_str())
        .expect("review-ready card");
    assert_eq!(review_card["node"]["kind"], "REVIEW_READY");
    assert_eq!(
        review_card["node"]["task"]["title"],
        "Needs You review task"
    );
    assert!(
        review_card["node"]["task"]["validActions"]
            .as_array()
            .is_some_and(|actions| actions.iter().any(|action| action == "ACCEPT"))
    );
    assert!(review_card["node"]["gate"].is_null());
    assert_eq!(review_card["node"]["review"]["verdict"], "APPROVE");
    assert_eq!(
        review_card["node"]["review"]["criteria"][0]["outcome"],
        "PASS"
    );

    let ready = store
        .get_work_task(&review_task.task_id)
        .await
        .expect("review-ready detail")
        .expect("review-ready task");
    let accepted = schema
        .execute(format!(
            r#"mutation {{
              acceptTask(input: {{
                taskId: "{}"
                expectedRevision: {}
                expectedGeneration: {}
                clientMutationId: "accept-graphql-work-result"
              }}) {{
                task {{
                  acceptedResult {{ submissionId summary artifacts {{ artifactId artifactVersionId }} }}
                  artifacts {{ edges {{ node {{ artifactId currentVersion {{ artifactVersionId }} }} }} }}
                }}
              }}
            }}"#,
            ready.task.task_id, ready.task.revision, ready.task.generation,
        ))
        .await;
    assert!(accepted.errors.is_empty(), "{:?}", accepted.errors);
    let accepted = accepted.data.into_json().expect("accepted result JSON");
    assert_eq!(
        accepted["acceptTask"]["task"]["acceptedResult"]["submissionId"],
        "submission:graphql-work-result"
    );
    assert_eq!(
        accepted["acceptTask"]["task"]["acceptedResult"]["artifacts"][0]["artifactVersionId"],
        "artifact_version:graphql-work-result"
    );
    assert_eq!(
        accepted["acceptTask"]["task"]["artifacts"]["edges"][0]["node"]["currentVersion"]["artifactVersionId"],
        "artifact_version:graphql-work-result"
    );
}
